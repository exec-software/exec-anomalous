from __future__ import annotations

import hashlib
import json
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
    file_header,
    frame_header,
    mode_pixel_rle,
    pixel_run,
    tile_header,
)


eidolon_root = Path(__file__).parents[1]
fixture_path = Path(__file__).parent / "fixtures" / "tiny_trace.json"

block_names = {
    "KEY_PAYLOAD",
    "KEY_MODES",
    "DELTA_PAYLOAD",
    "DELTA_MODES",
    "RLE_PAYLOAD",
    "RLE_MODES",
    "DECODED_KEY",
    "DECODED_DELTA",
    "DECODED_RLE",
}

scalar_names = {
    "CRC32C_STANDARD",
    "KEY_STATUS",
    "KEY_PAYLOAD_CRC32C",
    "DELTA_STATUS",
    "DELTA_PAYLOAD_CRC32C",
    "RLE_STATUS",
    "DECODED_KEY_STATUS",
    "DECODED_DELTA_STATUS",
    "DECODED_RLE_STATUS",
    "MALFORMED_RAW_STATUS",
    "ZERO_RUN_STATUS",
    "RUN_OVERFLOW_STATUS",
    "UNKNOWN_MODE_STATUS",
    "TRAILING_STATUS",
    "TEMPORAL_KEY_STATUS",
    "ENCODE_FRAME_BUDGET_STATUS",
    "ENCODE_CAPACITY_BUDGET_STATUS",
    "DECODE_FRAME_BUDGET_STATUS",
    "DECODE_PAYLOAD_BUDGET_STATUS",
    "DECODE_MINIMUM_HEADER_STATUS",
}

llvm_unavailable_markers = (
    "native backend (llvm) is not compiled",
    "llvm native codegen disabled",
)


def _fixture() -> tuple[int, int, list[bytes]]:
    document = json.loads(fixture_path.read_text(encoding="utf-8"))
    return (
        document["width"],
        document["height"],
        [bytes.fromhex(value) for value in document["frames_hex"]],
    )


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


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


class ny_codec_tool_resolution_tests(unittest.TestCase):
    def test_explicit_tool_path_never_falls_back(self) -> None:
        with tempfile.TemporaryDirectory(prefix="eidolon-codec-tool-") as directory:
            root = Path(directory)
            executable = root / "nycompiler"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o700)

            for configured in ("", str(root), str(root / "missing")):
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


def _parse_evidence(stdout: str) -> dict[str, int | bytes | tuple[int, ...]]:
    lines = stdout.splitlines()
    parsed: dict[str, int | bytes | tuple[int, ...]] = {}
    offset = 0
    while offset < len(lines):
        label = lines[offset]
        offset += 1
        if label in parsed:
            raise AssertionError(f"Ny evidence repeats {label}")
        if label not in block_names and label not in scalar_names:
            raise AssertionError(f"Ny evidence contains unknown label {label!r}")
        if offset >= len(lines):
            raise AssertionError(f"Ny evidence omits the value for {label}")
        value = int(lines[offset], 10)
        offset += 1
        if label in block_names:
            if value < 0 or value > len(lines) - offset:
                raise AssertionError(f"Ny evidence gives an invalid length for {label}")
            values = tuple(int(item, 10) for item in lines[offset : offset + value])
            if any(item < 0 or item > 255 for item in values):
                raise AssertionError(f"Ny evidence gives a non-octet in {label}")
            offset += value
            parsed[label] = values if label.endswith("_MODES") else bytes(values)
        else:
            parsed[label] = value

    expected_names = block_names | scalar_names
    if parsed.keys() != expected_names:
        missing = sorted(expected_names - parsed.keys())
        raise AssertionError(f"Ny evidence omits labels: {missing}")
    return parsed


def _oracle_payloads() -> tuple[list[bytes], list[tuple[int, ...]], list[bytes]]:
    width, height, frames = _fixture()
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
    payloads: list[bytes] = []
    offset = file_header.size
    for _ in frames:
        fields = frame_header.unpack(encoded[offset : offset + frame_header.size])
        offset += frame_header.size
        payload_size = fields[7]
        payloads.append(encoded[offset : offset + payload_size])
        offset += payload_size
    return payloads, [frame.tile_modes for frame in decoded.frames], frames


