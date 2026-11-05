from __future__ import annotations

from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

from reference.cli import _atomic_write, _decode
from reference.codec import (
    _read_regular_file,
    file_header,
    file_header_prefix,
    frame_header,
    frame_header_prefix,
    mode_pixel_rle,
    mode_raw,
    mode_unchanged,
    mode_xor_rle,
    pixel_run,
    tile_header,
    default_limits,
    eidolon_error,
    crc32c,
    decode_trace,
    encode_trace,
    read_ppm,
    write_ppm,
)


fixture_path = Path(__file__).parent / "fixtures" / "tiny_trace.json"
eidolon_root = Path(__file__).parents[1]


def _fixture() -> tuple[int, int, list[bytes], str]:
    document = json.loads(fixture_path.read_text(encoding="utf-8"))
    return (
        document["width"],
        document["height"],
        [bytes.fromhex(value) for value in document["frames_hex"]],
        document["expected_sha256"],
    )


def _replace_file_header(encoded: bytes, field: int, value: int) -> bytes:
    fields = list(file_header.unpack(encoded[: file_header.size]))
    fields[field] = value
    prefix = file_header_prefix.pack(*fields[:-1])
    return prefix + struct.pack("<I", crc32c(prefix)) + encoded[file_header.size :]


def _replace_first_frame_payload(encoded: bytes, payload: bytes) -> bytes:
    frame_offset = file_header.size
    fields = list(
        frame_header.unpack(encoded[frame_offset : frame_offset + frame_header.size])
    )
    old_payload_size = fields[7]
    fields[7] = len(payload)
    fields[8] = crc32c(payload)
    prefix = frame_header_prefix.pack(*fields[:-1])
    packed_header = prefix + struct.pack("<I", crc32c(prefix))
    old_payload_end = frame_offset + frame_header.size + old_payload_size
    return encoded[:frame_offset] + packed_header + payload + encoded[old_payload_end:]


