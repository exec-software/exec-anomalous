#!/usr/bin/env python3
"""Qualify profile-10 source-debug metadata with unchanged whole-edition execution.

Metadata remains descriptive: no path is opened and no lexical binding grants
runtime type, service authority or production intent. Complete captured editions
retain their original temporary source paths and their exact per-run hashes.
"""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import copy
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest

import program_execution_proof as baseline
import program_flow_proof as flow
import program_record_proof as records
import program_enum_proof as enums_proof
import program_conformance_proof as conformance

require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
source_pins = dict(enums_proof.source_pins, **{
    "program_enum_proof.py": "15674db0f0f4224e6dbfdcbda8734e9a2ff6c3eb83cf78f91b11b0f8a1a07864",
    "program_enum_reference.cpp": "462b276c1af3f2861126f52eecc564239d52333feada100cd879365e7d279db4",
})
archive_pins = enums_proof.archive_pins
known_raw_matches = dict(enums_proof.known_raw_matches)
expected_raw_matches = sorted([*known_raw_matches, 4])
primary_pins = {
    "nilcompiler/src/source_debug.hpp": "a0570dd8be895a081de265c248b4da4d8d269a8b769a260359f3a435e6c614ae",
    "nilcompiler/src/nil_loader.cpp": "158b3a9fea96c0db0e4dfb249b3a4072d535c309c29aba23db6951f9e2410b69",
    "nilcompiler/src/codegen.hpp": "12dad0791a46441fe008dd8ee7d84c080efc478cbabea36d1f61615c19230561",
    "nilcompiler/src/codegen.cpp": "57cf7b4ac662fa238e676686d8d22cfea01b08755aca6acc54fee1c9b6139721",
    "nilcompiler/src/vm.cpp": "a864f20755805a2ecfe3e056e0191245f5c35190aa9e4adf8435e89d4cb19dec",
    "nilcompiler/src/vm.hpp": "0238de4f2be537d5e9e5db2186f0e1b413b0690fbef017bd9ad9e669c15dda99",
}


def validate_snapshot(value, *, reference=False):
    require(type(value) is list and len(value) <= 4096, "invalid metadata record array")
    seen, closed, current, previous_index = set(), set(), None, -1
    for record in value:
        require(type(record) is dict and set(record) == {"function", "instruction_index", "path_hex", "bindings"},
                "metadata record schema changed")
        name, index = record["function"], record["instruction_index"]
        require(type(name) is str and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", name) is not None,
                "invalid metadata function name")
        require(type(index) is int and 0 <= index < 4096, "invalid metadata instruction index")
        require((name, index) not in seen, "duplicate metadata location")
        seen.add((name, index))
        if name != current:
            require(name not in closed, "noncontiguous metadata function")
            if current is not None:
                closed.add(current)
            current, previous_index = name, -1
        require(index > previous_index, "metadata indices not increasing")
        previous_index = index
        def string(hex_value, maximum):
            require(type(hex_value) is str and re.fullmatch(r"(?:[0-9a-f]{2})+", hex_value) is not None,
                    "noncanonical metadata byte string")
            raw = bytes.fromhex(hex_value)
            require(len(raw) <= maximum, "metadata string exceeds cap")
            text = raw.decode("utf-8", errors="strict")
            require(all(ord(c) >= 32 and ord(c) != 127 for c in text), "metadata includes control byte")
            require(reference or all(not 0x80 <= ord(c) <= 0x9f for c in text), "candidate metadata includes C1 control")
            return raw
        string(record["path_hex"], 16384)
        bindings = record["bindings"]
        require(type(bindings) is list and len(bindings) <= (1024 if reference else 128), "invalid metadata bindings")
        names, storage = set(), set()
        for binding in bindings:
            require(type(binding) is dict and set(binding) == {"name_hex", "storage_hex", "type_hex", "scope_depth"},
                    "metadata binding schema changed")
            for field in ("name_hex", "storage_hex", "type_hex"):
                string(binding[field], 16384 if reference else 128)
            require(binding["name_hex"] not in names and binding["storage_hex"] not in storage, "duplicate lexical binding")
            names.add(binding["name_hex"])
            storage.add(binding["storage_hex"])
            require(type(binding["scope_depth"]) is int and 0 <= binding["scope_depth"] <= 1024, "invalid metadata scope depth")
    return value


def parse_probe(process, output, *, reference=False, production=False):
    value = baseline.strict_json(output.decode("utf-8"))
    require(type(value) is dict and "source_debug" in value, "source-debug snapshot missing")
    metadata = validate_snapshot(value.pop("source_debug"), reference=reference)
    parser = flow.parse_probe if production else conformance.parse_probe
    value = parser(process, json.dumps(value).encode(), reference=reference)
    if value["status"] in {"admission_refused", "validation_refused"}:
        require(metadata == [], "refused admission published debug metadata")
    value["source_debug"] = metadata
    return value


def parse_production(process, output):
    return parse_probe(process, output, production=True)


def quote(value):
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'


def snapshot(function, index, path, bindings=()):
    return {"function": function, "instruction_index": index, "path_hex": path.encode().hex(),
            "bindings": [{"name_hex": name.encode().hex(), "storage_hex": storage.encode().hex(),
                          "type_hex": kind.encode().hex(), "scope_depth": depth}
                         for name, storage, kind, depth in bindings]}


def debug(index, path="/source/main.matrix", bindings=(), *, text=None):
    line = f"# source_debug {index} {quote(path)} {len(bindings)}"
    for name, storage, kind, depth in bindings:
        line += f" {quote(name)} {quote(storage)} {quote(kind)} {depth}"
    return {"line": line if text is None else text, "index": index, "path": path, "bindings": bindings}


