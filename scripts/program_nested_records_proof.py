#!/usr/bin/env python3
"""Qualify profile12 nested nominal records and bounded concrete aggregate names.

Complete unchanged editions, prior profiles, and production intent remain separate
obligations. Missing nested fields and erased generic enum tags are explicit
finite policy contrasts with the pinned VM, never claimed semantic matches.
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
import program_source_debug_proof as debug_proof
import program_typed_scalars_proof as typed_proof
import program_conformance_proof as conformance

require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
parse_probe, parse_production = typed_proof.parse_probe, typed_proof.parse_production
source_pins = dict(typed_proof.source_pins, **{
    'program_typed_scalars_proof.py': '91c03de8a7cbfa41f10e20ea1773de37301a13c5db39d253982c0f8d267b9f73',
    'program_typed_scalars_reference.cpp': '41f1526ed5f4c4dc027e98315f579b5491babd177ec35fbd6a513cdda09c2afc',
})
archive_pins = typed_proof.archive_pins
primary_pins = dict(typed_proof.primary_pins, **{
    'nilcompiler/src/array_intrinsic.hpp': '543c4f32238b0e7d6ff235f5fc844184a90f3bbe4108cbb9e80b3566645335cb',
    'nilcompiler/src/class_contract.cpp': '25d99ca8c2516eeebed46c18f6f89fb8f49aad4d6c33d75b7c4e81316b608f4e',
})
known_raw_matches = dict(sorted({**typed_proof.known_raw_matches,
    7: (7534, '70021ba440a8fe8c050607325a6b6d250bbfa5e3e19077bf6cee86b43eb7bef7'),
}.items()))
expected_raw_matches = sorted([*known_raw_matches, 4])
aggregate_output = b'3\n4\n4\n42\n7\nqueued\nclient failure\nclient failure\n42\n'
def authored_edition(*args, **kwargs):
    edition, metadata, literals = typed_proof.authored_edition(*args, **kwargs)
    edition = edition.replace(b'Exercise bounded typed scalar values', b'Exercise bounded nested nominal records')
    return edition, metadata, literals

floating = typed_proof.floating


def layout(name, fields=(), contexts=()):
    return name, tuple(fields), tuple(contexts)


def generated_cases():
    cases = []
    leaf = layout('tree::leaf', (('value', 'i64'),))
    outer = layout('tree::outer', (('child', 'tree::leaf'), ('count', 'i64')))
    node = layout('tree::node', (('next', 'tree::node'), ('value', 'i64')))
    default_layouts = (leaf, outer)
    def add(name, body, *, layouts=default_layouts, enums=(), helpers=(), result=42, output=b'', calls=0,
            instructions=None, error=None, validation=False, policy=False, admission_policy=False,
            reference_result=None, reference_error=None, reference_output=None, annotated=True,
            production_error=None, ceiling=0, steps=65536, frames=16, cross_profile=True,
            previous_error='manifest_unsupported', strings=(), transform=None):
        edition, metadata, literals = authored_edition([flow.function('main', body), *helpers], annotated=annotated,
                                                       strings=strings, layouts=layouts, enums=enums)
        if transform:
            edition = transform(edition)
        require(len(edition) <= 65536, 'fixture exceeds edition bound: ' + name)
        dispatch = sum((isinstance(item, str) and bool(item.strip())) or (isinstance(item, dict) and 'floating' in item) for item in body)
        cases.append({'name': name, 'edition': edition, 'result': None if error else result, 'error': error,
                      'output': output, 'console_calls': calls, 'instructions': 0 if validation else dispatch if instructions is None else instructions,
                      'validation': validation, 'policy': policy, 'admission_policy': admission_policy,
                      'reference_result': reference_result, 'reference_error': reference_error, 'reference_output': reference_output,
                      'source_debug': [] if validation else metadata, 'reference_source_debug': metadata,
                      'f64_literals': [] if validation else literals, 'reference_f64_literals': literals,
                      'annotated': annotated, 'production_error': production_error or (error if validation else 'manifest_intent' if not annotated else None),
                      'ceiling': ceiling, 'steps': steps, 'frames': frames, 'cross_profile': cross_profile, 'previous_error': previous_error,
                      'origin': 'independently_authored_textual_nil'})
    def reject(name, body=('PUSH_CONST 42', 'RET'), *, error='manifest_syntax', **kwargs):
        add('reject_' + name, list(body), error=error, validation=True, cross_profile=False, **kwargs)
    def initialized(value=42):
        return ['NEW_STRUCT tree::outer', 'NEW_STRUCT tree::leaf', f'PUSH_CONST {value}', 'SET_FIELD value', 'SET_FIELD child']
    def get_child():
        return ['GET_FIELD child', 'GET_FIELD value', 'RET']
    def choice(name='choice<i64>'):
        return enums_proof.enum_layout(name, (('Present', 'i64'), ('Absent', None)))

    add('old_integer_field_defaults_to_zero', ['NEW_STRUCT tree::leaf', 'GET_FIELD value', 'RET'], layouts=(leaf,), result=0, cross_profile=False)
    add('nested_field_roundtrip', initialized() + get_child())
    add('forward_record_field_resolution', initialized() + get_child(), layouts=(outer, leaf))
    add('unused_forward_schema_resolves', ['PUSH_CONST 42', 'RET'], layouts=(outer, leaf))
    add('nested_and_integer_fields_are_distinct', initialized(40) + ['PUSH_CONST 2', 'SET_FIELD count', 'STORE_LOCAL owner', 'LOAD_LOCAL owner',
        'GET_FIELD count', 'LOAD_LOCAL owner', 'GET_FIELD child', 'GET_FIELD value', 'ADD i64', 'RET'])
    for name, value in [('minimum', -(1 << 63)), ('maximum', (1 << 63) - 1), ('above_f64_exactness', 9007199254740993)]:
        add('nested_integer_' + name, initialized(value) + get_child(), result=value)
    add('nested_child_integer_default', ['NEW_STRUCT tree::outer', 'NEW_STRUCT tree::leaf', 'SET_FIELD child'] + get_child(), result=0)
    add('child_alias_mutation_visible_through_parent', ['NEW_STRUCT tree::leaf', 'STORE_LOCAL child', 'NEW_STRUCT tree::outer', 'LOAD_LOCAL child',
        'SET_FIELD child', 'STORE_LOCAL parent', 'LOAD_LOCAL child', 'PUSH_CONST 42', 'SET_FIELD value', 'POP', 'LOAD_LOCAL parent'] + get_child())
    add('retrieved_child_alias_mutates_original', ['NEW_STRUCT tree::leaf', 'STORE_LOCAL child', 'NEW_STRUCT tree::outer', 'LOAD_LOCAL child',
        'SET_FIELD child', 'GET_FIELD child', 'PUSH_CONST 42', 'SET_FIELD value', 'POP', 'LOAD_LOCAL child', 'GET_FIELD value', 'RET'])
    pair = layout('tree::pair', (('left', 'tree::leaf'), ('right', 'tree::leaf')))
    add('two_fields_share_one_child', ['NEW_STRUCT tree::leaf', 'STORE_LOCAL child', 'NEW_STRUCT tree::pair', 'LOAD_LOCAL child', 'SET_FIELD left',
        'LOAD_LOCAL child', 'SET_FIELD right', 'STORE_LOCAL parent', 'LOAD_LOCAL parent', 'GET_FIELD left', 'PUSH_CONST 42', 'SET_FIELD value', 'POP',
        'LOAD_LOCAL parent', 'GET_FIELD right', 'GET_FIELD value', 'RET'], layouts=(leaf, pair))
    add('two_parents_share_one_child', ['NEW_STRUCT tree::leaf', 'STORE_LOCAL child', 'NEW_STRUCT tree::outer', 'LOAD_LOCAL child', 'SET_FIELD child',
        'STORE_LOCAL first', 'NEW_STRUCT tree::outer', 'LOAD_LOCAL child', 'SET_FIELD child', 'STORE_LOCAL second', 'LOAD_LOCAL first', 'GET_FIELD child',
        'PUSH_CONST 42', 'SET_FIELD value', 'POP', 'LOAD_LOCAL second'] + get_child())
    add('field_reassignment_preserves_old_alias', initialized(40) + ['STORE_LOCAL owner', 'LOAD_LOCAL owner', 'GET_FIELD child', 'STORE_LOCAL previous',
        'LOAD_LOCAL owner', 'NEW_STRUCT tree::leaf', 'PUSH_CONST 2', 'SET_FIELD value', 'SET_FIELD child', 'GET_FIELD child', 'GET_FIELD value',
        'LOAD_LOCAL previous', 'GET_FIELD value', 'ADD i64', 'RET'])
    add('replaced_child_stays_mutable', initialized(1) + ['STORE_LOCAL owner', 'LOAD_LOCAL owner', 'GET_FIELD child', 'STORE_LOCAL previous',
        'LOAD_LOCAL owner', 'NEW_STRUCT tree::leaf', 'SET_FIELD child', 'POP', 'LOAD_LOCAL previous', 'PUSH_CONST 42', 'SET_FIELD value', 'GET_FIELD value', 'RET'])
    add('self_recursive_layout_unused', ['PUSH_CONST 42', 'RET'], layouts=(node,))
    add('self_cycle_alias_mutation', ['NEW_STRUCT tree::node', 'STORE_LOCAL root', 'LOAD_LOCAL root', 'LOAD_LOCAL root', 'SET_FIELD next', 'POP',
        'LOAD_LOCAL root', 'GET_FIELD next', 'PUSH_CONST 42', 'SET_FIELD value', 'POP', 'LOAD_LOCAL root', 'GET_FIELD value', 'RET'], layouts=(node,))
    left = layout('left', (('other', 'right'), ('value', 'i64')))
    right = layout('right', (('other', 'left'),))
    add('mutual_cycle_alias_mutation', ['NEW_STRUCT left', 'STORE_LOCAL first', 'NEW_STRUCT right', 'STORE_LOCAL second',
        'LOAD_LOCAL first', 'LOAD_LOCAL second', 'SET_FIELD other', 'POP', 'LOAD_LOCAL second', 'LOAD_LOCAL first', 'SET_FIELD other', 'POP',
        'LOAD_LOCAL second', 'GET_FIELD other', 'PUSH_CONST 42', 'SET_FIELD value', 'POP', 'LOAD_LOCAL first', 'GET_FIELD other', 'GET_FIELD other',
        'GET_FIELD value', 'RET'], layouts=(left, right))
    add('mutual_schema_reverse_order', ['PUSH_CONST 42', 'RET'], layouts=(right, left))
    for count in (8, 128):
        chain = ['NEW_STRUCT tree::node', 'PUSH_CONST 42', 'SET_FIELD value', 'STORE_LOCAL head']
        chain += ['NEW_STRUCT tree::node', 'LOAD_LOCAL head', 'SET_FIELD next', 'STORE_LOCAL head'] * (count - 1)
        chain += ['LOAD_LOCAL head'] + ['GET_FIELD next'] * (count - 1) + ['GET_FIELD value', 'RET']
        add(f'chain_{count}_nodes_no_recursive_walk', chain, layouts=(node,))
    linked = tuple(layout(f'level{i}', (('next', f'level{i + 1}'),)) for i in range(63)) + (layout('level63', (('value', 'i64'),)),)
    body = ['NEW_STRUCT level63', 'PUSH_CONST 42', 'SET_FIELD value', 'STORE_LOCAL tail']
    # Avoid changing one local's nominal kind as the chain is constructed.
    body += ['NEW_STRUCT level' + str(i) for i in range(63)]
    body += ['LOAD_LOCAL tail', 'SET_FIELD next'] + ['SET_FIELD next'] * 62
    body += ['GET_FIELD next'] * 63 + ['GET_FIELD value', 'RET']
    add('maximum_layout_dependency_chain', body, layouts=linked)
    add('caller_prefix_survives_nested_calls', ['PUSH_CONST 100', *initialized(42), 'CALL extract 1', 'ADD i64', 'RET'],
        helpers=(flow.function('extract', ['STORE_LOCAL owner', 'LOAD_LOCAL owner', 'GET_FIELD child', 'GET_FIELD value', 'RET'], parameters=(('owner', 'tree::outer'),)),),
        result=142, instructions=14)
    identity = flow.function('identity', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'RET'], parameters=(('item', 'tree::outer'),), result='tree::outer')
    maker = flow.function('make', initialized() + ['RET'], result='tree::outer')
    add('returned_parent_and_child_survive_frame', ['CALL make 0', *get_child()], helpers=(maker,), instructions=10)
    add('direct_nested_identity', initialized() + ['CALL identity 1', *get_child()], helpers=(identity,), instructions=12)
    add('indirect_nested_identity', ['PUSH_FN_PTR identity', 'STORE_LOCAL selected', *initialized(), 'CALL selected 1', *get_child()],
        helpers=(identity,), instructions=14)
    mutator = flow.function('mutate', ['STORE_LOCAL owner', 'LOAD_LOCAL owner', 'GET_FIELD child', 'PUSH_CONST 42', 'SET_FIELD value', 'POP', 'RET'],
        parameters=(('owner', 'tree::outer'),), result='void')
    add('void_helper_mutates_shared_nested_child', initialized(1) + ['STORE_LOCAL held', 'LOAD_LOCAL held', 'CALL mutate 1', 'LOAD_LOCAL held', *get_child()],
        helpers=(mutator,), instructions=19)
    for condition in (0, 1):
        add(f'same_nominal_nested_join_{condition}', [f'PUSH_CONST {condition}', 'JMP_IF_NOT alternate', *initialized(42), 'JMP joined', 'LABEL alternate',
            *initialized(7), 'LABEL joined', *get_child()], result=42 if condition else 7, instructions=12)
    for arg in ('i32', 'i64', 'bool', 'u8', 'f64', 'text'):
        name = f'pkg::box<{arg}>'
        box = layout(name, (('value', 'i64'),))
        add('concrete_record_argument_' + arg, [f'NEW_STRUCT {name}', 'PUSH_CONST 42', 'SET_FIELD value', 'GET_FIELD value', 'RET'],
            layouts=(box,), previous_error='manifest_syntax')
    generic_outer = layout('pkg::wrapper<i64>', (('child', 'pkg::box<i64>'),))
    generic_leaf = layout('pkg::box<i64>', (('value', 'i64'),))
    add('generic_forward_nested_record', ['NEW_STRUCT pkg::wrapper<i64>', 'NEW_STRUCT pkg::box<i64>', 'PUSH_CONST 42', 'SET_FIELD value',
        'SET_FIELD child', 'GET_FIELD child', 'GET_FIELD value', 'RET'], layouts=(generic_outer, generic_leaf), previous_error='manifest_syntax')
    for arg in ('i64', 'text'):
        typ = f'pkg::box<{arg}>'
        helper = flow.function('read', ['STORE_LOCAL box', 'LOAD_LOCAL box', 'GET_FIELD value', 'RET'], parameters=(('box', typ),))
        add('generic_nominal_parameter_' + arg, [f'NEW_STRUCT {typ}', 'PUSH_CONST 42', 'SET_FIELD value', 'CALL read 1', 'RET'],
            layouts=(layout(typ, (('value', 'i64'),)),), helpers=(helper,), instructions=9, previous_error='manifest_syntax')
    for arg in ('i32', 'i64', 'bool', 'u8', 'f64', 'text'):
        typ = f'pkg::choice<{arg}>'
        add('concrete_enum_argument_' + arg, ['PUSH_CONST 42', f'NEW_VARIANT pkg::choice::Present 1 {typ} i64', f'UNWRAP_VARIANT 1 {typ} i64', 'RET'],
            layouts=(), enums=(choice(typ),), previous_error='manifest_syntax')
    add('generic_unit_variant', ['NEW_VARIANT choice::Absent 0 choice<i64> void', 'IS_VARIANT choice::Absent choice<i64> void', 'RET'],
        layouts=(), enums=(choice(),), result=1, previous_error='manifest_syntax')
    add('generic_payload_variant_is_exact', ['PUSH_CONST 42', 'NEW_VARIANT choice::Present 1 choice<i64> i64', 'IS_VARIANT choice::Present choice<i64> i64', 'RET'],
        layouts=(), enums=(choice(),), result=1, previous_error='manifest_syntax')
    for typ in ('choice<i64>', 'choice<text>'):
        helper = flow.function('identity', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'RET'], parameters=(('item', typ),), result=typ)
        add('generic_enum_indirect_' + typ[7:-1], ['PUSH_FN_PTR identity', 'STORE_LOCAL selected', 'PUSH_CONST 42',
            f'NEW_VARIANT choice::Present 1 {typ} i64', 'CALL selected 1', f'UNWRAP_VARIANT 1 {typ} i64', 'RET'], layouts=(),
            enums=(choice('choice<i64>'), choice('choice<text>')), helpers=(helper,), instructions=10, previous_error='manifest_syntax')
    add('generic_parameter_is_nominal_not_substitution', ['PUSH_CONST 42', 'NEW_VARIANT choice::Present 1 choice<text> i64', 'UNWRAP_VARIANT 1 choice<text> i64', 'RET'],
        layouts=(), enums=(choice('choice<text>'),), previous_error='manifest_syntax')
    add('nested_records_keep_float_literal_metadata', initialized() + get_child(),
        helpers=(flow.function('unused', [floating('-0.0', '8000000000000000'), 'RET'], result='f64'),))
    debug = debug_proof.debug(0, 'nested.ny', (('held', 'held', 'tree::outer', 1),))
    add('source_debug_describes_nested_kind', [debug, *initialized(), *get_child()])
    add('record_annotation_cannot_supply_main_intent', initialized() + get_child(), annotated=False,
        layouts=(leaf, layout('tree::outer', outer[1], ('@ai_context("Describe a nested record")',))))
    add('nested_values_need_explicit_console_ceiling', initialized() + ['GET_FIELD child', 'GET_FIELD value', 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'],
        output=b'42\n', calls=1, ceiling=1)
    add('cyclic_layout_does_not_allocate_children', ['NEW_STRUCT tree::node', 'POP', 'PUSH_CONST 42', 'RET'], layouts=(node,))
    # Finite policy differences: reference missing-field zero and larger arenas.
    for name, body, used_layouts, instructions in (
        ('absent_nested_field', ['NEW_STRUCT tree::outer', 'GET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'], default_layouts, 2),
        ('absent_self_field', ['NEW_STRUCT tree::node', 'GET_FIELD next', 'POP', 'PUSH_CONST 42', 'RET'], (node,), 2),
        ('output_before_absent_field', ['PUSH_CONST 7', 'CONSOLE_WRITELN', 'NEW_STRUCT tree::outer', 'GET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'], default_layouts, 4)):
        add('policy_' + name, body, layouts=used_layouts, error='record_uninitialized', policy=True, reference_result=42, instructions=instructions,
            output=b'7\n' if name.startswith('output') else b'', calls=1 if name.startswith('output') else 0, ceiling=1 if name.startswith('output') else 0)
    add('policy_nested_field_descriptors_exhaust', ['NEW_STRUCT tree::node', 'POP'] * 129 + ['PUSH_CONST 42', 'RET'], layouts=(node,),
        error='record_limit', policy=True, reference_result=42, instructions=257)
    wide = layout('wide', tuple((f'field{i}', 'wide') for i in range(32)))
    add('nested_fields_exact_cell_capacity', ['NEW_STRUCT wide', 'POP'] * 32 + ['PUSH_CONST 42', 'RET'], layouts=(wide,))
    add('policy_nested_cells_exhaust', ['NEW_STRUCT wide', 'POP'] * 33 + ['PUSH_CONST 42', 'RET'], layouts=(wide,),
        error='record_limit', policy=True, reference_result=42, instructions=65)
    add('nested_aliases_after_descriptor_capacity', ['NEW_STRUCT tree::node', 'STORE_LOCAL held'] + ['NEW_STRUCT tree::node', 'POP'] * 127 +
        ['LOAD_LOCAL held', 'LOAD_LOCAL held', 'SET_FIELD next', 'GET_FIELD next', 'PUSH_CONST 42', 'SET_FIELD value', 'GET_FIELD value', 'RET'], layouts=(node,))
    add('policy_nested_finite_fuel', initialized() + get_child(), error='instruction_limit', policy=True, reference_result=42, steps=3, instructions=3)
    add('policy_nested_finite_frames', ['CALL make 0', *get_child()], helpers=(maker,), error='frame_limit', policy=True, reference_result=42, frames=1, instructions=1)
    add('nested_output_before_real_division_fault', initialized() + ['GET_FIELD child', 'GET_FIELD value', 'CONSOLE_WRITELN', 'PUSH_CONST 1', 'PUSH_CONST 0', 'DIV i64', 'RET'],
        output=b'42\n', calls=1, ceiling=1, error='division_by_zero', reference_error='Division by zero', instructions=11)
    for tag in ('Present', 'Absent'):
        arity, payload, prefix = ('1', 'i64', ['PUSH_CONST 7']) if tag == 'Present' else ('0', 'void', [])
        add('policy_erased_tag_cross_concrete_' + tag.lower(), prefix + [f'NEW_VARIANT choice::{tag} {arity} choice<i64> {payload}',
            f'IS_VARIANT choice::{tag} choice<text> {payload}', 'RET'], layouts=(), enums=(choice(), choice('choice<text>')),
            error='manifest_syntax', validation=True, admission_policy=True, reference_result=1, previous_error='manifest_syntax')
    # Whole-file/schema and finite nominal rejection boundaries.
    reject('undefined_forward_record', layouts=(layout('outer', (('child', 'missing'),)),), error='manifest_unsupported')
    reject('enum_is_not_record_field', layouts=(layout('outer', (('child', 'choice<i64>'),)),), enums=(choice(),), error='manifest_unsupported')
    for kind in ('i32', 'bool', 'u8', 'f64', 'text', 'String', 'array<i64>', 'ptr<i64>', 'ref<tree::leaf>'):
        reject('unadmitted_field_type_' + kind.replace('<', '_').replace('>', '').replace(':', '_'), layouts=(leaf, layout('outer', (('child', kind),))), error='manifest_unsupported')
    reject('duplicate_nested_field', layouts=(leaf, layout('outer', (('child', 'tree::leaf'), ('child', 'tree::leaf')))))
    reject('duplicate_concrete_record', layouts=(generic_leaf, generic_leaf))
    reject('duplicate_concrete_enum', layouts=(), enums=(choice(), choice()))
    for typ in ('box<>', 'box<', 'box<i64', 'box<i64>>', 'box<i64,text>', 'box<box<i64>>', 'box<T>', 'box<void>', 'box<String>', 'box< i64>', 'box<i64 >', 'box<i64>::tail'):
        reject('generic_spelling_' + str(len(cases)), layouts=(layout(typ),), error='manifest_unsupported')
    reject('stray_generic_closer', layouts=(layout('box>'),), error='manifest_syntax')
    reject('invalid_qualified_base', layouts=(layout('bad-name<i64>'),), error='manifest_syntax')
    for typ in ('i64<i64>', 'bool<i64>', 'text<i64>'):
        reject('reserved_record_base_' + typ.split('<')[0], layouts=(layout(typ),), error='manifest_unsupported')
    for typ in ('Option<i64>', 'pkg::Option<i64>', 'Result<text>'):
        reject('reserved_enum_base_' + typ.replace(':', '_').split('<')[0], layouts=(), enums=(choice(typ),), error='manifest_unsupported')
    exact_name = 'b' * 123 + '<i64>'
    add('concrete_name_exact_capacity', ['NEW_STRUCT ' + exact_name, 'POP', 'PUSH_CONST 42', 'RET'], layouts=(layout(exact_name),), previous_error='manifest_syntax')
    reject('concrete_name_over_capacity', layouts=(layout('b' * 124 + '<i64>'),), error='manifest_limit')
    add('nested_declared_fields_exact_capacity', ['PUSH_CONST 42', 'RET'], layouts=tuple(layout('r' + str(i), tuple((f'f{j}', 'r' + str((i + 1) % 8)) for j in range(32))) for i in range(8)))
    reject('nested_declared_fields_over_capacity', layouts=tuple(layout('r' + str(i), tuple((f'f{j}', 'r0') for j in range(32))) for i in range(8)) + (layout('extra', (('last', 'r0'),)),), error='manifest_limit')
    reject('nested_fields_per_record_over_capacity', layouts=(layout('r', tuple((f'f{i}', 'r') for i in range(33))),), error='manifest_limit')
    reject('nested_layouts_over_capacity', layouts=linked + (layout('extra'),), error='manifest_limit')
    for name, value in [('integer', ['PUSH_CONST 0']), ('bool', ['PUSH_CONST 1', 'CAST bool']), ('byte', ['PUSH_CONST 1', 'CAST u8']),
                        ('float', [floating('0.5', '3fe0000000000000')]), ('text', ['PUSH_CONST_STR S0']), ('array', ['NEW_ARRAY_LITERAL 0 i64'])]:
        reject('nested_field_rejects_' + name, ['NEW_STRUCT tree::outer', *value, 'SET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'], strings=('text',))
    reject('nested_field_rejects_other_record', ['NEW_STRUCT tree::outer', 'NEW_STRUCT tree::outer', 'SET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('integer_field_rejects_nested_handle', ['NEW_STRUCT tree::outer', 'NEW_STRUCT tree::leaf', 'SET_FIELD count', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('nested_get_does_not_become_integer', initialized() + ['GET_FIELD child', 'RET'])
    reject('nested_get_does_not_become_console_authority', initialized() + ['GET_FIELD child', 'CONSOLE_WRITE', 'PUSH_CONST 42', 'RET'], ceiling=1)
    reject('nested_get_no_implicit_array_element', initialized() + ['GET_FIELD child', 'NEW_ARRAY_LITERAL 1 i64', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('nested_set_underflow', ['NEW_STRUCT tree::outer', 'SET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('nested_get_underflow', ['GET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('callee_cannot_pop_caller_record', initialized() + ['CALL bad 0', 'POP', 'PUSH_CONST 42', 'RET'],
        helpers=(flow.function('bad', ['GET_FIELD child', 'POP', 'RET'], result='void'),))
    reject('callee_cannot_use_caller_set_operand', initialized() + ['NEW_STRUCT tree::leaf', 'CALL bad 1', 'POP', 'PUSH_CONST 42', 'RET'],
        helpers=(flow.function('bad', ['SET_FIELD child', 'POP', 'RET'], parameters=(('item', 'tree::leaf'),), result='void'),))
    reject('different_nested_kind_join', ['PUSH_CONST 1', 'JMP_IF alternate', 'NEW_STRUCT tree::leaf', 'JMP joined', 'LABEL alternate', 'NEW_STRUCT tree::outer',
        'LABEL joined', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('different_nested_kind_local_reassignment', ['NEW_STRUCT tree::leaf', 'STORE_LOCAL held', 'NEW_STRUCT tree::outer', 'STORE_LOCAL held', 'PUSH_CONST 42', 'RET'])
    reject('unused_helper_invalid_nested_field', ['PUSH_CONST 42', 'RET'], helpers=(flow.function('unused', ['NEW_STRUCT tree::outer', 'PUSH_CONST 1', 'SET_FIELD child', 'POP', 'PUSH_CONST 0', 'RET']),))
    reject('dead_unknown_nested_field', ['PUSH_CONST 42', 'RET', 'GET_FIELD absent'], error='manifest_unsupported')
    reject('dead_unknown_record', ['PUSH_CONST 42', 'RET', 'NEW_STRUCT missing<i64>'], error='manifest_unsupported')
    reject('hygienic_locals_remain_out', initialized() + ['STORE_LOCAL $ny.local.0.owner', 'PUSH_CONST 42', 'RET'])
    generic_other = layout('pkg::box<text>', (('value', 'i64'),))
    for mode in ('direct', 'indirect'):
        identity_generic = flow.function('identity', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'RET'], parameters=(('item', 'pkg::box<i64>'),), result='pkg::box<i64>')
        prefix = ['PUSH_FN_PTR identity', 'STORE_LOCAL selected'] if mode == 'indirect' else []
        reject('different_generic_record_argument_' + mode, prefix + ['NEW_STRUCT pkg::box<text>', 'CALL ' + ('selected' if mode == 'indirect' else 'identity') + ' 1',
            'POP', 'PUSH_CONST 42', 'RET'], layouts=(generic_leaf, generic_other), helpers=(identity_generic,))
    reject('different_generic_record_return', ['PUSH_CONST 42', 'RET'], layouts=(generic_leaf, generic_other),
        helpers=(flow.function('unused', ['NEW_STRUCT pkg::box<text>', 'RET'], result='pkg::box<i64>'),))
    reject('different_generic_record_field', ['NEW_STRUCT pkg::wrapper<i64>', 'NEW_STRUCT pkg::box<text>', 'SET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET'],
        layouts=(generic_outer, generic_leaf, generic_other))
    first = flow.function('first', ['STORE_LOCAL item', 'PUSH_CONST 42', 'RET'], parameters=(('item', 'pkg::box<i64>'),))
    second = flow.function('second', ['STORE_LOCAL item', 'PUSH_CONST 42', 'RET'], parameters=(('item', 'pkg::box<text>'),))
    reject('generic_parameter_only_callable_signature_mismatch', ['PUSH_FN_PTR first', 'STORE_LOCAL selected', 'PUSH_FN_PTR second', 'STORE_LOCAL selected', 'PUSH_CONST 42', 'RET'],
        layouts=(generic_leaf, generic_other), helpers=(first, second))
    for name, line, error in [('tag_has_suffix', 'NEW_VARIANT choice<i64>::Absent 0 choice<i64> void', 'manifest_unsupported'),
                              ('tag_owner_disagrees', 'NEW_VARIANT wrong::Absent 0 choice<i64> void', 'manifest_syntax'),
                              ('unknown_variant', 'NEW_VARIANT choice::Missing 0 choice<i64> void', 'manifest_unsupported'),
                              ('missing_concrete_metadata', 'NEW_VARIANT choice::Absent 0', 'manifest_unsupported'),
                              ('wrong_payload_type', 'NEW_VARIANT choice::Absent 0 choice<i64> i64', 'manifest_syntax'),
                              ('wrong_payload_count', 'NEW_VARIANT choice::Absent 1 choice<i64> void', 'manifest_syntax')]:
        reject('generic_enum_' + name, [line, 'POP', 'PUSH_CONST 42', 'RET'], layouts=(), enums=(choice(),), error=error)
    reject('generic_enum_unwrap_wrong_instantiation', ['PUSH_CONST 42', 'NEW_VARIANT choice::Present 1 choice<i64> i64', 'UNWRAP_VARIANT 1 choice<text> i64', 'RET'],
        layouts=(), enums=(choice(), choice('choice<text>')))
    reject('nested_records_do_not_grant_console', initialized() + ['GET_FIELD child', 'GET_FIELD value', 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], error='manifest_ceiling')
    reject('dead_nested_console_still_needs_ceiling', initialized() + ['POP', 'PUSH_CONST 42', 'RET', 'CONSOLE_WRITELN'], error='manifest_ceiling')
    reject('dead_foreign_stays_excluded', initialized() + ['POP', 'PUSH_CONST 42', 'RET', 'INTEROP_CALL libc puts 1 i64'], error='manifest_unsupported')
    other_leaf = layout('tree::other_leaf', (('value', 'i64'),))
    other_outer = layout('tree::other_outer', (('child', 'tree::other_leaf'),))
    add('same_field_name_keeps_receiver_specific_kind', initialized(40) + ['STORE_LOCAL first', 'NEW_STRUCT tree::other_outer',
        'NEW_STRUCT tree::other_leaf', 'PUSH_CONST 2', 'SET_FIELD value', 'SET_FIELD child', 'GET_FIELD child', 'GET_FIELD value',
        'LOAD_LOCAL first', 'GET_FIELD child', 'GET_FIELD value', 'ADD i64', 'RET'], layouts=(outer, other_outer, other_leaf, leaf))
    reject('same_field_name_cannot_confuse_receiver_kind', ['NEW_STRUCT tree::outer', 'NEW_STRUCT tree::other_leaf', 'SET_FIELD child',
        'POP', 'PUSH_CONST 42', 'RET'], layouts=(outer, other_outer, other_leaf, leaf))
    add('generic_and_plain_record_bases_stay_distinct', ['NEW_STRUCT box', 'PUSH_CONST 40', 'SET_FIELD value', 'GET_FIELD value',
        'NEW_STRUCT box<i64>', 'PUSH_CONST 2', 'SET_FIELD value', 'GET_FIELD value', 'ADD i64', 'RET'],
        layouts=(layout('box', (('value', 'i64'),)), layout('box<i64>', (('value', 'i64'),))), previous_error='manifest_syntax')
    generic_node = layout('node<i64>', (('next', 'node<i64>'), ('value', 'i64')))
    add('generic_self_cycle_alias', ['NEW_STRUCT node<i64>', 'STORE_LOCAL root', 'LOAD_LOCAL root', 'LOAD_LOCAL root', 'SET_FIELD next',
        'GET_FIELD next', 'PUSH_CONST 42', 'SET_FIELD value', 'GET_FIELD value', 'RET'], layouts=(generic_node,), previous_error='manifest_syntax')
    another_maker = flow.function('another', initialized(7) + ['RET'], result='tree::outer')
    add('same_nested_callable_signature_reassignment', ['PUSH_FN_PTR another', 'STORE_LOCAL selected', 'PUSH_FN_PTR make', 'STORE_LOCAL selected',
        'CALL selected 0', *get_child()], helpers=(maker, another_maker), instructions=14)
    reject('generic_return_only_callable_signature_mismatch', ['PUSH_FN_PTR first', 'STORE_LOCAL selected', 'PUSH_FN_PTR second', 'STORE_LOCAL selected',
        'PUSH_CONST 42', 'RET'], layouts=(generic_leaf, generic_other), helpers=(
            flow.function('first', ['NEW_STRUCT pkg::box<i64>', 'RET'], result='pkg::box<i64>'),
            flow.function('second', ['NEW_STRUCT pkg::box<text>', 'RET'], result='pkg::box<text>')))
    reject('generic_enum_parameter_only_signature_mismatch', ['PUSH_FN_PTR first', 'STORE_LOCAL selected', 'PUSH_FN_PTR second', 'STORE_LOCAL selected',
        'PUSH_CONST 42', 'RET'], layouts=(), enums=(choice(), choice('choice<text>')), helpers=(
            flow.function('first', ['STORE_LOCAL item', 'PUSH_CONST 42', 'RET'], parameters=(('item', 'choice<i64>'),)),
            flow.function('second', ['STORE_LOCAL item', 'PUSH_CONST 42', 'RET'], parameters=(('item', 'choice<text>'),))))
    for condition in (0, 1):
        body = ['NEW_STRUCT tree::outer', 'STORE_LOCAL held', f'PUSH_CONST {condition}', 'JMP_IF_NOT joined',
            'LOAD_LOCAL held', 'NEW_STRUCT tree::leaf', 'PUSH_CONST 42', 'SET_FIELD value', 'SET_FIELD child', 'POP',
            'LABEL joined', 'LOAD_LOCAL held', 'GET_FIELD child', 'POP', 'PUSH_CONST 42', 'RET']
        add('nested_initialization_after_cfg_join_' + str(condition), body, instructions=16 if condition else 7,
            error=None if condition else 'record_uninitialized', policy=not bool(condition), reference_result=None if condition else 42)
    for base in ('ptr', 'ref', 'owned', 'borrow', 'fn', 'tuple', 'REF'):
        reject('reserved_wrapper_base_' + base, layouts=(layout(base + '<i64>'),), error='manifest_unsupported')
    add('qualified_wrapper_word_remains_nominal', ['NEW_STRUCT app::ref<i64>', 'PUSH_CONST 42', 'SET_FIELD value', 'GET_FIELD value', 'RET'],
        layouts=(layout('app::ref<i64>', (('value', 'i64'),)),), previous_error='manifest_syntax')
    add('plain_wrapper_word_preserves_previous_grammar', ['NEW_STRUCT ref', 'PUSH_CONST 42', 'SET_FIELD value', 'GET_FIELD value', 'RET'],
        layouts=(layout('ref', (('value', 'i64'),)),), cross_profile=False)
    reject('overlong_generic_tag_checks_length_first', ['NEW_VARIANT ' + 'choice<i64>::' + 'V' * 116 + ' 0 choice<i64> void', 'POP', 'PUSH_CONST 42', 'RET'],
        layouts=(), enums=(choice(),), error='manifest_limit')
    require(len(cases) == len({case['name'] for case in cases}), 'duplicate boundary names')
    return cases


def compare_case(case, candidate, reference, production):
    status = 'validation_refused' if case['validation'] else 'execution_refused' if case['error'] else 'returned'
    require(candidate['status'] == status, 'raw outcome differs from expectation')
    for field in ('result', 'error', 'console_calls', 'instructions', 'source_debug', 'f64_literals'):
        require(candidate[field] == case[field], 'raw ' + field + ' differs from expectation')
    require(candidate['output_hex'] == case['output'].hex() and candidate['output_bytes'] == len(case['output']), 'raw output differs')
    if case['production_error']:
        require(production['status'] == 'admission_refused' and production['error'] == case['production_error'] and
                production['source_debug'] == [] and production['f64_literals'] == [], 'production admission contract changed')
    else:
        require(production == candidate, 'production and raw dispatch/metadata differ')
    if case['validation'] and not case['admission_policy']:
        require(reference is None, 'validation-only fixture ran reference')
        return 'expected_validation_refusal'
    require(reference is not None and reference['status'] == ('execution_refused' if case['reference_error'] else 'returned'), 'finite reference status differs')
    require(reference['error'] == case['reference_error'], 'finite reference error differs')
    expected_result = case['reference_result'] if case['policy'] or case['admission_policy'] else case['result']
    expected_output = case['reference_output'] if case['reference_output'] is not None else case['output']
    require(reference['result'] == expected_result and reference['output_hex'] == expected_output.hex(), 'finite reference semantics differ')
    require(reference['source_debug'] == case['reference_source_debug'], 'reference debug metadata differs')
    require(reference['f64_literals'] == case['reference_f64_literals'], 'reference binary64 literals differ')
    if case['admission_policy']:
        return 'finite_admission_policy_contrast'
    return 'finite_runtime_policy_contrast' if case['policy'] else 'matching_runtime_refusal' if case['error'] else 'matching_semantics'


def boundary_proof(args, output, environment):
    directory = output / 'boundary-editions'; directory.mkdir()
    results, failures = [], []
    for number, case in enumerate(generated_cases()):
        path = directory / f"{number:03d}-{case['name']}.nil"
        path.write_bytes(case['edition'])
        record = {key: value for key, value in case.items() if key not in {'edition', 'output', 'reference_output'}}
        record.update(edition_bytes=len(case['edition']), edition_sha256=digest(case['edition']), retained_path=str(path.relative_to(output)),
                      expected_output_hex=case['output'].hex(), reference_output_hex=None if case['reference_output'] is None else case['reference_output'].hex())
        try:
            values = {}
            for name, tool, parser in (('candidate', args.candidate, parse_probe), ('production', args.production_candidate, parse_production)):
                prefix = 'raw' if name == 'candidate' else 'production'
                process, stdout, _ = baseline.run([tool, path, str(case['ceiling']), str(case['steps']), str(case['frames'])], output / f'boundary-{prefix}-{number:03d}', environment=environment)
                values[name] = parser(process, stdout)
            reference = None
            if not case['validation'] or case['admission_policy']:
                process, stdout, _ = baseline.run([args.reference, path], output / f'boundary-reference-{number:03d}', environment=environment)
                reference = parse_probe(process, stdout, reference=True)
            record.update(values, reference=reference)
            record['outcome'] = compare_case(case, values['candidate'], reference, values['production'])
            if case['cross_profile']:
                for name, tool, parser, status in (('previous_typed_scalars', args.typed_scalars_candidate, typed_proof.parse_production, 'admission_refused'),
                                                   ('previous_typed_scalars_raw', args.typed_scalars_conformance_candidate, typed_proof.parse_probe, 'validation_refused')):
                    process, stdout, _ = baseline.run([tool, path, str(case['ceiling'])], output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = case['previous_error']
                    require(previous['status'] == status and previous['error'] == expected, 'earlier typed-scalar profile expanded')
                    record[name] = previous
        except (AssertionError, ValueError, OSError) as error:
            record.update(outcome='failed', failure=str(error)); failures.append(case['name'])
        results.append(record)
    write_json(output / 'boundary-results.json', {'cases': results, 'failures': failures})
    return {'passed': not failures, 'cases': len(results), 'failures': failures,
            'semantic_matches': sum(item['outcome'] == 'matching_semantics' for item in results),
            'matching_runtime_refusals': sum(item['outcome'] == 'matching_runtime_refusal' for item in results),
            'finite_runtime_policy_contrasts': sum(item['outcome'] == 'finite_runtime_policy_contrast' for item in results),
            'finite_admission_policy_contrasts': sum(item['outcome'] == 'finite_admission_policy_contrast' for item in results),
            'expected_validation_refusals': sum(item['outcome'] == 'expected_validation_refusal' for item in results),
            'production_comparisons': sum('production' in item for item in results),
            'reference_comparisons': sum(item.get('reference') is not None for item in results),
            'f64_literal_comparisons': sum(len(item['reference']['f64_literals']) for item in results if item.get('reference') is not None),
            'typed_scalars_refusals': sum('previous_typed_scalars' in item for item in results),
            'typed_scalars_conformance_refusals': sum('previous_typed_scalars_raw' in item for item in results)}


def instruction_inventory(data):
    """Lexical metadata only; no declaration, type, alias or CFG proof."""
    result = typed_proof.instruction_inventory(data)
    result.update(profile=12, lane='raw_nested_records_conformance',
                  scope='profile12 lexical coverage only; resolved declarations, exact field and generic identity, authority and CFG require full validation')
    if result['unsupported_format']:
        return result
    identifier = r'[A-Za-z_][A-Za-z_0-9]*'
    base = identifier + r'(?:::' + identifier + r')*'
    nominal = base + r'(?:<(?:i32|i64|bool|u8|f64|text)>)?'
    lines = data.decode('utf-8').splitlines()
    record_names, enum_names = set(), set()
    for line in lines:
        match = re.fullmatch(r'(struct|enum) (' + nominal + r')', line.strip())
        if match:
            (record_names if match[1] == 'struct' else enum_names).add(match[2])
    types = ['i32', 'i64', 'bool', 'u8', 'f64', 'text', 'array<i64>', 'array<text>', *sorted(record_names | enum_names)]
    kind = '(?:' + '|'.join(re.escape(value) for value in types) + ')'
    header = re.compile(r'function (' + identifier + r')(?:\(([^()]*)\))? -> (' + kind + r'|void):')
    functions, supported_lines = {}, set()
    active = None
    for number, line in enumerate(lines, 1):
        stripped = line.strip()
        match = re.fullmatch(r'(struct|enum) (' + nominal + r')', stripped)
        if match:
            active = match[1]; supported_lines.add(number); continue
        if active:
            if stripped == 'end ' + active:
                supported_lines.add(number); active = None
            elif active == 'struct' and ':' in stripped:
                name, field = stripped.split(':', 1)
                if re.fullmatch(identifier, name.strip()) and field.strip() in {'i64', *record_names}:
                    supported_lines.add(number)
            elif active == 'enum' and re.fullmatch(identifier + r'(?:: i64)?', stripped):
                supported_lines.add(number)
            continue
        match = header.fullmatch(stripped)
        if match:
            parameters = match[2].split(', ') if match[2] else []
            if all(re.fullmatch(identifier + r': ' + kind, parameter) for parameter in parameters):
                functions[match[1]] = len(parameters); supported_lines.add(number)
    result['unsupported_declarations'] = [item for item in result['unsupported_declarations'] if item['line'] not in supported_lines]
    for item in result['instructions']:
        opcode, operands = item['opcode'], item['operands']
        if opcode == 'NEW_STRUCT' and len(operands) == 1 and operands[0] in record_names:
            item['supported_opcode'] = item['supported_form'] = True
        elif opcode in {'NEW_VARIANT', 'IS_VARIANT'} and len(operands) == (4 if opcode == 'NEW_VARIANT' else 3):
            concrete, payload = operands[-2:]
            item['supported_form'] = concrete in enum_names and payload in {'i64', 'void'} and bool(re.fullmatch(base + '::' + identifier, operands[0]))
        elif opcode == 'UNWRAP_VARIANT' and len(operands) == 3:
            item['supported_form'] = operands[1] in enum_names and operands[2] in {'i64', 'void'}
        elif opcode == 'CALL' and len(operands) == 2 and operands[0] in functions:
            item['supported_form'] = operands[1] == str(functions[operands[0]])
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
    if [item['invocation'] for item in records] != list(range(45)):
        failures.append('raw invocation sequence changed')
    matched = [item['invocation'] for item in records if item.get('matched') is True]
    if matched != expected_raw_matches:
        failures.append('known raw semantic coverage changed')
    for record in records:
        number = record['invocation']; expected = known_raw_matches.get(number)
        if expected is not None and (record.get('edition_bytes'), record.get('edition_sha256')) != expected:
            failures.append(f'known complete edition changed: {number}')
        if record.get('outcome') != ('returned_match' if number in expected_raw_matches else 'validation_refused'):
            failures.append(f'known corpus outcome changed: {number}')
        if number in (4, 7, 42) and record.get('target_verified') is not True:
            failures.append(f'complete target not independently verified: {number}')
        if number in known_raw_matches and record.get('candidate', {}).get('source_debug') != []:
            failures.append(f'non-debug complete edition gained metadata: {number}')
        if number in expected_raw_matches and number != 42 and record.get('candidate', {}).get('f64_literals') != []:
            failures.append(f'previous scalar-free edition gained floats: {number}')
    errors = dict(Counter(item['candidate']['error'] for item in records if item.get('outcome') == 'validation_refused'))
    expected_errors = {'manifest_unsupported': 16, 'manifest_syntax': 2, 'manifest_edition': 9}
    if errors != expected_errors:
        failures.append('raw validation refusal inventory changed')
    return {'passed': not failures, 'failures': failures, 'matched_invocations': matched,
            'expected_matched_invocations': expected_raw_matches, 'expected_refusal_errors': expected_errors,
            'edition_pins': [{'invocation': item['invocation'], 'bytes': item.get('edition_bytes'), 'sha256': item.get('edition_sha256')}
                             for item in records if item['invocation'] in expected_raw_matches],
            'temporary_source_paths': 'Invocation4 retains its exact fresh capture and is never normalized.'}


def compare_aggregate_target(target, reference):
    require((target['edition_bytes'], target['edition_sha256']) == known_raw_matches[7], 'complete nested target changed')
    require(target['invocation'] == 7 and target['root_case'] == 'aggregates-twin.ny' and target['category'] == 'root_execution', 'aggregate root identity changed')
    for value in (target['candidate'], reference):
        require(value['status'] == 'returned' and value['result'] == 0 and value['error'] is None and value['output_hex'] == aggregate_output.hex(),
                'complete aggregate target full-result semantics differ')
        require(value['source_debug'] == [] and value['f64_literals'] == [], 'complete aggregate target gained metadata')
    require(target['candidate']['instructions'] == 150 and target['candidate']['console_calls'] == 9 and
            target['candidate']['output_bytes'] == len(aggregate_output), 'complete aggregate target accounting differs')


def verify_complete_targets(args, output, environment, records):
    calls = baseline.strict_json((output / 'corpus-calls.json').read_text())
    for number in (4, 7, 42):
        target = records[number]; path = output / target['retained_path']; data = path.read_bytes()
        require(len(data) == target['edition_bytes'] and digest(data) == target['edition_sha256'], 'retained complete target changed')
        process, stdout, _ = baseline.run([args.reference, path], output / f'corpus-full-reference-{number:03d}', environment=environment)
        reference = parse_probe(process, stdout, reference=True)
        if number == 4:
            source_path, metadata = debug_proof.compare_target_receipt(target, calls[53], reference)
            require(target['candidate']['f64_literals'] == reference['f64_literals'] == [], 'old debug target gained floats')
            receipt = {'invocation': 4, 'source_path': source_path, 'source_debug': metadata, 'paths_normalized': False}
        elif number == 42:
            receipt = {'invocation': 42, 'f64_literals': typed_proof.compare_scalar_target(target, reference), 'root_case': 'text-and-types.matrix',
                       'functions': 6, 'unused_helpers_preserved': True}
        else:
            compare_aggregate_target(target, reference)
            receipt = {'invocation': 7, 'root_case': 'aggregates-twin.ny', 'functions': 2, 'record_layouts': 3, 'enum_layouts': 2,
                       'complete_nested_and_concrete_generic_artifact': True}
        receipt.update(edition_bytes=len(data), edition_sha256=digest(data), retained_path=target['retained_path'],
                       candidate=target['candidate'], reference=reference, passed=True, result_comparison='full_signed_i64_zero_against_independent_reference')
        write_json(output / f'complete-target-{number:03d}.json', receipt)
        target['target_verified'] = True


def qualify_corpus(args, output, environment, before):
    with corpus_adapters():
        raw = baseline.corpus_proof(args, output, environment, before)
    records = baseline.strict_json((output / 'corpus-executions.json').read_text())
    verify_complete_targets(args, output, environment, records)
    for record in records:
        record['lane'] = 'raw_nested_records_conformance'
    raw.update(lane='raw_nested_records_conformance',
               unsupported_or_refused_executions=sum(item['outcome'] in {'validation_refused', 'execution_refused'} for item in records),
               refusal_errors=dict(Counter(item['candidate']['error'] for item in records if item['outcome'] in {'validation_refused', 'execution_refused'})),
               scope='Exact unchanged upstream semantics; production intent is a separate lane. Refusal is unmet coverage.',
               regression_contract=raw_regression_contract(records))
    write_json(output / 'corpus-executions.json', records); write_json(output / 'corpus-summary.json', raw)
    production_records, failures, cross = [], [], []
    for record in records:
        number = record['invocation']
        item = {key: record[key] for key in ('invocation', 'upstream_call', 'edition_sha256', 'edition_bytes', 'retained_path')}
        item['lane'] = 'production_profile12_admission'
        try:
            path = output / record['retained_path']; data = path.read_bytes()
            require(len(data) == record['edition_bytes'] and digest(data) == record['edition_sha256'], 'corpus bytes changed between lanes')
            process, stdout, _ = baseline.run([args.production_candidate, path, '1'], output / f'corpus-production-{number:03d}', environment=environment)
            candidate = parse_production(process, stdout); item['candidate'] = candidate
            require(candidate['status'] == 'admission_refused', 'unmodified corpus unexpectedly passed production admission')
            item['outcome'] = 'production_refused'
            if number == 7:
                pair = dict(item, lane='prior_typed_scalars_grammar')
                for name, tool, parser, status in (('previous_typed_scalars', args.typed_scalars_candidate, typed_proof.parse_production, 'admission_refused'),
                                                   ('previous_typed_scalars_raw', args.typed_scalars_conformance_candidate, typed_proof.parse_probe, 'validation_refused')):
                    process, stdout, _ = baseline.run([tool, path, '1'], output / f"corpus-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    require(previous['status'] == status and previous['error'] == 'manifest_unsupported', 'previous complete nested target contract changed')
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome='failed', failure=str(error)); failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item['candidate']['error'] for item in production_records if item['outcome'] == 'production_refused'))
    expected = {'manifest_unsupported': 13, 'manifest_syntax': 2, 'manifest_edition': 9, 'manifest_intent': 21}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 1
    production = {'lane': 'production_profile12_admission', 'passed': passed, 'invocations': len(production_records), 'failures': failures,
                  'refusals': sum(item['outcome'] == 'production_refused' for item in production_records),
                  'refusal_errors': errors, 'expected_refusal_errors': expected, 'typed_scalars_corpus_comparisons': len(cross),
                  'scope': 'Production admission attempts on identical complete editions; no raw semantic coverage credit'}
    write_json(output / 'production-corpus-executions.json', production_records); write_json(output / 'production-corpus-summary.json', production)
    write_json(output / 'typed-scalars-corpus-executions.json', cross)
    return raw, production


def qualification(args):
    binary_names = ('candidate', 'production_candidate', 'typed_scalars_candidate', 'typed_scalars_conformance_candidate', 'reference', 'compiler', 'interpreter')
    for name in (*binary_names[:-1], 'root', 'output'):
        setattr(args, name, getattr(args, name).resolve())
    args.interpreter = (args.interpreter or args.compiler.with_name('nilinterpreter')).resolve()
    require(not args.output.exists(), 'output must be a new directory; evidence is never overwritten')
    args.output.mkdir(parents=True); args.output_created = True
    tools = {name: baseline.record_tool(getattr(args, name)) for name in binary_names}
    require(tools['compiler']['sha256'] == baseline.compiler_hash and tools['interpreter']['sha256'] == baseline.interpreter_hash, 'qualified toolchain pins changed')
    require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=args.root, text=True).strip() == baseline.revision, 'qualified upstream revision changed')
    require(not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=args.root, text=True), 'qualified tracked upstream source changed')
    tools['reference_core_archives'], tools['earlier_profile_sources'], tools['primary_sources'] = [], [], []
    for name, expected in archive_pins.items():
        record = baseline.record_tool(args.root / 'build/context-registration/lib' / name)
        require(record['sha256'] == expected, 'qualified archive changed: ' + name); tools['reference_core_archives'].append(record)
    for name, expected in source_pins.items():
        record = baseline.record_tool(Path(__file__).with_name(name))
        require(record['sha256'] == expected, 'earlier proof source changed: ' + name); tools['earlier_profile_sources'].append(record)
    for name, expected in primary_pins.items():
        record = baseline.record_tool(args.root / name)
        require(record['sha256'] == expected, 'primary source changed: ' + name); tools['primary_sources'].append(record)
    tools['reference_source'] = baseline.record_tool(Path(__file__).with_name('program_nested_records_reference.cpp'))
    tools['script'] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / 'tools.json', tools); write_json(args.output / 'corpus-inventory.json', before)
    write_json(args.output / 'environment-policy.json', {
        'NIL_CRASH_REPORT': '0', 'removed': ['NY_LIBRARY_PATH', 'NIL_LIBRARY_PATH', 'NY_LEX_DUMP', 'NY_PARSE_TRACE', 'NY_STDLIB_PATH', 'NIL_MEMORY_PROFILE', 'NIL_STRICT_BOUNDS'],
        'upstream_argv': 'unchanged; original capture wrappers forward all arguments',
        'raw_lane': 'explicit validate_nested_records host entry; only missing original main intent is relaxed',
        'production_lane': 'explicit profile12 encoder/admission/execution on the same bytes',
        'limits': {'edition_bytes': 65536, 'static_instructions': 4096, 'steps': 65536, 'frames': 16, 'output_bytes': 65536,
                   'record_layouts': 64, 'fields_per_record': 32, 'declared_record_fields': 256,
                   'runtime_records': 128, 'runtime_record_fields': 1024, 'name_bytes': 128},
        'nested_fields': 'Exact nominal record handles preserve shared aliases; allocation does not recursively allocate children; missing record field is an explicit record_uninitialized policy fault.',
        'generic_names': 'One terminal primitive argument is a nominal discriminator, not template substitution; exact concrete enum identity remains distinct despite reference erased-tag comparison.',
        'earlier_proofs': 'All twelve preceding proof suites remain unchanged separate obligations;23 prior proof/oracle sources are pinned.',
    })
    environment = baseline.clean_environment()
    boundary = boundary_proof(args, args.output, environment)
    raw, production = qualify_corpus(args, args.output, environment, before)
    require(baseline.inventory(args.root) == before, 'upstream inputs changed during proof')
    for name in binary_names:
        require(baseline.record_tool(getattr(args, name)) == tools[name], 'tool changed: ' + name)
    for record in [*tools['reference_core_archives'], *tools['earlier_profile_sources'], *tools['primary_sources'], tools['reference_source'], tools['script']]:
        require(baseline.record_tool(Path(record['path'])) == record, 'proof source/archive changed')
    for number, case in enumerate(generated_cases()):
        require((args.output / 'boundary-editions' / f"{number:03d}-{case['name']}.nil").read_bytes() == case['edition'], 'boundary input changed during proof')
    passed = baseline.checkpoint_passed(boundary, raw) and raw['regression_contract']['passed'] and production['passed']
    result = {'schema': 1, 'status': 'bounded_nested_records_checkpoint_passed' if passed else 'bounded_nested_records_checkpoint_failed',
              'candidate_qualified': False, 'native_execution': False, 'production_admission_waived': False, 'language_profile': 12,
              'boundary_results': boundary, 'raw_corpus': raw, 'production_corpus': production,
              'input_manifest_sha256': before['input_manifest_sha256'], 'tools': tools}
    write_json(args.output / 'summary.json', result); print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_explicit_inventory_and_immutable_sources(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 161)
        self.assertEqual(len({case['name'] for case in cases}), 161)
        self.assertEqual(sum(not c['error'] for c in cases), 65)
        self.assertEqual(sum(c['policy'] for c in cases), 8)
        self.assertEqual(sum(c['admission_policy'] for c in cases), 2)
        self.assertEqual(sum(c['validation'] and not c['admission_policy'] for c in cases), 85)
        self.assertEqual(sum(bool(c['error']) and not c['validation'] and not c['policy'] for c in cases), 1)
        self.assertEqual(sum(c['cross_profile'] for c in cases), 74)
        self.assertEqual(sum(not c['validation'] or c['admission_policy'] for c in cases), 76)
        self.assertEqual(len(source_pins), 23)
        self.assertEqual(len(primary_pins), 11)
        self.assertEqual(len(expected_raw_matches), 18)
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)
        self.assertTrue(all(len(c['edition']) <= 65536 and c['origin'] == 'independently_authored_textual_nil' for c in cases))

    @staticmethod
    def values(case):
        candidate = {'status': 'validation_refused' if case['validation'] else 'execution_refused' if case['error'] else 'returned',
            'result': case['result'], 'error': case['error'], 'output_hex': case['output'].hex(), 'instructions': case['instructions'],
            'console_calls': case['console_calls'], 'output_bytes': len(case['output']), 'source_debug': copy.deepcopy(case['source_debug']),
            'f64_literals': copy.deepcopy(case['f64_literals'])}
        reference = {'schema': 1, 'status': 'execution_refused' if case['reference_error'] else 'returned',
            'result': case['reference_result'] if case['policy'] or case['admission_policy'] else case['result'],
            'error': case['reference_error'], 'output_hex': (case['reference_output'] if case['reference_output'] is not None else case['output']).hex(),
            'source_debug': copy.deepcopy(case['reference_source_debug']), 'f64_literals': copy.deepcopy(case['reference_f64_literals'])}
        production = dict(candidate)
        if case['production_error']:
            production.update(status='admission_refused', result=None, error=case['production_error'], instructions=0, console_calls=0,
                              output_hex='', output_bytes=0, source_debug=[], f64_literals=[])
        return candidate, reference, production

    def test_all_authored_expectations_have_consistent_comparison_shapes(self):
        for case in generated_cases():
            candidate, reference, production = self.values(case)
            if case['validation'] and not case['admission_policy']:
                reference = None
            expected = ('finite_admission_policy_contrast' if case['admission_policy'] else 'expected_validation_refusal' if case['validation'] else
                        'finite_runtime_policy_contrast' if case['policy'] else 'matching_runtime_refusal' if case['error'] else 'matching_semantics')
            self.assertEqual(compare_case(case, candidate, reference, production), expected, case['name'])

    def test_missing_record_field_policy_cannot_be_reported_as_matching_fault(self):
        case = next(c for c in generated_cases() if c['name'] == 'policy_absent_nested_field')
        candidate, reference, production = self.values(case)
        self.assertEqual(compare_case(case, candidate, reference, production), 'finite_runtime_policy_contrast')
        for changed in (dict(reference, status='execution_refused', result=None, error='record_uninitialized'),
                        dict(reference, result=0), dict(reference, error='timeout')):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, production)
        for changed in (dict(candidate, status='returned', result=42, error=None), dict(candidate, error='record_limit'), dict(candidate, instructions=3)):
            with self.assertRaises(AssertionError):
                compare_case(case, changed, reference, changed)

    def test_erased_generic_tag_policy_requires_actual_true_reference(self):
        case = next(c for c in generated_cases() if c['name'] == 'policy_erased_tag_cross_concrete_present')
        candidate, reference, production = self.values(case)
        self.assertEqual(compare_case(case, candidate, reference, production), 'finite_admission_policy_contrast')
        for changed in (dict(reference, result=0), dict(reference, status='guard_refused', result=None),
                        dict(reference, status='execution_refused', result=None, error='wrong type')):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, production)
        with self.assertRaises(AssertionError):
            compare_case(case, dict(candidate, error='manifest_unsupported'), reference, production)

    def test_metadata_and_float_receipts_stay_exact(self):
        for name, field in [('source_debug_describes_nested_kind', 'source_debug'), ('nested_records_keep_float_literal_metadata', 'f64_literals')]:
            case = next(c for c in generated_cases() if c['name'] == name)
            candidate, reference, production = self.values(case)
            for changed in (dict(candidate, **{field: []}), dict(candidate, **{field: candidate[field] * 2})):
                with self.assertRaises(AssertionError):
                    compare_case(case, changed, reference, changed)
            changed = copy.deepcopy(reference); changed[field][0]['instruction_index'] += 1
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, production)
        process = {'exit_code': 0, 'timed_out': False}
        case = next(c for c in generated_cases() if c['name'] == 'nested_records_keep_float_literal_metadata')
        candidate, _, _ = self.values(case)
        self.assertEqual(parse_probe(process, json.dumps(candidate).encode()), candidate)
        for bits in ['0000000000000000', '7ff0000000000000']:
            changed = copy.deepcopy(candidate); changed['f64_literals'][0]['bits_hex'] = bits
            if bits.startswith('7ff'):
                with self.assertRaises(AssertionError):
                    parse_probe(process, json.dumps(changed).encode())
            else:
                with self.assertRaises(AssertionError):
                    compare_case(case, changed, self.values(case)[1], changed)

    def test_complete_target_requires_full_result_exact_input_and_accounting(self):
        value = {'status': 'returned', 'result': 0, 'error': None, 'output_hex': aggregate_output.hex(), 'instructions': 150,
            'console_calls': 9, 'output_bytes': len(aggregate_output), 'source_debug': [], 'f64_literals': []}
        target = {'invocation': 7, 'root_case': 'aggregates-twin.ny', 'category': 'root_execution', 'edition_bytes': 7534,
                  'edition_sha256': known_raw_matches[7][1], 'candidate': value}
        compare_aggregate_target(target, value)
        for field, changed_value in [('invocation', 8), ('root_case', 'aggregates.matrix'), ('category', 'auxiliary_execution'),
                                     ('edition_bytes', 7533), ('edition_sha256', '0' * 64)]:
            with self.assertRaises(AssertionError):
                compare_aggregate_target(dict(target, **{field: changed_value}), value)
        for field, changed_value in [('result', 256), ('instructions', 149), ('console_calls', 8), ('output_bytes', 0),
                                     ('output_hex', ''), ('source_debug', [{}]), ('f64_literals', [{}])]:
            with self.assertRaises(AssertionError):
                compare_aggregate_target(dict(target, candidate=dict(value, **{field: changed_value})), value)
        with self.assertRaises(AssertionError):
            compare_aggregate_target(target, dict(value, result=256))

    def test_raw_contract_retains_all_previous_positive_coverage(self):
        corpus = []
        syntax = {8, 35}
        edition_refusals = {5, 10, 16, 20, 21, 26, 27, 40, 41}
        for number in range(45):
            match = number in expected_raw_matches
            count, sha = known_raw_matches.get(number, (3511, 'fresh-source-path-capture') if number == 4 else (1, 'test'))
            candidate = {'source_debug': [] if number != 4 else [{}], 'f64_literals': [] if number != 42 else [{}],
                         'error': None if match else 'manifest_edition' if number in edition_refusals else 'manifest_syntax' if number in syntax else 'manifest_unsupported'}
            corpus.append({'invocation': number, 'edition_bytes': count, 'edition_sha256': sha, 'matched': match,
                           'outcome': 'returned_match' if match else 'validation_refused', 'candidate': candidate, 'target_verified': number in (4, 7, 42)})
        self.assertTrue(raw_regression_contract(corpus)['passed'])
        for number in expected_raw_matches:
            changed = copy.deepcopy(corpus); changed[number].update(matched=False, outcome='validation_refused'); changed[number]['candidate']['error'] = 'manifest_unsupported'
            self.assertFalse(raw_regression_contract(changed)['passed'])
        for number in (4, 7, 42):
            changed = copy.deepcopy(corpus); changed[number]['target_verified'] = False
            self.assertFalse(raw_regression_contract(changed)['passed'])
        changed = copy.deepcopy(corpus); changed[7]['edition_sha256'] = '0' * 64
        self.assertFalse(raw_regression_contract(changed)['passed'])

    def test_lexical_inventory_is_not_validation_and_adapters_restore(self):
        case = next(c for c in generated_cases() if c['name'] == 'generic_forward_nested_record')
        inventory = instruction_inventory(case['edition'])
        self.assertFalse(inventory['unsupported_declarations'])
        self.assertTrue(all(x['supported_form'] for x in inventory['instructions']))
        malformed = next(c for c in generated_cases() if c['name'] == 'reject_different_generic_record_field')
        self.assertTrue(all(x['supported_form'] for x in instruction_inventory(malformed['edition'])['instructions']))
        original_parser, original_inventory = baseline.parse_probe, baseline.instruction_inventory
        with self.assertRaises(RuntimeError):
            with corpus_adapters():
                self.assertIs(baseline.parse_probe, parse_probe)
                raise RuntimeError('restore')
        self.assertIs(baseline.parse_probe, original_parser)
        self.assertIs(baseline.instruction_inventory, original_inventory)
        self.assertEqual(baseline.clean_environment()['NIL_CRASH_REPORT'], '0')
        self.assertNotIn('NIL_STRICT_BOUNDS', baseline.clean_environment())

    def test_initialization_is_runtime_policy_not_implicit_static_proof(self):
        cases = {c['name']: c for c in generated_cases()}
        absent = cases['nested_initialization_after_cfg_join_0']; present = cases['nested_initialization_after_cfg_join_1']
        self.assertFalse(absent['validation']); self.assertTrue(absent['policy']); self.assertEqual(absent['error'], 'record_uninitialized')
        self.assertFalse(present['validation']); self.assertIsNone(present['error'])
        self.assertIn(b'function main -> i64:', absent['edition'])
        self.assertEqual(cases['concrete_record_argument_text']['previous_error'], 'manifest_syntax')
        self.assertEqual(cases['forward_record_field_resolution']['previous_error'], 'manifest_unsupported')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('candidate', 'production-candidate', 'typed-scalars-candidate', 'typed-scalars-conformance-candidate', 'reference', 'compiler', 'interpreter', 'root', 'output'):
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ('candidate', 'production_candidate', 'typed_scalars_candidate', 'typed_scalars_conformance_candidate', 'reference', 'compiler', 'root', 'output'):
        if getattr(args, name) is None:
            parser.error('--' + name.replace('_', '-') + ' is required')
    try:
        return qualification(args)
    except (AssertionError, ValueError, OSError) as error:
        failure = {'schema': 1, 'status': 'qualification_infrastructure_failed', 'candidate_qualified': False,
                   'native_execution': False, 'production_admission_waived': False, 'error': str(error)}
        if getattr(args, 'output_created', False):
            write_json(args.output / 'failure.json', failure)
        print(json.dumps(failure), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