class codec_tests(unittest.TestCase):
    def test_regular_reader_uses_exact_size_and_rejects_oversize(self) -> None:
        content = b"one stable snapshot"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.bin"
            path.write_bytes(content)
            self.assertEqual(
                _read_regular_file(path, len(content), "test input"), content
            )
            with self.assertRaisesRegex(eidolon_error, "configured byte limit"):
                _read_regular_file(path, len(content) - 1, "test input")

    def test_regular_reader_rejects_same_size_metadata_mutation(self) -> None:
        content = b"metadata must remain stable"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.bin"
            path.write_bytes(content)
            original_read = os.read
            mutated = False

            def read_then_touch(descriptor: int, count: int) -> bytes:
                nonlocal mutated
                chunk = original_read(descriptor, count)
                if not mutated:
                    mutated = True
                    information = path.stat()
                    os.utime(
                        path,
                        ns=(
                            information.st_atime_ns,
                            information.st_mtime_ns + 1_000_000_000,
                        ),
                    )
                return chunk

            with mock.patch("reference.codec.os.read", side_effect=read_then_touch):
                with self.assertRaisesRegex(eidolon_error, "changed while being read"):
                    _read_regular_file(path, len(content), "test input")

    def test_regular_reader_rejects_growth_after_exact_read(self) -> None:
        content = b"bounded before allocation"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.bin"
            path.write_bytes(content)
            original_read = os.read
            mutated = False

            def read_then_grow(descriptor: int, count: int) -> bytes:
                nonlocal mutated
                chunk = original_read(descriptor, count)
                if not mutated:
                    mutated = True
                    with path.open("ab") as output:
                        output.write(b"x")
                return chunk

            with mock.patch("reference.codec.os.read", side_effect=read_then_grow):
                with self.assertRaisesRegex(eidolon_error, "changed while being read"):
                    _read_regular_file(path, len(content), "test input")

    @unittest.skipUnless(os.name == "posix", "short-read mutation uses POSIX handles")
    def test_regular_reader_rejects_short_read_after_initial_metadata(self) -> None:
        content = b"the initial size cannot become stale"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.bin"
            path.write_bytes(content)
            original_read = os.read
            mutated = False

            def truncate_then_read(descriptor: int, count: int) -> bytes:
                nonlocal mutated
                if not mutated:
                    mutated = True
                    path.write_bytes(b"")
                return original_read(descriptor, count)

            with mock.patch("reference.codec.os.read", side_effect=truncate_then_read):
                with self.assertRaisesRegex(eidolon_error, "changed while being read"):
                    _read_regular_file(path, len(content), "test input")

    @unittest.skipUnless(
        os.name == "posix" and hasattr(os, "O_NOFOLLOW"),
        "final-component no-follow test requires POSIX O_NOFOLLOW",
    )
    def test_regular_reader_rejects_symlink_without_following(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "private-target.bin"
            target.write_bytes(b"private pixels")
            link = root / "input-link.bin"
            link.symlink_to(target)
            with self.assertRaisesRegex(eidolon_error, "cannot open"):
                _read_regular_file(link, 1024, "test input")
            self.assertEqual(target.read_bytes(), b"private pixels")

    def test_crc32c_standard_vector(self) -> None:
        self.assertEqual(crc32c(b"123456789"), 0xE3069283)

    def test_fixture_round_trip_and_golden_digest(self) -> None:
        width, height, frames, expected_sha256 = _fixture()
        encoded = encode_trace(
            width,
            height,
            frames,
            tile_width=4,
            tile_height=2,
            timestamps=(0, 16),
            durations=(16, 16),
        )
        decoded = decode_trace(encoded)
        self.assertEqual([frame.pixels for frame in decoded.frames], frames)
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), expected_sha256)

    def test_encoding_is_deterministic(self) -> None:
        width, height, frames, _ = _fixture()
        arguments = {
            "tile_width": 4,
            "tile_height": 2,
            "timestamps": (3, 19),
            "durations": (16, 16),
        }
        self.assertEqual(
            encode_trace(width, height, frames, **arguments),
            encode_trace(width, height, frames, **arguments),
        )

    def test_edge_tiles_and_temporal_modes_round_trip(self) -> None:
        width = 19
        height = 17
        first = bytes(
            component
            for y in range(height)
            for x in range(width)
            for component in ((x * 13) & 0xFF, (y * 17) & 0xFF, (x ^ y) & 0xFF)
        )
        second = bytearray(first)
        for y in range(8):
            for x in range(8):
                offset = (y * width + x) * 3
                second[offset] ^= 0xFF
        encoded = encode_trace(
            width,
            height,
            (first, bytes(second), bytes(second)),
            tile_width=8,
            tile_height=8,
            keyframe_interval=20,
        )
        decoded = decode_trace(encoded)
        self.assertEqual(
            [frame.pixels for frame in decoded.frames],
            [first, bytes(second), bytes(second)],
        )
        temporal_modes = decoded.frames[1].tile_modes + decoded.frames[2].tile_modes
        self.assertTrue(
            mode_unchanged in temporal_modes or mode_xor_rle in temporal_modes
        )

    def test_header_corruption_is_rejected(self) -> None:
        width, height, frames, _ = _fixture()
        encoded = bytearray(encode_trace(width, height, frames))
        encoded[20] ^= 1
        with self.assertRaisesRegex(eidolon_error, "header CRC32C"):
            decode_trace(encoded)

    def test_payload_corruption_is_rejected(self) -> None:
        width, height, frames, _ = _fixture()
        encoded = bytearray(encode_trace(width, height, frames))
        encoded[-1] ^= 1
        with self.assertRaisesRegex(eidolon_error, "payload CRC32C"):
            decode_trace(encoded)

    def test_decoded_crc_detects_validly_reframed_corruption(self) -> None:
        pixels = bytes(range(3 * 4 * 4))
        encoded = encode_trace(4, 4, (pixels,), tile_width=4, tile_height=4)
        payload_offset = file_header.size + frame_header.size
        frame_fields = frame_header.unpack(
            encoded[file_header.size : payload_offset]
        )
        payload_size = frame_fields[7]
        payload = bytearray(encoded[payload_offset : payload_offset + payload_size])
        self.assertEqual(payload[0], mode_raw)
        payload[-1] ^= 1
        corrupted = _replace_first_frame_payload(encoded, bytes(payload))
        with self.assertRaisesRegex(eidolon_error, "decoded CRC32C"):
            decode_trace(corrupted)

    def test_unknown_tile_mode_is_rejected_after_crc_validation(self) -> None:
        pixels = bytes(range(3 * 4 * 4))
        encoded = encode_trace(4, 4, (pixels,), tile_width=4, tile_height=4)
        payload_offset = file_header.size + frame_header.size
        frame_fields = frame_header.unpack(
            encoded[file_header.size : payload_offset]
        )
        payload_size = frame_fields[7]
        payload = bytearray(encoded[payload_offset : payload_offset + payload_size])
        payload[0] = 0xFF
        corrupted = _replace_first_frame_payload(encoded, bytes(payload))
        with self.assertRaisesRegex(eidolon_error, "unknown tile mode"):
            decode_trace(corrupted)

    def test_zero_length_xor_run_is_rejected(self) -> None:
        first = bytes((10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120))
        second = bytearray(first)
        second[0] = 11
        encoded = encode_trace(2, 2, (first, second), tile_width=2, tile_height=2)
        first_header_offset = file_header.size
        first_fields = frame_header.unpack(
            encoded[first_header_offset : first_header_offset + frame_header.size]
        )
        second_header_offset = first_header_offset + frame_header.size + first_fields[7]
        second_payload_offset = second_header_offset + frame_header.size
        second_fields = list(
            frame_header.unpack(
                encoded[second_header_offset : second_payload_offset]
            )
        )
        payload = bytearray(
            encoded[second_payload_offset : second_payload_offset + second_fields[7]]
        )
        self.assertEqual(payload[0], mode_xor_rle)
        struct.pack_into("<H", payload, 5, 0)
        second_fields[8] = crc32c(payload)
        prefix = frame_header_prefix.pack(*second_fields[:-1])
        new_header = prefix + struct.pack("<I", crc32c(prefix))
        corrupted = (
            encoded[:second_header_offset]
            + new_header
            + payload
            + encoded[second_payload_offset + second_fields[7] :]
        )
        with self.assertRaisesRegex(eidolon_error, "zero-length run"):
            decode_trace(corrupted)

    def test_unknown_file_flags_are_rejected(self) -> None:
        width, height, frames, _ = _fixture()
        encoded = encode_trace(width, height, frames)
        with self.assertRaisesRegex(eidolon_error, "unknown flags"):
            decode_trace(_replace_file_header(encoded, 3, 1))

    def test_declared_resource_limit_is_enforced(self) -> None:
        width, height, frames, _ = _fixture()
        encoded = encode_trace(width, height, frames)
        with self.assertRaisesRegex(eidolon_error, "frame count"):
            decode_trace(encoded, limits=replace(default_limits, max_frames=1))
        with self.assertRaisesRegex(eidolon_error, "container"):
            decode_trace(
                encoded,
                limits=replace(default_limits, max_container_bytes=len(encoded) - 1),
            )
        with self.assertRaisesRegex(eidolon_error, "container"):
            encode_trace(
                width,
                height,
                frames,
                limits=replace(default_limits, max_container_bytes=64),
            )

    def test_aggregate_budget_precedes_frame_materialization(self) -> None:
        class forbidden_frame:
            def __bytes__(self) -> bytes:
                raise AssertionError("an over-budget frame was materialized")

        with self.assertRaisesRegex(eidolon_error, "total decoded bytes"):
            encode_trace(
                1,
                1,
                (forbidden_frame(), forbidden_frame()),
                limits=replace(default_limits, max_total_decoded_bytes=5),
            )

    def test_encoder_rejects_non_integer_wire_values(self) -> None:
        with self.assertRaisesRegex(eidolon_error, "width.*u32"):
            encode_trace(1.0, 1, (bytes(3),))  # type: ignore[arg-type]
        with self.assertRaisesRegex(eidolon_error, "duration.*u32"):
            encode_trace(1, 1, (bytes(3),), durations=(True,))
        with self.assertRaisesRegex(eidolon_error, "keyframe interval"):
            encode_trace(
                1,
                1,
                (bytes(3),),
                keyframe_interval=1.5,  # type: ignore[arg-type]
            )

    def test_pixel_rle_run_bounds_are_rejected(self) -> None:
        pixels = bytes((1, 2, 3)) * 4
        encoded = encode_trace(2, 2, (pixels,), tile_width=2, tile_height=2)
        cases = (
            (0, "zero-length run"),
            (3, "does not fill"),
            (5, "exceeds"),
        )
        for run, message in cases:
            with self.subTest(run=run):
                run_payload = pixel_run.pack(run, b"\x01\x02\x03")
                payload = (
                    tile_header.pack(mode_pixel_rle, len(run_payload)) + run_payload
                )
                corrupted = _replace_first_frame_payload(encoded, payload)
                with self.assertRaisesRegex(eidolon_error, message):
                    decode_trace(corrupted)

    def test_truncation_and_trailing_data_are_rejected(self) -> None:
        width, height, frames, _ = _fixture()
        encoded = encode_trace(width, height, frames)
        with self.assertRaisesRegex(eidolon_error, "truncated"):
            decode_trace(encoded[:-1])
        with self.assertRaisesRegex(eidolon_error, "trailing"):
            decode_trace(encoded + b"x")

    def test_ppm_round_trip_and_comment_header(self) -> None:
        width, height, frames, _ = _fixture()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame.ppm"
            write_ppm(path, width, height, frames[0])
            self.assertEqual(read_ppm(path), (width, height, frames[0]))
            path.write_bytes(
                f"P6\n# qemu-compatible comment\n{width} {height}\n255\n".encode(
                    "ascii"
                )
                + frames[0]
            )
            self.assertEqual(read_ppm(path), (width, height, frames[0]))

    def test_ppm_noncanonical_number_and_oversized_header_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.ppm"
            path.write_bytes(b"P6\n1_0 1\n255\n" + bytes(30))
            with self.assertRaisesRegex(eidolon_error, "non-decimal"):
                read_ppm(path)
            path.write_bytes(b"P6\n#" + b"x" * 5000 + b"\n1 1\n255\n\x00\x00\x00")
            with self.assertRaisesRegex(eidolon_error, "header exceeds"):
                read_ppm(path)

    def test_ppm_decimal_overflow_retains_semantic_error_category(self) -> None:
        cases = (
            ("width", b"P6\n2147483648 1\n255\n", "width is zero or exceeds"),
            ("height", b"P6\n1 2147483648\n255\n", "height is zero or exceeds"),
            ("maximum", b"P6\n1 1\n2147483648\n", "8-bit PPM samples"),
            ("mixed", b"P6\n2147483648x 1\n255\n", "non-decimal"),
            (
                "width-and-maximum",
                b"P6\n2147483648 1\n256\n",
                "8-bit PPM samples",
            ),
            (
                "width-and-maximum-overflow",
                b"P6\n2147483648 1\n2147483648\n",
                "8-bit PPM samples",
            ),
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, header, diagnostic in cases:
                with self.subTest(case=name):
                    path = root / f"{name}.ppm"
                    path.write_bytes(header)
                    with self.assertRaisesRegex(eidolon_error, diagnostic):
                        read_ppm(path)

    def test_ppm_header_limit_includes_complete_raster_separator(self) -> None:
        prefix = b"P6\n1 1\n"
        padding = b" " * (4095 - len(prefix) - len(b"255"))
        with tempfile.TemporaryDirectory() as directory:
            accepted = Path(directory) / "accepted.ppm"
            accepted.write_bytes(prefix + padding + b"255\n\x01\x02\x03")
            self.assertEqual(read_ppm(accepted), (1, 1, b"\x01\x02\x03"))

            rejected = Path(directory) / "rejected.ppm"
            rejected.write_bytes(prefix + padding + b"255\r\n\x01\x02\x03")
            with self.assertRaisesRegex(eidolon_error, "header exceeds"):
                read_ppm(rejected)

    def test_ppm_distinguishes_cr_separator_from_crlf_by_raster_size(self) -> None:
        pixels = b"\n\x02\x03"
        documents = {
            "cr": b"P6\n1 1\n255\r" + pixels,
            "crlf": b"P6\n1 1\n255\r\n" + pixels,
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in documents.items():
                with self.subTest(separator=name):
                    path = root / f"{name}.ppm"
                    path.write_bytes(content)
                    self.assertEqual(read_ppm(path), (1, 1, pixels))

    def test_ppm_magic_starts_at_zero_and_short_headers_are_bounded(self) -> None:
        cases = (
            ("leading-space", b" P6\n1 1\n255\n\x01\x02\x03", "only binary P6"),
            ("leading-comment", b"# no\nP6\n1 1\n255\n\x01\x02\x03", "only binary P6"),
            ("empty", b"", "truncated PPM header"),
            ("one-byte", b"P", "truncated PPM header"),
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content, diagnostic in cases:
                with self.subTest(case=name):
                    path = root / f"{name}.ppm"
                    path.write_bytes(content)
                    with self.assertRaisesRegex(eidolon_error, diagnostic):
                        read_ppm(path)

    def test_ppm_unterminated_comment_limit_has_exact_status(self) -> None:
        prefix = b"P6\n#"
        below_limit = prefix + b"x" * (4095 - len(prefix))
        at_limit = prefix + b"x" * (4096 - len(prefix))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            short = root / "short-comment.ppm"
            short.write_bytes(below_limit)
            with self.assertRaisesRegex(eidolon_error, "unterminated PPM header comment"):
                read_ppm(short)

            exact = root / "exact-comment.ppm"
            exact.write_bytes(at_limit)
            with self.assertRaisesRegex(eidolon_error, "header exceeds"):
                read_ppm(exact)

    def test_ppm_whitespace_eof_distinguishes_limit_from_over_limit(self) -> None:
        exact_content = b"P6" + b" " * 4094
        over_content = exact_content + b" "
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            exact = root / "exact-whitespace.ppm"
            exact.write_bytes(exact_content)
            with self.assertRaisesRegex(eidolon_error, "truncated PPM header"):
                read_ppm(exact)

            over = root / "over-whitespace.ppm"
            over.write_bytes(over_content)
            with self.assertRaisesRegex(eidolon_error, "header exceeds"):
                read_ppm(over)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO test requires POSIX")
    def test_ppm_rejects_special_files_without_blocking(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame.fifo"
            os.mkfifo(path)
            with self.assertRaisesRegex(eidolon_error, "not a regular file"):
                read_ppm(path)

    def test_cli_failure_does_not_echo_input_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            private_path = Path(directory) / "sensitive-frame-trace.eidolon"
            rejected = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "reference.cli",
                    "inspect",
                    str(private_path),
                ],
                cwd=eidolon_root,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(rejected.returncode, 2)
            self.assertEqual(rejected.stdout, "")
            self.assertNotIn(str(private_path), rejected.stderr)
            self.assertNotIn(private_path.name, rejected.stderr)
            self.assertNotIn(private_path.stem, rejected.stderr)
            self.assertNotIn("sensitive-frame-trace", rejected.stderr)

    def test_atomic_write_staging_is_private_and_cleans_up_failures(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / "sensitive-window-title.eidolon"
            observed_sources: list[Path] = []

            def reject_publish(source: str | Path, target: str | Path) -> None:
                observed_sources.append(Path(source))
                self.assertEqual(Path(target), destination)
                if os.name == "posix":
                    self.assertEqual(
                        stat.S_IMODE(Path(source).stat().st_mode), 0o600
                    )
                raise OSError(5, "injected publication failure")

            with mock.patch("reference.cli.os.replace", side_effect=reject_publish):
                with self.assertRaises(OSError):
                    _atomic_write(destination, b"private framebuffer bytes")
            self.assertEqual(len(observed_sources), 1)
            staging = observed_sources[0]
            self.assertTrue(staging.name.startswith(".eidolon-write-"))
            self.assertNotIn(destination.name, staging.name)
            self.assertNotIn(destination.stem, staging.name)
            self.assertFalse(staging.exists())
            self.assertFalse(destination.exists())

            if os.name == "posix":
                for operation in ("fchmod", "fsync"):
                    with self.subTest(failure=operation):
                        failure = OSError(5, f"injected {operation} failure")
                        with mock.patch(
                            f"reference.cli.os.{operation}", side_effect=failure
                        ):
                            with self.assertRaises(OSError):
                                _atomic_write(
                                    destination, b"private framebuffer bytes"
                                )
                        self.assertFalse(destination.exists())
                        self.assertEqual(
                            list(root.glob(".eidolon-write-*")), []
                        )

    @unittest.skipUnless(hasattr(os, "mkfifo"), "special output test requires POSIX")
    def test_cli_encode_rejects_symlink_and_fifo_output_without_disclosure(
        self,
    ) -> None:
        width, height, frames, _ = _fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "private-frame-input.ppm"
            write_ppm(source, width, height, frames[0])
            protected = root / "protected-private-target.eidolon"
            protected.write_bytes(b"must remain unchanged")
            symlink = root / "sensitive-output-link.eidolon"
            symlink.symlink_to(protected)
            fifo = root / "sensitive-output-pipe.eidolon"
            os.mkfifo(fifo)

            for destination in (symlink, fifo):
                with self.subTest(destination=destination.name):
                    rejected = subprocess.run(
                        [
                            sys.executable,
                            "-m",
                            "reference.cli",
                            "encode",
                            str(destination),
                            str(source),
                        ],
                        cwd=eidolon_root,
                        check=False,
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(rejected.returncode, 2)
                    self.assertEqual(rejected.stdout, "")
                    self.assertIn("not a regular file", rejected.stderr)
                    for path in (source, destination, protected):
                        self.assertNotIn(str(path), rejected.stderr)
                        self.assertNotIn(path.name, rejected.stderr)
                        self.assertNotIn(path.stem, rejected.stderr)

            self.assertTrue(symlink.is_symlink())
            self.assertEqual(protected.read_bytes(), b"must remain unchanged")
            self.assertTrue(stat.S_ISFIFO(fifo.lstat().st_mode))

            regular = root / "replaceable-regular-output.eidolon"
            regular.write_bytes(b"old")
            _atomic_write(regular, b"new complete trace")
            self.assertEqual(regular.read_bytes(), b"new complete trace")
            if os.name == "posix":
                self.assertEqual(stat.S_IMODE(regular.stat().st_mode), 0o600)

    def test_decode_staging_is_private_and_cleans_up_failures(self) -> None:
        width, height, frames, _ = _fixture()
        encoded = encode_trace(width, height, frames)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "private-source.eidolon"
            source.write_bytes(encoded)
            destination = root / "sensitive-decoded-frames"
            observed_sources: list[Path] = []

            def reject_publish(staging: str | Path, target: str | Path) -> None:
                observed_sources.append(Path(staging))
                self.assertEqual(Path(target), destination)
                if os.name == "posix":
                    self.assertEqual(
                        stat.S_IMODE(Path(staging).stat().st_mode), 0o700
                    )
                    for frame_path in Path(staging).glob("frame-*.ppm"):
                        self.assertEqual(
                            stat.S_IMODE(frame_path.stat().st_mode), 0o600
                        )
                raise OSError(5, "injected publication failure")

            arguments = SimpleNamespace(input=source, output=destination)
            with mock.patch("reference.cli.os.replace", side_effect=reject_publish):
                with self.assertRaises(OSError):
                    _decode(arguments)
            self.assertEqual(len(observed_sources), 1)
            staging = observed_sources[0]
            self.assertTrue(staging.name.startswith(".eidolon-decode-"))
            self.assertNotIn(destination.name, staging.name)
            self.assertNotIn(destination.stem, staging.name)
            self.assertFalse(staging.exists())
            self.assertFalse(destination.exists())

            if os.name == "posix":
                permission_failures = (
                    (OSError(5, "injected directory chmod failure"),),
                    (
                        None,
                        OSError(5, "injected frame chmod failure"),
                    ),
                )
                for side_effects in permission_failures:
                    with self.subTest(chmod_calls=len(side_effects)):
                        with mock.patch(
                            "reference.cli.os.chmod", side_effect=side_effects
                        ):
                            with self.assertRaises(OSError):
                                _decode(arguments)
                        self.assertFalse(destination.exists())
                        self.assertEqual(
                            list(root.glob(".eidolon-decode-*")), []
                        )

            with mock.patch(
                "reference.cli.write_ppm",
                side_effect=OSError(5, "injected PPM write failure"),
            ):
                with self.assertRaises(OSError):
                    _decode(arguments)
            self.assertFalse(destination.exists())
            self.assertEqual(list(root.glob(".eidolon-decode-*")), [])

    @unittest.skipUnless(os.name == "posix", "umask mode contract requires POSIX")
    def test_cli_publication_ignores_restrictive_umask_and_leaves_no_staging(
        self,
    ) -> None:
        width, height, frames, _ = _fixture()

        def set_restrictive_umask() -> None:
            os.umask(0o777)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs: list[Path] = []
            for index, pixels in enumerate(frames):
                source = root / f"private-input-{index}.ppm"
                write_ppm(source, width, height, pixels)
                inputs.append(source)

            encoded = root / "private-trace.eidolon"
            encoded_result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "reference.cli",
                    "encode",
                    str(encoded),
                    *(str(path) for path in inputs),
                    "--tile-width",
                    "4",
                    "--tile-height",
                    "2",
                ],
                cwd=eidolon_root,
                check=False,
                capture_output=True,
                text=True,
                timeout=30,
                preexec_fn=set_restrictive_umask,
            )
            self.assertEqual(encoded_result.returncode, 0, encoded_result.stderr)
            self.assertEqual(encoded_result.stderr, "")
            self.assertEqual(stat.S_IMODE(encoded.stat().st_mode), 0o600)
            self.assertEqual(list(root.glob(".eidolon-write-*")), [])

            output = root / "private-decoded-frames"
            decoded_result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "reference.cli",
                    "decode",
                    str(encoded),
                    str(output),
                ],
                cwd=eidolon_root,
                check=False,
                capture_output=True,
                text=True,
                timeout=30,
                preexec_fn=set_restrictive_umask,
            )
            self.assertEqual(decoded_result.returncode, 0, decoded_result.stderr)
            self.assertEqual(decoded_result.stderr, "")
            self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o700)
            decoded_paths = sorted(output.glob("frame-*.ppm"))
            self.assertEqual(len(decoded_paths), len(frames))
            for path, pixels in zip(decoded_paths, frames):
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
                self.assertEqual(read_ppm(path)[2], pixels)
            self.assertEqual(list(root.glob(".eidolon-decode-*")), [])

    def test_cli_encode_inspect_decode(self) -> None:
        width, height, frames, _ = _fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = []
            for index, pixels in enumerate(frames):
                path = root / f"input-{index}.ppm"
                write_ppm(path, width, height, pixels)
                inputs.append(path)
            encoded = root / "trace.eidolon"
            subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "reference.cli",
                    "encode",
                    str(encoded),
                    *(str(path) for path in inputs),
                    "--tile-width",
                    "4",
                    "--tile-height",
                    "2",
                ],
                cwd=eidolon_root,
                check=True,
                capture_output=True,
                text=True,
            )
            inspected = subprocess.run(
                [sys.executable, "-m", "reference.cli", "inspect", str(encoded)],
                cwd=eidolon_root,
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(json.loads(inspected.stdout)["frames"], 2)
            output = root / "decoded"
            subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "reference.cli",
                    "decode",
                    str(encoded),
                    str(output),
                ],
                cwd=eidolon_root,
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(read_ppm(output / "frame-000000.ppm")[2], frames[0])
            self.assertEqual(read_ppm(output / "frame-000001.ppm")[2], frames[1])

            dangling_output = root / "dangling-output"
            dangling_output.symlink_to("missing-target", target_is_directory=True)
            rejected = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "reference.cli",
                    "decode",
                    str(encoded),
                    str(dangling_output),
                ],
                cwd=eidolon_root,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(rejected.returncode, 2)
            self.assertIn("destination already exists", rejected.stderr)
            self.assertTrue(dangling_output.is_symlink())


if __name__ == "__main__":
    unittest.main()
