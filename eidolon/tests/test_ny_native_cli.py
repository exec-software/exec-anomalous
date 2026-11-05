from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest

from reference.codec import crc32c, decode_trace, encode_trace, mode_histogram
from tests.test_ny_cli import _fixture_frames, _resolve_stdlib, _resolve_tool, _sha256
from tests.test_ny_codec import _parse_evidence, _run_ny_route


eidolon_root = Path(__file__).parents[1]
native_sources = (
    eidolon_root / "src" / "cli.ny",
    eidolon_root / "src" / "format.ny",
    eidolon_root / "src" / "ppm.ny",
    eidolon_root / "src" / "codec.ny",
    eidolon_root / "src" / "limits.ny",
)
llvm_unavailable_markers = (
    "native backend (llvm) is not compiled",
    "llvm native codegen disabled",
    "no llvm backend",
)


class llvm_backend_disabled(RuntimeError):
    pass


def _llvm_is_unavailable(completed: subprocess.CompletedProcess[str]) -> bool:
    diagnostic = (completed.stdout + "\n" + completed.stderr).lower()
    return any(marker in diagnostic for marker in llvm_unavailable_markers)


def _stdlib_sha256(stdlib: Path) -> str:
    digest = hashlib.sha256()
    for source in sorted(stdlib.rglob("*.ny")):
        relative = source.relative_to(stdlib).as_posix().encode("utf-8")
        content = source.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return digest.hexdigest()


def _pin_toolchain() -> tuple[Path, Path, Path, Path, Path, Path]:
    compiler = _resolve_tool("NY_COMPILER", "nycompiler")
    interpreter = _resolve_tool("NY_INTERPRETER", "nyinterpreter")
    assembler = _resolve_tool("NY_ASSEMBLER", "nyassembler")
    linker = _resolve_tool("NY_LINKER", "nylinker")
    if (
        compiler is None
        or interpreter is None
        or assembler is None
        or linker is None
    ):
        raise AssertionError(
            "the native CLI harness requires nycompiler, nyinterpreter, "
            "nyassembler, and nylinker from one shared Ny runtime"
        )
    stdlib = _resolve_stdlib()
    sdk_root = compiler.parent
    if (
        interpreter.parent != sdk_root
        or assembler.parent != sdk_root
        or linker.parent != sdk_root
        or stdlib.parent != sdk_root
    ):
        raise AssertionError(
            "NY_COMPILER, NY_INTERPRETER, NY_ASSEMBLER, NY_LINKER, and "
            "NY_STDLIB must resolve inside one shared Ny runtime"
        )

    manifest_path = sdk_root / "manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise AssertionError(
            "the native CLI harness requires the SDK manifest beside nycompiler"
        ) from error

    source = manifest.get("source", {})
    revision = source.get("revision")
    if (
        not isinstance(revision, str)
        or len(revision) != 40
        or any(value not in "0123456789abcdef" for value in revision)
        or not isinstance(source.get("worktree_dirty"), bool)
    ):
        raise AssertionError("the SDK manifest does not pin a source revision")
    # The shared Anomalous runtime is an explicitly marked development SDK: it
    # may be built from a dirty Ny worktree (for example, the in-progress Matrix
    # front end). Every consumed artifact below is still pinned by digest, so a
    # dirty marker does not weaken this harness.
    if source["worktree_dirty"]:
        print(
            "Ny native CLI toolchain: development SDK built from a dirty Ny "
            f"worktree at revision {revision}"
        )

    tools = {
        entry.get("name"): entry
        for entry in manifest.get("tools", [])
        if isinstance(entry, dict)
    }
    for name, path in (
        ("nycompiler", compiler),
        ("nyinterpreter", interpreter),
        ("nyassembler", assembler),
        ("nylinker", linker),
    ):
        entry = tools.get(name)
        if not isinstance(entry, dict) or entry.get("sha256") != _sha256(path):
            raise AssertionError(f"SDK manifest hash mismatch for {name}")

    native_runtime = manifest.get("native_runtime")
    if not isinstance(native_runtime, dict):
        raise AssertionError("SDK manifest does not identify the native runtime")
    runtime_relative = native_runtime.get("path")
    if runtime_relative not in (
        "nylib/native/libnyruntime.a",
        "nylib/native/nyruntime.lib",
    ):
        raise AssertionError("SDK manifest has a non-canonical native runtime path")
    runtime = sdk_root / runtime_relative
    if (
        not runtime.is_file()
        or native_runtime.get("bytes") != runtime.stat().st_size
        or native_runtime.get("sha256") != _sha256(runtime)
    ):
        raise AssertionError("SDK manifest hash mismatch for the native runtime")

    payload_files = {
        entry.get("path"): entry.get("sha256")
        for entry in manifest.get("payload", {}).get("files", [])
        if isinstance(entry, dict)
    }
    if payload_files.get(runtime_relative) != _sha256(runtime):
        raise AssertionError("SDK payload does not pin the native runtime")
    actual_sources = {
        f"nystd/{source_path.relative_to(stdlib).as_posix()}": _sha256(source_path)
        for source_path in sorted(stdlib.rglob("*.ny"))
    }
    manifest_sources = {
        path: digest
        for path, digest in payload_files.items()
        if isinstance(path, str) and path.startswith("nystd/") and path.endswith(".ny")
    }
    if actual_sources != manifest_sources:
        raise AssertionError("SDK manifest does not exactly pin the nystd source tree")

    print(
        "Ny native CLI toolchain: "
        f"sdk={sdk_root}; revision={revision}; "
        f"llvm={manifest.get('build', {}).get('llvm')}; "
        f"nycompiler_sha256={_sha256(compiler)}; "
        f"nyinterpreter_sha256={_sha256(interpreter)}; "
        f"nyassembler_sha256={_sha256(assembler)}; "
        f"nylinker_sha256={_sha256(linker)}; "
        f"nyruntime_sha256={_sha256(runtime)}; "
        f"nystd_sha256={_stdlib_sha256(stdlib)}"
    )
    return compiler, interpreter, assembler, linker, runtime, stdlib


