#!/usr/bin/env python3
"""Qualify bounded boolean control and finite f64 builtin console formatting.

Exact retained corpus bytes and full signed results are preserved. The internal
bool/integer local join requires an explicit checked CAST bool before use.
"""
from __future__ import annotations
import argparse
from collections import Counter
import copy
import json
from pathlib import Path
import sys
import unittest

import program_indexed_storage_proof as prior

base, flow, debug, typed, nested = prior.base, prior.flow, prior.debug, prior.typed, prior.nested
require, digest, write_json, parse_probe, invoke = prior.require, prior.digest, prior.write_json, prior.parse_probe, prior.invoke
pins = dict(prior.pins, **{
    'program_indexed_storage_proof.py': '0e9b07a1d563f06266e842d7a3104e33a446e0b46d38eaa7f581ee26aab3aedb',
    'program_indexed_storage_reference.py': 'cb1cc9abb375d181ce06d71548358956eec5f51a394131c637dfbe3bc9783e0f',
    'program_indexed_storage_observer.hpp': 'adac8143ce68f475ca33d6df9e4495595a2b5788436b07dd2ab5f5d8f7e357ec',
})
previous_pins = {
    'summary.json': 'bf424f068525372194852e9f4d601c388cd26d43f9d238684d0f02a9c72a054b',
    'corpus-executions.json': '5781da471c5156e2b3ba0c7241156d41845381f8ce98113737c340ff5b1780f6',
    'production-corpus-executions.json': 'f29186f8b299c52921a733892b4ea8a640299cd8725e9ff65df020441ba1dff3',
    'inputs.json': 'df728beaf62b22ddcbc06eb19e737405a5664d54ae2432e2f8c2e9b8111b11bf',
}
expected_matches = sorted([*prior.expected_matches, 9])
target_hash = 'e4fb6e2fef87f8332980697ca676d819fdc1c21aa9f2c87397cadba1c60f41b3'
target_output = b'1\n3\n43\n42\n0.5\ndispatch\n\n'
# Authored classic defaultfloat precision-six expectations, independently checked
# against the real VM. These are not shortest-round-trip CAST text expectations.
float_cases = [
    ('0.0', '0000000000000000', '0'),
    ('-0.0', '8000000000000000', '-0'),
    ('0.1', '3fb999999999999a', '0.1'),
    ('0.5', '3fe0000000000000', '0.5'),
    ('1.23456789', '3ff3c0ca4283de1b', '1.23457'),
    ('123456.5', '40fe240800000000', '123456'),
    ('123457.5', '40fe241800000000', '123458'),
    ('-123456.5', 'c0fe240800000000', '-123456'),
    ('-123457.5', 'c0fe241800000000', '-123458'),
    ('0.0001', '3f1a36e2eb1c432d', '0.0001'),
    ('0.00001', '3ee4f8b588e368f1', '1e-05'),
    ('0.00009999995', '3f1a36e20f35445e', '0.0001'),
    ('999999.4', '412e847ecccccccd', '999999'),
    ('999999.5', '412e847f00000000', '1e+06'),
    ('100000.0', '40f86a0000000000', '100000'),
    ('1000000.0', '412e848000000000', '1e+06'),
    ('5e-324', '0000000000000001', '4.94066e-324'),
    ('2.2250738585072014e-308', '0010000000000000', '2.22507e-308'),
    ('2.225073858507201e-308', '000fffffffffffff', '2.22507e-308'),
    ('1.7976931348623157e308', '7fefffffffffffff', '1.79769e+308'),
    ('-1.7976931348623157e308', 'ffefffffffffffff', '-1.79769e+308'),
    ('9007199254740991.0', '433fffffffffffff', '9.0072e+15'),
    ('9007199254740992.0', '4340000000000000', '9.0072e+15'),
    ('1e100', '54b249ad2594c37d', '1e+100'),
    ('1e-100', '2b2bff2ee48e0530', '1e-100'),
]


def join(condition=1, integer=0, boolean=1, name='choice'):
    return [f'PUSH_CONST {condition}', 'JMP_IF_NOT integer_arm', f'PUSH_CONST {boolean}', 'CAST bool', 'STORE_LOCAL ' + name,
        'JMP joined', 'LABEL integer_arm', f'PUSH_CONST {integer}', 'STORE_LOCAL ' + name, 'LABEL joined']