def authored_edition(functions, *, annotated=True, enums=(), layouts=(), strings=()):
    normalized, expected = [], []
    for name, parameters, result, body in functions:
        lines = []
        for value in body:
            if isinstance(value, dict):
                expected.append(snapshot(name, value["index"], value["path"], value["bindings"]))
                lines.append(value["line"])
            else:
                lines.append(value)
        normalized.append(flow.function(name, lines, parameters, result))
    annotations = {"main": ('@ai_context("Exercise bounded source debug metadata")',)} if annotated else {}
    return enums_proof.authored_enum_edition(normalized, enums, strings, annotations, layouts=layouts), expected


def generated_cases():
    cases = []
    basic = ("value", "slot", "i64", 0)
    def add(name, body, *, helpers=(), result=42, output=b"", calls=0, instructions=None, error=None,
            validation=False, policy=False, metadata_policy=False, reference_result=None, reference_error=None,
            reference_output=None, annotated=True, production_error=None, ceiling=0, steps=65536,
            frames=16, cross_profile=True, enums=(), layouts=(), strings=(), transform=None):
        edition, metadata = authored_edition([flow.function("main", body), *helpers], annotated=annotated,
                                            enums=enums, layouts=layouts, strings=strings)
        if transform:
            edition = transform(edition)
        require(len(edition) <= 65536, f"fixture exceeds edition bound: {name}")
        cases.append({"name": name, "edition": edition, "result": None if error else result, "error": error,
                      "output": output, "console_calls": calls,
                      "instructions": 0 if validation else sum(isinstance(line, str) and bool(line.strip()) for line in body)
                      if instructions is None else instructions,
                      "validation": validation, "policy": policy, "metadata_policy": metadata_policy,
                      "reference_result": reference_result, "reference_error": reference_error,
                      "reference_output": reference_output, "source_debug": [] if validation else metadata,
                      "reference_source_debug": metadata, "annotated": annotated,
                      "production_error": production_error or (error if validation and annotated else "manifest_intent" if not annotated else None),
                      "ceiling": ceiling, "steps": steps, "frames": frames, "cross_profile": cross_profile,
                      "origin": "independently_authored_textual_nil"})
    def reject(name, body, *, error="manifest_syntax", **kwargs):
        add("reject_" + name, body, error=error, validation=True, cross_profile=False, **kwargs)
    def contrast(name, record, *, error="manifest_syntax", **kwargs):
        add("policy_" + name, [record, "PUSH_CONST 42", "RET"], error=error, validation=True,
            metadata_policy=True, reference_result=42, reference_output=b"", **kwargs)

    add("plain_profile9_inherited", ["PUSH_CONST 42", "RET"], cross_profile=False)
    add("first_instruction", [debug(0), "PUSH_CONST 42", "RET"])
    add("last_instruction", ["PUSH_CONST 42", debug(1), "RET"])
    add("every_instruction", [debug(0), "PUSH_CONST 42", debug(1, bindings=(basic,)), "RET"])
    add("metadata_gaps_do_not_renumber", ["PUSH_CONST 1", "POP", debug(2), "PUSH_CONST 42", "RET"])
    add("labels_are_instructions", [debug(0), "LABEL begin", debug(1), "PUSH_CONST 42", debug(2), "RET"])
    add("blank_lines_do_not_consume_pending", [debug(0), "", " ", "PUSH_CONST 42", "RET"])
    add("whole_dead_metadata_retained", ["PUSH_CONST 42", "RET", debug(2, bindings=(basic,)), "PUSH_CONST 99"], instructions=2)
    add("unbound_descriptive_names", [debug(0, bindings=(("not an identifier", "$unbound.slot", "missing<type>", 1024),)), "PUSH_CONST 42", "RET"])
    for name, path in (("semicolon", 'book; chapter.matrix'), ("comma", 'book,chapter.matrix'),
                       ("quote_and_backslash", 'C:\\source\\"page".matrix'), ("unicode", '/資料/π🙂.matrix'),
                       ("not_a_file", '/never-created/source-debug-proof/absent.matrix'),
                       ("uri_is_data", 'https://example.invalid/not-fetched.matrix'),
                       ("sensitive_path_is_data", '/proc/self/environ')):
        add("path_" + name, [debug(0, path), "PUSH_CONST 42", "RET"])
    add("unicode_binding_fields", [debug(0, bindings=(("変数🙂", "保存π", "任意<型>", 3),)), "PUSH_CONST 42", "RET"])
    add("quoted_binding_punctuation", [debug(0, bindings=(('name;"', 'slot,\\', 'opaque type;"', 1),)), "PUSH_CONST 42", "RET"])
    add("path_exact_capacity", [debug(0, 'p' * 16384), "PUSH_CONST 42", "RET"])
    add("path_decoded_capacity_with_escapes", [debug(0, '\\' * 16384), "PUSH_CONST 42", "RET"])
    for index, field in enumerate(("name", "storage", "type")):
        binding = list(basic); binding[index] = "x" * 128
        add("binding_" + field + "_exact_capacity", [debug(0, bindings=(binding,)), "PUSH_CONST 42", "RET"])
    add("binding_decoded_capacity_with_escapes", [debug(0, bindings=(("\\" * 128, "slot", "i64", 0),)), "PUSH_CONST 42", "RET"])
    many = tuple((f"v{i}", f"s{i}", "i64", i % 1025) for i in range(128))
    add("bindings_exact_capacity", [debug(0, bindings=many), "PUSH_CONST 42", "RET"])
    add("scope_exact_capacity", [debug(0, bindings=(("value", "slot", "i64", 1024),)), "PUSH_CONST 42", "RET"])
    add("zero_padded_counts", [debug(0, bindings=(basic,), text='# source_debug 000 "/source/main.matrix" 0001 "value" "slot" "i64" 000'), "PUSH_CONST 42", "RET"])
    add("horizontal_token_spacing", [debug(0, text='# source_debug\t0\t"/source/main.matrix"\t0'), "PUSH_CONST 42", "RET"])
    helper = flow.function("echo", [debug(0, bindings=(("argument", "argument", "fiction", 0),)), "STORE_LOCAL argument", debug(1), "LOAD_LOCAL argument", "RET"], parameters=(("argument", "i64"),))
    add("per_function_indices_reset", [debug(0), "PUSH_CONST 42", debug(1), "CALL echo 1", debug(2), "RET"], helpers=(helper,), instructions=6)
    unused = flow.function("unused", [debug(0, bindings=(basic,)), "PUSH_CONST 19", debug(1), "RET"])
    add("unused_helper_metadata_retained", [debug(0), "PUSH_CONST 42", "RET"], helpers=(unused,))
    add("metadata_does_not_supply_main_intent", [debug(0, bindings=(("@ai_context", "console", "authority", 0),)), "PUSH_CONST 42", "RET"], annotated=False)
    add("console_accounting_ignores_metadata", [debug(0), "PUSH_CONST 42", debug(1), "CONSOLE_WRITELN", debug(2), "PUSH_CONST 42", debug(3), "RET"], output=b"42\n", calls=1, ceiling=1)
    add("loop_reuses_instruction_metadata", [debug(0), "PUSH_CONST 0", "STORE_LOCAL index", debug(2), "LABEL again", "LOAD_LOCAL index", "PUSH_CONST 1", "ADD", "STORE_LOCAL index", "LOAD_LOCAL index", "PUSH_CONST 3", "CMP_LT", "JMP_IF again", "PUSH_CONST 42", "RET"], instructions=31)
    add("full_i64_return_unchanged", [debug(0), "PUSH_CONST 9007199254740993", debug(1), "RET"], result=9007199254740993)
    add("enum_metadata_is_descriptive", [debug(0, bindings=(("chosen", "chosen", "not::real", 0),)), "PUSH_CONST 42", "NEW_VARIANT choice::Busy 1 choice i64", "UNWRAP_VARIANT 1 choice i64", "RET"], enums=(enums_proof.enum_layout("choice"),))
    add("record_metadata_is_descriptive", [debug(0, bindings=(("point", "point", "i64", 0),)), "NEW_STRUCT point", "PUSH_CONST 42", "SET_FIELD value", "GET_FIELD value", "RET"], layouts=(records.layout("point", ("value",)),))
    full = [debug(0), *[item for _ in range(2047) for item in ("PUSH_CONST 0", "POP")], "PUSH_CONST 42", "RET"]
    add("instruction_capacity_includes_no_metadata", full, instructions=4096)
    add("metadata_retained_after_arithmetic_fault", [debug(0), "PUSH_CONST 7", "CONSOLE_WRITELN", debug(2), "PUSH_CONST 1", "PUSH_CONST 0", debug(4), "DIV i64", "RET"], error="division_by_zero", output=b"7\n", calls=1, instructions=5, reference_error="Division by zero", ceiling=1)
    add("metadata_does_not_use_instruction_fuel", [debug(0), "PUSH_CONST 42", debug(1), "RET"], error="instruction_limit", instructions=1, policy=True, reference_result=42, reference_output=b"", steps=1)

    contrast("unquoted_path", debug(0, "plain.matrix", text='# source_debug 0 plain.matrix 0'))
    contrast("unquoted_binding_strings", debug(0, bindings=(basic,), text='# source_debug 0 "/source/main.matrix" 1 value slot i64 0'))
    contrast("quoted_index", debug(0, text='# source_debug "0" "/source/main.matrix" 0'))
    contrast("quoted_binding_count", debug(0, text='# source_debug 0 "/source/main.matrix" "0"'))
    contrast("quoted_scope", debug(0, bindings=(basic,), text='# source_debug 0 "/source/main.matrix" 1 "value" "slot" "i64" "0"'))
    contrast("comma_separators", debug(0, bindings=(basic,), text='# source_debug 0, "/source/main.matrix", 1, "value", "slot", "i64", 0'))
    for suffix in ("q", "x41", "u0041"):
        contrast("unknown_escape_" + suffix, debug(0, "\\" + suffix, text=f'# source_debug 0 "\\{suffix}" 0'), error="manifest_unsupported")
    contrast("c1_unicode_control", debug(0, "path\u0085name"))
    contrast("bindings_over_candidate_capacity", debug(0, bindings=(*many, ("last", "last", "i64", 0))), error="manifest_limit")
    for index, field in enumerate(("name", "storage", "type")):
        binding = list(basic); binding[index] = "x" * 129
        contrast("binding_" + field + "_over_candidate_capacity", debug(0, bindings=(binding,)), error="manifest_limit")

    for name, text in (("wrong_first_index", '# source_debug 1 "p" 0'), ("negative_index", '# source_debug -1 "p" 0'),
                       ("plus_index", '# source_debug +0 "p" 0'), ("hex_index", '# source_debug 0x0 "p" 0'),
                       ("fractional_index", '# source_debug 0.0 "p" 0'), ("unicode_index", '# source_debug ０ "p" 0'),
                       ("overflow_index", '# source_debug 184467440737095516160 "p" 0'),
                       ("negative_count", '# source_debug 0 "p" -1'), ("plus_count", '# source_debug 0 "p" +0'),
                       ("hex_count", '# source_debug 0 "p" 0x0'), ("fractional_count", '# source_debug 0 "p" 0.0'),
                       ("missing_count", '# source_debug 0 "p"'), ("missing_path", '# source_debug 0'),
                       ("missing_index", '# source_debug'), ("extra_operand", '# source_debug 0 "p" 0 tail'),
                       ("trailing_comment", '# source_debug 0 "p" 0 ; line 5'),
                       ("missing_binding", '# source_debug 0 "p" 1'),
                       ("extra_binding", '# source_debug 0 "p" 0 "name" "slot" "i64" 0'),
                       ("missing_type", '# source_debug 0 "p" 1 "name" "slot"'),
                       ("missing_depth", '# source_debug 0 "p" 1 "name" "slot" "i64"'),
                       ("negative_depth", '# source_debug 0 "p" 1 "name" "slot" "i64" -1'),
                       ("plus_depth", '# source_debug 0 "p" 1 "name" "slot" "i64" +0'),
                       ("hex_depth", '# source_debug 0 "p" 1 "name" "slot" "i64" 0x0'),
                       ("unterminated_quote", '# source_debug 0 "p 0'),
                       ("trailing_backslash", '# source_debug 0 "p\\'),
                       ("adjacent_string", '# source_debug 0 "p""other" 0'),
                       ("empty_path", '# source_debug 0 "" 0')):
        reject(name, [text, "PUSH_CONST 42", "RET"])
    for index, field in enumerate(("name", "storage", "type")):
        binding = list(basic); binding[index] = ""
        reject("empty_" + field, [debug(0, bindings=(binding,)), "PUSH_CONST 42", "RET"])
    for name, bindings in (("duplicate_names", (basic, ("value", "other", "text", 1))),
                           ("duplicate_storage", (basic, ("other", "slot", "text", 1)))):
        reject(name, [debug(0, bindings=bindings), "PUSH_CONST 42", "RET"])
    for escape in ('n', 'r', 't', 'v', 'f', '0'):
        reject("escaped_control_" + escape, [f'# source_debug 0 "p\\{escape}" 0', "PUSH_CONST 42", "RET"])
    reject("literal_tab_in_string", ['# source_debug 0 "p\tq" 0', "PUSH_CONST 42", "RET"])
    reject("literal_delete_in_string", ['# source_debug 0 "p\x7fq" 0', "PUSH_CONST 42", "RET"])
    reject("path_over_capacity", [debug(0, "p" * 16385), "PUSH_CONST 42", "RET"], error="manifest_limit")
    reject("scope_over_capacity", [debug(0, bindings=(("value", "slot", "i64", 1025),)), "PUSH_CONST 42", "RET"], error="manifest_limit")
    reject("count_over_candidate_capacity_without_bindings", ['# source_debug 0 "p" 129', "PUSH_CONST 42", "RET"], error="manifest_limit")
    reject("duplicate_pending", [debug(0), debug(0), "PUSH_CONST 42", "RET"])
    reject("orphan_at_function_end", ["PUSH_CONST 42", "RET", debug(2)])
    reject("orphan_at_eof", ["PUSH_CONST 42", "RET", debug(2)], transform=lambda data: data.rsplit(b'end function', 1)[0])
    reject("outside_function_before", ["PUSH_CONST 42", "RET"], transform=lambda data: data.replace(b'module raw_enum_conformance\n', b'module raw_enum_conformance\n# source_debug 0 "p" 0\n'))
    reject("outside_function_after", ["PUSH_CONST 42", "RET"], transform=lambda data: data + b'# source_debug 0 "p" 0\n')
    reject("repeated_index_after_instruction", [debug(0), "PUSH_CONST 42", debug(0), "RET"])
    reject("dead_wrong_index", ["PUSH_CONST 42", "RET", debug(99), "PUSH_CONST 1"])
    bad_helper = flow.function("unused", [debug(1), "PUSH_CONST 1", "RET"])
    reject("unused_helper_wrong_index", ["PUSH_CONST 42", "RET"], helpers=(bad_helper,))
    reject("arbitrary_comment_still_unsupported", [debug(0), "# ignored comment", "PUSH_CONST 42", "RET"], error="manifest_unsupported")
    reject("lookalike_marker_still_unsupported", ['# source_debugging 0 "p" 0', "PUSH_CONST 42", "RET"], error="manifest_unsupported")
    reject("metadata_does_not_change_operand_type", [debug(0, bindings=(("value", "value", "i64", 0),)), "PUSH_CONST_STR S0", "PUSH_CONST 2", "SUB i64", "RET"], strings=("42",))
    reject("metadata_does_not_grant_console", [debug(0, bindings=(("console", "console", "authority", 0),)), "PUSH_CONST 42", "CONSOLE_WRITELN", "PUSH_CONST 42", "RET"], error="manifest_ceiling")
    reject("dead_console_still_needs_authority", ["PUSH_CONST 42", "RET", debug(2), "CONSOLE_WRITELN"], error="manifest_ceiling")
    reject("foreign_opcode_still_unsupported", [debug(0), "PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"], error="manifest_unsupported")
    reject("instruction_capacity_still_bounded", [*full[:-2], "LABEL extra", *full[-2:]], error="manifest_limit")
    require(len(cases) == len({case["name"] for case in cases}), "duplicate boundary case")
    return cases


