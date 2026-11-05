from __future__ import annotations

import hashlib
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest import mock

from reference.codec import (
    crc32c,
    decode_trace,
    encode_trace,
    eidolon_error,
    file_header,
    file_header_prefix,
    frame_header,
    frame_header_prefix,
    mode_histogram,
    read_ppm,
    write_ppm,
)


eidolon_root = Path(__file__).parents[1]


def _resolve_tool(environment_name: str, executable_name: str) -> Path | None:
    configured = os.environ.get(environment_name)
    if configured is not None:
        try:
            candidate = Path(configured).expanduser().resolve(strict=True)
        except (OSError, RuntimeError) as error:
            raise RuntimeError(
                f"{environment_name} does not name an executable regular file"
            ) from error
        if not candidate.is_file() or not os.access(candidate, os.X_OK):
            raise RuntimeError(
                f"{environment_name} does not name an executable regular file"
            )
        return candidate

    candidates = (
        eidolon_root.parent / "runtime" / executable_name,
    )
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate.resolve()
    return None


def _resolve_stdlib() -> Path:
    configured = os.environ.get("NY_STDLIB")
    candidate = (
        Path(configured).expanduser()
        if configured is not None
        else eidolon_root.parent / "runtime" / "nystd"
    )
    try:
        resolved = candidate.resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise RuntimeError(
            "NY_STDLIB does not name a complete Ny standard-library source tree"
        ) from error
    required_sources = (
        resolved / "prelude.ny",
        resolved / "ny" / "lang" / "io" / "file.ny",
    )
    if not resolved.is_dir() or any(not source.is_file() for source in required_sources):
        raise RuntimeError(
            "NY_STDLIB does not name a complete Ny standard-library source tree"
        )
    return resolved


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


class ny_tool_resolution_tests(unittest.TestCase):
    def test_explicit_tool_path_never_falls_back(self) -> None:
        with tempfile.TemporaryDirectory(prefix="eidolon-tool-resolution-") as directory:
            root = Path(directory)
            non_executable = root / "non_executable"
            non_executable.write_text("not executable", encoding="utf-8")
            executable = root / "executable"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o700)
            broken_link = root / "broken_link"
            broken_link.symlink_to(root / "missing_target")

            invalid_values = (
                "",
                str(root / "missing"),
                str(root),
                str(non_executable),
                str(broken_link),
            )
            for configured in invalid_values:
                with self.subTest(configured=configured):
                    with mock.patch.dict(
                        os.environ, {"NY_RESOLVER_TEST": configured}, clear=False
                    ):
                        with self.assertRaisesRegex(
                            RuntimeError, "NY_RESOLVER_TEST"
                        ):
                            _resolve_tool("NY_RESOLVER_TEST", "nycompiler")

            with mock.patch.dict(
                os.environ, {"NY_RESOLVER_TEST": str(executable)}, clear=False
            ):
                self.assertEqual(
                    _resolve_tool("NY_RESOLVER_TEST", "nycompiler"),
                    executable.resolve(),
                )

    def test_explicit_stdlib_requires_core_sources(self) -> None:
        with tempfile.TemporaryDirectory(prefix="eidolon-stdlib-resolution-") as directory:
            root = Path(directory)
            with mock.patch.dict(os.environ, {"NY_STDLIB": str(root)}, clear=False):
                with self.assertRaisesRegex(RuntimeError, "NY_STDLIB"):
                    _resolve_stdlib()

            (root / "ny" / "lang" / "io").mkdir(parents=True)
            (root / "prelude.ny").write_text("", encoding="utf-8")
            (root / "ny" / "lang" / "io" / "file.ny").write_text(
                "", encoding="utf-8"
            )
            with mock.patch.dict(os.environ, {"NY_STDLIB": str(root)}, clear=False):
                self.assertEqual(_resolve_stdlib(), root.resolve())


