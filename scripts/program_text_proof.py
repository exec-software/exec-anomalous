#!/usr/bin/env python3
"""Qualify bounded profile-4 computed text without changing earlier profiles.

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
import program_scalar_flow_proof as scalar
import program_conformance_proof as conformance


require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe = conformance.parse_probe
source_pins = dict(conformance.source_pins, **{
    "program_conformance_proof.py": "1648fd36cb3e8290bb60deda61e38816587b20ffdf173619d4c6a580422a4675",
})
archive_pins = conformance.archive_pins
known_raw_matches = dict(conformance.known_raw_matches)
known_raw_matches.update({
    36: (10881, "f4db3e3207fb88f0f540f2b0a101dd890b3e5b3fb031ddb3c82a3ae757d50ef1"),
    37: (10881, "fa66b63f7b112836e193d2bf45520aa1a62ec242b529ba26d4c9e265f11a43f2"),
    38: (612, "9c715aa5402870b6ff0151701f82d98921d101ec7b4d4353d482d7d0e636b3ed"),
})
hex_pattern = re.compile(r"[+-]?0[xX][0-9a-fA-F]+\Z", re.ASCII)


def generated_cases():
    cases = []
    annotation = '@ai_context("Exercise bounded computed text and integer literals")'

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
                      "instructions": 0 if validation else instructions,
                      "validation": validation, "policy": policy,
                      "reference_result": reference_result, "reference_output": reference_output,
                      "reference_error": reference_error, "ceiling": ceiling,
                      "steps": steps, "frames": 16, "annotated": annotated,
                      "production_error": production_error or (error if validation and annotated else
                                                                 "manifest_intent" if not annotated else None),
                      "cross_profile": cross_profile,
                      "origin": "independently_authored_textual_nil"})

    def concat(name, left, right, expected, **kwargs):
        add(name, ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "CONSOLE_WRITE",
                   "PUSH_CONST 42", "RET"], strings=(left, right), output=expected,
            calls=int(bool(expected)), instructions=6, **kwargs)

    concat("literal_order", "left", "right", b"leftright")
    concat("empty_left", "", "right", b"right")
    concat("empty_right", "left", "", b"left")
    concat("empty_both", "", "", b"")
    concat("unicode_bytes", "é🙂", "—✓", "é🙂—✓".encode())
    concat("decode_once_boundary", r"\\", "n", b"\\n")
    concat("escaped_quote_boundary", r"\"", r"\\", b'"\\')
    concat("unknown_escape_preserved", r"\b", "n", b"\\bn")
    concat("decoded_newline_tab", r"a\n", r"\tb", b"a\n\tb")
    add("computed_then_literal", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT",
        "PUSH_CONST_STR S2", "STR_CONCAT", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=("a", "b", "c"), output=b"abc", calls=1, instructions=8)
    add("literal_then_computed", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "PUSH_CONST_STR S2",
        "STR_CONCAT", "STR_CONCAT", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=("a", "b", "c"), output=b"abc", calls=1, instructions=8)
    add("computed_then_computed", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT",
        "PUSH_CONST_STR S2", "PUSH_CONST_STR S3", "STR_CONCAT", "STR_CONCAT", "CONSOLE_WRITE",
        "PUSH_CONST 42", "RET"], strings=("a", "b", "c", "d"), output=b"abcd", calls=1, instructions=10)
    add("immutable_aliases", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "STORE_LOCAL original",
        "LOAD_LOCAL original", "PUSH_CONST_STR S2", "STR_CONCAT", "STORE_LOCAL extended",
        "LOAD_LOCAL original", "CONSOLE_WRITE", "LOAD_LOCAL extended", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=("a", "b", "c"), output=b"ababc", calls=2, instructions=14)
    helper = flow.function("greet", ["STORE_LOCAL name", "PUSH_CONST_STR S0", "LOAD_LOCAL name", "STR_CONCAT", "RET"],
                           (("name", "text"),), result="text")
    add("callee_result_survives_return", ["PUSH_CONST_STR S1", "CALL greet 1", "STORE_LOCAL result",
        "LOAD_LOCAL result", "CONSOLE_WRITELN", "LOAD_LOCAL result", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        helpers=(helper,), strings=("hello ", "world"), output=b"hello world\nhello world", calls=2, instructions=14)
    helper = flow.function("combine", ["STORE_LOCAL right", "STORE_LOCAL left", "LOAD_LOCAL left", "LOAD_LOCAL right",
                                        "STR_CONCAT", "RET"], (("left", "text"), ("right", "text")), result="text")
    add("text_parameter_order", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "PUSH_CONST_STR S2",
        "CALL combine 2", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], helpers=(helper,), strings=("left", ":", "right"),
        output=b"left:right", calls=1, instructions=14)
    add("branch_text_join", ["PUSH_CONST 0", "JMP_IF_NOT right", "PUSH_CONST_STR S0", "PUSH_CONST_STR S1",
        "STR_CONCAT", "JMP done", "LABEL right", "PUSH_CONST_STR S1", "PUSH_CONST_STR S0", "STR_CONCAT",
        "LABEL done", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], strings=("a", "b"), output=b"ba", calls=1, instructions=10)
    add("intentless_text", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        strings=("raw", " text"), output=b"raw text", calls=1, instructions=6, annotated=False)

    for name, literal, value in (
        ("zero", "0x0", 0), ("negative_zero", "-0X000", 0), ("plus_upper", "+0XfF", 255),
        ("leading_zeroes", "0x0000000000000000002a", 42), ("mixed_digits", "0xAbCdEf", 11259375),
        ("negative", "-0x80", -128), ("maximum", "0x7fffffffffffffff", (1 << 63) - 1),
        ("minimum", "-0X8000000000000000", -(1 << 63)), ("exact_above_53_bits", "0x20000000000001", 9007199254740993),
    ):
        add("hex_" + name, [f"PUSH_CONST {literal}", "RET"], result=value, instructions=2, ceiling=0)
    add("hex_explicit_i32_cast", ["PUSH_CONST 0xffffffff", "CAST i32", "RET"], result=-1, instructions=3, ceiling=0)
    add("hex_scalar_add", ["PUSH_CONST 0x7fffffffffffffff", "PUSH_CONST 1", "ADD", "RET"],
        result=-(1 << 63), instructions=4, ceiling=0)
    add("intentless_hex", ["PUSH_CONST 0x2a", "RET"], instructions=2, ceiling=0, annotated=False)

    block = ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "POP"]
    add("arena_exact_capacity", block * 64 + ["PUSH_CONST 42", "RET"], strings=("x" * 1024, ""), instructions=258, ceiling=0)
    add("arena_alias_after_capacity", block[:3] + ["STORE_LOCAL held"] + block * 63 +
        ["LOAD_LOCAL held", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], strings=("x" * 1024, ""),
        output=b"x" * 1024, calls=1, instructions=260)
    add("arena_no_reclamation_after_pop", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + block * 65 + ["PUSH_CONST 42", "RET"],
        strings=("x" * 1024, ""), error="text_limit", instructions=261, output=b"7\n", calls=1,
        policy=True, reference_result=42, reference_output=b"7\n")
    helper = flow.function("discarded", block[:3] + ["RET"], result="text")
    add("arena_no_reclamation_after_return", ["CALL discarded 0", "POP"] * 65 + ["PUSH_CONST 42", "RET"],
        helpers=(helper,), strings=("x" * 1024, ""), error="text_limit", instructions=388,
        policy=True, reference_result=42, reference_output=b"")
    add("empty_concat_after_capacity", block * 64 + ["PUSH_CONST_STR S1", "PUSH_CONST_STR S1", "STR_CONCAT",
        "CONSOLE_WRITE", "PUSH_CONST 42", "RET"], strings=("x" * 1024, ""), instructions=262)
    add("payload_exact_capacity", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "CONSOLE_WRITE",
        "PUSH_CONST 42", "RET"], strings=("x" * 2048, "y" * 2048), output=b"x" * 2048 + b"y" * 2048, calls=1, instructions=6)
    add("payload_over_capacity", ["PUSH_CONST 7", "CONSOLE_WRITELN", "PUSH_CONST_STR S0", "PUSH_CONST_STR S1",
        "STR_CONCAT", "CONSOLE_WRITELN", "PUSH_CONST 42", "RET"], strings=("x" * 2048, "y" * 2048),
        output=b"7\n", calls=1, error="output_limit", instructions=6, policy=True, reference_result=42,
        reference_output=b"7\n" + b"x" * 2048 + b"y" * 2048 + b"\n")
    payload = b"x" * 2048 + b"y" * 2048
    add("total_output_limit_reuses_text", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "STORE_LOCAL held"] +
        ["LOAD_LOCAL held", "CONSOLE_WRITE"] * 17 + ["PUSH_CONST 42", "RET"], strings=("x" * 2048, "y" * 2048),
        output=payload * 16, calls=16, error="output_limit", instructions=38, policy=True, reference_result=42,
        reference_output=payload * 17)
    add("computed_control_output_policy", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "CONSOLE_WRITE",
        "PUSH_CONST 42", "RET"], strings=(r"\r", "end"), error="output_policy", instructions=4,
        policy=True, reference_result=42, reference_output=b"\rend")
    add("computed_text_before_fault", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "CONSOLE_WRITELN",
        "PUSH_CONST 5", "PUSH_CONST 0", "DIV i64", "RET"], strings=("o", "k"), output=b"ok\n", calls=1,
        error="division_by_zero", instructions=7, reference_error="Division by zero")
    add("text_finite_fuel", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S1", "STR_CONCAT", "POP", "PUSH_CONST 42", "RET"],
        strings=("a", "b"), error="instruction_limit", instructions=3, steps=3, policy=True, reference_result=42,
        reference_output=b"")

    for name, body in (
        ("integer_left", ["PUSH_CONST 1", "PUSH_CONST_STR S0", "STR_CONCAT", "POP", "PUSH_CONST 0", "RET"]),
        ("integer_right", ["PUSH_CONST_STR S0", "PUSH_CONST 1", "STR_CONCAT", "POP", "PUSH_CONST 0", "RET"]),
        ("two_integers", ["PUSH_CONST 1", "PUSH_CONST 2", "STR_CONCAT", "RET"]),
        ("stack_underflow", ["PUSH_CONST_STR S0", "STR_CONCAT", "POP", "PUSH_CONST 0", "RET"]),
    ):
        add("reject_concat_" + name, body, strings=("text",), error="manifest_syntax", validation=True, cross_profile=False)
    add("reject_concat_operand", ["PUSH_CONST_STR S0", "PUSH_CONST_STR S0", "STR_CONCAT text", "POP", "PUSH_CONST 0", "RET"],
        strings=("text",), error="manifest_unsupported", validation=True, cross_profile=False)
    add("reject_dead_foreign_effect", ["PUSH_CONST 0x2a", "RET", "INTEROP_CALL libc puts 1 i64"],
        error="manifest_unsupported", validation=True, cross_profile=False)
    add("reject_dead_console_ceiling", ["PUSH_CONST 0x2a", "RET", "PUSH_CONST_STR S0", "CONSOLE_WRITE"],
        strings=("unused",), error="manifest_ceiling", validation=True, ceiling=0, cross_profile=False)
    add("dead_concat_keeps_syntax_only", ["PUSH_CONST 0x2a", "RET", "STR_CONCAT"], instructions=2, ceiling=0)
    for name, literal, error in (
        ("positive_overflow", "0x8000000000000000", "manifest_syntax"),
        ("negative_overflow", "-0x8000000000000001", "manifest_syntax"),
        ("unsigned_max", "0xffffffffffffffff", "manifest_syntax"),
        ("missing_digits", "0x", "manifest_unsupported"),
        ("bad_digit", "0x1g", "manifest_unsupported"),
        ("separator", "0x1_2", "manifest_unsupported"),
        ("decimal_plus_unchanged", "+42", "manifest_unsupported"),
    ):
        add("reject_hex_" + name, [f"PUSH_CONST {literal}", "RET"], error=error, validation=True, cross_profile=False)
    require(len({case["name"] for case in cases}) == len(cases), "duplicate text boundary case")
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
                for name, tool, parser, status in (("scalar", args.scalar_candidate, flow.parse_probe, "admission_refused"),
                                                   ("scalar_raw", args.scalar_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])],
                        output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_intent" if name == "scalar" and not case["annotated"] else "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier scalar grammar expanded")
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
            "scalar_refusals": sum("scalar" in item for item in records),
            "scalar_conformance_refusals": sum("scalar_raw" in item for item in records)}


def instruction_inventory(data):
    result = scalar.instruction_inventory(data)
    result.update(profile=4, lane="raw_text_conformance",
                  scope="profile-4 lexical coverage only; references, text kinds, authority and CFG require complete validation")
    for item in result["instructions"]:
        if item["opcode"] == "STR_CONCAT" and not item["operands"]:
            item.update(supported_opcode=True, supported_form=True)
        if item["opcode"] == "PUSH_CONST" and len(item["operands"]) == 1:
            operand = item["operands"][0]
            if hex_pattern.fullmatch(operand) and -(1 << 63) <= int(operand, 16) < (1 << 63):
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
    expected_errors = {"manifest_unsupported": 29, "manifest_edition": 9}
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
        record["lane"] = "raw_text_conformance"
    raw.update(lane="raw_text_conformance",
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
        item["lane"] = "production_profile4_admission"
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
            if number in (36, 37, 38):
                pair = dict(item, lane="prior_scalar_grammar")
                for name, tool, parser, status in (("scalar", args.scalar_candidate, flow.parse_probe, "admission_refused"),
                                                   ("scalar_raw", args.scalar_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] == "manifest_unsupported", "earlier corpus scalar contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    expected = {"manifest_unsupported": 25, "manifest_edition": 9, "manifest_intent": 11}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 3
    production = {"lane": "production_profile4_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "scalar_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "scalar-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "scalar_candidate", "scalar_conformance_candidate", "reference", "compiler", "interpreter")
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
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_text_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_text host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 4 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536},
        "earlier_proofs": "126-, 77-, 61- and 27-case proofs remain separate required runs",
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
    result = {"schema": 1, "status": "bounded_text_checkpoint_passed" if passed else "bounded_text_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 4, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_and_exact_counts_are_explicit(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 55)
        self.assertEqual(len(cases), len({item["name"] for item in cases}))
        self.assertEqual(sum(item["cross_profile"] for item in cases), 41)
        self.assertEqual(sum(item["policy"] for item in cases), 6)
        self.assertTrue(all(type(item["instructions"]) is int for item in cases))
        self.assertTrue(all(item["origin"] == "independently_authored_textual_nil" for item in cases))
        self.assertTrue(all(item["reference_result"] is not None for item in cases if item["policy"]))
        self.assertIn("callee_result_survives_return", {item["name"] for item in cases})
        self.assertIn("arena_no_reclamation_after_return", {item["name"] for item in cases})

    def test_lexical_coverage_does_not_claim_operand_kinds(self):
        for name in ("literal_order", "hex_minimum", "hex_plus_upper", "computed_then_computed"):
            case = next(item for item in generated_cases() if item["name"] == name)
            coverage = instruction_inventory(case["edition"])
            self.assertEqual(coverage["profile"], 4)
            self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]))
            self.assertIn("lexical", coverage["scope"])
        self.assertIsNone(hex_pattern.fullmatch("0x1_2"))
        self.assertIsNone(hex_pattern.fullmatch("+42"))

    def test_exact_semantics_and_policy_contrasts_cannot_be_confused(self):
        case = next(item for item in generated_cases() if item["name"] == "hex_exact_above_53_bits")
        candidate = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
                     "instructions": 2, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": candidate["result"], "error": None, "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        case = next(item for item in generated_cases() if item["name"] == "arena_no_reclamation_after_pop")
        candidate = {"status": "execution_refused", "result": None, "error": "text_limit", "output_hex": "370a",
                     "instructions": 261, "console_calls": 1, "output_bytes": 2}
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
                error = "manifest_unsupported" if unsupported < 29 else "manifest_edition"
                unsupported += 1
                records.append({"invocation": number, "matched": False, "outcome": "validation_refused", "candidate": {"error": error}})
        self.assertTrue(raw_regression_contract(records)["passed"])
        changed = [dict(item, matched=False, outcome="validation_refused", candidate={"error": "manifest_unsupported"})
                   if item["invocation"] == 38 else item for item in records]
        self.assertFalse(raw_regression_contract(changed)["passed"])
        changed = [dict(item, edition_sha256="0" * 64) if item["invocation"] == 36 else item for item in records]
        self.assertFalse(raw_regression_contract(changed)["passed"])
        self.assertFalse(raw_regression_contract(records[:-1])["passed"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "scalar-candidate", "scalar-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "scalar_candidate", "scalar_conformance_candidate", "reference", "compiler", "root", "output"):
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