def compare_case(case, candidate, reference, production):
    status = "validation_refused" if case["validation"] else "execution_refused" if case["error"] else "returned"
    require(candidate["status"] == status, "raw outcome differs from expectation")
    for field in ("result", "error", "console_calls", "instructions", "source_debug"):
        require(candidate[field] == case[field], f"raw {field} differs from expectation")
    require(candidate["output_hex"] == case["output"].hex() and candidate["output_bytes"] == len(case["output"]), "raw output differs")
    if case["production_error"]:
        require(production["status"] == "admission_refused" and production["error"] == case["production_error"] and production["source_debug"] == [],
                "production admission contract changed")
    else:
        require(production == candidate, "production and raw dispatch/metadata differ")
    if case["validation"] and not case["metadata_policy"]:
        require(reference is None, "validation-only fixture ran reference")
        return "expected_validation_refusal"
    require(reference is not None and reference["status"] == ("execution_refused" if case["reference_error"] else "returned"), "finite reference status differs")
    require(reference["error"] == case["reference_error"], "finite reference error differs")
    result = case["reference_result"] if case["policy"] or case["metadata_policy"] else case["result"]
    output = case["reference_output"] if case["reference_output"] is not None else case["output"]
    require(reference["result"] == result and reference["output_hex"] == output.hex(), "finite reference semantics differ")
    require(reference["source_debug"] == case["reference_source_debug"], "reference metadata differs from independent fixture")
    if case["metadata_policy"]:
        return "finite_metadata_policy_contrast"
    return "finite_runtime_policy_contrast" if case["policy"] else "matching_runtime_refusal" if case["error"] else "matching_semantics"