def _compile_route(
    compiler: Path,
    stdlib: Path,
    output: Path,
    *,
    binary: bool,
    sources: tuple[Path, ...] | None = None,
) -> None:
    command = [
        str(compiler),
        "--no-stdlib",
        "--sdk-path",
        str(stdlib),
        "--no-bundle",
        "--quiet",
    ]
    if binary:
        command.append("--emit-binary-nil")
    if sources is None:
        sources = (
            eidolon_root / "src" / "cli.ny",
            eidolon_root / "src" / "format.ny",
            eidolon_root / "src" / "ppm.ny",
            eidolon_root / "src" / "codec.ny",
            eidolon_root / "src" / "limits.ny",
        )
    command.extend(("--output", str(output), *(str(source) for source in sources)))
    completed = subprocess.run(
        command,
        cwd=eidolon_root,
        check=False,
        capture_output=True,
        text=True,
        timeout=60,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "Ny Eidolon route did not compile: "
            f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
        )


def _fields(stdout: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in stdout.splitlines():
        name, separator, value = line.partition("=")
        if not separator or not name or name in fields:
            raise AssertionError(f"invalid Ny CLI field line: {line!r}")
        fields[name] = value
    return fields


def _fixture_frames() -> tuple[int, int, tuple[bytes, ...]]:
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
    return width, height, (first, bytes(second), bytes(second))


def _frame_offset(encoded: bytes, selected_index: int) -> int:
    offset = file_header.size
    for frame_index in range(selected_index):
        fields = frame_header.unpack(encoded[offset : offset + frame_header.size])
        offset += frame_header.size + fields[7]
    return offset


def _replace_frame_field(
    encoded: bytes, selected_index: int, field_index: int, value: int
) -> bytes:
    offset = _frame_offset(encoded, selected_index)
    fields = list(frame_header.unpack(encoded[offset : offset + frame_header.size]))
    fields[field_index] = value
    prefix = frame_header_prefix.pack(*fields[:-1])
    header = prefix + struct.pack("<I", crc32c(prefix))
    return encoded[:offset] + header + encoded[offset + frame_header.size :]


def _replace_file_field(encoded: bytes, field_index: int, value: int) -> bytes:
    fields = list(file_header.unpack(encoded[: file_header.size]))
    fields[field_index] = value
    prefix = file_header_prefix.pack(*fields[:-1])
    header = prefix + struct.pack("<I", crc32c(prefix))
    return header + encoded[file_header.size :]


def _replace_first_payload(encoded: bytes, payload: bytes) -> bytes:
    offset = file_header.size
    fields = list(frame_header.unpack(encoded[offset : offset + frame_header.size]))
    old_payload_size = fields[7]
    fields[7] = len(payload)
    fields[8] = crc32c(payload)
    prefix = frame_header_prefix.pack(*fields[:-1])
    header = prefix + struct.pack("<I", crc32c(prefix))
    old_end = offset + frame_header.size + old_payload_size
    return encoded[:offset] + header + payload + encoded[old_end:]


class ny_cli_tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        compiler = _resolve_tool("NY_COMPILER", "nycompiler")
        interpreter = _resolve_tool("NY_INTERPRETER", "nyinterpreter")
        if compiler is None or interpreter is None:
            raise unittest.SkipTest(
                "a project SDK or sibling Ny build is required for Ny CLI conformance"
            )
        stdlib = _resolve_stdlib()
        print(
            "Ny CLI toolchain: "
            f"nycompiler={compiler} sha256={_sha256(compiler)}; "
            f"nyinterpreter={interpreter} sha256={_sha256(interpreter)}; "
            f"nystd={stdlib}"
        )
        temporary = tempfile.TemporaryDirectory(prefix="eidolon-ny-cli-")
        cls.addClassCleanup(temporary.cleanup)
        cls.interpreter = interpreter
        cls.artifacts: dict[str, Path] = {}
        cls.timing_artifacts: dict[str, Path] = {}
        for route, binary in (("text", False), ("binary", True)):
            artifact = Path(temporary.name) / f"eidolon-cli-{route}.nil"
            _compile_route(compiler, stdlib, artifact, binary=binary)
            cls.artifacts[route] = artifact
            timing_artifact = Path(temporary.name) / f"format-timing-{route}.nil"
            _compile_route(
                compiler,
                stdlib,
                timing_artifact,
                binary=binary,
                sources=(
                    eidolon_root / "tests" / "ny" / "format_timing_probe.ny",
                    eidolon_root / "src" / "format.ny",
                    eidolon_root / "src" / "ppm.ny",
                    eidolon_root / "src" / "codec.ny",
                    eidolon_root / "src" / "limits.ny",
                ),
            )
            cls.timing_artifacts[route] = timing_artifact

    def _run(
        self, route: str, arguments: list[str], *, timeout: int = 30
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                str(self.interpreter),
                "--no-ai",
                str(self.artifacts[route]),
                "--",
                *arguments,
            ],
            cwd=eidolon_root,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def _assert_private_paths_absent(
        self, completed: subprocess.CompletedProcess[str], *paths: Path
    ) -> None:
        self.assertEqual(completed.stdout, "")
        combined = completed.stdout + completed.stderr
        for path in paths:
            for secret in (str(path), path.name, path.stem):
                self.assertNotIn(secret, combined)

    def _write_inputs(
        self, root: Path, width: int, height: int, frames: tuple[bytes, ...]
    ) -> list[Path]:
        inputs: list[Path] = []
        for index, pixels in enumerate(frames):
            path = root / f"input-{index}.ppm"
            write_ppm(path, width, height, pixels)
            inputs.append(path)
        return inputs

    def _run_timing(
        self, route: str, output: Path
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                str(self.interpreter),
                "--no-ai",
                str(self.timing_artifacts[route]),
                "--",
                str(output),
            ],
            cwd=eidolon_root,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )

    def test_encoder_supports_full_u32_timing_and_i64_timestamp_product(
        self,
    ) -> None:
        frames = (bytes((0, 1, 2)), bytes((255, 128, 64)))
        expected = encode_trace(
            1,
            1,
            frames,
            tile_width=1,
            tile_height=1,
            timebase_numerator=0xFFFFFFFF,
            timebase_denominator=0x80000000,
            timestamps=(0, 0xFFFFFFFF),
            durations=(0xFFFFFFFF, 0xFFFFFFFF),
            keyframe_interval=120,
        )
        oracle = decode_trace(expected)
        self.assertEqual(oracle.frames[1].timestamp_ticks, 0xFFFFFFFF)
        self.assertGreater(oracle.frames[1].timestamp_ticks, 0x7FFFFFFF)

        with tempfile.TemporaryDirectory(prefix="eidolon-ny-wide-encode-") as directory:
            root = Path(directory)
            route_outputs: dict[str, bytes] = {}
            for route in self.timing_artifacts:
                output = root / f"{route}.eidolon"
                completed = self._run_timing(route, output)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertEqual(completed.stderr, "")
                self.assertEqual(
                    _fields(completed.stdout),
                    {
                        "status": "0",
                        "maximum_frame_bytes": "1048576",
                        "maximum_payload_bytes": "1048576",
                        "maximum_container_bytes": "1048576",
                        "maximum_total_decoded_bytes": "1048576",
                        "payload_capacity_status": "23",
                        "ppm_input_extent_at_limit_status": "0",
                        "ppm_input_extent_over_limit_status": "8",
                        "invalid_timebase_low": "4",
                        "invalid_timebase_high": "4",
                        "invalid_denominator_low": "4",
                        "invalid_denominator_high": "4",
                        "invalid_duration_low": "4",
                        "invalid_duration_high": "4",
                    },
                )
                route_outputs[route] = output.read_bytes()
                self.assertEqual(route_outputs[route], expected)
            self.assertEqual(route_outputs["text"], route_outputs["binary"])

    def test_text_and_binary_nil_cross_encode_inspect_and_decode(self) -> None:
        width, height, frames = _fixture_frames()
        expected = encode_trace(
            width,
            height,
            frames,
            tile_width=16,
            tile_height=16,
            timebase_numerator=1,
            timebase_denominator=1000,
            timestamps=(0, 16, 32),
            durations=(16, 16, 16),
            keyframe_interval=120,
        )
        expected_trace = decode_trace(expected)
        expected_modes = mode_histogram(expected_trace)
        route_outputs: dict[str, bytes] = {}

        with tempfile.TemporaryDirectory(prefix="eidolon-ny-cli-routes-") as directory:
            root = Path(directory)
            inputs = self._write_inputs(root, width, height, frames)
            for route in self.artifacts:
                route_root = root / route
                route_root.mkdir()
                encoded = route_root / "trace.eidolon"
                encoded.write_bytes(b"stale output replaced atomically")
                completed = self._run(
                    route,
                    ["encode", str(encoded), *(str(path) for path in inputs)],
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertEqual(completed.stderr, "")
                self.assertEqual(encoded.read_bytes(), expected)
                route_outputs[route] = encoded.read_bytes()
                encode_fields = _fields(completed.stdout)
                self.assertEqual(encode_fields["operation"], "encode")
                self.assertEqual(encode_fields["status"], "ok")
                self.assertEqual(int(encode_fields["frames"]), len(frames))
                self.assertEqual(int(encode_fields["bytes"]), len(expected))
                self.assertEqual(int(encode_fields["crc32c"]), crc32c(expected))

                inspected = self._run(route, ["inspect", str(encoded)])
                self.assertEqual(inspected.returncode, 0, inspected.stderr)
                self.assertEqual(inspected.stderr, "")
                inspect_fields = _fields(inspected.stdout)
                self.assertEqual(inspect_fields["operation"], "inspect")
                self.assertEqual(inspect_fields["status"], "ok")
                self.assertEqual(int(inspect_fields["width"]), width)
                self.assertEqual(int(inspect_fields["height"]), height)
                self.assertEqual(int(inspect_fields["frames"]), len(frames))
                self.assertEqual(int(inspect_fields["tile_width"]), 16)
                self.assertEqual(int(inspect_fields["tile_height"]), 16)
                for mode_name, count in expected_modes.items():
                    self.assertEqual(int(inspect_fields[mode_name]), count)

                for index, pixels in enumerate(frames):
                    output = route_root / f"frame-{index}.ppm"
                    decoded = self._run(
                        route,
                        ["decode-frame", str(encoded), str(index), str(output)],
                    )
                    self.assertEqual(decoded.returncode, 0, decoded.stderr)
                    self.assertEqual(decoded.stderr, "")
                    self.assertEqual(read_ppm(output), (width, height, pixels))
                    expected_ppm = f"P6\n{width} {height}\n255\n".encode("ascii") + pixels
                    self.assertEqual(output.read_bytes(), expected_ppm)
                    decode_fields = _fields(decoded.stdout)
                    self.assertEqual(decode_fields["operation"], "decode-frame")
                    self.assertEqual(int(decode_fields["frame"]), index)
                    self.assertEqual(int(decode_fields["pixel_crc32c"]), crc32c(pixels))

        self.assertEqual(route_outputs["text"], route_outputs["binary"])

    def test_ppm_header_limit_matches_oracle_in_both_routes(self) -> None:
        prefix = b"P6\n1 1\n"
        padding = b" " * (4095 - len(prefix) - len(b"255"))
        accepted_ppm = prefix + padding + b"255\n\x01\x02\x03"
        rejected_ppm = prefix + padding + b"255\r\n\x01\x02\x03"

        with tempfile.TemporaryDirectory(prefix="eidolon-ny-ppm-bound-") as directory:
            root = Path(directory)
            accepted = root / "accepted.ppm"
            rejected = root / "rejected.ppm"
            accepted.write_bytes(accepted_ppm)
            rejected.write_bytes(rejected_ppm)
            self.assertEqual(read_ppm(accepted), (1, 1, b"\x01\x02\x03"))
            with self.assertRaisesRegex(eidolon_error, "header exceeds"):
                read_ppm(rejected)

            for route in self.artifacts:
                output = root / f"{route}.eidolon"
                passed = self._run(route, ["encode", str(output), str(accepted)])
                self.assertEqual(passed.returncode, 0, passed.stderr)
                output.unlink()
                failed = self._run(route, ["encode", str(output), str(rejected)])
                self.assertEqual(failed.returncode, 66)
                self.assertEqual(
                    failed.stderr,
                    "eidolon: encode failed at ppm (status 2)\n",
                )
                self.assertFalse(output.exists())

    def test_ppm_cr_separator_and_crlf_use_exact_raster_length(self) -> None:
        pixels = b"\n\x02\x03"
        documents = {
            "cr": b"P6\n1 1\n255\r" + pixels,
            "crlf": b"P6\n1 1\n255\r\n" + pixels,
        }
        expected = encode_trace(
            1,
            1,
            (pixels, pixels),
            tile_width=16,
            tile_height=16,
            timebase_numerator=1,
            timebase_denominator=1000,
            timestamps=(0, 16),
            durations=(16, 16),
            keyframe_interval=120,
        )

        with tempfile.TemporaryDirectory(prefix="eidolon-ny-ppm-crlf-") as directory:
            root = Path(directory)
            inputs: list[Path] = []
            for name, content in documents.items():
                path = root / f"{name}.ppm"
                path.write_bytes(content)
                self.assertEqual(read_ppm(path), (1, 1, pixels))
                inputs.append(path)

            for route in self.artifacts:
                output = root / f"{route}.eidolon"
                completed = self._run(
                    route,
                    ["encode", str(output), *(str(path) for path in inputs)],
                )
                self.assertEqual(completed.returncode, 0, completed)
                self.assertEqual(completed.stderr, "")
                self.assertEqual(output.read_bytes(), expected)

    def test_ppm_magic_bounds_and_comment_status_match_oracle(self) -> None:
        valid_tail = b"P6\n1 1\n255\n\x01\x02\x03"
        comment_prefix = b"P6\n#"
        cases = (
            ("leading-space", b" " + valid_tail, "only binary P6", 4),
            ("leading-comment", b"# no\n" + valid_tail, "only binary P6", 4),
            ("empty", b"", "truncated PPM header", 3),
            ("one-byte", b"P", "truncated PPM header", 3),
            (
                "short-comment",
                comment_prefix + b"x" * (4095 - len(comment_prefix)),
                "unterminated PPM header comment",
                3,
            ),
            (
                "exact-comment",
                comment_prefix + b"x" * (4096 - len(comment_prefix)),
                "header exceeds",
                2,
            ),
            (
                "exact-whitespace-eof",
                b"P6" + b" " * 4094,
                "truncated PPM header",
                3,
            ),
            (
                "over-whitespace-limit",
                b"P6" + b" " * 4095,
                "header exceeds",
                2,
            ),
            (
                "width-overflow",
                b"P6\n2147483648 1\n255\n",
                "width is zero or exceeds",
                7,
            ),
            (
                "height-overflow",
                b"P6\n1 2147483648\n255\n",
                "height is zero or exceeds",
                7,
            ),
            (
                "maximum-overflow",
                b"P6\n1 1\n2147483648\n",
                "8-bit PPM samples",
                6,
            ),
            (
                "overflow-then-nondecimal",
                b"P6\n2147483648x 1\n255\n",
                "non-decimal",
                5,
            ),
            (
                "width-and-maximum",
                b"P6\n2147483648 1\n256\n",
                "8-bit PPM samples",
                6,
            ),
            (
                "width-and-maximum-overflow",
                b"P6\n2147483648 1\n2147483648\n",
                "8-bit PPM samples",
                6,
            ),
        )
        with tempfile.TemporaryDirectory(prefix="eidolon-ny-ppm-shape-") as directory:
            root = Path(directory)
            for name, content, diagnostic, expected_status in cases:
                with self.subTest(case=name):
                    path = root / f"{name}.ppm"
                    path.write_bytes(content)
                    with self.assertRaisesRegex(eidolon_error, diagnostic):
                        read_ppm(path)
                    for route in self.artifacts:
                        output = root / f"{name}-{route}.eidolon"
                        failed = self._run(route, ["encode", str(output), str(path)])
                        self.assertEqual(failed.returncode, 66)
                        self.assertEqual(
                            failed.stderr,
                            "eidolon: encode failed at ppm "
                            f"(status {expected_status})\n",
                        )
                        self.assertEqual(failed.stdout, "")
                        self.assertFalse(output.exists())

    def test_hostile_paths_and_failed_decode_never_publish_or_disclose(self) -> None:
        width, height, frames = _fixture_frames()
        expected = encode_trace(
            width,
            height,
            frames,
            tile_width=16,
            tile_height=16,
            timestamps=(0, 16, 32),
            durations=(16, 16, 16),
        )
        with tempfile.TemporaryDirectory(prefix="eidolon-ny-private-") as directory:
            root = Path(directory)
            private_container = root / "sensitive-window-title.eidolon"
            private_container.write_bytes(expected)
            linked_container = root / "private-container-link.eidolon"
            linked_container.symlink_to(private_container)

            for route in self.artifacts:
                rejected_link = self._run(route, ["inspect", str(linked_container)])
                self.assertEqual(rejected_link.returncode, 66)
                self._assert_private_paths_absent(
                    rejected_link, linked_container, private_container
                )

                corrupt = root / f"private-corrupt-{route}.eidolon"
                corrupt.write_bytes(expected[:-1])
                absent_output = root / f"absent-{route}.ppm"
                rejected_decode = self._run(
                    route,
                    ["decode-frame", str(corrupt), "0", str(absent_output)],
                )
                self.assertEqual(rejected_decode.returncode, 67)
                self.assertFalse(absent_output.exists())
                self._assert_private_paths_absent(
                    rejected_decode, corrupt, absent_output
                )

                existing_output = root / f"existing-{route}.ppm"
                existing_output.write_bytes(b"do not replace")
                rejected_existing = self._run(
                    route,
                    [
                        "decode-frame",
                        str(private_container),
                        "0",
                        str(existing_output),
                    ],
                )
                self.assertEqual(rejected_existing.returncode, 68)
                self.assertEqual(existing_output.read_bytes(), b"do not replace")
                self._assert_private_paths_absent(
                    rejected_existing, private_container, existing_output
                )

                dangling_output = root / f"dangling-{route}.ppm"
                dangling_output.symlink_to("missing-private-target")
                rejected_dangling = self._run(
                    route,
                    [
                        "decode-frame",
                        str(private_container),
                        "0",
                        str(dangling_output),
                    ],
                )
                self.assertEqual(rejected_dangling.returncode, 68)
                self.assertTrue(dangling_output.is_symlink())
                self.assertEqual(os.readlink(dangling_output), "missing-private-target")
                self._assert_private_paths_absent(
                    rejected_dangling,
                    private_container,
                    dangling_output,
                    Path("missing-private-target"),
                )

                protected_target = root / f"protected-target-{route}"
                protected_target.write_bytes(b"must remain untouched")
                replace_link = root / f"replace-link-{route}.eidolon"
                replace_link.symlink_to(protected_target)
                input_ppm = root / f"replace-input-{route}.ppm"
                write_ppm(input_ppm, width, height, frames[0])
                replaced = self._run(
                    route,
                    ["encode", str(replace_link), str(input_ppm)],
                )
                self.assertEqual(replaced.returncode, 68)
                self.assertTrue(replace_link.is_symlink())
                self.assertEqual(os.readlink(replace_link), str(protected_target))
                self.assertEqual(protected_target.read_bytes(), b"must remain untouched")
                self._assert_private_paths_absent(
                    replaced, replace_link, protected_target, input_ppm
                )

    def test_interpreter_profile_rejects_container_above_one_mib(self) -> None:
        with tempfile.TemporaryDirectory(prefix="eidolon-ny-budget-") as directory:
            root = Path(directory)
            oversized = root / "private-oversized-container.eidolon"
            oversized.write_bytes(b"x" * (1024 * 1024 + 1))
            for route in self.artifacts:
                completed = self._run(route, ["inspect", str(oversized)])
                self.assertEqual(completed.returncode, 66)
                self.assertEqual(
                    completed.stderr,
                    "eidolon: inspect failed at read (status 1)\n",
                )
                self._assert_private_paths_absent(completed, oversized)

    def test_complete_container_rejections_match_the_python_oracle(self) -> None:
        width, height, frames = _fixture_frames()
        encoded = encode_trace(
            width,
            height,
            frames,
            tile_width=16,
            tile_height=16,
            timestamps=(0, 16, 32),
            durations=(16, 16, 16),
        )
        payload_offset = file_header.size + frame_header.size
        first_fields = frame_header.unpack(
            encoded[file_header.size : payload_offset]
        )
        first_payload = bytearray(
            encoded[payload_offset : payload_offset + first_fields[7]]
        )
        first_payload[0] = 255

        bad_file_crc = bytearray(encoded)
        bad_file_crc[20] ^= 1
        bad_payload_crc = bytearray(encoded)
        bad_payload_crc[-1] ^= 1
        cases = {
            "empty": (b"", 13),
            "tiny-file-header": (encoded[:47], 13),
            "missing-frame-header": (encoded[: file_header.size], 13),
            "truncated-frame-header": (
                encoded[: file_header.size + frame_header.size - 1],
                13,
            ),
            "file-crc": (bytes(bad_file_crc), 9),
            "file-flags": (_replace_file_field(encoded, 3, 1), 10),
            "zero-width": (_replace_file_field(encoded, 4, 0), 1),
            "maximum-width": (_replace_file_field(encoded, 4, 0xFFFFFFFF), 1),
            "zero-height": (_replace_file_field(encoded, 5, 0), 1),
            "maximum-height": (_replace_file_field(encoded, 5, 0xFFFFFFFF), 1),
            "zero-frame-count": (_replace_file_field(encoded, 11, 0), 2),
            "maximum-frame-count": (
                _replace_file_field(encoded, 11, 0xFFFFFFFF),
                2,
            ),
            "timebase-before-frame-count-and-geometry": (
                _replace_file_field(
                    _replace_file_field(
                        _replace_file_field(encoded, 4, 0), 11, 0
                    ),
                    9,
                    0,
                ),
                12,
            ),
            "frame-count-before-geometry": (
                _replace_file_field(_replace_file_field(encoded, 4, 0), 11, 0),
                2,
            ),
            "payload-crc": (bytes(bad_payload_crc), 24),
            "frame-flags": (_replace_frame_field(encoded, 0, 2, 2), 17),
            "zero-duration": (_replace_frame_field(encoded, 0, 5, 0), 20),
            "uncompressed-size": (
                _replace_frame_field(encoded, 0, 6, width * height * 3 + 1),
                22,
            ),
            "maximum-payload-size": (
                _replace_frame_field(encoded, 0, 7, 0xFFFFFFFF),
                23,
            ),
            "unknown-mode": (_replace_first_payload(encoded, bytes(first_payload)), 25),
            "decoded-crc": (
                _replace_frame_field(encoded, 0, 9, first_fields[9] ^ 1),
                26,
            ),
            "timestamp-regression": (_replace_frame_field(encoded, 0, 4, 17), 21),
            "zero-duration-before-timestamp-regression": (
                _replace_frame_field(
                    _replace_frame_field(encoded, 2, 4, 0), 2, 5, 0
                ),
                20,
            ),
            "trailing-data": (encoded + b"x", 27),
        }
        precedence_diagnostics = {
            "timebase-before-frame-count-and-geometry": "timebase numerator",
            "frame-count-before-geometry": "frame count is zero",
            "zero-duration-before-timestamp-regression": "duration is zero",
        }

        with tempfile.TemporaryDirectory(prefix="eidolon-ny-mutations-") as directory:
            root = Path(directory)
            for case_name, (mutation, expected_status) in cases.items():
                with self.subTest(case=case_name):
                    diagnostic = precedence_diagnostics.get(case_name)
                    if diagnostic is None:
                        with self.assertRaises(eidolon_error):
                            decode_trace(mutation)
                    else:
                        with self.assertRaisesRegex(eidolon_error, diagnostic):
                            decode_trace(mutation)
                    path = root / f"private-{case_name}.eidolon"
                    path.write_bytes(mutation)
                    for route in self.artifacts:
                        completed = self._run(route, ["inspect", str(path)])
                        self.assertEqual(completed.returncode, 67)
                        self.assertEqual(
                            completed.stderr,
                            "eidolon: inspect failed at container "
                            f"(status {expected_status})\n",
                        )
                        self._assert_private_paths_absent(completed, path)

    def test_decoder_accepts_full_u64_timestamps_and_u32_timing(self) -> None:
        width, height, frames = _fixture_frames()
        timestamps = (
            0x8000000000000000,
            0x8000000000000010,
            0xFFFFFFFFFFFFFFFF,
        )
        durations = (0xFFFFFFFF, 1, 0x80000000)
        encoded = encode_trace(
            width,
            height,
            frames,
            tile_width=8,
            tile_height=7,
            timebase_numerator=0xFFFFFFFF,
            timebase_denominator=0x80000000,
            timestamps=timestamps,
            durations=durations,
            keyframe_interval=2,
        )
        oracle = decode_trace(encoded)
        expected_modes = mode_histogram(oracle)

        with tempfile.TemporaryDirectory(prefix="eidolon-ny-wire-widths-") as directory:
            root = Path(directory)
            container = root / "wide-timing.eidolon"
            container.write_bytes(encoded)
            for route in self.artifacts:
                inspected = self._run(route, ["inspect", str(container)])
                self.assertEqual(inspected.returncode, 0, inspected.stderr)
                fields = _fields(inspected.stdout)
                self.assertEqual(int(fields["timebase_numerator"]), 0xFFFFFFFF)
                self.assertEqual(int(fields["timebase_denominator"]), 0x80000000)
                self.assertEqual(int(fields["tile_width"]), 8)
                self.assertEqual(int(fields["tile_height"]), 7)
                for mode_name, count in expected_modes.items():
                    self.assertEqual(int(fields[mode_name]), count)

                output = root / f"wide-timing-{route}.ppm"
                decoded = self._run(
                    route,
                    ["decode-frame", str(container), "2", str(output)],
                )
                self.assertEqual(decoded.returncode, 0, decoded.stderr)
                self.assertEqual(read_ppm(output), (width, height, frames[2]))

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO test requires POSIX")
    def test_fifo_input_is_rejected_without_blocking(self) -> None:
        with tempfile.TemporaryDirectory(prefix="eidolon-ny-fifo-") as directory:
            root = Path(directory)
            fifo = root / "sensitive-capture.fifo"
            os.mkfifo(fifo)
            for route in self.artifacts:
                output = root / f"{route}.eidolon"
                completed = self._run(
                    route,
                    ["encode", str(output), str(fifo)],
                    timeout=5,
                )
                self.assertEqual(completed.returncode, 66)
                self.assertEqual(
                    completed.stderr,
                    "eidolon: encode failed at read (status 1)\n",
                )
                self.assertFalse(output.exists())
                self._assert_private_paths_absent(completed, fifo, output)


if __name__ == "__main__":
    unittest.main()
