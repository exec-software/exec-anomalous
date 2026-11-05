"""Command-line oracle for the experimental Eidolon draft format."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile

from .codec import (
    _read_regular_file,
    default_limits,
    decode_trace,
    eidolon_error,
    encode_trace,
    mode_histogram,
    read_ppm,
    write_ppm,
)


def _atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        destination = path.lstat()
    except FileNotFoundError:
        destination = None
    except OSError as error:
        raise eidolon_error("cannot inspect encode destination") from error
    if destination is not None and not stat.S_ISREG(destination.st_mode):
        raise eidolon_error("encode destination is not a regular file")

    descriptor, temporary_name = tempfile.mkstemp(
        prefix=".eidolon-write-", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        if os.name == "posix":
            os.fchmod(descriptor, 0o600)
        output_stream = os.fdopen(descriptor, "wb")
        descriptor = -1
        with output_stream as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    except BaseException:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError:
                pass
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            # Preserve the publication failure. Retention policy must also
            # handle rare cleanup failures and uncatchable termination.
            pass
        raise


def _read_container(path: Path) -> bytes:
    return _read_regular_file(
        path, default_limits.max_container_bytes, "Eidolon container input"
    )


def _encode(arguments: argparse.Namespace) -> int:
    frame_count = len(arguments.inputs)
    if not 1 <= frame_count <= default_limits.max_frames:
        raise eidolon_error("input frame count exceeds the encoder limit")
    images = [read_ppm(arguments.inputs[0])]
    width, height, _ = images[0]
    frame_bytes = len(images[0][2])
    if frame_bytes > default_limits.max_total_decoded_bytes // frame_count:
        raise eidolon_error("input frames exceed the total decoded-byte limit")
    for index, path in enumerate(arguments.inputs[1:], start=1):
        candidate = read_ppm(path)
        candidate_width, candidate_height, _ = candidate
        if (candidate_width, candidate_height) != (width, height):
            raise eidolon_error(f"PPM geometry differs at input frame {index}")
        images.append(candidate)
    frames = [pixels for _, _, pixels in images]
    duration = arguments.duration
    timestamps = [index * duration for index in range(len(frames))]
    encoded = encode_trace(
        width,
        height,
        frames,
        tile_width=arguments.tile_width,
        tile_height=arguments.tile_height,
        timebase_numerator=arguments.timebase_numerator,
        timebase_denominator=arguments.timebase_denominator,
        timestamps=timestamps,
        durations=[duration] * len(frames),
        keyframe_interval=arguments.keyframe_interval,
    )
    _atomic_write(arguments.output, encoded)
    print(
        json.dumps(
            {
                "bytes": len(encoded),
                "frames": len(frames),
                "height": height,
                "output": str(arguments.output),
                "sha256": hashlib.sha256(encoded).hexdigest(),
                "width": width,
            },
            sort_keys=True,
        )
    )
    return 0


def _decode(arguments: argparse.Namespace) -> int:
    encoded = _read_container(arguments.input)
    trace = decode_trace(encoded)
    if arguments.output.exists() or arguments.output.is_symlink():
        raise eidolon_error("decode destination already exists")
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(prefix=".eidolon-decode-", dir=arguments.output.parent)
    )
    try:
        if os.name == "posix":
            os.chmod(staging, 0o700)
    except BaseException:
        try:
            staging.rmdir()
        except OSError:
            pass
        raise
    try:
        for index, frame in enumerate(trace.frames):
            frame_path = staging / f"frame-{index:06d}.ppm"
            write_ppm(frame_path, trace.width, trace.height, frame.pixels)
            if os.name == "posix":
                os.chmod(frame_path, 0o600)
        os.replace(staging, arguments.output)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    print(
        json.dumps(
            {
                "frames": len(trace.frames),
                "height": trace.height,
                "output": str(arguments.output),
                "width": trace.width,
            },
            sort_keys=True,
        )
    )
    return 0


def _inspect(arguments: argparse.Namespace) -> int:
    encoded = _read_container(arguments.input)
    trace = decode_trace(encoded)
    print(
        json.dumps(
            {
                "bytes": len(encoded),
                "frames": len(trace.frames),
                "height": trace.height,
                "modes": mode_histogram(trace),
                "sha256": hashlib.sha256(encoded).hexdigest(),
                "tile_height": trace.tile_height,
                "tile_width": trace.tile_width,
                "timebase_denominator": trace.timebase_denominator,
                "timebase_numerator": trace.timebase_numerator,
                "width": trace.width,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="eidolon-reference",
        description="Strict dependency-free oracle for Eidolon draft 1",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    encode_parser = subparsers.add_parser("encode", help="encode P6 PPM frames")
    encode_parser.add_argument("output", type=Path)
    encode_parser.add_argument("inputs", type=Path, nargs="+")
    encode_parser.add_argument("--tile-width", type=int, default=16)
    encode_parser.add_argument("--tile-height", type=int, default=16)
    encode_parser.add_argument("--keyframe-interval", type=int, default=120)
    encode_parser.add_argument("--timebase-numerator", type=int, default=1)
    encode_parser.add_argument("--timebase-denominator", type=int, default=1_000)
    encode_parser.add_argument("--duration", type=int, default=16)
    encode_parser.set_defaults(handler=_encode)

    decode_parser = subparsers.add_parser("decode", help="decode to P6 PPM frames")
    decode_parser.add_argument("input", type=Path)
    decode_parser.add_argument("output", type=Path)
    decode_parser.set_defaults(handler=_decode)

    inspect_parser = subparsers.add_parser("inspect", help="validate and describe a trace")
    inspect_parser.add_argument("input", type=Path)
    inspect_parser.set_defaults(handler=_inspect)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _parser()
    arguments = parser.parse_args(argv)
    try:
        return arguments.handler(arguments)
    except eidolon_error as error:
        parser.exit(2, f"eidolon-reference: {error}\n")
    except OSError as error:
        parser.exit(
            2,
            f"eidolon-reference: operating-system failure (errno {error.errno})\n",
        )


if __name__ == "__main__":
    sys.exit(main())
