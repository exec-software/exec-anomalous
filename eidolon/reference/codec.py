"""Strict reference codec for the experimental Eidolon draft-1 container.

This module intentionally uses only the Python standard library.  It is an
independent oracle for the Ny implementation, not production Anomalous code.
Every length is checked before allocation or slicing and every structural
header, encoded frame, and decoded frame is protected by CRC32C.
"""

from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path
import stat
import struct
from typing import Sequence


format_magic = b"EIDOLON\x00"
frame_record_magic = b"FRM1"
format_version = 1
pixel_format_rgb8 = 1

file_flag_mask = 0
frame_flag_key = 1
frame_flag_mask = frame_flag_key

mode_raw = 0
mode_solid = 1
mode_pixel_rle = 2
mode_unchanged = 3
mode_xor_rle = 4

mode_names = {
    mode_raw: "raw",
    mode_solid: "solid",
    mode_pixel_rle: "pixel_rle",
    mode_unchanged: "unchanged",
    mode_xor_rle: "xor_rle",
}

# CRC is the final field in both headers and covers every preceding byte.
file_header_prefix = struct.Struct("<8sHHIIIIHHIII")
file_header = struct.Struct("<8sHHIIIIHHIIII")
frame_header_prefix = struct.Struct("<4sHHIQIIIII")
frame_header = struct.Struct("<4sHHIQIIIIII")
tile_header = struct.Struct("<BI")
pixel_run = struct.Struct("<H3s")
byte_run = struct.Struct("<HB")
ppm_header_limit = 4096


class eidolon_error(ValueError):
    """Raised when input violates the draft format or a resource bound."""


@dataclass(frozen=True)
class resource_limits:
    """Explicit decoder budgets.

    Defaults are intentionally conservative for a host-side development tool.
    Callers processing less-trusted data should normally lower them further.
    """

    max_width: int = 16_384
    max_height: int = 16_384
    max_tile_edge: int = 256
    max_frames: int = 10_000
    max_tiles_per_frame: int = 1_048_576
    max_frame_bytes: int = 256 * 1024 * 1024
    max_payload_bytes: int = 512 * 1024 * 1024
    max_container_bytes: int = 1024 * 1024 * 1024
    max_total_decoded_bytes: int = 512 * 1024 * 1024


default_limits = resource_limits()


@dataclass(frozen=True)
class decoded_frame:
    pixels: bytes
    timestamp_ticks: int
    duration_ticks: int
    key_frame: bool
    tile_modes: tuple[int, ...]


@dataclass(frozen=True)
class decoded_trace:
    width: int
    height: int
    tile_width: int
    tile_height: int
    timebase_numerator: int
    timebase_denominator: int
    frames: tuple[decoded_frame, ...]


def _build_crc32c_table() -> tuple[int, ...]:
    polynomial = 0x82F63B78
    entries: list[int] = []
    for value in range(256):
        accumulator = value
        for _ in range(8):
            accumulator = (
                (accumulator >> 1) ^ polynomial
                if accumulator & 1
                else accumulator >> 1
            )
        entries.append(accumulator & 0xFFFFFFFF)
    return tuple(entries)


_crc32c_table = _build_crc32c_table()


def crc32c(data: bytes | bytearray | memoryview) -> int:
    """Return the Castagnoli CRC-32C of *data*."""

    accumulator = 0xFFFFFFFF
    for value in memoryview(data).cast("B"):
        accumulator = _crc32c_table[(accumulator ^ value) & 0xFF] ^ (
            accumulator >> 8
        )
    return (accumulator ^ 0xFFFFFFFF) & 0xFFFFFFFF


def _checked_product(left: int, right: int, limit: int, label: str) -> int:
    if left < 0 or right < 0:
        raise eidolon_error(f"{label} cannot be negative")
    if left != 0 and right > limit // left:
        raise eidolon_error(f"{label} exceeds its resource limit")
    return left * right


def _require_u16(value: int, label: str) -> None:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not 0 <= value <= 0xFFFF
    ):
        raise eidolon_error(f"{label} does not fit in u16")


def _require_u32(value: int, label: str) -> None:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not 0 <= value <= 0xFFFFFFFF
    ):
        raise eidolon_error(f"{label} does not fit in u32")