def boundary_proof(args, output, environment):
    directory = output / "boundary-editions"
    directory.mkdir()
    results, failures = [], []
    for number, case in enumerate(generated_cases()):
        path = directory / f"{number:03d}-{case['name']}.nil"
        path.write_bytes(case["edition"])
        record = {key: value for key, value in case.items() if key not in {"edition", "output", "reference_output"}}
        record.update(edition_bytes=len(case["edition"]), edition_sha256=digest(case["edition"]), retained_path=str(path.relative_to(output)),
                      expected_output_hex=case["output"].hex(), reference_output_hex=None if case["reference_output"] is None else case["reference_output"].hex())
        try:
            values = {}
            for name, tool, parser in (("candidate", args.candidate, parse_probe), ("production", args.production_candidate, parse_production)):
                prefix = "raw" if name == "candidate" else "production"
                process, stdout, _ = baseline.run([tool, path, str(case["ceiling"]), str(case["steps"]), str(case["frames"])], output / f"boundary-{prefix}-{number:03d}", environment=environment)
                values[name] = parser(process, stdout)
            reference = None
            if not case["validation"] or case["metadata_policy"]:
                process, stdout, _ = baseline.run([args.reference, path], output / f"boundary-reference-{number:03d}", environment=environment)
                reference = parse_probe(process, stdout, reference=True)
            record.update(values, reference=reference)
            record["outcome"] = compare_case(case, values["candidate"], reference, values["production"])
            if case["cross_profile"]:
                for name, tool, parser, status in (("enum", args.enum_candidate, flow.parse_probe, "admission_refused"),
                                                   ("enum_raw", args.enum_conformance_candidate, conformance.parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])], output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_syntax" if case["name"] == "policy_c1_unicode_control" else "manifest_intent" if not case["annotated"] and name == "enum" else "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier enum grammar expanded")
                    record[name] = previous
        except (AssertionError, ValueError, OSError) as error:
            record.update(outcome="failed", failure=str(error)); failures.append(case["name"])
        results.append(record)
    write_json(output / "boundary-results.json", {"cases": results, "failures": failures})
    return {"passed": not failures, "cases": len(results), "failures": failures,
            "semantic_matches": sum(item["outcome"] == "matching_semantics" for item in results),
            "matching_runtime_refusals": sum(item["outcome"] == "matching_runtime_refusal" for item in results),
            "finite_runtime_policy_contrasts": sum(item["outcome"] == "finite_runtime_policy_contrast" for item in results),
            "finite_metadata_policy_contrasts": sum(item["outcome"] == "finite_metadata_policy_contrast" for item in results),
            "expected_validation_refusals": sum(item["outcome"] == "expected_validation_refusal" for item in results),
            "production_comparisons": sum("production" in item for item in results),
            "metadata_reference_comparisons": sum(item.get("reference") is not None for item in results),
            "enum_refusals": sum("enum" in item for item in results), "enum_conformance_refusals": sum("enum_raw" in item for item in results)}
