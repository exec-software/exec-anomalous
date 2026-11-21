#!/usr/bin/env python3
"""Replay the pinned profile12 capture and qualify canonical hygienic local names.

No compiler rerun, corpus rewrite or source-path normalization occurs. Authored
cases execute shadowed storage; target35 separately proves complete-body admission.
"""
from __future__ import annotations
import argparse
from collections import Counter
import copy
import json
from pathlib import Path
import sys
import unittest

import program_execution_proof as base
import program_flow_proof as flow
import program_source_debug_proof as debug
import program_typed_scalars_proof as typed
import program_nested_records_proof as nested
import program_hygienic_locals_reference as adapter

require, digest, write_json = base.require, base.digest, base.write_json
parse_probe = typed.parse_probe
pins = dict(nested.source_pins, **{
    'program_nested_records_proof.py': '3f0c19c623f3745349aeb306d0d199907abd4429851ec271796f379101040a3c',
    'program_nested_records_reference.cpp': '59a1bf0b4272a65bca19cfef4be5074810bfbea785b3e723bd3a874b6142ebbc',
})
capture_pins = {
    'summary.json': '6e4b5c2943cffe317688935ccee7d257bc68c1df216a8d3a78d3fc2f3d6e29e9',
    'corpus-executions.json': '94d2fadd606c55758c8a228bcad7360e3cf2f21e8cea293f67fe9253721b115b',
    'production-corpus-executions.json': '752223c5712a068b6032a6c0a6633b1ada2eae89d58ca8dafaac2ddf2293dd3b',
    'corpus-calls.json': '231a36a070ab3c7ece8c775a81587042a0f91db7433b9adeb60eaf461f7e8362',
    'tools.json': '5ed2bfa8918c1b7c3de329989629943aa3bc1b9059c2ebaa07314d316d0667b9',
}
adapter_hash = 'e0894c957a14eebab0290ac882d5ca9f904d051529891a812ff25b2ff20a80a5'
generated_hash = 'fa9cc3cdc47eae81ead41b2475974d0bd2df3c78a5980728de5ba04c53976aa4'
target_hash = '19fb31d7b8a1a3eedafc48337d4f5a616aa8cfc9009978e982c7e459bafabe79'
target_output = b'3\n4\n42\n7\nqueued\nserver failure\n42\n'
expected_matches = sorted([*nested.expected_raw_matches, 35])
slot = '$ny.local.0.value'


