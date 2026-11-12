#!/usr/bin/env python3
"""Qualify bounded host execution while keeping the full NIL corpus gate closed.

This script never builds tools, edits the upstream corpus, or runs a native VM.
The unmodified upstream runner is executed through transparent capture wrappers;
every interpreter input is retained and attempted by the candidate separately.
The current experimental checkpoint always reports candidate_qualified: false.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import unittest


revision = "4dad0512dfd3fa4e351f2816196cb48956f5442e"
runner_hash = "16e7a6b436b21fab3d93f5cd087ad19efe5c80c21ce6f1d680ce175b50f36581"
compiler_hash = "120f9db7c2ddebf6f91a80576da84d15c47109bbee54105fb16b85d62b4a1ed0"
interpreter_hash = "27b5ffe536b4bf4785a17782dea11dc9b50f722b9e0271296371b854cee117df"
root_names = """aggregates-twin.ny aggregates.matrix bindings.matrix byte-access.matrix
class-dispatch.matrix classes-twin.ny classes.matrix context-and-interop.matrix
context-interop.matrix contracts.matrix control-flow.matrix cross-book-extension.matrix
cross-language.matrix entropy.matrix extensions.matrix fault-handling.matrix
faults-and-ownership.matrix faults-twin.ny faults.matrix file-time.matrix
guide-constructs.matrix hello.matrix memory.matrix names-and-case.matrix paragraphs.matrix
parallel-and-time.matrix parallel-time.matrix record-context.matrix records-and-choices.matrix
scalar-subset-in-ny.ny scalar-subset.matrix short-extension.mat stdlib-choice-in-record.matrix
stdlib-map-construction.matrix telemetry-relay.matrix text-and-types.matrix two-books.matrix
value-position-then.matrix""".split()
compiler_fixture_hashes = {
    "arithmetic": "b015b6d807ab0b01821364d3d71bbe1d38dacdb073910dcba42f41c91ee86b63",
    "console": "7dd9640422d35d31386b72393162ce4d08451956384d624b53b34714fe4a051f",
    "large": "184a478dec89d4d096610b49f6fa6d5c3be0917a490a8e369678605ea4b19994",
}
console_forms = [
    "CALL println 1", "CALL core__print 1", "CALL console__writeln 1",
    "CALL console__write 1", "CALL console__sys_write 1",
    "CONSOLE_WRITE", "CONSOLE_WRITELN",
    "INTEROP_CALL intrinsic console.write 1 void",
    "INTEROP_CALL intrinsic console.writeln 1 void",
]
newline_forms = {console_forms[index] for index in (0, 1, 2, 6, 8)}
supported_opcodes = set("""PUSH_CONST PUSH_CONST_STR CAST NEG ADD SUB MUL DIV MOD BIT_AND
BIT_OR BIT_XOR SHL SHR CMP_EQ CMP_NE CMP_LT CMP_LE CMP_GT CMP_GE NOT STORE_LOCAL
LOAD_LOCAL POP NOP RET CALL CONSOLE_WRITE CONSOLE_WRITELN INTEROP_CALL""".split())


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def strict_json(data):
    def object_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"duplicate JSON key: {key}")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=object_pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))


def record_tool(path):
    path = path.resolve(strict=True)
    require(path.is_file(), f"tool is not a file: {path}")
    data = path.read_bytes()
    return {"path": str(path), "bytes": len(data), "sha256": digest(data)}


def clean_environment():
    environment = os.environ.copy()
    for key in ("NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"):
        environment.pop(key, None)
    environment["NIL_CRASH_REPORT"] = "0"
    return environment


def run(argv, directory, *, timeout=60, environment=None):
    """Retain raw tool output and terminate the whole child group on timeout."""
    directory.mkdir(parents=True, exist_ok=False)
    argv = [str(value) for value in argv]
    with (directory / "stdout.bin").open("wb") as stdout, (directory / "stderr.bin").open("wb") as stderr:
        process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                   cwd=directory, env=environment, start_new_session=True)
        timed_out = False
        try:
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            status = process.wait()
    output = (directory / "stdout.bin").read_bytes()
    errors = (directory / "stderr.bin").read_bytes()
    result = {"argv": argv, "exit_code": status, "timed_out": timed_out,
              "stdout_sha256": digest(output), "stderr_sha256": digest(errors),
              "stdout_bytes": len(output), "stderr_bytes": len(errors)}
    write_json(directory / "process.json", result)
    require(len(output) <= 8 * 1024 * 1024 and len(errors) <= 8 * 1024 * 1024,
            f"tool output exceeded qualification retention limit: {argv[0]}")
    return result, output, errors


def parse_probe(result, output, *, reference=False):
    require(result["exit_code"] == 0 and not result["timed_out"], "probe infrastructure failure")
    value = strict_json(output.decode("utf-8"))
    expected = {"status", "result", "error", "output_hex"}
    expected |= {"schema"} if reference else {"instructions", "console_calls", "output_bytes"}
    require(type(value) is dict and set(value) == expected, f"probe fields changed: {value}")
    statuses = {"returned", "execution_refused"} | (set() if reference else {"admission_refused"})
    require(type(value["status"]) is str and value["status"] in statuses, "invalid probe status")
    if reference:
        require(type(value["schema"]) is int and value["schema"] == 1, "reference schema")
    require(type(value["output_hex"]) is str and re.fullmatch(r"(?:[0-9a-f]{2})*", value["output_hex"]),
            "output is not canonical lowercase byte hex")
    if value["status"] == "returned":
        require(type(value["result"]) is int and -(1 << 63) <= value["result"] < (1 << 63), "full signed result missing")
        require(value["error"] is None, "successful probe has an error")
    else:
        require(value["result"] is None and type(value["error"]) is str and value["error"], "refusal lacks error")
    if not reference:
        for field in ("instructions", "console_calls", "output_bytes"):
            require(type(value[field]) is int and value[field] >= 0, f"invalid candidate {field}")
        require(value["instructions"] <= 4096 and value["output_bytes"] <= 65536, "candidate exceeded profile budget")
        require(value["output_bytes"] == len(value["output_hex"]) // 2, "candidate output accounting mismatch")
        if value["status"] == "admission_refused":
            require(all(value[field] == 0 for field in ("instructions", "console_calls", "output_bytes"))
                    and value["output_hex"] == "", "admission refusal produced effects")
    return value


def nil_text(instructions, *, raw_strings=(), return_type="i64", unused=""):
    constants = ""
    if raw_strings:
        constants = "# String constants\n" + "".join(
            f'const S{index} = "{value}"\n' for index, value in enumerate(raw_strings)) + "\n"
    return ("# NIL Textual Module\nmodule root\n\n" + constants
            + '@ai_context("Exercise bounded host execution")\n# declaration_line 1\n'
            + f"function main -> {return_type}:\n"
            + "".join(f"    {instruction}\n" for instruction in instructions)
            + "end function\n" + unused).encode("utf-8")


def narrow(value, width):
    bits = value & ((1 << width) - 1)
    return bits - (1 << width) if bits & (1 << (width - 1)) else bits


def generated_cases():
    cases = []

    def add(name, body, result=0, *, strings=(), output=b"", calls=0,
            error=None, reference_error=None, policy=False, count=None,
            reference_output=None, return_type="i64", unused="", ceiling=1):
        cases.append({"name": name, "edition": nil_text(body, raw_strings=strings,
                      return_type=return_type, unused=unused), "ceiling": ceiling,
                      "result": None if error else result, "output": output,
                      "console_calls": calls, "instructions": len(body) if count is None else count,
                      "error": error, "reference_error": reference_error,
                      "policy_refusal": policy, "reference_output": reference_output})

    for width in (32, 64):
        low, high = -(1 << (width - 1)), (1 << (width - 1)) - 1
        for operation, left, right, expected in [
            ("ADD", high, 1, low), ("SUB", low, 1, high), ("MUL", high, 2, -2),
            ("BIT_AND", -1, high, high), ("BIT_OR", low, high, -1),
            ("BIT_XOR", -1, high, low), ("SHL", 1, width - 1, low),
            ("SHR", low, width - 1, -1), ("SHR", -8, 2, -2),
        ]:
            add(f"{operation.lower()}_{width}_{left}_{right}",
                [f"PUSH_CONST {left}", f"PUSH_CONST {right}", f"{operation} i{width}", "RET"], expected)
        add(f"neg_min_{width}", [f"PUSH_CONST {low}", f"NEG i{width}", "RET"], low)
        for left, right in ((7, -3), (-7, 3), (-7, -3)):
            quotient = (abs(left) // abs(right)) * (-1 if (left < 0) != (right < 0) else 1)
            for operation, expected in (("DIV", quotient), ("MOD", left - quotient * right)):
                add(f"{operation.lower()}_{width}_{left}_{right}",
                    [f"PUSH_CONST {left}", f"PUSH_CONST {right}", f"{operation} i{width}", "RET"], expected)
        for operation in ("DIV", "MOD"):
            for suffix, left, right, error, phrase in (
                ("zero", 9, 0, "division_by_zero", "Division by zero" if operation == "DIV" else "Modulo by zero"),
                ("overflow", low, -1, "division_overflow", "Integer division overflow" if operation == "DIV" else "Integer modulo overflow"),
            ):
                add(f"trap_{operation.lower()}_{width}_{suffix}",
                    [f"PUSH_CONST {left}", f"PUSH_CONST {right}", f"{operation} i{width}", "RET"],
                    error=error, reference_error=phrase, count=3)
        for operation in ("SHL", "SHR"):
            for count in (-1, width, 4294967296):
                add(f"trap_{operation.lower()}_{width}_{count}",
                    ["PUSH_CONST 8", f"PUSH_CONST {count}", f"{operation} i{width}", "RET"],
                    error="shift_count", reference_error="Shift count out of range", count=3)
    for value in (0, -1, 2147483648, 4294967297, -4294967297, (1 << 63) - 1, -(1 << 63)):
        for width in (32, 64):
            add(f"cast_{width}_{value}", [f"PUSH_CONST {value}", f"CAST i{width}", "RET"], narrow(value, width))
    add("narrow_operands_i32", ["PUSH_CONST 4294967297", "PUSH_CONST 4294967298", "ADD i32", "RET"], 3)
    add("return_does_not_implicitly_narrow", ["PUSH_CONST 4294967297", "RET"], 4294967297, return_type="i32")
    for opcode, expected in (("CMP_EQ", 0), ("CMP_NE", 1), ("CMP_LT", 1),
                             ("CMP_LE", 1), ("CMP_GT", 0), ("CMP_GE", 0)):
        add(opcode.lower(), ["PUSH_CONST 9007199254740992", "PUSH_CONST 9007199254740993", opcode, "RET"], expected)
    for value in (0, 1, -1):
        add(f"not_{value}", [f"PUSH_CONST {value}", "NOT", "RET"], int(value == 0))
    add("locals_overwrite_pop_nop", ["PUSH_CONST 1", "STORE_LOCAL value", "PUSH_CONST -7", "STORE_LOCAL value",
                                     "LOAD_LOCAL value", "POP", "NOP", "LOAD_LOCAL value", "RET"], -7)
    add("decimal_leading_zero", ["PUSH_CONST 00042", "RET"], 42)
    add("decimal_negative_zero", ["PUSH_CONST -0", "RET"], 0)
    for index, form in enumerate(console_forms):
        ending = b"\n" if form in newline_forms else b""
        add(f"console_integer_{index}", ["PUSH_CONST -9223372036854775808", form, "PUSH_CONST 0", "RET"],
            output=b"-9223372036854775808" + ending, calls=1)
        add(f"console_text_{index}", ["PUSH_CONST_STR S0", form, "PUSH_CONST 0", "RET"],
            strings=("Hello — ✓; CALL println 1",), output="Hello — ✓; CALL println 1".encode() + ending, calls=1)
        add(f"console_empty_{index}", ["PUSH_CONST_STR S0", form, "PUSH_CONST 0", "RET"],
            strings=("",), output=ending, calls=int(bool(ending)))
    for name, raw, expected in (("lf", r"a\nb", b"a\nb"), ("tab", r"a\tb", b"a\tb"),
                                ("slash", r"a\\b", b"a\\b"), ("quote", r'a\"b', b'a"b'),
                                ("literal_backspace_escape", r"a\bb", b"a\\bb")):
        add(f"escape_{name}", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"],
            strings=(raw,), output=expected, calls=1)
    for name, raw, expected in (("cr", r"a\rb", b"a\rb"), ("vt", r"a\vb", b"a\vb"), ("ff", r"a\fb", b"a\fb")):
        add(f"policy_{name}", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"],
            strings=(raw,), error="output_policy", policy=True, reference_output=expected, count=2)
    for length in (4096, 4097):
        add(f"write_size_{length}", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"],
            strings=("x" * length,), output=b"x" * length if length == 4096 else b"",
            calls=int(length == 4096), error="output_limit" if length > 4096 else None,
            policy=length > 4096, reference_output=b"x" * length, count=2 if length > 4096 else 4)
    for count in (16, 17):
        add(f"total_writes_{count}", [item for _ in range(count) for item in ("PUSH_CONST_STR S0", "CONSOLE_WRITE")]
            + ["PUSH_CONST 0", "RET"], strings=("x" * 4096,), output=b"x" * 65536, calls=16,
            error="output_limit" if count == 17 else None, policy=count == 17,
            reference_output=b"x" * (count * 4096), count=34)
    unused = ("\n# declaration_line 2\nfunction unused -> i64:\n"
              "    PUSH_CONST 99\n    CONSOLE_WRITE\n    PUSH_CONST 0\n    RET\nend function\n")
    add("unused_console_function", ["PUSH_CONST 42", "RET"], 42, unused=unused)
    add("text_local_copy", ["PUSH_CONST_STR S0", "STORE_LOCAL text", "LOAD_LOCAL text", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"],
        strings=("text",), output=b"text", calls=1)
    add("policy_refusal_preserves_earlier_output", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST_STR S1", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"],
        strings=("ok", r"bad\r"), output=b"ok", calls=1, error="output_policy", policy=True, reference_output=b"okbad\r", count=4)
    add("instruction_budget_exact", ["NOP"] * 4094 + ["PUSH_CONST 0", "RET"])
    require(len({case["name"] for case in cases}) == len(cases), "duplicate generated case")
    return cases


def inventory(root):
    directory = root / "tests/matrix/execution"
    groups, secondary = {}, set()
    for manifest in sorted(directory.glob("*.sources")):
        pages = [line.strip() for line in manifest.read_text().splitlines() if line.strip()]
        require(pages and len(pages) == len(set(pages)), f"empty/duplicate source group {manifest}")
        for name in pages:
            require(Path(name).name == name and (directory / name).is_file(), f"invalid source group member {name}")
        groups[manifest.name] = pages
        secondary.update(name for name in pages if name != manifest.with_suffix(".matrix").name)
    sources = sorted([*directory.glob("*.matrix"), *directory.glob("*.mat"), *directory.glob("*.ny")])
    roots = [path for path in sources if path.name not in secondary]
    require(sorted(path.name for path in roots) == sorted(root_names), "root corpus inventory differs from all 38 pinned cases")
    cases = []
    for source in roots:
        expected, exit_only = source.with_suffix(".expected"), source.with_suffix(".exit-only")
        require(expected.exists() != exit_only.exists(), f"missing/ambiguous result contract: {source.name}")
        cases.append({"name": source.name, "sources": groups.get(source.with_suffix(".sources").name, [source.name]),
                      "exit_only": exit_only.exists(), "expected_sha256": digest(expected.read_bytes()) if expected.exists() else None,
                      "needs_stdlib": source.with_suffix(".needs-stdlib").exists()})
    twins = [(path.name, path.stem + "-twin.ny") for path in sources
             if path.suffix in (".matrix", ".mat") and path.with_name(path.stem + "-twin.ny").exists()]
    require(len(sources) == 43 and len(secondary) == 5 and len(groups) == 3 and len(twins) == 3, "corpus group/twin inventory changed")
    require(sum(case["exit_only"] for case in cases) == 1 and sum(case["needs_stdlib"] for case in cases) == 8, "corpus markers changed")
    runner = root / "tests/matrix/run_execution_tests.py"
    require(digest(runner.read_bytes()) == runner_hash, "upstream runner changed")
    parsed = ast.parse(runner.read_text())
    expectations = {}
    for node in parsed.body:
        if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name) and node.targets[0].id in ("DIAGNOSTIC_EXPECTATIONS", "FOREIGN_KEYWORDS"):
            expectations[node.targets[0].id] = ast.literal_eval(node.value)
    require(len(expectations["DIAGNOSTIC_EXPECTATIONS"]) == 13 and len(expectations["FOREIGN_KEYWORDS"]) == 26, "diagnostic inventory changed")
    files = {}
    for base in (root / "tests/matrix", root / "nilstd/src"):
        for path in sorted(base.rglob("*")):
            if path.is_file():
                data = path.read_bytes()
                files[str(path.relative_to(root))] = {"bytes": len(data), "sha256": digest(data)}
    return {"revision": revision, "historical_root_cases": 32, "root_cases": cases,
            "root_case_count": 38, "exact_output_cases": 37, "exit_only_cases": 1,
            "secondary_files": sorted(secondary), "source_groups": groups, "schema_twins": twins,
            "diagnostics": expectations["DIAGNOSTIC_EXPECTATIONS"],
            "foreign_keyword_diagnostics": expectations["FOREIGN_KEYWORDS"],
            "auxiliary_obligations": {"source_suffix_executions": 3, "debug_metadata_executions": 3,
                                      "include_path_execution": 1, "declaration_context": 1,
                                      "import_renderer_refusals": 4, "binary_import_dependency_builds": 2},
            "expected_compiler_invocations": 97, "expected_interpreter_invocations": 45,
            "files": files, "input_manifest_sha256": digest(json.dumps(files, sort_keys=True).encode())}


def instruction_inventory(data):
    """Coverage metadata only; this scan does not admit, derive or authorize NIL."""
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return {"format": "binary_or_non_utf8", "unsupported_format": True, "instructions": [],
                "unsupported_declarations": [], "intent_present": False}
    if not text.startswith("# NIL Textual Module\n"):
        return {"format": "unknown", "unsupported_format": True, "instructions": [],
                "unsupported_declarations": [], "intent_present": False}
    function, instructions, declarations = None, [], []
    known_outer = ("# NIL Textual Module", "module ", "# String constants", "const ", "@ai_context(", "# declaration_line ")
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if line.startswith("function "):
            function = line[len("function "):].split(" -> ", 1)[0]
            if not re.fullmatch(r"function [A-Za-z_][A-Za-z_0-9]* -> i(?:32|64):", line):
                declarations.append({"line": number, "text": raw, "category": "unsupported_function_shape"})
            continue
        if line == "end function":
            function = None
            continue
        if function is None:
            if line and not line.startswith(known_outer):
                declarations.append({"line": number, "text": raw, "category": "unsupported_outer_record"})
            continue
        if not line:
            continue
        body = line.split(";", 1)[0].strip()
        words = body.split()
        opcode = words[0] if words else ""
        form = opcode in supported_opcodes
        if opcode in ("CALL", "INTEROP_CALL"):
            form = " ".join(words) in console_forms
        elif opcode in ("CAST", "NEG", "ADD", "SUB", "MUL", "DIV", "MOD", "BIT_AND", "BIT_OR", "BIT_XOR", "SHL", "SHR"):
            form = len(words) == 2 and words[1] in ("i32", "i64")
        elif opcode == "PUSH_CONST":
            form = len(words) == 2 and re.fullmatch(r"-?[0-9]+", words[1]) is not None
        elif opcode in ("PUSH_CONST_STR", "STORE_LOCAL", "LOAD_LOCAL"):
            form = len(words) == 2 and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", words[1]) is not None
        else:
            form = form and len(words) == 1
        instructions.append({"line": number, "function": function, "opcode": opcode,
                             "operands": words[1:], "text": raw, "supported_opcode": opcode in supported_opcodes,
                             "supported_form": bool(form)})
    counts = {}
    for instruction in instructions:
        opcode = instruction["opcode"]
        counts[opcode] = counts.get(opcode, 0) + 1
    return {"format": "text_nil", "unsupported_format": False, "instructions": instructions,
            "opcode_counts": counts, "unsupported_declarations": declarations,
            "intent_present": "@ai_context(" in text,
            "scope": "lexical coverage metadata; not parsing, authority derivation, or admission"}


def capture_tool(kind, actual, directory, arguments):
    """Forward the exact runner invocation, retaining its inputs and results."""
    import fcntl
    directory.mkdir(parents=True, exist_ok=True)
    with (directory / "counter").open("a+") as counter:
        fcntl.flock(counter, fcntl.LOCK_EX)
        counter.seek(0)
        index = int(counter.read() or "0")
        counter.seek(0)
        counter.truncate()
        counter.write(str(index + 1))
        counter.flush()
    output = directory / f"{index:04d}"
    output.mkdir()
    output_index = next((arguments.index(option) + 1 for option in ("--output", "-o") if option in arguments), None)
    retained = []
    for position, argument in enumerate(arguments):
        path = Path(argument)
        if position != output_index and path.is_file() and path.suffix.lower() in (".nil", ".ny", ".matrix", ".mat"):
            data = path.read_bytes()
            name = f"input-{position}{path.suffix.lower()}"
            (output / name).write_bytes(data)
            retained.append({"argv_index": position, "path": str(path.resolve()), "retained": name,
                             "bytes": len(data), "sha256": digest(data)})
    includes = []
    configuration = strict_json((directory / "inputs.json").read_text())
    for position, argument in enumerate(arguments[:-1]):
        if argument != "-I":
            continue
        include = Path(arguments[position + 1]).resolve()
        item = {"argv_index": position + 1, "path": str(include), "exists": include.is_dir(), "files": []}
        if str(include) == configuration["stdlib_path"]:
            # This complete tree is hashed before and after the whole run.
            # Avoid retaining eight identical copies from stdlib opt-in cases.
            item["pinned_inventory"] = configuration["inventory_sha256"]
        elif include.is_dir():
            for path in sorted(include.rglob("*")):
                if not path.is_file():
                    continue
                relative = path.relative_to(include)
                data = path.read_bytes()
                saved = Path(f"include-{position + 1}") / relative
                (output / saved).parent.mkdir(parents=True, exist_ok=True)
                (output / saved).write_bytes(data)
                item["files"].append({"relative": str(relative), "retained": str(saved),
                                      "bytes": len(data), "sha256": digest(data)})
        includes.append(item)
    process = subprocess.run([str(actual), *arguments], stdin=subprocess.DEVNULL, capture_output=True)
    inputs_unchanged = all(Path(item["path"]).is_file() and digest(Path(item["path"]).read_bytes()) == item["sha256"] for item in retained)
    includes_unchanged = all(
        (Path(include["path"]) / item["relative"]).is_file()
        and digest((Path(include["path"]) / item["relative"]).read_bytes()) == item["sha256"]
        for include in includes for item in include["files"])
    artifact = None
    for option in ("--output", "-o"):
        if option in arguments:
            at = arguments.index(option) + 1
            if at < len(arguments) and Path(arguments[at]).is_file():
                data = Path(arguments[at]).read_bytes()
                (output / "artifact.nil").write_bytes(data)
                artifact = {"path": str(Path(arguments[at]).resolve()), "retained": "artifact.nil",
                            "bytes": len(data), "sha256": digest(data)}
            break
    (output / "stdout.bin").write_bytes(process.stdout)
    (output / "stderr.bin").write_bytes(process.stderr)
    write_json(output / "call.json", {"index": index, "kind": kind, "argv": arguments,
               "tool": str(actual), "cwd": os.getcwd(), "exit_code": process.returncode,
               "inputs": retained, "includes": includes, "inputs_unchanged": inputs_unchanged,
               "includes_unchanged": includes_unchanged, "artifact": artifact,
               "stdout_sha256": digest(process.stdout), "stderr_sha256": digest(process.stderr)})
    sys.stdout.buffer.write(process.stdout)
    sys.stderr.buffer.write(process.stderr)
    sys.stdout.buffer.flush()
    sys.stderr.buffer.flush()
    if process.returncode < 0:
        os.kill(os.getpid(), -process.returncode)
    return process.returncode


def wrapper(path, kind, actual, capture):
    source = (f"#!{sys.executable}\nimport os,sys\n"
              f"os.execv({sys.executable!r},[{sys.executable!r},{str(Path(__file__).resolve())!r},"
              f"'--capture-tool',{kind!r},{str(actual)!r},{str(capture)!r},'--',*sys.argv[1:]])\n")
    path.write_text(source)
    path.chmod(0o700)


def compare_case(case, candidate, reference):
    require(candidate["status"] == ("execution_refused" if case["error"] else "returned"), f"candidate status: {case['name']}")
    for field, expected in (("result", case["result"]), ("error", case["error"]),
                            ("output_hex", case["output"].hex()), ("instructions", case["instructions"]),
                            ("console_calls", case["console_calls"]), ("output_bytes", len(case["output"]))):
        require(type(candidate[field]) is type(expected) and candidate[field] == expected,
                f"candidate {field}: {case['name']}: {candidate[field]!r} != {expected!r}")
    if case["policy_refusal"]:
        require(reference["status"] == "returned" and reference["result"] == 0
                and reference["output_hex"] == case["reference_output"].hex(), f"reference policy contrast: {case['name']}")
        return "deliberate_output_policy_refusal"
    if case["error"]:
        require(reference["status"] == "execution_refused" and reference["result"] is None
                and case["reference_error"] == reference["error"]
                and reference["output_hex"] == case["output"].hex(), f"reference trap: {case['name']}")
        return "matching_runtime_refusal"
    require(reference["status"] == "returned" and reference["result"] == case["result"]
            and reference["output_hex"] == case["output"].hex(), f"reference result: {case['name']}")
    return "matching_semantics"


def profile_proof(args, output, environment):
    cases = generated_cases()
    source_records = []
    retained_sources = output / "compiler-fixture-inputs"
    retained_sources.mkdir()
    for name, result, calls, stdout in (("arithmetic", 42, 0, b""), ("console", 0, 1, b"Anomalous manifest: hello\n"), ("large", 79, 0, b"")):
        fixtures = Path(__file__).resolve().parents[1] / "crates/anomalous-program-manifest/fixtures/native"
        for extension in ("ny", "nil"):
            original = fixtures / f"{name}.{extension}"
            data = original.read_bytes()
            saved = retained_sources / original.name
            saved.write_bytes(data)
            source_records.append({"path": str(original), "retained": str(saved.relative_to(output)),
                                   "bytes": len(data), "sha256": digest(data)})
        target = output / f"compiler-{name}.nil"
        process, _, errors = run([args.compiler, "--quiet", "--no-stdlib", "--no-bundle", "--no-context-registration",
                                 "--output", target, fixtures / f"{name}.ny"], output / f"compile-{name}", environment=environment)
        require(process["exit_code"] == 0 and not process["timed_out"], f"fixture compilation failed: {name}: {errors!r}")
        data = target.read_bytes()
        require(digest(data) == compiler_fixture_hashes[name] and data == (fixtures / f"{name}.nil").read_bytes(), f"complete compiler edition changed: {name}")
        coverage = instruction_inventory(data)
        main_count = sum(item["function"] == "main" for item in coverage["instructions"])
        cases.append({"name": f"compiler_{name}", "edition": data, "ceiling": 1, "result": result,
                      "output": stdout, "console_calls": calls, "instructions": main_count,
                      "error": None, "reference_error": None, "policy_refusal": False, "reference_output": None})
    write_json(output / "compiler-fixture-inputs.json", source_records)
    results, failures = [], []
    editions_path = output / "profile-editions"
    editions_path.mkdir()
    for index, case in enumerate(cases):
        path = editions_path / f"{index:03d}-{case['name']}.nil"
        path.write_bytes(case["edition"])
        item = {"name": case["name"], "edition_bytes": len(case["edition"]), "edition_sha256": digest(case["edition"]),
                "expected_result": case["result"], "expected_error": case["error"],
                "expected_output_hex": case["output"].hex(), "expected_instructions": case["instructions"],
                "expected_console_calls": case["console_calls"], "policy_refusal": case["policy_refusal"]}
        try:
            process, stdout, _ = run([args.reference, path], output / f"reference-{index:03d}", environment=environment)
            reference = parse_probe(process, stdout, reference=True)
            process, stdout, _ = run([args.candidate, path, str(case["ceiling"])], output / f"candidate-{index:03d}", environment=environment)
            candidate = parse_probe(process, stdout)
            item.update(reference=reference, candidate=candidate, outcome=compare_case(case, candidate, reference))
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(case["name"])
        results.append(item)
    write_json(output / "profile-results.json", {"cases": results, "failures": failures})
    for source in source_records:
        data = Path(source["path"]).read_bytes()
        require(len(data) == source["bytes"] and digest(data) == source["sha256"], "Anomalous compiler fixture input changed")
    return {"passed": not failures, "cases": len(results), "failures": failures,
            "semantic_matches": sum(item["outcome"] == "matching_semantics" for item in results),
            "matching_runtime_refusals": sum(item["outcome"] == "matching_runtime_refusal" for item in results),
            "deliberate_policy_refusals": sum(item["outcome"] == "deliberate_output_policy_refusal" for item in results)}


def corpus_proof(args, output, environment, before):
    capture = output / "upstream-capture"
    capture.mkdir()
    write_json(capture / "inputs.json", {"stdlib_path": str((args.root / "nilstd/src").resolve()),
                                         "inventory_sha256": before["input_manifest_sha256"]})
    tools = output / "capture-tools"
    tools.mkdir()
    wrapper(tools / "nilcompiler", "compiler", args.compiler, capture)
    wrapper(tools / "nilinterpreter", "interpreter", args.interpreter, capture)
    runner = args.root / "tests/matrix/run_execution_tests.py"
    process, stdout, stderr = run([sys.executable, runner, "--compiler", tools / "nilcompiler",
                                  "--interpreter", tools / "nilinterpreter", "--root", args.root],
                                 output / "upstream-runner", timeout=120, environment=environment)
    calls = []
    for path in sorted(capture.glob("[0-9][0-9][0-9][0-9]/call.json")):
        item = strict_json(path.read_text())
        item["directory"] = str(path.parent.relative_to(output))
        for retained in item["inputs"]:
            data = (path.parent / retained["retained"]).read_bytes()
            require(len(data) == retained["bytes"] and digest(data) == retained["sha256"], "captured source input changed")
        for include in item["includes"]:
            if "pinned_inventory" in include:
                require(include["pinned_inventory"] == before["input_manifest_sha256"]
                        and include["path"] == str((args.root / "nilstd/src").resolve()), "include inventory binding changed")
            for retained in include["files"]:
                data = (path.parent / retained["retained"]).read_bytes()
                require(len(data) == retained["bytes"] and digest(data) == retained["sha256"], "captured include dependency changed")
        if item["artifact"] is not None:
            artifact = item["artifact"]
            data = (path.parent / artifact["retained"]).read_bytes()
            require(len(data) == artifact["bytes"] and digest(data) == artifact["sha256"], "captured compiler artifact changed")
            item["artifact_coverage"] = instruction_inventory(data)
        calls.append(item)
    compiler_calls = [item for item in calls if item["kind"] == "compiler"]
    interpreter_calls = [item for item in calls if item["kind"] == "interpreter"]
    invocation_complete = (len(compiler_calls) == 97 and len(interpreter_calls) == 45
                           and [item["index"] for item in calls] == list(range(142)))
    summary_text = stdout.decode("utf-8", errors="strict")
    reference_passed = (process["exit_code"] == 0 and not process["timed_out"]
                        and reference_calls_ok(calls)
                        and "38 fixtures passed, 3 schema comparison(s) identical, 13 diagnostic(s) fired" in summary_text
                        and "1 clock-dependent fixture(s) checked for exit only, 8 against the standard library, 26 foreign keyword(s) redirected" in summary_text)
    roots = {Path(item["name"]).stem: item for item in before["root_cases"]}
    seen_roots, executions = set(), []
    probe_failures = []
    semantic_failures = []
    for number, call in enumerate(interpreter_calls):
        record = {"invocation": number, "upstream_call": call["index"], "argv": call["argv"],
                  "reference_exit_code": call["exit_code"], "candidate_qualified": False}
        try:
            retained = call["inputs"]
            require(len(retained) == 1 and retained[0]["path"].endswith(".nil"), "interpreter invocation input shape changed")
            source = retained[0]
            path = output / call["directory"] / source["retained"]
            edition = path.read_bytes()
            require(len(edition) == source["bytes"] and digest(edition) == source["sha256"], "retained interpreter input changed")
            record.update(edition_sha256=digest(edition), edition_bytes=len(edition),
                          input_path=source["path"], retained_path=str(path.relative_to(output)),
                          coverage=instruction_inventory(edition))
            name = Path(source["path"]).stem
            root_case = roots.get(name)
            if root_case:
                require(name not in seen_roots, f"duplicate root corpus invocation: {name}")
                seen_roots.add(name)
                record.update(root_case=root_case["name"], exit_only=root_case["exit_only"], category="root_execution")
            else:
                record.update(root_case=None, exit_only=False, category="auxiliary_execution")
            reference_output = (output / call["directory"] / "stdout.bin").read_bytes()
            require(digest(reference_output) == call["stdout_sha256"], "retained reference stdout changed")
            record["reference_stdout_hex"] = reference_output.hex()
            child, result, _ = run([args.candidate, path, "1"], output / f"corpus-candidate-{number:03d}", environment=environment)
            candidate = parse_probe(child, result)
            record["candidate"] = candidate
            if candidate["status"] == "returned":
                # The unchanged upstream CLI exposes process status, not a
                # full i64 result. Full-width return checking belongs to the
                # separate generated/reference-driver proof above.
                status_matches = (candidate["result"] & 255) == call["exit_code"]
                output_matches = record["exit_only"] or candidate["output_hex"] == reference_output.hex()
                matched = status_matches and output_matches
                record.update(outcome="returned_match" if matched else "returned_mismatch",
                              matched=matched, result_comparison="process_status_low_byte_only")
                if not matched:
                    semantic_failures.append(number)
            else:
                record.update(outcome=candidate["status"], matched=False,
                              qualification_blocker="unmodified corpus invocation not executed compatibly")
                if candidate["status"] == "execution_refused" and candidate["error"] not in ("output_policy", "output_limit", "instruction_limit"):
                    semantic_failures.append(number)
        except (AssertionError, ValueError, OSError) as error:
            record.update(outcome="probe_failure", matched=False, failure=str(error))
            probe_failures.append(number)
        executions.append(record)
    missing_roots = sorted(set(roots) - seen_roots)
    # All compiler invocations, including diagnostics, schemas, binary import
    # dependencies and metadata checks, remain in this ledger. There is no
    # supported-only filter and no discarded negative fixture.
    write_json(output / "corpus-calls.json", calls)
    write_json(output / "corpus-executions.json", executions)
    result = {
        "candidate_qualified": False,
        "qualification_status": "experimental_profile_only_full_corpus_gate_closed",
        "reference_passed": reference_passed,
        "invocation_inventory_complete": invocation_complete and not missing_roots,
        "compiler_invocations": len(compiler_calls), "interpreter_invocations": len(interpreter_calls),
        "expected_root_cases": 38, "observed_root_cases": len(seen_roots), "missing_root_cases": missing_roots,
        "probe_failures": probe_failures,
        "semantic_failures": semantic_failures,
        "matched_executions": sum(item["matched"] for item in executions),
        "unmatched_executions": sum(not item["matched"] for item in executions),
        "unsupported_or_refused_executions": sum(item["outcome"] in ("admission_refused", "execution_refused") for item in executions),
        "upstream_runner_sha256": runner_hash,
        "upstream_stdout_sha256": digest(stdout), "upstream_stderr_sha256": digest(stderr),
        "capture_wrappers": [record_tool(tools / name) for name in ("nilcompiler", "nilinterpreter")],
        "scope": "Exact upstream corpus run plus complete candidate attempts; unsupported is unmet coverage, never a pass.",
    }
    write_json(output / "corpus-summary.json", result)
    return result


def reference_calls_ok(calls):
    return all(type(item["exit_code"]) is int
               and item["exit_code"] in ({0} if item["kind"] == "interpreter" else {0, 1})
               and item["kind"] in ("compiler", "interpreter")
               and item["inputs_unchanged"] is True and item["includes_unchanged"] is True
               for item in calls)


def checkpoint_passed(profile, corpus):
    return (profile["passed"] and corpus["reference_passed"] and corpus["invocation_inventory_complete"]
            and not corpus["probe_failures"] and not corpus["semantic_failures"])


def qualification(args):
    for name in ("candidate", "reference", "compiler", "root", "output"):
        setattr(args, name, getattr(args, name).resolve())
    args.interpreter = (args.interpreter or args.compiler.with_name("nilinterpreter")).resolve()
    require(not args.output.exists(), "output must be a new directory; prior evidence is never overwritten")
    args.output.mkdir(parents=True)
    args.output_created = True
    tools = {name: record_tool(getattr(args, name)) for name in ("candidate", "reference", "compiler", "interpreter")}
    require(tools["compiler"]["sha256"] == compiler_hash and tools["interpreter"]["sha256"] == interpreter_hash,
            "compiler/reference CLI do not match the qualified toolchain pins")
    observed_revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.root, text=True).strip()
    require(observed_revision == revision, "upstream source revision changed")
    tracked_changes = subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=args.root, text=True)
    require(not tracked_changes, "upstream tracked worktree is modified")
    tools["reference_core_archives"] = [record_tool(args.root / "build/context-registration/lib" / name)
                                        for name in ("libnilcompiler_core.a", "libnil.a")]
    tools["reference_source"] = record_tool(Path(__file__).with_name("program_execution_reference.cpp"))
    tools["script"] = record_tool(Path(__file__))
    before = inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    environment = clean_environment()
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; capture wrappers only forward the runner's arguments",
    })
    profile = profile_proof(args, args.output, environment)
    corpus = corpus_proof(args, args.output, environment, before)
    require(inventory(args.root) == before, "upstream corpus/stdlib changed during qualification")
    for name in ("candidate", "reference", "compiler", "interpreter"):
        require(record_tool(getattr(args, name)) == tools[name], f"tool changed during qualification: {name}")
    for artifact in [*tools["reference_core_archives"], tools["reference_source"]]:
        require(record_tool(Path(artifact["path"])) == artifact, "reference source/archive changed during qualification")
    for source in strict_json((args.output / "compiler-fixture-inputs.json").read_text()):
        for path in (Path(source["path"]), args.output / source["retained"]):
            data = path.read_bytes()
            require(len(data) == source["bytes"] and digest(data) == source["sha256"], "compiler fixture original/retained bytes changed")
    require(record_tool(Path(__file__)) == tools["script"], "qualification script changed while running")
    passed = checkpoint_passed(profile, corpus)
    result = {"schema": 1, "status": "bounded_host_checkpoint_passed" if passed else "bounded_host_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "profile": profile, "corpus": corpus,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_generated_cases_cover_every_admitted_opcode_and_console_form(self):
        cases = generated_cases()
        self.assertEqual(len(cases), len({case["name"] for case in cases}))
        opcodes, forms = set(), set()
        for case in cases:
            self.assertLessEqual(len(case["edition"]), 65536)
            metadata = instruction_inventory(case["edition"])
            self.assertFalse(metadata["unsupported_declarations"])
            for instruction in metadata["instructions"]:
                self.assertTrue(instruction["supported_form"], instruction)
                opcodes.add(instruction["opcode"])
                forms.add(" ".join([instruction["opcode"], *instruction["operands"]]))
        self.assertEqual(opcodes, supported_opcodes)
        self.assertTrue(set(console_forms) <= forms)
        self.assertTrue(any(case["error"] == "division_overflow" for case in cases))
        self.assertTrue(any(case["instructions"] == 4096 for case in cases))

    def test_probe_schema_rejects_duplicate_unknown_missing_and_bool_integer_fields(self):
        candidate = {"status": "returned", "result": 0, "error": None, "output_hex": "",
                     "instructions": 2, "console_calls": 0, "output_bytes": 0}
        process = {"exit_code": 0, "timed_out": False}
        parse_probe(process, json.dumps(candidate).encode())
        for field in candidate:
            changed = dict(candidate)
            del changed[field]
            with self.assertRaises((AssertionError, ValueError)):
                parse_probe(process, json.dumps(changed).encode())
        for field in ("result", "instructions", "console_calls", "output_bytes"):
            changed = dict(candidate, **{field: False})
            with self.assertRaises(AssertionError):
                parse_probe(process, json.dumps(changed).encode())
        with self.assertRaises(AssertionError):
            parse_probe(process, json.dumps(dict(candidate, extra=0)).encode())
        with self.assertRaises(AssertionError):
            parse_probe(process, json.dumps(candidate).replace('{', '{"status":"returned",', 1).encode())
        with self.assertRaises(AssertionError):
            parse_probe({"exit_code": -11, "timed_out": False}, json.dumps(candidate).encode())

    def test_policy_refusals_cannot_be_counted_as_matching_semantics(self):
        case = next(case for case in generated_cases() if case["name"] == "policy_cr")
        candidate = {"status": "execution_refused", "result": None, "error": "output_policy", "output_hex": "",
                     "instructions": 2, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": 0, "error": None, "output_hex": "610d62"}
        self.assertEqual(compare_case(case, candidate, reference), "deliberate_output_policy_refusal")
        for field, value in (("output_hex", "61"), ("console_calls", 1), ("instructions", 1), ("error", "division_by_zero")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference)

    def test_runtime_trap_diagnostic_must_be_exact(self):
        case = next(case for case in generated_cases() if case["name"] == "trap_div_32_zero")
        candidate = {"status": "execution_refused", "result": None, "error": "division_by_zero", "output_hex": "",
                     "instructions": 3, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "execution_refused", "result": None,
                     "error": "Division by zero", "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference), "matching_runtime_refusal")
        for error in ("prefix Division by zero", "Division by zero suffix", "Modulo by zero", ""):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, dict(reference, error=error))

    def test_corpus_return_mismatch_and_reference_signals_fail_the_checkpoint(self):
        profile = {"passed": True}
        corpus = {"reference_passed": True, "invocation_inventory_complete": True,
                  "probe_failures": [], "semantic_failures": []}
        self.assertTrue(checkpoint_passed(profile, corpus))
        for field in ("probe_failures", "semantic_failures"):
            self.assertFalse(checkpoint_passed(profile, dict(corpus, **{field: [0]})))
        call = {"kind": "interpreter", "exit_code": 0, "inputs_unchanged": True, "includes_unchanged": True}
        self.assertTrue(reference_calls_ok([call]))
        for code in (1, -11, -9, False):
            self.assertFalse(reference_calls_ok([dict(call, exit_code=code)]))
        self.assertTrue(reference_calls_ok([dict(call, kind="compiler", exit_code=1)]))
        self.assertFalse(reference_calls_ok([dict(call, kind="compiler", exit_code=2)]))
        self.assertFalse(reference_calls_ok([dict(call, includes_unchanged=False)]))

    def test_coverage_reports_unsupported_forms_and_binary_without_calling_them_pure(self):
        metadata = instruction_inventory(nil_text(["CALL unknown 0", "PUSH_CONST 0", "RET"]))
        self.assertTrue(metadata["instructions"][0]["supported_opcode"])
        self.assertFalse(metadata["instructions"][0]["supported_form"])
        metadata = instruction_inventory(b"NILB\x00\xff")
        self.assertTrue(metadata["unsupported_format"])
        self.assertFalse(metadata["intent_present"])


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--capture-tool":
        require(len(sys.argv) >= 6 and sys.argv[5] == "--", "capture wrapper arguments")
        return capture_tool(sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4]), sys.argv[6:])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--compiler", type=Path)
    parser.add_argument("--interpreter", type=Path)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "reference", "compiler", "root", "output"):
        if getattr(args, name) is None:
            parser.error(f"--{name} is required")
    try:
        return qualification(args)
    except (AssertionError, ValueError, OSError) as error:
        failure = {"schema": 1, "status": "qualification_infrastructure_failed", "candidate_qualified": False,
                   "native_execution": False, "error": str(error)}
        if getattr(args, "output_created", False):
            write_json(args.output / "failure.json", failure)
        print(json.dumps(failure), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