def instruction_inventory(data):
    """Reserved metadata is lexical inventory, never proof of validity or types."""
    result = enums_proof.instruction_inventory(data)
    result.update(profile=10, lane="raw_source_debug_conformance",
                  scope="profile-10 lexical coverage only; metadata placement, decoded strings, indices, authority and CFG require complete validation")
    if result["unsupported_format"]:
        return result
    metadata_lines = {number for number, line in enumerate(data.decode("utf-8").splitlines(), 1)
                      if re.match(r"\s*#\s+source_debug(?:\s|$)", line)}
    result["instructions"] = [item for item in result["instructions"] if item["line"] not in metadata_lines]
    result["source_debug_records"] = len(metadata_lines)
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
    if matched != expected_raw_matches:
        failures.append("known raw semantic coverage changed")
    for record in records:
        number = record["invocation"]
        expected = known_raw_matches.get(number)
        if expected is not None and (record.get("edition_bytes"), record.get("edition_sha256")) != expected:
            failures.append(f"known complete edition changed: {number}")
        if record.get("outcome") != ("returned_match" if number in expected_raw_matches else "validation_refused"):
            failures.append(f"known corpus outcome changed: {number}")
        if number in known_raw_matches and record.get("candidate", {}).get("source_debug") != []:
            failures.append(f"non-debug complete edition gained metadata: {number}")
        if number == 4 and record.get("source_debug_verified") is not True:
            failures.append("exact temporary-path metadata target was not verified")
    errors = dict(Counter(item["candidate"]["error"] for item in records if item.get("outcome") == "validation_refused"))
    expected_errors = {"manifest_unsupported": 20, "manifest_edition": 9}
    if errors != expected_errors:
        failures.append("raw validation refusal inventory changed")
    return {"passed": not failures, "failures": failures, "matched_invocations": matched,
            "expected_matched_invocations": expected_raw_matches, "expected_refusal_errors": expected_errors,
            "edition_pins": [{"invocation": item["invocation"], "bytes": item.get("edition_bytes"), "sha256": item.get("edition_sha256")}
                             for item in records if item["invocation"] in expected_raw_matches],
            "temporary_source_paths": "Invocation4 is pinned to this exact capture; no path normalization or cross-run fixed hash."}