def cases():
    result = []
    def add(name, body, *, helpers=(), expected=42, output=b'', calls=0, instructions=None, error=None,
            validation=False, annotated=True, ceiling=0, strings=(), cross=True, transform=None):
        edition, metadata, literals = typed.authored_edition([flow.function('main', body), *helpers], annotated=annotated, strings=strings)
        edition = edition.replace(b'Exercise bounded typed scalar values', b'Exercise distinct hygienic local identities')
        if transform:
            edition = transform(edition)
        result.append(dict(name=name, edition=edition, result=None if error else expected, error=error, validation=validation,
            output=output, calls=calls, instructions=0 if validation else len([x for x in body if isinstance(x, str) or 'floating' in x]) if instructions is None else instructions,
            source_debug=[] if validation else metadata, f64_literals=[] if validation else literals,
            annotated=annotated, ceiling=ceiling, cross=cross))
    def reject(name, body, error='manifest_syntax', **kwargs):
        add('reject_' + name, body, validation=True, error=error, cross=False, **kwargs)
    def roundtrip(name):
        return ['PUSH_CONST 42', f'STORE_LOCAL {name}', f'LOAD_LOCAL {name}', 'RET']
    add('legacy_simple_local', roundtrip('value'), cross=False)
    add('generated_store_load', roundtrip(slot))
    add('source_and_generated_names_are_distinct', ['PUSH_CONST 40', 'STORE_LOCAL value', 'PUSH_CONST 2', f'STORE_LOCAL {slot}',
        'LOAD_LOCAL value', f'LOAD_LOCAL {slot}', 'ADD i64', 'RET'])
    add('multiple_shadows_do_not_restore_or_erase_slots', ['PUSH_CONST 30', f'STORE_LOCAL {slot}', 'PUSH_CONST 10', 'STORE_LOCAL $ny.local.1.value',
        'PUSH_CONST 2', 'STORE_LOCAL $ny.local.2.value', f'LOAD_LOCAL {slot}', 'LOAD_LOCAL $ny.local.1.value', 'ADD i64', 'LOAD_LOCAL $ny.local.2.value', 'ADD i64', 'RET'])
    add('same_counter_different_suffixes_are_distinct', ['PUSH_CONST 40', 'STORE_LOCAL $ny.local.7.first', 'PUSH_CONST 2', 'STORE_LOCAL $ny.local.7.second',
        'LOAD_LOCAL $ny.local.7.first', 'LOAD_LOCAL $ny.local.7.second', 'ADD i64', 'RET'])
    add('counters_need_not_be_sequential', roundtrip('$ny.local.9007199254740993.value'))
    add('counter_is_not_host_integer', roundtrip('$ny.local.' + '9' * 115 + '.x'))
    add('name_exact_128_bytes', roundtrip('$ny.local.0.' + 'x' * 116))
    add('same_source_suffix_may_hold_distinct_kinds', ['PUSH_CONST 1', 'CAST bool', f'STORE_LOCAL {slot}', 'PUSH_CONST 42',
        'STORE_LOCAL $ny.local.1.value', 'LOAD_LOCAL $ny.local.1.value', 'RET'])
    add('float_shadow_keeps_literal_snapshot', [typed.floating('-0.0', '8000000000000000'), f'STORE_LOCAL {slot}',
        'PUSH_CONST 42', 'STORE_LOCAL $ny.local.1.value', 'LOAD_LOCAL $ny.local.1.value', 'RET'])
    helper = flow.function('helper', ['PUSH_CONST 2', f'STORE_LOCAL {slot}', f'LOAD_LOCAL {slot}', 'RET'])
    add('same_generated_name_is_frame_local', ['PUSH_CONST 40', f'STORE_LOCAL {slot}', 'CALL helper 0', f'LOAD_LOCAL {slot}', 'ADD i64', 'RET'],
        helpers=(helper,), instructions=10)
    binder = flow.function('bind', [f'STORE_LOCAL {slot}', f'LOAD_LOCAL {slot}', 'RET'], parameters=(('input', 'i64'),))
    add('parameter_moves_into_generated_storage', ['PUSH_CONST 42', 'CALL bind 1', 'RET'], helpers=(binder,), instructions=6)
    maker = flow.function('make', ['PUSH_CONST 42', 'RET'])
    add('generated_indirect_callable', ['PUSH_FN_PTR make', 'STORE_LOCAL $ny.local.0.operation', 'CALL $ny.local.0.operation 0', 'RET'], helpers=(maker,), instructions=6)
    add('compiler_callable_snapshot_preserves_shadow', ['PUSH_FN_PTR make', 'STORE_LOCAL $ny.local.0.operation', 'LOAD_LOCAL $ny.local.0.operation',
        'STORE_LOCAL $ny.temp.callee.0', 'CALL $ny.temp.callee.0 0', 'RET'], helpers=(maker,), instructions=8)
    add('generated_builtin_suffix_is_ordinary_callable', ['PUSH_FN_PTR make', 'STORE_LOCAL $ny.local.0.println', 'CALL $ny.local.0.println 0', 'RET'],
        helpers=(maker,), instructions=6)
    add('generated_function_suffix_does_not_alias_function', ['PUSH_CONST 7', 'STORE_LOCAL $ny.local.0.make', 'CALL make 0', 'RET'], helpers=(maker,), instructions=6)
    add('generated_builtin_suffix_does_not_shadow_plain_builtin', ['PUSH_CONST 7', 'STORE_LOCAL $ny.local.0.println', 'PUSH_CONST 42', 'CALL println 1', 'PUSH_CONST 42', 'RET'],
        output=b'42\n', calls=1, ceiling=1)
    for condition in (0, 1):
        add('initialized_join_' + str(condition), [f'PUSH_CONST {condition}', 'JMP_IF_NOT other', 'PUSH_CONST 42', f'STORE_LOCAL {slot}',
            'JMP joined', 'LABEL other', 'PUSH_CONST 7', f'STORE_LOCAL {slot}', 'LABEL joined', f'LOAD_LOCAL {slot}', 'RET'],
            expected=42 if condition else 7, instructions=8)
    add('dead_generated_store_still_validates_grammar', ['PUSH_CONST 42', 'RET', f'STORE_LOCAL {slot}'], instructions=2)
    add('generated_storage_has_descriptive_debug_mapping', [debug.debug(0, 'shadow.ny', (('value', slot, 'i64', 1),)), *roundtrip(slot)])
    add('generated_storage_cannot_supply_main_intent', roundtrip(slot), annotated=False)
    add('generated_value_console_then_division_fault', ['PUSH_CONST 7', f'STORE_LOCAL {slot}', f'LOAD_LOCAL {slot}', 'CONSOLE_WRITELN',
        'PUSH_CONST 1', 'PUSH_CONST 0', 'DIV i64', 'RET'], error='division_by_zero', instructions=7, output=b'7\n', calls=1, ceiling=1)
    # Execute the second guarded arm with a distinct code slot, unlike retained target35's first arm.
    select = flow.function('classify', ['STORE_LOCAL code', 'LOAD_LOCAL code', 'PUSH_CONST 500', 'CMP_GE', 'JMP_IF_NOT client',
        'PUSH_CONST 500', 'RET', 'LABEL client', 'LOAD_LOCAL code', 'STORE_LOCAL $ny.local.0.code', 'LOAD_LOCAL $ny.local.0.code', 'RET'], parameters=(('code', 'i64'),))
    add('guarded_fallthrough_executes_generated_shadow', ['PUSH_CONST 404', 'CALL classify 1', 'RET'], helpers=(select,), expected=404, instructions=13)
    for value in ('$ny.local..value', '$ny.local.00.value', '$ny.local.01.value', '$ny.local.-1.value', '$ny.local.+1.value', '$ny.local.1a.value',
                  '$ny.local.0.', '$ny.local.0.1value', '$ny.local.0.x.y', '$ny.local.0.bad-name', '$ny.local.0.é', '$ny.local.١.value'):
        reject('malformed_' + str(len(result)), roundtrip(value))
    reject('length_129', roundtrip('$ny.local.0.' + 'x' * 117), error='manifest_limit')
    reject('same_slot_cannot_change_kind', ['PUSH_CONST 1', f'STORE_LOCAL {slot}', 'PUSH_CONST_STR S0', f'STORE_LOCAL {slot}', 'PUSH_CONST 42', 'RET'], strings=('text',))
    reject('branch_does_not_initialize_other_slot', ['PUSH_CONST 1', 'JMP_IF_NOT other', 'PUSH_CONST 42', f'STORE_LOCAL {slot}', 'JMP joined',
        'LABEL other', 'PUSH_CONST 7', 'STORE_LOCAL $ny.local.1.value', 'LABEL joined', f'LOAD_LOCAL {slot}', 'RET'])
    reject('frame_does_not_inherit_caller_slot', ['PUSH_CONST 42', f'STORE_LOCAL {slot}', 'CALL missing 0', 'RET'],
        helpers=(flow.function('missing', [f'LOAD_LOCAL {slot}', 'RET']),))
    reject('uninitialized_indirect_slot', ['CALL $ny.local.0.operation 0', 'RET', 'PUSH_FN_PTR make', 'STORE_LOCAL $ny.local.0.operation'], helpers=(maker,))
    reject('integer_generated_slot_is_not_callable', ['PUSH_CONST 1', f'STORE_LOCAL {slot}', f'CALL {slot} 0', 'RET'])
    for opcode in ('REF_LOCAL', 'BINDING_REF_LOCAL', 'OWN_BIND', 'OWN_MOVE'):
        reject('unsupported_' + opcode.lower(), [*roundtrip(slot)[:-1], 'POP', opcode + ' ' + slot, 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
    reject('generated_label_namespace_stays_closed', ['LABEL ' + slot, 'PUSH_CONST 42', 'RET'])
    reject('generated_global_namespace_stays_closed', ['PUSH_CONST 42', 'STORE_GLOBAL ' + slot + ' i64', 'PUSH_CONST 42', 'RET'])
    reject('generated_parameter_namespace_stays_closed', ['PUSH_CONST 42', 'RET'], helpers=(flow.function('unused', ['PUSH_CONST 0', 'RET'], parameters=((slot, 'i64'),)),))
    reject('generated_function_namespace_stays_closed', ['PUSH_CONST 42', 'RET'], helpers=(flow.function(slot, ['PUSH_CONST 0', 'RET']),))
    reject('metadata_does_not_initialize_generated_slot', [debug.debug(0, 'shadow.ny', (('value', slot, 'i64', 1),)), f'LOAD_LOCAL {slot}', 'RET'])
    reject('generated_slot_does_not_grant_console', [*roundtrip(slot)[:-1], 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], error='manifest_ceiling')
    reject('dead_generated_console_requires_ceiling', [*roundtrip(slot), 'CONSOLE_WRITELN'], error='manifest_ceiling')
    require(len(result) == len({x['name'] for x in result}), 'duplicate fixtures')
    return result


def compare(case, candidate, production, reference):
    expected_status = 'validation_refused' if case['validation'] else 'execution_refused' if case['error'] else 'returned'
    require(candidate['status'] == expected_status, 'candidate status differs')
    for key in ('result', 'error', 'instructions', 'source_debug', 'f64_literals'):
        require(candidate[key] == case[key], 'candidate ' + key + ' differs')
    require(candidate['console_calls'] == case['calls'] and candidate['output_hex'] == case['output'].hex() and
            candidate['output_bytes'] == len(case['output']), 'candidate output/accounting differs')
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
    require(reference is not None and reference['status'] == expected_status and reference['result'] == case['result'] and
            reference['error'] == ('Division by zero' if case['error'] else None) and reference['output_hex'] == case['output'].hex(), 'actual VM result differs')
    require(reference['source_debug'] == case['source_debug'] and reference['f64_literals'] == case['f64_literals'], 'reference metadata differs')
    return 'matching_runtime_fault' if case['error'] else 'matching_semantics'


def invoke(tool, path, output, label, *, ceiling=1, production=False, reference=False):
    argv = [tool, path] if reference else [tool, path, str(ceiling)]
    process, stdout, _ = base.run(argv, output / label, environment=base.clean_environment())
    return parse_probe(process, stdout, production=production, reference=reference)


def boundary(args):
    folder = args.output / 'boundary-editions'; folder.mkdir()
    results = []
    for index, case in enumerate(cases()):
        path = folder / f'{index:03d}-{case["name"]}.nil'; path.write_bytes(case['edition'])
        row = {k: v for k, v in case.items() if k not in {'edition', 'output'}}
        row.update(edition=base.record_tool(path), expected_output_hex=case['output'].hex())
        row['candidate'] = invoke(args.candidate, path, args.output, f'boundary-raw-{index:03d}', ceiling=case['ceiling'])
        row['production'] = invoke(args.production_candidate, path, args.output, f'boundary-production-{index:03d}', ceiling=case['ceiling'], production=True)
        row['reference'] = None if case['validation'] else invoke(args.reference, path, args.output, f'boundary-reference-{index:03d}', reference=True)
        try:
            row['outcome'] = compare(case, row['candidate'], row['production'], row['reference'])
            if case['cross']:
                for label, tool, status in [('previous', args.previous_candidate, 'admission_refused'), ('previous_raw', args.previous_conformance_candidate, 'validation_refused')]:
                    value = invoke(tool, path, args.output, f'boundary-{label}-{index:03d}', ceiling=case['ceiling'], production=label == 'previous')
                    expected_previous = 'manifest_intent' if label == 'previous' and not case['annotated'] else 'manifest_syntax'
                    require(value['status'] == status and value['error'] == expected_previous, 'profile12 first-error differs')
                    row[label] = value
        except AssertionError as error:
            row.update(outcome='failed', failure=str(error))
        results.append(row)
    write_json(args.output / 'boundary-results.json', results)
    return {'cases': len(results), 'counts': dict(Counter(r['outcome'] for r in results)),
            'prior_pairs': sum('previous_raw' in r for r in results), 'reference_comparisons': sum(r['reference'] is not None for r in results),
            'passed': all(r['outcome'] != 'failed' for r in results)}


def verify_capture(path):
    objects, evidence = {}, []
    for name, sha in capture_pins.items():
        item = base.record_tool(path / name); require(item['sha256'] == sha, 'retained capture metadata changed: ' + name)
        evidence.append(item); objects[name] = base.strict_json((path / name).read_text())
    summary = objects['summary.json']; raw = objects['corpus-executions.json']; production = objects['production-corpus-executions.json']; calls = objects['corpus-calls.json']
    require(summary['status'] == 'bounded_nested_records_checkpoint_passed' and summary['language_profile'] == 12, 'capture is not qualified profile12')
    require([r['invocation'] for r in raw] == list(range(45)) and [r['invocation'] for r in production] == list(range(45)), 'capture inventory incomplete')
    require([r['invocation'] for r in raw if r['matched']] == nested.expected_raw_matches, 'prior coverage changed')
    require(len(calls) == 142 and Counter(r['kind'] for r in calls) == {'compiler': 97, 'interpreter': 45}, 'upstream obligations changed')
    for row in raw:
        item = base.record_tool(path / row['retained_path']); require((item['bytes'], item['sha256']) == (row['edition_bytes'], row['edition_sha256']), 'captured edition changed')
        old_production = production[row['invocation']]; call = calls[row['upstream_call']]
        require((old_production['edition_bytes'], old_production['edition_sha256']) == (item['bytes'], item['sha256']), 'prior lane bytes differ')
        require(call['kind'] == 'interpreter' and call['inputs'][0]['sha256'] == item['sha256'], 'capture call linkage differs')
        evidence.append(item)
    return raw, production, calls, objects['tools.json'], evidence


def check_target(row, reference):
    require(row['invocation'] == 35 and row['root_case'] == 'records-and-choices.matrix' and row['category'] == 'root_execution' and
            row['edition_bytes'] == 7966 and row['edition_sha256'] == target_hash, 'complete target identity changed')
    for value in (row['candidate'], reference):
        require(value['status'] == 'returned' and value['result'] == 0 and value['error'] is None and value['output_hex'] == target_output.hex() and
                value['source_debug'] == [] and value['f64_literals'] == [], 'complete target full result/output differs')
    require(row['candidate']['instructions'] == 123 and row['candidate']['console_calls'] == 7 and row['candidate']['output_bytes'] == 34, 'target accounting differs')


def replay(args, old_raw, old_production, calls):
    rows, productions, changes = [], [], []
    for old in old_raw:
        number = old['invocation']; path = args.capture / old['retained_path']
        row = {key: old[key] for key in ('invocation', 'root_case', 'category', 'edition_bytes', 'edition_sha256', 'upstream_call')}
        row['source_capture_path'] = str(path); row['candidate'] = invoke(args.candidate, path, args.output, f'corpus-raw-{number:03d}')
        production = invoke(args.production_candidate, path, args.output, f'corpus-production-{number:03d}', production=True)
        expected_production = 'manifest_intent' if number in (8, 35) else old_production[number]['candidate']['error']
        require(production['status'] == 'admission_refused' and production['error'] == expected_production, 'production first-error changed: ' + str(number))
        productions.append({**{k: row[k] for k in ('invocation', 'source_capture_path', 'edition_bytes', 'edition_sha256')}, 'candidate': production})
        if number in expected_matches:
            reference = invoke(args.reference, path, args.output, f'corpus-reference-{number:03d}', reference=True)
            for key in ('status', 'result', 'error', 'output_hex', 'source_debug', 'f64_literals'):
                require(row['candidate'][key] == reference[key], 'fresh full-result reference differs: ' + str(number) + '/' + key)
            require(reference['status'] == 'returned' and reference['error'] is None, 'expected match refused')
            require(reference['output_hex'] == old['reference_stdout_hex'] and (reference['result'] & 255) == old['reference_exit_code'], 'retained interpreter output/exit differs')
            row.update(matched=True, outcome='returned_match', reference=reference, result_comparison='fresh_actual_vm_full_signed_i64')
            if number == 4:
                debug.compare_target_receipt(row, calls[53], reference)
            elif number == 7:
                nested.compare_aggregate_target(row, reference)
            elif number == 42:
                typed.compare_scalar_target(row, reference)
            elif number == 35:
                check_target(row, reference)
                for label, tool, status in [('previous', args.previous_candidate, 'admission_refused'), ('previous_raw', args.previous_conformance_candidate, 'validation_refused')]:
                    value = invoke(tool, path, args.output, 'target-' + label, production=label == 'previous')
                    require(value['status'] == status and value['error'] == 'manifest_syntax', 'prior target grammar changed')
                    row[label] = value
                write_json(args.output / 'complete-target-035.json', dict(row, production=production,
                    hygiene_evidence='Generated slot at helper instruction52 is unexecuted by this retained input; authored shadow fixtures execute generated storage.',
                    edition_rewritten=False, complete_functions=2, passed=True))
        else:
            expected = 'manifest_unsupported' if number == 8 else old['candidate']['error']
            require(row['candidate']['status'] == 'validation_refused' and row['candidate']['error'] == expected, 'raw refusal changed: ' + str(number))
            row.update(matched=False, outcome='validation_refused')
        for lane, before, after in [('raw', old['candidate'], row['candidate']), ('production', old_production[number]['candidate'], production)]:
            if (before['status'], before['error']) != (after['status'], after['error']):
                changes.append({'invocation': number, 'lane': lane, 'before': [before['status'], before['error']], 'after': [after['status'], after['error']]})
        rows.append(row)
    require([(x['invocation'], x['lane']) for x in changes] == [(8, 'raw'), (8, 'production'), (35, 'raw'), (35, 'production')], 'unexpected lane changes')
    require([r['invocation'] for r in rows if r['matched']] == expected_matches, 'raw coverage differs')
    require(sum(r['matched'] and r['category'] == 'root_execution' for r in rows) == 13, 'root gain differs')
    raw_errors = dict(Counter(r['candidate']['error'] for r in rows if not r['matched']))
    production_errors = dict(Counter(r['candidate']['error'] for r in productions))
    require(raw_errors == {'manifest_edition': 9, 'manifest_unsupported': 17}, 'raw refusal totals differ')
    require(production_errors == {'manifest_edition': 9, 'manifest_unsupported': 13, 'manifest_intent': 23}, 'production refusal totals differ')
    write_json(args.output / 'corpus-executions.json', rows); write_json(args.output / 'production-corpus-executions.json', productions)
    write_json(args.output / 'lane-changes.json', changes)
    return {'invocations': 45, 'raw_matches': 19, 'root_matches': 13, 'root_cases': 38, 'raw_refusals': raw_errors,
            'production_refusals': production_errors, 'production_attempts': 45, 'fresh_full_i64_reference_comparisons': 19, 'passed': True}


def qualification(args):
    for name in ('candidate', 'production_candidate', 'previous_candidate', 'previous_conformance_candidate', 'reference', 'reference_source', 'capture', 'output'):
        setattr(args, name, getattr(args, name).resolve())
    require(not args.output.exists(), 'output must be a new evidence directory')
    args.output.mkdir(parents=True); args.output_created = True
    old_raw, old_production, calls, capture_tools, evidence = verify_capture(args.capture)
    sources = []
    for name, sha in pins.items():
        item = base.record_tool(Path(__file__).with_name(name)); require(item['sha256'] == sha, 'prior source changed: ' + name); sources.append(item)
    adapter_path = Path(adapter.__file__).resolve(); require(digest(adapter_path.read_bytes()) == adapter_hash, 'reference adapter changed')
    generated, offset = adapter.adapt(Path(__file__).with_name('program_nested_records_reference.cpp').read_bytes())
    require(args.reference_source.read_bytes() == generated and digest(generated) == generated_hash, 'generated reference source changed')
    provenance_path = args.reference_source.with_suffix('.json'); provenance = base.strict_json(provenance_path.read_text())
    require(provenance['base_source']['sha256'] == adapter.base_sha256 and provenance['adapter_source'] == base.record_tool(adapter_path) and
            provenance['generated_source'] == base.record_tool(args.reference_source) and provenance['insertion']['offset'] == offset and
            provenance['insertion']['bytes'] == len(adapter.insertion) and provenance['insertion']['sha256'] == digest(adapter.insertion) and
            provenance['edition_bytes_rewritten'] is False and provenance['prior_source_modified'] is False, 'reference build provenance differs')
    for item in capture_tools['reference_core_archives'] + capture_tools['primary_sources']:
        require(base.record_tool(Path(item['path'])) == item, 'pinned reference dependency changed'); sources.append(item)
    tools = {name: base.record_tool(getattr(args, name)) for name in ('candidate', 'production_candidate', 'previous_candidate', 'previous_conformance_candidate', 'reference')}
    sources += [base.record_tool(adapter_path), base.record_tool(args.reference_source), base.record_tool(provenance_path), base.record_tool(Path(__file__).resolve())]
    write_json(args.output / 'inputs.json', {'capture': evidence, 'sources': sources, 'tools': tools,
        'capture_strategy': 'Exact retained profile12 bytes and metadata; no compiler invocation, input rewrite, or path normalization.'})
    boundaries = boundary(args); corpus = replay(args, old_raw, old_production, calls)
    for item in evidence + sources + list(tools.values()):
        require(base.record_tool(Path(item['path'])) == item, 'input changed during proof')
    for index, case in enumerate(cases()):
        require((args.output / 'boundary-editions' / f'{index:03d}-{case["name"]}.nil').read_bytes() == case['edition'], 'authored fixture changed')
    summary = {'schema': 1, 'language_profile': 13, 'status': 'bounded_hygienic_locals_checkpoint_passed' if boundaries['passed'] else 'bounded_hygienic_locals_checkpoint_failed',
        'boundary': boundaries, 'corpus': corpus, 'candidate_qualified': False, 'native_execution': False, 'production_admission_waived': False,
        'captured_compiler_calls_reused': 97, 'captured_interpreter_calls_reused': 45, 'new_compiler_calls': 0,
        'scope': 'Host-only exact corpus replay and authored executed storage tests. Refusal is unmet coverage; target35 proves complete helper admission.'}
    write_json(args.output / 'summary.json', summary); print(json.dumps(summary, sort_keys=True))
    return 0 if boundaries['passed'] else 1


class proof_tests(unittest.TestCase):
    @staticmethod
    def values(case):
        candidate = {'status': 'validation_refused' if case['validation'] else 'execution_refused' if case['error'] else 'returned',
            'result': case['result'], 'error': case['error'], 'instructions': case['instructions'], 'console_calls': case['calls'],
            'output_bytes': len(case['output']), 'output_hex': case['output'].hex(), 'source_debug': copy.deepcopy(case['source_debug']), 'f64_literals': copy.deepcopy(case['f64_literals'])}
        production = dict(candidate)
        if case['validation'] or not case['annotated']:
            production.update(status='admission_refused', result=None, error=case['error'] if case['validation'] else 'manifest_intent',
                instructions=0, console_calls=0, output_bytes=0, output_hex='', source_debug=[], f64_literals=[])
        reference = None if case['validation'] else {k: candidate[k] for k in ('status', 'result', 'error', 'output_hex', 'source_debug', 'f64_literals')}
        if reference is not None:
            reference.update(schema=1, error='Division by zero' if case['error'] else None)
        return candidate, production, reference

    def test_inventory_pins_and_expected_comparisons(self):
        rows = cases()
        self.assertEqual((len(rows), sum(not r['error'] for r in rows), sum(r['validation'] for r in rows), sum(r['cross'] for r in rows)), (53, 23, 29, 23))
        self.assertEqual(len(pins), 25); self.assertEqual(len(expected_matches), 19)
        for name, sha in pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), sha)
        for row in rows:
            self.assertLessEqual(len(row['edition']), 65536)
            self.assertIn(compare(row, *self.values(row)), ('matching_semantics', 'matching_runtime_fault', 'validation_refused'))

    def test_shadow_semantics_and_accounting_cannot_be_forged(self):
        case = next(r for r in cases() if r['name'] == 'source_and_generated_names_are_distinct')
        raw, production, reference = self.values(case)
        for key, value in [('result', 2), ('instructions', 7), ('console_calls', 1), ('output_hex', '00'), ('output_bytes', 1)]:
            with self.assertRaises(AssertionError):
                compare(case, dict(raw, **{key: value}), production, reference)
        with self.assertRaises(AssertionError):
            compare(case, raw, production, dict(reference, result=2))
        case = next(r for r in cases() if not r['annotated'])
        raw, production, reference = self.values(case)
        for changed in (dict(production, result=42), dict(production, output_hex='00'), dict(production, error='manifest_syntax'), dict(production, instructions=1)):
            with self.assertRaises(AssertionError):
                compare(case, raw, changed, reference)

    def test_metadata_and_literal_receipts_remain_exact(self):
        for name, field in [('generated_storage_has_descriptive_debug_mapping', 'source_debug'), ('float_shadow_keeps_literal_snapshot', 'f64_literals')]:
            case = next(r for r in cases() if r['name'] == name); raw, production, reference = self.values(case)
            for changed in ([], raw[field] * 2):
                with self.assertRaises(AssertionError):
                    compare(case, dict(raw, **{field: changed}), production, reference)
            changed = copy.deepcopy(reference); changed[field][0]['instruction_index'] += 1
            with self.assertRaises(AssertionError):
                compare(case, raw, production, changed)
            self.assertEqual(parse_probe({'exit_code': 0, 'timed_out': False}, json.dumps(raw).encode()), raw)
        changed = copy.deepcopy(raw); changed['f64_literals'][0]['bits_hex'] = '7ff0000000000000'
        with self.assertRaises(AssertionError):
            parse_probe({'exit_code': 0, 'timed_out': False}, json.dumps(changed).encode())

    def test_target_requires_complete_bytes_full_i64_and_runtime_counts(self):
        raw = {'status': 'returned', 'result': 0, 'error': None, 'instructions': 123, 'console_calls': 7,
               'output_bytes': 34, 'output_hex': target_output.hex(), 'source_debug': [], 'f64_literals': []}
        row = {'invocation': 35, 'root_case': 'records-and-choices.matrix', 'category': 'root_execution', 'edition_bytes': 7966,
               'edition_sha256': target_hash, 'candidate': raw}
        check_target(row, raw)
        for key, value in [('result', 256), ('instructions', 122), ('console_calls', 6), ('output_bytes', 33), ('output_hex', ''), ('source_debug', [{}])]:
            with self.assertRaises(AssertionError):
                check_target(dict(row, candidate=dict(raw, **{key: value})), raw)
        for key, value in [('invocation', 8), ('root_case', 'other'), ('edition_sha256', '0' * 64), ('edition_bytes', 7965), ('category', 'auxiliary_execution')]:
            with self.assertRaises(AssertionError):
                check_target(dict(row, **{key: value}), raw)
        with self.assertRaises(AssertionError):
            check_target(row, dict(raw, result=256))

    def test_reference_adapter_is_one_exact_insertion(self):
        source = Path(__file__).with_name('program_nested_records_reference.cpp').read_bytes()
        generated, offset = adapter.adapt(source)
        self.assertEqual(digest(generated), generated_hash)
        self.assertEqual(generated[:offset], source[:offset])
        self.assertEqual(generated[offset + len(adapter.insertion):], source[offset:])
        with self.assertRaises(ValueError):
            adapter.adapt(source + b'\n')
        self.assertEqual(digest(Path(adapter.__file__).read_bytes()), adapter_hash)

    def test_counter_boundaries_and_callable_compiler_pattern_are_explicit(self):
        rows = {r['name']: r for r in cases()}
        self.assertIn(('$ny.local.' + '9' * 115 + '.x').encode(), rows['counter_is_not_host_integer']['edition'])
        self.assertEqual(rows['reject_length_129']['error'], 'manifest_limit')
        pattern = rows['compiler_callable_snapshot_preserves_shadow']['edition']
        for required in [b'LOAD_LOCAL $ny.local.0.operation', b'STORE_LOCAL $ny.temp.callee.0', b'CALL $ny.temp.callee.0 0']:
            self.assertIn(required, pattern)
        self.assertFalse(rows['legacy_simple_local']['cross'])
        self.assertIn(b'STORE_LOCAL $ny.local.0.operation', rows['reject_uninitialized_indirect_slot']['edition'])

    def test_changed_capture_metadata_is_refused_before_replay(self):
        import tempfile
        with tempfile.TemporaryDirectory() as name:
            path = Path(name); (path / 'summary.json').write_text('{}')
            with self.assertRaisesRegex(AssertionError, 'retained capture metadata changed'):
                verify_capture(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    names = ('candidate', 'production-candidate', 'previous-candidate', 'previous-conformance-candidate', 'reference', 'reference-source', 'capture', 'output')
    for name in names:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        return 0 if unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)).wasSuccessful() else 1
    for name in names:
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