def _run_ny_route(
    compiler: Path, interpreter: Path, directory: Path, binary: bool
) -> dict[str, int | bytes | tuple[int, ...]]:
    route_name = "binary" if binary else "text"
    output = directory / f"eidolon-{route_name}.nil"
    command = [
        str(compiler),
        "--no-stdlib",
        "--no-bundle",
    ]
    if binary:
        command.append("--emit-binary-nil")
    command.extend(
        (
            "-o",
            str(output),
            str(eidolon_root / "src" / "main.ny"),
            str(eidolon_root / "src" / "codec.ny"),
            str(eidolon_root / "src" / "limits.ny"),
        )
    )
    subprocess.run(command, cwd=eidolon_root, check=True, capture_output=True, text=True)
    completed = subprocess.run(
        [str(interpreter), str(output)],
        cwd=eidolon_root,
        check=True,
        capture_output=True,
        text=True,
    )
    if completed.stderr:
        raise AssertionError(f"{route_name} nyinterpreter wrote stderr: {completed.stderr}")
    return _parse_evidence(completed.stdout)


def _llvm_is_unavailable(completed: subprocess.CompletedProcess[str]) -> bool:
    diagnostic = (completed.stdout + "\n" + completed.stderr).lower()
    return any(marker in diagnostic for marker in llvm_unavailable_markers)