def target_metadata(path):
    expected = []
    for index in range(1, 7):
        expected.append(snapshot("debug_check__Reading__doubled", index, path,
                                 (("self", "self", "debug_check::Reading", 0),)))
    for index in range(4, 7):
        expected.append(snapshot("debug_check__identity", index, path,
                                 (("input_value", "input_value", "i64", 0),)))
    for index in range(14):
        expected.append(snapshot("main", index, path,
                                 (("sample", "sample", "debug_check::Reading", 1),) if index >= 6 else ()))
    return expected


def compare_target_receipt(target, compiler, reference):
    require(compiler["kind"] == "compiler" and compiler["index"] == 53 and target["upstream_call"] == 54,
            "source-debug compiler capture binding changed")
    require(len(compiler["inputs"]) == 1 and compiler["inputs"][0]["sha256"] ==
            "5c9fe00aa4196e8728fe4df2c551b3a30d6653ae4415cea9f0771eab13090360", "source-debug Matrix source changed")
    source_path = compiler["inputs"][0]["path"]
    require(source_path.endswith("/source-debug.matrix"), "source-debug source path shape changed")
    artifact = compiler["artifact"]
    require(artifact is not None and artifact["sha256"] == target["edition_sha256"] and artifact["bytes"] == target["edition_bytes"],
            "source-debug compiler/interpreter artifact differs")
    expected = target_metadata(source_path)
    candidate = target["candidate"]
    require(candidate["status"] == "returned" and candidate["result"] == 0 and candidate["error"] is None and
            candidate["output_hex"] == b"14\n".hex(), "complete debug candidate full-result semantics differ")
    require(candidate["instructions"] == 28 and candidate["console_calls"] == 1 and candidate["output_bytes"] == 3,
            "complete debug candidate accounting differs")
    require(candidate["source_debug"] == expected, "complete corpus source-debug map differs")
    require(reference["status"] == "returned" and reference["result"] == 0 and reference["error"] is None and
            reference["output_hex"] == b"14\n".hex(), "complete debug reference semantics differ")
    require(reference["source_debug"] == expected, "independent reference debug snapshot differs")
    return source_path, expected


def verify_target_metadata(args, output, environment, records):
    target = records[4]
    calls = baseline.strict_json((output / "corpus-calls.json").read_text())
    compiler = calls[53]
    path = output / target["retained_path"]
    data = path.read_bytes()
    require(digest(data) == target["edition_sha256"] and len(data) == target["edition_bytes"], "exact debug edition changed")
    process, stdout, _ = baseline.run([args.reference, path], output / "corpus-metadata-reference-004", environment=environment)
    reference = parse_probe(process, stdout, reference=True)
    source_path, expected = compare_target_receipt(target, compiler, reference)
    receipt = {"invocation": 4, "upstream_call": 54, "source_path": source_path,
               "source_sha256": compiler["inputs"][0]["sha256"], "edition_bytes": len(data),
               "edition_sha256": digest(data), "retained_path": target["retained_path"],
               "metadata_records": 23, "metadata_functions": 3, "candidate": target["candidate"], "reference": reference,
               "source_debug": expected, "paths_normalized": False, "passed": True,
               "result_comparison": "full_signed_i64_zero_against_independent_reference"}
    write_json(output / "source-debug-target.json", receipt)
    target["source_debug_verified"] = True
    return receipt


