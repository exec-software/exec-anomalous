#!/usr/bin/env python3
"""Qualify bounded profile-7 nominal records without changing earlier profiles.

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
import program_callable_proof as callables
import program_conformance_proof as conformance


require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe = conformance.parse_probe
source_pins = dict(callables.source_pins, **{
    "program_callable_proof.py": "5fa756b26ff1baea1efb9ce0715984aaca019d6f90780d5e3dbaf81d8457929c",
    "program_callable_reference.cpp": "4272e5f31048417738a3ad08006d2b85bb018b8ccb63d440c010337f0b0979b7",
})
archive_pins = callables.archive_pins
known_raw_matches = dict(sorted({**callables.known_raw_matches,
    3: (1395, "96814aa904a897e5260df03fd4924387111d2032c6bd68caa210ac4d4eb8f5a1"),
    6: (1171, "b97874de3e896b9db7c175c26d1fecd522590d0daf98f7978159481b411d396b"),
    34: (1684, "557d9c5b67a2e88a920a62afab535a3c2d646a0be39423fd20f95b18dff04b1a"),
}.items()))


def authored_record_edition(functions, records, strings=(), annotations=None, *, section=True):
    """Independent textual fixtures; never a transformation of compiler output."""
    value = "# NIL Textual Module\nmodule raw_record_conformance\n\n"
    if strings:
        value += "# String constants\n"
        value += "".join(f'const S{index} = "{text}"\n' for index, text in enumerate(strings)) + "\n"
    if records:
        if section:
            value += "# Struct definitions\n"
        for name, fields, contexts in records:
            value += "".join(context + "\n" for context in contexts)
            value += f"struct {name}\n"
            value += "".join(f"    {field}: {kind}\n" for field, kind in fields)
            value += "end struct\n\n"
    for number, (name, parameters, result, body) in enumerate(functions, 1):
        value += "".join(context + "\n" for context in (annotations or {}).get(name, ()))
        value += f"# declaration_line {number}\nfunction {name}"
        if parameters:
            value += "(" + ", ".join(f"{name}: {kind}" for name, kind in parameters) + ")"
        value += f" -> {result}:\n" + "".join(f"    {instruction}\n" for instruction in body) + "end function\n\n"
    return value.encode()


def layout(name="demo::point", fields=("x", "y"), contexts=()):
    return name, tuple((name, "i64") for name in fields), tuple(contexts)


def generated_cases():
    cases = []
    annotation = '@ai_context("Exercise bounded nominal integer-field records")'
    point = layout()

    def add(name, body, *, records=(point,), helpers=(), strings=(), result=42, output=b"", calls=0,
            instructions=None, error=None, validation=False, policy=False,
            reference_result=None, reference_output=None, reference_error=None,
            ceiling=1, steps=65536, frames=16, annotated=True, production_error=None,
            cross_profile=True, section=True, extra_annotations=None):
        functions = [flow.function("main", body), *helpers]
        annotations = dict(extra_annotations or {})
        if annotated:
            annotations["main"] = (annotation,)
        edition = authored_record_edition(functions, records, strings, annotations, section=section)
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

    def reject(name, body=("PUSH_CONST 42", "RET"), *, error="manifest_syntax", **kwargs):
        add("reject_" + name, body, error=error, validation=True, cross_profile=False, **kwargs)

    add("default_first_field", ["NEW_STRUCT demo::point", "GET_FIELD x", "RET"], result=0, ceiling=0)
    add("default_last_field", ["NEW_STRUCT demo::point", "GET_FIELD y", "RET"], result=0, ceiling=0)
    add("set_retains_record", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD x", "RET"], ceiling=0)
    add("write_one_keeps_other_default", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD y", "RET"], result=0, ceiling=0)
    add("field_order_by_name", ["NEW_STRUCT demo::point", "PUSH_CONST 7", "SET_FIELD y", "PUSH_CONST 42", "SET_FIELD x", "STORE_LOCAL value",
        "LOAD_LOCAL value", "GET_FIELD y", "CONSOLE_WRITELN", "LOAD_LOCAL value", "GET_FIELD x", "RET"], output=b"7\n", calls=1)
    add("repeated_field_assignment", ["NEW_STRUCT demo::point", "PUSH_CONST 7", "SET_FIELD x", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD x", "RET"], ceiling=0)
    for name, number in (("minimum", -(1 << 63)), ("maximum", (1 << 63) - 1), ("exact_above_53_bits", 9007199254740993)):
        add("field_i64_" + name, ["NEW_STRUCT demo::point", f"PUSH_CONST {number}", "SET_FIELD x", "GET_FIELD x", "RET"], result=number, ceiling=0)
    add("shared_mutation_through_alias", ["NEW_STRUCT demo::point", "STORE_LOCAL original", "LOAD_LOCAL original", "STORE_LOCAL alias",
        "LOAD_LOCAL alias", "PUSH_CONST 42", "SET_FIELD x", "POP", "LOAD_LOCAL original", "GET_FIELD x", "RET"], ceiling=0)
    add("independent_allocations", ["NEW_STRUCT demo::point", "STORE_LOCAL first", "NEW_STRUCT demo::point", "STORE_LOCAL second",
        "LOAD_LOCAL first", "PUSH_CONST 42", "SET_FIELD x", "POP", "LOAD_LOCAL second", "GET_FIELD x", "RET"], result=0, ceiling=0)
    add("alias_survives_local_rebind", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "STORE_LOCAL original", "LOAD_LOCAL original",
        "STORE_LOCAL held", "NEW_STRUCT demo::point", "STORE_LOCAL original", "LOAD_LOCAL held", "GET_FIELD x", "RET"], ceiling=0)
    mutate = flow.function("mutate", ["STORE_LOCAL value", "LOAD_LOCAL value", "PUSH_CONST 42", "SET_FIELD x", "POP", "RET"], (("value", "demo::point"),), "void")
    add("callee_mutation_visible_to_caller", ["NEW_STRUCT demo::point", "STORE_LOCAL held", "LOAD_LOCAL held", "CALL mutate 1", "LOAD_LOCAL held", "GET_FIELD x", "RET"],
        helpers=(mutate,), instructions=13, ceiling=0)
    identity = flow.function("identity", ["STORE_LOCAL value", "LOAD_LOCAL value", "RET"], (("value", "demo::point"),), "demo::point")
    add("returned_alias_shares_mutation", ["NEW_STRUCT demo::point", "STORE_LOCAL held", "LOAD_LOCAL held", "CALL identity 1", "PUSH_CONST 42", "SET_FIELD x",
        "POP", "LOAD_LOCAL held", "GET_FIELD x", "RET"], helpers=(identity,), instructions=13, ceiling=0)
    make = flow.function("make", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "RET"], result="demo::point")
    add("callee_created_record_survives_return", ["CALL make 0", "GET_FIELD x", "RET"], helpers=(make,), instructions=7, ceiling=0)
    add("callee_allocations_are_distinct", ["CALL make 0", "STORE_LOCAL first", "CALL make 0", "STORE_LOCAL second", "LOAD_LOCAL second", "PUSH_CONST 7",
        "SET_FIELD x", "POP", "LOAD_LOCAL first", "GET_FIELD x", "RET"], helpers=(make,), instructions=19, ceiling=0)
    choose = flow.function("choose", ["STORE_LOCAL right", "STORE_LOCAL left", "LOAD_LOCAL right", "RET"],
        (("left", "demo::point"), ("right", "demo::point")), "demo::point")
    add("record_parameter_order", ["NEW_STRUCT demo::point", "PUSH_CONST 7", "SET_FIELD x", "NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x",
        "CALL choose 2", "GET_FIELD x", "RET"], helpers=(choose,), instructions=13, ceiling=0)
    add("indirect_record_parameter", ["PUSH_FN_PTR mutate", "STORE_LOCAL operation", "NEW_STRUCT demo::point", "STORE_LOCAL held", "LOAD_LOCAL held",
        "CALL operation 1", "LOAD_LOCAL held", "GET_FIELD x", "RET"], helpers=(mutate,), instructions=15, ceiling=0)
    add("indirect_record_return", ["PUSH_FN_PTR make", "STORE_LOCAL operation", "CALL operation 0", "GET_FIELD x", "RET"], helpers=(make,), instructions=9, ceiling=0)
    make_second = flow.function("make_second", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "RET"], result="demo::point")
    add("rebind_same_nominal_callable_signature", ["PUSH_FN_PTR make", "STORE_LOCAL operation", "PUSH_FN_PTR make_second", "STORE_LOCAL operation",
        "CALL operation 0", "GET_FIELD x", "RET"], helpers=(make, make_second), instructions=11, ceiling=0)
    add("branch_same_nominal_record", ["PUSH_CONST 0", "JMP_IF_NOT right", "NEW_STRUCT demo::point", "JMP done", "LABEL right", "NEW_STRUCT demo::point",
        "PUSH_CONST 42", "SET_FIELD x", "LABEL done", "GET_FIELD x", "RET"], instructions=9, ceiling=0)
    add("loop_record_alias_survives_join", ["NEW_STRUCT demo::point", "STORE_LOCAL held", "PUSH_CONST 0", "STORE_LOCAL index", "LABEL loop", "LOAD_LOCAL index",
        "PUSH_CONST 2", "CMP_LT", "JMP_IF_NOT done", "LOAD_LOCAL held", "PUSH_CONST 42", "SET_FIELD x", "POP", "LOAD_LOCAL index", "PUSH_CONST 1", "ADD",
        "STORE_LOCAL index", "JMP loop", "LABEL done", "LOAD_LOCAL held", "GET_FIELD x", "RET"], instructions=41, ceiling=0)
    named = (layout("left::Point", ("value",)), layout("right::Point", ("value",)))
    add("qualified_nominal_names_are_distinct", ["NEW_STRUCT left::Point", "PUSH_CONST 7", "SET_FIELD value", "STORE_LOCAL left", "NEW_STRUCT right::Point",
        "PUSH_CONST 42", "SET_FIELD value", "STORE_LOCAL right", "LOAD_LOCAL left", "GET_FIELD value", "CONSOLE_WRITELN", "LOAD_LOCAL right", "GET_FIELD value", "RET"],
        records=named, output=b"7\n", calls=1)
    add("field_named_like_console_builtin", ["NEW_STRUCT box", "PUSH_CONST 42", "SET_FIELD println", "GET_FIELD println", "RET"],
        records=(layout("box", ("println",)),), ceiling=0)
    add("qualified_name_has_primitive_spelled_segment", ["NEW_STRUCT geo::bool", "PUSH_CONST 42", "SET_FIELD value", "GET_FIELD value", "RET"],
        records=(layout("geo::bool", ("value",)),), ceiling=0)
    add("empty_record", ["NEW_STRUCT empty", "POP", "PUSH_CONST 42", "RET"], records=(layout("empty", ()),), ceiling=0)
    context = '@ai_context("A point in device pixels")'
    add("record_and_main_contexts", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD x", "RET"],
        records=(layout(contexts=(context,)),), ceiling=0)
    add("record_only_context_is_not_main_intent", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD x", "RET"],
        records=(layout(contexts=(context,)),), ceiling=0, annotated=False)
    add("multiple_record_contexts", ["NEW_STRUCT second", "PUSH_CONST 42", "SET_FIELD value", "GET_FIELD value", "RET"],
        records=(layout("first", ("value",), (context,)), layout("second", ("value",), (context,))), ceiling=0)
    add("record_context_exact_capacity", ["NEW_STRUCT demo::point", "POP", "PUSH_CONST 42", "RET"],
        records=(layout(contexts=('@ai_context("' + "x" * 512 + '")',)),), ceiling=0)
    add("dead_known_record_and_field", ["PUSH_CONST 42", "RET", "NEW_STRUCT demo::point", "GET_FIELD x", "SET_FIELD y"], instructions=2, ceiling=0)
    add("dead_field_exists_on_other_layout", ["PUSH_CONST 42", "RET", "NEW_STRUCT first", "GET_FIELD second_only"],
        records=(layout("first", ("first_only",)), layout("second", ("second_only",))), instructions=2, ceiling=0)

    many_layouts = tuple(layout(f"record_{index}", ("value",)) for index in range(64))
    add("layouts_exact_capacity", ["NEW_STRUCT record_63", "PUSH_CONST 42", "SET_FIELD value", "GET_FIELD value", "RET"], records=many_layouts, ceiling=0)
    wide = layout("wide", tuple(f"field_{index}" for index in range(32)))
    add("fields_per_layout_exact_capacity", ["NEW_STRUCT wide", "PUSH_CONST 42", "SET_FIELD field_31", "GET_FIELD field_31", "RET"], records=(wide,), ceiling=0)
    all_fields = tuple(layout(f"wide_{index}", tuple(f"field_{field}" for field in range(32))) for index in range(8))
    add("declared_fields_exact_capacity", ["NEW_STRUCT wide_7", "PUSH_CONST 42", "SET_FIELD field_31", "GET_FIELD field_31", "RET"], records=all_fields, ceiling=0)
    unique_fields = tuple(layout(f"record_{index}", tuple(f"field_{index * 4 + field}" for field in range(4))) for index in range(64))
    add("highest_layout_and_global_field_identity", ["NEW_STRUCT record_63", "PUSH_CONST 42", "SET_FIELD field_255", "GET_FIELD field_255", "RET"],
        records=unique_fields, ceiling=0)
    high = flow.function("high", ["NEW_STRUCT record_63", "PUSH_CONST 42", "SET_FIELD value", "RET"], result="record_63")
    add("highest_layout_callable_identity", ["PUSH_FN_PTR high", "STORE_LOCAL operation", "CALL operation 0", "GET_FIELD value", "RET"],
        records=many_layouts, helpers=(high,), instructions=9, ceiling=0)
    empty = ["NEW_STRUCT empty", "POP"]
    add("record_descriptors_exact_capacity", empty * 128 + ["PUSH_CONST 42", "RET"], records=(layout("empty", ()),), ceiling=0)
    add("record_descriptors_no_reclamation_after_pop", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + empty * 129 + ["PUSH_CONST 42", "RET"],
        records=(layout("empty", ()),), error="record_limit", instructions=259, output=b"7\n", calls=1, policy=True, reference_result=42, reference_output=b"7\n")
    empty_helper = flow.function("empty_value", ["NEW_STRUCT empty", "RET"], result="empty")
    add("record_descriptors_no_reclamation_after_return", ["CALL empty_value 0", "POP"] * 129 + ["PUSH_CONST 42", "RET"], records=(layout("empty", ()),),
        helpers=(empty_helper,), error="record_limit", instructions=514, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    add("alias_mutation_after_descriptor_capacity", ["NEW_STRUCT demo::point", "STORE_LOCAL held"] + empty * 127 +
        ["LOAD_LOCAL held", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD x", "RET"], records=(point, layout("empty", ())), ceiling=0)
    cell_block = ["NEW_STRUCT wide", "POP"]
    add("record_cells_exact_capacity", cell_block * 32 + ["PUSH_CONST 42", "RET"], records=(wide,), ceiling=0)
    add("record_cells_no_reclamation_after_pop", ["PUSH_CONST 7", "CONSOLE_WRITELN"] + cell_block * 33 + ["PUSH_CONST 42", "RET"],
        records=(wide,), error="record_limit", instructions=67, output=b"7\n", calls=1, policy=True, reference_result=42, reference_output=b"7\n")
    wide_helper = flow.function("wide_value", ["NEW_STRUCT wide", "RET"], result="wide")
    add("record_cells_no_reclamation_after_return", ["CALL wide_value 0", "POP"] * 33 + ["PUSH_CONST 42", "RET"], records=(wide,), helpers=(wide_helper,),
        error="record_limit", instructions=130, policy=True, reference_result=42, reference_output=b"", ceiling=0)
    add("alias_mutation_after_cell_capacity", ["NEW_STRUCT wide", "STORE_LOCAL held"] + cell_block * 31 +
        ["LOAD_LOCAL held", "PUSH_CONST 42", "SET_FIELD field_31", "GET_FIELD field_31", "RET"], records=(wide,), ceiling=0)
    add("empty_record_after_cell_capacity", cell_block * 32 + ["NEW_STRUCT empty", "POP", "PUSH_CONST 42", "RET"], records=(wide, layout("empty", ())), ceiling=0)
    add("record_output_before_fault", ["NEW_STRUCT demo::point", "PUSH_CONST 7", "SET_FIELD x", "GET_FIELD x", "CONSOLE_WRITELN", "PUSH_CONST 5",
        "PUSH_CONST 0", "DIV i64", "RET"], output=b"7\n", calls=1, error="division_by_zero", instructions=8, reference_error="Division by zero")
    add("record_finite_fuel", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD x", "GET_FIELD x", "RET"], error="instruction_limit", instructions=3,
        steps=3, policy=True, reference_result=42, reference_output=b"", ceiling=0)

    reject("unknown_record_dead", ["PUSH_CONST 42", "RET", "NEW_STRUCT missing"], error="manifest_unsupported")
    reject("unknown_field_dead", ["PUSH_CONST 42", "RET", "GET_FIELD missing"], error="manifest_unsupported")
    reject("unknown_set_field_dead", ["PUSH_CONST 42", "RET", "SET_FIELD missing"], error="manifest_unsupported")
    other_fields = (point, layout("other", ("other_only",)))
    reject("field_not_in_receiver", ["NEW_STRUCT demo::point", "GET_FIELD other_only", "RET"], records=other_fields)
    reject("set_field_not_in_receiver", ["NEW_STRUCT demo::point", "PUSH_CONST 42", "SET_FIELD other_only", "POP", "PUSH_CONST 42", "RET"], records=other_fields)
    reject("duplicate_layout", records=(point, point))
    reject("duplicate_field", records=(layout(fields=("x", "x")),))
    reject("missing_record_section", section=False)
    for name in ("i64", "bool", "f64"):
        reject("primitive_record_name_" + name, records=(layout(name),), error="manifest_unsupported")
    for name, field_type in (("text", "text"), ("i32", "i32"), ("bool", "bool"), ("array", "array<i64>"), ("nested", "demo::point"), ("pointer", "ptr<i64>")):
        reject("field_type_" + name, records=(("demo::point", (("value", field_type),), ()),), error="manifest_unsupported")
    reject("layouts_over_capacity", records=many_layouts + (layout("record_64", ()),), error="manifest_limit")
    reject("fields_per_layout_over_capacity", records=(layout("wide", tuple(f"field_{index}" for index in range(33))),), error="manifest_limit")
    reject("declared_fields_over_capacity", records=all_fields + (layout("extra", ("value",)),), error="manifest_limit")
    reject("record_context_empty", records=(layout(contexts=('@ai_context("")',)),), error="manifest_intent")
    reject("record_context_duplicate", records=(layout(contexts=(context, context)),), error="manifest_intent")
    reject("record_context_over_capacity", records=(layout(contexts=('@ai_context("' + "x" * 513 + '")',)),), error="manifest_limit")
    reject("record_context_unsupported_shape", records=(layout(contexts=('@ai_context(value)',)),), error="manifest_unsupported")
    reject("helper_context_is_not_record_context", helpers=(flow.function("helper", ["PUSH_CONST 42", "RET"]),),
        extra_annotations={"helper": (context,)}, error="manifest_intent")
    for name, instruction in (("new_missing_operand", "NEW_STRUCT"), ("new_extra_operand", "NEW_STRUCT demo::point extra"),
                              ("get_missing_operand", "GET_FIELD"), ("get_extra_operand", "GET_FIELD x i64"),
                              ("set_missing_operand", "SET_FIELD"), ("set_extra_operand", "SET_FIELD x i64"),
                              ("field_pointer", "SET_FIELD_PTR x"), ("free", "FREE")):
        reject(name, ["PUSH_CONST 42", "RET", instruction], error="manifest_unsupported")
    reject("get_integer_receiver", ["PUSH_CONST 0", "GET_FIELD x", "RET"])
    reject("get_text_receiver", ["PUSH_CONST_STR S0", "GET_FIELD x", "RET"], strings=("text",))
    reject("get_array_receiver", ["NEW_ARRAY_LITERAL 0 i64", "GET_FIELD x", "RET"])
    reject("set_integer_receiver", ["PUSH_CONST 0", "PUSH_CONST 42", "SET_FIELD x", "RET"])
    reject("get_underflow", ["GET_FIELD x", "RET"])
    reject("set_underflow", ["NEW_STRUCT demo::point", "SET_FIELD x", "POP", "PUSH_CONST 42", "RET"])
    for name, value in (("text", "PUSH_CONST_STR S0"), ("array", "NEW_ARRAY_LITERAL 0 i64"), ("record", "NEW_STRUCT demo::point")):
        reject("set_noninteger_" + name, ["NEW_STRUCT demo::point", value, "SET_FIELD x", "POP", "PUSH_CONST 42", "RET"], strings=("text",))
    for name, tail in (
        ("console", ["CONSOLE_WRITE", "PUSH_CONST 42", "RET"]),
        ("cast", ["CAST i64", "RET"]), ("integer_return", ["RET"]),
        ("arithmetic", ["PUSH_CONST 1", "ADD", "RET"]), ("comparison", ["PUSH_CONST 1", "CMP_EQ", "RET"]),
        ("branch", ["JMP_IF done", "LABEL done", "PUSH_CONST 42", "RET"]),
        ("concat", ["PUSH_CONST_STR S0", "STR_CONCAT", "POP", "PUSH_CONST 42", "RET"]),
        ("array_element", ["NEW_ARRAY_LITERAL 1 i64", "POP", "PUSH_CONST 42", "RET"]),
        ("len", ["LEN", "RET"]), ("index", ["PUSH_CONST 0", "GET_INDEX", "RET"]),
    ):
        reject("opaque_record_" + name, ["NEW_STRUCT demo::point", *tail], strings=("text",))
    answer = flow.function("answer", ["PUSH_CONST 42", "RET"])
    reject("call_record_as_function", ["NEW_STRUCT demo::point", "STORE_LOCAL operation", "CALL operation 0", "RET"])
    reject("set_callable_field", ["NEW_STRUCT demo::point", "PUSH_FN_PTR answer", "SET_FIELD x", "POP", "PUSH_CONST 42", "RET"], helpers=(answer,))
    twin = layout("other::point")
    reject("rebind_nominal_record", ["NEW_STRUCT demo::point", "STORE_LOCAL value", "NEW_STRUCT other::point", "STORE_LOCAL value", "PUSH_CONST 42", "RET"], records=(point, twin))
    reject("join_nominal_records", ["PUSH_CONST 0", "JMP_IF_NOT right", "NEW_STRUCT demo::point", "JMP done", "LABEL right", "NEW_STRUCT other::point",
        "LABEL done", "POP", "PUSH_CONST 42", "RET"], records=(point, twin))
    reject("direct_nominal_argument", ["NEW_STRUCT other::point", "CALL identity 1", "POP", "PUSH_CONST 42", "RET"], records=(point, twin), helpers=(identity,))
    reject("indirect_nominal_argument", ["PUSH_FN_PTR identity", "STORE_LOCAL operation", "NEW_STRUCT other::point", "CALL operation 1", "POP", "PUSH_CONST 42", "RET"],
        records=(point, twin), helpers=(identity,))
    wrong_return = flow.function("wrong", ["NEW_STRUCT other::point", "RET"], result="demo::point")
    reject("nominal_return", ["CALL wrong 0", "POP", "PUSH_CONST 42", "RET"], records=(point, twin), helpers=(wrong_return,))
    for index in (1, 8, 32, 63):
        first = flow.function("first", ["NEW_STRUCT record_0", "RET"], result="record_0")
        different = flow.function("different", [f"NEW_STRUCT record_{index}", "RET"], result=f"record_{index}")
        reject(f"callable_nominal_return_identity_{index}", ["PUSH_FN_PTR first", "STORE_LOCAL operation", "PUSH_FN_PTR different", "STORE_LOCAL operation", "PUSH_CONST 42", "RET"],
            records=many_layouts, helpers=(first, different))
    first = flow.function("first", ["STORE_LOCAL value", "PUSH_CONST 42", "RET"], (("value", "record_0"),))
    different = flow.function("different", ["STORE_LOCAL value", "PUSH_CONST 42", "RET"], (("value", "record_8"),))
    reject("callable_nominal_parameter_identity", ["PUSH_FN_PTR first", "STORE_LOCAL operation", "PUSH_FN_PTR different", "STORE_LOCAL operation", "PUSH_CONST 42", "RET"],
        records=many_layouts, helpers=(first, different))
    floor = flow.function("floor", ["PUSH_CONST 42", "SET_FIELD x", "POP", "PUSH_CONST 0", "RET"])
    reject("callee_cannot_mutate_caller_record", ["NEW_STRUCT demo::point", "CALL floor 0", "POP", "GET_FIELD x", "RET"], helpers=(floor,))
    reject("dead_console_ceiling", ["NEW_STRUCT demo::point", "POP", "PUSH_CONST 42", "RET", "PUSH_CONST 7", "CONSOLE_WRITE"], error="manifest_ceiling", ceiling=0)
    reject("dead_foreign_effect", ["NEW_STRUCT demo::point", "POP", "PUSH_CONST 42", "RET", "INTEROP_CALL libc puts 1 i64"], error="manifest_unsupported")
    require(len({case["name"] for case in cases}) == len(cases), "duplicate record boundary case")
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
                for name, tool, parser, status in (("callable", args.callable_candidate, flow.parse_probe, "admission_refused"),
                                                   ("callable_raw", args.callable_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, str(case["ceiling"])],
                        output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = "manifest_unsupported"
                    require(previous["status"] == status and previous["error"] == expected, "earlier callable grammar expanded")
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
            "callable_refusals": sum("callable" in item for item in records),
            "callable_conformance_refusals": sum("callable_raw" in item for item in records)}


def instruction_inventory(data):
    """Lexical metadata only; complete admission still checks nominal types and CFG."""
    result = callables.instruction_inventory(data)
    result.update(profile=7, lane="raw_record_conformance",
                  scope="profile-7 lexical coverage only; nominal identities, fields, authority and CFG require complete validation")
    if result["unsupported_format"]:
        return result
    identifier = r"[A-Za-z_][A-Za-z_0-9]*"
    nominal = identifier + r"(?:::" + identifier + r")*"
    record_header = re.compile(r"struct (" + nominal + ")")
    layouts, record_lines, active = set(), set(), False
    for number, line in enumerate(data.decode("utf-8").splitlines(), 1):
        line = line.strip()
        if line == "# Struct definitions":
            record_lines.add(number)
        match = record_header.fullmatch(line)
        if match:
            layouts.add(match[1])
            record_lines.add(number)
            active = True
        elif active and line == "end struct":
            record_lines.add(number)
            active = False
        elif active and re.fullmatch(identifier + r": i64", line):
            record_lines.add(number)
    types = ["i32", "i64", "text", "array<i64>", "array<text>", *sorted(layouts)]
    kind = "(?:" + "|".join(re.escape(value) for value in types) + ")"
    header_pattern = re.compile(r"function (" + identifier + r")(?:\(([^()]*)\))? -> (" + kind + r"|void):")
    headers = {}
    for line in data.decode("utf-8").splitlines():
        match = header_pattern.fullmatch(line.strip())
        if match:
            parameters = match[2].split(", ") if match[2] else []
            if all(re.fullmatch(identifier + r": " + kind, parameter) for parameter in parameters):
                headers[match[1]] = len(parameters)
    result["unsupported_declarations"] = [record for record in result["unsupported_declarations"]
        if not ((record["category"] == "unsupported_outer_record" and record["line"] in record_lines) or
                (record["category"] == "unsupported_function_shape" and header_pattern.fullmatch(record["text"].strip()) and
                 header_pattern.fullmatch(record["text"].strip())[1] in headers))]
    for item in result["instructions"]:
        opcode, operands = item["opcode"], item["operands"]
        if opcode == "NEW_STRUCT":
            item["supported_opcode"] = True
            item["supported_form"] = len(operands) == 1 and re.fullmatch(nominal, operands[0]) is not None
        elif opcode in {"GET_FIELD", "SET_FIELD"}:
            item["supported_opcode"] = True
            item["supported_form"] = len(operands) == 1 and re.fullmatch(identifier, operands[0]) is not None
        elif opcode == "CALL" and not item["supported_form"] and len(operands) == 2 and operands[0] in headers:
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
    expected_errors = {"manifest_unsupported": 24, "manifest_edition": 9}
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
        record["lane"] = "raw_record_conformance"
    raw.update(lane="raw_record_conformance",
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
        item["lane"] = "production_profile7_admission"
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
            if number in (3, 6, 34):
                pair = dict(item, lane="prior_callable_grammar")
                for name, tool, parser, status in (("callable", args.callable_candidate, flow.parse_probe, "admission_refused"),
                                                   ("callable_raw", args.callable_conformance_candidate, parse_probe, "validation_refused")):
                    process, stdout, _ = baseline.run([tool, path, "1"], output / f"corpus-{name.replace('_', '-')}-{number:03d}",
                                                      environment=environment)
                    previous = parser(process, stdout)
                    require(previous["status"] == status and previous["error"] ==
                            "manifest_unsupported", "earlier corpus callable contract changed")
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome="failed", failure=str(error))
            failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item["candidate"]["error"] for item in production_records if item["outcome"] == "production_refused"))
    # All original editions still lack main intent. Keep the complete measured
    # first-error inventory pinned; new record parsing can change error order
    # in helpers before main, so the initial development run must verify it.
    expected = {"manifest_unsupported": 21, "manifest_edition": 9, "manifest_intent": 15}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 3
    production = {"lane": "production_profile7_admission", "passed": passed, "invocations": len(production_records),
                  "failures": failures, "refusals": sum(item["outcome"] == "production_refused" for item in production_records),
                  "refusal_errors": errors, "expected_refusal_errors": expected, "callable_corpus_comparisons": len(cross),
                  "scope": "Production admission attempts on identical complete editions; no raw semantic coverage credit"}
    write_json(output / "production-corpus-executions.json", production_records)
    write_json(output / "production-corpus-summary.json", production)
    write_json(output / "callable-corpus-executions.json", cross)
    return raw, production


def qualification(args):
    binary_names = ("candidate", "production_candidate", "callable_candidate", "callable_conformance_candidate", "reference", "compiler", "interpreter")
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
    tools["reference_source"] = baseline.record_tool(Path(__file__).with_name("program_record_reference.cpp"))
    tools["script"] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / "tools.json", tools)
    write_json(args.output / "corpus-inventory.json", before)
    write_json(args.output / "environment-policy.json", {
        "NIL_CRASH_REPORT": "0", "removed": ["NY_LIBRARY_PATH", "NIL_LIBRARY_PATH", "NY_LEX_DUMP", "NY_PARSE_TRACE",
                                                "NY_STDLIB_PATH", "NIL_MEMORY_PROFILE", "NIL_STRICT_BOUNDS"],
        "upstream_argv": "unchanged; original capture wrappers forward all arguments",
        "raw_lane": "explicit validate_record host entry, production intent absent only where originally absent",
        "production_lane": "explicit profile 7 encoder/admission/execution on the same bytes",
        "limits": {"steps": 65536, "frames": 16, "output_bytes": 65536, "computed_text_bytes": 65536, "array_descriptors": 128, "array_elements": 1024, "array_literal_elements": 256, "record_layouts": 64, "fields_per_layout": 32, "declared_fields": 256, "record_descriptors": 128, "record_cells": 1024},
        "earlier_proofs": "126-, 77-, 61-, 27-, 55-, 87- and 103-case proofs remain separate required runs",
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
    result = {"schema": 1, "status": "bounded_record_checkpoint_passed" if passed else "bounded_record_checkpoint_failed",
              "candidate_qualified": False, "native_execution": False, "production_admission_waived": False,
              "language_profile": 7, "boundary_results": boundary, "raw_corpus": raw, "production_corpus": production,
              "input_manifest_sha256": before["input_manifest_sha256"], "tools": tools}
    write_json(args.output / "summary.json", result)
    print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_inventory_and_exact_counts_are_explicit(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 115)
        self.assertEqual(len(cases), len({item["name"] for item in cases}))
        self.assertEqual(sum(item["cross_profile"] for item in cases), 48)
        self.assertEqual(sum(item["policy"] for item in cases), 5)
        self.assertEqual(sum(item["validation"] for item in cases), 67)
        self.assertEqual(sum(not item["error"] for item in cases), 42)
        self.assertTrue(all(type(item["instructions"]) is int for item in cases))
        self.assertTrue(all(item["origin"] == "independently_authored_textual_nil" for item in cases))
        self.assertTrue(all(item["reference_result"] is not None for item in cases if item["policy"]))
        self.assertIn("callee_mutation_visible_to_caller", {item["name"] for item in cases})
        self.assertIn("highest_layout_and_global_field_identity", {item["name"] for item in cases})
        self.assertEqual(len(source_pins), 13)

    def test_lexical_coverage_does_not_claim_nominal_or_field_validation(self):
        for name in ("default_first_field", "indirect_record_parameter", "highest_layout_and_global_field_identity",
                     "qualified_name_has_primitive_spelled_segment", "record_and_main_contexts", "record_parameter_order"):
            case = next(item for item in generated_cases() if item["name"] == name)
            coverage = instruction_inventory(case["edition"])
            self.assertEqual(coverage["profile"], 7)
            self.assertTrue(all(item["supported_form"] for item in coverage["instructions"]))
            self.assertEqual(coverage["unsupported_declarations"], [])
            self.assertIn("lexical", coverage["scope"])
        for name in ("reject_unknown_record_dead", "reject_field_not_in_receiver", "reject_callable_nominal_return_identity_63"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(all(item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))
        for name in ("reject_new_missing_operand", "reject_get_extra_operand", "reject_field_pointer"):
            case = next(item for item in generated_cases() if item["name"] == name)
            self.assertTrue(any(not item["supported_form"] for item in instruction_inventory(case["edition"])["instructions"]))

    def test_exact_semantics_and_policy_contrasts_cannot_be_confused(self):
        case = next(item for item in generated_cases() if item["name"] == "field_i64_exact_above_53_bits")
        candidate = {"status": "returned", "result": 9007199254740993, "error": None, "output_hex": "",
                     "instructions": 5, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": candidate["result"], "error": None, "output_hex": ""}
        self.assertEqual(compare_case(case, candidate, reference, candidate), "matching_semantics")
        for field, value in (("result", 1), ("instructions", 1), ("console_calls", 1), ("output_hex", "00")):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, **{field: value}), reference, candidate)
        case = next(item for item in generated_cases() if item["name"] == "record_descriptors_no_reclamation_after_pop")
        candidate = {"status": "execution_refused", "result": None, "error": "record_limit", "output_hex": "370a",
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
                error = "manifest_unsupported" if unsupported < 24 else "manifest_edition"
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

    def test_record_context_cannot_supply_main_intent(self):
        case = next(item for item in generated_cases() if item["name"] == "record_only_context_is_not_main_intent")
        candidate = {"status": "returned", "result": 42, "error": None, "output_hex": "",
                     "instructions": 5, "console_calls": 0, "output_bytes": 0}
        reference = {"schema": 1, "status": "returned", "result": 42, "error": None, "output_hex": ""}
        production = dict(candidate, status="admission_refused", result=None, error="manifest_intent", instructions=0)
        self.assertEqual(compare_case(case, candidate, reference, production), "matching_semantics")
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, reference, candidate)
        self.assertIn(b'@ai_context("A point in device pixels")\nstruct ', case["edition"])
        self.assertNotIn(b"Exercise bounded nominal integer-field records", case["edition"])

    def test_reference_bounds_mode_is_unset_and_old_sources_pinned(self):
        environment = baseline.clean_environment()
        self.assertNotIn("NIL_STRICT_BOUNDS", environment)
        self.assertEqual(environment["NIL_CRASH_REPORT"], "0")
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate", "production-candidate", "callable-candidate", "callable-conformance-candidate", "reference", "compiler", "interpreter", "root", "output"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ("candidate", "production_candidate", "callable_candidate", "callable_conformance_candidate", "reference", "compiler", "root", "output"):
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
