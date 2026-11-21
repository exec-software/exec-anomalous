#!/usr/bin/env python3
"""Qualify bounded indexed storage against exact retained editions and the real VM.

The prior corpus capture is reused without rewriting editions or temporary paths.
Opaque buffer lifetimes and cumulative arena limits are explicit finite policies.
"""
from __future__ import annotations
import argparse
from collections import Counter
import copy
import json
from pathlib import Path
import sys
import unittest

import program_hygienic_locals_proof as prior

base, flow, debug, typed, nested = prior.base, prior.flow, prior.debug, prior.typed, prior.nested
require, digest, write_json, parse_probe = prior.require, prior.digest, prior.write_json, prior.parse_probe
pins = dict(prior.pins, **{
    'program_hygienic_locals_proof.py': 'd264528469796cd48778b180bb577d2a060b5fc27088c7e20cd4dde820883e61',
    'program_hygienic_locals_reference.py': prior.adapter_hash,
})
previous_pins = {
    'summary.json': '193cc6b6a5eb91626914ad81f15ac8f0f9cf3c626c7e3c9581204dc976bcb40a',
    'corpus-executions.json': 'fd5e4ced2ad18635f0e7bbc6e15b732a5fa4e24bf99b3f9b189b581ce7954373',
    'production-corpus-executions.json': 'f29186f8b299c52921a733892b4ea8a640299cd8725e9ff65df020441ba1dff3',
    'inputs.json': 'f6704df193acca6533708ca0892025e819a760f366adf0e1cf7296f73cf6df76',
}
expected_matches = sorted([*prior.expected_matches, 8])
target_hash = '04973bd3779931a6128e4e4e9250ecd954dca79ea0ad9875a2876cea0d1f190a'
target_output = b'3\n4\n4\n42\n7\nqueued\nserver failure\nclient failure\n42\npresent\n1\n7\n3\n4\n42\n'