def qualify_corpus(args, output, environment, before):
    with corpus_adapters():
        raw = baseline.corpus_proof(args, output, environment, before)
    records = baseline.strict_json((output / "corpus-executions.json").read_text())
    target_receipt = verify_target_metadata(args, output, environment, records)
    for record in records:
        record["lane"] = "raw_source_debug_conformance"
    raw.update(lane="raw_source_debug_conformance",
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
        item["lane"] = "production_profile10_admission"
        try:
            path = output / record["retained_path"]
            data = path.read_bytes()
            require(len(data) == record["edition_bytes"] and digest(data) == record["edition_sha256"], "corpus bytes changed between lanes")
            process, stdout, _ = baseline.run([args.production_candidate, path, "1"], output / f"corpus-production-{number:03d}",
                                              environment=environment)
            candidate = parse_production(process, stdout)
            item["candidate"] = candidate
            require(candidate["status"] == "admission_refused", "unmodified corpus unexpectedly passed production admission")
            item["outcome"] = "production_refused"
            if number == 4:
                pair = dict(item, lane="prior_enum_grammar")
                for name, tool, parser, status in (("enum", args.enum_candidate, flow.parse_probe, "admission_refused"),
                                                   ("enum_raw", args.enum_conformance_candidate, conformance.parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] ==
                            "manifest_unsupported", "earlier corpus enum contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    # All original editions still lack main intent. Keep the complete measured
    # first-error inventory pinned; new metadata parsing can change error order
    # in helpers before main, so the initial development run must verify it.
    expected = {"manifest_unsupported": 17, "manifest_edition": 9, "manifest_intent": 19}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 1
    production = {"lane": "production_profile10_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "enum_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "enum-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "enum_candidate", "enum_conformance_candidate", "reference", "compiler", "interpreter")
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
    tools["primary_sources"] = []
    for name, expected in primary_pins.items():
        record = baseline.record_tool(args.root / name)
        require(record["sha256"] == expected, f"primary source changed: {name}")
        tools["primary_sources"].append(record)
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_source_debug_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_source_debug host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 10 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536, "array_descriptors": 128, "array_elements": 1024, "array_literal_elements": 256, "record_layouts": 64, "fields_per_layout": 32, "declared_fields": 256, "record_descriptors": 128, "record_cells": 1024, "global_slots": 128, "global_name_bytes": 128, "integer_text_maximum_bytes": 20, "enum_layouts": 64, "variants_per_enum": 32, "declared_variants": 256, "enum_tag_bytes": 128, "enum_payload_values": 1, "source_debug_records": 4096, "debug_path_bytes": 16384, "debug_binding_bytes": 128, "debug_bindings": 128, "debug_scope_depth": 1024},
        "earlier_proofs": "126-, 77-, 61-, 27-, 55-, 87-, 103-, 115-, 105- and 149-case proofs remain separate required runs",
    })
    environment = baseline.clean_environment()
    boundary = boundary_proof(args, args.output, environment)
    raw, production = qualify_corpus(args, args.output, environment, before)
    require(baseline.inventory(args.root) == before, "upstream inputs changed during proof")
    for name in binary_names:
        require(baseline.record_tool(getattr(args, name)) == tools[name], f"tool changed: {name}")
    for record in [*tools["reference_core_archives"], *tools["earlier_profile_sources"], *tools["primary_sources"], tools["reference_source"], tools["script"]]:
        require(baseline.record_tool(Path(record["path"])) == record, "proof source/archive changed")
    for number, case in enumerate(generated_cases()):
        path = args.output / "boundary-editions" / f"{number:03d}-{case['name']}.nil"
        require(path.read_bytes() == case["edition"], "boundary input changed during proof")
    passed = baseline.checkpoint_passed(boundary, raw) and raw["regression_contract"]["passed"] and production["passed"]
    result = {"schema": 1, "status": "bounded_source_debug_checkpoint_passed" if passed else "bounded_source_debug_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 10, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_has_explicit_counts_and_prior_source_pins(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 111)
        self.assertEqual(len({case["name"] for case in cases}), 111)
        self.assertEqual(sum(not case["error"] for case in cases), 37)
        self.assertEqual(sum(case["policy"] for case in cases), 1)
        self.assertEqual(sum(case["metadata_policy"] for case in cases), 14)
        self.assertEqual(sum(case["validation"] and not case["metadata_policy"] for case in cases), 58)
        self.assertEqual(sum(bool(case["error"]) and not case["policy"] and not case["validation"] for case in cases), 1)
        self.assertEqual(sum(case["cross_profile"] for case in cases), 52)
        self.assertEqual(sum(not case["validation"] or case["metadata_policy"] for case in cases), 53)
        self.assertTrue(all(type(case["instructions"]) is int and len(case["edition"]) <= 65536 for case in cases))
        self.assertTrue(all(case["origin"] == "independently_authored_textual_nil" for case in cases))
        self.assertEqual(len(source_pins), 19)
        self.assertEqual(len(expected_raw_matches), 16)

    def test_snapshot_parser_rejects_missing_noncanonical_and_duplicate_fields(self):
        process = {"exit_code": 0, "timed_out": False}
        value = {"status": "returned", "result": 42, "error": None, "output_hex": "", "instructions": 2,
                 "console_calls": 0, "output_bytes": 0, "source_debug": [snapshot("main", 0, "path")]}
        self.assertEqual(parse_probe(process, json.dumps(value).encode()), value)
        mutations = []
        absent = copy.deepcopy(value); del absent["source_debug"]; mutations.append(absent)
        bad = copy.deepcopy(value); bad["source_debug"][0]["path_hex"] = "CA"; mutations.append(bad)
        bad = copy.deepcopy(value); bad["source_debug"][0]["path_hex"] = "ff"; mutations.append(bad)
        bad = copy.deepcopy(value); bad["source_debug"][0]["path_hex"] = "00"; mutations.append(bad)
        bad = copy.deepcopy(value); bad["source_debug"][0]["path_hex"] = "c285"; mutations.append(bad)
        bad = copy.deepcopy(value); bad["source_debug"][0]["instruction_index"] = True; mutations.append(bad)
        bad = copy.deepcopy(value); bad["source_debug"].append(bad["source_debug"][0]); mutations.append(bad)
        bad = copy.deepcopy(value); bad["source_debug"][0]["extra"] = 1; mutations.append(bad)
        bad = copy.deepcopy(value); bad.update(status="validation_refused", result=None, error="manifest_syntax", instructions=0); mutations.append(bad)
        for changed in mutations:
            with self.assertRaises((AssertionError, UnicodeDecodeError)):
                parse_probe(process, json.dumps(changed).encode())

    def test_snapshot_caps_keep_reference_policy_separate(self):
        metadata = [snapshot("main", 0, "path", (("name", "slot", "x" * 129, 0),))]
        with self.assertRaises(AssertionError):
            validate_snapshot(metadata)
        self.assertEqual(validate_snapshot(metadata, reference=True), metadata)
        for field, value in (("scope_depth", True), ("scope_depth", 1025), ("type_hex", "")):
            changed = copy.deepcopy(metadata); changed[0]["bindings"][0][field] = value
            with self.assertRaises(AssertionError):
                validate_snapshot(changed, reference=True)
        changed = copy.deepcopy(metadata); changed[0]["bindings"].append(changed[0]["bindings"][0])
        with self.assertRaises(AssertionError):
            validate_snapshot(changed, reference=True)

    def test_comparison_checks_exact_metadata_and_execution(self):
        case = next(case for case in generated_cases() if case["name"] == "first_instruction")
        candidate = {"status": "returned", "result": 42, "error": None, "output_hex": "", "instructions": 2,
                     "console_calls": 0, "output_bytes": 0, "source_debug": case["source_debug"]}
        reference = {"schema": 1, "status": "returned", "result": 42, "error": None, "output_hex": "", "source_debug": case["source_debug"]}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("source_debug", []), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        changed = copy.deepcopy(reference); changed["source_debug"][0]["path_hex"] = "78"
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, changed, candidate)

    def test_metadata_policy_is_not_a_matching_runtime_fault(self):
        case = next(case for case in generated_cases() if case["name"] == "policy_unknown_escape_q")
        candidate = {"status": "validation_refused", "result": None, "error": "manifest_unsupported", "output_hex": "",
                     "instructions": 0, "console_calls": 0, "output_bytes": 0, "source_debug": []}
        production = dict(candidate, status="admission_refused")
        reference = {"schema": 1, "status": "returned", "result": 42, "error": None, "output_hex": "", "source_debug": case["reference_source_debug"]}
        self.assertEqual(compare_case(case, candidate, reference, production), "finite_metadata_policy_contrast")
        for changed in (dict(reference, result=0), dict(reference, source_debug=[]), dict(reference, status="execution_refused", error="timeout", result=None)):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, production)

    def test_lexical_inventory_is_not_metadata_validation(self):
        for case in generated_cases():
            if not case["validation"]:
                inventory = instruction_inventory(case["edition"])
                self.assertEqual(inventory["profile"], 10)
                self.assertIn("lexical", inventory["scope"])
                self.assertTrue(all(item["supported_form"] for item in inventory["instructions"]), case["name"])
                self.assertEqual(inventory["unsupported_declarations"], [], case["name"])
        case = next(case for case in generated_cases() if case["name"] == "reject_wrong_first_index")
        self.assertEqual(instruction_inventory(case["edition"])["source_debug_records"], 1)
        self.assertTrue(all(item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))
        previous = baseline.parse_probe, baseline.instruction_inventory
        with self.assertRaises(RuntimeError):
            with corpus_adapters():
                raise RuntimeError("deliberate")
        self.assertEqual(previous, (baseline.parse_probe, baseline.instruction_inventory))

    def test_corpus_contract_and_exact_per_capture_metadata_target(self):
        records, refused = [], 0
        for number in range(45):
            if number in expected_raw_matches:
                size, identity = known_raw_matches.get(number, (3511, "a" * 64))
                records.append({"invocation": number, "matched": True, "outcome": "returned_match", "edition_bytes": size,
                                "edition_sha256": identity, "candidate": {"error": None, "source_debug": []}, "source_debug_verified": number == 4})
            else:
                error = "manifest_unsupported" if refused < 20 else "manifest_edition"; refused += 1
                records.append({"invocation": number, "matched": False, "outcome": "validation_refused", "candidate": {"error": error}})
        self.assertTrue(raw_regression_contract(records)["passed"])
        for number in expected_raw_matches:
            changed = copy.deepcopy(records); changed[number].update(matched=False, outcome="validation_refused", candidate={"error": "manifest_unsupported"})
            self.assertFalse(raw_regression_contract(changed)["passed"])
        for number in known_raw_matches:
            changed = copy.deepcopy(records); changed[number]["edition_sha256"] = "0" * 64
            self.assertFalse(raw_regression_contract(changed)["passed"])
        changed = copy.deepcopy(records); changed[4]["source_debug_verified"] = False
        self.assertFalse(raw_regression_contract(changed)["passed"])
        first, second = target_metadata("/tmp/original/source-debug.matrix"), target_metadata("/tmp/new/source-debug.matrix")
        self.assertEqual(len(first), 23)
        self.assertNotEqual(first, second)
        self.assertEqual([entry["instruction_index"] for entry in first if entry["function"] == "main"], list(range(14)))
        self.assertFalse(raw_regression_contract(records[:-1])["passed"])

    def test_target_receipt_rejects_low_byte_alias_and_capture_mutations(self):
        path = "/tmp/exact-capture/source-debug.matrix"
        metadata = target_metadata(path)
        candidate = {"status": "returned", "result": 0, "error": None, "output_hex": "31340a",
                     "instructions": 28, "console_calls": 1, "output_bytes": 3, "source_debug": metadata}
        reference = {"schema": 1, "status": "returned", "result": 0, "error": None, "output_hex": "31340a", "source_debug": metadata}
        target = {"upstream_call": 54, "edition_bytes": 3511, "edition_sha256": "a" * 64, "candidate": candidate}
        compiler = {"kind": "compiler", "index": 53, "artifact": {"bytes": 3511, "sha256": "a" * 64},
                    "inputs": [{"path": path, "sha256": "5c9fe00aa4196e8728fe4df2c551b3a30d6653ae4415cea9f0771eab13090360"}]}
        self.assertEqual(compare_target_receipt(target, compiler, reference), (path, metadata))
        for field, value in (("result", 256), ("instructions", 27), ("source_debug", []), ("output_hex", "31340a00")):
            changed = copy.deepcopy(target); changed["candidate"][field] = value
            with self.assertRaises(AssertionError):
                compare_target_receipt(changed, compiler, reference)
        changed = copy.deepcopy(compiler); changed["inputs"][0]["path"] = "/tmp/rewritten/source-debug.matrix"
        with self.assertRaises(AssertionError):
            compare_target_receipt(target, changed, reference)
        for field, value in (("bytes", 3512), ("sha256", "0" * 64)):
            changed = copy.deepcopy(compiler); changed["artifact"][field] = value
            with self.assertRaises(AssertionError):
                compare_target_receipt(target, changed, reference)
        with self.assertRaises(AssertionError):
            compare_target_receipt(target, compiler, dict(reference, result=256))

    def test_metadata_cannot_supply_intent_and_old_sources_stay_pinned(self):
        case = next(case for case in generated_cases() if case["name"] == "metadata_does_not_supply_main_intent")
        self.assertEqual(case["production_error"], "manifest_intent")
        self.assertNotIn(b'@ai_context("Exercise', case["edition"])
        self.assertEqual(case["source_debug"][0]["bindings"][0]["name_hex"], b"@ai_context".hex())
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)
        environment = baseline.clean_environment()
        self.assertNotIn("NIL_STRICT_BOUNDS", environment)
        self.assertEqual(environment["NIL_CRASH_REPORT"], "0")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "enum-candidate", "enum-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "enum_candidate", "enum_conformance_candidate", "reference", "compiler", "root", "output"):
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