def _require_u64(value: int, label: str) -> None:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not 0 <= value <= 0xFFFFFFFFFFFFFFFF
    ):
        raise eidolon_error(f"{label} does not fit in u64")


def _read_regular_file(path: Path, maximum_bytes: int, label: str) -> bytes:
    """Read one stable regular-file snapshot within *maximum_bytes*.

    POSIX opens fail closed without following the final path component. Python's
    standard library does not expose the equivalent Windows reparse-point flag;
    callers on Windows receive the exact-read and metadata-stability checks but
    not the same no-follow guarantee.
    """

    if maximum_bytes < 0:
        raise eidolon_error(f"{label} has an invalid configured byte limit")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_BINARY", 0)
        | getattr(os, "O_NONBLOCK", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if os.name == "posix":
        if not hasattr(os, "O_NOFOLLOW"):
            raise eidolon_error(f"{label} no-follow reads are unavailable")
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise eidolon_error(f"cannot open {label}: {error.strerror}") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise eidolon_error(f"{label} is not a regular file")
        if before.st_size < 0 or before.st_size > maximum_bytes:
            raise eidolon_error(f"{label} exceeds its configured byte limit")

        data = bytearray(before.st_size)
        offset = 0
        while offset < len(data):
            chunk = os.read(descriptor, len(data) - offset)
            if not chunk:
                raise eidolon_error(f"{label} changed while being read")
            data[offset : offset + len(chunk)] = chunk
            offset += len(chunk)
        if os.read(descriptor, 1):
            raise eidolon_error(f"{label} changed while being read")

        after = os.fstat(descriptor)
        before_snapshot = (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        )
        after_snapshot = (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        )
        if not stat.S_ISREG(after.st_mode) or after_snapshot != before_snapshot:
            raise eidolon_error(f"{label} changed while being read")
    except OSError as error:
        raise eidolon_error(f"cannot read {label}: {error.strerror}") from error
    finally:
        os.close(descriptor)
    return bytes(data)


class _byte_reader:
    def __init__(self, data: bytes | bytearray | memoryview, label: str) -> None:
        self._data = memoryview(data).cast("B")
        self.offset = 0
        self.label = label

    @property
    def remaining(self) -> int:
        return len(self._data) - self.offset

    def take(self, count: int, what: str) -> memoryview:
        if count < 0 or count > self.remaining:
            raise eidolon_error(
                f"{self.label}: truncated {what} at byte {self.offset}"
            )
        start = self.offset
        self.offset += count
        return self._data[start : self.offset]

    def finish(self, what: str = "data") -> None:
        if self.remaining != 0:
            raise eidolon_error(
                f"{self.label}: {self.remaining} trailing byte(s) after {what}"
            )


def _tile_geometry(
    width: int,
    height: int,
    tile_width: int,
    tile_height: int,
    limits: resource_limits,
) -> tuple[int, int, int]:
    if not 1 <= width <= limits.max_width:
        raise eidolon_error("width is zero or exceeds the decoder limit")
    if not 1 <= height <= limits.max_height:
        raise eidolon_error("height is zero or exceeds the decoder limit")
    if not 1 <= tile_width <= limits.max_tile_edge:
        raise eidolon_error("tile width is zero or exceeds the decoder limit")
    if not 1 <= tile_height <= limits.max_tile_edge:
        raise eidolon_error("tile height is zero or exceeds the decoder limit")

    columns = (width + tile_width - 1) // tile_width
    rows = (height + tile_height - 1) // tile_height
    tile_count = _checked_product(
        columns, rows, limits.max_tiles_per_frame, "tile count"
    )
    return columns, rows, tile_count


def _frame_size(width: int, height: int, limits: resource_limits) -> int:
    pixels = _checked_product(width, height, limits.max_frame_bytes, "pixel count")
    return _checked_product(pixels, 3, limits.max_frame_bytes, "frame byte size")


def _extract_tile(
    frame: bytes | bytearray,
    width: int,
    height: int,
    tile_width: int,
    tile_height: int,
    tile_x: int,
    tile_y: int,
) -> bytes:
    x = tile_x * tile_width
    y = tile_y * tile_height
    actual_width = min(tile_width, width - x)
    actual_height = min(tile_height, height - y)
    row_bytes = actual_width * 3
    frame_stride = width * 3
    output = bytearray(row_bytes * actual_height)
    output_offset = 0
    for row in range(actual_height):
        source_offset = (y + row) * frame_stride + x * 3
        output[output_offset : output_offset + row_bytes] = frame[
            source_offset : source_offset + row_bytes
        ]
        output_offset += row_bytes
    return bytes(output)


def _place_tile(
    frame: bytearray,
    tile: bytes,
    width: int,
    height: int,
    tile_width: int,
    tile_height: int,
    tile_x: int,
    tile_y: int,
) -> None:
    x = tile_x * tile_width
    y = tile_y * tile_height
    actual_width = min(tile_width, width - x)
    actual_height = min(tile_height, height - y)
    row_bytes = actual_width * 3
    expected = row_bytes * actual_height
    if len(tile) != expected:
        raise eidolon_error("internal tile size mismatch")
    frame_stride = width * 3
    tile_offset = 0
    for row in range(actual_height):
        destination_offset = (y + row) * frame_stride + x * 3
        frame[destination_offset : destination_offset + row_bytes] = tile[
            tile_offset : tile_offset + row_bytes
        ]
        tile_offset += row_bytes


def _encode_pixel_rle(tile: bytes) -> bytes:
    if len(tile) % 3 != 0:
        raise eidolon_error("internal RGB tile is not pixel aligned")
    output = bytearray()
    pixel_count = len(tile) // 3
    index = 0
    while index < pixel_count:
        pixel = tile[index * 3 : index * 3 + 3]
        run = 1
        while (
            index + run < pixel_count
            and run < 0xFFFF
            and tile[(index + run) * 3 : (index + run) * 3 + 3] == pixel
        ):
            run += 1
        output.extend(pixel_run.pack(run, pixel))
        index += run
    return bytes(output)


def _decode_pixel_rle(payload: memoryview, pixel_count: int) -> bytes:
    reader = _byte_reader(payload, "pixel_rle tile")
    output = bytearray(pixel_count * 3)
    output_pixel = 0
    while reader.remaining:
        packed = reader.take(pixel_run.size, "pixel run")
        run, pixel = pixel_run.unpack(packed)
        if run == 0:
            raise eidolon_error("pixel_rle tile contains a zero-length run")
        if run > pixel_count - output_pixel:
            raise eidolon_error("pixel_rle tile exceeds its decoded size")
        start = output_pixel * 3
        output[start : start + run * 3] = pixel * run
        output_pixel += run
    if output_pixel != pixel_count:
        raise eidolon_error("pixel_rle tile does not fill its decoded size")
    return bytes(output)


def _encode_byte_rle(values: bytes) -> bytes:
    output = bytearray()
    index = 0
    while index < len(values):
        value = values[index]
        run = 1
        while (
            index + run < len(values)
            and run < 0xFFFF
            and values[index + run] == value
        ):
            run += 1
        output.extend(byte_run.pack(run, value))
        index += run
    return bytes(output)


def _decode_byte_rle(payload: memoryview, byte_count: int) -> bytes:
    reader = _byte_reader(payload, "xor_rle tile")
    output = bytearray(byte_count)
    output_offset = 0
    while reader.remaining:
        packed = reader.take(byte_run.size, "byte run")
        run, value = byte_run.unpack(packed)
        if run == 0:
            raise eidolon_error("xor_rle tile contains a zero-length run")
        if run > byte_count - output_offset:
            raise eidolon_error("xor_rle tile exceeds its decoded size")
        output[output_offset : output_offset + run] = bytes((value,)) * run
        output_offset += run
    if output_offset != byte_count:
        raise eidolon_error("xor_rle tile does not fill its decoded size")
    return bytes(output)


def _choose_tile(tile: bytes, previous: bytes | None) -> tuple[int, bytes]:
    candidates: list[tuple[int, bytes]] = [(mode_raw, tile)]

    first_pixel = tile[:3]
    if first_pixel and all(
        tile[offset : offset + 3] == first_pixel
        for offset in range(0, len(tile), 3)
    ):
        candidates.append((mode_solid, first_pixel))

    candidates.append((mode_pixel_rle, _encode_pixel_rle(tile)))

    if previous is not None:
        if tile == previous:
            candidates.append((mode_unchanged, b""))
        delta = bytes(current ^ old for current, old in zip(tile, previous))
        candidates.append((mode_xor_rle, _encode_byte_rle(delta)))

    priority = {
        mode_unchanged: 0,
        mode_solid: 1,
        mode_pixel_rle: 2,
        mode_xor_rle: 3,
        mode_raw: 4,
    }
    return min(candidates, key=lambda candidate: (len(candidate[1]), priority[candidate[0]]))


def encode_trace(
    width: int,
    height: int,
    frames: Sequence[bytes | bytearray | memoryview],
    *,
    tile_width: int = 16,
    tile_height: int = 16,
    timebase_numerator: int = 1,
    timebase_denominator: int = 1_000,
    timestamps: Sequence[int] | None = None,
    durations: Sequence[int] | None = None,
    keyframe_interval: int = 120,
    limits: resource_limits = default_limits,
) -> bytes:
    """Encode RGB8 frames into deterministic draft-1 bytes."""

    _require_u32(width, "width")
    _require_u32(height, "height")
    _require_u16(tile_width, "tile width")
    _require_u16(tile_height, "tile height")
    columns, rows, _ = _tile_geometry(
        width, height, tile_width, tile_height, limits
    )
    expected_frame_size = _frame_size(width, height, limits)

    if not 1 <= len(frames) <= limits.max_frames:
        raise eidolon_error("frame count is zero or exceeds the encoder limit")
    if (
        not isinstance(keyframe_interval, int)
        or isinstance(keyframe_interval, bool)
        or keyframe_interval < 1
    ):
        raise eidolon_error("keyframe interval must be positive")
    _require_u32(timebase_numerator, "timebase numerator")
    _require_u32(timebase_denominator, "timebase denominator")
    if timebase_numerator == 0 or timebase_denominator == 0:
        raise eidolon_error("timebase numerator and denominator must be nonzero")

    frame_count = len(frames)
    _require_u32(frame_count, "frame count")
    _checked_product(
        expected_frame_size,
        frame_count,
        limits.max_total_decoded_bytes,
        "total decoded bytes",
    )
    if timestamps is None:
        timestamps = tuple(range(frame_count))
    if durations is None:
        durations = (1,) * frame_count
    if len(timestamps) != frame_count or len(durations) != frame_count:
        raise eidolon_error("timestamp and duration counts must match frame count")

    normalized_frames: list[bytes] = []
    for index, frame in enumerate(frames):
        normalized = bytes(frame)
        if len(normalized) != expected_frame_size:
            raise eidolon_error(
                f"frame {index} has {len(normalized)} bytes; "
                f"expected {expected_frame_size}"
            )
        normalized_frames.append(normalized)

    output = bytearray()
    header_prefix = file_header_prefix.pack(
        format_magic,
        format_version,
        file_header.size,
        0,
        width,
        height,
        pixel_format_rgb8,
        tile_width,
        tile_height,
        timebase_numerator,
        timebase_denominator,
        frame_count,
    )
    output.extend(header_prefix)
    output.extend(struct.pack("<I", crc32c(header_prefix)))

    previous_timestamp = -1
    previous_frame: bytes | None = None
    for frame_index, frame in enumerate(normalized_frames):
        timestamp = timestamps[frame_index]
        duration = durations[frame_index]
        _require_u64(timestamp, f"frame {frame_index} timestamp")
        _require_u32(duration, f"frame {frame_index} duration")
        if duration == 0:
            raise eidolon_error(f"frame {frame_index} duration must be nonzero")
        if timestamp < previous_timestamp:
            raise eidolon_error("frame timestamps must be monotonic")
        previous_timestamp = timestamp

        key_frame = frame_index == 0 or frame_index % keyframe_interval == 0
        reference = None if key_frame else previous_frame
        payload = bytearray()
        for tile_y in range(rows):
            for tile_x in range(columns):
                tile = _extract_tile(
                    frame,
                    width,
                    height,
                    tile_width,
                    tile_height,
                    tile_x,
                    tile_y,
                )
                previous_tile = (
                    None
                    if reference is None
                    else _extract_tile(
                        reference,
                        width,
                        height,
                        tile_width,
                        tile_height,
                        tile_x,
                        tile_y,
                    )
                )
                mode, tile_payload = _choose_tile(tile, previous_tile)
                record_size = tile_header.size + len(tile_payload)
                if record_size > limits.max_payload_bytes - len(payload):
                    raise eidolon_error(
                        f"frame {frame_index} payload exceeds the encoder limit"
                    )
                payload.extend(tile_header.pack(mode, len(tile_payload)))
                payload.extend(tile_payload)

        if len(output) + frame_header.size + len(payload) > limits.max_container_bytes:
            raise eidolon_error("container exceeds the encoder limit")
        payload_crc = crc32c(payload)
        decoded_crc = crc32c(frame)
        frame_prefix = frame_header_prefix.pack(
            frame_record_magic,
            frame_header.size,
            frame_flag_key if key_frame else 0,
            frame_index,
            timestamp,
            duration,
            expected_frame_size,
            len(payload),
            payload_crc,
            decoded_crc,
        )
        output.extend(frame_prefix)
        output.extend(struct.pack("<I", crc32c(frame_prefix)))
        output.extend(payload)
        previous_frame = frame

    return bytes(output)


def _decode_tile(
    mode: int,
    payload: memoryview,
    expected_bytes: int,
    previous: bytes | None,
    key_frame: bool,
) -> bytes:
    if mode == mode_raw:
        if len(payload) != expected_bytes:
            raise eidolon_error("raw tile length does not match its geometry")
        return bytes(payload)
    if mode == mode_solid:
        if len(payload) != 3:
            raise eidolon_error("solid tile payload must contain one RGB8 pixel")
        return bytes(payload) * (expected_bytes // 3)
    if mode == mode_pixel_rle:
        return _decode_pixel_rle(payload, expected_bytes // 3)
    if mode == mode_unchanged:
        if key_frame or previous is None:
            raise eidolon_error("unchanged tile is invalid in a key frame")
        if len(payload) != 0:
            raise eidolon_error("unchanged tile payload must be empty")
        return previous
    if mode == mode_xor_rle:
        if key_frame or previous is None:
            raise eidolon_error("xor_rle tile is invalid in a key frame")
        delta = _decode_byte_rle(payload, expected_bytes)
        return bytes(value ^ old for value, old in zip(delta, previous))
    raise eidolon_error(f"unknown tile mode {mode}")


def decode_trace(
    data: bytes | bytearray | memoryview,
    *,
    limits: resource_limits = default_limits,
) -> decoded_trace:
    """Decode and fully validate an Eidolon draft-1 container."""

    if len(data) > limits.max_container_bytes:
        raise eidolon_error("container exceeds the decoder limit")
    reader = _byte_reader(data, "Eidolon container")
    packed_header = reader.take(file_header.size, "file header")
    (
        magic,
        version,
        header_size,
        flags,
        width,
        height,
        pixel_format,
        tile_width,
        tile_height,
        timebase_numerator,
        timebase_denominator,
        frame_count,
        header_crc,
    ) = file_header.unpack(packed_header)

    if magic != format_magic:
        raise eidolon_error("invalid Eidolon magic")
    if version != format_version:
        raise eidolon_error(f"unsupported Eidolon version {version}")
    if header_size != file_header.size:
        raise eidolon_error("invalid file header size")
    if crc32c(packed_header[:-4]) != header_crc:
        raise eidolon_error("file header CRC32C mismatch")
    if flags & ~file_flag_mask:
        raise eidolon_error("file header contains unknown flags")
    if pixel_format != pixel_format_rgb8:
        raise eidolon_error("unsupported pixel format")
    if timebase_numerator == 0 or timebase_denominator == 0:
        raise eidolon_error("timebase numerator and denominator must be nonzero")
    if not 1 <= frame_count <= limits.max_frames:
        raise eidolon_error("frame count is zero or exceeds the decoder limit")

    columns, rows, tile_count = _tile_geometry(
        width, height, tile_width, tile_height, limits
    )
    expected_frame_size = _frame_size(width, height, limits)
    _checked_product(
        expected_frame_size,
        frame_count,
        limits.max_total_decoded_bytes,
        "total decoded bytes",
    )

    frames: list[decoded_frame] = []
    previous_frame: bytes | None = None
    previous_timestamp = -1
    for expected_index in range(frame_count):
        packed_frame_header = reader.take(frame_header.size, "frame header")
        (
            frame_magic,
            frame_header_size,
            frame_flags,
            frame_index,
            timestamp,
            duration,
            uncompressed_size,
            payload_size,
            payload_crc,
            decoded_crc,
            frame_header_crc,
        ) = frame_header.unpack(packed_frame_header)

        if frame_magic != frame_record_magic:
            raise eidolon_error(f"frame {expected_index} has invalid magic")
        if frame_header_size != frame_header.size:
            raise eidolon_error(f"frame {expected_index} has invalid header size")
        if crc32c(packed_frame_header[:-4]) != frame_header_crc:
            raise eidolon_error(f"frame {expected_index} header CRC32C mismatch")
        if frame_flags & ~frame_flag_mask:
            raise eidolon_error(f"frame {expected_index} contains unknown flags")
        if frame_index != expected_index:
            raise eidolon_error(
                f"frame index {frame_index} appears where {expected_index} was required"
            )
        key_frame = bool(frame_flags & frame_flag_key)
        if expected_index == 0 and not key_frame:
            raise eidolon_error("the first frame must be a key frame")
        if duration == 0:
            raise eidolon_error(f"frame {expected_index} duration is zero")
        if timestamp < previous_timestamp:
            raise eidolon_error("frame timestamps are not monotonic")
        previous_timestamp = timestamp
        if uncompressed_size != expected_frame_size:
            raise eidolon_error(
                f"frame {expected_index} decoded size does not match file geometry"
            )
        if payload_size > limits.max_payload_bytes:
            raise eidolon_error(f"frame {expected_index} payload exceeds decoder limit")

        payload = reader.take(payload_size, f"frame {expected_index} payload")
        if crc32c(payload) != payload_crc:
            raise eidolon_error(f"frame {expected_index} payload CRC32C mismatch")
        payload_reader = _byte_reader(payload, f"frame {expected_index} payload")
        decoded = bytearray(expected_frame_size)
        modes: list[int] = []
        for tile_number in range(tile_count):
            tile_y, tile_x = divmod(tile_number, columns)
            x = tile_x * tile_width
            y = tile_y * tile_height
            actual_width = min(tile_width, width - x)
            actual_height = min(tile_height, height - y)
            expected_tile_bytes = actual_width * actual_height * 3

            packed_tile_header = payload_reader.take(
                tile_header.size, f"tile {tile_number} header"
            )
            mode, tile_payload_size = tile_header.unpack(packed_tile_header)
            tile_payload = payload_reader.take(
                tile_payload_size, f"tile {tile_number} payload"
            )
            previous_tile = (
                None
                if key_frame or previous_frame is None
                else _extract_tile(
                    previous_frame,
                    width,
                    height,
                    tile_width,
                    tile_height,
                    tile_x,
                    tile_y,
                )
            )
            tile = _decode_tile(
                mode,
                tile_payload,
                expected_tile_bytes,
                previous_tile,
                key_frame,
            )
            _place_tile(
                decoded,
                tile,
                width,
                height,
                tile_width,
                tile_height,
                tile_x,
                tile_y,
            )
            modes.append(mode)
        payload_reader.finish("tile records")

        decoded_bytes = bytes(decoded)
        if crc32c(decoded_bytes) != decoded_crc:
            raise eidolon_error(f"frame {expected_index} decoded CRC32C mismatch")
        decoded_frame_record = decoded_frame(
            pixels=decoded_bytes,
            timestamp_ticks=timestamp,
            duration_ticks=duration,
            key_frame=key_frame,
            tile_modes=tuple(modes),
        )
        frames.append(decoded_frame_record)
        previous_frame = decoded_bytes

    reader.finish("declared frames")
    return decoded_trace(
        width=width,
        height=height,
        tile_width=tile_width,
        tile_height=tile_height,
        timebase_numerator=timebase_numerator,
        timebase_denominator=timebase_denominator,
        frames=tuple(frames),
    )


def _ppm_token(data: bytes, offset: int) -> tuple[bytes, int]:
    while offset < len(data):
        if offset >= ppm_header_limit:
            raise eidolon_error("PPM header exceeds the 4096-byte limit")
        value = data[offset]
        if value in b" \t\r\n\v\f":
            offset += 1
            continue
        if value == ord("#"):
            newline = data.find(
                b"\n", offset + 1, min(len(data), ppm_header_limit)
            )
            if newline < 0:
                if len(data) >= ppm_header_limit:
                    raise eidolon_error("PPM header exceeds the 4096-byte limit")
                raise eidolon_error("unterminated PPM header comment")
            offset = newline + 1
            continue
        break
    start = offset
    while offset < len(data) and data[offset] not in b" \t\r\n\v\f#":
        if offset >= ppm_header_limit:
            raise eidolon_error("PPM header exceeds the 4096-byte limit")
        offset += 1
    if start == offset:
        raise eidolon_error("truncated PPM header")
    return data[start:offset], offset


def read_ppm(
    path: str | Path,
    *,
    limits: resource_limits = default_limits,
) -> tuple[int, int, bytes]:
    """Read the strict P6 RGB8 subset produced by QEMU ``screendump``."""

    source = Path(path)
    maximum_file_bytes = limits.max_frame_bytes + ppm_header_limit
    data = _read_regular_file(source, maximum_file_bytes, "PPM input")

    if len(data) < 2:
        raise eidolon_error("truncated PPM header")
    if data[:2] != b"P6":
        raise eidolon_error("only binary P6 PPM input is supported")
    offset = 0
    magic, offset = _ppm_token(data, offset)
    width_text, offset = _ppm_token(data, offset)
    height_text, offset = _ppm_token(data, offset)
    maximum_text, offset = _ppm_token(data, offset)
    if magic != b"P6":
        raise eidolon_error("only binary P6 PPM input is supported")
    numeric_tokens = (width_text, height_text, maximum_text)
    if any(
        not token
        or any(value < ord("0") or value > ord("9") for value in token)
        for token in numeric_tokens
    ):
        raise eidolon_error("PPM geometry contains a non-decimal value")
    width = int(width_text, 10)
    height = int(height_text, 10)
    maximum = int(maximum_text, 10)
    if maximum != 255:
        raise eidolon_error("only 8-bit PPM samples with maxval 255 are supported")
    if not 1 <= width <= limits.max_width:
        raise eidolon_error("PPM width is zero or exceeds the decoder limit")
    if not 1 <= height <= limits.max_height:
        raise eidolon_error("PPM height is zero or exceeds the decoder limit")
    expected = _frame_size(width, height, limits)
    # The mandatory raster separator is part of the bounded header. A
    # one-byte separator may end exactly at the limit; a CRLF pair must also
    # fit completely within it.
    if offset >= ppm_header_limit:
        raise eidolon_error("PPM header exceeds the 4096-byte limit")
    if offset >= len(data) or data[offset] not in b" \t\r\n\v\f":
        raise eidolon_error("PPM maxval is not followed by a raster separator")
    # CR is itself a valid one-byte separator, so a following LF may be the
    # first raster octet. Prefer that interpretation when it leaves the exact
    # declared raster; otherwise treat the pair as the accepted CRLF separator.
    if (
        data[offset] == ord("\r")
        and offset + 1 < len(data)
        and data[offset + 1] == ord("\n")
        and len(data) - (offset + 1) != expected
    ):
        offset += 2
    else:
        offset += 1
    if offset > ppm_header_limit:
        raise eidolon_error("PPM header exceeds the 4096-byte limit")
    pixels = data[offset:]
    if len(pixels) != expected:
        raise eidolon_error(
            f"PPM raster has {len(pixels)} bytes; expected exactly {expected}"
        )
    return width, height, pixels


def write_ppm(path: str | Path, width: int, height: int, pixels: bytes) -> None:
    """Write a canonical binary P6 RGB8 image."""

    expected = width * height * 3
    if width < 1 or height < 1 or len(pixels) != expected:
        raise eidolon_error("invalid PPM geometry or raster size")
    destination = Path(path)
    destination.write_bytes(f"P6\n{width} {height}\n255\n".encode("ascii") + pixels)


def mode_histogram(decoded: decoded_trace) -> dict[str, int]:
    """Return stable human-readable tile-mode counts for inspection."""

    counts = {name: 0 for name in mode_names.values()}
    for frame in decoded.frames:
        for mode in frame.tile_modes:
            counts[mode_names[mode]] += 1
    return counts
