#!/usr/bin/env python3
"""Qualify bounded profile-9 nominal unit/integer enums without changing earlier profiles.

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
import program_global_proof as globals_proof
import program_conformance_proof as conformance


require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe = conformance.parse_probe
source_pins = dict(globals_proof.source_pins, **{
    "program_global_proof.py": "2877a2ec56e4c84112775614a2d0e336e416d690edced1f1e64e1fa6fdcc4a01",
    "program_global_reference.cpp": "503a226e0c60218ed93cba383a0d8aef8a99dd26b9261919c8f2c3a0a7b6cd3b",
})
archive_pins = globals_proof.archive_pins
known_raw_matches = dict(sorted({**globals_proof.known_raw_matches,
    30: (3769, "5fed5a7122c09ad2d597072efdf3ac856ca8372019015b68130df8be9465d385"),
    44: (4057, "bac98e0cba04770cab9f67dd290f9180d48c08da37f94c1d5b683d4cab9ad276"),
}.items()))


def authored_enum_edition(functions, enums, strings=(), annotations=None, *, layouts=(), section=True):
    """Independent textual fixtures, never transformed compiler outputs."""
    value = "# NIL Textual Module\nmodule raw_enum_conformance\n\n"
    if strings:
        value += "# String constants\n" + "".join(f'const S{index} = "{text}"\n' for index, text in enumerate(strings)) + "\n"
    if layouts:
        value += "# Struct definitions\n"
        for name, fields, contexts in layouts:
            value += "".join(context + "\n" for context in contexts) + f"struct {name}\n"
            value += "".join(f"    {field}: {kind}\n" for field, kind in fields) + "end struct\n\n"
    if enums:
        if section:
            value += "# Enum definitions\n"
        for name, variants, contexts in enums:
            value += "".join(context + "\n" for context in contexts) + f"enum {name}\n"
            value += "".join(f"    {variant}" + (f": {kind}" if kind is not None else "") + "\n" for variant, kind in variants)
            value += "end enum\n\n"
    for number, (name, parameters, result, body) in enumerate(functions, 1):
        value += "".join(context + "\n" for context in (annotations or {}).get(name, ()))
        value += f"# declaration_line {number}\nfunction {name}"
        if parameters:
            value += "(" + ", ".join(f"{name}: {kind}" for name, kind in parameters) + ")"
        value += f" -> {result}:\n" + "".join(f"    {instruction}\n" for instruction in body) + "end function\n\n"
    return value.encode()


def enum_layout(name="demo::choice", variants=(("Idle", None), ("Busy", "i64")), contexts=()):
    return name, tuple(variants), tuple(contexts)


def generated_cases():
    cases = []
    annotation = '@ai_context("Exercise bounded nominal unit and integer enums")'
    choice = enum_layout()
    other = enum_layout("demo::other")
    idle = "NEW_VARIANT demo::choice::Idle 0 demo::choice void"
    busy = "NEW_VARIANT demo::choice::Busy 1 demo::choice i64"
    is_idle = "IS_VARIANT demo::choice::Idle demo::choice void"
    is_busy = "IS_VARIANT demo::choice::Busy demo::choice i64"
    unwrap = "UNWRAP_VARIANT 1 demo::choice i64"
    drop_unit = "UNWRAP_VARIANT 0 demo::choice void"

    def add(name, body, *, enums=(choice,), layouts=(), helpers=(), strings=(), result=42, output=b"", calls=0,
            instructions=None, error=None, validation=False, policy=False, reference_result=None,
            reference_output=None, reference_error=None, ceiling=1, steps=65536, frames=16,
            annotated=True, production_error=None, cross_profile=True, section=True, extra_annotations=None):
        functions = [flow.function("main", body), *helpers]
        annotations = dict(extra_annotations or {})
        if annotated:
            annotations["main"] = (annotation,)
        edition = authored_enum_edition(functions, enums, strings, annotations, layouts=layouts, section=section)
        require(len(edition) <= 65536, f"boundary fixture exceeds edition bound: {name}")
        cases.append({"name": name, "edition": edition, "result": None if error else result,
                      "error": error, "output": output, "console_calls": calls,
                      "instructions": 0 if validation else len(body) if instructions is None else instructions,
                      "validation": validation, "policy": policy,
                      "reference_result": reference_result, "reference_output": reference_output,
                      "reference_error": reference_error, "ceiling": ceiling, "steps": steps, "frames": frames,
                      "annotated": annotated, "production_error": production_error or
                      (error if validation and annotated else "manifest_intent" if not annotated else None),
                      "cross_profile": cross_profile, "origin": "independently_authored_textual_nil"})

    def reject(name, body, *, error="manifest_syntax", **kwargs):
        add("reject_" + name, body, error=error, validation=True, cross_profile=False, **kwargs)

    def loop(count, body):
        return ["PUSH_CONST 0", "STORE_LOCAL index", "LABEL loop", "LOAD_LOCAL index", f"PUSH_CONST {count}", "CMP_LT",
                "JMP_IF_NOT done", *body, "LOAD_LOCAL index", "PUSH_CONST 1", "ADD", "STORE_LOCAL index", "JMP loop", "LABEL done"]

    add("unused_enum_declaration", ["PUSH_CONST 42", "RET"], ceiling=0)
    add("empty_enum_declaration", ["PUSH_CONST 42", "RET"], enums=(enum_layout("empty", ()),), ceiling=0)
    add("unit_matching_tag", [idle, is_idle, "RET"], result=1, ceiling=0)
    add("unit_different_tag", [idle, is_busy, "RET"], result=0, ceiling=0)
    add("payload_matching_tag", ["PUSH_CONST 42", busy, is_busy, "RET"], result=1, ceiling=0)
    add("payload_different_tag", ["PUSH_CONST 42", busy, is_idle, "RET"], result=0, ceiling=0)
    add("tag_test_consumes_owner", ["PUSH_CONST 42", idle, is_busy, "POP", "RET"], ceiling=0)
    add("unit_unwrap_consumes_owner", ["PUSH_CONST 42", idle, drop_unit, "RET"], ceiling=0)
    add("payload_unwrap", ["PUSH_CONST 42", busy, unwrap, "RET"], ceiling=0)
    for name, value in (("zero", 0), ("negative", -1), ("minimum", -(1 << 63)), ("maximum", (1 << 63) - 1),
                        ("exact_above_53_bits", 9007199254740993), ("above_32_bits", 4294967297), ("below_32_bits", -4294967297)):
        add("payload_i64_" + name, [f"PUSH_CONST {value}", busy, unwrap, "RET"], result=value, ceiling=0)
    add("payload_alias_survives_tag_consumption", ["PUSH_CONST 42", busy, "STORE_LOCAL held", "LOAD_LOCAL held", is_busy,
        "CONSOLE_WRITELN", "LOAD_LOCAL held", unwrap, "RET"], output=b"1\n", calls=1)
    add("distinct_same_payload_tags", ["PUSH_CONST 42", busy, "IS_VARIANT demo::choice::Other demo::choice i64", "RET"],
        enums=(enum_layout(variants=(("Idle", None), ("Busy", "i64"), ("Other", "i64"))),), result=0, ceiling=0)
    for condition in (0, 1):
        add(f"same_enum_join_{condition}", [f"PUSH_CONST {condition}", "JMP_IF_NOT unit", "PUSH_CONST 42", busy, "JMP joined",
            "LABEL unit", idle, "LABEL joined", "STORE_LOCAL chosen", "LOAD_LOCAL chosen", is_busy, "CONSOLE_WRITELN", "PUSH_CONST 42", "RET"],
            output=f"{condition}\n".encode(), calls=1, instructions=12 if condition else 11)
    add("same_enum_loop_reassignment", [idle, "STORE_LOCAL chosen"] + loop(4, ["LOAD_LOCAL index", busy, "STORE_LOCAL chosen"]) +
        ["LOAD_LOCAL chosen", unwrap, "RET"], result=3, instructions=65, ceiling=0)
    add("inline_values_have_no_cumulative_object_arena", loop(300, [idle, "POP"]) + ["PUSH_CONST 42", "RET"], instructions=3610, ceiling=0)
    maker = flow.function("make", ["PUSH_CONST 42", busy, "RET"], result="demo::choice")
    make_unit = flow.function("make_unit", [idle, "RET"], result="demo::choice")
    extract = flow.function("extract", ["STORE_LOCAL choice", "LOAD_LOCAL choice", unwrap, "RET"], parameters=(("choice", "demo::choice"),))
    add("returned_enum_lifetime", ["CALL make 0", "STORE_LOCAL held", "LOAD_LOCAL held", unwrap, "RET"], helpers=(maker,), instructions=8, ceiling=0)
    add("direct_enum_parameter", ["PUSH_CONST 42", busy, "CALL extract 1", "RET"], helpers=(extract,), instructions=8, ceiling=0)
    consume = flow.function("consume", ["STORE_LOCAL choice", "LOAD_LOCAL choice", drop_unit, "RET"], parameters=(("choice", "demo::choice"),), result="void")
    add("unit_enum_void_helper", [idle, "CALL consume 1", "PUSH_CONST 42", "RET"], helpers=(consume,), instructions=8, ceiling=0)
    subtract = flow.function("subtract", ["STORE_LOCAL second", "STORE_LOCAL first", "LOAD_LOCAL first", unwrap, "LOAD_LOCAL second", unwrap, "SUB i64", "RET"],
        parameters=(("first", "demo::choice"), ("second", "demo::choice")))
    add("enum_parameter_order", ["PUSH_CONST 50", busy, "PUSH_CONST 8", busy, "CALL subtract 2", "RET"], helpers=(subtract,), instructions=14, ceiling=0)
    add("indirect_enum_return", ["PUSH_FN_PTR make", "STORE_LOCAL operation", "CALL operation 0", unwrap, "RET"], helpers=(maker,), instructions=8, ceiling=0)
    add("indirect_enum_parameter", ["PUSH_FN_PTR extract", "STORE_LOCAL operation", "PUSH_CONST 42", busy, "CALL operation 1", "RET"], helpers=(extract,), instructions=10, ceiling=0)
    render = flow.function("render", ["STORE_LOCAL choice", "LOAD_LOCAL choice", unwrap, "CAST text", "RET"], parameters=(("choice", "demo::choice"),), result="text")
    add("enum_payload_to_returned_text", ["PUSH_FN_PTR render", "STORE_LOCAL operation", "PUSH_CONST 42", busy, "CALL operation 1", "CONSOLE_WRITE", "PUSH_CONST 42", "RET"],
        helpers=(render,), output=b"42", calls=1, instructions=13)
    add("same_signature_callable_reassignment", ["PUSH_FN_PTR make_unit", "STORE_LOCAL operation", "PUSH_FN_PTR make", "STORE_LOCAL operation", "CALL operation 0", unwrap, "RET"],
        helpers=(maker, make_unit), instructions=10, ceiling=0)
    for condition in (0, 1):
        add(f"same_signature_callable_join_{condition}", [f"PUSH_CONST {condition}", "JMP_IF_NOT unit", "PUSH_FN_PTR make", "STORE_LOCAL operation", "JMP joined",
            "LABEL unit", "PUSH_FN_PTR make_unit", "STORE_LOCAL operation", "LABEL joined", "CALL operation 0", is_busy, "CONSOLE_WRITELN", "PUSH_CONST 42", "RET"],
            helpers=(maker, make_unit), output=f"{condition}\n".encode(), calls=1, instructions=14 if condition else 13)
    add("global_and_enum_names_separate", ["PUSH_CONST 42", "STORE_GLOBAL demo::choice i64", idle, "POP", "LOAD_GLOBAL demo::choice i64", "RET"], ceiling=0)
    add("record_and_enum_values_coexist", ["NEW_STRUCT box", "PUSH_CONST 42", "SET_FIELD value", idle, is_idle, "POP", "GET_FIELD value", "RET"],
        layouts=(records.layout("box", ("value",)),), ceiling=0)
    add("shadowed_console_is_not_authority", ["PUSH_FN_PTR make", "STORE_LOCAL println", "CALL println 0", unwrap, "RET"], helpers=(maker,), instructions=8, ceiling=0)
    add("empty_enum_at_name_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout("e" * 128, ()),), ceiling=0)
    for name, layout_name, tag in (("layout", "e" * 125, "V"), ("variant", "E", "V" * 125)):
        add("full_tag_exact_capacity_" + name, [f"NEW_VARIANT {layout_name}::{tag} 0 {layout_name} void", f"IS_VARIANT {layout_name}::{tag} {layout_name} void", "POP", "PUSH_CONST 42", "RET"],
            enums=(enum_layout(layout_name, ((tag, None),)),), ceiling=0)
    add("enum_layouts_exact_capacity", ["PUSH_CONST 42", "RET"], enums=tuple(enum_layout(f"E{i}", ()) for i in range(64)), ceiling=0)
    add("variants_per_enum_exact_capacity", ["NEW_VARIANT E::V31 0 E void", "IS_VARIANT E::V31 E void", "POP", "PUSH_CONST 42", "RET"],
        enums=(enum_layout("E", tuple((f"V{i}", None) for i in range(32))),), ceiling=0)
    many = tuple(enum_layout(f"E{i}", tuple((f"V{j}", "i64") for j in range(4))) for i in range(64))
    add("highest_enum_and_variant_identity", ["PUSH_CONST 9007199254740993", "NEW_VARIANT E63::V3 1 E63 i64", "UNWRAP_VARIANT 1 E63 i64", "RET"],
        enums=many, result=9007199254740993, ceiling=0)
    add("zero_padded_unit_count", ["NEW_VARIANT demo::choice::Idle 000 demo::choice void", "UNWRAP_VARIANT 000 demo::choice void", "PUSH_CONST 42", "RET"], ceiling=0)
    add("zero_padded_payload_count", ["PUSH_CONST 42", "NEW_VARIANT demo::choice::Busy 0001 demo::choice i64", "UNWRAP_VARIANT 0001 demo::choice i64", "RET"], ceiling=0)
    context = '@ai_context("Describe an application choice")'
    add("enum_context_and_main_context", ["PUSH_CONST 42", busy, unwrap, "RET"], enums=(enum_layout(contexts=(context,)),), ceiling=0)
    add("enum_context_does_not_supply_main_intent", ["PUSH_CONST 42", busy, unwrap, "RET"], enums=(enum_layout(contexts=(context,)),), annotated=False, ceiling=0)
    add("multiple_enum_contexts", ["PUSH_CONST 42", busy, unwrap, "RET"], enums=(enum_layout(contexts=(context,)), enum_layout("demo::other", contexts=(context,))), ceiling=0)
    add("enum_context_exact_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout(contexts=('@ai_context("' + 'x' * 512 + '")',)),), ceiling=0)
    add("dead_enum_ops_keep_syntax_only", ["PUSH_CONST 42", "RET", busy, is_busy, unwrap], instructions=2, ceiling=0)
    add("enum_output_before_division_fault", ["PUSH_CONST 7", busy, unwrap, "CONSOLE_WRITELN", "PUSH_CONST 5", "PUSH_CONST 0", "DIV i64", "RET"],
        error="division_by_zero", output=b"7\n", calls=1, instructions=7, reference_error="Division by zero")
    add("unit_unwrap_does_not_reach_caller_sentinel", ["PUSH_CONST 99", idle, unwrap, "CONSOLE_WRITELN", "RET"],
        error="enum_shape", instructions=3, policy=True, reference_output=b"99\n",
        reference_error="invalid RET stack in function 'main': expected 1 value(s), found 0")
    add("payload_cannot_be_unwrapped_as_unit", ["PUSH_CONST 42", busy, drop_unit, "PUSH_CONST 42", "RET"],
        error="enum_shape", instructions=3, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    add("shape_fault_preserves_prior_output", ["PUSH_CONST 7", "CONSOLE_WRITELN", idle, unwrap, "RET"],
        error="enum_shape", output=b"7\n", calls=1, instructions=4, policy=True, reference_output=b"7\n",
        reference_error="invalid RET stack in function 'main': expected 1 value(s), found 0")
    add("enum_finite_fuel", [idle, is_idle, "POP", "PUSH_CONST 42", "RET"], error="instruction_limit", instructions=1,
        policy=True, steps=1, reference_result=42, reference_output=b"", ceiling=0)

    reject("enum_section_missing", ["PUSH_CONST 42", "RET"], section=False)
    reject("duplicate_enum", ["PUSH_CONST 42", "RET"], enums=(choice, choice))
    reject("duplicate_variant", ["PUSH_CONST 42", "RET"], enums=(enum_layout(variants=(("Busy", "i64"), ("Busy", "i64"))),))
    reject("record_enum_nominal_collision", ["PUSH_CONST 42", "RET"], enums=(enum_layout("box"),), layouts=(records.layout("box"),))
    for name in ("i64", "bool", "text", "array", "Option", "option", "OPTION", "Result", "demo::result", "demo::Option"):
        reject("reserved_enum_" + name.replace('::', '_'), ["PUSH_CONST 42", "RET"], enums=(enum_layout(name),), error="manifest_unsupported")
    for suffix, name in (("dotted", "demo.choice"), ("empty_segment", "demo::::choice"), ("leading_separator", "::choice"), ("unicode", "choicé")):
        reject("enum_name_" + suffix, ["PUSH_CONST 42", "RET"], enums=(enum_layout(name),))
    reject("enum_name_over_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout("E" * 129, ()),), error="manifest_limit")
    reject("full_tag_over_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout("E" * 126, (("V", None),)),), error="manifest_limit")
    reject("enum_layouts_over_capacity", ["PUSH_CONST 42", "RET"], enums=tuple(enum_layout(f"E{i}", ()) for i in range(65)), error="manifest_limit")
    reject("variants_per_enum_over_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout("E", tuple((f"V{i}", None) for i in range(33))),), error="manifest_limit")
    reject("total_variants_over_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout("E0", tuple((f"V{i}", "i64") for i in range(5))), *many[1:]), error="manifest_limit")
    for name, kind in (("void_alias", "void"), ("i32", "i32"), ("bool", "bool"), ("text", "text"), ("array", "array<i64>"), ("record", "box"), ("enum", "demo::choice")):
        reject("variant_payload_" + name, ["PUSH_CONST 42", "RET"], enums=(enum_layout(variants=(("Payload", kind),)),), error="manifest_unsupported")
    for name in ("a.b", "a::b", "9value"):
        reject("variant_name_" + name.replace(':', '_').replace('.', '_'), ["PUSH_CONST 42", "RET"], enums=(enum_layout(variants=((name, None),)),),
            error="manifest_unsupported" if "::" in name else "manifest_syntax")
    for name, opcode in (("new_unknown_layout", "NEW_VARIANT missing::Idle 0 missing void"), ("is_unknown_layout", "IS_VARIANT missing::Idle missing void"),
                         ("unwrap_unknown_layout", "UNWRAP_VARIANT 1 missing i64"), ("new_unknown_tag", "NEW_VARIANT demo::choice::Missing 0 demo::choice void"),
                         ("is_unknown_tag", "IS_VARIANT demo::choice::Missing demo::choice void")):
        reject(name + "_dead", ["PUSH_CONST 42", "RET", opcode], error="manifest_unsupported")
    for name, opcode in (("new_nominal_metadata", "NEW_VARIANT demo::choice::Busy 1 demo::other i64"),
                         ("is_nominal_metadata", "IS_VARIANT demo::choice::Busy demo::other i64"),
                         ("new_payload_metadata", "NEW_VARIANT demo::choice::Busy 0 demo::choice void"),
                         ("is_payload_metadata", "IS_VARIANT demo::choice::Idle demo::choice i64"),
                         ("new_count_type", "NEW_VARIANT demo::choice::Busy 0 demo::choice i64"),
                         ("unwrap_count_type", "UNWRAP_VARIANT 0 demo::choice i64")):
        reject(name + "_dead", ["PUSH_CONST 42", "RET", opcode], enums=(choice, other))
    for name, opcode in (("new_dotted_tag", "NEW_VARIANT demo.choice.Idle 0 demo::choice void"), ("is_dotted_tag", "IS_VARIANT demo.choice.Idle demo::choice void"),
                         ("new_negative_count", "NEW_VARIANT demo::choice::Idle -1 demo::choice void"), ("new_plus_count", "NEW_VARIANT demo::choice::Idle +0 demo::choice void"),
                         ("unwrap_hex_count", "UNWRAP_VARIANT 0x1 demo::choice i64")):
        reject(name, ["PUSH_CONST 42", "RET", opcode])
    for name, opcode in (("new_missing_metadata", "NEW_VARIANT demo::choice::Busy 1"), ("new_extra_operand", busy + " extra"),
                         ("is_missing_metadata", "IS_VARIANT demo::choice::Busy"), ("is_extra_operand", is_busy + " extra"),
                         ("unwrap_missing_metadata", "UNWRAP_VARIANT 1"), ("unwrap_extra_operand", unwrap + " extra"),
                         ("new_two_payloads", "NEW_VARIANT demo::choice::Busy 2 demo::choice i64"),
                         ("unwrap_two_payloads", "UNWRAP_VARIANT 2 demo::choice i64"), ("unwrap_text", "UNWRAP_VARIANT 1 demo::choice text")):
        reject(name, ["PUSH_CONST 42", "RET", opcode], error="manifest_unsupported")
    reject("unwrap_absent_payload_shape", ["PUSH_CONST 42", "RET", "UNWRAP_VARIANT 1 E i64"], enums=(enum_layout("E", (("Unit", None),)),), error="manifest_unsupported")
    reject("unwrap_absent_unit_shape", ["PUSH_CONST 42", "RET", "UNWRAP_VARIANT 0 E void"], enums=(enum_layout("E", (("Payload", "i64"),)),), error="manifest_unsupported")
    reject("constructor_underflow", [busy, "POP", "PUSH_CONST 42", "RET"])
    for name, prefix, kwargs in (("text", ["PUSH_CONST_STR S0"], {"strings": ("42",)}), ("array", ["NEW_ARRAY_LITERAL 0 i64"], {}),
                                ("record", ["NEW_STRUCT box"], {"layouts": (records.layout("box"),)}), ("enum", [idle], {}),
                                ("callable", ["PUSH_FN_PTR make"], {"helpers": (maker,)})):
        reject("noninteger_payload_" + name, prefix + [busy, "POP", "PUSH_CONST 42", "RET"], **kwargs)
    for name, prefix, kwargs in (("integer", ["PUSH_CONST 42"], {}), ("text", ["PUSH_CONST_STR S0"], {"strings": ("demo::choice::Idle",)}),
                                ("array", ["NEW_ARRAY_LITERAL 0 i64"], {}), ("record", ["NEW_STRUCT box"], {"layouts": (records.layout("box"),)})):
        reject("forged_tag_test_" + name, prefix + [is_idle, "RET"], **kwargs)
    reject("forged_unwrap_integer", ["PUSH_CONST 42", unwrap, "RET"])
    reject("cross_enum_tag_receiver", [idle, "IS_VARIANT demo::other::Idle demo::other void", "RET"], enums=(choice, other))
    reject("cross_enum_unwrap_receiver", ["PUSH_CONST 42", busy, "UNWRAP_VARIANT 1 demo::other i64", "RET"], enums=(choice, other))
    reject("cross_enum_join", ["PUSH_CONST 1", "JMP_IF_NOT other", idle, "JMP joined", "LABEL other", "NEW_VARIANT demo::other::Idle 0 demo::other void",
        "LABEL joined", "STORE_LOCAL value", "PUSH_CONST 42", "RET"], enums=(choice, other))
    reject("cross_enum_call_argument", ["PUSH_CONST 42", "NEW_VARIANT demo::other::Busy 1 demo::other i64", "CALL extract 1", "RET"], enums=(choice, other), helpers=(extract,))
    wrong_return = flow.function("wrong", ["NEW_VARIANT demo::other::Idle 0 demo::other void", "RET"], result="demo::choice")
    reject("cross_enum_helper_return", ["PUSH_CONST 42", "RET"], enums=(choice, other), helpers=(wrong_return,))
    make_other = flow.function("make_other", ["PUSH_CONST 42", "NEW_VARIANT demo::other::Busy 1 demo::other i64", "RET"], result="demo::other")
    reject("callable_enum_result_identity", ["PUSH_FN_PTR make", "STORE_LOCAL operation", "PUSH_FN_PTR make_other", "STORE_LOCAL operation", "PUSH_CONST 42", "RET"],
        enums=(choice, other), helpers=(maker, make_other))
    extract_other = flow.function("extract_other", ["STORE_LOCAL choice", "LOAD_LOCAL choice", "UNWRAP_VARIANT 1 demo::other i64", "RET"], parameters=(("choice", "demo::other"),))
    reject("callable_enum_parameter_identity", ["PUSH_FN_PTR extract", "STORE_LOCAL operation", "PUSH_FN_PTR extract_other", "STORE_LOCAL operation", "PUSH_CONST 42", "RET"],
        enums=(choice, other), helpers=(extract, extract_other))
    make_record = flow.function("make_record", ["NEW_STRUCT box", "RET"], result="box")
    reject("callable_record_enum_identity", ["PUSH_FN_PTR make", "STORE_LOCAL operation", "PUSH_FN_PTR make_record", "STORE_LOCAL operation", "PUSH_CONST 42", "RET"],
        layouts=(records.layout("box"),), helpers=(maker, make_record))
    for name, body in (("console", [idle, "CONSOLE_WRITE", "PUSH_CONST 42", "RET"]),
                       ("branch", [idle, "JMP_IF_NOT done", "LABEL done", "PUSH_CONST 42", "RET"]),
                       ("arithmetic", [idle, "PUSH_CONST 1", "ADD", "RET"]), ("integer_return", [idle, "RET"]),
                       ("array_element", [idle, "NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 42", "RET"]),
                       ("global_store", [idle, "STORE_GLOBAL value i64", "PUSH_CONST 42", "RET"]),
                       ("record_field", ["NEW_STRUCT box", idle, "SET_FIELD x", "POP", "PUSH_CONST 42", "RET"]),
                       ("integer_cast", [idle, "CAST i64", "RET"]), ("text_cast", [idle, "CAST text", "POP", "PUSH_CONST 42", "RET"]),
                       ("callable", [idle, "STORE_LOCAL operation", "CALL operation 0", "RET"])):
        reject("enum_not_" + name, body, layouts=(records.layout("box"),) if name == "record_field" else ())
    reject("enum_context_empty", ["PUSH_CONST 42", "RET"], enums=(enum_layout(contexts=('@ai_context("")',)),), error="manifest_intent")
    reject("enum_context_duplicate", ["PUSH_CONST 42", "RET"], enums=(enum_layout(contexts=(context, context)),), error="manifest_intent")
    reject("enum_context_over_capacity", ["PUSH_CONST 42", "RET"], enums=(enum_layout(contexts=('@ai_context("' + 'x' * 513 + '")',)),), error="manifest_limit")
    reject("enum_context_invalid_shape", ["PUSH_CONST 42", "RET"], enums=(enum_layout(contexts=('@ai_context(raw)',)),), error="manifest_unsupported")
    noisy = flow.function("unused", [idle, is_idle, "CONSOLE_WRITE", "RET"], result="void")
    reject("unused_helper_console_ceiling", ["PUSH_CONST 42", "RET"], helpers=(noisy,), ceiling=0, error="manifest_ceiling")
    reject("dead_console_ceiling", ["PUSH_CONST 42", "RET", idle, is_idle, "CONSOLE_WRITE"], ceiling=0, error="manifest_ceiling")
    reject("dead_foreign_effect", ["PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"], error="manifest_unsupported")
    require(len({case["name"] for case in cases}) == len(cases), "duplicate enum boundary case")
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
    reference_status = "execution_refused" if case["reference_error"] else "returned"
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
                for name, tool, parser, status in (("global", args.global_candidate, flow.parse_probe, "admission_refused"),
                                                   ("global_raw", args.global_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])],
                        output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier global grammar expanded")
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
            "global_refusals": sum("global" in item for item in records),
            "global_conformance_refusals": sum("global_raw" in item for item in records)}


def instruction_inventory(data):
    """Lexical metadata only; admission checks layouts, exact shapes and CFG."""
    result = globals_proof.instruction_inventory(data)
    result.update(profile=9, lane="raw_enum_conformance",
                  scope="profile-9 lexical coverage only; nominal identities, variant membership, payload shapes, authority and CFG require complete validation")
    if result["unsupported_format"]:
        return result
    identifier = r"[A-Za-z_][A-Za-z_0-9]*"
    nominal = identifier + r"(?:::" + identifier + r")*"
    header = re.compile(r"enum (" + nominal + ")")
    schemas, enum_lines, active = set(), set(), False
    lines = data.decode("utf-8").splitlines()
    for number, line in enumerate(lines, 1):
        line = line.strip()
        if line == "# Enum definitions":
            enum_lines.add(number)
        match = header.fullmatch(line)
        if match:
            schemas.add(match[1])
            enum_lines.add(number)
            active = True
        elif active and line == "end enum":
            enum_lines.add(number)
            active = False
        elif active and re.fullmatch(identifier + r"(?:: i64)?", line):
            enum_lines.add(number)
    record_names = {match[1] for line in lines if (match := re.fullmatch(r"struct (" + nominal + ")", line.strip()))}
    types = ["i32", "i64", "text", "array<i64>", "array<text>", *sorted(schemas | record_names)]
    kind = "(?:" + "|".join(re.escape(value) for value in types) + ")"
    function_header = re.compile(r"function (" + identifier + r")(?:\(([^()]*)\))? -> (" + kind + r"|void):")
    functions = {}
    for line in lines:
        match = function_header.fullmatch(line.strip())
        if match:
            parameters = match[2].split(", ") if match[2] else []
            if all(re.fullmatch(identifier + r": " + kind, parameter) for parameter in parameters):
                functions[match[1]] = len(parameters)
    result["unsupported_declarations"] = [item for item in result["unsupported_declarations"]
        if not ((item["category"] == "unsupported_outer_record" and item["line"] in enum_lines) or
                (item["category"] == "unsupported_function_shape" and function_header.fullmatch(item["text"].strip()) and
                 function_header.fullmatch(item["text"].strip())[1] in functions))]
    def count(value):
        return re.fullmatch(r"[0-9]+", value) is not None and int(value) <= 1
    for item in result["instructions"]:
        opcode, operands = item["opcode"], item["operands"]
        if opcode in {"NEW_VARIANT", "IS_VARIANT", "UNWRAP_VARIANT"}:
            item["supported_opcode"] = True
            if opcode == "NEW_VARIANT":
                item["supported_form"] = (len(operands) == 4 and re.fullmatch(nominal, operands[0]) is not None and count(operands[1]) and
                    re.fullmatch(nominal, operands[2]) is not None and operands[3] in {"void", "i64"})
            elif opcode == "IS_VARIANT":
                item["supported_form"] = (len(operands) == 3 and re.fullmatch(nominal, operands[0]) is not None and
                    re.fullmatch(nominal, operands[1]) is not None and operands[2] in {"void", "i64"})
            else:
                item["supported_form"] = (len(operands) == 3 and count(operands[0]) and
                    re.fullmatch(nominal, operands[1]) is not None and operands[2] in {"void", "i64"})
        elif opcode == "CALL" and not item["supported_form"] and len(operands) == 2 and operands[0] in functions:
            item["supported_form"] = operands[1] == str(functions[operands[0]])
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
    expected_errors = {"manifest_unsupported": 21, "manifest_edition": 9}
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
        record["lane"] = "raw_enum_conformance"
    raw.update(lane="raw_enum_conformance",
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
        item["lane"] = "production_profile9_admission"
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
            if number in (30, 44):
                pair = dict(item, lane="prior_global_grammar")
                for name, tool, parser, status in (("global", args.global_candidate, flow.parse_probe, "admission_refused"),
                                                   ("global_raw", args.global_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] ==
                            "manifest_unsupported", "earlier corpus global contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    # All original editions still lack main intent. Keep the complete measured
    # first-error inventory pinned; new enum parsing can change error order
    # in helpers before main, so the initial development run must verify it.
    expected = {"manifest_unsupported": 18, "manifest_edition": 9, "manifest_intent": 18}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 2
    production = {"lane": "production_profile9_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "global_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "global-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "global_candidate", "global_conformance_candidate", "reference", "compiler", "interpreter")
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
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_enum_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_enums host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 9 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536, "array_descriptors": 128, "array_elements": 1024, "array_literal_elements": 256, "record_layouts": 64, "fields_per_layout": 32, "declared_fields": 256, "record_descriptors": 128, "record_cells": 1024, "global_slots": 128, "global_name_bytes": 128, "integer_text_maximum_bytes": 20, "enum_layouts": 64, "variants_per_enum": 32, "declared_variants": 256, "enum_tag_bytes": 128, "enum_payload_values": 1},
        "earlier_proofs": "126-, 77-, 61-, 27-, 55-, 87-, 103-, 115- and 105-case proofs remain separate required runs",
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
    result = {"schema": 1, "status": "bounded_enum_checkpoint_passed" if passed else "bounded_enum_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 9, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_and_exact_counts_are_explicit(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 149)
        self.assertEqual(len(cases), len({item["name"] for item in cases}))
        self.assertEqual(sum(item["cross_profile"] for item in cases), 53)
        self.assertEqual(sum(item["policy"] for item in cases), 4)
        self.assertEqual(sum(item["validation"] for item in cases), 96)
        self.assertEqual(sum(not item["error"] for item in cases), 48)
        self.assertEqual(sum(bool(item["error"]) and not item["validation"] and not item["policy"] for item in cases), 1)
        self.assertTrue(all(type(item["instructions"]) is int for item in cases))
        self.assertTrue(all(item["origin"] == "independently_authored_textual_nil" for item in cases))
        self.assertTrue(all(item["reference_result"] is not None or item["reference_error"] for item in cases if item["policy"]))
        self.assertEqual(len(source_pins), 17)
        self.assertEqual(len(known_raw_matches), 15)

    def test_lexical_coverage_does_not_claim_nominal_shapes_or_types(self):
        for case in generated_cases():
            if not case["validation"]:
                coverage = instruction_inventory(case["edition"])
                self.assertEqual(coverage["profile"], 9)
                self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]), case["name"])
                self.assertEqual(coverage["unsupported_declarations"], [], case["name"])
                self.assertIn("lexical", coverage["scope"])
        for name in ("reject_new_unknown_tag_dead", "reject_cross_enum_tag_receiver", "reject_noninteger_payload_text", "reject_new_count_type_dead"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(all(item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))
        for name in ("reject_new_dotted_tag", "reject_new_missing_metadata", "reject_unwrap_text"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(any(not item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))

    def test_full_i64_semantics_and_finite_shape_contrasts_are_distinct(self):
        case = next(item for item in generated_cases() if item["name"] == "payload_i64_exact_above_53_bits")
        candidate = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
                     "instructions": 4, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": candidate["result"], "error": None, "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        case = next(item for item in generated_cases() if item["name"] == "unit_unwrap_does_not_reach_caller_sentinel")
        candidate = {"status": "execution_refused", "result": None, "error": "enum_shape", "output_hex": "",
                     "instructions": 3, "console_calls": 0, "output_bytes": 0}
        reference = dict(reference, status="execution_refused", result=None, error=case["reference_error"], output_hex="39390a")
        self.assertEqual(compare_case(case, candidate, reference, candidate), "finite_runtime_policy_contrast")
        for changed in (dict(reference, error="timeout"), dict(reference, error="enum_shape"),
                        dict(reference, status="returned", error=None, result=42), dict(reference, output_hex="")):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, candidate)
        case = next(item for item in generated_cases() if item["name"] == "payload_cannot_be_unwrapped_as_unit")
        reference = dict(reference, status="returned", result=42, error=None, output_hex="")
        self.assertEqual(compare_case(case, candidate, reference, candidate), "finite_runtime_policy_contrast")

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
                error = "manifest_unsupported" if unsupported < 21 else "manifest_edition"
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

    def test_enum_context_cannot_supply_main_intent(self):
        case = next(item for item in generated_cases() if item["name"] == "enum_context_does_not_supply_main_intent")
        candidate = {"status": "returned", "result": 42, "error": None, "output_hex": "",
                     "instructions": 4, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": 42, "error": None, "output_hex": ""}
        production = dict(candidate, status="admission_refused", result=None, error="manifest_intent", instructions=0)
        self.assertEqual(compare_case(case, candidate, reference, production), "matching_semantics")
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, reference, candidate)
        self.assertIn(b'@ai_context("Describe an application choice")\nenum ', case["edition"])
        self.assertNotIn(b"Exercise bounded nominal unit and integer enums", case["edition"])

    def test_reference_bounds_mode_is_unset_and_old_sources_pinned(self):
        environment = baseline.clean_environment()
        self.assertNotIn("NIL_STRICT_BOUNDS", environment)
        self.assertEqual(environment["NIL_CRASH_REPORT"], "0")
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "global-candidate", "global-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "global_candidate", "global_conformance_candidate", "reference", "compiler", "root", "output"):
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
