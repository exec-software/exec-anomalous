#!/usr/bin/env python3
"""Qualify profile-3 scalar flow while retaining both earlier profile gates.

No builds, corpus edits or native execution occur here. Compiler editions stay
complete, copied tools are hashed before/after, and every upstream interpreter
input is attempted through a fresh unmodified full-corpus capture.
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
import program_flow_proof as flow


require = baseline.require
digest = baseline.digest
write_json = baseline.write_json
fixtures = Path(__file__).resolve().parents[1] / "crates/anomalous-program-executor/fixtures/scalar-flow"
compiler_fixture_hashes = {
    "bounds_once": "517a4e0cdfa225ff79e6782b93d28a9a1901831ddfc7f2c16827b1f98da1cce2",
    "break_continue": "1ff7284b4c4a62b27ec1f7c8a8448c2571e622f79a80dd8088fc8bc581caf03f",
    "fuel": "870f0910c27b41c6e193aec3c8e587318bb91e3ded64e07b3fe692314ce471fd",
    "nested_ranges": "36632a21a36b00b7623ab08cf450f49b30287109c8418e88d3f4abcd03b53b36",
    "range_widths": "d2e3601d8e748c2a6443bc36c2dfb66acbdf6f418b2a666a25d8c9385e1b5012",
    "scratch_joins": "98faa97f55cecbbc641eeace02569502860d64452aaa8e2f53633d74770f7cb2",
    "short_circuit": "b73bed9dc490970614e4a04010f2eb519fb45884de90f91e7309a808f758de8a",
    "zero_ranges": "826a01f5ff08dfb5500322e2206500451743494d2b36f06139fd74783ce0e32d",
}
compiler_source_hashes = {
    "bounds_once": "3940fa542368b7fb40147398cf8d16ae4b74ef069187fbb50d177d9806a595bc",
    "break_continue": "d7802f750c1bfd4141341f6b1c960addf41eabe674b1c1dc72ce954cc9ab4636",
    "fuel": "b1f21799a8f057e63938676914f78edf53a1c6324b4a1a470df0269eba2a68ac",
    "nested_ranges": "3f316555a6ea237917e9076ae0613480b1c90d4357c94a08d352b48a98584d91",
    "range_widths": "088f0b3fe2e776c9dfa81f944c45f406d5a83ebddb6cb771cc911ea663d3d749",
    "scratch_joins": "5d1698c9f2c8c2d395baf335f39463c8cf6febcf1cc0ed5f8154edab5e6b9573",
    "short_circuit": "361e5cab3a445a9b61605ff5ce9d2a81413e81e57e8d840ccc70b333346ab1c0",
    "zero_ranges": "3b3d568cbb9c7336a2c118cbee7a0fe01b45c9eb65f0d285c2868715f8fd962f",
}
compiler_expectations = {
    "zero_ranges": (7, b"", 0), "nested_ranges": (189, b"", 0),
    "break_continue": (13, b"0\n1\n3\n4\n5\n", 5), "bounds_once": (9, b"2\n5\n", 2),
    "short_circuit": (15, b"33\n44\n", 2), "scratch_joins": (430, b"", 0),
    "range_widths": (-3, b"2147483647\n-2147483648\n-2\n", 3), "fuel": (10000, b"", 0),
}
temporary_pattern = re.compile(r"\$ny\.temp\.[A-Za-z_][A-Za-z_0-9]*\.(?:0|[1-9][0-9]*)\Z")


def generated_cases():
    cases = []

    def add(name, body, result=0, *, helpers=(), strings=(), output=b"", calls=0, instructions=None,
            error=None, admission=False, ceiling=1, policy=False, reference_result=None,
            reference_output=None, cross_profile=True):
        cases.append({"name": name, "edition": flow.edition([flow.function("main", body), *helpers], strings),
                      "ceiling": ceiling, "steps": flow.maximum_steps, "frames": flow.maximum_frames,
                      "result": None if error else result, "output": output, "console_calls": calls,
                      "instructions": instructions, "error": error, "admission": admission,
                      "reference_error": None, "policy": policy, "reference_result": reference_result,
                      "reference_output": reference_output, "candidate_only": admission,
                      "cross_profile": cross_profile and not admission, "origin": "authored_textual_nil"})

    for name, left, right, expected in (
        ("high_wrap", 9223372036854775807, 1, -9223372036854775808),
        ("low_wrap", -9223372036854775808, -1, 9223372036854775807),
        ("i32_boundary", 2147483647, 1, 2147483648),
        ("exact_above_53_bits", 9007199254740992, 1, 9007199254740993),
        ("mixed_sign", -7, 4, -3),
        ("zero", 0, 0, 0),
    ):
        add(f"integer_add_{name}", [f"PUSH_CONST {left}", f"PUSH_CONST {right}", "ADD", "RET"], expected, instructions=4)
    valid_names = ("$ny.temp.idx.0", "$ny.temp.logical_and.7", "$ny.temp._scratch2.19",
                   "$ny.temp.a.18446744073709551616", "$ny.temp." + "a" * 117 + ".0")
    for index, name in enumerate(valid_names):
        add(f"temporary_name_{index}", ["PUSH_CONST 42", f"STORE_LOCAL {name}", f"LOAD_LOCAL {name}", "RET"],
            42, instructions=4)
    add("authored_and_generated_locals_are_distinct", ["PUSH_CONST 70", "STORE_LOCAL idx_0", "PUSH_CONST 3",
        "STORE_LOCAL $ny.temp.idx.0", "LOAD_LOCAL idx_0", "LOAD_LOCAL $ny.temp.idx.0", "ADD i64", "RET"], 73, instructions=8)
    add("temporary_call_frame_isolation", ["PUSH_CONST 10", "STORE_LOCAL $ny.temp.idx.0", "CALL helper 0",
        "LOAD_LOCAL $ny.temp.idx.0", "ADD i64", "RET"], 30, instructions=10,
        helpers=(flow.function("helper", ["PUSH_CONST 20", "STORE_LOCAL $ny.temp.idx.0", "LOAD_LOCAL $ny.temp.idx.0", "RET"]),))
    for name, condition, opcode, fallback, expected in (
        ("and_taken_rhs_verbatim", 1, "JMP_IF_NOT", 0, 7),
        ("and_short_circuit_zero", 0, "JMP_IF_NOT", 0, 0),
        ("or_taken_rhs_verbatim", 0, "JMP_IF", 1, 7),
        ("or_short_circuit_one", -1, "JMP_IF", 1, 1),
    ):
        add(name, [f"PUSH_CONST {condition}", f"{opcode} fallback", "PUSH_CONST 7", "STORE_LOCAL $ny.temp.logical.0", "JMP done",
            "LABEL fallback", f"PUSH_CONST {fallback}", "STORE_LOCAL $ny.temp.logical.0", "LABEL done",
            "LOAD_LOCAL $ny.temp.logical.0", "RET"], expected, instructions=8)
    add("unreachable_bare_add_is_not_executed", ["PUSH_CONST 42", "RET", "ADD"], 42, instructions=2)
    dead_console = ["PUSH_CONST 0", "STORE_LOCAL $ny.temp.logical_and.0", "LOAD_LOCAL $ny.temp.logical_and.0", "JMP_IF_NOT skip",
                    "PUSH_CONST_STR S0", "CONSOLE_WRITE", "LABEL skip", "PUSH_CONST 42", "RET"]
    add("skipped_console_granted", dead_console, 42, strings=("unreachable",), instructions=7)
    add("skipped_console_denied", dead_console, strings=("unreachable",), error="manifest_ceiling", admission=True, ceiling=0)
    for count in (128, 129):
        body = [instruction for index in range(count) for instruction in (f"PUSH_CONST {index}", f"STORE_LOCAL $ny.temp.slot.{index}")]
        add(f"temporary_locals_{count}", body + [f"LOAD_LOCAL $ny.temp.slot.{count - 1}", "RET"], count - 1,
            instructions=258 if count == 128 else None, error="manifest_limit" if count == 129 else None, admission=count == 129)
    malformed = ("$ny.temp.idx.00", "$ny.temp.idx.01", "$ny.temp.idx.+1", "$ny.temp.idx.-1", "$ny.temp.idx",
                 "$ny.temp..0", "$ny.temp.query.index.0", "$ny.temp.1idx.0", "$ny.temp.idx.", "$ny.temp.idx.0x1",
                 "$ny.other.idx.0", "$ny.temp.idx.٠", "$ny.temp." + "a" * 118 + ".0")
    for index, name in enumerate(malformed):
        add(f"reject_temporary_spelling_{index}", ["PUSH_CONST 1", f"STORE_LOCAL {name}", "PUSH_CONST 0", "RET"],
            error="manifest_limit" if len(name.encode()) > 128 else "manifest_syntax", admission=True)
    for name, body in {
        "uninitialized_temporary": ["LOAD_LOCAL $ny.temp.idx.0", "RET"],
        "temporary_not_definite": ["PUSH_CONST 0", "JMP_IF join", "PUSH_CONST 1", "STORE_LOCAL $ny.temp.idx.0",
                                   "LABEL join", "LOAD_LOCAL $ny.temp.idx.0", "RET"],
        "temporary_kind_join": ["PUSH_CONST 0", "JMP_IF right", "PUSH_CONST 1", "STORE_LOCAL $ny.temp.idx.0", "JMP join",
                                "LABEL right", "PUSH_CONST_STR S0", "STORE_LOCAL $ny.temp.idx.0", "LABEL join", "PUSH_CONST 0", "RET"],
        "bare_add_text_left": ["PUSH_CONST_STR S0", "PUSH_CONST 1", "ADD", "RET"],
        "bare_add_text_right": ["PUSH_CONST 1", "PUSH_CONST_STR S0", "ADD", "RET"],
        "bare_add_text_both": ["PUSH_CONST_STR S0", "PUSH_CONST_STR S0", "ADD", "RET"],
        "bare_add_underflow": ["PUSH_CONST 1", "ADD", "RET"],
    }.items():
        add(f"reject_{name}", body, strings=("text",), error="manifest_syntax", admission=True)
    for name, body, helpers in (
        ("label", ["LABEL $ny.temp.target.0", "PUSH_CONST 0", "RET"], ()),
        ("jump", ["JMP $ny.temp.target.0", "PUSH_CONST 0", "RET"], ()),
        ("call", ["CALL $ny.temp.target.0 0", "RET"], ()),
        ("function", ["PUSH_CONST 0", "RET"], (flow.function("$ny.temp.helper.0", ["PUSH_CONST 0", "RET"]),)),
        ("parameter", ["PUSH_CONST 0", "RET"], (flow.function("helper", ["STORE_LOCAL $ny.temp.arg.0", "PUSH_CONST 0", "RET"],
                                                                 (("$ny.temp.arg.0", "i64"),)),)),
    ):
        add(f"reject_temporary_{name}_context", body, helpers=helpers, error="manifest_syntax", admission=True)
    add("reject_temporary_constant_context", ["PUSH_CONST 0", "RET"], strings=("text",), error="manifest_syntax", admission=True)
    cases[-1]["edition"] = cases[-1]["edition"].replace(b"const S0 =", b"const $ny.temp.constant.0 =")
    for opcode in ("SUB", "MUL", "NEG", "BIT_AND"):
        add(f"reject_untyped_{opcode.lower()}", ["PUSH_CONST 1", "PUSH_CONST 2", opcode, "RET"],
            error="manifest_unsupported", admission=True)
    add("reject_dead_foreign_effect", ["PUSH_CONST 0", "RET", "INTEROP_CALL libc puts 1 i64"],
        error="manifest_unsupported", admission=True)
    require(len({case["name"] for case in cases}) == len(cases), "duplicate scalar-flow case")
    return cases


@contextmanager
def compiler_adapters():
    values = {"fixtures": fixtures, "compiler_fixture_hashes": compiler_fixture_hashes,
              "compiler_source_hashes": compiler_source_hashes, "compiler_expectations": compiler_expectations}
    previous = {name: getattr(flow, name) for name in values}
    try:
        for name, value in values.items():
            setattr(flow, name, value)
        yield
    finally:
        for name, value in previous.items():
            setattr(flow, name, value)


def instruction_inventory(data):
    result = flow.instruction_inventory(data)
    result["profile"] = 3
    for instruction in result["instructions"]:
        opcode, operands = instruction["opcode"], instruction["operands"]
        if opcode == "ADD" and not operands:
            instruction["supported_form"] = True
        if opcode in {"LOAD_LOCAL", "STORE_LOCAL"} and len(operands) == 1:
            if temporary_pattern.fullmatch(operands[0]) and len(operands[0]) <= 128:
                instruction["supported_form"] = True
    result["scope"] = "profile-3 lexical coverage only; integer kinds, references, authority and CFG require admission"
    return result


@contextmanager
def corpus_adapters():
    previous = baseline.parse_probe, baseline.instruction_inventory
    baseline.parse_probe, baseline.instruction_inventory = flow.parse_probe, instruction_inventory
    try:
        yield
    finally:
        baseline.parse_probe, baseline.instruction_inventory = previous


def profile_proof(args, output, environment):
    with compiler_adapters():
        compiled, sources = flow.compiler_cases(args, output, environment)
    fuel = next(case for case in compiled if case["name"] == "compiler_fuel")
    fuel.update(result=None, error="instruction_limit", instructions=65536, policy=True,
                reference_result=10000, reference_output=b"")
    cases = generated_cases() + compiled
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
            candidate = flow.parse_probe(process, stdout)
            reference = None
            if not case["candidate_only"]:
                require(candidate["status"] != "admission_refused", "expected admitted edition was refused")
                process, stdout, _ = baseline.run([args.reference, path], output / f"reference-{index:03d}", environment=environment)
                reference = flow.parse_probe(process, stdout, reference=True)
            item.update(candidate=candidate, reference=reference)
            outcome = flow.compare_case(case, candidate, reference)
            if case["cross_profile"]:
                process, stdout, _ = baseline.run([args.profile2_candidate, path, str(case["ceiling"])],
                                                  output / f"profile2-{index:03d}", environment=environment)
                profile2 = flow.parse_probe(process, stdout)
                require(profile2["status"] == "admission_refused", "profile-2 admitted a profile-3 extension")
                item["profile2"] = profile2
            item["outcome"] = outcome
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(case["name"])
        results.append(item)
    write_json(output / "profile-results.json", {"profile": 3, "cases": results, "failures": failures})
    return {"passed": not failures, "cases": len(results), "failures": failures,
            "semantic_matches": sum(item["outcome"] == "matching_semantics" for item in results),
            "deliberate_runtime_policy_refusals": sum(item["outcome"] == "deliberate_runtime_policy_refusal" for item in results),
            "expected_admission_refusals": sum(item["outcome"] == "expected_admission_refusal" for item in results),
            "profile2_raw_edition_refusals": sum("profile2" in item for item in results)}, sources


def qualification(args):
    for name in ("candidate", "profile2_candidate", "reference", "compiler", "root", "output"):
        setattr(args, name, getattr(args, name).resolve())
    args.interpreter = (args.interpreter or args.compiler.with_name("nilinterpreter")).resolve()
    require(not args.output.exists(), "output must be a new directory")
    args.output.mkdir(parents=True)
    args.output_created = True
    binaries = ("candidate", "profile2_candidate", "reference", "compiler", "interpreter")
    tools = {name: baseline.record_tool(getattr(args, name)) for name in binaries}
    require(tools["compiler"]["sha256"] == baseline.compiler_hash and tools["interpreter"]["sha256"] == baseline.interpreter_hash,
            "qualified toolchain pins changed")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.root, text=True).strip() == baseline.revision,
            "qualified upstream revision changed")
    require(not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=args.root, text=True),
            "qualified upstream tracked source changed")
    tools["reference_core_archives"] = [baseline.record_tool(args.root / "build/context-registration/lib" / name)
                                        for name in ("libnilcompiler_core.a", "libnil.a")]
    archive_pins = {
        "libnilcompiler_core.a": "edf0059ec7a93bb6b6292021048891edfedc741448717eede678b4a8bb087c51",
        "libnil.a": "f36a65ff09ebaa43586782a95d567c1ad28352011d0fe736d6915fa1bf2683e3",
    }
    for archive in tools["reference_core_archives"]:
        require(archive["sha256"] == archive_pins[Path(archive["path"]).name], "qualified reference archive pin changed")
    source_pins = {
        "program_execution_proof.py": "c86a0a9357f2a0e361baea1e4fbcabe4e8a1018111e6aa5cf57e98eb92bc75b3",
        "program_execution_reference.cpp": "259aef1f61c850cd887a913e75024e936c81f8bb34dff8a62473022e9658fa0e",
        "program_flow_proof.py": "d9bd4098e0c8ca5ca6f372752c8d6e6698c1a75467d21c91e34ce6f636b8400f",
        "program_flow_reference.cpp": "73431f73d39bf72831fb3f727ed94035ae654c8a0bf8e10dab554984db9031e9",
    }
    tools["earlier_profile_sources"] = []
    for name, expected in source_pins.items():
        record = baseline.record_tool(Path(__file__).with_name(name))
        require(record["sha256"] == expected, f"earlier profile source changed: {name}")
        tools["earlier_profile_sources"].append(record)
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_scalar_flow_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "candidate_profile": 3, "candidate_limits": {"steps": 65536, "frames": 16, "output_bytes": 65536},
        "earlier_profiles": "original 126-case and 77-case proofs remain separate required runs",
    })
    environment = baseline.clean_environment()
    profile, sources = profile_proof(args, args.output, environment)
    with corpus_adapters():
        corpus = baseline.corpus_proof(args, args.output, environment, before)
    require(baseline.inventory(args.root) == before, "upstream inputs changed during proof")
    for name in binaries:
        require(baseline.record_tool(getattr(args, name)) == tools[name], f"tool changed: {name}")
    for record in [*tools["reference_core_archives"], *tools["earlier_profile_sources"], tools["reference_source"], tools["script"]]:
        require(baseline.record_tool(Path(record["path"])) == record, "proof source/archive changed")
    for source in sources:
        for path in (Path(source["path"]), args.output / source["retained"]):
            data = path.read_bytes()
            require(len(data) == source["bytes"] and digest(data) == source["sha256"], "fixture snapshot changed")
    passed = baseline.checkpoint_passed(profile, corpus)
    result = {"schema": 1, "profile": 3, "status": "bounded_scalar_flow_checkpoint_passed" if passed else "bounded_scalar_flow_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "profile_results": profile, "corpus": corpus,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_preserves_distinct_semantics_and_refusals(self):
        cases = generated_cases()
        self.assertEqual(len(cases), len({case["name"] for case in cases}))
        self.assertTrue(all(case["candidate_only"] == case["admission"] for case in cases))
        self.assertTrue(all(not case["admission"] for case in cases if case["cross_profile"]))
        self.assertIn("integer_add_exact_above_53_bits", {case["name"] for case in cases})
        self.assertEqual(set(compiler_expectations), {path.stem for path in fixtures.glob("*.ny")})

    def test_temporary_lexical_metadata_remains_narrow(self):
        for value in ("$ny.temp.idx.0", "$ny.temp._a2.10"):
            self.assertIsNotNone(temporary_pattern.fullmatch(value))
        for value in ("$ny.temp.idx.00", "$ny.temp.query.index.0", "$ny.temp.2a.0", "$ny.temp.idx.-1", "$ny.temp.idx.٠"):
            self.assertIsNone(temporary_pattern.fullmatch(value))
        case = next(case for case in generated_cases() if case["name"] == "temporary_call_frame_isolation")
        coverage = instruction_inventory(case["edition"])
        self.assertEqual(coverage["profile"], 3)
        self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]))
        self.assertIn("lexical", coverage["scope"])

    def test_contexts_restore_imported_modules_after_failure(self):
        previous = baseline.parse_probe, baseline.instruction_inventory, flow.fixtures, flow.compiler_expectations
        with self.assertRaises(RuntimeError):
            with compiler_adapters(), corpus_adapters():
                self.assertIs(flow.fixtures, fixtures)
                self.assertIs(baseline.instruction_inventory, instruction_inventory)
                raise RuntimeError("deliberate")
        self.assertEqual(previous, (baseline.parse_probe, baseline.instruction_inventory, flow.fixtures, flow.compiler_expectations))

    def test_result_and_counter_mutations_cannot_pass(self):
        case = next(case for case in generated_cases() if case["name"] == "integer_add_high_wrap")
        candidate = {"status": "returned", "result": -(1 << 63), "error": None, "output_hex": "",
                     "instructions": 4, "console_calls": 0, "output_bytes": 0}
        reference = {key: value for key, value in candidate.items() if key in {"status", "result", "error", "output_hex"}}
        reference["schema"] = 1
        self.assertEqual(flow.compare_case(case, candidate, reference), "matching_semantics")
        for key, value in (("result", 0), ("instructions", 3), ("console_calls", 1), ("error", "other")):
            with self.assertRaises(AssertionError):
                flow.compare_case(case, dict(candidate, **{key: value}), reference)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "profile2-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "profile2_candidate", "reference", "compiler", "root", "output"):
        if getattr(args, name) is None:
            parser.error(f"--{name.replace('_', '-')} is required")
    try:
        return qualification(args)
    except (AssertionError, ValueError, OSError) as error:
        failure = {"schema": 1, "profile": 3, "status": "qualification_infrastructure_failed",
                   "candidate_qualified": False, "native_execution": False, "error": str(error)}
        if getattr(args, "output_created", False):
            write_json(args.output / "failure.json", failure)
        print(json.dumps(failure), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
