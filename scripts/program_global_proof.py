#!/usr/bin/env python3
"""Qualify bounded profile-8 module globals and integer text casts without changing earlier profiles.

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
import program_record_proof as records
import program_conformance_proof as conformance


require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe = conformance.parse_probe
source_pins = dict(records.source_pins, **{
    "program_record_proof.py": "ef81c6bea6be3099564a99f4a4b56bb6772716a1954b9d9265bc10c5f488c5e0",
    "program_record_reference.cpp": "e2a59ecf729fff58958f59a8f9be4fe802da343a16ed9cffd72a95bdae29346e",
})
archive_pins = records.archive_pins
known_raw_matches = dict(sorted({**records.known_raw_matches,
    43: (4214, "0906256ac45e18e81df635e70f9657fb6c74cc42e5907add63de4b1fd1d135e5"),
}.items()))


def generated_cases():
    cases = []
    annotation = '@ai_context("Exercise bounded module globals and integer formatting")'

    def add(name, body, *, helpers=(), strings=(), layouts=(), result=42, output=b"", calls=0,
            instructions=None, error=None, validation=False, policy=False,
            reference_result=None, reference_output=None, reference_error=None,
            ceiling=1, steps=65536, frames=16, annotated=True, production_error=None,
            cross_profile=True):
        functions = [flow.function("main", body), *helpers]
        annotations = {"main": (annotation,)} if annotated else None
        edition = (records.authored_record_edition(functions, layouts, strings, annotations) if layouts else
                   conformance.authored_edition(functions, strings, annotations))
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

    def loop(count, body):
        return ["PUSH_CONST 0", "STORE_LOCAL index", "LABEL loop", "LOAD_LOCAL index", f"PUSH_CONST {count}", "CMP_LT",
                "JMP_IF_NOT done", *body, "LOAD_LOCAL index", "PUSH_CONST 1", "ADD", "STORE_LOCAL index", "JMP loop", "LABEL done"]

    add("global_store_and_load", ["PUSH_CONST 42", "STORE_GLOBAL answer i64", "LOAD_GLOBAL answer i64", "RET"], ceiling=0)
    add("global_store_consumes_value", ["PUSH_CONST 1", "PUSH_CONST 42", "STORE_GLOBAL answer i64", "LOAD_GLOBAL answer i64", "ADD", "RET"], result=43, ceiling=0)
    add("qualified_name", ["PUSH_CONST 42", "STORE_GLOBAL app::module::answer i64", "LOAD_GLOBAL app::module::answer i64", "RET"], ceiling=0)
    add("global_repeated_assignment", ["PUSH_CONST 7", "STORE_GLOBAL answer i64", "PUSH_CONST 42", "STORE_GLOBAL answer i64", "LOAD_GLOBAL answer i64", "RET"], ceiling=0)
    add("repeated_name_does_not_consume_slots", ["PUSH_CONST 42", "STORE_GLOBAL answer i64"] * 129 + ["LOAD_GLOBAL answer i64", "RET"], ceiling=0)
    for name, value in (("zero", 0), ("negative", -1), ("minimum", -(1 << 63)), ("maximum", (1 << 63) - 1), ("exact_above_53_bits", 9007199254740993)):
        add("global_i64_" + name, [f"PUSH_CONST {value}", "STORE_GLOBAL value i64", "LOAD_GLOBAL value i64", "RET"], result=value, ceiling=0)
    add("global_case_sensitive_identity", ["PUSH_CONST 7", "STORE_GLOBAL value i64", "PUSH_CONST 42", "STORE_GLOBAL Value i64",
        "LOAD_GLOBAL value i64", "CONSOLE_WRITELN", "LOAD_GLOBAL Value i64", "RET"], output=b"7\n", calls=1)
    add("local_and_global_separate", ["PUSH_CONST 7", "STORE_LOCAL value", "PUSH_CONST 42", "STORE_GLOBAL value i64", "LOAD_LOCAL value",
        "CONSOLE_WRITELN", "LOAD_GLOBAL value i64", "RET"], output=b"7\n", calls=1)
    answer = flow.function("answer", ["PUSH_CONST 7", "RET"])
    add("global_does_not_shadow_function", ["PUSH_CONST 42", "STORE_GLOBAL answer i64", "CALL answer 0", "CONSOLE_WRITELN", "LOAD_GLOBAL answer i64", "RET"],
        helpers=(answer,), output=b"7\n", calls=1, instructions=8)
    add("global_does_not_shadow_builtin", ["PUSH_CONST 42", "STORE_GLOBAL println i64", "PUSH_CONST 7", "CALL println 1", "LOAD_GLOBAL println i64", "RET"],
        output=b"7\n", calls=1)
    add("global_and_local_callable_separate", ["PUSH_CONST 42", "STORE_GLOBAL operation i64", "PUSH_FN_PTR answer", "STORE_LOCAL operation",
        "CALL operation 0", "LOAD_GLOBAL operation i64", "ADD", "RET"], helpers=(answer,), result=49, instructions=10, ceiling=0)
    add("global_and_string_constant_separate", ["PUSH_CONST 42", "STORE_GLOBAL S0 i64", "PUSH_CONST_STR S0", "CONSOLE_WRITE", "LOAD_GLOBAL S0 i64", "RET"],
        strings=("constant",), output=b"constant", calls=1)
    add("global_and_nominal_record_separate", ["PUSH_CONST 42", "STORE_GLOBAL counter i64", "NEW_STRUCT counter", "POP", "LOAD_GLOBAL counter i64", "RET"],
        layouts=(records.layout("counter", ("value",)),), ceiling=0)
    for name in ("i64", "bool", "f64", "void", "main"):
        add("global_name_" + name, ["PUSH_CONST 42", f"STORE_GLOBAL {name} i64", f"LOAD_GLOBAL {name} i64", "RET"], ceiling=0)
    increment = flow.function("increment", ["LOAD_GLOBAL app::counter i64", "PUSH_CONST 1", "ADD", "STORE_GLOBAL app::counter i64", "RET"], result="void")
    add("global_shared_across_direct_calls", ["PUSH_CONST 0", "STORE_GLOBAL app::counter i64", "CALL increment 0", "CALL increment 0", "LOAD_GLOBAL app::counter i64", "RET"],
        helpers=(increment,), result=2, instructions=16, ceiling=0)
    add("global_shared_across_indirect_call", ["PUSH_CONST 42", "STORE_GLOBAL app::counter i64", "PUSH_FN_PTR increment", "STORE_LOCAL operation",
        "CALL operation 0", "LOAD_GLOBAL app::counter i64", "RET"], helpers=(increment,), result=43, instructions=12, ceiling=0)
    initializer = flow.function("__globals_init__", ["PUSH_CONST 40", "STORE_GLOBAL app::seed i64", "LOAD_GLOBAL app::seed i64", "PUSH_CONST 2", "ADD",
        "STORE_GLOBAL app::derived i64", "RET"], result="void")
    add("explicit_initializer_dependency_order", ["CALL __globals_init__ 0", "LOAD_GLOBAL app::derived i64", "RET"], helpers=(initializer,), instructions=10, ceiling=0)
    reset = flow.function("__globals_init__", ["PUSH_CONST 1", "STORE_GLOBAL app::counter i64", "RET"], result="void")
    add("repeated_initializer_executes_each_call", ["CALL __globals_init__ 0", "LOAD_GLOBAL app::counter i64", "PUSH_CONST 1", "ADD",
        "STORE_GLOBAL app::counter i64", "CALL __globals_init__ 0", "LOAD_GLOBAL app::counter i64", "RET"], helpers=(reset,), result=1, instructions=14, ceiling=0)
    noisy_init = flow.function("__globals_init__", ["PUSH_CONST 7", "STORE_GLOBAL app::counter i64", "PUSH_CONST_STR S0", "CONSOLE_WRITE", "RET"], result="void")
    add("initializer_is_not_automatic", ["PUSH_CONST 42", "RET"], helpers=(noisy_init,), strings=("unexpected",), instructions=2)
    add("initializer_definition_does_not_initialize", ["LOAD_GLOBAL app::counter i64", "RET"], helpers=(reset,), error="global_uninitialized", instructions=1,
        reference_error="Global not found: app::counter", ceiling=0)
    add("load_only_uninitialized_slot", ["LOAD_GLOBAL missing i64", "RET"], error="global_uninitialized", instructions=1,
        reference_error="Global not found: missing", ceiling=0)
    add("uninitialized_preserves_output", ["PUSH_CONST 7", "CONSOLE_WRITELN", "LOAD_GLOBAL app::missing i64", "RET"],
        output=b"7\n", calls=1, error="global_uninitialized", instructions=3, reference_error="Global not found: app::missing")
    for condition in (0, 1):
        add(f"conditional_global_initialization_{condition}", [f"PUSH_CONST {condition}", "JMP_IF_NOT done", "PUSH_CONST 42", "STORE_GLOBAL app::maybe i64",
            "LABEL done", "LOAD_GLOBAL app::maybe i64", "RET"], error=None if condition else "global_uninitialized", instructions=7 if condition else 4,
            reference_error=None if condition else "Global not found: app::maybe", ceiling=0)
    reader = flow.function("reader", ["LOAD_GLOBAL app::missing i64", "RET"])
    add("uninitialized_in_indirect_callee", ["PUSH_CONST 7", "CONSOLE_WRITELN", "PUSH_FN_PTR reader", "STORE_LOCAL operation", "CALL operation 0", "RET"],
        helpers=(reader,), output=b"7\n", calls=1, error="global_uninitialized", instructions=6, reference_error="Global not found: app::missing")
    add("cast_output_before_uninitialized_load", ["PUSH_CONST 17", "CAST text", "CONSOLE_WRITE", "LOAD_GLOBAL missing i64", "RET"], output=b"17", calls=1,
        error="global_uninitialized", instructions=4, reference_error="Global not found: missing")
    add("global_output_before_division_fault", ["PUSH_CONST 7", "STORE_GLOBAL value i64", "LOAD_GLOBAL value i64", "CONSOLE_WRITELN", "PUSH_CONST 5", "PUSH_CONST 0",
        "DIV i64", "RET"], output=b"7\n", calls=1, error="division_by_zero", instructions=7, reference_error="Division by zero")
    add("intentless_global", ["PUSH_CONST 42", "STORE_GLOBAL answer i64", "LOAD_GLOBAL answer i64", "RET"], annotated=False, ceiling=0)
    for name, global_name in (("simple", "x" * 128), ("qualified", "a::" + "b" * 125)):
        add("global_name_exact_capacity_" + name, ["PUSH_CONST 42", f"STORE_GLOBAL {global_name} i64", f"LOAD_GLOBAL {global_name} i64", "RET"], ceiling=0)
    fill = [instruction for index in range(128) for instruction in (f"PUSH_CONST {index}", f"STORE_GLOBAL global_{index} i64")]
    add("global_slots_exact_capacity", fill + ["LOAD_GLOBAL global_127 i64", "RET"], result=127, ceiling=0)
    add("dead_load_names_exact_capacity", ["PUSH_CONST 42", "RET"] + [f"LOAD_GLOBAL global_{index} i64" for index in range(128)], instructions=2, ceiling=0)
    add("last_slot_after_dead_load_names", ["JMP live"] + [f"LOAD_GLOBAL global_{index} i64" for index in range(127)] +
        ["LABEL live", "PUSH_CONST 42", "STORE_GLOBAL global_127 i64", "LOAD_GLOBAL global_127 i64", "RET"], instructions=6, ceiling=0)
    add("dead_store_keeps_syntax_only", ["PUSH_CONST 42", "RET", "STORE_GLOBAL dead i64"], instructions=2, ceiling=0)

    for name, literal, value in (("zero", "0", 0), ("one", "1", 1), ("negative", "-1", -1), ("negative_zero", "-0", 0),
                                 ("minimum", str(-(1 << 63)), -(1 << 63)), ("maximum", str((1 << 63) - 1), (1 << 63) - 1),
                                 ("exact_above_53_bits", "9007199254740993", 9007199254740993), ("hex_source", "0x2a", 42)):
        add("cast_decimal_" + name, [f"PUSH_CONST {literal}", "CAST text", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], output=str(value).encode(), calls=1)
    add("cast_and_concat", ["PUSH_CONST -42", "CAST text", "PUSH_CONST_STR S0", "STR_CONCAT", "PUSH_CONST 7", "CAST text", "STR_CONCAT", "CONSOLE_WRITE",
        "PUSH_CONST 42", "RET"], strings=(":",), output=b"-42:7", calls=1)
    formatter = flow.function("format_global", ["LOAD_GLOBAL app::number i64", "CAST text", "RET"], result="text")
    add("returned_cast_is_immutable_snapshot", ["PUSH_CONST 42", "STORE_GLOBAL app::number i64", "CALL format_global 0", "STORE_LOCAL held", "PUSH_CONST 7",
        "STORE_GLOBAL app::number i64", "LOAD_LOCAL held", "CONSOLE_WRITE", "CALL format_global 0", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        helpers=(formatter,), output=b"427", calls=2, instructions=18)
    boxed = flow.function("boxed_text", ["LOAD_GLOBAL app::number i64", "CAST text", "NEW_ARRAY_LITERAL 1 text", "RET"], result="array<text>")
    add("cast_inside_returned_array", [f"PUSH_CONST {-(1 << 63)}", "STORE_GLOBAL app::number i64", "PUSH_FN_PTR boxed_text", "STORE_LOCAL operation", "CALL operation 0",
        "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], helpers=(boxed,), output=str(-(1 << 63)).encode(), calls=1, instructions=14)
    add("cast_alias_survives_concat", ["PUSH_CONST 42", "CAST text", "STORE_LOCAL held", "LOAD_LOCAL held", "PUSH_CONST_STR S0", "STR_CONCAT", "CONSOLE_WRITE",
        "LOAD_LOCAL held", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], strings=("!",), output=b"42!42", calls=2)
    add("intentless_cast", ["PUSH_CONST 42", "CAST text", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], output=b"42", calls=1, annotated=False)
    add("dead_cast_keeps_syntax_only", ["PUSH_CONST 42", "RET", "CAST text"], instructions=2, ceiling=0)
    cast16 = ["PUSH_CONST 9007199254740993", "CAST text", "POP"]
    add("cast_arena_exact_capacity", loop(4096, cast16) + ["PUSH_CONST 42", "RET"], instructions=53258, ceiling=0)
    add("cast_arena_one_more_value", loop(4097, cast16) + ["PUSH_CONST 42", "RET"], error="text_limit", instructions=53257,
        policy=True, reference_result=42, reference_output=b"", ceiling=0)
    add("cast_alias_after_arena_capacity", ["PUSH_CONST 9007199254740993", "CAST text", "STORE_LOCAL held"] + loop(4095, cast16) +
        ["LOAD_LOCAL held", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], output=b"9007199254740993", calls=1, instructions=53250)
    cast20 = [f"PUSH_CONST {-(1 << 63)}", "CAST text", "POP"]
    add("mixed_width_casts_exact_capacity", loop(3276, cast20) + cast16 + ["PUSH_CONST 42", "RET"], instructions=42601, ceiling=0)
    add("cast_arena_not_reclaimed_after_return", ["PUSH_CONST 9007199254740993", "STORE_GLOBAL app::number i64"] +
        loop(4097, ["CALL format_global 0", "POP"]) + ["PUSH_CONST 42", "RET"], helpers=(formatter,),
        error="text_limit", instructions=61452, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    payload = str(-(1 << 63)).encode()
    add("cast_alias_total_output_limit", [f"PUSH_CONST {-(1 << 63)}", "CAST text", "STORE_LOCAL held"] +
        loop(3277, ["LOAD_LOCAL held", "CONSOLE_WRITE"]) + ["PUSH_CONST 42", "RET"], output=payload * 3276, calls=3276,
        error="output_limit", instructions=39324, policy=True, reference_result=42, reference_output=payload * 3277)
    add("global_finite_fuel", ["PUSH_CONST 42", "STORE_GLOBAL value i64", "LOAD_GLOBAL value i64", "RET"], error="instruction_limit", instructions=2,
        steps=2, policy=True, reference_result=42, reference_output=b"", ceiling=0)

    for name, operand in (("dotted", "a.b"), ("empty_segment", "a::::b"), ("leading_separator", "::value"), ("trailing_separator", "value::"),
                           ("numeric_prefix", "9value"), ("unicode", "café")):
        reject("global_name_" + name, ["PUSH_CONST 42", "RET", f"LOAD_GLOBAL {operand} i64"])
    reject("global_name_over_capacity", ["PUSH_CONST 42", "RET", "LOAD_GLOBAL " + "x" * 129 + " i64"], error="manifest_limit")
    reject("dead_global_slots_over_capacity", ["PUSH_CONST 42", "RET"] + [f"LOAD_GLOBAL global_{index} i64" for index in range(129)], error="manifest_limit")
    reject("mixed_load_store_slots_over_capacity", fill + ["PUSH_CONST 42", "RET", "LOAD_GLOBAL extra i64"], error="manifest_limit")
    for name, kind in (("i32", "i32"), ("bool", "bool"), ("text", "text"), ("array", "array<i64>"), ("record", "box"), ("alias", "int")):
        reject("global_type_" + name, ["PUSH_CONST 42", "RET", f"LOAD_GLOBAL value {kind}"], error="manifest_unsupported")
    for name, opcode in (("load_missing_type", "LOAD_GLOBAL value"), ("load_extra_operand", "LOAD_GLOBAL value i64 extra"),
                          ("store_missing_type", "STORE_GLOBAL value"), ("store_extra_operand", "STORE_GLOBAL value i64 extra"),
                          ("cast_extra_operand", "CAST text extra"), ("cast_string_alias", "CAST string")):
        reject(name, ["PUSH_CONST 42", "RET", opcode], error="manifest_unsupported")
    reject("global_type_conflict", ["PUSH_CONST 42", "STORE_GLOBAL value i64", "LOAD_GLOBAL value text", "POP", "PUSH_CONST 42", "RET"], error="manifest_unsupported")
    reject("store_underflow", ["STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"])
    reject("store_text_value", ["PUSH_CONST_STR S0", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"], strings=("text",))
    reject("store_array_value", ["NEW_ARRAY_LITERAL 0 i64", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"])
    reject("store_record_value", ["NEW_STRUCT box", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"], layouts=(records.layout("box", ("value",)),))
    reject("store_callable_value", ["PUSH_FN_PTR answer", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"], helpers=(answer,))
    reject("local_load_has_no_global_fallback", ["PUSH_CONST 42", "STORE_GLOBAL value i64", "LOAD_LOCAL value", "RET", "PUSH_CONST 7", "STORE_LOCAL value"])
    reject("call_has_no_global_fallback", ["PUSH_CONST 42", "STORE_GLOBAL operation i64", "CALL operation 0", "RET"], error="manifest_unsupported")
    reject("global_does_not_initialize_callable_local", ["PUSH_CONST 42", "STORE_GLOBAL operation i64", "CALL operation 0", "RET", "PUSH_FN_PTR answer", "STORE_LOCAL operation"], helpers=(answer,))
    for name, prefix, kwargs in (
        ("text", ["PUSH_CONST_STR S0"], {"strings": ("42",)}),
        ("array", ["NEW_ARRAY_LITERAL 0 i64"], {}),
        ("record", ["NEW_STRUCT box"], {"layouts": (records.layout("box", ("value",)),)}),
        ("callable", ["PUSH_FN_PTR answer"], {"helpers": (answer,)}),
        ("underflow", [], {}),
    ):
        reject("integer_only_cast_" + name, prefix + ["CAST text", "POP", "PUSH_CONST 42", "RET"], **kwargs)
    reject("cast_result_is_not_integer", ["PUSH_CONST 42", "CAST text", "RET"])
    reject("cast_result_not_global_i64", ["PUSH_CONST 42", "CAST text", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"])
    reject("cast_result_not_array_i64", ["PUSH_CONST 42", "CAST text", "NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 42", "RET"])
    reject("cast_result_not_callable", ["PUSH_CONST 42", "CAST text", "STORE_LOCAL operation", "CALL operation 0", "RET"])
    reject("unused_initializer_console_ceiling", ["PUSH_CONST 42", "RET"], helpers=(noisy_init,), strings=("unused",), ceiling=0, error="manifest_ceiling")
    reject("dead_console_ceiling", ["PUSH_CONST 42", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET", "PUSH_CONST 7", "CONSOLE_WRITE"], ceiling=0, error="manifest_ceiling")
    reject("dead_foreign_effect", ["PUSH_CONST 42", "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"], error="manifest_unsupported")
    require(len({case["name"] for case in cases}) == len(cases), "duplicate global boundary case")
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
                for name, tool, parser, status in (("record", args.record_candidate, flow.parse_probe, "admission_refused"),
                                                   ("record_raw", args.record_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])],
                        output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_intent" if name == "record" and not case["annotated"] else "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier record grammar expanded")
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
            "record_refusals": sum("record" in item for item in records),
            "record_conformance_refusals": sum("record_raw" in item for item in records)}


def instruction_inventory(data):
    """Lexical metadata only; complete admission checks kinds and whole-file names."""
    result = records.instruction_inventory(data)
    result.update(profile=8, lane="raw_global_conformance",
                  scope="profile-8 lexical coverage only; global slot bounds, operand kinds, authority and CFG require complete validation")
    name = r"[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*"
    for item in result["instructions"]:
        opcode, operands = item["opcode"], item["operands"]
        if opcode in {"LOAD_GLOBAL", "STORE_GLOBAL"}:
            item["supported_opcode"] = True
            item["supported_form"] = (len(operands) == 2 and operands[1] == "i64" and
                re.fullmatch(name, operands[0], re.ASCII) is not None and len(operands[0].encode()) <= 128)
        elif opcode == "CAST" and operands == ["text"]:
            item["supported_form"] = True
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
    expected_errors = {"manifest_unsupported": 23, "manifest_edition": 9}
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
        record["lane"] = "raw_global_conformance"
    raw.update(lane="raw_global_conformance",
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
        item["lane"] = "production_profile8_admission"
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
            if number == 43:
                pair = dict(item, lane="prior_record_grammar")
                for name, tool, parser, status in (("record", args.record_candidate, flow.parse_probe, "admission_refused"),
                                                   ("record_raw", args.record_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] ==
                            "manifest_unsupported", "earlier corpus record contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    # All original editions still lack main intent. Keep the complete measured
    # first-error inventory pinned; new global parsing can change error order
    # in helpers before main, so the initial development run must verify it.
    expected = {"manifest_unsupported": 20, "manifest_edition": 9, "manifest_intent": 16}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 1
    production = {"lane": "production_profile8_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "record_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "record-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "record_candidate", "record_conformance_candidate", "reference", "compiler", "interpreter")
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
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_global_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_globals host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 8 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536, "array_descriptors": 128, "array_elements": 1024, "array_literal_elements": 256, "record_layouts": 64, "fields_per_layout": 32, "declared_fields": 256, "record_descriptors": 128, "record_cells": 1024, "global_slots": 128, "global_name_bytes": 128, "integer_text_maximum_bytes": 20},
        "earlier_proofs": "126-, 77-, 61-, 27-, 55-, 87-, 103- and 115-case proofs remain separate required runs",
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
    result = {"schema": 1, "status": "bounded_global_checkpoint_passed" if passed else "bounded_global_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 8, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_and_exact_counts_are_explicit(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 105)
        self.assertEqual(len(cases), len({item["name"] for item in cases}))
        self.assertEqual(sum(item["cross_profile"] for item in cases), 63)
        self.assertEqual(sum(item["policy"] for item in cases), 4)
        self.assertEqual(sum(item["validation"] for item in cases), 42)
        self.assertEqual(sum(not item["error"] for item in cases), 52)
        self.assertEqual(sum(bool(item["error"]) and not item["validation"] and not item["policy"] for item in cases), 7)
        self.assertTrue(all(type(item["instructions"]) is int for item in cases))
        self.assertTrue(all(item["origin"] == "independently_authored_textual_nil" for item in cases))
        self.assertTrue(all(item["reference_result"] is not None for item in cases if item["policy"]))
        self.assertEqual(len(source_pins), 15)
        self.assertEqual(len(known_raw_matches), 13)

    def test_lexical_coverage_does_not_claim_types_or_initialization(self):
        for case in generated_cases():
            if not case["validation"]:
                coverage = instruction_inventory(case["edition"])
                self.assertEqual(coverage["profile"], 8)
                self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]), case["name"])
                self.assertEqual(coverage["unsupported_declarations"], [], case["name"])
                self.assertIn("lexical", coverage["scope"])
        for name in ("reject_store_text_value", "reject_integer_only_cast_array", "reject_dead_global_slots_over_capacity"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(all(item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))
        for name in ("reject_global_name_dotted", "reject_load_missing_type", "reject_global_type_text"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(any(not item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))

    def test_exact_semantics_and_policy_contrasts_cannot_be_confused(self):
        case = next(item for item in generated_cases() if item["name"] == "global_i64_exact_above_53_bits")
        candidate = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
                     "instructions": 4, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": candidate["result"], "error": None, "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        case = next(item for item in generated_cases() if item["name"] == "cast_arena_one_more_value")
        candidate = {"status": "execution_refused", "result": None, "error": "text_limit", "output_hex": "",
                     "instructions": 53257, "console_calls": 0, "output_bytes": 0}
        reference = dict(reference, result=42)
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
                error = "manifest_unsupported" if unsupported < 23 else "manifest_edition"
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

    def test_uninitialized_is_runtime_and_main_intent_remains_independent(self):
        case = next(item for item in generated_cases() if item["name"] == "uninitialized_preserves_output")
        candidate = {"status": "execution_refused", "result": None, "error": "global_uninitialized", "output_hex": "370a",
                     "instructions": 3, "console_calls": 1, "output_bytes": 2}
        reference = {"schema": 1, "status": "execution_refused", "result": None, "error": "Global not found: app::missing", "output_hex": "370a"}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_runtime_refusal")
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, dict(reference, output_hex=""), candidate)
        case = next(item for item in generated_cases() if item["name"] == "intentless_global")
        candidate = dict(candidate, status="returned", result=42, error=None, output_hex="", instructions=4, console_calls=0, output_bytes=0)
        reference = dict(reference, status="returned", result=42, error=None, output_hex="")
        production = dict(candidate, status="admission_refused", result=None, error="manifest_intent", instructions=0)
        self.assertEqual(compare_case(case, candidate, reference, production), "matching_semantics")
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, reference, candidate)

    def test_reference_bounds_mode_is_unset_and_old_sources_pinned(self):
        environment = baseline.clean_environment()
        self.assertNotIn("NIL_STRICT_BOUNDS", environment)
        self.assertEqual(environment["NIL_CRASH_REPORT"], "0")
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "record-candidate", "record-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "record_candidate", "record_conformance_candidate", "reference", "compiler", "root", "output"):
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
