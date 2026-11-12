#!/usr/bin/env python3
"""Qualify experimental host control flow without opening the full corpus gate.

The original profile-1 proof remains a separate, unchanged obligation. This
script builds nothing, preserves complete compiler outputs, and freshly runs
the unmodified upstream corpus through its existing transparent capture tool.
Generated infinite programs run only in the bounded candidate: timeouts never
count as semantic agreement. Native execution and full qualification stay false.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest

import program_execution_proof as baseline


require = baseline.require
digest = baseline.digest
write_json = baseline.write_json
maximum_steps = 65536
maximum_frames = 16
original_parse_probe = baseline.parse_probe
original_instruction_inventory = baseline.instruction_inventory
fixtures = Path(__file__).resolve().parents[1] / "crates/anomalous-program-executor/fixtures/flow"
# Complete outputs of the pinned compiler; these are not candidate-derived.
compiler_fixture_hashes = {
    "branches": "bf14aa9997013b1753e76f513ddfb4d1345d2b890085f6d3805777af65e23774",
    "calls": "7e923aa1f9a4c8837d0067d2bf4c40136145b925bed4f7c6caa901601532cf06",
    "early_returns": "cc3f6a37f70f1b46cdbecc63873032312739b25979aeab014c23896ee7866663",
    "recursion": "7eb4040f8010338aaf5b4ee387df44d9ad00442653bc7f5442c78676a241150a",
    "text_void": "11e9a40309ac76b5d85eeba2876d4f1001695441128ada790ed88255aa9b41f9",
    "while": "5caccb39df831d7f125ba355ad3ac17e5dbbad717d345c4997fac5de992e2952",
}
compiler_source_hashes = {
    "branches": "f077afa04e8eabdd69b847616da3561377661905cfb10ff1e9a2e3cd43a64e46",
    "calls": "29ef63a5dddef9056b2caca466878c87bac61fefc68d4575c54f9cc45721e0da",
    "early_returns": "171334a147ef4d0c2311a2e0482354ea759bab4cc2b5211c7146fef0e017936a",
    "recursion": "1320e71de796261379c3f07206a1a6ddc7dfb5b83a4c41118af93d1a585dd947",
    "text_void": "402864c4b2e0e2b1ba2590e97576d17be04da621c723a358e146eadfb2df0567",
    "while": "52ce6d0b45859252f30141f74a516b905b7e04e717e2f59258f3e02e582162b5",
}
compiler_expectations = {
    "calls": (42, b"", 0), "branches": (32, b"", 0),
    "while": (13, b"", 0), "recursion": (120, b"", 0),
    "text_void": (0, b"yes\n", 1), "early_returns": (0, b"", 0),
}


def parse_probe(process, output, *, reference=False):
    if reference:
        return original_parse_probe(process, output, reference=True)
    require(process["exit_code"] == 0 and not process["timed_out"], "probe infrastructure failure")
    value = baseline.strict_json(output.decode("utf-8"))
    fields = {"status", "result", "error", "output_hex", "instructions", "console_calls", "output_bytes"}
    require(type(value) is dict and set(value) == fields, "flow probe fields changed")
    require(type(value["status"]) is str and value["status"] in
            {"returned", "execution_refused", "admission_refused"}, "invalid flow status")
    require(type(value["output_hex"]) is str and
            re.fullmatch(r"(?:[0-9a-f]{2})*", value["output_hex"]) is not None, "noncanonical output")
    if value["status"] == "returned":
        require(type(value["result"]) is int and -(1 << 63) <= value["result"] < (1 << 63), "full i64 result missing")
        require(value["error"] is None, "successful flow probe has error")
    else:
        require(value["result"] is None and type(value["error"]) is str and value["error"], "refusal lacks error")
    for field in ("instructions", "console_calls", "output_bytes"):
        require(type(value[field]) is int and value[field] >= 0, f"invalid {field}")
    require(value["instructions"] <= maximum_steps and value["output_bytes"] <= 65536, "flow budget exceeded")
    require(value["console_calls"] <= value["instructions"], "more console calls than dispatched instructions")
    require(value["output_bytes"] == len(value["output_hex"]) // 2, "output accounting mismatch")
    if value["status"] == "admission_refused":
        require(all(value[field] == 0 for field in ("instructions", "console_calls", "output_bytes"))
                and value["output_hex"] == "", "admission refusal produced effects")
    return value


def function(name, body, parameters=(), result="i64"):
    return (name, parameters, result, body)


def edition(functions, strings=()):
    text = "# NIL Textual Module\nmodule root\n\n"
    if strings:
        text += "# String constants\n"
        text += "".join(f'const S{index} = "{value}"\n' for index, value in enumerate(strings)) + "\n"
    for number, (name, parameters, result, body) in enumerate(functions, 1):
        if name == "main":
            text += '@ai_context("Exercise bounded functions and control flow")\n'
        text += f"# declaration_line {number}\nfunction {name}"
        if parameters:
            text += "(" + ", ".join(f"{name}: {kind}" for name, kind in parameters) + ")"
        text += f" -> {result}:\n" + "".join(f"    {instruction}\n" for instruction in body) + "end function\n\n"
    return text.encode()


def generated_cases():
    cases = []

    def add(name, functions, result=0, *, strings=(), output=b"", calls=0, instructions=None,
            error=None, admission=False, ceiling=1, steps=maximum_steps, frames=maximum_frames,
            reference_error=None, policy=False, reference_result=None, reference_output=None,
            candidate_only=False, cross_profile=False):
        cases.append({"name": name, "edition": edition(functions, strings), "ceiling": ceiling,
                      "steps": steps, "frames": frames, "result": None if error else result,
                      "output": output, "console_calls": calls, "instructions": instructions,
                      "error": error, "admission": admission, "reference_error": reference_error,
                      "policy": policy, "reference_result": reference_result,
                      "reference_output": reference_output, "candidate_only": candidate_only or admission,
                      "cross_profile": cross_profile, "origin": "authored_textual_nil"})

    main = lambda body: function("main", body)
    subtract = function("subtract", ["STORE_LOCAL right", "STORE_LOCAL left", "LOAD_LOCAL left",
                                    "LOAD_LOCAL right", "SUB i64", "RET"], (("left", "i64"), ("right", "i64")))
    add("ordered_arguments", [main(["PUSH_CONST 17", "PUSH_CONST 5", "CALL subtract 2", "RET"]), subtract],
        12, instructions=10, cross_profile=True)
    add("zero_argument_call", [main(["CALL answer 0", "RET"]), function("answer", ["PUSH_CONST 42", "RET"])],
        42, instructions=4, ceiling=0, cross_profile=True)
    add("nested_caller_stack_and_forward_target", [
        main(["PUSH_CONST 100", "PUSH_CONST 9", "CALL outer 1", "ADD i64", "RET"]),
        function("outer", ["STORE_LOCAL value", "LOAD_LOCAL value", "CALL inner 1", "PUSH_CONST 1", "ADD i64", "RET"], (("value", "i64"),)),
        function("inner", ["STORE_LOCAL value", "LOAD_LOCAL value", "PUSH_CONST 2", "MUL i64", "RET"], (("value", "i64"),)),
    ], 119, instructions=16, cross_profile=True)
    increment = function("increment", ["STORE_LOCAL value", "LOAD_LOCAL value", "PUSH_CONST 1", "ADD i64", "RET"], (("value", "i64"),))
    add("repeated_fresh_locals", [main(["PUSH_CONST 3", "CALL increment 1", "STORE_LOCAL value", "PUSH_CONST 8",
                                      "CALL increment 1", "LOAD_LOCAL value", "ADD i64", "RET"]), increment],
        13, instructions=18)
    for kind in ("i32", "i64"):
        add(f"no_implicit_narrowing_{kind}", [main(["PUSH_CONST 4294967297", "CALL identity 1", "RET"]),
            function("identity", ["STORE_LOCAL value", "LOAD_LOCAL value", "RET"], (("value", kind),), kind)],
            4294967297, instructions=6)
    parameters = tuple((f"p{index}", "i64") for index in range(8))
    add("eight_ordered_arguments", [main([f"PUSH_CONST {i}" for i in range(1, 9)] + ["CALL eight 8", "RET"]),
        function("eight", [f"STORE_LOCAL p{i}" for i in reversed(range(8))] +
                 ["LOAD_LOCAL p0", "PUSH_CONST 100", "MUL i64", "LOAD_LOCAL p7", "ADD i64", "RET"], parameters)],
        108, instructions=24)
    echo = function("echo", ["STORE_LOCAL message", "LOAD_LOCAL message", "RET"], (("message", "text"),), "text")
    add("borrowed_text_return", [main(["PUSH_CONST_STR S0", "CALL echo 1", "CALL println 1", "PUSH_CONST 0", "RET"]), echo],
        strings=("flow — ✓",), output="flow — ✓\n".encode(), calls=1, instructions=8, cross_profile=True)
    announce = function("announce", ["STORE_LOCAL message", "LOAD_LOCAL message", "CALL println 1", "RET"], (("message", "text"),), "void")
    add("void_helper", [main(["PUSH_CONST_STR S0", "CALL announce 1", "PUSH_CONST 42", "RET"]), announce],
        42, strings=("announced",), output=b"announced\n", calls=1, instructions=8, cross_profile=True)
    for opcode in ("JMP_IF", "JMP_IF_NOT"):
        for condition in (0, 1, -1, 9007199254740993):
            taken = condition != 0 if opcode == "JMP_IF" else condition == 0
            add(f"{opcode.lower()}_{condition}", [main([f"PUSH_CONST {condition}", f"{opcode} taken", "PUSH_CONST 11", "JMP done",
                                                      "LABEL taken", "PUSH_CONST 22", "LABEL done", "RET"])],
                22 if taken else 11, instructions=6, cross_profile=condition == 0)
    add("unconditional_forward_jump", [main(["JMP target", "PUSH_CONST 99", "RET", "LABEL target", "PUSH_CONST 42", "RET"])],
        42, instructions=4)
    for count in (0, 1, 5):
        body = [f"PUSH_CONST {count}", "STORE_LOCAL count", "PUSH_CONST 0", "STORE_LOCAL sum", "LABEL loop",
                "LOAD_LOCAL count", "PUSH_CONST 0", "CMP_GT", "JMP_IF_NOT done", "LOAD_LOCAL sum", "LOAD_LOCAL count",
                "ADD i64", "STORE_LOCAL sum", "LOAD_LOCAL count", "PUSH_CONST 1", "SUB i64", "STORE_LOCAL count",
                "JMP loop", "LABEL done", "LOAD_LOCAL sum", "RET"]
        add(f"backward_sum_{count}", [main(body)], count * (count + 1) // 2, instructions=12 + count * 14, cross_profile=True)
        if count == 5:
            add("loop_fuel_exact", [main(body)], 15, steps=82, instructions=82)
            add("loop_fuel_one_short", [main(body)], error="instruction_limit", steps=81, instructions=81,
                policy=True, reference_result=15, reference_output=b"")
    factorial = function("factorial", ["STORE_LOCAL count", "LOAD_LOCAL count", "PUSH_CONST 1", "CMP_LE", "JMP_IF_NOT recurse",
                         "PUSH_CONST 1", "RET", "LABEL recurse", "LOAD_LOCAL count", "LOAD_LOCAL count", "PUSH_CONST 1",
                         "SUB i64", "CALL factorial 1", "MUL i64", "RET"], (("count", "i64"),))
    for count, result in ((0, 1), (1, 1), (5, 120), (15, 1307674368000)):
        add(f"factorial_{count}", [main([f"PUSH_CONST {count}", "CALL factorial 1", "RET"]), factorial],
            result, instructions=10 + max(0, count - 1) * 13, cross_profile=count == 5)
    add("finite_frame_limit", [main(["PUSH_CONST 3", "CALL factorial 1", "RET"]), factorial],
        error="frame_limit", frames=2, instructions=13, policy=True, reference_result=6, reference_output=b"")
    parity_functions = []
    for name, other, value in (("even", "odd", 1), ("odd", "even", 0)):
        parity_functions.append(function(name, ["STORE_LOCAL count", "LOAD_LOCAL count", "PUSH_CONST 0", "CMP_EQ",
            f"JMP_IF_NOT {name}_recurse", f"PUSH_CONST {value}", "RET", f"LABEL {name}_recurse", "LOAD_LOCAL count",
            "PUSH_CONST 1", "SUB i64", f"CALL {other} 1", "RET"], (("count", "i64"),)))
    for count in (5, 6):
        add(f"mutual_recursion_{count}", [main([f"PUSH_CONST {count}", "CALL even 1", "RET"]), *parity_functions],
            int(count % 2 == 0), instructions=10 + 11 * count)
    caller_stack = ["PUSH_CONST 1"] * 248 + ["PUSH_CONST 0", "CALL stack_user 1"] + ["POP"] * 248 + ["RET"]
    for depth in (8, 9):
        callee = function("stack_user", ["STORE_LOCAL value"] + ["PUSH_CONST 1"] * depth + ["POP"] * (depth - 1) + ["RET"],
                          (("value", "i64"),))
        add(f"aggregate_stack_{248 + depth}", [main(caller_stack), callee], 1,
            error="stack_limit" if depth == 9 else None, instructions=260 if depth == 9 else 516,
            policy=depth == 9, reference_result=1, reference_output=b"")
    add("infinite_branch_fuel", [main(["LABEL again", "JMP again", "PUSH_CONST 0", "RET"])],
        error="instruction_limit", steps=17, instructions=17, policy=True, candidate_only=True)
    add("infinite_recursion_frames", [main(["CALL recurse 0", "RET"]), function("recurse", ["CALL recurse 0", "RET"])],
        error="frame_limit", instructions=16, policy=True, candidate_only=True)
    for name, steps, frames in (("invalid_step_cap", maximum_steps + 1, maximum_frames),
                                ("invalid_frame_cap", maximum_steps, maximum_frames + 1)):
        add(name, [main(["PUSH_CONST 0", "RET"])], error="invalid_limits", instructions=0,
            steps=steps, frames=frames, policy=True, candidate_only=True)
    add("generated_fallthrough_trap", [main(["CALL broken 0", "RET"]),
        function("broken", ["RET $ny.generated.nonvoid-fallthrough"])], error="generated_fallthrough", instructions=2,
        reference_error="non-void function reached compiler-generated fallthrough")
    add("unreachable_generated_fallthrough", [main(["PUSH_CONST 42", "RET", "RET $ny.generated.nonvoid-fallthrough"])],
        42, instructions=2, cross_profile=True)
    add("callee_trap_preserves_output", [main(["PUSH_CONST_STR S0", "CALL println 1", "CALL divide 0", "RET"]),
        function("divide", ["PUSH_CONST 1", "PUSH_CONST 0", "DIV i64", "RET"])], strings=("before",),
        output=b"before\n", calls=1, error="division_by_zero", reference_error="Division by zero", instructions=6)
    output_loop = ["PUSH_CONST 0", "STORE_LOCAL index", "LABEL loop", "LOAD_LOCAL index", "PUSH_CONST 17", "CMP_LT", "JMP_IF_NOT done",
                   "PUSH_CONST_STR S0", "CONSOLE_WRITE", "LOAD_LOCAL index", "PUSH_CONST 1", "ADD i64", "STORE_LOCAL index", "JMP loop",
                   "LABEL done", "PUSH_CONST 0", "RET"]
    add("loop_total_output_policy", [main(output_loop)], strings=("x" * 4096,), output=b"x" * 65536, calls=16,
        error="output_limit", instructions=201, policy=True, reference_result=0, reference_output=b"x" * 69632)
    dead_console = [main(["JMP done", "PUSH_CONST_STR S0", "CONSOLE_WRITE", "LABEL done", "PUSH_CONST 42", "RET"])]
    add("dead_console_granted", dead_console, 42, strings=("unreachable",), instructions=4)
    add("dead_console_denied", dead_console, strings=("unreachable",), ceiling=0, error="manifest_ceiling", admission=True)
    unused_console = [main(["PUSH_CONST 42", "RET"]), function("unused", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"])]
    add("unused_console_granted", unused_console, 42, strings=("unreachable",), instructions=2)
    add("unused_console_denied", unused_console, strings=("unreachable",), ceiling=0, error="manifest_ceiling", admission=True)
    invalid = {
        "missing_label": [main(["JMP missing", "PUSH_CONST 0", "RET"])],
        "duplicate_label": [main(["LABEL same", "LABEL same", "PUSH_CONST 0", "RET"])],
        "cross_function_jump": [main(["JMP other_label", "PUSH_CONST 0", "RET"]), function("other", ["LABEL other_label", "PUSH_CONST 0", "RET"])],
        "wrong_arity": [main(["PUSH_CONST 1", "CALL subtract 1", "RET"]), subtract],
        "wrong_argument_kind": [main(["PUSH_CONST_STR S0", "CALL increment 1", "RET"]), increment],
        "wrong_return_kind": [main(["CALL wrong 0", "RET"]), function("wrong", ["PUSH_CONST_STR S0", "RET"])],
        "void_return_value": [main(["CALL wrong 0", "PUSH_CONST 0", "RET"]), function("wrong", ["PUSH_CONST 1", "RET"], result="void")],
        "text_condition": [main(["PUSH_CONST_STR S0", "JMP_IF target", "LABEL target", "PUSH_CONST 0", "RET"])],
        "join_height": [main(["PUSH_CONST 0", "JMP_IF right", "PUSH_CONST 1", "JMP join", "LABEL right", "LABEL join", "PUSH_CONST 0", "RET"])],
        "join_kind": [main(["PUSH_CONST 0", "JMP_IF right", "PUSH_CONST 1", "JMP join", "LABEL right", "PUSH_CONST_STR S0", "LABEL join", "POP", "PUSH_CONST 0", "RET"])],
        "local_not_definite": [main(["PUSH_CONST 0", "JMP_IF right", "PUSH_CONST 1", "STORE_LOCAL value", "LABEL right", "LOAD_LOCAL value", "RET"])],
        "fall_off_function": [main(["PUSH_CONST 0", "JMP_IF done", "PUSH_CONST 1", "RET", "LABEL done"])],
        "dead_missing_label": [main(["PUSH_CONST 0", "RET", "JMP missing"])],
        "dead_wrong_arity": [main(["PUSH_CONST 0", "RET", "CALL subtract 1"]), subtract],
        "dead_missing_constant": [main(["PUSH_CONST 0", "RET", "PUSH_CONST_STR absent"])],
    }
    for name, functions in invalid.items():
        add(f"reject_{name}", functions, strings=("text",), error="manifest_syntax", admission=True)
    for name, body in (("foreign_call", ["INTEROP_CALL libc puts 1 i64"]), ("time", ["TIME_NOW"]),
                       ("indirect", ["PUSH_FN_PTR helper"]), ("untyped_add", ["PUSH_CONST 1", "PUSH_CONST 2", "ADD"]),
                       ("unknown_call", ["CALL missing 0"]), ("malformed_call", ["CALL increment 0 extra"])):
        add(f"reject_dead_{name}", [main(["PUSH_CONST 0", "RET", *body])], error="manifest_unsupported", admission=True)
    for name, opcode in (("foreign", "INTEROP_CALL libc puts 1 i64"), ("time", "TIME_NOW")):
        add(f"reject_unused_{name}", [main(["PUSH_CONST 0", "RET"]), function("unused", [opcode, "PUSH_CONST 0", "RET"])],
            error="manifest_unsupported", admission=True)
    add("reject_written_instruction_cap", [main(["NOP"] * 4096 + ["PUSH_CONST 0", "RET"])],
        error="manifest_limit", admission=True)
    add("reject_basic_block_cap", [main([f"LABEL block{i}" for i in range(300)] + ["PUSH_CONST 0", "RET"])],
        error="manifest_limit", admission=True)
    add("reject_function_cap", [main(["PUSH_CONST 0", "RET"])] +
        [function(f"helper{i}", ["PUSH_CONST 0", "RET"]) for i in range(64)], error="manifest_limit", admission=True)
    add("reject_constant_cap", [main(["PUSH_CONST 0", "RET"])], strings=tuple(f"value{i}" for i in range(129)),
        error="manifest_limit", admission=True)
    require(len({case["name"] for case in cases}) == len(cases), "duplicate flow case")
    return cases


def compare_case(case, candidate, reference):
    status = "admission_refused" if case["admission"] else "execution_refused" if case["error"] else "returned"
    require(candidate["status"] == status, f"candidate status: {case['name']}")
    require(candidate["result"] == case["result"] and candidate["error"] == case["error"], f"candidate outcome: {case['name']}")
    require(candidate["output_hex"] == case["output"].hex() and candidate["console_calls"] == case["console_calls"],
            f"candidate effects: {case['name']}")
    if case["instructions"] is not None:
        require(candidate["instructions"] == case["instructions"], f"candidate dispatch accounting: {case['name']}")
    if case["candidate_only"]:
        require(reference is None, "candidate-only case unexpectedly ran the reference")
        return "expected_admission_refusal" if case["admission"] else "candidate_bounded_policy"
    require(reference is not None, "differential evidence missing")
    if case["policy"]:
        require(reference["status"] == "returned" and reference["result"] == case["reference_result"]
                and reference["error"] is None and reference["output_hex"] == case["reference_output"].hex(),
                f"finite policy reference: {case['name']}")
        return "deliberate_runtime_policy_refusal"
    require(reference["status"] == status and reference["result"] == case["result"]
            and reference["output_hex"] == case["output"].hex(), f"reference semantics: {case['name']}")
    require(reference["error"] == case["reference_error"], f"reference error: {case['name']}")
    return "matching_runtime_refusal" if case["error"] else "matching_semantics"


def instruction_inventory(data):
    """Lexical coverage metadata only, never a CFG verifier or admission path."""
    result = original_instruction_inventory(data)
    result["profile"] = 2
    if result["unsupported_format"]:
        return result
    headers = {}
    header_pattern = re.compile(r"function ([A-Za-z_][A-Za-z_0-9]*)(?:\(([^()]*)\))? -> (i32|i64|text|void):")
    for line in data.decode("utf-8").splitlines():
        match = header_pattern.fullmatch(line.strip())
        if match:
            parameters = match[2].split(", ") if match[2] else []
            if all(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*: (?:i32|i64|text)", parameter) for parameter in parameters):
                headers[match[1]] = len(parameters)
    result["unsupported_declarations"] = [record for record in result["unsupported_declarations"]
        if not (record["category"] == "unsupported_function_shape" and
                header_pattern.fullmatch(record["text"].strip()) and
                header_pattern.fullmatch(record["text"].strip())[1] in headers)]
    for instruction in result["instructions"]:
        opcode, operands = instruction["opcode"], instruction["operands"]
        instruction["function"] = instruction["function"].split("(", 1)[0]
        if opcode in {"LABEL", "JMP", "JMP_IF", "JMP_IF_NOT"}:
            instruction["supported_opcode"] = True
            instruction["supported_form"] = (len(operands) == 1 and
                re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", operands[0]) is not None)
        elif opcode == "CALL" and len(operands) == 2 and operands[0] in headers:
            instruction["supported_form"] = operands[1] == str(headers[operands[0]])
        elif opcode == "RET" and operands == ["$ny.generated.nonvoid-fallthrough"]:
            instruction["supported_form"] = True
    result["scope"] = "profile-2 lexical coverage only; references, authority, CFG and admission are independently checked"
    return result


@contextmanager
def corpus_flow_adapters():
    """Reuse the exact corpus runner and capture logic with a wider JSON bound.

    This changes only bindings in this process, never the original source file.
    Both functions are restored even if a child or verification fails. The old
    corpus's conservative treatment of unexpected execution failures is kept.
    """
    previous_parse, previous_inventory = baseline.parse_probe, baseline.instruction_inventory
    baseline.parse_probe, baseline.instruction_inventory = parse_probe, instruction_inventory
    try:
        yield
    finally:
        baseline.parse_probe, baseline.instruction_inventory = previous_parse, previous_inventory


def compiler_cases(args, output, environment):
    records, cases = [], []
    retained = output / "compiler-fixture-inputs"
    retained.mkdir()
    require(set(compiler_fixture_hashes) == set(compiler_expectations) == set(compiler_source_hashes), "compiler fixture pins incomplete")
    for name, (result, stdout, calls) in compiler_expectations.items():
        for extension, pins in (("ny", compiler_source_hashes), ("nil", compiler_fixture_hashes)):
            path = fixtures / f"{name}.{extension}"
            data = path.read_bytes()
            require(digest(data) == pins[name], f"checked-in compiler input changed: {name}.{extension}")
            saved = retained / path.name
            saved.write_bytes(data)
            records.append({"path": str(path), "retained": str(saved.relative_to(output)),
                            "bytes": len(data), "sha256": digest(data)})
        target = output / f"compiler-{name}.nil"
        process, _, stderr = baseline.run([args.compiler, "--quiet", "--no-stdlib", "--no-bundle", "--no-context-registration",
                                           "--output", target, fixtures / f"{name}.ny"],
                                          output / f"compile-{name}", environment=environment)
        require(process["exit_code"] == 0 and not process["timed_out"], f"compiler fixture failed: {name}: {stderr!r}")
        data = target.read_bytes()
        require(digest(data) == compiler_fixture_hashes[name] and data == (fixtures / f"{name}.nil").read_bytes(),
                f"complete compiler output changed: {name}")
        cases.append({"name": f"compiler_{name}", "edition": data, "ceiling": 1,
                      "steps": maximum_steps, "frames": maximum_frames, "result": result,
                      "output": stdout, "console_calls": calls, "instructions": None,
                      "error": None, "admission": False, "reference_error": None, "policy": False,
                      "reference_result": None, "reference_output": None, "candidate_only": False,
                      "cross_profile": True, "origin": "complete_pinned_compiler_output"})
    write_json(output / "compiler-fixture-inputs.json", records)
    return cases, records


def profile_proof(args, output, environment):
    compiled, sources = compiler_cases(args, output, environment)
    cases = generated_cases() + compiled
    require(len({case["name"] for case in cases}) == len(cases), "flow fixture collision")
    editions = output / "profile-editions"
    editions.mkdir()
    results, failures = [], []
    for index, case in enumerate(cases):
        path = editions / f"{index:03d}-{case['name']}.nil"
        path.write_bytes(case["edition"])
        item = {key: value for key, value in case.items() if key not in {"edition", "output", "reference_output"}}
        item.update(edition_sha256=digest(case["edition"]), edition_bytes=len(case["edition"]),
                    retained_path=str(path.relative_to(output)), expected_output_hex=case["output"].hex(),
                    reference_output_hex=None if case["reference_output"] is None else case["reference_output"].hex(),
                    instruction_expectation="exact" if case["instructions"] is not None else "bounded; compiler semantics checked independently")
        try:
            process, stdout, _ = baseline.run([args.candidate, path, str(case["ceiling"]), str(case["steps"]), str(case["frames"])],
                                              output / f"candidate-{index:03d}", environment=environment)
            candidate = parse_probe(process, stdout)
            reference = None
            if not case["candidate_only"]:
                # An admission failure must not cause a new/unknown edition to
                # reach the hosted VM merely because the harness expected it.
                require(candidate["status"] != "admission_refused", "expected admitted edition was refused")
                process, stdout, _ = baseline.run([args.reference, path], output / f"reference-{index:03d}", environment=environment)
                reference = parse_probe(process, stdout, reference=True)
            item.update(candidate=candidate, reference=reference)
            outcome = compare_case(case, candidate, reference)
            if case["cross_profile"]:
                process, stdout, _ = baseline.run([args.profile1_candidate, path, str(case["ceiling"])],
                                                  output / f"profile1-{index:03d}", environment=environment)
                profile1 = original_parse_probe(process, stdout)
                require(profile1["status"] == "admission_refused", "profile-1 admitted a profile-2 extension")
                item["profile1"] = profile1
            item["outcome"] = outcome
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(case["name"])
        results.append(item)
    write_json(output / "profile-results.json", {"profile": 2, "cases": results, "failures": failures})
    return {"passed": not failures, "cases": len(results), "failures": failures,
            "semantic_matches": sum(item["outcome"] == "matching_semantics" for item in results),
            "matching_runtime_refusals": sum(item["outcome"] == "matching_runtime_refusal" for item in results),
            "deliberate_runtime_policy_refusals": sum(item["outcome"] == "deliberate_runtime_policy_refusal" for item in results),
            "candidate_bounded_policy_cases": sum(item["outcome"] == "candidate_bounded_policy" for item in results),
            "expected_admission_refusals": sum(item["outcome"] == "expected_admission_refusal" for item in results),
            "profile1_refusals": sum("profile1" in item and item["profile1"]["status"] == "admission_refused" for item in results)}, sources


def qualification(args):
    for name in ("candidate", "profile1_candidate", "reference", "compiler", "root", "output"):
        setattr(args, name, getattr(args, name).resolve())
    args.interpreter = (args.interpreter or args.compiler.with_name("nilinterpreter")).resolve()
    require(not args.output.exists(), "output must be a new directory")
    args.output.mkdir(parents=True)
    args.output_created = True
    binaries = ("candidate", "profile1_candidate", "reference", "compiler", "interpreter")
    tools = {name: baseline.record_tool(getattr(args, name)) for name in binaries}
    require(tools["compiler"]["sha256"] == baseline.compiler_hash and tools["interpreter"]["sha256"] == baseline.interpreter_hash,
            "qualified compiler/interpreter pins changed")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.root, text=True).strip() == baseline.revision,
            "qualified source revision changed")
    require(not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=args.root, text=True),
            "qualified upstream tracked source changed")
    tools["reference_core_archives"] = [baseline.record_tool(args.root / "build/context-registration/lib" / name)
                                        for name in ("libnilcompiler_core.a", "libnil.a")]
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_flow_reference.cpp"))
    tools["baseline_harness"] = baseline.record_tool(Path(baseline.__file__))
    tools["baseline_reference_source"] = baseline.record_tool(Path(__file__).with_name("program_execution_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    require(tools["baseline_harness"]["sha256"] == "c86a0a9357f2a0e361baea1e4fbcabe4e8a1018111e6aa5cf57e98eb92bc75b3"
            and tools["baseline_reference_source"]["sha256"] == "259aef1f61c850cd887a913e75024e936c81f8bb34dff8a62473022e9658fa0e",
            "original 126-case harness/reference source changed")
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "candidate_profile": 2, "candidate_limits": {"steps": maximum_steps, "frames": maximum_frames, "output_bytes": 65536},
        "candidate_only": "infinite cycles and invalid caller caps never run in the reference; timeouts always fail",
        "original_profile1_proof": "separate required run; this proof does not replace its 126 cases",
    })
    environment = baseline.clean_environment()
    profile, sources = profile_proof(args, args.output, environment)
    with corpus_flow_adapters():
        corpus = baseline.corpus_proof(args, args.output, environment, before)
    require(baseline.inventory(args.root) == before, "upstream corpus/stdlib changed during proof")
    for name in binaries:
        require(baseline.record_tool(getattr(args, name)) == tools[name], f"tool changed: {name}")
    for artifact in [*tools["reference_core_archives"], tools["reference_source"], tools["baseline_harness"],
                     tools["baseline_reference_source"], tools["script"]]:
        require(baseline.record_tool(Path(artifact["path"])) == artifact, "proof source/archive changed")
    for source in sources:
        for path in (Path(source["path"]), args.output / source["retained"]):
            data = path.read_bytes()
            require(len(data) == source["bytes"] and digest(data) == source["sha256"], "fixture snapshot changed")
    passed = baseline.checkpoint_passed(profile, corpus)
    result = {"schema": 1, "profile": 2, "status": "bounded_flow_checkpoint_passed" if passed else "bounded_flow_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "profile_results": profile, "corpus": corpus,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_case_inventory_and_no_infinite_reference_execution(self):
        cases = generated_cases()
        names = {case["name"] for case in cases}
        self.assertEqual(len(names), len(cases))
        for case in cases:
            if case["name"].startswith("infinite_"):
                self.assertTrue(case["candidate_only"])
                self.assertTrue(case["policy"])
        self.assertTrue({"ordered_arguments", "borrowed_text_return", "void_helper", "factorial_15",
                         "loop_fuel_one_short", "reject_join_kind", "dead_console_denied"} <= names)
        self.assertEqual(set(compiler_expectations), set(compiler_fixture_hashes))
        self.assertEqual(set(compiler_expectations), set(compiler_source_hashes))

    def test_probe_rejects_hostile_json_and_effectful_admission_refusal(self):
        process = {"exit_code": 0, "timed_out": False}
        valid = {"status": "returned", "result": 9007199254740993, "error": None,
                 "output_hex": "", "instructions": 65536, "console_calls": 0, "output_bytes": 0}
        self.assertEqual(parse_probe(process, json.dumps(valid).encode())["result"], 9007199254740993)
        for key, changed in (("result", True), ("instructions", True), ("instructions", 65537),
                             ("output_hex", "AA"), ("console_calls", 65537), ("output_bytes", 1)):
            with self.assertRaises(AssertionError):
                parse_probe(process, json.dumps(dict(valid, **{key: changed})).encode())
        with self.assertRaises(AssertionError):
            parse_probe(process, json.dumps(valid).replace('"status":', '"status":"returned","status":').encode())
        with self.assertRaises(AssertionError):
            parse_probe(process, json.dumps(dict(valid, status="admission_refused", result=None, error="manifest_syntax")).encode())
        with self.assertRaises(AssertionError):
            parse_probe({"exit_code": 0, "timed_out": True}, json.dumps(valid).encode())

    def test_adapter_restores_original_bindings_on_failure(self):
        with self.assertRaises(RuntimeError):
            with corpus_flow_adapters():
                self.assertIs(baseline.parse_probe, parse_probe)
                raise RuntimeError("deliberate")
        self.assertIs(baseline.parse_probe, original_parse_probe)
        self.assertIs(baseline.instruction_inventory, original_instruction_inventory)

    def test_exact_errors_and_policy_outcomes_are_distinct(self):
        case = next(case for case in generated_cases() if case["name"] == "generated_fallthrough_trap")
        candidate = {"status": "execution_refused", "result": None, "error": "generated_fallthrough", "output_hex": "",
                     "instructions": 2, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "execution_refused", "result": None,
                     "error": case["reference_error"], "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference), "matching_runtime_refusal")
        for error in ("prefix " + case["reference_error"], case["reference_error"] + " suffix", "other"):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, dict(reference, error=error))
        infinite = next(case for case in generated_cases() if case["name"] == "infinite_branch_fuel")
        bounded = dict(candidate, error="instruction_limit", instructions=17)
        self.assertEqual(compare_case(infinite, bounded, None), "candidate_bounded_policy")
        with self.assertRaises(AssertionError):
            compare_case(infinite, bounded, reference)

    def test_lexical_coverage_does_not_claim_cfg_validation(self):
        case = next(case for case in generated_cases() if case["name"] == "ordered_arguments")
        coverage = instruction_inventory(case["edition"])
        self.assertEqual(coverage["unsupported_declarations"], [])
        self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]))
        self.assertIn("lexical", coverage["scope"])
        self.assertEqual(coverage["profile"], 2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "profile1-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "profile1_candidate", "reference", "compiler", "root", "output"):
        if getattr(args, name) is None:
            parser.error(f"--{name.replace('_', '-')} is required")
    try:
        return qualification(args)
    except (AssertionError, ValueError, OSError) as error:
        failure = {"schema": 1, "profile": 2, "status": "qualification_infrastructure_failed",
                   "candidate_qualified": False, "native_execution": False, "error": str(error)}
        if getattr(args, "output_created", False):
            write_json(args.output / "failure.json", failure)
        print(json.dumps(failure), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