def cases():
    result = []
    def add(name, body, *, helpers=(), expected=42, output=b'', calls=0, instructions=None, error=None,
            validation=False, policy=False, reference_result=42, reference_error=None, annotated=True,
            ceiling=0, strings=(), cross=True, steps=65536, frames=16, candidate_only=False, transform=None):
        edition, metadata, literals = typed.authored_edition([flow.function('main', body), *helpers], annotated=annotated, strings=strings)
        edition = edition.replace(b'Exercise bounded typed scalar values', b'Exercise bounded indexed storage')
        if transform:
            edition = transform(edition)
        result.append(dict(name=name, edition=edition, result=None if error else expected, error=error, validation=validation,
            output=output, calls=calls, instructions=0 if validation else sum(isinstance(x, str) or 'floating' in x for x in body) if instructions is None else instructions,
            source_debug=[] if validation else metadata, f64_literals=[] if validation else literals, policy=policy,
            reference_result=reference_result if policy else None if error else expected, reference_error=reference_error,
            annotated=annotated, ceiling=ceiling, cross=cross, steps=steps, frames=frames, candidate_only=candidate_only))
    def reject(name, body, error='manifest_syntax', **kwargs):
        add('reject_' + name, body, validation=True, error=error, cross=False, **kwargs)
    def array(values=(40, 2), name='a'):
        return [*(f'PUSH_CONST {v}' for v in values), f'NEW_ARRAY_LITERAL {len(values)} i64', 'STORE_LOCAL ' + name]
    def buffer(count=2, name='b'):
        return [f'PUSH_CONST {count}', 'ALLOC i64', 'STORE_LOCAL ' + name]
    def get(name='b', index=0):
        return ['LOAD_LOCAL ' + name, f'PUSH_CONST {index}', 'GET_INDEX']
    def put(name='b', index=0, value=42):
        return ['LOAD_LOCAL ' + name, f'PUSH_CONST {index}', f'PUSH_CONST {value}', 'SET_INDEX']
    def push(name='a', value=42):
        return ['LOAD_LOCAL ' + name, f'PUSH_CONST {value}', 'CALL array__push 2']
    add('legacy_integer_array_get', array((42,)) + get('a') + ['RET'], cross=False)
    add('array_set_consumes_three_and_preserves_older_value', ['PUSH_CONST 42'] + array() + put('a', 0, 7) + ['RET'])
    add('array_write_shared_alias', array() + ['LOAD_LOCAL a', 'STORE_LOCAL alias'] + put('a', 0) + get('alias') + ['RET'])
    add('array_write_full_i64', array() + put('a', 1, -9223372036854775808) + get('a', 1) + ['RET'], expected=-9223372036854775808)
    add('tail_append_alias_sees_length_and_value', array((1,)) + ['LOAD_LOCAL a', 'STORE_LOCAL alias'] + push() + get('alias', 1) + ['RET'])
    add('relocated_append_alias_survives_other_allocation', array((1,)) + ['LOAD_LOCAL a', 'STORE_LOCAL alias'] + buffer() + push() + get('alias', 1) + ['RET'])
    add('empty_array_append', array(()) + push() + get('a') + ['RET'])
    add('append_is_void_and_keeps_caller_prefix', ['PUSH_CONST 42'] + array() + push() + ['RET'])
    add('append_does_not_change_other_array', array((42,), 'a') + array((7,), 'other') + push('a', 9) + get('other') + ['RET'], expected=7)
    for index in (-9223372036854775808, -1, 2, 9223372036854775807):
        add('array_oob_write_dropped_' + str(index), array() + put('a', index) + get('a', 0) + ['RET'], expected=40)
        add('buffer_oob_write_and_read_' + str(index), buffer() + put('b', index) + get('b', index) + ['RET'], expected=0)
    for count in (-9223372036854775808, -1, 0, 1, 1024):
        add('buffer_zeroed_clamped_count_' + str(count), buffer(count) + get() + ['RET'], expected=0)
    add('buffer_alias_write', buffer() + ['LOAD_LOCAL b', 'STORE_LOCAL alias'] + put('alias') + get() + ['RET'])
    add('buffer_both_endpoints_and_signed_values', buffer() + put('b', 0, -2) + put('b', 1, 44) + get('b', 0) + get('b', 1) + ['ADD i64', 'RET'])
    add('free_consumes_handle_and_preserves_prefix', ['PUSH_CONST 42'] + buffer() + ['LOAD_LOCAL b', 'FREE', 'RET'])
    add('separate_buffer_is_live_after_free', buffer(1, 'old') + buffer(1, 'b') + ['LOAD_LOCAL old', 'FREE'] + put() + get() + ['RET'])
    add('free_then_new_buffer_does_not_alias_old_descriptor', buffer(1, 'old') + ['LOAD_LOCAL old', 'FREE'] + buffer(1) + put() + get() + ['RET'])
    add('buffer_generated_local_and_debug_metadata', [debug.debug(0, 'indexed.ny', (('buffer', '$ny.local.0.buffer', 'ptr<i64>', 1),))] +
        buffer(1, '$ny.local.0.buffer') + put('$ny.local.0.buffer') + get('$ny.local.0.buffer') + ['RET'])
    add('float_snapshot_survives_unrelated_mutation', [typed.floating('-0.0', '8000000000000000'), 'POP'] + buffer() + put() + get() + ['RET'])
    for condition in (0, 1):
        add('joined_buffer_alias_' + str(condition), buffer(1, 'left') + buffer(1, 'right') + [f'PUSH_CONST {condition}', 'JMP_IF_NOT other',
            'LOAD_LOCAL left', 'STORE_LOCAL joined', 'JMP done', 'LABEL other', 'LOAD_LOCAL right', 'STORE_LOCAL joined', 'LABEL done'] +
            put('joined') + get('left' if condition else 'right') + ['RET'], instructions=20)
    mutate = flow.function('mutate', ['STORE_LOCAL items', 'LOAD_LOCAL items', 'PUSH_CONST 0', 'PUSH_CONST 42', 'SET_INDEX', 'LOAD_LOCAL items', 'RET'],
        parameters=(('items', 'array<i64>'),), result='array<i64>')
    add('direct_array_argument_return_shares_storage', array((0,)) + ['LOAD_LOCAL a', 'CALL mutate 1', 'POP'] + get('a') + ['RET'], helpers=(mutate,), instructions=17)
    add('indirect_array_mutation_respects_caller_floor', array((0,)) + ['PUSH_CONST 42', 'PUSH_FN_PTR mutate', 'STORE_LOCAL fn',
        'LOAD_LOCAL a', 'CALL fn 1', 'POP', 'RET'], helpers=(mutate,), instructions=17)
    helper = flow.function('local_buffer', buffer() + put() + get() + ['RET'])
    add('callee_buffer_allocation_preserves_caller_prefix', ['PUSH_CONST 2', 'CALL local_buffer 0', 'ADD i64', 'RET'], helpers=(helper,), expected=44, instructions=15)
    named = flow.function('array__push', ['STORE_LOCAL right', 'STORE_LOCAL left', 'LOAD_LOCAL left', 'LOAD_LOCAL right', 'ADD i64', 'RET'],
        parameters=(('left', 'i64'), ('right', 'i64')))
    add('declared_array_push_function_wins_fallback', ['PUSH_CONST 40', 'PUSH_CONST 2', 'CALL array__push 2', 'RET'], helpers=(named,), instructions=10, cross=False)
    maker = flow.function('make', ['PUSH_CONST 42', 'RET'])
    add('local_callable_array_push_wins_fallback', ['PUSH_FN_PTR make', 'STORE_LOCAL array__push', 'CALL array__push 0', 'RET'], helpers=(maker,), instructions=6, cross=False)
    add('generated_suffix_does_not_shadow_array_push', ['PUSH_CONST 7', 'STORE_LOCAL $ny.local.0.array__push'] + array(()) + push() + get('a') + ['RET'])
    add('new_storage_does_not_supply_main_intent', buffer() + put() + get() + ['RET'], annotated=False)
    add('dead_storage_still_has_valid_grammar', ['PUSH_CONST 42', 'RET', 'ALLOC i64', 'FREE'], instructions=2)
    add('console_before_storage_and_after_mutation', ['PUSH_CONST 7', 'CONSOLE_WRITELN'] + buffer() + put() + get() + ['CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], output=b'7\n42\n', calls=2, ceiling=1)
    add('finite_fuel_before_buffer_write', buffer() + put() + get() + ['RET'], error='instruction_limit', instructions=6, steps=6, policy=True)
    add('storage_then_division_fault', buffer() + put() + ['PUSH_CONST 1', 'PUSH_CONST 0', 'DIV i64', 'RET'],
        error='division_by_zero', instructions=10, reference_error='Division by zero')
    released = buffer(1) + ['LOAD_LOCAL b', 'STORE_LOCAL alias', 'LOAD_LOCAL b', 'FREE']
    add('released_alias_read_policy', released + get('alias') + ['POP', 'PUSH_CONST 42', 'RET'], error='buffer_released', instructions=10, policy=True)
    add('released_alias_write_policy', released + put('alias') + ['PUSH_CONST 42', 'RET'], error='buffer_released', instructions=11, policy=True)
    # A second FREE emits a raw VM address on stderr; tested candidate-only below.
    add('released_alias_free_policy', released + ['LOAD_LOCAL alias', 'FREE', 'PUSH_CONST 42', 'RET'], error='buffer_released', instructions=9, policy=True, candidate_only=True)
    add('released_branch_join_is_runtime_policy', buffer(1) + ['PUSH_CONST 1', 'JMP_IF_NOT joined', 'LOAD_LOCAL b', 'FREE', 'LABEL joined'] +
        get() + ['POP', 'PUSH_CONST 42', 'RET'], error='buffer_released', instructions=11, policy=True)
    add('allocation_1025_policy', buffer(1025) + ['PUSH_CONST 42', 'RET'], error='array_limit', instructions=2, policy=True)
    add('shared_cells_exact_1024', array((0,)) + buffer(1023) + put('a') + get('a') + ['RET'])
    add('shared_cells_1025_policy', array((0,)) + buffer(1024) + ['PUSH_CONST 42', 'RET'], error='array_limit', instructions=5, policy=True)
    add('free_does_not_refund_cells_policy', buffer(1024) + ['LOAD_LOCAL b', 'FREE'] + buffer(1, 'next') + ['PUSH_CONST 42', 'RET'], error='array_limit', instructions=7, policy=True)
    add('relocation_charges_full_new_length_exact_fit', array((40, 1)) + buffer(1019) + push('a', 2) + get('a', 0) + get('a', 2) + ['ADD i64', 'RET'])
    add('relocation_then_tail_append_exhausts_policy', array((40, 1)) + buffer(1019) + push('a', 2) + push('a', 3) + ['PUSH_CONST 42', 'RET'], error='array_limit', instructions=13, policy=True)
    add('relocation_needs_full_new_length_policy', array((40, 1)) + buffer(1020) + push('a', 2) + ['PUSH_CONST 42', 'RET'], error='array_limit', instructions=10, policy=True)
    for count in (128, 129):
        body = [operation for _ in range(count) for operation in ['PUSH_CONST 1', 'ALLOC i64', 'FREE']] + ['PUSH_CONST 42', 'RET']
        add('released_descriptors_' + str(count), body, error='array_limit' if count == 129 else None,
            instructions=386 if count == 129 else None, policy=count == 129)
    body = ['NEW_ARRAY_LITERAL 0 i64', 'POP'] * 127 + buffer(1) + put() + get() + ['RET']
    add('shared_array_buffer_descriptor_boundary', body)
    add('shared_array_buffer_descriptor_overflow_policy', ['NEW_ARRAY_LITERAL 0 i64', 'POP'] * 128 + buffer(1) + ['PUSH_CONST 42', 'RET'],
        error='array_limit', instructions=258, policy=True)
    add('tail_append_exact_1024', array(()) + [op for _ in range(1024) for op in push('a', 42)] + get('a', 1023) + ['RET'])
    for op in ('SET_INDEX extra', 'ALLOC', 'ALLOC i32', 'ALLOC text', 'ALLOC i64 extra', 'FREE extra', 'CALL array__push 1', 'CALL array__push 3'):
        reject('opcode_shape_' + str(len(result)), ['PUSH_CONST 42', 'RET', op], error='manifest_unsupported')
    for op in ('READ_PTR', 'WRITE_PTR', 'PTR_ADD', 'OWN_ALLOC i64', 'REF_LOCAL b', 'BINDING_REF_LOCAL b', 'OWN_BIND b', 'OWN_MOVE b'):
        reject('pointer_surface_' + str(len(result)), buffer() + [op, 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
    reject('allocation_requires_count', ['ALLOC i64', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('allocation_requires_integer_count', ['PUSH_CONST 1', 'CAST bool', 'ALLOC i64', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('free_requires_buffer', ['PUSH_CONST 0', 'FREE', 'PUSH_CONST 42', 'RET'])
    reject('free_array_not_buffer', array() + ['LOAD_LOCAL a', 'FREE', 'PUSH_CONST 42', 'RET'])
    reject('buffer_not_array_length', buffer() + ['LOAD_LOCAL b', 'LEN', 'RET'])
    reject('buffer_not_array_append', buffer() + push('b') + ['PUSH_CONST 42', 'RET'])
    reject('buffer_not_integer_arithmetic', buffer() + ['LOAD_LOCAL b', 'PUSH_CONST 1', 'ADD i64', 'RET'])
    reject('buffer_not_integer_cast', buffer() + ['LOAD_LOCAL b', 'CAST i64', 'RET'])
    reject('buffer_not_main_integer_return', buffer() + ['LOAD_LOCAL b', 'RET'])
    reject('buffer_not_console_value', buffer() + ['LOAD_LOCAL b', 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], ceiling=1)
    reject('buffer_not_integer_global', buffer() + ['LOAD_LOCAL b', 'STORE_GLOBAL saved i64', 'PUSH_CONST 42', 'RET'])
    reject('integer_cannot_forge_buffer_index', ['PUSH_CONST 4096', 'PUSH_CONST 0', 'GET_INDEX', 'RET'])
    reject('integer_cannot_forge_buffer_write', ['PUSH_CONST 4096', 'PUSH_CONST 0', 'PUSH_CONST 42', 'SET_INDEX', 'PUSH_CONST 42', 'RET'])
    reject('set_requires_three_operands', buffer() + ['LOAD_LOCAL b', 'PUSH_CONST 0', 'SET_INDEX', 'PUSH_CONST 42', 'RET'])
    reject('set_does_not_leave_receiver', buffer() + put() + ['POP', 'PUSH_CONST 42', 'RET'])
    reject('push_does_not_leave_receiver', array() + push() + ['POP', 'PUSH_CONST 42', 'RET'])
    reject('free_does_not_leave_receiver', buffer() + ['LOAD_LOCAL b', 'FREE', 'POP', 'PUSH_CONST 42', 'RET'])
    for owner in ('a', 'b'):
        init = array() if owner == 'a' else buffer()
        reject('typed_boolean_index_' + owner, init + ['LOAD_LOCAL ' + owner, 'PUSH_CONST 1', 'CAST bool', 'PUSH_CONST 42', 'SET_INDEX', 'PUSH_CONST 42', 'RET'])
        reject('typed_byte_value_' + owner, init + ['LOAD_LOCAL ' + owner, 'PUSH_CONST 0', 'PUSH_CONST 42', 'CAST u8', 'SET_INDEX', 'PUSH_CONST 42', 'RET'])
    text_array = ['PUSH_CONST_STR S0', 'NEW_ARRAY_LITERAL 1 text', 'STORE_LOCAL a']
    reject('text_array_write_excluded', text_array + ['LOAD_LOCAL a', 'PUSH_CONST 0', 'PUSH_CONST 42', 'SET_INDEX', 'PUSH_CONST 42', 'RET'], strings=('text',))
    reject('text_array_append_excluded', text_array + ['LOAD_LOCAL a', 'PUSH_CONST 42', 'CALL array__push 2', 'PUSH_CONST 42', 'RET'], strings=('text',))
    reject('array_buffer_join_distinct_kinds', array() + buffer() + ['PUSH_CONST 1', 'JMP_IF_NOT other', 'LOAD_LOCAL a', 'JMP done',
        'LABEL other', 'LOAD_LOCAL b', 'LABEL done', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('array_local_cannot_become_buffer', array() + ['PUSH_CONST 1', 'ALLOC i64', 'STORE_LOCAL a', 'PUSH_CONST 42', 'RET'])
    reject('unused_buffer_signature_excluded', ['PUSH_CONST 42', 'RET'], helpers=(flow.function('unused', ['POP', 'PUSH_CONST 0', 'RET'], parameters=(('value', 'ptr<i64>'),)),), error='manifest_unsupported')
    reject('callee_cannot_consume_caller_buffer', buffer() + ['LOAD_LOCAL b', 'CALL bad 0', 'POP', 'PUSH_CONST 42', 'RET'], helpers=(flow.function('bad', ['FREE', 'PUSH_CONST 0', 'RET']),))
    reject('storage_does_not_grant_console', buffer() + put() + get() + ['CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], error='manifest_ceiling')
    require(len(result) == len({r['name'] for r in result}), 'duplicate fixtures')
    require(all(len(r['edition']) <= 65536 for r in result), 'authored edition exceeds cap')
    return result


def invoke(tool, path, output, label, *, ceiling=1, production=False, reference=False, steps=65536, frames=16):
    argv = [tool, path] if reference else [tool, path, str(ceiling), str(steps), str(frames)]
    process, stdout, _ = base.run(argv, output / label, environment=base.clean_environment())
    return parse_probe(process, stdout, production=production, reference=reference)


def compare(case, candidate, production, reference):
    expected_status = 'validation_refused' if case['validation'] else 'execution_refused' if case['error'] else 'returned'
    require(candidate['status'] == expected_status, 'candidate status differs')
    for key in ('result', 'error', 'instructions', 'source_debug', 'f64_literals'):
        require(candidate[key] == case[key], 'candidate ' + key + ' differs')
    require(candidate['console_calls'] == case['calls'] and candidate['output_hex'] == case['output'].hex() and candidate['output_bytes'] == len(case['output']), 'candidate output/accounting differs')
    if case['validation'] or not case['annotated']:
        expected = case['error'] if case['validation'] else 'manifest_intent'
        require(production['status'] == 'admission_refused' and production['error'] == expected and production['result'] is None and
            production['instructions'] == production['console_calls'] == production['output_bytes'] == 0 and production['output_hex'] == '' and
            production['source_debug'] == production['f64_literals'] == [], 'production refusal differs')
    else:
        require(production == candidate, 'production execution differs')
    if case['validation']:
        require(reference is None, 'validation-only case ran reference')
        return 'validation_refused'
    if case['candidate_only']:
        require(case['policy'] and reference is None, 'candidate-only policy ran reference')
        return 'candidate_only_runtime_policy'
    require(reference is not None and reference['status'] == ('execution_refused' if case['reference_error'] else 'returned') and
        reference['result'] == case['reference_result'] and reference['error'] == case['reference_error'] and reference['output_hex'] == case['output'].hex(), 'actual VM result differs')
    require(reference['source_debug'] == case['source_debug'] and reference['f64_literals'] == case['f64_literals'], 'reference metadata differs')
    return 'finite_runtime_policy_contrast' if case['policy'] else 'matching_runtime_fault' if case['error'] else 'matching_semantics'


def boundary(args):
    folder = args.output / 'boundary-editions'; folder.mkdir()
    results = []
    for index, case in enumerate(cases()):
        path = folder / f'{index:03d}-{case["name"]}.nil'; path.write_bytes(case['edition'])
        row = {k: v for k, v in case.items() if k not in {'edition', 'output'}}
        row.update(edition=base.record_tool(path), expected_output_hex=case['output'].hex())
        try:
            kwargs = {k: case[k] for k in ('ceiling', 'steps', 'frames')}
            row['candidate'] = invoke(args.candidate, path, args.output, f'boundary-raw-{index:03d}', **kwargs)
            row['production'] = invoke(args.production_candidate, path, args.output, f'boundary-production-{index:03d}', production=True, **kwargs)
            row['reference'] = None if case['validation'] or case['candidate_only'] else invoke(args.reference, path, args.output, f'boundary-reference-{index:03d}', reference=True)
            row['outcome'] = compare(case, row['candidate'], row['production'], row['reference'])
            if case['cross']:
                for label, tool, status in [('previous', args.previous_candidate, 'admission_refused'), ('previous_raw', args.previous_conformance_candidate, 'validation_refused')]:
                    value = invoke(tool, path, args.output, f'boundary-{label}-{index:03d}', ceiling=case['ceiling'], production=label == 'previous')
                    expected = 'manifest_intent' if label == 'previous' and not case['annotated'] else 'manifest_unsupported'
                    require(value['status'] == status and value['error'] == expected, 'profile13 first-error differs')
                    row[label] = value
        except (AssertionError, ValueError, OSError) as error:
            row.update(outcome='failed', failure=str(error))
        results.append(row)
    write_json(args.output / 'boundary-results.json', results)
    return {'cases': len(results), 'counts': dict(Counter(r['outcome'] for r in results)), 'prior_pairs': sum('previous_raw' in r for r in results),
        'reference_comparisons': sum(r.get('reference') is not None for r in results), 'passed': all(r['outcome'] != 'failed' for r in results)}


def verify_previous(path, old_raw, old_production):
    objects, evidence = {}, []
    for name, sha in previous_pins.items():
        item = base.record_tool(path / name); require(item['sha256'] == sha, 'profile13 proof changed: ' + name)
        objects[name] = base.strict_json((path / name).read_text()); evidence.append(item)
    summary, raw, production = (objects[name] for name in ('summary.json', 'corpus-executions.json', 'production-corpus-executions.json'))
    require(summary['status'] == 'bounded_hygienic_locals_checkpoint_passed' and summary['language_profile'] == 13, 'previous proof is not qualified profile13')
    require([r['invocation'] for r in raw] == list(range(45)) and [r['invocation'] for r in production] == list(range(45)), 'profile13 inventory incomplete')
    require([r['invocation'] for r in raw if r['matched']] == prior.expected_matches, 'profile13 coverage changed')
    for row, prod, original, original_prod in zip(raw, production, old_raw, old_production):
        item = base.record_tool(Path(row['source_capture_path']))
        identity = item['bytes'], item['sha256']
        require(identity == (row['edition_bytes'], row['edition_sha256']) == (prod['edition_bytes'], prod['edition_sha256']) ==
            (original['edition_bytes'], original['edition_sha256']) == (original_prod['edition_bytes'], original_prod['edition_sha256']), 'prior input lineage changed')
        require((row['root_case'], row['category'], row['upstream_call']) == (original['root_case'], original['category'], original['upstream_call']), 'prior corpus identity changed')
    return raw, production, evidence


def check_target(row, reference):
    require(row['invocation'] == 8 and row['root_case'] == 'aggregates.matrix' and row['category'] == 'root_execution' and
        row['edition_bytes'] == 11250 and row['edition_sha256'] == target_hash, 'complete indexed target identity changed')
    for value in (row['candidate'], reference):
        require(value['status'] == 'returned' and value['result'] == 0 and value['error'] is None and value['output_hex'] == target_output.hex() and
            value['source_debug'] == [] and value['f64_literals'] == [], 'complete indexed target full result/output differs')
    require(row['candidate']['instructions'] == 231 and row['candidate']['console_calls'] == 15 and row['candidate']['output_bytes'] == 70, 'indexed target accounting differs')


def replay(args, old_raw, old_production, calls, previous_raw, previous_production):
    rows, productions, changes = [], [], []
    for old, before_raw, before_prod in zip(old_raw, previous_raw, previous_production):
        number = old['invocation']; path = args.capture / old['retained_path']
        row = {key: old[key] for key in ('invocation', 'root_case', 'category', 'edition_bytes', 'edition_sha256', 'upstream_call')}
        row['source_capture_path'] = str(path); row['candidate'] = invoke(args.candidate, path, args.output, f'corpus-raw-{number:03d}')
        production = invoke(args.production_candidate, path, args.output, f'corpus-production-{number:03d}', production=True)
        require(production == before_prod['candidate'], 'production receipt changed: ' + str(number))
        productions.append({**{k: row[k] for k in ('invocation', 'source_capture_path', 'edition_bytes', 'edition_sha256')}, 'candidate': production})
        if number in expected_matches:
            reference = invoke(args.reference, path, args.output, f'corpus-reference-{number:03d}', reference=True)
            for key in ('status', 'result', 'error', 'output_hex', 'source_debug', 'f64_literals'):
                require(row['candidate'][key] == reference[key], 'fresh full-result reference differs: ' + str(number) + '/' + key)
            require(reference['status'] == 'returned' and reference['error'] is None, 'expected match refused')
            require(reference['output_hex'] == old['reference_stdout_hex'] and (reference['result'] & 255) == old['reference_exit_code'], 'original interpreter output/exit differs')
            row.update(matched=True, outcome='returned_match', reference=reference, result_comparison='fresh_actual_vm_full_signed_i64')
            if number == 4:
                debug.compare_target_receipt(row, calls[53], reference)
            elif number == 7:
                nested.compare_aggregate_target(row, reference)
            elif number == 35:
                prior.check_target(row, reference)
            elif number == 42:
                typed.compare_scalar_target(row, reference)
            elif number == 8:
                check_target(row, reference)
                for label, tool, status, error in [('previous', args.previous_candidate, 'admission_refused', 'manifest_intent'),
                                                   ('previous_raw', args.previous_conformance_candidate, 'validation_refused', 'manifest_unsupported')]:
                    value = invoke(tool, path, args.output, 'target-' + label, production=label == 'previous')
                    require(value['status'] == status and value['error'] == error, 'prior indexed target refusal changed')
                    row[label] = value
                write_json(args.output / 'complete-target-008.json', dict(row, production=production, edition_rewritten=False,
                    complete_functions=2, executed_paths={'main': 159, 'describe_queued': 11, 'describe_failed503': 27, 'describe_failed404': 34}, passed=True))
        else:
            expected = 'manifest_syntax' if number == 9 else before_raw['candidate']['error']
            require(row['candidate']['status'] == 'validation_refused' and row['candidate']['error'] == expected, 'raw refusal changed: ' + str(number))
            row.update(matched=False, outcome='validation_refused')
        if number not in (8, 9):
            require(row['candidate'] == before_raw['candidate'], 'existing raw receipt changed: ' + str(number))
        for lane, before, after in [('raw', before_raw['candidate'], row['candidate']), ('production', before_prod['candidate'], production)]:
            if (before['status'], before['error']) != (after['status'], after['error']):
                changes.append({'invocation': number, 'lane': lane, 'before': [before['status'], before['error']], 'after': [after['status'], after['error']]})
        rows.append(row)
    require(changes == [
        {'invocation': 8, 'lane': 'raw', 'before': ['validation_refused', 'manifest_unsupported'], 'after': ['returned', None]},
        {'invocation': 9, 'lane': 'raw', 'before': ['validation_refused', 'manifest_unsupported'], 'after': ['validation_refused', 'manifest_syntax']},
    ], 'unexpected lane changes')
    require([r['invocation'] for r in rows if r['matched']] == expected_matches, 'raw coverage differs')
    require(sum(r['matched'] and r['category'] == 'root_execution' for r in rows) == 14, 'root gain differs')
    raw_errors = dict(Counter(r['candidate']['error'] for r in rows if not r['matched']))
    production_errors = dict(Counter(r['candidate']['error'] for r in productions))
    require(raw_errors == {'manifest_edition': 9, 'manifest_syntax': 1, 'manifest_unsupported': 15}, 'raw refusal totals differ')
    require(production_errors == {'manifest_edition': 9, 'manifest_unsupported': 13, 'manifest_intent': 23}, 'production refusal totals differ')
    write_json(args.output / 'corpus-executions.json', rows); write_json(args.output / 'production-corpus-executions.json', productions)
    write_json(args.output / 'lane-changes.json', changes)
    return {'invocations': 45, 'raw_matches': 20, 'root_matches': 14, 'root_cases': 38, 'raw_refusals': raw_errors,
        'production_refusals': production_errors, 'production_attempts': 45, 'fresh_full_i64_reference_comparisons': 20, 'passed': True}


def reference_provenance(args):
    # The adapter is an exact-source build-input transformation, never an edition rewrite.
    import program_indexed_storage_reference as adapter
    generated, provenance = adapter.generate()
    require(args.reference_source.read_bytes() == generated, 'generated indexed reference source changed')
    provenance_path = args.reference_source.with_suffix('.json'); receipt = base.strict_json(provenance_path.read_text())
    require(receipt['generated_source'] == base.record_tool(args.reference_source) and receipt['adapter_source'] == base.record_tool(Path(adapter.__file__).resolve()) and
        receipt['edition_bytes_rewritten'] is False and receipt['prior_source_modified'] is False, 'indexed reference provenance differs')
    require(all(receipt.get(key) == value for key, value in provenance.items()), 'indexed reference ordered edits/provenance changed')
    return [base.record_tool(Path(adapter.__file__).resolve()), receipt['observer_source'], base.record_tool(args.reference_source), base.record_tool(provenance_path)]


def qualification(args):
    for name in cli_paths:
        key = name.replace('-', '_'); setattr(args, key, getattr(args, key).resolve())
    require(not args.output.exists(), 'output must be a new evidence directory')
    args.output.mkdir(parents=True); args.output_created = True
    old_raw, old_production, calls, capture_tools, evidence = prior.verify_capture(args.capture)
    previous_raw, previous_production, previous_evidence = verify_previous(args.previous_proof, old_raw, old_production)
    sources = []
    for name, sha in pins.items():
        item = base.record_tool(Path(__file__).with_name(name)); require(item['sha256'] == sha, 'prior source changed: ' + name); sources.append(item)
    for item in capture_tools['reference_core_archives'] + capture_tools['primary_sources']:
        require(base.record_tool(Path(item['path'])) == item, 'pinned reference dependency changed'); sources.append(item)
    sources += reference_provenance(args) + [base.record_tool(Path(__file__).resolve())]
    tools = {name: base.record_tool(getattr(args, name)) for name in ('candidate', 'production_candidate', 'previous_candidate', 'previous_conformance_candidate', 'reference')}
    write_json(args.output / 'inputs.json', {'capture': evidence, 'previous_proof': previous_evidence, 'sources': sources, 'tools': tools,
        'capture_strategy': 'Exact original profile12 capture and qualified profile13 replay; no new compiler invocation, edition rewrite, or path normalization.'})
    boundaries = boundary(args); corpus = replay(args, old_raw, old_production, calls, previous_raw, previous_production)
    for item in evidence + previous_evidence + sources + list(tools.values()):
        require(base.record_tool(Path(item['path'])) == item, 'input changed during proof')
    for index, case in enumerate(cases()):
        require((args.output / 'boundary-editions' / f'{index:03d}-{case["name"]}.nil').read_bytes() == case['edition'], 'authored fixture changed')
    summary = {'schema': 1, 'language_profile': 14, 'status': 'bounded_indexed_storage_checkpoint_passed' if boundaries['passed'] else 'bounded_indexed_storage_checkpoint_failed',
        'boundary': boundaries, 'corpus': corpus, 'candidate_qualified': False, 'native_execution': False, 'production_admission_waived': False,
        'captured_compiler_calls_reused': 97, 'captured_interpreter_calls_reused': 45, 'new_compiler_calls': 0,
        'scope': 'Host-only exact corpus replay and bounded mutation/alias/lifetime tests. Stale-buffer and cumulative storage limits are explicit policies; refusal is unmet coverage.'}
    write_json(args.output / 'summary.json', summary); print(json.dumps(summary, sort_keys=True))
    return 0 if boundaries['passed'] else 1


class proof_tests(unittest.TestCase):
    @staticmethod
    def values(case):
        raw, production, reference = prior.proof_tests.values(case)
        if case['candidate_only']:
            reference = None
        elif reference is not None:
            reference.update(status='execution_refused' if case['reference_error'] else 'returned', result=case['reference_result'], error=case['reference_error'])
        return raw, production, reference

    def test_inventory_and_prior_source_pins(self):
        rows = cases()
        self.assertEqual(len(rows), 102); self.assertEqual(len(pins), 27); self.assertEqual(len(expected_matches), 20)
        for name, sha in pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), sha)
        for row in rows:
            self.assertIn(compare(row, *self.values(row)), ('matching_semantics', 'matching_runtime_fault', 'finite_runtime_policy_contrast', 'candidate_only_runtime_policy', 'validation_refused'))

    def test_mutation_results_accounting_and_production_cannot_be_forged(self):
        case = next(r for r in cases() if r['name'] == 'array_write_shared_alias'); raw, production, reference = self.values(case)
        for key, value in [('result', 40), ('instructions', 0), ('console_calls', 1), ('output_hex', '00'), ('output_bytes', 1)]:
            with self.assertRaises(AssertionError):
                compare(case, dict(raw, **{key: value}), production, reference)
        with self.assertRaises(AssertionError):
            compare(case, raw, production, dict(reference, result=40))
        case = next(r for r in cases() if not r['annotated']); raw, production, reference = self.values(case)
        for value in (dict(production, error='manifest_syntax'), dict(production, instructions=1), dict(production, result=42)):
            with self.assertRaises(AssertionError):
                compare(case, raw, value, reference)

    def test_policy_differences_do_not_become_semantic_matches(self):
        for name in ('released_alias_read_policy', 'relocation_needs_full_new_length_policy', 'finite_fuel_before_buffer_write'):
            case = next(r for r in cases() if r['name'] == name); raw, production, reference = self.values(case)
            self.assertEqual(compare(case, raw, production, reference), 'finite_runtime_policy_contrast')
            with self.assertRaises(AssertionError):
                compare(case, raw, production, dict(reference, status='execution_refused', result=None, error=case['error']))
            with self.assertRaises(AssertionError):
                compare(case, dict(raw, status='returned', result=42, error=None), production, reference)
        case = next(r for r in cases() if r['candidate_only']); raw, production, reference = self.values(case)
        with self.assertRaises(AssertionError):
            compare(case, raw, production, {'status': 'returned'})

    def test_metadata_snapshots_are_still_exact(self):
        for name, field in [('buffer_generated_local_and_debug_metadata', 'source_debug'), ('float_snapshot_survives_unrelated_mutation', 'f64_literals')]:
            case = next(r for r in cases() if r['name'] == name); raw, production, reference = self.values(case)
            for value in ([], raw[field] * 2):
                with self.assertRaises(AssertionError):
                    compare(case, dict(raw, **{field: value}), production, reference)
            changed = copy.deepcopy(reference); changed[field][0]['instruction_index'] += 1
            with self.assertRaises(AssertionError):
                compare(case, raw, production, changed)

    def test_text_receiver_refusals_reach_receiver_kind_check(self):
        rows = {r['name']: r for r in cases()}
        for name, operation in [('reject_text_array_write_excluded', b'PUSH_CONST 42\n    SET_INDEX'),
                                ('reject_text_array_append_excluded', b'PUSH_CONST 42\n    CALL array__push 2')]:
            edition = rows[name]['edition']
            self.assertIn(b'NEW_ARRAY_LITERAL 1 text', edition)
            self.assertIn(operation, edition)
            self.assertEqual(edition.count(b'PUSH_CONST_STR S0'), 1)

    def test_complete_target_full_result_and_identity(self):
        raw = {'status': 'returned', 'result': 0, 'error': None, 'instructions': 231, 'console_calls': 15,
            'output_bytes': 70, 'output_hex': target_output.hex(), 'source_debug': [], 'f64_literals': []}
        row = {'invocation': 8, 'root_case': 'aggregates.matrix', 'category': 'root_execution', 'edition_bytes': 11250, 'edition_sha256': target_hash, 'candidate': raw}
        check_target(row, raw)
        for key, value in [('result', 256), ('instructions', 230), ('console_calls', 14), ('output_bytes', 69), ('output_hex', ''), ('source_debug', [{}])]:
            with self.assertRaises(AssertionError):
                check_target(dict(row, candidate=dict(raw, **{key: value})), raw)
        for key, value in [('invocation', 9), ('root_case', 'other'), ('edition_sha256', '0' * 64), ('edition_bytes', 11249), ('category', 'auxiliary_execution')]:
            with self.assertRaises(AssertionError):
                check_target(dict(row, **{key: value}), raw)
        with self.assertRaises(AssertionError):
            check_target(row, dict(raw, result=256))

    def test_changed_prior_receipt_is_refused_before_execution(self):
        import tempfile
        with tempfile.TemporaryDirectory() as name:
            path = Path(name); (path / 'summary.json').write_text('{}')
            with self.assertRaisesRegex(AssertionError, 'profile13 proof changed'):
                verify_previous(path, [], [])


cli_paths = ('candidate', 'production-candidate', 'previous-candidate', 'previous-conformance-candidate', 'reference', 'reference-source', 'capture', 'previous-proof', 'output')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in cli_paths:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        return 0 if unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)).wasSuccessful() else 1
    for name in cli_paths:
        if getattr(args, name.replace('-', '_')) is None:
            parser.error('--' + name + ' is required')
    try:
        return qualification(args)
    except (AssertionError, ValueError, OSError) as error:
        failure = {'schema': 1, 'status': 'qualification_failed', 'error': str(error), 'candidate_qualified': False}
        if getattr(args, 'output_created', False):
            write_json(args.output / 'failure.json', failure)
        print(json.dumps(failure), file=sys.stderr); return 1


if __name__ == '__main__':
    sys.exit(main())