def _run_native_routes(
    compiler: Path, directory: Path
) -> tuple[
    dict[str, int | bytes | tuple[int, ...]] | None,
    dict[str, int | bytes | tuple[int, ...]] | None,
    str | None,
]:
    sources = (
        str(eidolon_root / "src" / "main.ny"),
        str(eidolon_root / "src" / "codec.ny"),
        str(eidolon_root / "src" / "limits.ny"),
    )
    jit_output = directory / "eidolon-jit.nil"
    jit = subprocess.run(
        [
            str(compiler),
            "--no-stdlib",
            "--native",
            "--jit",
            "--quiet",
            "--output",
            str(jit_output),
            *sources,
        ],
        cwd=eidolon_root,
        check=False,
        capture_output=True,
        text=True,
        timeout=60,
    )
    if _llvm_is_unavailable(jit):
        return None, None, "nycompiler was built without a usable LLVM backend"
    if jit.returncode != 0 or jit.stderr:
        raise AssertionError(
            "Ny LLVM JIT failed: "
            f"exit={jit.returncode} stdout={jit.stdout!r} stderr={jit.stderr!r}"
        )
    jit_evidence = _parse_evidence(jit.stdout)

    output_base = directory / "eidolon-aot"
    executable = output_base.with_suffix(".exe") if os.name == "nt" else output_base
    aot = subprocess.run(
        [
            str(compiler),
            "--no-stdlib",
            "--native",
            "--quiet",
            "--output",
            str(output_base),
            *sources,
        ],
        cwd=eidolon_root,
        check=False,
        capture_output=True,
        text=True,
        timeout=60,
    )
    if _llvm_is_unavailable(aot):
        return jit_evidence, None, "nycompiler AOT backend is unavailable"
    if aot.returncode != 0 or aot.stdout or aot.stderr:
        raise AssertionError(
            "Ny LLVM AOT compile failed: "
            f"exit={aot.returncode} stdout={aot.stdout!r} stderr={aot.stderr!r}"
        )
    completed = subprocess.run(
        [str(executable)],
        cwd=eidolon_root,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if completed.returncode != 0 or completed.stderr:
        raise AssertionError(
            "Ny LLVM AOT executable failed: "
            f"exit={completed.returncode} stdout={completed.stdout!r} "
            f"stderr={completed.stderr!r}"
        )
    return jit_evidence, _parse_evidence(completed.stdout), None


class ny_codec_tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        compiler = _resolve_tool("NY_COMPILER", "nycompiler")
        interpreter = _resolve_tool("NY_INTERPRETER", "nyinterpreter")
        if compiler is None or interpreter is None:
            raise unittest.SkipTest(
                "a project SDK or sibling Ny build is required for Ny conformance"
            )
        print(
            "Ny codec toolchain: "
            f"nycompiler={compiler} sha256={_sha256(compiler)}; "
            f"nyinterpreter={interpreter} sha256={_sha256(interpreter)}"
        )
        temporary = tempfile.TemporaryDirectory(prefix="eidolon-ny-")
        cls.addClassCleanup(temporary.cleanup)
        directory = Path(temporary.name)
        cls.text_evidence = _run_ny_route(compiler, interpreter, directory, False)
        cls.binary_evidence = _run_ny_route(compiler, interpreter, directory, True)
        (
            cls.jit_evidence,
            cls.aot_evidence,
            cls.native_skip_reason,
        ) = _run_native_routes(compiler, directory)

    def test_text_and_binary_nil_are_identical(self) -> None:
        self.assertEqual(self.text_evidence, self.binary_evidence)

    def test_llvm_jit_matches_the_interpreted_oracle(self) -> None:
        if self.jit_evidence is None:
            self.skipTest(self.native_skip_reason or "LLVM JIT unavailable")
        self.assertEqual(self.jit_evidence, self.binary_evidence)

    def test_llvm_aot_matches_the_interpreted_oracle(self) -> None:
        if self.aot_evidence is None:
            self.skipTest(self.native_skip_reason or "LLVM AOT unavailable")
        self.assertEqual(self.aot_evidence, self.binary_evidence)

    def test_crc32c_matches_oracle(self) -> None:
        evidence = self.binary_evidence
        self.assertEqual(evidence["CRC32C_STANDARD"], crc32c(b"123456789"))
        self.assertEqual(evidence["CRC32C_STANDARD"], 0xE3069283)
        self.assertEqual(
            evidence["KEY_PAYLOAD_CRC32C"], crc32c(evidence["KEY_PAYLOAD"])
        )
        self.assertEqual(
            evidence["DELTA_PAYLOAD_CRC32C"], crc32c(evidence["DELTA_PAYLOAD"])
        )

    def test_key_and_delta_payloads_match_oracle_byte_for_byte(self) -> None:
        payloads, modes, _ = _oracle_payloads()
        evidence = self.binary_evidence
        self.assertEqual(evidence["KEY_PAYLOAD"], payloads[0])
        self.assertEqual(evidence["DELTA_PAYLOAD"], payloads[1])
        self.assertEqual(evidence["KEY_MODES"], modes[0])
        self.assertEqual(evidence["DELTA_MODES"], modes[1])
        self.assertEqual(evidence["KEY_STATUS"], 0)
        self.assertEqual(evidence["DELTA_STATUS"], 0)

    def test_pixel_rle_payload_matches_oracle_layout(self) -> None:
        pixel_payload = pixel_run.pack(3, b"\x01\x02\x03") + pixel_run.pack(
            1, b"\x04\x05\x06"
        )
        expected = tile_header.pack(mode_pixel_rle, len(pixel_payload)) + pixel_payload
        self.assertEqual(self.binary_evidence["RLE_PAYLOAD"], expected)
        self.assertEqual(self.binary_evidence["RLE_MODES"], (mode_pixel_rle,))
        self.assertEqual(self.binary_evidence["RLE_STATUS"], 0)

    def test_ny_decoder_reconstructs_exact_oracle_frames(self) -> None:
        _, _, frames = _oracle_payloads()
        evidence = self.binary_evidence
        self.assertEqual(evidence["DECODED_KEY"], frames[0])
        self.assertEqual(evidence["DECODED_DELTA"], frames[1])
        self.assertEqual(
            evidence["DECODED_RLE"],
            bytes((1, 2, 3, 1, 2, 3, 1, 2, 3, 4, 5, 6)),
        )
        self.assertEqual(evidence["DECODED_KEY_STATUS"], 0)
        self.assertEqual(evidence["DECODED_DELTA_STATUS"], 0)
        self.assertEqual(evidence["DECODED_RLE_STATUS"], 0)

    def test_ny_decoder_rejects_malformed_tile_records(self) -> None:
        evidence = self.binary_evidence
        self.assertEqual(evidence["MALFORMED_RAW_STATUS"], 6)
        self.assertEqual(evidence["ZERO_RUN_STATUS"], 7)
        self.assertEqual(evidence["RUN_OVERFLOW_STATUS"], 8)
        self.assertEqual(evidence["UNKNOWN_MODE_STATUS"], 5)
        self.assertEqual(evidence["TRAILING_STATUS"], 12)
        self.assertEqual(evidence["TEMPORAL_KEY_STATUS"], 10)

    def test_public_codec_rejects_resource_claims_before_large_work(self) -> None:
        evidence = self.binary_evidence
        self.assertEqual(evidence["ENCODE_FRAME_BUDGET_STATUS"], 13)
        self.assertEqual(evidence["ENCODE_CAPACITY_BUDGET_STATUS"], 13)
        self.assertEqual(evidence["DECODE_FRAME_BUDGET_STATUS"], 13)
        self.assertEqual(evidence["DECODE_PAYLOAD_BUDGET_STATUS"], 13)
        self.assertEqual(evidence["DECODE_MINIMUM_HEADER_STATUS"], 4)


if __name__ == "__main__":
    unittest.main()
