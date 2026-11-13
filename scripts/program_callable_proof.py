#!/usr/bin/env python3
"""Qualify bounded profile-6 internal callable values without changing earlier profiles.

Raw language conformance and production admission remain separate lanes over
complete, identical editions. The unchanged upstream corpus is never rewritten.
All preceding proofs and their pinned sources remain separate obligations.
"""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest

import program_execution_proof as baseline
import program_flow_proof as flow
import program_array_proof as arrays
import program_conformance_proof as conformance


require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe = conformance.parse_probe
source_pins = dict(arrays.source_pins, **{
    "program_array_proof.py": "7e99ace47ace5a457e5ac037e835764872bc3d6de24f4253c4e4c64c98e377c2",
    "program_array_reference.cpp": "d2943b2221be3e8d21df5ce2887d63e5462389e4f544ae8a81a47eb48656e025",
})
archive_pins = arrays.archive_pins
known_raw_matches = dict(sorted({**arrays.known_raw_matches,
    31: (7278, "4e3da892139e5201831e51c7638d2420702b9ef09de66867810818b02d92a41a"),
}.items()))


def generated_cases():
    cases = []
    annotation = '@ai_context("Exercise bounded internal callable values")'

    def add(name, body, *, helpers=(), strings=(), result=42, output=b"", calls=0,
            instructions=None, error=None, validation=False, policy=False,
            reference_result=None, reference_output=None, reference_error=None,
            ceiling=1, steps=65536, frames=16, annotated=True, production_error=None,
            cross_profile=True):
        functions = [flow.function("main", body), *helpers]
        annotations = {"main": (annotation,)} if annotated else None
        edition = conformance.authored_edition(functions, strings, annotations)
        require(len(edition) <= 65536, f"boundary fixture exceeds edition bound: {name}")
        cases.append({"name": name, "edition": edition, "result": None if error else result,
                      "error": error, "output": output, "console_calls": calls,
                      "instructions": 0 if validation else len(body) if instructions is None else instructions,
                      "validation": validation, "policy": policy,
                      "reference_result": reference_result, "reference_output": reference_output,
                      "reference_error": reference_error, "ceiling": ceiling,
                      "steps": steps, "frames": frames, "annotated": annotated,
                      "production_error": production_error or (error if validation and annotated else
                                                                 "manifest_intent" if not annotated else None),
                      "cross_profile": cross_profile,
                      "origin": "independently_authored_textual_nil"})

    def reject(name, body, *, error="manifest_syntax", **kwargs):
        add("reject_" + name, body, error=error, validation=True, cross_profile=False, **kwargs)

    answer = flow.function("answer", ["PUSH_CONST 42", "RET"])
    seven = flow.function("seven", ["PUSH_CONST 7", "RET"])
    invoke = ["PUSH_FN_PTR answer", "STORE_LOCAL operation", "CALL operation 0", "RET"]
    add("zero_argument_forward_target", invoke, helpers=(answer,), instructions=6, ceiling=0)
    for name, value in (("maximum", (1 << 63) - 1), ("minimum", -(1 << 63)),
                        ("exact_above_53_bits", 9007199254740993)):
        helper = flow.function("answer", [f"PUSH_CONST {value}", "RET"])
        add("i64_result_" + name, invoke, helpers=(helper,), result=value, instructions=6, ceiling=0)
    helper = flow.function("__lambda_0", ["STORE_LOCAL value", "LOAD_LOCAL value", "PUSH_CONST 2", "MUL i64", "RET"],
                           (("value", "i64"),))
    add("noncapturing_lambda_one_argument", ["PUSH_FN_PTR __lambda_0", "STORE_LOCAL operation", "PUSH_CONST 21",
        "CALL operation 1", "RET"], helpers=(helper,), instructions=10, ceiling=0)
    helper = flow.function("subtract", ["STORE_LOCAL right", "STORE_LOCAL left", "LOAD_LOCAL left", "LOAD_LOCAL right",
        "SUB i64", "RET"], (("left", "i64"), ("right", "i64")))
    add("parameter_order", ["PUSH_FN_PTR subtract", "STORE_LOCAL operation", "PUSH_CONST 50", "PUSH_CONST 8",
        "CALL operation 2", "RET"], helpers=(helper,), instructions=12, ceiling=0)
    helper = flow.function("sum_eight", [f"STORE_LOCAL p{i}" for i in reversed(range(8))] +
        [f"LOAD_LOCAL p{i}" for i in range(8)] + ["ADD i64"] * 7 + ["RET"], tuple((f"p{i}", "i64") for i in range(8)))
    add("eight_arguments", ["PUSH_FN_PTR sum_eight", "STORE_LOCAL operation"] + [f"PUSH_CONST {i}" for i in range(1, 9)] +
        ["CALL operation 8", "RET"], helpers=(helper,), result=36, instructions=36, ceiling=0)
    helper = flow.function("answer", ["PUSH_CONST 4294967295", "CAST i32", "RET"], result="i32")
    add("i32_return_explicit_cast", invoke, helpers=(helper,), result=-1, instructions=7, ceiling=0)
    helper = flow.function("narrow", ["STORE_LOCAL value", "LOAD_LOCAL value", "CAST i32", "RET"], (("value", "i32"),), "i32")
    add("i32_parameter_existing_integer_operand", ["PUSH_FN_PTR narrow", "STORE_LOCAL operation", "PUSH_CONST 4294967295",
        "CALL operation 1", "RET"], helpers=(helper,), result=-1, instructions=9, ceiling=0)
    helper = flow.function("announce", ["PUSH_CONST_STR S0", "CONSOLE_WRITELN", "RET"], result="void")
    add("void_target", ["PUSH_FN_PTR announce", "STORE_LOCAL operation", "CALL operation 0", "PUSH_CONST 42", "RET"],
        helpers=(helper,), strings=("called",), output=b"called\n", calls=1, instructions=8)
    for name, body, strings, output in (
        ("literal_text_return", ["PUSH_CONST_STR S0", "RET"], ("hello",), b"hello"),
        ("computed_text_return", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "RET"], ("hello ", "world"), b"hello world"),
    ):
        helper = flow.function("text_value", body, result="text")
        add(name, ["PUSH_FN_PTR text_value", "STORE_LOCAL operation", "CALL operation 0", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
            helpers=(helper,), strings=strings, output=output, calls=1, instructions=6 + len(body))
    helper = flow.function("combine", ["STORE_LOCAL right", "STORE_LOCAL left", "LOAD_LOCAL left", "LOAD_LOCAL right", "STR_CONCAT", "RET"],
                           (("left", "text"), ("right", "text")), "text")
    add("text_parameter_order", ["PUSH_FN_PTR combine", "STORE_LOCAL operation", "PUSH_CONST_STR S0", "PUSH_CONST_STR S1",
        "CALL operation 2", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], helpers=(helper,), strings=("left:", "right"),
        output=b"left:right", calls=1, instructions=14)
    helper = flow.function("values", ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", "RET"], result="array<i64>")
    add("integer_array_return", ["PUSH_FN_PTR values", "STORE_LOCAL operation", "CALL operation 0", "PUSH_CONST 0", "GET_INDEX", "RET"],
        helpers=(helper,), instructions=9, ceiling=0)
    helper = flow.function("values", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "NEW_ARRAY_LITERAL 1 text", "RET"],
                           result="array<text>")
    add("computed_text_array_return", ["PUSH_FN_PTR values", "STORE_LOCAL operation", "CALL operation 0", "PUSH_CONST 0", "GET_INDEX",
        "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], helpers=(helper,), strings=("a", "b"), output=b"ab", calls=1, instructions=13)
    for kind, push, strings, ending, output, calls in (
        ("i64", "PUSH_CONST 42", (), ["RET"], b"", 0),
        ("text", "PUSH_CONST_STR S0", ("selected",), ["CONSOLE_WRITE", "PUSH_CONST 42", "RET"], b"selected", 1),
    ):
        helper = flow.function("first", ["STORE_LOCAL values", "LOAD_LOCAL values", "PUSH_CONST 0", "GET_INDEX", "RET"],
                               (("values", f"array<{kind}>"),), kind)
        body = ["PUSH_FN_PTR first", "STORE_LOCAL operation", push, f"NEW_ARRAY_LITERAL 1 {kind}", "CALL operation 1", *ending]
        add(kind + "_array_parameter", body, helpers=(helper,), strings=strings, output=output, calls=calls, instructions=len(body) + 5)
    helper = flow.function("mixed", ["STORE_LOCAL prefix", "STORE_LOCAL words", "STORE_LOCAL number", "LOAD_LOCAL prefix", "LOAD_LOCAL words",
        "PUSH_CONST 0", "GET_INDEX", "STR_CONCAT", "CONSOLE_WRITE", "LOAD_LOCAL number", "RET"],
        (("number", "i64"), ("words", "array<text>"), ("prefix", "text")))
    add("mixed_target_signature", ["PUSH_FN_PTR mixed", "STORE_LOCAL operation", "PUSH_CONST 42", "PUSH_CONST_STR S1", "NEW_ARRAY_LITERAL 1 text",
        "PUSH_CONST_STR S0", "CALL operation 3", "RET"], helpers=(helper,), strings=("prefix:", "value"), output=b"prefix:value", calls=1, instructions=19)
    add("immutable_callable_alias", ["PUSH_FN_PTR answer", "STORE_LOCAL original", "LOAD_LOCAL original", "STORE_LOCAL alias", "CALL alias 0", "RET"],
        helpers=(answer,), instructions=8, ceiling=0)
    add("rebind_same_signature_target", ["PUSH_FN_PTR seven", "STORE_LOCAL operation", "PUSH_FN_PTR answer", "STORE_LOCAL operation", "CALL operation 0", "RET"],
        helpers=(seven, answer), instructions=8, ceiling=0)
    add("alias_survives_target_rebind", ["PUSH_FN_PTR seven", "STORE_LOCAL original", "LOAD_LOCAL original", "STORE_LOCAL alias",
        "PUSH_FN_PTR answer", "STORE_LOCAL original", "CALL alias 0", "CONSOLE_WRITELN", "CALL original 0", "RET"],
        helpers=(seven, answer), output=b"7\n", calls=1, instructions=14)
    shadow = flow.function("shadow", ["PUSH_CONST 7", "RET"])
    add("local_precedes_declared_function", ["PUSH_FN_PTR answer", "STORE_LOCAL shadow", "CALL shadow 0", "RET"],
        helpers=(shadow, answer), instructions=6, ceiling=0)
    for name in ("println", "console__write"):
        add("local_precedes_builtin_" + name, ["PUSH_FN_PTR answer", f"STORE_LOCAL {name}", f"CALL {name} 0", "RET"],
            helpers=(answer,), instructions=6, ceiling=0)
    add("generated_callee_temporary", ["PUSH_FN_PTR answer", "STORE_LOCAL operation", "LOAD_LOCAL operation", "STORE_LOCAL $ny.temp.callee.0",
        "CALL $ny.temp.callee.0 0", "RET"], helpers=(answer,), instructions=8, ceiling=0)
    add("repeat_target_call", ["PUSH_FN_PTR answer", "STORE_LOCAL operation", "CALL operation 0", "CONSOLE_WRITELN", "CALL operation 0", "RET"],
        helpers=(answer,), output=b"42\n", calls=1, instructions=10)
    for condition, result in ((0, 42), (1, 7)):
        add(f"local_target_join_{condition}", [f"PUSH_CONST {condition}", "JMP_IF_NOT right", "PUSH_FN_PTR seven", "STORE_LOCAL operation", "JMP done",
            "LABEL right", "PUSH_FN_PTR answer", "STORE_LOCAL operation", "LABEL done", "CALL operation 0", "RET"],
            helpers=(seven, answer), result=result, instructions=10, ceiling=0)
        add(f"stack_target_join_{condition}", [f"PUSH_CONST {condition}", "JMP_IF_NOT right", "PUSH_FN_PTR seven", "JMP done", "LABEL right",
            "PUSH_FN_PTR answer", "LABEL done", "STORE_LOCAL operation", "CALL operation 0", "RET"],
            helpers=(seven, answer), result=result, instructions=10, ceiling=0)
    add("loop_rebind_same_signature", ["PUSH_FN_PTR seven", "STORE_LOCAL operation", "PUSH_CONST 0", "STORE_LOCAL index", "LABEL loop",
        "LOAD_LOCAL index", "PUSH_CONST 2", "CMP_LT", "JMP_IF_NOT done", "PUSH_FN_PTR answer", "STORE_LOCAL operation", "LOAD_LOCAL index",
        "PUSH_CONST 1", "ADD", "STORE_LOCAL index", "JMP loop", "LABEL done", "CALL operation 0", "RET"],
        helpers=(seven, answer), instructions=38, ceiling=0)
    add("intentless_callable", invoke, helpers=(answer,), instructions=6, ceiling=0, annotated=False)
    add("dead_known_target", ["PUSH_CONST 42", "RET", "PUSH_FN_PTR answer", "POP"], helpers=(answer,), instructions=2, ceiling=0)
    add("dead_known_local_call", ["PUSH_CONST 42", "RET", "CALL later 0", "PUSH_FN_PTR answer", "STORE_LOCAL later"], helpers=(answer,), instructions=2, ceiling=0)
    add("discard_callable", ["PUSH_FN_PTR answer", "POP", "PUSH_CONST 42", "RET"], helpers=(answer,), ceiling=0)
    add("discard_main_reference", ["PUSH_FN_PTR main", "POP", "PUSH_CONST 42", "RET"], ceiling=0)
    noisy = flow.function("unused_console", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"])
    add("unused_console_body_granted", ["PUSH_FN_PTR answer", "STORE_LOCAL println", "CALL println 0", "RET"],
        helpers=(answer, noisy), strings=("unused",), instructions=6)

    divide = flow.function("divide", ["PUSH_CONST 5", "PUSH_CONST 0", "DIV i64", "RET"])
    add("callee_fault_preserves_output", ["PUSH_FN_PTR divide", "STORE_LOCAL operation", "PUSH_CONST_STR S0", "CONSOLE_WRITELN", "CALL operation 0", "RET"],
        helpers=(divide,), strings=("before",), output=b"before\n", calls=1, error="division_by_zero", instructions=8, reference_error="Division by zero")
    broken = flow.function("broken", ["RET $ny.generated.nonvoid-fallthrough"])
    add("callee_generated_fallthrough", ["PUSH_FN_PTR broken", "STORE_LOCAL operation", "CALL operation 0", "RET"],
        helpers=(broken,), error="generated_fallthrough", instructions=4, reference_error="non-void function reached compiler-generated fallthrough")
    add("fuel_exact_capacity", invoke, helpers=(answer,), instructions=6, steps=6, ceiling=0)
    add("fuel_one_short", invoke, helpers=(answer,), error="instruction_limit", instructions=5, steps=5, policy=True,
        reference_result=42, reference_output=b"", ceiling=0)
    outer = flow.function("outer", ["PUSH_FN_PTR answer", "STORE_LOCAL operation", "CALL operation 0", "RET"])
    chain = ["PUSH_FN_PTR outer", "STORE_LOCAL operation", "CALL operation 0", "RET"]
    add("frames_exact_capacity", chain, helpers=(outer, answer), instructions=10, frames=3, ceiling=0)
    add("finite_frame_limit_partial_output", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + chain, helpers=(outer, answer),
        output=b"7\n", calls=1, error="frame_limit", instructions=8, frames=2, policy=True, reference_result=42, reference_output=b"7\n")
    caller_stack = (["PUSH_FN_PTR stack_user", "STORE_LOCAL operation"] + ["PUSH_CONST 1"] * 248 +
        ["PUSH_CONST 0", "CALL operation 1"] + ["POP"] * 248 + ["RET"])
    for depth in (8, 9):
        helper = flow.function("stack_user", ["STORE_LOCAL value"] + ["PUSH_CONST 1"] * depth + ["POP"] * (depth - 1) + ["RET"],
                               (("value", "i64"),))
        add(f"aggregate_stack_{248 + depth}", caller_stack, helpers=(helper,), result=1, error="stack_limit" if depth == 9 else None,
            instructions=262 if depth == 9 else 518, policy=depth == 9, reference_result=1 if depth == 9 else None,
            reference_output=b"" if depth == 9 else None, ceiling=0)
    helper = flow.function("text_value", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "RET"], result="text")
    add("text_allocation_not_reclaimed_after_call", ["PUSH_FN_PTR text_value", "STORE_LOCAL operation"] +
        ["CALL operation 0", "POP"] * 65 + ["PUSH_CONST 42", "RET"], helpers=(helper,), strings=("x" * 1024, ""),
        error="text_limit", instructions=390, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    helper = flow.function("empty_values", ["NEW_ARRAY_LITERAL 0 i64", "RET"], result="array<i64>")
    add("array_descriptors_not_reclaimed_after_call", ["PUSH_FN_PTR empty_values", "STORE_LOCAL operation"] +
        ["CALL operation 0", "POP"] * 129 + ["PUSH_CONST 42", "RET"], helpers=(helper,),
        error="array_limit", instructions=516, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    helper = flow.function("full_values", [f"PUSH_CONST {index}" for index in range(256)] + ["NEW_ARRAY_LITERAL 256 i64", "RET"], result="array<i64>")
    add("array_elements_not_reclaimed_after_call", ["PUSH_FN_PTR full_values", "STORE_LOCAL operation"] +
        ["CALL operation 0", "POP"] * 5 + ["PUSH_CONST 42", "RET"], helpers=(helper,),
        error="array_limit", instructions=1300, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    helper = flow.function("control_text", ["PUSH_CONST_STR S0", "RET"], result="text")
    add("returned_text_output_policy", ["PUSH_FN_PTR control_text", "STORE_LOCAL operation", "CALL operation 0", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        helpers=(helper,), strings=(r"\r",), error="output_policy", instructions=6, policy=True, reference_result=42, reference_output=b"\r")

    for name in ("missing", "println", "console__write", "libc_puts"):
        reject("target_" + name, ["PUSH_CONST 42", "RET", f"PUSH_FN_PTR {name}"], error="manifest_unsupported")
    for name, instruction in (("missing_operand", "PUSH_FN_PTR"), ("extra_capture_operand", "PUSH_FN_PTR answer captured")):
        reject(name, ["PUSH_CONST 42", "RET", instruction], helpers=(answer,), error="manifest_unsupported")
    for name, target in (("temporary", "$ny.temp.callee.0"), ("qualified_native", "libc::puts")):
        reject("target_identifier_" + name, ["PUSH_CONST 42", "RET", f"PUSH_FN_PTR {target}"])
    reject("unresolved_call", ["PUSH_FN_PTR answer", "POP", "CALL absent 0", "RET"], helpers=(answer,), error="manifest_unsupported")
    reject("future_store_does_not_initialize_call", ["CALL operation 0", "RET", "PUSH_FN_PTR answer", "STORE_LOCAL operation"], helpers=(answer,))
    reject("future_local_overrides_declared_target", ["CALL answer 0", "RET", "PUSH_FN_PTR seven", "STORE_LOCAL answer"], helpers=(answer, seven))
    reject("future_local_overrides_builtin", ["PUSH_CONST 7", "CALL println 1", "PUSH_CONST 42", "RET", "PUSH_FN_PTR answer", "STORE_LOCAL println"], helpers=(answer,))
    for name, value in (("text_name", "PUSH_CONST_STR S0"), ("integer_address", "PUSH_CONST 1"), ("array_handle", "NEW_ARRAY_LITERAL 0 i64")):
        reject("forged_" + name, [value, "STORE_LOCAL operation", "CALL operation 0", "RET"], helpers=(answer,), strings=("answer",))
    reject("integer_local_overrides_declared_target", ["PUSH_CONST 7", "STORE_LOCAL answer", "CALL answer 0", "RET"], helpers=(answer,))
    reject("text_local_overrides_builtin", ["PUSH_CONST_STR S0", "STORE_LOCAL println", "CALL println 0", "RET"], helpers=(answer,), strings=("answer",))
    reject("wrong_arity", ["PUSH_FN_PTR answer", "STORE_LOCAL operation", "PUSH_CONST 1", "CALL operation 1", "RET"], helpers=(answer,))
    identity = flow.function("identity", ["STORE_LOCAL value", "LOAD_LOCAL value", "RET"], (("value", "i64"),))
    reject("missing_argument", ["PUSH_FN_PTR identity", "STORE_LOCAL operation", "CALL operation 1", "RET"], helpers=(identity,))
    reject("wrong_argument_kind", ["PUSH_FN_PTR identity", "STORE_LOCAL operation", "PUSH_CONST_STR S0", "CALL operation 1", "RET"],
        helpers=(identity,), strings=("wrong",))
    texts = flow.function("texts", ["STORE_LOCAL values", "PUSH_CONST 42", "RET"], (("values", "array<text>"),))
    reject("wrong_array_argument_kind", ["PUSH_FN_PTR texts", "STORE_LOCAL operation", "NEW_ARRAY_LITERAL 0 i64", "CALL operation 1", "RET"], helpers=(texts,))
    void = flow.function("empty", ["RET"], result="void")
    reject("void_call_has_no_return_value", ["PUSH_FN_PTR empty", "STORE_LOCAL operation", "CALL operation 0", "RET"], helpers=(void,))
    for name, helper in (
        ("i32_result", flow.function("different", ["PUSH_CONST 42", "RET"], result="i32")),
        ("text_result", flow.function("different", ["PUSH_CONST_STR S0", "RET"], result="text")),
        ("arity", flow.function("different", ["STORE_LOCAL value", "LOAD_LOCAL value", "RET"], (("value", "i64"),))),
    ):
        reject("rebind_different_" + name, ["PUSH_FN_PTR answer", "STORE_LOCAL operation", "PUSH_FN_PTR different", "STORE_LOCAL operation",
            "PUSH_CONST 42", "RET"], helpers=(answer, helper), strings=("text",))
        reject("join_different_" + name, ["PUSH_CONST 0", "JMP_IF_NOT right", "PUSH_FN_PTR answer", "JMP done", "LABEL right",
            "PUSH_FN_PTR different", "LABEL done", "POP", "PUSH_CONST 42", "RET"], helpers=(answer, helper), strings=("text",))
    narrow = flow.function("narrow", ["STORE_LOCAL value", "LOAD_LOCAL value", "RET"], (("value", "i32"),))
    reject("rebind_i32_vs_i64_parameter", ["PUSH_FN_PTR identity", "STORE_LOCAL operation", "PUSH_FN_PTR narrow", "STORE_LOCAL operation",
        "PUSH_CONST 42", "RET"], helpers=(identity, narrow))
    reject("join_callable_with_text", ["PUSH_CONST 0", "JMP_IF_NOT right", "PUSH_FN_PTR answer", "JMP done", "LABEL right", "PUSH_CONST_STR S0",
        "LABEL done", "POP", "PUSH_CONST 42", "RET"], helpers=(answer,), strings=("answer",))
    reject("join_not_definitely_initialized", ["PUSH_CONST 0", "JMP_IF_NOT done", "PUSH_FN_PTR answer", "STORE_LOCAL operation", "LABEL done",
        "CALL operation 0", "RET"], helpers=(answer,))
    for name, tail in (
        ("console_write", ["CONSOLE_WRITE", "PUSH_CONST 42", "RET"]),
        ("console_writeln", ["CONSOLE_WRITELN", "PUSH_CONST 42", "RET"]),
        ("arithmetic", ["PUSH_CONST 1", "ADD", "RET"]),
        ("compare", ["PUSH_CONST 1", "CMP_EQ", "RET"]),
        ("cast", ["CAST i64", "RET"]),
        ("branch", ["JMP_IF done", "LABEL done", "PUSH_CONST 42", "RET"]),
        ("concat", ["PUSH_CONST_STR S0", "STR_CONCAT", "POP", "PUSH_CONST 42", "RET"]),
        ("array_element", ["NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 42", "RET"]),
        ("len", ["LEN", "RET"]),
        ("get_index", ["PUSH_CONST 0", "GET_INDEX", "RET"]),
        ("integer_return", ["RET"]),
    ):
        reject("opaque_callable_" + name, ["PUSH_FN_PTR answer", *tail], helpers=(answer,), strings=("text",))
    capture = flow.function("capturing", ["LOAD_LOCAL captured", "RET"])
    reject("caller_local_is_not_capture", ["PUSH_CONST 42", "STORE_LOCAL captured", "PUSH_FN_PTR capturing", "STORE_LOCAL operation",
        "CALL operation 0", "RET"], helpers=(capture,))
    floor = flow.function("floor", ["POP", "POP", "PUSH_CONST 42", "RET"], (("value", "i64"),))
    reject("callee_cannot_pop_caller_values", ["PUSH_FN_PTR floor", "STORE_LOCAL operation", "PUSH_CONST 7", "PUSH_CONST 0", "CALL operation 1",
        "POP", "RET"], helpers=(floor,))
    callback = flow.function("apply", ["STORE_LOCAL value", "STORE_LOCAL callback", "PUSH_CONST 42", "RET"],
                             (("callback", "fn(i64) -> i64"), ("value", "i64")))
    reject("callable_parameter_signature", ["PUSH_CONST 42", "RET"], helpers=(callback,), error="manifest_unsupported")
    callback = flow.function("make", ["PUSH_FN_PTR identity", "RET"], result="fn(i64) -> i64")
    reject("callable_return_signature", ["PUSH_CONST 42", "RET"], helpers=(callback, identity), error="manifest_unsupported")
    for name, count, error in (("negative", "-1", "manifest_syntax"), ("hex", "0x0", "manifest_syntax"), ("over_capacity", "9", "manifest_limit")):
        reject("call_count_" + name, ["PUSH_FN_PTR answer", "STORE_LOCAL operation", f"CALL operation {count}", "RET"], helpers=(answer,), error=error)
    reject("dead_foreign_effect", ["PUSH_FN_PTR answer", "POP", "PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"], helpers=(answer,), error="manifest_unsupported")
    reject("unused_console_body_denied", ["PUSH_FN_PTR answer", "STORE_LOCAL println", "CALL println 0", "RET"], helpers=(answer, noisy),
        strings=("unused",), ceiling=0, error="manifest_ceiling")
    helper = flow.function("parameter_collision", ["STORE_LOCAL answer", "LOAD_LOCAL answer", "RET"], (("answer", "i64"),))
    reject("parameter_declaration_collision", invoke, helpers=(answer, helper), error="manifest_unsupported")
    helper = flow.function("parameter_collision", ["STORE_LOCAL println", "LOAD_LOCAL println", "RET"], (("println", "i64"),))
    reject("parameter_builtin_collision", invoke, helpers=(answer, helper), error="manifest_unsupported")
    helper = flow.function("println", ["PUSH_CONST 42", "RET"])
    reject("declaration_builtin_collision", invoke, helpers=(answer, helper), error="manifest_unsupported")
    require(len({case["name"] for case in cases}) == len(cases), "duplicate callable boundary case")
    return cases


def compare_case(case, candidate, reference, production):
    expected_status = "validation_refused" if case["validation"] else "execution_refused" if case["error"] else "returned"
    require(candidate["status"] == expected_status, "raw outcome differs from boundary expectation")
    for key in ("result", "error", "console_calls"):
        require(candidate[key] == case[key], f"raw {key} differs from boundary expectation")
    require(case["instructions"] is not None and candidate["instructions"] == case["instructions"], "dispatch count differs")
    require(candidate["output_hex"] == case["output"].hex() and candidate["output_bytes"] == len(case["output"]),
            "raw output differs from boundary expectation")
    if case["production_error"]:
        require(production["status"] == "admission_refused" and production["error"] == case["production_error"],
                "production admission contract changed")
    else:
        require(production == candidate, "production and raw dispatch differ")
    if case["validation"]:
        require(reference is None, "validation-only fixture unexpectedly ran a reference")
        return "expected_validation_refusal"
    reference_status = "returned" if case["policy"] or not case["error"] else "execution_refused"
    require(reference is not None and reference["status"] == reference_status and reference["error"] == case["reference_error"],
            "finite reference outcome differs")
    expected_result = case["reference_result"] if case["policy"] else case["result"]
    expected_output = case["reference_output"] if case["reference_output"] is not None else case["output"]
    require(reference["result"] == expected_result and reference["output_hex"] == expected_output.hex(),
            "exact reference result/output differs")
    return "finite_runtime_policy_contrast" if case["policy"] else "matching_runtime_refusal" if case["error"] else "matching_semantics"


def boundary_proof(args, output, environment):
    directory = output / "boundary-editions"
    directory.mkdir()
    records, failures = [], []
    for number, case in enumerate(generated_cases()):
        path = directory / f"{number:03d}-{case['name']}.nil"
        path.write_bytes(case["edition"])
        record = {key: value for key, value in case.items() if key not in {"edition", "output", "reference_output"}}
        record.update(edition_bytes=len(case["edition"]), edition_sha256=digest(case["edition"]),
                      retained_path=str(path.relative_to(output)), expected_output_hex=case["output"].hex(),
                      reference_output_hex=None if case["reference_output"] is None else case["reference_output"].hex())
        try:
            values = {}
            for name, tool, parser in (("candidate", args.candidate, parse_probe),
                                       ("production", args.production_candidate, flow.parse_probe)):
                prefix = "raw" if name == "candidate" else "production"
                process, stdout, _ = baseline.run([tool, path, str(case["ceiling"]), str(case["steps"]), str(case["frames"])],
                                                  output / f"boundary-{prefix}-{number:03d}", environment=environment)
                values[name] = parser(process, stdout)
            reference = None
            if not case["validation"]:
                process, stdout, _ = baseline.run([args.reference, path], output / f"boundary-reference-{number:03d}",
                                                  environment=environment)
                reference = parse_probe(process, stdout, reference=True)
            record.update(values, reference=reference)
            record["outcome"] = compare_case(case, values["candidate"], reference, values["production"])
            if case["cross_profile"]:
                for name, tool, parser, status in (("array", args.array_candidate, flow.parse_probe, "admission_refused"),
                                                   ("array_raw", args.array_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])],
                        output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_intent" if name == "array" and not case["annotated"] else "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier array grammar expanded")
                    record[name] = previous
        except (AssertionError, ValueError, OSError) as error:
            record.update(outcome="failed", failure=str(error))
            failures.append(case["name"])
        records.append(record)
    write_json(output / "boundary-results.json", {"cases": records, "failures": failures})
    return {"passed": not failures, "cases": len(records), "failures": failures,
            "semantic_matches": sum(item["outcome"] == "matching_semantics" for item in records),
            "matching_runtime_refusals": sum(item["outcome"] == "matching_runtime_refusal" for item in records),
            "finite_runtime_policy_contrasts": sum(item["outcome"] == "finite_runtime_policy_contrast" for item in records),
            "expected_validation_refusals": sum(item["outcome"] == "expected_validation_refusal" for item in records),
            "production_comparisons": sum("production" in item for item in records),
            "array_refusals": sum("array" in item for item in records),
            "array_conformance_refusals": sum("array_raw" in item for item in records)}


def instruction_inventory(data):
    """Lexical metadata only; complete admission still checks targets and CFG."""
    result = arrays.instruction_inventory(data)
    result.update(profile=6, lane="raw_callable_conformance",
                  scope="profile-6 lexical coverage only; target provenance, signatures, authority and CFG require complete validation")
    locals_by_function = {}
    for item in result["instructions"]:
        if item["opcode"] == "STORE_LOCAL" and len(item["operands"]) == 1:
            locals_by_function.setdefault(item["function"], set()).add(item["operands"][0])
    for item in result["instructions"]:
        opcode, operands = item["opcode"], item["operands"]
        if opcode == "PUSH_FN_PTR":
            item["supported_opcode"] = True
            item["supported_form"] = (len(operands) == 1 and
                re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", operands[0], re.ASCII) is not None)
        elif opcode == "CALL" and len(operands) == 2 and operands[0] in locals_by_function.get(item["function"], set()):
            item["supported_form"] = (re.fullmatch(r"[0-9]+", operands[1], re.ASCII) is not None and int(operands[1]) <= 8)
    return result


@contextmanager
def corpus_adapters():
    previous = baseline.parse_probe, baseline.instruction_inventory
    baseline.parse_probe, baseline.instruction_inventory = parse_probe, instruction_inventory
    try:
        yield
    finally:
        baseline.parse_probe, baseline.instruction_inventory = previous


def raw_regression_contract(records):
    failures = []
    if [item["invocation"] for item in records] != list(range(45)):
        failures.append("raw invocation sequence changed")
    matched = [item["invocation"] for item in records if item.get("matched") is True]
    if matched != list(known_raw_matches):
        failures.append("known raw semantic coverage changed")
    for record in records:
        expected = known_raw_matches.get(record["invocation"])
        if expected is not None and (record.get("edition_bytes"), record.get("edition_sha256")) != expected:
            failures.append(f"known complete edition changed: {record['invocation']}")
        if record.get("outcome") != ("returned_match" if expected is not None else "validation_refused"):
            failures.append(f"known corpus outcome changed: {record['invocation']}")
    errors = dict(Counter(item["candidate"]["error"] for item in records if item.get("outcome") == "validation_refused"))
    expected_errors = {"manifest_unsupported": 27, "manifest_edition": 9}
    if errors != expected_errors:
        failures.append("raw validation refusal inventory changed")
    return {"passed": not failures, "failures": failures, "matched_invocations": matched,
            "expected_matched_invocations": list(known_raw_matches), "expected_refusal_errors": expected_errors,
            "edition_pins": [{"invocation": number, "bytes": size, "sha256": identity}
                             for number, (size, identity) in known_raw_matches.items()]}


def qualify_corpus(args, output, environment, before):
    with corpus_adapters():
        raw = baseline.corpus_proof(args, output, environment, before)
    records = baseline.strict_json((output / "corpus-executions.json").read_text())
    for record in records:
        record["lane"] = "raw_callable_conformance"
    raw.update(lane="raw_callable_conformance",
               unsupported_or_refused_executions=sum(item["outcome"] in {"validation_refused", "execution_refused"} for item in records),
               refusal_errors=dict(Counter(item["candidate"]["error"] for item in records
                                           if item["outcome"] in {"validation_refused", "execution_refused"})),
               scope="Exact unchanged upstream semantics; production intent is a separate lane. Refusal is unmet coverage.",
               regression_contract=raw_regression_contract(records))
    write_json(output / "corpus-executions.json", records)
    write_json(output / "corpus-summary.json", raw)
    production_records, failures, cross = [], [], []
    for record in records:
        require(all(key in record for key in ("invocation", "upstream_call", "edition_sha256", "edition_bytes", "retained_path")),
                "raw corpus record lacks complete edition binding")
        number = record["invocation"]
        item = {key: record[key] for key in ("invocation", "upstream_call", "edition_sha256", "edition_bytes", "retained_path")}
        item["lane"] = "production_profile6_admission"
        try:
            path = output / record["retained_path"]
            data = path.read_bytes()
            require(len(data) == record["edition_bytes"] and digest(data) == record["edition_sha256"], "corpus bytes changed between lanes")
            process, stdout, _ = baseline.run([args.production_candidate, path, "1"], output / f"corpus-production-{number:03d}",
                                              environment=environment)
            candidate = flow.parse_probe(process, stdout)
            item["candidate"] = candidate
            require(candidate["status"] == "admission_refused", "unmodified corpus unexpectedly passed production admission")
            item["outcome"] = "production_refused"
            if number == 31:
                pair = dict(item, lane="prior_array_grammar")
                for name, tool, parser, status in (("array", args.array_candidate, flow.parse_probe, "admission_refused"),
                                                   ("array_raw", args.array_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] ==
                            ("manifest_intent" if name == "array" else "manifest_unsupported"), "earlier corpus array contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    # All original editions still lack main intent. Keep the complete measured
    # first-error inventory pinned; new callable parsing can change error order
    # in helpers before main, so the initial development run must verify it.
    expected = {"manifest_unsupported": 24, "manifest_edition": 9, "manifest_intent": 12}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 1
    production = {"lane": "production_profile6_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "array_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "array-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "array_candidate", "array_conformance_candidate", "reference", "compiler", "interpreter")
    for name in (*binary_names[:-1], "root", "output"):
        setattr(args, name, getattr(args, name).resolve())
    args.interpreter = (args.interpreter or args.compiler.with_name("nilinterpreter")).resolve()
    require(not args.output.exists(), "output must be a new directory; evidence is never overwritten")
    args.output.mkdir(parents=True)
    args.output_created = True
    tools = {name: baseline.record_tool(getattr(args, name)) for name in binary_names}
    require(tools["compiler"]["sha256"] == baseline.compiler_hash and tools["interpreter"]["sha256"] == baseline.interpreter_hash,
            "qualified toolchain pins changed")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.root, text=True).strip() == baseline.revision,
            "qualified upstream revision changed")
    require(not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=args.root, text=True),
            "qualified upstream tracked source changed")
    tools["reference_core_archives"] = []
    for name, expected in archive_pins.items():
        record = baseline.record_tool(args.root / "build/context-registration/lib" / name)
        require(record["sha256"] == expected, f"qualified archive changed: {name}")
        tools["reference_core_archives"].append(record)
    tools["earlier_profile_sources"] = []
    for name, expected in source_pins.items():
        record = baseline.record_tool(Path(__file__).with_name(name))
        require(record["sha256"] == expected, f"earlier proof source changed: {name}")
        tools["earlier_profile_sources"].append(record)
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_callable_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_callable host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 6 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536, "array_descriptors": 128, "array_elements": 1024, "array_literal_elements": 256},
        "earlier_proofs": "126-, 77-, 61-, 27-, 55- and 87-case proofs remain separate required runs",
    })
    environment = baseline.clean_environment()
    boundary = boundary_proof(args, args.output, environment)
    raw, production = qualify_corpus(args, args.output, environment, before)
    require(baseline.inventory(args.root) == before, "upstream inputs changed during proof")
    for name in binary_names:
        require(baseline.record_tool(getattr(args, name)) == tools[name], f"tool changed: {name}")
    for record in [*tools["reference_core_archives"], *tools["earlier_profile_sources"], tools["reference_source"], tools["script"]]:
        require(baseline.record_tool(Path(record["path"])) == record, "proof source/archive changed")
    for number, case in enumerate(generated_cases()):
        path = args.output / "boundary-editions" / f"{number:03d}-{case['name']}.nil"
        require(path.read_bytes() == case["edition"], "boundary input changed during proof")
    passed = baseline.checkpoint_passed(boundary, raw) and raw["regression_contract"]["passed"] and production["passed"]
    result = {"schema": 1, "status": "bounded_callable_checkpoint_passed" if passed else "bounded_callable_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 6, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_and_exact_counts_are_explicit(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 103)
        self.assertEqual(len(cases), len({item["name"] for item in cases}))
        self.assertEqual(sum(item["cross_profile"] for item in cases), 49)
        self.assertEqual(sum(item["policy"] for item in cases), 7)
        self.assertEqual(sum(item["validation"] for item in cases), 54)
        self.assertEqual(sum(not item["error"] for item in cases), 40)
        self.assertTrue(all(type(item["instructions"]) is int for item in cases))
        self.assertTrue(all(item["origin"] == "independently_authored_textual_nil" for item in cases))
        self.assertTrue(all(item["reference_result"] is not None for item in cases if item["policy"]))
        self.assertIn("loop_rebind_same_signature", {item["name"] for item in cases})
        self.assertIn("reject_future_local_overrides_builtin", {item["name"] for item in cases})
        self.assertEqual(len(source_pins), 11)

    def test_lexical_coverage_does_not_claim_provenance_or_signature(self):
        for name in ("zero_argument_forward_target", "local_precedes_builtin_println", "mixed_target_signature",
                     "computed_text_array_return", "generated_callee_temporary", "stack_target_join_0"):
            case = next(item for item in generated_cases() if item["name"] == name)
            coverage = instruction_inventory(case["edition"])
            self.assertEqual(coverage["profile"], 6)
            self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]))
            self.assertEqual(coverage["unsupported_declarations"], [])
            self.assertIn("lexical", coverage["scope"])
        for name in ("reject_forged_text_name", "reject_target_missing", "reject_rebind_different_i32_result"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(all(item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))
        for name in ("reject_missing_operand", "reject_extra_capture_operand", "reject_call_count_over_capacity"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(any(not item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))

    def test_exact_semantics_and_policy_contrasts_cannot_be_confused(self):
        case = next(item for item in generated_cases() if item["name"] == "i64_result_exact_above_53_bits")
        candidate = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
                     "instructions": 6, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": candidate["result"], "error": None, "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        case = next(item for item in generated_cases() if item["name"] == "finite_frame_limit_partial_output")
        candidate = {"status": "execution_refused", "result": None, "error": "frame_limit", "output_hex": "370a",
                     "instructions": 8, "console_calls": 1, "output_bytes": 2}
        reference = dict(reference, result=42, output_hex="370a")
        self.assertEqual(compare_case(case, candidate, reference, candidate), "finite_runtime_policy_contrast")
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, dict(reference, status="execution_refused", result=None, error="timeout"), candidate)

    def test_adapters_restore_previous_helpers_on_failure(self):
        previous = baseline.parse_probe, baseline.instruction_inventory
        with self.assertRaises(RuntimeError):
            with corpus_adapters():
                self.assertIs(baseline.instruction_inventory, instruction_inventory)
                raise RuntimeError("deliberate")
        self.assertEqual(previous, (baseline.parse_probe, baseline.instruction_inventory))

    def test_positive_corpus_coverage_cannot_regress_to_refusal(self):
        records, unsupported = [], 0
        for number in range(45):
            if number in known_raw_matches:
                size, identity = known_raw_matches[number]
                records.append({"invocation": number, "matched": True, "outcome": "returned_match", "edition_bytes": size,
                                "edition_sha256": identity, "candidate": {"error": None}})
            else:
                error = "manifest_unsupported" if unsupported < 27 else "manifest_edition"
                unsupported += 1
                records.append({"invocation": number, "matched": False, "outcome": "validation_refused", "candidate": {"error": error}})
        self.assertTrue(raw_regression_contract(records)["passed"])
        for number in known_raw_matches:
            changed = [dict(item, matched=False, outcome="validation_refused", candidate={"error": "manifest_unsupported"})
                       if item["invocation"] == number else item for item in records]
            self.assertFalse(raw_regression_contract(changed)["passed"])
            changed = [dict(item, edition_sha256="0" * 64) if item["invocation"] == number else item for item in records]
            self.assertFalse(raw_regression_contract(changed)["passed"])
        self.assertFalse(raw_regression_contract(records[:-1])["passed"])

    def test_resource_and_authority_policies_remain_explicit(self):
        cases = {item["name"]: item for item in generated_cases()}
        self.assertEqual(cases["frames_exact_capacity"]["frames"], 3)
        self.assertEqual(cases["finite_frame_limit_partial_output"]["frames"], 2)
        self.assertEqual(cases["aggregate_stack_257"]["error"], "stack_limit")
        self.assertEqual(cases["local_precedes_builtin_println"]["ceiling"], 0)
        self.assertIsNone(cases["local_precedes_builtin_println"]["error"])
        self.assertEqual(cases["reject_unused_console_body_denied"]["error"], "manifest_ceiling")
        self.assertEqual(cases["unused_console_body_granted"]["output"], b"")
        self.assertEqual(cases["reject_rebind_different_i32_result"]["error"], "manifest_syntax")

    def test_reference_bounds_mode_is_unset_and_old_sources_pinned(self):
        environment = baseline.clean_environment()
        self.assertNotIn("NIL_STRICT_BOUNDS", environment)
        self.assertEqual(environment["NIL_CRASH_REPORT"], "0")
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "array-candidate", "array-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "array_candidate", "array_conformance_candidate", "reference", "compiler", "root", "output"):
        if getattr(args, name) is None:
            parser.error(f"--{name.replace('_', '-')} is required")
    try:
        return qualification(args)
    except (AssertionError, ValueError, OSError) as error:
        failure = {"schema": 1, "status": "qualification_infrastructure_failed", "candidate_qualified": False,
                   "native_execution": False, "production_admission_waived": False, "error": str(error)}
        if getattr(args, "output_created", False):
            write_json(args.output / "failure.json", failure)
        print(json.dumps(failure), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