def _native_compile_command(
    compiler: Path, stdlib: Path, output: Path
) -> list[str]:
    return [
        str(compiler),
        "--no-stdlib",
        "--sdk-path",
        str(stdlib),
        "--native",
        "--quiet",
        "--output",
        str(output),
        *(str(source) for source in native_sources),
    ]


def _textual_nil_compile_command(
    compiler: Path,
    stdlib: Path,
    output: Path,
    sources: tuple[Path, ...] = native_sources,
) -> list[str]:
    return [
        str(compiler),
        "--no-stdlib",
        "--sdk-path",
        str(stdlib),
        "--no-bundle",
        "--quiet",
        "--output",
        str(output),
        *(str(source) for source in sources),
    ]


def _run_process(
    arguments: list[str], *, timeout: int = 120
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        arguments,
        cwd=eidolon_root,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def _require_native_compile(
    completed: subprocess.CompletedProcess[str], route: str
) -> None:
    if _llvm_is_unavailable(completed):
        raise llvm_backend_disabled(
            f"{route} is unavailable because nycompiler explicitly disabled LLVM"
        )
    if completed.returncode != 0 or completed.stdout or completed.stderr:
        raise AssertionError(
            f"Ny {route} compile failed: exit={completed.returncode} "
            f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
        )


def _require_tool_success(
    completed: subprocess.CompletedProcess[str], route: str
) -> None:
    if _llvm_is_unavailable(completed):
        raise llvm_backend_disabled(
            f"{route} is unavailable because the SDK explicitly disabled LLVM"
        )
    if completed.returncode != 0 or completed.stderr:
        raise AssertionError(
            f"Ny {route} failed: exit={completed.returncode} "
            f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
        )


def _assert_host_relocatable(path: Path) -> None:
    try:
        with path.open("rb") as stream:
            header = stream.read(20)
    except OSError as error:
        raise AssertionError("nyassembler did not publish a host object") from error
    if len(header) < 20:
        raise AssertionError("nyassembler published a truncated host object")

    if header.startswith(b"\x7fELF"):
        if header[5] == 1:
            byte_order = "little"
        elif header[5] == 2:
            byte_order = "big"
        else:
            raise AssertionError("nyassembler published ELF with invalid byte order")
        if int.from_bytes(header[16:18], byte_order) != 1:
            raise AssertionError("nyassembler ELF output is not relocatable")
        return

    mach_o_orders = {
        b"\xce\xfa\xed\xfe": "little",
        b"\xcf\xfa\xed\xfe": "little",
        b"\xfe\xed\xfa\xce": "big",
        b"\xfe\xed\xfa\xcf": "big",
    }
    byte_order = mach_o_orders.get(header[:4])
    if byte_order is not None:
        if int.from_bytes(header[12:16], byte_order) != 1:
            raise AssertionError("nyassembler Mach-O output is not relocatable")
        return

    coff_machines = {0x014C, 0x01C4, 0x8664, 0xAA64}
    if (
        int.from_bytes(header[0:2], "little") in coff_machines
        and int.from_bytes(header[2:4], "little") > 0
        and int.from_bytes(header[16:18], "little") == 0
    ):
        return

    raise AssertionError("nyassembler output is not a recognized host relocatable")


class aot_native_cli:
    def __init__(self, compiler: Path, stdlib: Path, root: Path) -> None:
        output = root / "eidolon-native"
        completed = _run_process(_native_compile_command(compiler, stdlib, output))
        _require_native_compile(completed, "LLVM AOT")
        self.executable = output.with_suffix(".exe") if os.name == "nt" else output
        if not self.executable.is_file() or not os.access(self.executable, os.X_OK):
            raise AssertionError("Ny LLVM AOT did not produce an executable")

    def run(
        self, arguments: list[str], *, timeout: int = 30
    ) -> subprocess.CompletedProcess[str]:
        return _run_process([str(self.executable), *arguments], timeout=timeout)


class jit_native_cli:
    def __init__(self, compiler: Path, stdlib: Path, root: Path) -> None:
        self.compiler = compiler
        self.stdlib = stdlib
        self.root = root
        self.invocation = 0

    def run(
        self, arguments: list[str], *, timeout: int = 120
    ) -> subprocess.CompletedProcess[str]:
        self.invocation += 1
        output = self.root / f"eidolon-jit-{self.invocation}.nil"
        completed = _run_process(
            [
                *_native_compile_command(self.compiler, self.stdlib, output),
                "--jit",
                "--",
                *arguments,
            ],
            timeout=timeout,
        )
        if _llvm_is_unavailable(completed):
            raise llvm_backend_disabled(
                "LLVM JIT is unavailable because nycompiler explicitly disabled LLVM"
            )
        return completed


class textual_nil_native_cli:
    def __init__(
        self,
        compiler: Path,
        assembler: Path,
        linker: Path,
        runtime: Path,
        stdlib: Path,
        root: Path,
        sources: tuple[Path, ...] = native_sources,
    ) -> None:
        textual_nil = root / "eidolon-cli.text.nil"
        compiled = _run_process(
            _textual_nil_compile_command(compiler, stdlib, textual_nil, sources)
        )
        if compiled.returncode != 0 or compiled.stdout or compiled.stderr:
            raise AssertionError(
                "Ny textual NIL compile failed: "
                f"exit={compiled.returncode} stdout={compiled.stdout!r} "
                f"stderr={compiled.stderr!r}"
            )
        if not textual_nil.is_file():
            raise AssertionError("nycompiler did not publish textual NIL")

        object_path = root / (
            "eidolon-cli.obj" if os.name == "nt" else "eidolon-cli.o"
        )
        assembled = _run_process(
            [str(assembler), "-o", str(object_path), str(textual_nil)]
        )
        _require_tool_success(assembled, "textual NIL assembly")
        _assert_host_relocatable(object_path)

        output = root / "eidolon-toolchain"
        linked = _run_process(
            [
                str(linker),
                "--runtime",
                str(runtime),
                "-o",
                str(output),
                str(object_path),
            ]
        )
        _require_tool_success(linked, "standalone native link")
        self.executable = output.with_suffix(".exe") if os.name == "nt" else output
        if not self.executable.is_file() or not os.access(self.executable, os.X_OK):
            raise AssertionError("nylinker did not publish an executable")

    def run(
        self, arguments: list[str], *, timeout: int = 30
    ) -> subprocess.CompletedProcess[str]:
        return _run_process([str(self.executable), *arguments], timeout=timeout)


def _ppm_document(width: int, height: int, pixels: bytes) -> bytes:
    return f"P6\n{width} {height}\n255\n".encode("ascii") + pixels


class ny_native_cli_tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        (
            cls.compiler,
            cls.interpreter,
            cls.assembler,
            cls.linker,
            cls.runtime,
            cls.stdlib,
        ) = _pin_toolchain()

    def _assert_private_paths_absent(
        self, completed: subprocess.CompletedProcess[str], *paths: Path
    ) -> None:
        combined = completed.stdout + completed.stderr
        for path in paths:
            for secret in (str(path), path.name, path.stem):
                self.assertNotIn(secret, combined)

    def _assert_failure(
        self,
        completed: subprocess.CompletedProcess[str],
        returncode: int,
        stderr: str,
        *private_paths: Path,
    ) -> None:
        self.assertEqual(completed.returncode, returncode, completed)
        self.assertEqual(completed.stdout, "")
        self.assertEqual(completed.stderr, stderr)
        self._assert_private_paths_absent(completed, *private_paths)

    def _exercise_complete_cli(self, route: str) -> None:
        width, height, frames = _fixture_frames()
        expected_container = encode_trace(
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
        expected_modes = mode_histogram(decode_trace(expected_container))

        with tempfile.TemporaryDirectory(
            prefix=f"eidolon-native-cli-{route}-"
        ) as directory:
            root = Path(directory)
            private_root = root / "private paths with spaces"
            private_root.mkdir()
            if route == "jit":
                artifact_root = root / "jit-artifacts"
                artifact_root.mkdir()
                native_cli = jit_native_cli(self.compiler, self.stdlib, artifact_root)
            elif route == "aot":
                artifact_root = root / "aot-artifacts"
                artifact_root.mkdir()
                native_cli = aot_native_cli(self.compiler, self.stdlib, artifact_root)
            elif route == "textual-nil":
                artifact_root = root / "textual-nil-artifacts"
                artifact_root.mkdir()
                native_cli = textual_nil_native_cli(
                    self.compiler,
                    self.assembler,
                    self.linker,
                    self.runtime,
                    self.stdlib,
                    artifact_root,
                )
            else:
                raise AssertionError(f"unknown native CLI route {route!r}")

            inputs: list[Path] = []
            for index, pixels in enumerate(frames):
                input_path = private_root / f"input frame {index}.ppm"
                input_path.write_bytes(_ppm_document(width, height, pixels))
                inputs.append(input_path)

            encoded = private_root / "native trace.eidolon"
            encoded.write_bytes(b"stale output must be atomically replaced")
            original_encoded_stat = encoded.stat()
            completed = native_cli.run(
                ["encode", str(encoded), *(str(path) for path in inputs)]
            )
            self.assertEqual(completed.returncode, 0, completed)
            self.assertEqual(completed.stderr, "")
            self.assertEqual(encoded.read_bytes(), expected_container)
            self._assert_private_paths_absent(completed, encoded, *inputs)
            if os.name == "posix":
                published_stat = encoded.stat()
                self.assertNotEqual(
                    (published_stat.st_dev, published_stat.st_ino),
                    (original_encoded_stat.st_dev, original_encoded_stat.st_ino),
                )
                self.assertEqual(stat.S_IMODE(published_stat.st_mode), 0o600)
            encode_fields = self._fields(completed.stdout)
            self.assertEqual(
                encode_fields,
                {
                    "operation": "encode",
                    "status": "ok",
                    "width": str(width),
                    "height": str(height),
                    "frames": str(len(frames)),
                    "bytes": str(len(expected_container)),
                    "crc32c": str(crc32c(expected_container)),
                },
            )

            separator_pixels = b"\n\x02\x03"
            cr_input = private_root / "cr separator.ppm"
            crlf_input = private_root / "crlf separator.ppm"
            cr_input.write_bytes(b"P6\n1 1\n255\r" + separator_pixels)
            crlf_input.write_bytes(b"P6\n1 1\n255\r\n" + separator_pixels)
            separator_output = private_root / "separator trace.eidolon"
            separator_expected = encode_trace(
                1,
                1,
                (separator_pixels, separator_pixels),
                tile_width=16,
                tile_height=16,
                timebase_numerator=1,
                timebase_denominator=1000,
                timestamps=(0, 16),
                durations=(16, 16),
                keyframe_interval=120,
            )
            separator_encoded = native_cli.run(
                [
                    "encode",
                    str(separator_output),
                    str(cr_input),
                    str(crlf_input),
                ]
            )
            self.assertEqual(separator_encoded.returncode, 0, separator_encoded)
            self.assertEqual(separator_encoded.stderr, "")
            self.assertEqual(separator_output.read_bytes(), separator_expected)
            self._assert_private_paths_absent(
                separator_encoded,
                separator_output,
                cr_input,
                crlf_input,
            )

            inspected = native_cli.run(["inspect", str(encoded)])
            self.assertEqual(inspected.returncode, 0, inspected)
            self.assertEqual(inspected.stderr, "")
            self._assert_private_paths_absent(inspected, encoded)
            inspect_fields = self._fields(inspected.stdout)
            self.assertEqual(
                inspect_fields,
                {
                    "operation": "inspect",
                    "status": "ok",
                    "bytes": str(len(expected_container)),
                    "width": str(width),
                    "height": str(height),
                    "frames": str(len(frames)),
                    "tile_width": "16",
                    "tile_height": "16",
                    "timebase_numerator": "1",
                    "timebase_denominator": "1000",
                    **{mode: str(count) for mode, count in expected_modes.items()},
                    "crc32c": str(crc32c(expected_container)),
                },
            )

            decoded_output = root / "native-frame-1.ppm"
            decoded = native_cli.run(
                ["decode-frame", str(encoded), "1", str(decoded_output)]
            )
            self.assertEqual(decoded.returncode, 0, decoded)
            self.assertEqual(decoded.stderr, "")
            self.assertEqual(
                decoded_output.read_bytes(), _ppm_document(width, height, frames[1])
            )
            self._assert_private_paths_absent(decoded, encoded, decoded_output)
            if os.name == "posix":
                self.assertEqual(
                    stat.S_IMODE(decoded_output.stat().st_mode),
                    0o600,
                )
            decode_fields = self._fields(decoded.stdout)
            self.assertEqual(
                decode_fields,
                {
                    "operation": "decode-frame",
                    "status": "ok",
                    "frame": "1",
                    "width": str(width),
                    "height": str(height),
                    "bytes": str(len(_ppm_document(width, height, frames[1]))),
                    "pixel_crc32c": str(crc32c(frames[1])),
                },
            )

            malformed_ppm = private_root / "truncated raster.ppm"
            malformed_ppm.write_bytes(b"P6\n1 1\n255\n\x01\x02")
            preserved_container = private_root / "preserved container.eidolon"
            preserved_container.write_bytes(b"must remain unchanged")
            rejected_encode = native_cli.run(
                ["encode", str(preserved_container), str(malformed_ppm)]
            )
            self._assert_failure(
                rejected_encode,
                66,
                "eidolon: encode failed at ppm (status 8)\n",
                preserved_container,
                malformed_ppm,
            )
            self.assertEqual(
                preserved_container.read_bytes(), b"must remain unchanged"
            )

            corrupted = private_root / "corrupted trace.eidolon"
            damaged = bytearray(expected_container)
            damaged[20] ^= 1
            corrupted.write_bytes(damaged)
            rejected_inspect = native_cli.run(["inspect", str(corrupted)])
            self._assert_failure(
                rejected_inspect,
                67,
                "eidolon: inspect failed at container (status 9)\n",
                corrupted,
            )

            absent_output = private_root / "absent after rejection.ppm"
            rejected_decode = native_cli.run(
                ["decode-frame", str(corrupted), "0", str(absent_output)]
            )
            self._assert_failure(
                rejected_decode,
                67,
                "eidolon: decode-frame failed at container (status 9)\n",
                corrupted,
                absent_output,
            )
            self.assertFalse(absent_output.exists())

            existing_output = private_root / "existing output.ppm"
            existing_output.write_bytes(b"must not be replaced")
            rejected_existing = native_cli.run(
                ["decode-frame", str(encoded), "0", str(existing_output)]
            )
            self._assert_failure(
                rejected_existing,
                68,
                "eidolon: decode-frame failed at publish (status 1)\n",
                encoded,
                existing_output,
            )
            self.assertEqual(existing_output.read_bytes(), b"must not be replaced")

            if os.name == "posix":
                input_link = private_root / "native input link.eidolon"
                input_link.symlink_to(encoded)
                rejected_input_link = native_cli.run(["inspect", str(input_link)])
                self._assert_failure(
                    rejected_input_link,
                    66,
                    "eidolon: inspect failed at read (status 1)\n",
                    input_link,
                    encoded,
                )
                self.assertTrue(input_link.is_symlink())

                fifo = private_root / "sensitive capture.fifo"
                os.mkfifo(fifo)
                fifo_output = private_root / "absent fifo trace.eidolon"
                rejected_fifo = native_cli.run(
                    ["encode", str(fifo_output), str(fifo)],
                    timeout=10,
                )
                self._assert_failure(
                    rejected_fifo,
                    66,
                    "eidolon: encode failed at read (status 1)\n",
                    fifo,
                    fifo_output,
                )
                self.assertFalse(fifo_output.exists())

                dangling_target = "missing native private target"
                dangling_output = private_root / "dangling native output.ppm"
                dangling_output.symlink_to(dangling_target)
                rejected_dangling = native_cli.run(
                    ["decode-frame", str(encoded), "0", str(dangling_output)]
                )
                self._assert_failure(
                    rejected_dangling,
                    68,
                    "eidolon: decode-frame failed at publish (status 1)\n",
                    encoded,
                    dangling_output,
                    Path(dangling_target),
                )
                self.assertTrue(dangling_output.is_symlink())
                self.assertEqual(os.readlink(dangling_output), dangling_target)

                protected = private_root / "protected target"
                protected.write_bytes(b"protected bytes")
                output_link = private_root / "native output link.eidolon"
                output_link.symlink_to(protected)
                rejected_link = native_cli.run(
                    ["encode", str(output_link), str(inputs[0])]
                )
                self._assert_failure(
                    rejected_link,
                    68,
                    "eidolon: encode failed at publish (status 1)\n",
                    output_link,
                    protected,
                    inputs[0],
                )
                self.assertTrue(output_link.is_symlink())
                self.assertEqual(protected.read_bytes(), b"protected bytes")

    def _fields(self, stdout: str) -> dict[str, str]:
        fields: dict[str, str] = {}
        for line in stdout.splitlines():
            name, separator, value = line.partition("=")
            self.assertTrue(separator and name and name not in fields, line)
            fields[name] = value
        return fields

    def test_llvm_jit_complete_cli_matches_oracle_and_publication_contract(
        self,
    ) -> None:
        try:
            self._exercise_complete_cli("jit")
        except llvm_backend_disabled as error:
            self.skipTest(str(error))

    def test_llvm_aot_complete_cli_matches_oracle_and_publication_contract(
        self,
    ) -> None:
        try:
            self._exercise_complete_cli("aot")
        except llvm_backend_disabled as error:
            self.skipTest(str(error))

    def test_textual_nil_toolchain_matches_complete_cli_contract(
        self,
    ) -> None:
        try:
            self._exercise_complete_cli("textual-nil")
        except llvm_backend_disabled as error:
            self.skipTest(str(error))

    def test_textual_nil_toolchain_enforces_public_codec_resource_contract(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(
            prefix="eidolon-native-codec-budget-"
        ) as directory:
            try:
                executable = textual_nil_native_cli(
                    self.compiler,
                    self.assembler,
                    self.linker,
                    self.runtime,
                    self.stdlib,
                    Path(directory),
                    sources=(
                        eidolon_root / "src" / "main.ny",
                        eidolon_root / "src" / "codec.ny",
                        eidolon_root / "src" / "limits.ny",
                    ),
                )
            except llvm_backend_disabled as error:
                self.skipTest(str(error))
            completed = executable.run([])
            self.assertEqual(completed.returncode, 0, completed)
            self.assertEqual(completed.stderr, "")
            interpreted = _run_ny_route(
                self.compiler,
                self.interpreter,
                Path(directory),
                False,
            )
            self.assertEqual(_parse_evidence(completed.stdout), interpreted)


if __name__ == "__main__":
    unittest.main()