def cases():
    result = []
    def add(name, body, *, helpers=(), expected=42, output=b'', calls=0, instructions=None, error=None,
            validation=False, policy=False, reference_result=42, reference_error=None, reference_output=None,
            annotated=True, ceiling=0, strings=(), cross=True, steps=65536, frames=16, transform=None):
        edition, metadata, literals = typed.authored_edition([flow.function('main', body), *helpers], annotated=annotated, strings=strings)
        edition = edition.replace(b'Exercise bounded typed scalar values', b'Exercise bounded boolean control')
        if transform:
            edition = transform(edition)
        result.append(dict(name=name, edition=edition, result=None if error else expected, error=error, validation=validation,
            output=output, calls=calls, instructions=0 if validation else sum(isinstance(x, str) or 'floating' in x for x in body) if instructions is None else instructions,
            source_debug=[] if validation else metadata, f64_literals=[] if validation else literals, policy=policy, candidate_only=False,
            reference_result=reference_result if policy else None if error else expected, reference_error=reference_error,
            reference_output=output if reference_output is None else reference_output,
            annotated=annotated, ceiling=ceiling, cross=cross, steps=steps, frames=frames))
    def reject(name, body, error='manifest_syntax', **kwargs):
        add('reject_' + name, body, validation=True, error=error, cross=False, **kwargs)
    floating = typed.floating('0.5', '3fe0000000000000')
    for opcode in ('JMP_IF', 'JMP_IF_NOT'):
        for value in (0, 1):
            taken = bool(value) == (opcode == 'JMP_IF')
            add('boolean_' + opcode.lower() + '_' + str(value), [f'PUSH_CONST {value}', 'CAST bool', opcode + ' taken',
                'PUSH_CONST 7', 'RET', 'LABEL taken', 'PUSH_CONST 42', 'RET'], expected=42 if taken else 7, instructions=6 if taken else 5)
    for value in (0, 1):
        add('boolean_not_returns_integer_' + str(value), [f'PUSH_CONST {value}', 'CAST bool', 'NOT', 'PUSH_CONST 41', 'ADD i64', 'RET'], expected=42 - value)
    add('legacy_integer_not', ['PUSH_CONST -1', 'NOT', 'RET'], expected=0, cross=False)
    add('legacy_integer_truthy_branch', ['PUSH_CONST -7', 'JMP_IF taken', 'PUSH_CONST 0', 'RET', 'LABEL taken', 'PUSH_CONST 42', 'RET'], instructions=5, cross=False)
    for condition in (0, 1):
        for name in ('choice', '$ny.temp.logical_or.1', '$ny.local.0.choice'):
            add('explicit_bool_join_' + str(condition) + '_' + name.replace('$', '').replace('.', '_'), join(condition, name=name) +
                ['LOAD_LOCAL ' + name, 'CAST bool', 'CAST i64', 'RET'], expected=condition, instructions=11 if condition else 10)
    add('mixed_local_checked_cast_can_branch', join() + ['LOAD_LOCAL choice', 'CAST bool', 'JMP_IF good', 'PUSH_CONST 0', 'RET',
        'LABEL good', 'PUSH_CONST 42', 'RET'], instructions=13)
    add('mixed_local_checked_cast_can_call_boolean_signature', join() + ['LOAD_LOCAL choice', 'CAST bool', 'CALL identity 1', 'CAST i64', 'RET'],
        helpers=(flow.function('identity', ['RET'], parameters=(('value', 'bool'),), result='bool'),), expected=1, instructions=13)
    add('mixed_local_checked_cast_can_call_indirect_boolean_signature', ['PUSH_FN_PTR identity', 'STORE_LOCAL fn'] + join() +
        ['LOAD_LOCAL choice', 'CAST bool', 'CALL fn 1', 'CAST i64', 'RET'],
        helpers=(flow.function('identity', ['RET'], parameters=(('value', 'bool'),), result='bool'),), expected=1, instructions=15)
    add('store_kills_mixed_local_to_exact_integer', join() + ['PUSH_CONST 42', 'STORE_LOCAL choice', 'LOAD_LOCAL choice', 'RET'], instructions=11)
    add('store_kills_mixed_local_to_exact_boolean', join() + ['PUSH_CONST 0', 'CAST bool', 'STORE_LOCAL choice', 'LOAD_LOCAL choice', 'NOT', 'RET'], expected=1, instructions=13)
    add('mixed_local_cast_accepts_integer_one', join(0, 1) + ['LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'], expected=1, instructions=10)
    add('mixed_local_cast_rejects_integer_two', join(0, 2) + ['LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'],
        error='scalar_conversion', instructions=8, reference_error='CAST: boolean values must be exactly 0 or 1')
    add('mixed_local_cast_rejects_negative_integer', join(0, -1) + ['LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'],
        error='scalar_conversion', instructions=8, reference_error='CAST: boolean values must be exactly 0 or 1')
    add('unselected_noncanonical_integer_is_not_executed', join(1, 2) + ['LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'], expected=1, instructions=11)
    add('debug_record_between_load_and_cast_is_not_an_operation', join() + ['LOAD_LOCAL choice', debug.debug(11, 'boolean.ny', (('choice', 'choice', 'bool', 1),)),
        'CAST bool', 'CAST i64', 'RET'], expected=1, instructions=11)
    loop = ['PUSH_CONST 0', 'STORE_LOCAL counter', 'LABEL again', 'LOAD_LOCAL counter', 'PUSH_CONST 0', 'CMP_EQ', 'JMP_IF_NOT integer_arm',
        'PUSH_CONST 1', 'CAST bool', 'STORE_LOCAL choice', 'JMP joined', 'LABEL integer_arm', 'PUSH_CONST 0', 'STORE_LOCAL choice',
        'LABEL joined', 'LOAD_LOCAL choice', 'CAST bool', 'NOT', 'POP', 'LOAD_LOCAL counter', 'PUSH_CONST 1', 'ADD i64', 'STORE_LOCAL counter',
        'LOAD_LOCAL counter', 'PUSH_CONST 2', 'CMP_LT', 'JMP_IF again', 'PUSH_CONST 42', 'RET']
    add('loop_revisits_join_to_fixed_point', loop, instructions=47)
    helper = flow.function('negate', ['STORE_LOCAL input', 'LOAD_LOCAL input', 'NOT', 'RET'], parameters=(('input', 'bool'),))
    add('boolean_parameter_not_preserves_caller_floor', ['PUSH_CONST 41', 'PUSH_CONST 0', 'CAST bool', 'CALL negate 1', 'ADD i64', 'RET'], helpers=(helper,), instructions=10)
    for index, (spelling, bits, output) in enumerate(float_cases):
        add('float_builtin_precision6_' + str(index), [typed.floating(spelling, bits), 'CALL println 1', 'PUSH_CONST 42', 'RET'], output=(output + '\n').encode(), calls=1, ceiling=1)
    for alias, newline in [('core__print', True), ('console__writeln', True), ('console__write', False), ('console__sys_write', False)]:
        add('float_builtin_alias_' + alias, [floating, 'CALL ' + alias + ' 1', 'PUSH_CONST 42', 'RET'], output=b'0.5' + (b'\n' if newline else b''), calls=1, ceiling=1)
    add('float_text_integer_output_preserves_order', [typed.floating('-0.0', '8000000000000000'), 'CALL console__write 1', 'PUSH_CONST_STR S0',
        'CALL console__write 1', 'PUSH_CONST 42', 'CALL println 1', 'PUSH_CONST 42', 'RET'], strings=(':',), output=b'-0:42\n', calls=3, ceiling=1)
    printer = flow.function('printer', ['STORE_LOCAL value', 'LOAD_LOCAL value', 'CALL println 1', 'PUSH_CONST 42', 'RET'], parameters=(('value', 'f64'),))
    add('float_helper_signature_builtin_output', [floating, 'CALL printer 1', 'RET'], helpers=(printer,), instructions=8, output=b'0.5\n', calls=1, ceiling=1)
    add('float_indirect_helper_builtin_output', ['PUSH_FN_PTR printer', 'STORE_LOCAL fn', floating, 'CALL fn 1', 'RET'], helpers=(printer,), instructions=10, output=b'0.5\n', calls=1, ceiling=1)
    declared = flow.function('println', ['POP', 'PUSH_CONST 42', 'RET'], parameters=(('value', 'f64'),))
    reject('declared_console_name_remains_reserved', [floating, 'CALL println 1', 'RET'], helpers=(declared,), error='manifest_unsupported')
    maker = flow.function('make', ['PUSH_CONST 42', 'RET'])
    add('local_console_name_keeps_callable_precedence', ['PUSH_FN_PTR make', 'STORE_LOCAL println', 'CALL println 0', 'RET'], helpers=(maker,), instructions=6, cross=False)
    add('boolean_and_float_do_not_supply_main_intent', [floating, 'CALL println 1', 'PUSH_CONST 42', 'RET'], output=b'0.5\n', calls=1, ceiling=1, annotated=False)
    add('float_output_then_division_fault', [floating, 'CALL println 1', 'PUSH_CONST 1', 'PUSH_CONST 0', 'DIV i64', 'RET'],
        output=b'0.5\n', calls=1, ceiling=1, error='division_by_zero', instructions=5, reference_error='Division by zero')
    add('float_output_then_mixed_cast_fault', [floating, 'CALL println 1'] + join(0, 2) + ['LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'],
        output=b'0.5\n', calls=1, ceiling=1, error='scalar_conversion', instructions=10, reference_error='CAST: boolean values must be exactly 0 or 1')
    add('fuel_before_builtin_console', [floating, 'CALL println 1', 'PUSH_CONST 42', 'RET'], ceiling=1, steps=1, instructions=1,
        error='instruction_limit', policy=True, reference_output=b'0.5\n')
    add('dead_helper_boolean_control_and_float_output_are_validated', ['PUSH_CONST 42', 'RET'], helpers=(printer, helper), instructions=2, ceiling=1)
    for action in (['POP', 'PUSH_CONST 42', 'RET'], ['RET'], ['CAST i64', 'RET'], ['CAST u8', 'POP', 'PUSH_CONST 42', 'RET'],
                   ['NOT', 'RET'], ['JMP_IF end', 'LABEL end', 'PUSH_CONST 42', 'RET'], ['CALL println 1', 'PUSH_CONST 42', 'RET'],
                   ['NOP', 'CAST bool', 'CAST i64', 'RET'], ['LABEL barrier', 'CAST bool', 'CAST i64', 'RET']):
        reject('mixed_local_escape_' + str(len(result)), join() + ['LOAD_LOCAL choice'] + action, ceiling=1)
    reject('operand_stack_bool_integer_join_stays_exact', ['PUSH_CONST 1', 'JMP_IF_NOT other', 'PUSH_CONST 1', 'CAST bool', 'JMP joined',
        'LABEL other', 'PUSH_CONST 0', 'LABEL joined', 'CAST bool', 'CAST i64', 'RET'])
    reject('straight_line_local_kind_change_is_not_a_join', ['PUSH_CONST 1', 'CAST bool', 'STORE_LOCAL choice', 'PUSH_CONST 0', 'STORE_LOCAL choice', 'PUSH_CONST 42', 'RET'])
    reject('two_stores_cannot_keep_using_old_union', join() + ['PUSH_CONST 0', 'CAST bool', 'STORE_LOCAL choice', 'PUSH_CONST 1', 'STORE_LOCAL choice', 'PUSH_CONST 42', 'RET'])
    reject('mixed_local_not_initialized_on_every_path', ['PUSH_CONST 1', 'JMP_IF_NOT joined', 'PUSH_CONST 1', 'CAST bool', 'STORE_LOCAL choice',
        'LABEL joined', 'LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'])
    for conversion in ('u8', 'f64'):
        value = [floating] if conversion == 'f64' else ['PUSH_CONST 1', 'CAST u8']
        reject('unrelated_local_join_' + conversion, ['PUSH_CONST 1', 'JMP_IF_NOT other', 'PUSH_CONST 1', 'CAST bool', 'STORE_LOCAL choice', 'JMP joined',
            'LABEL other'] + value + ['STORE_LOCAL choice', 'LABEL joined', 'LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'])
    for conversion in ('u8', 'f64'):
        value = [floating] if conversion == 'f64' else ['PUSH_CONST 1', 'CAST u8']
        for op in ('NOT', 'JMP_IF end'):
            reject('non_boolean_control_' + conversion + '_' + op.split()[0], value + [op, 'LABEL end', 'PUSH_CONST 42', 'RET'])
    for conversion in ('bool', 'u8'):
        reject('builtin_console_still_refuses_' + conversion, ['PUSH_CONST 1', 'CAST ' + conversion, 'CALL println 1', 'PUSH_CONST 42', 'RET'], ceiling=1)
    for opcode in ('CONSOLE_WRITE', 'CONSOLE_WRITELN', 'INTEROP_CALL intrinsic console.write 1 void', 'INTEROP_CALL intrinsic console.writeln 1 void'):
        reject('non_builtin_float_console_' + str(len(result)), [floating, opcode, 'PUSH_CONST 42', 'RET'], ceiling=1)
    reject('float_add_remains_closed', [floating, floating, 'ADD f64', 'POP', 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
    reject('float_integer_add_remains_typed', [floating, floating, 'ADD i64', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('float_compare_remains_closed', [floating, floating, 'CMP_EQ', 'RET'])
    reject('float_cast_text_remains_closed', [floating, 'CAST text', 'POP', 'PUSH_CONST 42', 'RET'])
    reject('float_to_integer_remains_closed', [floating, 'CAST i64', 'RET'])
    reject('float_array_remains_closed', [floating, 'NEW_ARRAY_LITERAL 1 f64', 'POP', 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
    reject('boolean_not_returns_integer_not_bool', ['PUSH_CONST 42', 'RET'], helpers=(flow.function('bad', ['PUSH_CONST 1', 'CAST bool', 'NOT', 'RET'], result='bool'),))
    reject('builtin_float_console_requires_authority', [floating, 'CALL println 1', 'PUSH_CONST 42', 'RET'], error='manifest_ceiling')
    reject('dead_float_console_requires_authority', ['PUSH_CONST 42', 'RET', floating, 'CALL println 1'], error='manifest_ceiling')
    reject('mixed_metadata_does_not_supply_value_or_authority', [debug.debug(0, 'boolean.ny', (('choice', 'choice', 'bool', 1),)),
        'LOAD_LOCAL choice', 'CAST bool', 'CAST i64', 'RET'])
    require(len(result) == len({r['name'] for r in result}), 'duplicate fixtures')
    require(all(len(r['edition']) <= 65536 for r in result), 'authored edition exceeds cap')
    return result


def compare(case, candidate, production, reference):
    adjusted = dict(case)
    if case['reference_output'] == case['output']:
        return prior.compare(adjusted, candidate, production, reference)
    require(case['policy'] and not case['validation'], 'output contrast lacks explicit policy')
    require(reference is not None and reference['output_hex'] == case['reference_output'].hex(), 'policy reference output differs')
    return prior.compare(adjusted, candidate, production, dict(reference, output_hex=case['output'].hex()))


def boundary(args):
    folder = args.output / 'boundary-editions'; folder.mkdir(); results = []
    for index, case in enumerate(cases()):
        path = folder / f'{index:03d}-{case["name"]}.nil'; path.write_bytes(case['edition'])
        row = {k: v for k, v in case.items() if k not in {'edition', 'output', 'reference_output'}}
        row.update(edition=base.record_tool(path), expected_output_hex=case['output'].hex(), reference_output_hex=case['reference_output'].hex())
        try:
            kwargs = {k: case[k] for k in ('ceiling', 'steps', 'frames')}
            row['candidate'] = invoke(args.candidate, path, args.output, f'boundary-raw-{index:03d}', **kwargs)
            row['production'] = invoke(args.production_candidate, path, args.output, f'boundary-production-{index:03d}', production=True, **kwargs)
            row['reference'] = None if case['validation'] else invoke(args.reference, path, args.output, f'boundary-reference-{index:03d}', reference=True)
            row['outcome'] = compare(case, row['candidate'], row['production'], row['reference'])
            if case['cross']:
                for label, tool, status in [('previous', args.previous_candidate, 'admission_refused'), ('previous_raw', args.previous_conformance_candidate, 'validation_refused')]:
                    value = invoke(tool, path, args.output, f'boundary-{label}-{index:03d}', ceiling=case['ceiling'], production=label == 'previous')
                    expected = 'manifest_intent' if label == 'previous' and not case['annotated'] else 'manifest_syntax'
                    require(value['status'] == status and value['error'] == expected, 'profile14 first-error differs')
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
        item = base.record_tool(path / name); require(item['sha256'] == sha, 'profile14 proof changed: ' + name)
        objects[name] = base.strict_json((path / name).read_text()); evidence.append(item)
    summary, raw, production = (objects[name] for name in ('summary.json', 'corpus-executions.json', 'production-corpus-executions.json'))
    require(summary['status'] == 'bounded_indexed_storage_checkpoint_passed' and summary['language_profile'] == 14, 'previous proof is not qualified profile14')
    require([r['invocation'] for r in raw] == list(range(45)) and [r['invocation'] for r in production] == list(range(45)), 'profile14 inventory incomplete')
    require([r['invocation'] for r in raw if r['matched']] == prior.expected_matches, 'profile14 coverage changed')
    for row, prod, original, original_prod in zip(raw, production, old_raw, old_production):
        item = base.record_tool(Path(row['source_capture_path'])); identity = item['bytes'], item['sha256']
        require(identity == (row['edition_bytes'], row['edition_sha256']) == (prod['edition_bytes'], prod['edition_sha256']) ==
            (original['edition_bytes'], original['edition_sha256']) == (original_prod['edition_bytes'], original_prod['edition_sha256']), 'prior input lineage changed')
        require((row['root_case'], row['category'], row['upstream_call']) == (original['root_case'], original['category'], original['upstream_call']), 'prior corpus identity changed')
    return raw, production, evidence


def check_target(row, reference):
    require(row['invocation'] == 9 and row['root_case'] == 'bindings.matrix' and row['category'] == 'root_execution' and
        row['edition_bytes'] == 6445 and row['edition_sha256'] == target_hash, 'complete boolean target identity changed')
    metadata = [{'function': 'main', 'instruction_index': 24, 'bits_hex': '3fe0000000000000'}]
    for value in (row['candidate'], reference):
        require(value['status'] == 'returned' and value['result'] == 0 and value['error'] is None and value['output_hex'] == target_output.hex() and
            value['source_debug'] == [] and value['f64_literals'] == metadata, 'complete boolean target full result/output differs')
    require(row['candidate']['instructions'] == 179 and row['candidate']['console_calls'] == 7 and row['candidate']['output_bytes'] == 24, 'boolean target accounting differs')


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
            elif number == 8:
                prior.check_target(row, reference)
            elif number == 35:
                prior.prior.check_target(row, reference)
            elif number == 42:
                typed.compare_scalar_target(row, reference)
            elif number == 9:
                check_target(row, reference)
                for label, tool, status, error in [('previous', args.previous_candidate, 'admission_refused', 'manifest_intent'),
                                                   ('previous_raw', args.previous_conformance_candidate, 'validation_refused', 'manifest_syntax')]:
                    value = invoke(tool, path, args.output, 'target-' + label, production=label == 'previous')
                    require(value['status'] == status and value['error'] == error, 'prior boolean target refusal changed')
                    row[label] = value
                write_json(args.output / 'complete-target-009.json', dict(row, production=production, edition_rewritten=False,
                    complete_functions=1, static_instructions=185, dispatched_instructions=179, passed=True))
        else:
            require(row['candidate']['status'] == 'validation_refused' and row['candidate']['error'] == before_raw['candidate']['error'], 'raw refusal changed: ' + str(number))
            row.update(matched=False, outcome='validation_refused')
        if number != 9:
            require(row['candidate'] == before_raw['candidate'], 'existing raw receipt changed: ' + str(number))
        for lane, before, after in [('raw', before_raw['candidate'], row['candidate']), ('production', before_prod['candidate'], production)]:
            if (before['status'], before['error']) != (after['status'], after['error']):
                changes.append({'invocation': number, 'lane': lane, 'before': [before['status'], before['error']], 'after': [after['status'], after['error']]})
        rows.append(row)
    require(changes == [{'invocation': 9, 'lane': 'raw', 'before': ['validation_refused', 'manifest_syntax'], 'after': ['returned', None]}], 'unexpected lane changes')
    require([r['invocation'] for r in rows if r['matched']] == expected_matches, 'raw coverage differs')
    require(sum(r['matched'] and r['category'] == 'root_execution' for r in rows) == 15, 'root gain differs')
    raw_errors = dict(Counter(r['candidate']['error'] for r in rows if not r['matched']))
    production_errors = dict(Counter(r['candidate']['error'] for r in productions))
    require(raw_errors == {'manifest_edition': 9, 'manifest_unsupported': 15}, 'raw refusal totals differ')
    require(production_errors == {'manifest_edition': 9, 'manifest_unsupported': 13, 'manifest_intent': 23}, 'production refusal totals differ')
    write_json(args.output / 'corpus-executions.json', rows); write_json(args.output / 'production-corpus-executions.json', productions)
    write_json(args.output / 'lane-changes.json', changes)
    return {'invocations': 45, 'raw_matches': 21, 'root_matches': 15, 'root_cases': 38, 'raw_refusals': raw_errors,
        'production_refusals': production_errors, 'production_attempts': 45, 'fresh_full_i64_reference_comparisons': 21, 'passed': True}


def reference_provenance(args):
    import program_boolean_control_reference as adapter
    generated, provenance = adapter.generate()
    require(args.reference_source.read_bytes() == generated, 'generated boolean reference source changed')
    provenance_path = args.reference_source.with_suffix('.json'); receipt = base.strict_json(provenance_path.read_text())
    require(receipt['generated_source'] == base.record_tool(args.reference_source) and receipt['adapter_source'] == base.record_tool(Path(adapter.__file__).resolve()) and
        receipt['edition_bytes_rewritten'] is False and receipt['prior_source_modified'] is False, 'boolean reference provenance differs')
    require(all(receipt.get(key) == value for key, value in provenance.items()), 'boolean reference ordered edits/provenance changed')
    sources = []
    def collect(value):
        if isinstance(value, dict):
            if set(value) == {'path', 'bytes', 'sha256'}:
                require(base.record_tool(Path(value['path'])) == value, 'reference source pin differs'); sources.append(value)
            else:
                for child in value.values():
                    collect(child)
        elif isinstance(value, list):
            for child in value:
                collect(child)
    collect(provenance)
    return sources + [base.record_tool(Path(adapter.__file__).resolve()), base.record_tool(args.reference_source), base.record_tool(provenance_path)]


def qualification(args):
    for name in cli_paths:
        key = name.replace('-', '_'); setattr(args, key, getattr(args, key).resolve())
    require(not args.output.exists(), 'output must be a new evidence directory')
    args.output.mkdir(parents=True); args.output_created = True
    old_raw, old_production, calls, capture_tools, evidence = prior.prior.verify_capture(args.capture)
    previous_raw, previous_production, previous_evidence = verify_previous(args.previous_proof, old_raw, old_production)
    sources = []
    for name, sha in pins.items():
        item = base.record_tool(Path(__file__).with_name(name)); require(item['sha256'] == sha, 'prior source changed: ' + name); sources.append(item)
    for item in capture_tools['reference_core_archives'] + capture_tools['primary_sources']:
        require(base.record_tool(Path(item['path'])) == item, 'pinned reference dependency changed'); sources.append(item)
    sources += reference_provenance(args) + [base.record_tool(Path(__file__).resolve())]
    tools = {name: base.record_tool(getattr(args, name)) for name in ('candidate', 'production_candidate', 'previous_candidate', 'previous_conformance_candidate', 'reference')}
    write_json(args.output / 'inputs.json', {'capture': evidence, 'previous_proof': previous_evidence, 'sources': sources, 'tools': tools,
        'capture_strategy': 'Exact original profile12 capture and qualified profile14 replay; no new compiler invocation, edition rewrite, or path normalization.'})
    boundaries = boundary(args); corpus = replay(args, old_raw, old_production, calls, previous_raw, previous_production)
    for item in evidence + previous_evidence + sources + list(tools.values()):
        require(base.record_tool(Path(item['path'])) == item, 'input changed during proof')
    for index, case in enumerate(cases()):
        require((args.output / 'boundary-editions' / f'{index:03d}-{case["name"]}.nil').read_bytes() == case['edition'], 'authored fixture changed')
    summary = {'schema': 1, 'language_profile': 15, 'status': 'bounded_boolean_control_checkpoint_passed' if boundaries['passed'] else 'bounded_boolean_control_checkpoint_failed',
        'boundary': boundaries, 'corpus': corpus, 'candidate_qualified': False, 'native_execution': False, 'production_admission_waived': False,
        'captured_compiler_calls_reused': 97, 'captured_interpreter_calls_reused': 45, 'new_compiler_calls': 0,
        'scope': 'Host-only exact corpus replay, checked local bool/integer joins, boolean control and finite precision-six builtin float console. Refusal is unmet coverage.'}
    write_json(args.output / 'summary.json', summary); print(json.dumps(summary, sort_keys=True))
    return 0 if boundaries['passed'] else 1


class proof_tests(unittest.TestCase):
    @staticmethod
    def values(case):
        raw, production, reference = prior.proof_tests.values(case)
        if reference is not None:
            reference['output_hex'] = case['reference_output'].hex()
        return raw, production, reference

    def test_inventory_and_prior_source_pins(self):
        rows = cases()
        self.assertEqual((len(rows), sum(r['validation'] for r in rows), sum(r['cross'] for r in rows)), (100, 36, 61))
        self.assertEqual(len(pins), 30); self.assertEqual(len(expected_matches), 21)
        self.assertEqual(len(float_cases), 25)
        for name, sha in pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), sha)
        for row in rows:
            self.assertIn(compare(row, *self.values(row)), ('matching_semantics', 'matching_runtime_fault', 'finite_runtime_policy_contrast', 'validation_refused'))

    def test_checked_cast_fault_cannot_become_boolean_coercion(self):
        case = next(r for r in cases() if r['name'] == 'mixed_local_cast_rejects_integer_two'); raw, production, reference = self.values(case)
        for changed in (dict(raw, status='returned', result=1, error=None), dict(raw, error='manifest_syntax'), dict(raw, instructions=0)):
            with self.assertRaises(AssertionError):
                compare(case, changed, production, reference)
        with self.assertRaises(AssertionError):
            compare(case, raw, production, dict(reference, status='returned', result=1, error=None))

    def test_precision_and_signed_zero_outputs_are_exact(self):
        for spelling in ('-0.0', '123456.5', '123457.5', '999999.5', '5e-324'):
            index = next(i for i, x in enumerate(float_cases) if x[0] == spelling)
            case = next(r for r in cases() if r['name'] == 'float_builtin_precision6_' + str(index)); raw, production, reference = self.values(case)
            for key, value in [('output_hex', b'0.500000\n'.hex()), ('output_bytes', 0), ('console_calls', 0), ('instructions', 3)]:
                with self.assertRaises(AssertionError):
                    compare(case, dict(raw, **{key: value}), production, reference)
            with self.assertRaises(AssertionError):
                compare(case, raw, production, dict(reference, output_hex=b'wrong'.hex()))
            changed = copy.deepcopy(reference); changed['f64_literals'][0]['bits_hex'] = '0000000000000000'
            with self.assertRaises(AssertionError):
                compare(case, raw, production, changed)

    def test_metadata_adjacency_and_union_store_kill_are_explicit(self):
        case = next(r for r in cases() if r['name'] == 'debug_record_between_load_and_cast_is_not_an_operation'); raw, production, reference = self.values(case)
        for changed in ([], raw['source_debug'] * 2):
            with self.assertRaises(AssertionError):
                compare(case, dict(raw, source_debug=changed), production, reference)
        self.assertIn(b'LOAD_LOCAL choice\n    # source_debug', case['edition'])
        names = {r['name'] for r in cases()}
        self.assertIn('store_kills_mixed_local_to_exact_integer', names)
        self.assertIn('reject_two_stores_cannot_keep_using_old_union', names)
        self.assertIn('reject_operand_stack_bool_integer_join_stays_exact', names)

    def test_fuel_output_contrast_remains_explicit(self):
        case = next(r for r in cases() if r['name'] == 'fuel_before_builtin_console'); raw, production, reference = self.values(case)
        self.assertEqual(compare(case, raw, production, reference), 'finite_runtime_policy_contrast')
        with self.assertRaises(AssertionError):
            compare(case, raw, production, dict(reference, output_hex=''))
        with self.assertRaises(AssertionError):
            compare(case, dict(raw, output_hex=reference['output_hex']), production, reference)

    def test_complete_target_identity_result_and_literal_snapshot(self):
        raw = {'status': 'returned', 'result': 0, 'error': None, 'instructions': 179, 'console_calls': 7, 'output_bytes': 24,
            'output_hex': target_output.hex(), 'source_debug': [], 'f64_literals': [{'function': 'main', 'instruction_index': 24, 'bits_hex': '3fe0000000000000'}]}
        row = {'invocation': 9, 'root_case': 'bindings.matrix', 'category': 'root_execution', 'edition_bytes': 6445, 'edition_sha256': target_hash, 'candidate': raw}
        check_target(row, raw)
        for key, value in [('result', 256), ('instructions', 178), ('console_calls', 6), ('output_bytes', 23), ('output_hex', ''), ('f64_literals', [])]:
            with self.assertRaises(AssertionError):
                check_target(dict(row, candidate=dict(raw, **{key: value})), raw)
        for key, value in [('invocation', 8), ('root_case', 'other'), ('edition_sha256', '0' * 64), ('edition_bytes', 6444), ('category', 'auxiliary_execution')]:
            with self.assertRaises(AssertionError):
                check_target(dict(row, **{key: value}), raw)
        with self.assertRaises(AssertionError):
            check_target(row, dict(raw, result=256))

    def test_changed_previous_proof_refused_before_execution(self):
        import tempfile
        with tempfile.TemporaryDirectory() as name:
            path = Path(name); (path / 'summary.json').write_text('{}')
            with self.assertRaisesRegex(AssertionError, 'profile14 proof changed'):
                verify_previous(path, [], [])


cli_paths = prior.cli_paths

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
