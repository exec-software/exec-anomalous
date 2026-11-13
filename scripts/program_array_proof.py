#!/usr/bin/env python3
"""Qualify bounded profile-5 immutable arrays without changing earlier profiles.

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
import program_text_proof as computed_text
import program_conformance_proof as conformance


require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe = conformance.parse_probe
source_pins = dict(computed_text.source_pins, **{
    "program_text_proof.py": "a772a64af06b96039996d268782f2aa482b7eb58b3b3cd611f4fb3b576b8ab75",
    "program_text_reference.cpp": "11478227bab44eda44d05b4d672309a4e1074b4a8ef57921794fcda1752b1e94",
})
archive_pins = computed_text.archive_pins
known_raw_matches = dict(sorted({**computed_text.known_raw_matches,
    17: (7821, "2e4afc09a9d4af086664764b4d171d1544a81cb670d66a273472254dddc5a0d5"),
}.items()))
array_kinds = ("i64", "text")


def generated_cases():
    cases = []
    annotation = '@ai_context("Exercise bounded immutable typed arrays")'

    def add(name, body, *, helpers=(), strings=(), result=42, output=b"", calls=0,
            instructions=None, error=None, validation=False, policy=False,
            reference_result=None, reference_output=None, reference_error=None,
            ceiling=1, steps=65536, annotated=True, production_error=None,
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
                      "steps": steps, "frames": 16, "annotated": annotated,
                      "production_error": production_error or (error if validation and annotated else
                                                                 "manifest_intent" if not annotated else None),
                      "cross_profile": cross_profile,
                      "origin": "independently_authored_textual_nil"})

    integer_values = [-7, 9007199254740993, 42]
    for kind, values, strings, output in (
        ("i64", [f"PUSH_CONST {value}" for value in integer_values], (), b"-7\n9007199254740993\n42\n3\n"),
        ("text", [f"PUSH_CONST_STR S{index}" for index in range(3)], ("left", "é🙂", "right"),
         "left\né🙂\nright\n3\n".encode()),
    ):
        add(kind + "_literal_order", values + [f"NEW_ARRAY_LITERAL 3 {kind}", "STORE_LOCAL values"] +
            [instruction for index in range(3) for instruction in
             ("LOAD_LOCAL values", f"PUSH_CONST {index}", "GET_INDEX", "CONSOLE_WRITELN")] +
            ["LOAD_LOCAL values", "LEN", "CONSOLE_WRITELN", "PUSH_CONST 42", "RET"],
            strings=strings, output=output, calls=4)
        add(kind + "_empty_length", [f"NEW_ARRAY_LITERAL 0 {kind}", "LEN", "RET"], result=0, ceiling=0)
    for name, value in (("minimum", -(1 << 63)), ("maximum", (1 << 63) - 1),
                        ("exact_above_53_bits", 9007199254740993)):
        add("i64_" + name, [f"PUSH_CONST {value}", "NEW_ARRAY_LITERAL 1 i64", "PUSH_CONST 0", "GET_INDEX", "RET"],
            result=value, ceiling=0)
    add("count_leading_zeroes_empty", ["NEW_ARRAY_LITERAL 000 i64", "LEN", "RET"], result=0, ceiling=0)
    add("count_leading_zeroes_singleton", ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 0001 i64", "PUSH_CONST 0", "GET_INDEX", "RET"],
        ceiling=0)
    for name, index in (("negative", -1), ("minimum", -(1 << 63)), ("at_length", 1), ("maximum", (1 << 63) - 1)):
        add("i64_out_of_range_" + name, ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", f"PUSH_CONST {index}",
            "GET_INDEX", "RET"], result=0, ceiling=0)
        add("text_out_of_range_" + name, ["PUSH_CONST_STR S0", "NEW_ARRAY_LITERAL 1 text", f"PUSH_CONST {index}",
            "GET_INDEX", "CONSOLE_WRITE", "PUSH_CONST_STR S1", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
            strings=("unselected", "sentinel"), output=b"sentinel", calls=1)
    add("i64_empty_index", ["NEW_ARRAY_LITERAL 0 i64", "PUSH_CONST 0", "GET_INDEX", "RET"], result=0, ceiling=0)
    add("text_empty_index", ["NEW_ARRAY_LITERAL 0 text", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITELN",
        "PUSH_CONST 42", "RET"], output=b"\n", calls=1)
    add("immutable_aliases", ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", "STORE_LOCAL original",
        "LOAD_LOCAL original", "STORE_LOCAL alias", "LOAD_LOCAL original", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITELN",
        "LOAD_LOCAL alias", "PUSH_CONST 0", "GET_INDEX", "RET"], output=b"42\n", calls=1)
    add("computed_text_elements", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "PUSH_CONST_STR S2",
        "NEW_ARRAY_LITERAL 2 text", "STORE_LOCAL values", "LOAD_LOCAL values", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITE",
        "LOAD_LOCAL values", "PUSH_CONST 1", "GET_INDEX", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=("a", "b", "c"), output=b"abc", calls=2)
    add("decoded_text_survives_array", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT",
        "NEW_ARRAY_LITERAL 1 text", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=(r"a\n", r"\tb"), output=b"a\n\tb", calls=1)
    helper = flow.function("make_values", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT",
        "NEW_ARRAY_LITERAL 1 text", "RET"], result="array<text>")
    add("callee_array_and_text_survive_return", ["CALL make_values 0", "STORE_LOCAL values", "LOAD_LOCAL values",
        "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITE", "LOAD_LOCAL values", "LEN", "RET"],
        helpers=(helper,), strings=("hello ", "world"), result=1, output=b"hello world", calls=1, instructions=14)
    for kind, push, strings, result, ending, output, calls in (
        ("i64", "PUSH_CONST 42", (), 42, ["RET"], b"", 0),
        ("text", "PUSH_CONST_STR S0", ("passed",), 42, ["CONSOLE_WRITE", "PUSH_CONST 42", "RET"], b"passed", 1),
    ):
        helper = flow.function("identity", ["STORE_LOCAL values", "LOAD_LOCAL values", "RET"],
                               (("values", f"array<{kind}>"),), result=f"array<{kind}>")
        body = [push, f"NEW_ARRAY_LITERAL 1 {kind}", "CALL identity 1", "PUSH_CONST 0", "GET_INDEX", *ending]
        add(kind + "_parameter_and_return", body, helpers=(helper,), strings=strings,
            result=result, output=output, calls=calls, instructions=len(body) + 3)
    helper = flow.function("pick_second", ["STORE_LOCAL second", "STORE_LOCAL first", "LOAD_LOCAL second", "RET"],
        (("first", "array<i64>"), ("second", "array<i64>")), result="array<i64>")
    add("array_parameter_order", ["PUSH_CONST 7", "NEW_ARRAY_LITERAL 1 i64", "PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64",
        "CALL pick_second 2", "PUSH_CONST 0", "GET_INDEX", "RET"], helpers=(helper,), instructions=12, ceiling=0)
    helper = flow.function("churn", ["STORE_LOCAL values", "NEW_ARRAY_LITERAL 0 text", "POP", "LOAD_LOCAL values", "RET"],
        (("values", "array<i64>"),), result="array<i64>")
    add("alias_across_call_allocation", ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", "STORE_LOCAL held", "LOAD_LOCAL held",
        "CALL churn 1", "POP", "LOAD_LOCAL held", "PUSH_CONST 0", "GET_INDEX", "RET"], helpers=(helper,), instructions=15, ceiling=0)
    add("branch_array_join", ["PUSH_CONST 0", "JMP_IF_NOT right", "PUSH_CONST 7", "NEW_ARRAY_LITERAL 1 i64", "JMP done",
        "LABEL right", "PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", "LABEL done", "PUSH_CONST 0", "GET_INDEX", "RET"],
        instructions=9, ceiling=0)
    add("intentless_array", ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", "PUSH_CONST 0", "GET_INDEX", "RET"],
        annotated=False, ceiling=0)
    add("dead_array_keeps_syntax_only", ["PUSH_CONST 42", "RET", "NEW_ARRAY_LITERAL 1 text", "LEN", "GET_INDEX"],
        instructions=2, ceiling=0)

    empty = ["NEW_ARRAY_LITERAL 0 i64", "POP"]
    add("descriptors_exact_capacity", empty * 127 + ["NEW_ARRAY_LITERAL 0 i64", "LEN", "RET"], result=0, ceiling=0)
    add("descriptors_no_reclamation_after_pop", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + empty * 129 + ["PUSH_CONST 42", "RET"],
        error="array_limit", output=b"7\n", calls=1, instructions=259, policy=True, reference_result=42, reference_output=b"7\n")
    helper = flow.function("empty_values", ["NEW_ARRAY_LITERAL 0 i64", "RET"], result="array<i64>")
    add("descriptors_no_reclamation_after_return", ["CALL empty_values 0", "POP"] * 129 + ["PUSH_CONST 42", "RET"],
        helpers=(helper,), error="array_limit", instructions=514, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    block = [f"PUSH_CONST {value}" for value in range(256)] + ["NEW_ARRAY_LITERAL 256 i64", "POP"]
    add("literal_exact_capacity", block[:-1] + ["PUSH_CONST 255", "GET_INDEX", "RET"], result=255, ceiling=0)
    add("elements_exact_capacity", block * 4 + ["PUSH_CONST 42", "RET"], ceiling=0)
    add("elements_alias_after_capacity", block[:-1] + ["STORE_LOCAL held"] + block * 3 +
        ["LOAD_LOCAL held", "PUSH_CONST 255", "GET_INDEX", "RET"], result=255, ceiling=0)
    add("elements_no_reclamation_after_pop", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + block * 4 +
        ["PUSH_CONST 1", "NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 42", "RET"], error="array_limit",
        output=b"7\n", calls=1, instructions=1036, policy=True, reference_result=42, reference_output=b"7\n")
    helper = flow.function("full_values", block[:-1] + ["RET"], result="array<i64>")
    add("elements_no_reclamation_after_return", ["CALL full_values 0", "POP"] * 4 +
        ["PUSH_CONST 1", "NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 42", "RET"], helpers=(helper,),
        error="array_limit", instructions=1042, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    add("empty_array_after_element_capacity", block * 4 + ["NEW_ARRAY_LITERAL 0 text", "LEN", "RET"], result=0, ceiling=0)
    text_block = ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "POP"]
    add("computed_text_alias_after_text_capacity", text_block[:3] + ["NEW_ARRAY_LITERAL 1 text", "STORE_LOCAL held"] +
        text_block * 63 + ["LOAD_LOCAL held", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=("x" * 1024, ""), output=b"x" * 1024, calls=1)
    add("array_values_survive_text_limit", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + text_block[:3] +
        ["NEW_ARRAY_LITERAL 1 text", "STORE_LOCAL held"] + text_block * 64 + ["PUSH_CONST 42", "RET"],
        strings=("x" * 1024, ""), output=b"7\n", calls=1, error="text_limit", instructions=262,
        policy=True, reference_result=42, reference_output=b"7\n")
    add("array_text_before_fault", ["PUSH_CONST_STR S0", "NEW_ARRAY_LITERAL 1 text", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITELN",
        "PUSH_CONST 5", "PUSH_CONST 0", "DIV i64", "RET"], strings=("before",), output=b"before\n", calls=1,
        error="division_by_zero", instructions=8, reference_error="Division by zero")
    add("array_finite_fuel", ["PUSH_CONST 42", "NEW_ARRAY_LITERAL 1 i64", "PUSH_CONST 0", "GET_INDEX", "RET"],
        error="instruction_limit", instructions=2, steps=2, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    add("array_text_output_policy", ["PUSH_CONST_STR S0", "NEW_ARRAY_LITERAL 1 text", "PUSH_CONST 0", "GET_INDEX", "CONSOLE_WRITE",
        "PUSH_CONST 42", "RET"], strings=(r"\r",), error="output_policy", instructions=5,
        policy=True, reference_result=42, reference_output=b"\r")

    invalid = {
        "literal_underflow": ["NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 0", "RET"],
        "literal_i64_has_text": ["PUSH_CONST_STR S0", "NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 0", "RET"],
        "literal_text_has_integer": ["PUSH_CONST 1", "NEW_ARRAY_LITERAL 1 text", "POP", "PUSH_CONST 0", "RET"],
        "literal_mixed_elements": ["PUSH_CONST 1", "PUSH_CONST_STR S0", "NEW_ARRAY_LITERAL 2 i64", "POP", "PUSH_CONST 0", "RET"],
        "literal_array_element": ["NEW_ARRAY_LITERAL 0 i64", "NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 0", "RET"],
        "len_integer": ["PUSH_CONST 0", "LEN", "RET"],
        "len_text": ["PUSH_CONST_STR S0", "LEN", "RET"],
        "len_underflow": ["LEN", "RET"],
        "get_integer_base": ["PUSH_CONST 0", "PUSH_CONST 0", "GET_INDEX", "RET"],
        "get_text_base": ["PUSH_CONST_STR S0", "PUSH_CONST 0", "GET_INDEX", "RET"],
        "get_text_index": ["NEW_ARRAY_LITERAL 0 i64", "PUSH_CONST_STR S0", "GET_INDEX", "RET"],
        "get_array_index": ["NEW_ARRAY_LITERAL 0 i64", "NEW_ARRAY_LITERAL 0 i64", "GET_INDEX", "RET"],
        "get_underflow": ["NEW_ARRAY_LITERAL 0 i64", "GET_INDEX", "RET"],
        "array_as_integer_return": ["NEW_ARRAY_LITERAL 0 i64", "RET"],
        "array_integer_arithmetic": ["NEW_ARRAY_LITERAL 0 i64", "PUSH_CONST 1", "ADD", "RET"],
        "array_integer_branch": ["NEW_ARRAY_LITERAL 0 i64", "JMP_IF done", "LABEL done", "PUSH_CONST 0", "RET"],
        "array_scalar_join": ["PUSH_CONST 0", "JMP_IF_NOT other", "NEW_ARRAY_LITERAL 0 i64", "JMP done",
                              "LABEL other", "PUSH_CONST 0", "LABEL done", "POP", "PUSH_CONST 0", "RET"],
        "array_kind_join": ["PUSH_CONST 0", "JMP_IF_NOT other", "NEW_ARRAY_LITERAL 0 i64", "JMP done",
                            "LABEL other", "NEW_ARRAY_LITERAL 0 text", "LABEL done", "POP", "PUSH_CONST 0", "RET"],
    }
    for console in ("CONSOLE_WRITE", "CONSOLE_WRITELN", "CALL print 1", "CALL println 1"):
        invalid["array_console_" + console.lower().replace(" ", "_")] = ["NEW_ARRAY_LITERAL 0 i64", console, "PUSH_CONST 0", "RET"]
    for name, body in invalid.items():
        # `print` is not an admitted builtin. Name resolution refuses it before
        # CFG operand checks; the direct console forms and println check kinds.
        error = "manifest_unsupported" if name == "array_console_call_print_1" else "manifest_syntax"
        add("reject_" + name, body, strings=("text",), error=error, validation=True, cross_profile=False)
    for spelling in ("-1", "+1", "0x1"):
        add("reject_count_" + {"-1": "negative", "+1": "plus", "0x1": "hex"}[spelling],
            [f"NEW_ARRAY_LITERAL {spelling} i64", "POP", "PUSH_CONST 0", "RET"],
            error="manifest_syntax", validation=True, cross_profile=False)
    add("reject_literal_over_capacity", ["NEW_ARRAY_LITERAL 257 i64", "POP", "PUSH_CONST 0", "RET"],
        error="manifest_limit", validation=True, cross_profile=False)
    for name, opcode in (
        ("literal_missing_type", "NEW_ARRAY_LITERAL 0"), ("literal_extra_operand", "NEW_ARRAY_LITERAL 0 i64 extra"),
        ("literal_i32", "NEW_ARRAY_LITERAL 0 i32"), ("literal_bool", "NEW_ARRAY_LITERAL 0 bool"),
        ("literal_nested", "NEW_ARRAY_LITERAL 0 array<i64>"), ("literal_pointer", "NEW_ARRAY_LITERAL 0 ptr"),
        ("len_operand", "LEN i64"), ("get_operand", "GET_INDEX i64"),
        ("array_mutation", "SET_INDEX"), ("array_free", "FREE"),
    ):
        add("reject_" + name, ["PUSH_CONST 42", "RET", opcode], error="manifest_unsupported", validation=True, cross_profile=False)
    helper = flow.function("take_texts", ["STORE_LOCAL values", "PUSH_CONST 42", "RET"], (("values", "array<text>"),))
    add("reject_parameter_element_kind", ["NEW_ARRAY_LITERAL 0 i64", "CALL take_texts 1", "RET"], helpers=(helper,),
        error="manifest_syntax", validation=True, cross_profile=False)
    helper = flow.function("wrong_return", ["NEW_ARRAY_LITERAL 0 i64", "RET"], result="array<text>")
    add("reject_return_element_kind", ["CALL wrong_return 0", "POP", "PUSH_CONST 42", "RET"], helpers=(helper,),
        error="manifest_syntax", validation=True, cross_profile=False)
    for spelling in ("array<i32>", "array<array<i64>>", "array<ptr>"):
        helper = flow.function("unsupported", ["RET"], result=spelling)
        add("reject_signature_" + spelling.replace("<", "_").replace(">", ""), ["PUSH_CONST 42", "RET"], helpers=(helper,),
            error="manifest_unsupported", validation=True, cross_profile=False)
    add("reject_dead_foreign_effect", ["NEW_ARRAY_LITERAL 0 i64", "POP", "PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"],
        error="manifest_unsupported", validation=True, cross_profile=False)
    add("reject_dead_console_ceiling", ["NEW_ARRAY_LITERAL 0 i64", "POP", "PUSH_CONST 42", "RET", "PUSH_CONST_STR S0", "CONSOLE_WRITE"],
        strings=("unused",), error="manifest_ceiling", validation=True, ceiling=0, cross_profile=False)
    require(len({case["name"] for case in cases}) == len(cases), "duplicate array boundary case")
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
                process, stdout, _ = baseline.run([tool, path, str(case["ceiling"]), str(case["steps"]), "16"],
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
                for name, tool, parser, status in (("text", args.text_candidate, flow.parse_probe, "admission_refused"),
                                                   ("text_raw", args.text_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])],
                        output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_intent" if name == "text" and not case["annotated"] else "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier text grammar expanded")
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
            "text_refusals": sum("text" in item for item in records),
            "text_conformance_refusals": sum("text_raw" in item for item in records)}


def instruction_inventory(data):
    """Lexical metadata only; complete admission still checks kinds and CFG."""
    result = computed_text.instruction_inventory(data)
    result.update(profile=5, lane="raw_array_conformance",
                  scope="profile-5 lexical coverage only; array kinds, references, authority and CFG require complete validation")
    if result["unsupported_format"]:
        return result
    kind = r"(?:i32|i64|text|array<i64>|array<text>)"
    header_pattern = re.compile(r"function ([A-Za-z_][A-Za-z_0-9]*)(?:\(([^()]*)\))? -> (" + kind + r"|void):")
    headers = {}
    for line in data.decode("utf-8").splitlines():
        match = header_pattern.fullmatch(line.strip())
        if match:
            parameters = match[2].split(", ") if match[2] else []
            if all(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*: " + kind, parameter) for parameter in parameters):
                headers[match[1]] = len(parameters)
    result["unsupported_declarations"] = [record for record in result["unsupported_declarations"]
        if not (record["category"] == "unsupported_function_shape" and
                header_pattern.fullmatch(record["text"].strip()) and
                header_pattern.fullmatch(record["text"].strip())[1] in headers)]
    for item in result["instructions"]:
        opcode, operands = item["opcode"], item["operands"]
        if opcode == "NEW_ARRAY_LITERAL":
            item["supported_opcode"] = True
            item["supported_form"] = (len(operands) == 2 and operands[1] in array_kinds and
                re.fullmatch(r"[0-9]+", operands[0], re.ASCII) is not None and int(operands[0]) <= 256)
        elif opcode in {"LEN", "GET_INDEX"}:
            item.update(supported_opcode=True, supported_form=not operands)
        elif opcode == "CALL" and len(operands) == 2 and operands[0] in headers:
            item["supported_form"] = operands[1] == str(headers[operands[0]])
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
    expected_errors = {"manifest_unsupported": 28, "manifest_edition": 9}
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
        record["lane"] = "raw_array_conformance"
    raw.update(lane="raw_array_conformance",
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
        item["lane"] = "production_profile5_admission"
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
            if number == 17:
                pair = dict(item, lane="prior_text_grammar")
                for name, tool, parser, status in (("text", args.text_candidate, flow.parse_probe, "admission_refused"),
                                                   ("text_raw", args.text_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] ==
                            ("manifest_intent" if name == "text" else "manifest_unsupported"), "earlier corpus text contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    # Measured on all 45 identical editions. In paragraphs.matrix (invocation
    # 31), array<i64> now parses in a helper before the unannotated main. This
    # moves its first production error from unsupported to intent; raw still
    # refuses the later function pointers and indirect calls.
    expected = {"manifest_unsupported": 24, "manifest_edition": 9, "manifest_intent": 12}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 1
    production = {"lane": "production_profile5_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "text_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "text-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "text_candidate", "text_conformance_candidate", "reference", "compiler", "interpreter")
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
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_array_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_array host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 5 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536, "array_descriptors": 128, "array_elements": 1024, "array_literal_elements": 256},
        "earlier_proofs": "126-, 77-, 61-, 27- and 55-case proofs remain separate required runs",
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
    result = {"schema": 1, "status": "bounded_array_checkpoint_passed" if passed else "bounded_array_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 5, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_and_exact_counts_are_explicit(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 87)
        self.assertEqual(len(cases), len({item["name"] for item in cases}))
        self.assertEqual(sum(item["cross_profile"] for item in cases), 44)
        self.assertEqual(sum(item["policy"] for item in cases), 7)
        self.assertTrue(all(type(item["instructions"]) is int for item in cases))
        self.assertTrue(all(item["origin"] == "independently_authored_textual_nil" for item in cases))
        self.assertTrue(all(item["reference_result"] is not None for item in cases if item["policy"]))
        self.assertIn("callee_array_and_text_survive_return", {item["name"] for item in cases})
        self.assertIn("elements_no_reclamation_after_return", {item["name"] for item in cases})
        self.assertEqual(len(source_pins), 9)

    def test_lexical_coverage_does_not_claim_operand_kinds(self):
        for name in ("i64_literal_order", "text_literal_order", "callee_array_and_text_survive_return", "i64_parameter_and_return",
                     "count_leading_zeroes_empty", "count_leading_zeroes_singleton"):
            case = next(item for item in generated_cases() if item["name"] == name)
            coverage = instruction_inventory(case["edition"])
            self.assertEqual(coverage["profile"], 5)
            self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]))
            self.assertEqual(coverage["unsupported_declarations"], [])
            self.assertIn("lexical", coverage["scope"])
        case = next(item for item in generated_cases() if item["name"] == "reject_len_integer")
        self.assertTrue(all(item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))
        for name in ("reject_literal_over_capacity", "reject_literal_nested", "reject_count_hex", "reject_len_operand"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(any(not item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))

    def test_exact_semantics_and_policy_contrasts_cannot_be_confused(self):
        case = next(item for item in generated_cases() if item["name"] == "i64_exact_above_53_bits")
        candidate = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
                     "instructions": 5, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": candidate["result"], "error": None, "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        case = next(item for item in generated_cases() if item["name"] == "descriptors_no_reclamation_after_pop")
        candidate = {"status": "execution_refused", "result": None, "error": "array_limit", "output_hex": "370a",
                     "instructions": 259, "console_calls": 1, "output_bytes": 2}
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
                error = "manifest_unsupported" if unsupported < 28 else "manifest_edition"
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

    def test_reference_bounds_mode_is_unset_and_old_sources_pinned(self):
        environment = baseline.clean_environment()
        self.assertNotIn("NIL_STRICT_BOUNDS", environment)
        self.assertEqual(environment["NIL_CRASH_REPORT"], "0")
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "text-candidate", "text-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "text_candidate", "text_conformance_candidate", "reference", "compiler", "root", "output"):
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
