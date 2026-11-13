#!/usr/bin/env python3
"""Keep raw scalar-language conformance separate from production admission.

The upstream runner, inputs, expectations and complete compiler editions remain
unchanged. Every retained interpreter edition is attempted in both lanes. Raw
validation is a host-only test boundary, never a production manifest or native
execution claim. Earlier proofs remain separate, unchanged obligations.
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


require = baseline.require
digest = baseline.digest
write_json = baseline.write_json
source_pins = {
    "program_execution_proof.py": "c86a0a9357f2a0e361baea1e4fbcabe4e8a1018111e6aa5cf57e98eb92bc75b3",
    "program_execution_reference.cpp": "259aef1f61c850cd887a913e75024e936c81f8bb34dff8a62473022e9658fa0e",
    "program_flow_proof.py": "d9bd4098e0c8ca5ca6f372752c8d6e6698c1a75467d21c91e34ce6f636b8400f",
    "program_flow_reference.cpp": "73431f73d39bf72831fb3f727ed94035ae654c8a0bf8e10dab554984db9031e9",
    "program_scalar_flow_proof.py": "9dee89caa23d2e0f36fae4ab66f61a96a2ea5b5329e8d33f9459707774c0935d",
    "program_scalar_flow_reference.cpp": "28261d497a1490828cfb5aff94f6bd23538800ccd987d4db00f43230b1c556ff",
}
archive_pins = {
    "libnilcompiler_core.a": "edf0059ec7a93bb6b6292021048891edfedc741448717eede678b4a8bb087c51",
    "libnil.a": "f36a65ff09ebaa43586782a95d567c1ad28352011d0fe736d6915fa1bf2683e3",
}
# The first successful raw run established these complete, unmodified editions.
# A later all-refused candidate must not pass this checkpoint's coverage gate.
known_raw_matches = {
    0: (306, "cb33e5df82833829e1ee282b6c14070d559b1f504d912577718ad3ac7c194eea"),
    1: (306, "cb33e5df82833829e1ee282b6c14070d559b1f504d912577718ad3ac7c194eea"),
    2: (306, "cb33e5df82833829e1ee282b6c14070d559b1f504d912577718ad3ac7c194eea"),
    28: (298, "5dc599af428b948bde6351cec6f1de284b06017109e8659c1dd63aeddd33dd03"),
}


def parse_probe(process, output, *, reference=False):
    if reference:
        return flow.parse_probe(process, output, reference=True)
    require(process["exit_code"] == 0 and not process["timed_out"], "raw probe infrastructure failure")
    value = baseline.strict_json(output.decode("utf-8"))
    fields = {"status", "result", "error", "output_hex", "instructions", "console_calls", "output_bytes"}
    require(type(value) is dict and set(value) == fields, "raw probe fields changed")
    require(type(value["status"]) is str and value["status"] in
            {"returned", "execution_refused", "validation_refused"}, "invalid raw probe status")
    require(type(value["output_hex"]) is str and
            re.fullmatch(r"(?:[0-9a-f]{2})*", value["output_hex"]) is not None, "noncanonical output")
    if value["status"] == "returned":
        require(type(value["result"]) is int and -(1 << 63) <= value["result"] < (1 << 63), "full i64 result missing")
        require(value["error"] is None, "successful raw probe has error")
    else:
        require(value["result"] is None and type(value["error"]) is str and value["error"], "refusal lacks error")
    for field in ("instructions", "console_calls", "output_bytes"):
        require(type(value[field]) is int and value[field] >= 0, f"invalid {field}")
    require(value["instructions"] <= 65536 and value["output_bytes"] <= 65536, "raw budget exceeded")
    require(value["console_calls"] <= value["instructions"], "more console calls than dispatched instructions")
    require(value["output_bytes"] == len(value["output_hex"]) // 2, "output accounting mismatch")
    if value["status"] == "validation_refused":
        require(all(value[field] == 0 for field in ("instructions", "console_calls", "output_bytes"))
                and value["output_hex"] == "", "validation refusal produced effects")
    return value


def authored_edition(functions, strings=(), annotations=None):
    """Author independent harness fixtures; never transform upstream editions."""
    text = "# NIL Textual Module\nmodule raw_conformance\n\n"
    if strings:
        text += "# String constants\n"
        text += "".join(f'const S{index} = "{value}"\n' for index, value in enumerate(strings)) + "\n"
    for number, (name, parameters, result, body) in enumerate(functions, 1):
        for annotation in (annotations or {}).get(name, ()):
            text += annotation + "\n"
        text += f"# declaration_line {number}\nfunction {name}"
        if parameters:
            text += "(" + ", ".join(f"{name}: {kind}" for name, kind in parameters) + ")"
        text += f" -> {result}:\n" + "".join(f"    {instruction}\n" for instruction in body) + "end function\n\n"
    return text.encode()


def generated_cases():
    cases = []
    main = lambda body: flow.function("main", body)
    annotation = '@ai_context("Authored boundary fixture")'

    def add(name, functions, *, result=42, strings=(), annotations=None, output=b"", calls=0, instructions=2,
            error=None, validation=False, ceiling=1, steps=65536, policy=False,
            production_error="manifest_intent", production_returns=False, reference_result=None, reference_error=None):
        cases.append({"name": name, "edition": authored_edition(functions, strings, annotations),
                      "result": None if error else result, "error": error, "output": output,
                      "console_calls": calls, "instructions": 0 if validation else instructions,
                      "validation": validation, "ceiling": ceiling, "steps": steps, "frames": 16,
                      "policy": policy, "reference_result": reference_result, "reference_error": reference_error,
                      "production_error": None if production_returns else production_error,
                      "production_returns": production_returns, "origin": "independently_authored_textual_nil"})

    answer = [main(["PUSH_CONST 42", "RET"])]
    add("intentless_integer", answer, ceiling=0)
    add("intentless_exact_i64", [main(["PUSH_CONST 9007199254740993", "RET"])], result=9007199254740993, ceiling=0)
    add("intentless_console", [main(["PUSH_CONST_STR S0", "CALL println 1", "PUSH_CONST 42", "RET"])],
        strings=("raw — ✓",), output="raw — ✓\n".encode(), calls=1, instructions=4)
    add("intentless_scalar_temporary", [main(["PUSH_CONST 2147483647", "STORE_LOCAL $ny.temp.idx.0",
        "LOAD_LOCAL $ny.temp.idx.0", "PUSH_CONST 1", "ADD", "RET"])], result=2147483648, instructions=6, ceiling=0)
    add("intentless_direct_call", [main(["CALL answer 0", "RET"]), flow.function("answer", ["PUSH_CONST 42", "RET"])],
        instructions=4, ceiling=0)
    add("intentless_dead_console_granted", [main(["PUSH_CONST 42", "RET", "PUSH_CONST_STR S0", "CONSOLE_WRITE"])],
        strings=("dead",))
    add("intentless_dead_console_denied", [main(["PUSH_CONST 42", "RET", "PUSH_CONST_STR S0", "CONSOLE_WRITE"])],
        strings=("dead",), ceiling=0, error="manifest_ceiling", validation=True)
    add("intentless_unused_console_denied", [*answer, flow.function("unused", ["PUSH_CONST_STR S0", "CONSOLE_WRITE", "PUSH_CONST 0", "RET"])],
        strings=("unused",), ceiling=0, error="manifest_ceiling", validation=True)
    add("intentless_dead_foreign_effect", [main(["PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"])],
        error="manifest_unsupported", validation=True)
    add("intentless_unknown_ceiling", answer, ceiling=2, error="manifest_ceiling", validation=True)
    add("intentless_uninitialized_local", [main(["LOAD_LOCAL missing", "RET"])], error="manifest_syntax", validation=True)
    add("intentless_unresolved_dead_call", [main(["PUSH_CONST 42", "RET", "CALL absent 0"])],
        error="manifest_unsupported", validation=True)
    add("intentless_finite_fuel_refusal", answer, steps=1, error="instruction_limit", instructions=1,
        policy=True, reference_result=42)
    add("intentless_division_by_zero", [main(["PUSH_CONST 7", "CALL println 1", "PUSH_CONST 5", "PUSH_CONST 0", "DIV i64", "RET"])],
        output=b"7\n", calls=1, instructions=5, error="division_by_zero", reference_error="Division by zero")
    add("annotated_integer", answer, annotations={"main": (annotation,)}, production_returns=True)
    add("annotated_console", [main(["PUSH_CONST_STR S0", "CALL println 1", "PUSH_CONST 42", "RET"])],
        annotations={"main": (annotation,)}, strings=("declared",), output=b"declared\n", calls=1,
        instructions=4, production_returns=True)
    add("annotated_dead_console_denied", [main(["PUSH_CONST 42", "RET", "PUSH_CONST_STR S0", "CONSOLE_WRITE"])],
        annotations={"main": (annotation,)}, strings=("dead",), ceiling=0, error="manifest_ceiling", validation=True,
        production_error="manifest_ceiling")
    for name, annotations, expected in (
        ("empty_annotation", {'main': ('@ai_context("")',)}, "manifest_intent"),
        ("duplicate_annotation", {'main': (annotation, annotation)}, "manifest_intent"),
        ("unknown_annotation", {'main': ('@other("value")',)}, "manifest_unsupported"),
        ("malformed_annotation", {'main': ('@ai_context("unterminated)',)}, "manifest_unsupported"),
        ("escaped_annotation", {'main': ('@ai_context("back\\slash")',)}, "manifest_intent"),
        ("oversized_annotation", {'main': ('@ai_context("' + "a" * 513 + '")',)}, "manifest_limit"),
    ):
        add(name, answer, annotations=annotations, error=expected, validation=True, production_error=expected)
    add("helper_annotation", [flow.function("helper", ["PUSH_CONST 0", "RET"]), *answer],
        annotations={"helper": (annotation,)}, error="manifest_intent", validation=True)
    add("missing_main", [flow.function("helper", ["PUSH_CONST 42", "RET"])], error="manifest_intent", validation=True)
    add("main_parameter", [flow.function("main", ["STORE_LOCAL value", "LOAD_LOCAL value", "RET"], (("value", "i64"),))],
        error="manifest_intent", validation=True)
    add("main_void", [flow.function("main", ["RET"], result="void")], error="manifest_intent", validation=True)
    require(len(cases) == len({case["name"] for case in cases}), "duplicate boundary case")
    return cases


def compare_case(case, candidate, reference, production):
    expected_status = "validation_refused" if case["validation"] else "execution_refused" if case["error"] else "returned"
    require(candidate["status"] == expected_status, "raw outcome differs from boundary expectation")
    for key in ("result", "error", "instructions", "console_calls"):
        require(candidate[key] == case[key], f"raw {key} differs from boundary expectation")
    require(candidate["output_hex"] == case["output"].hex()
            and candidate["output_bytes"] == len(case["output"]), "raw output differs from boundary expectation")
    if case["production_returns"]:
        require(production == candidate, "annotated production behavior differs from raw semantics")
    else:
        require(production["status"] == "admission_refused" and production["error"] == case["production_error"],
                "production admission contract changed")
    if case["validation"]:
        require(reference is None, "validation-only fixture unexpectedly ran a reference")
        return "expected_validation_refusal"
    reference_status = "returned" if case["policy"] or not case["error"] else "execution_refused"
    require(reference is not None and reference["status"] == reference_status and reference["error"] == case["reference_error"],
            "finite reference outcome differs")
    expected_result = case["reference_result"] if case["policy"] else case["result"]
    require(reference["result"] == expected_result, "full i64 reference result differs")
    require(reference["output_hex"] == case["output"].hex(), "reference output differs")
    return "finite_runtime_policy_contrast" if case["policy"] else "matching_runtime_refusal" if case["error"] else "matching_semantics"


def boundary_proof(args, output, environment):
    directory = output / "boundary-editions"
    directory.mkdir()
    records, failures = [], []
    for number, case in enumerate(generated_cases()):
        path = directory / f"{number:03d}-{case['name']}.nil"
        path.write_bytes(case["edition"])
        record = {key: value for key, value in case.items() if key not in {"edition", "output"}}
        record.update(edition_bytes=len(case["edition"]), edition_sha256=digest(case["edition"]),
                      retained_path=str(path.relative_to(output)), expected_output_hex=case["output"].hex())
        try:
            process, stdout, _ = baseline.run([args.candidate, path, str(case["ceiling"]), str(case["steps"]), "16"],
                                              output / f"boundary-raw-{number:03d}", environment=environment)
            candidate = parse_probe(process, stdout)
            process, stdout, _ = baseline.run([args.production_candidate, path, str(case["ceiling"])],
                                              output / f"boundary-production-{number:03d}", environment=environment)
            production = flow.parse_probe(process, stdout)
            reference = None
            if not case["validation"]:
                process, stdout, _ = baseline.run([args.reference, path], output / f"boundary-reference-{number:03d}",
                                                  environment=environment)
                reference = parse_probe(process, stdout, reference=True)
            record.update(candidate=candidate, production=production, reference=reference)
            record["outcome"] = compare_case(case, candidate, reference, production)
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
            "production_comparisons": sum("production" in item for item in records)}


def instruction_inventory(data):
    result = scalar.instruction_inventory(data)
    result["lane"] = "raw_scalar_conformance"
    result["scope"] = "scalar lexical coverage only; absent main intent is permitted, complete validation still required"
    return result


@contextmanager
def corpus_adapters():
    previous = baseline.parse_probe, baseline.instruction_inventory
    baseline.parse_probe, baseline.instruction_inventory = parse_probe, instruction_inventory
    try:
        yield
    finally:
        baseline.parse_probe, baseline.instruction_inventory = previous


def production_corpus(args, output, environment, raw_records):
    records, failures = [], []
    for raw in raw_records:
        require(all(key in raw for key in ("invocation", "upstream_call", "edition_sha256", "edition_bytes", "retained_path")),
                "raw corpus record lacks complete edition binding")
        number = raw["invocation"]
        record = {key: raw[key] for key in ("invocation", "upstream_call", "edition_sha256", "edition_bytes", "retained_path")}
        record["lane"] = "production_profile3_admission"
        try:
            path = output / raw["retained_path"]
            edition = path.read_bytes()
            require(len(edition) == raw["edition_bytes"] and digest(edition) == raw["edition_sha256"],
                    "production corpus input differs from raw input")
            process, stdout, _ = baseline.run([args.production_candidate, path, "1"],
                                              output / f"corpus-production-{number:03d}", environment=environment)
            candidate = flow.parse_probe(process, stdout)
            record["candidate"] = candidate
            require(candidate["status"] == "admission_refused", "unmodified corpus unexpectedly passed production admission")
            record["outcome"] = "production_refused"
            record["scope"] = "production refusal is not raw semantic evidence"
        except (AssertionError, ValueError, OSError) as error:
            record.update(outcome="failed", failure=str(error))
            failures.append(number)
        records.append(record)
    refusal_errors = dict(Counter(item["candidate"]["error"] for item in records if item["outcome"] == "production_refused"))
    expected_errors = {"manifest_unsupported": 28, "manifest_edition": 9, "manifest_intent": 8}
    contract_passed = len(records) == 45 and refusal_errors == expected_errors
    result = {"lane": "production_profile3_admission", "passed": not failures and contract_passed,
              "invocations": len(records), "failures": failures,
              "refusals": sum(item["outcome"] == "production_refused" for item in records),
              "refusal_errors": refusal_errors, "expected_refusal_errors": expected_errors,
              "regression_contract_passed": contract_passed,
              "scope": "Independent production checks on the same complete bytes; no semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", records)
    write_json(output / "production-corpus-summary.json", result)
    return result


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
        if expected is not None and record.get("outcome") != "returned_match":
            failures.append(f"known returned comparison changed: {record['invocation']}")
        if expected is None and record.get("outcome") != "validation_refused":
            failures.append(f"known unsupported coverage changed: {record['invocation']}")
    errors = dict(Counter(item["candidate"]["error"] for item in records if item.get("outcome") == "validation_refused"))
    expected_errors = {"manifest_unsupported": 32, "manifest_edition": 9}
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
        record["lane"] = "raw_scalar_conformance"
    raw.update(lane="raw_scalar_conformance",
               unsupported_or_refused_executions=sum(item["outcome"] in {"validation_refused", "execution_refused"} for item in records),
               refusal_errors=dict(Counter(item["candidate"]["error"] for item in records
                                           if item["outcome"] in {"validation_refused", "execution_refused"})),
               scope="Exact unmodified upstream semantic comparisons; production intent is assessed in a separate lane. Refusal is unmet coverage.",
               regression_contract=raw_regression_contract(records))
    write_json(output / "corpus-executions.json", records)
    write_json(output / "corpus-summary.json", raw)
    production = production_corpus(args, output, environment, records)
    return raw, production


def qualification(args):
    for name in ("candidate", "production_candidate", "reference", "compiler", "root", "output"):
        setattr(args, name, getattr(args, name).resolve())
    args.interpreter = (args.interpreter or args.compiler.with_name("nilinterpreter")).resolve()
    require(not args.output.exists(), "output must be a new directory; evidence is never overwritten")
    args.output.mkdir(parents=True)
    args.output_created = True
    binaries = ("candidate", "production_candidate", "reference", "compiler", "interpreter")
    tools = {name: baseline.record_tool(getattr(args, name)) for name in binaries}
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
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit host_conformance feature, same scalar grammar without required main annotation",
        "production_lane": "separate unchanged profile-3 encoder/admission/execution probe on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536},
        "earlier_proofs": "126-, 77-, and 61-case proofs remain separate required runs",
    })
    environment = baseline.clean_environment()
    boundary = boundary_proof(args, args.output, environment)
    raw, production = qualify_corpus(args, args.output, environment, before)
    require(baseline.inventory(args.root) == before, "upstream inputs changed during proof")
    for name in binaries:
        require(baseline.record_tool(getattr(args, name)) == tools[name], f"tool changed: {name}")
    for record in [*tools["reference_core_archives"], *tools["earlier_profile_sources"], tools["script"]]:
        require(baseline.record_tool(Path(record["path"])) == record, "proof source/archive changed")
    for number, case in enumerate(generated_cases()):
        path = args.output / "boundary-editions" / f"{number:03d}-{case['name']}.nil"
        require(path.read_bytes() == case["edition"], "boundary input changed during proof")
    passed = (baseline.checkpoint_passed(boundary, raw) and raw["regression_contract"]["passed"]
              and production["passed"] and production["invocations"] == 45)
    result = {"schema": 1, "status": "host_conformance_boundary_checkpoint_passed" if passed else "host_conformance_boundary_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "scalar_language_profile": 3, "boundary_results": boundary, "raw_corpus": raw,
              "production_corpus": production, "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_boundary_inventory_is_authored_and_has_both_lanes(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 27)
        self.assertEqual(len({case["name"] for case in cases}), len(cases))
        self.assertEqual(sum(not case["error"] for case in cases), 8)
        self.assertEqual(sum(case["reference_error"] is not None for case in cases), 1)
        self.assertEqual(sum(case["policy"] for case in cases), 1)
        self.assertEqual(sum(case["validation"] for case in cases), 17)
        self.assertTrue(all(case["origin"] == "independently_authored_textual_nil" for case in cases))
        self.assertTrue(all(b"@ai_context" not in case["edition"] for case in cases if case["name"].startswith("intentless_")))

    def test_raw_parser_rejects_lane_confusion_and_effectful_refusals(self):
        process = {"exit_code": 0, "timed_out": False}
        record = {"status": "validation_refused", "result": None, "error": "manifest_intent", "output_hex": "",
                  "instructions": 0, "console_calls": 0, "output_bytes": 0}
        self.assertEqual(parse_probe(process, json.dumps(record).encode()), record)
        for field, value in (("status", "admission_refused"), ("instructions", 1), ("instructions", False),
                             ("console_calls", 1), ("output_bytes", 1), ("output_hex", "61"), ("result", 0)):
            with self.assertRaises(AssertionError):
                parse_probe(process, json.dumps(dict(record, **{field: value})).encode())
        with self.assertRaises(AssertionError):
            parse_probe(process, json.dumps(record).replace('{', '{"status":"returned",', 1).encode())
        with self.assertRaises(AssertionError):
            parse_probe(dict(process, timed_out=True), json.dumps(record).encode())

    def test_full_width_and_production_policy_mutations_fail(self):
        case = next(case for case in generated_cases() if case["name"] == "intentless_exact_i64")
        raw = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
               "instructions": 2, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": raw["result"], "error": None, "output_hex": ""}
        production = {"status": "admission_refused", "result": None, "error": "manifest_intent", "output_hex": "",
                      "instructions": 0, "console_calls": 0, "output_bytes": 0}
        self.assertEqual(compare_case(case, raw, reference, production), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(raw, **{field: value}), reference, production)
        with self.assertRaises(AssertionError):
            compare_case(case, raw, dict(reference, result=1), production)
        with self.assertRaises(AssertionError):
            compare_case(case, raw, reference, dict(production, error="manifest_unsupported"))

    def test_adapter_restores_older_helpers_on_failure(self):
        previous = baseline.parse_probe, baseline.instruction_inventory
        with self.assertRaises(RuntimeError):
            with corpus_adapters():
                self.assertIs(baseline.parse_probe, parse_probe)
                self.assertIs(baseline.instruction_inventory, instruction_inventory)
                raise RuntimeError("deliberate")
        self.assertEqual(previous, (baseline.parse_probe, baseline.instruction_inventory))

    def test_all_refused_or_changed_editions_cannot_preserve_known_coverage(self):
        records = []
        unsupported_count = 0
        for number in range(45):
            if number in known_raw_matches:
                size, identity = known_raw_matches[number]
                records.append({"invocation": number, "matched": True, "outcome": "returned_match",
                                "edition_bytes": size, "edition_sha256": identity,
                                "candidate": {"error": None}})
            else:
                error = "manifest_unsupported" if unsupported_count < 32 else "manifest_edition"
                unsupported_count += 1
                records.append({"invocation": number, "matched": False, "outcome": "validation_refused",
                                "candidate": {"error": error}})
        self.assertTrue(raw_regression_contract(records)["passed"])
        all_refused = [dict(item, matched=False, outcome="validation_refused", candidate={"error": "manifest_intent"})
                       if item["invocation"] in known_raw_matches else item for item in records]
        self.assertFalse(raw_regression_contract(all_refused)["passed"])
        changed = [dict(item, edition_sha256="0" * 64) if item["invocation"] == 28 else item for item in records]
        self.assertFalse(raw_regression_contract(changed)["passed"])
        self.assertFalse(raw_regression_contract(records[:-1])["passed"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "reference", "compiler", "root", "output"):
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
