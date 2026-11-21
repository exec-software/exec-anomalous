#!/usr/bin/env python3
"""Qualify explicit profile-11 bool/u8/finite-f64 values without widening prior profiles.

Full unchanged corpus artifacts remain mandatory. The f64 literal receipt checks
validated literal bits, including unused bodies; it is not a runtime trace.
Private executor tests separately verify bit preservation through execution.
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
import program_conformance_proof as conformance

require, digest, write_json = baseline.require, baseline.digest, baseline.write_json
source_pins = dict(debug_proof.source_pins, **{
    'program_source_debug_proof.py': '545ad38f85d7facb5dc1d52ff32ddc83dfdad53746f31aea0e03eb9268f69b80',
    'program_source_debug_reference.cpp': '51c7e78281ec439828506eed30ead9f395ecde004520e398ca92bb0bfc53f47d',
})
archive_pins = debug_proof.archive_pins
primary_pins = dict(debug_proof.primary_pins, **{
    'nilcompiler/src/numeric_literal.hpp': '1f05cac868e4453db2d4e8bc5381a47ceea60e97295bd260391411ea90c2076f',
    'nilcompiler/src/integer_conversion.hpp': 'e2555dffe092bcad9aa2d79ae53439f27541b7da1d34ee70a2f8adbfc690de52',
    'nilcompiler/src/scalar_text.hpp': '1da1339f44e342cbec4b35818f590943f54f74bfb49c24b2f12c9a2804e640a8',
})
known_raw_matches = dict(sorted({**debug_proof.known_raw_matches,
    42: (2990, '7477ac54c681108dc9f1c40b604ee5e2ff491ea07277ed0f3001159545f4a2cb'),
}.items()))
expected_raw_matches = sorted([*known_raw_matches, 4])
float_spelling = re.compile(r'[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?')
scalar_output = bytes.fromhex('433a5c6469725c66696c652e7478740a5e5b612d7a5d2b5c642a240a6865207361696420226869220a6f6e650a74776f0a6e616d650976616c75650a48652073616964202268692220616e642074686520706174682077617320433a5c55736572735c6d655c66696c652e7478740a202074686973206c696e65206b656570732069747320657874726120696e64656e740a7b226a736f6e223a2022776f726b7320746f6f222c202265736361706564223a20226e6f7468696e67205c6e206e6565646564227d0a')


def validate_f64_literals(value):
    require(type(value) is list and len(value) <= 4096, 'invalid f64 literal inventory')
    current, prior, finished, seen = None, -1, set(), set()
    for item in value:
        require(type(item) is dict and set(item) == {'function', 'instruction_index', 'bits_hex'}, 'f64 literal schema changed')
        name, index, bits = item['function'], item['instruction_index'], item['bits_hex']
        require(type(name) is str and re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', name) is not None, 'invalid literal function')
        require(type(index) is int and 0 <= index < 4096, 'invalid literal instruction index')
        require(type(bits) is str and re.fullmatch(r'[0-9a-f]{16}', bits) is not None, 'noncanonical binary64 bits')
        require((int(bits, 16) >> 52) & 0x7ff != 0x7ff, 'non-finite f64 literal published')
        require((name, index) not in seen, 'duplicate f64 literal location'); seen.add((name, index))
        if name != current:
            require(name not in finished, 'noncontiguous literal function')
            if current is not None:
                finished.add(current)
            current, prior = name, -1
        require(index > prior, 'literal indices not increasing'); prior = index
    return value


def parse_probe(process, output, *, reference=False, production=False):
    value = baseline.strict_json(output.decode('utf-8'))
    require(type(value) is dict and 'f64_literals' in value, 'f64 literal receipt missing')
    literals = validate_f64_literals(value.pop('f64_literals'))
    result = debug_proof.parse_probe(process, json.dumps(value).encode(), reference=reference, production=production)
    if result['status'] in {'validation_refused', 'admission_refused'}:
        require(literals == [], 'refused admission published literal plan')
    result['f64_literals'] = literals
    return result


def parse_production(process, output):
    return parse_probe(process, output, production=True)


def floating(spelling, bits):
    return {'floating': spelling, 'bits_hex': bits}


def authored_edition(functions, *, annotated=True, strings=(), layouts=(), enums=()):
    normalized, metadata, literals = [], [], []
    for name, parameters, result, body in functions:
        lines, index = [], 0
        for item in body:
            if isinstance(item, dict) and 'floating' in item:
                lines.append('PUSH_CONST ' + item['floating'])
                literals.append({'function': name, 'instruction_index': index, 'bits_hex': item['bits_hex']})
                index += 1
            elif isinstance(item, dict):
                lines.append(item['line'])
                metadata.append(debug_proof.snapshot(name, item['index'], item['path'], item['bindings']))
            else:
                lines.append(item)
                if item.strip():
                    index += 1
        normalized.append(flow.function(name, lines, parameters, result))
    annotations = {'main': ('@ai_context("Exercise bounded typed scalar values")',)} if annotated else {}
    edition = enums_proof.authored_enum_edition(normalized, enums, strings, annotations, layouts=layouts)
    return edition, metadata, literals


def generated_cases():
    cases = []
    def add(name, body, *, helpers=(), result=42, output=b'', calls=0, instructions=None, error=None,
            validation=False, policy=False, scalar_policy=False, reference_result=None, reference_error=None,
            reference_output=None, annotated=True, production_error=None, ceiling=0, steps=65536,
            frames=16, cross_profile=True, layouts=(), enums=(), strings=(), transform=None):
        edition, metadata, literals = authored_edition([flow.function('main', body), *helpers], annotated=annotated,
                                                       strings=strings, layouts=layouts, enums=enums)
        if transform:
            edition = transform(edition)
        require(len(edition) <= 65536, 'fixture exceeds edition bound: ' + name)
        dispatch = sum((isinstance(item, str) and bool(item.strip())) or (isinstance(item, dict) and 'floating' in item) for item in body)
        cases.append({'name': name, 'edition': edition, 'result': None if error else result, 'error': error,
                      'output': output, 'console_calls': calls, 'instructions': 0 if validation else dispatch if instructions is None else instructions,
                      'validation': validation, 'policy': policy, 'scalar_policy': scalar_policy,
                      'reference_result': reference_result, 'reference_error': reference_error, 'reference_output': reference_output,
                      'source_debug': [] if validation else metadata, 'reference_source_debug': metadata,
                      'f64_literals': [] if validation else literals, 'reference_f64_literals': literals,
                      'annotated': annotated, 'production_error': production_error or
                      (error if validation and annotated else 'manifest_intent' if not annotated else None),
                      'ceiling': ceiling, 'steps': steps, 'frames': frames, 'cross_profile': cross_profile,
                      'origin': 'independently_authored_textual_nil'})
    def reject(name, body, *, error='manifest_syntax', **kwargs):
        add('reject_' + name, body, error=error, validation=True, cross_profile=False, **kwargs)
    def contrast(name, body, result=42, **kwargs):
        add('policy_' + name, body, error='manifest_syntax', validation=True, scalar_policy=True,
            reference_result=result, reference_output=b'', **kwargs)
    def as_integer(value, kind):
        return [f'PUSH_CONST {value}', f'CAST {kind}', 'CAST i64', 'RET']

    add('plain_previous_integer_grammar', ['PUSH_CONST 42', 'RET'], cross_profile=False)
    add('hexadecimal_e_stays_integer', ['PUSH_CONST 0xFE', 'RET'], result=254, cross_profile=False)
    for value in (0, 1):
        add(f'bool_{value}_to_i64', as_integer(value, 'bool'), result=value)
        add(f'bool_{value}_identity', [f'PUSH_CONST {value}', 'CAST bool', 'CAST bool', 'CAST i64', 'RET'], result=value)
        add(f'bool_{value}_to_u8', [f'PUSH_CONST {value}', 'CAST bool', 'CAST u8', 'CAST i64', 'RET'], result=value)
        add(f'bool_{value}_to_i32', [f'PUSH_CONST {value}', 'CAST bool', 'CAST i32', 'RET'], result=value)
    for name, value, result in (('zero', 0, 0), ('one', 1, 1), ('maximum', 255, 255), ('wrap', 256, 0),
                                ('wrap_one', 257, 1), ('negative_one', -1, 255), ('negative_257', -257, 255),
                                ('i64_minimum', -(1 << 63), 0), ('i64_maximum', (1 << 63) - 1, 255),
                                ('above_float_exactness', 9007199254740993, 1)):
        add('u8_' + name, as_integer(value, 'u8'), result=result)
    add('u8_identity_preserves_255', ['PUSH_CONST -1', 'CAST u8', 'CAST u8', 'CAST i64', 'RET'], result=255)
    add('u8_to_i32_preserves_unsigned_value', ['PUSH_CONST -1', 'CAST u8', 'CAST i32', 'RET'], result=255)
    for value in (0, 1, 256, 257):
        add(f'u8_{value}_to_bool', [f'PUSH_CONST {value}', 'CAST u8', 'CAST bool', 'CAST i64', 'RET'], result=value & 255)
    add('bool_explicit_integer_branch', ['PUSH_CONST 1', 'CAST bool', 'CAST i64', 'JMP_IF taken', 'PUSH_CONST 0', 'RET', 'LABEL taken', 'PUSH_CONST 42', 'RET'], instructions=7)
    add('u8_explicit_integer_arithmetic', ['PUSH_CONST 511', 'CAST u8', 'CAST i64', 'PUSH_CONST 8', 'SHL i64', 'PUSH_CONST 1', 'BIT_OR i64', 'RET'], result=65281)
    add('comparison_result_explicit_bool', ['PUSH_CONST 7', 'PUSH_CONST 7', 'CMP_EQ', 'CAST bool', 'CAST i64', 'RET'], result=1)
    add('bool_explicit_integer_text', ['PUSH_CONST 1', 'CAST bool', 'CAST i64', 'CAST text', 'CONSOLE_WRITE', 'PUSH_CONST 42', 'RET'], output=b'1', calls=1, ceiling=1)
    add('u8_explicit_integer_console', ['PUSH_CONST -1', 'CAST u8', 'CAST i64', 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], output=b'255\n', calls=1, ceiling=1)
    for kind in ('bool', 'u8'):
        first, second = (1, 0) if kind == 'bool' else (42, 255)
        for condition in (0, 1):
            add(f'{kind}_same_kind_join_{condition}', [f'PUSH_CONST {condition}', 'JMP_IF_NOT alternate', f'PUSH_CONST {first}', f'CAST {kind}',
                'JMP joined', 'LABEL alternate', f'PUSH_CONST {second}', f'CAST {kind}', 'LABEL joined', 'CAST i64', 'RET'],
                result=first if condition else second, instructions=8)
        add(kind + '_local_copy', [f'PUSH_CONST {first}', f'CAST {kind}', 'STORE_LOCAL held', 'LOAD_LOCAL held', 'POP', 'LOAD_LOCAL held', 'CAST i64', 'RET'], result=first)
        identity = flow.function('identity', ['STORE_LOCAL item', 'LOAD_LOCAL item', f'CAST {kind}', 'RET'], parameters=(('item', kind),), result=kind)
        maker = flow.function('make', [f'PUSH_CONST {first}', f'CAST {kind}', 'RET'], result=kind)
        add(kind + '_direct_parameter_and_return', [f'PUSH_CONST {first}', f'CAST {kind}', 'CALL identity 1', 'CAST i64', 'RET'], helpers=(identity,), result=first, instructions=9)
        add(kind + '_direct_return', ['CALL make 0', 'CAST i64', 'RET'], helpers=(maker,), result=first, instructions=6)
        add(kind + '_indirect_parameter_and_return', ['PUSH_FN_PTR identity', 'STORE_LOCAL operation', f'PUSH_CONST {first}', f'CAST {kind}', 'CALL operation 1', 'CAST i64', 'RET'],
            helpers=(identity,), result=first, instructions=11)
        other = flow.function('another', [f'PUSH_CONST {second}', f'CAST {kind}', 'RET'], result=kind)
        add(kind + '_same_signature_callable_reassignment', ['PUSH_FN_PTR make', 'STORE_LOCAL operation', 'PUSH_FN_PTR another', 'STORE_LOCAL operation', 'CALL operation 0', 'CAST i64', 'RET'],
            helpers=(maker, other), result=second, instructions=10)

    # IEEE-754 encodings are independent expectations, never derived from the
    # candidate, oracle receipts or host float parsing during qualification.
    float_examples = (
        ('half', '0.5', '3fe0000000000000'), ('decimal_tenth', '0.1', '3fb999999999999a'),
        ('positive_zero', '0.0', '0000000000000000'), ('negative_zero', '-0.0', '8000000000000000'),
        ('plus_zero', '+.0', '0000000000000000'), ('leading_dot', '.5', '3fe0000000000000'),
        ('trailing_dot', '1.', '3ff0000000000000'), ('plus_sign', '+1.0', '3ff0000000000000'),
        ('exponent', '1e3', '408f400000000000'), ('uppercase_exponent', '000.5000E+00', '3fe0000000000000'),
        ('negative', '-2.5', 'c004000000000000'), ('minimum_normal', '2.2250738585072014e-308', '0010000000000000'),
        ('largest_subnormal', '2.225073858507201e-308', '000fffffffffffff'),
        ('minimum_subnormal', '5e-324', '0000000000000001'), ('negative_subnormal', '-5e-324', '8000000000000001'),
        ('above_half_subnormal', '2.4703282292062328e-324', '0000000000000001'),
        ('maximum_finite', '1.7976931348623157e308', '7fefffffffffffff'),
        ('rounded_maximum_finite', '1.7976931348623158e308', '7fefffffffffffff'),
        ('above_integer_exactness', '9007199254740993.0', '4340000000000000'),
        ('negative_above_integer_exactness', '-9007199254740993.0', 'c340000000000000'),
        ('halfway_even_down', '1.00000000000000011102230246251565404236316680908203125', '3ff0000000000000'),
        ('halfway_even_up', '1.00000000000000033306690738754696212708950042724609375', '3ff0000000000002'),
        ('zero_huge_positive_exponent', '0e+' + '9' * 60000, '0000000000000000'),
        ('negative_zero_huge_negative_exponent', '-0.000e-' + '9' * 2000, '8000000000000000'),
        ('long_cancelled_exponent', '1' + '0' * 1000 + '.0e-1000', '3ff0000000000000'),
    )
    for name, spelling, bits in float_examples:
        add('f64_literal_' + name, [floating(spelling, bits), 'CAST f64', 'POP', 'PUSH_CONST 42', 'RET'])
    add('f64_local_alias_and_identity', [floating('-0.0', '8000000000000000'), 'STORE_LOCAL held', 'LOAD_LOCAL held', 'CAST f64', 'POP', 'LOAD_LOCAL held', 'POP', 'PUSH_CONST 42', 'RET'])
    float_id = flow.function('float_identity', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'CAST f64', 'RET'], parameters=(('item', 'f64'),), result='f64')
    float_make = flow.function('float_make', [floating('0.5', '3fe0000000000000'), 'CAST f64', 'RET'], result='f64')
    add('f64_direct_return', ['CALL float_make 0', 'POP', 'PUSH_CONST 42', 'RET'], helpers=(float_make,), instructions=7)
    add('f64_direct_parameter_preserves_caller_prefix', ['PUSH_CONST 42', floating('-0.0', '8000000000000000'), 'CALL float_identity 1', 'POP', 'RET'], helpers=(float_id,), instructions=9)
    add('f64_indirect_parameter_and_return', ['PUSH_FN_PTR float_identity', 'STORE_LOCAL operation', floating('5e-324', '0000000000000001'), 'CALL operation 1', 'POP', 'PUSH_CONST 42', 'RET'], helpers=(float_id,), instructions=11)
    add('f64_unused_helper_literal_retained', ['PUSH_CONST 42', 'RET'], helpers=(float_make,))
    for condition in (0, 1):
        add(f'f64_same_kind_join_{condition}', [f'PUSH_CONST {condition}', 'JMP_IF_NOT alternate', floating('0.5', '3fe0000000000000'), 'JMP joined',
            'LABEL alternate', floating('-0.0', '8000000000000000'), 'LABEL joined', 'CAST f64', 'POP', 'PUSH_CONST 42', 'RET'], instructions=9)
    mix = flow.function('mix', ['STORE_LOCAL fraction', 'STORE_LOCAL byte', 'STORE_LOCAL flag', 'LOAD_LOCAL byte', 'CAST i64', 'LOAD_LOCAL flag', 'CAST i64', 'ADD', 'RET'],
                        parameters=(('flag', 'bool'), ('byte', 'u8'), ('fraction', 'f64')))
    add('mixed_typed_parameter_order', ['PUSH_CONST 1', 'CAST bool', 'PUSH_CONST 255', 'CAST u8', floating('0.5', '3fe0000000000000'), 'CALL mix 3', 'RET'], helpers=(mix,), result=256, instructions=16)
    add('source_debug_remains_descriptive', [debug_proof.debug(0, bindings=(('pretend', 'absent', 'foreign::anything', 0),)), floating('-0.0', '8000000000000000'),
        debug_proof.debug(1, bindings=(('pretend', 'absent', 'u8', 0),)), 'POP', 'PUSH_CONST 42', 'RET'])
    add('typed_values_cannot_supply_main_intent', ['PUSH_CONST 1', 'CAST bool', 'CAST i64', 'RET'], result=1, annotated=False)
    add('dead_float_literal_stays_in_plan', ['PUSH_CONST 42', 'RET', floating('-0.0', '8000000000000000')], instructions=2)

    for name, value in (('two', 2), ('negative', -1), ('256', 256), ('i64_minimum', -(1 << 63)), ('i64_maximum', (1 << 63) - 1)):
        add('bool_refuses_' + name, as_integer(value, 'bool'), error='scalar_conversion', instructions=2,
            reference_error='CAST: boolean values must be exactly 0 or 1')
    for value in (2, 255, 258):
        add(f'byte_to_bool_refuses_{value}', [f'PUSH_CONST {value}', 'CAST u8', 'CAST bool', 'CAST i64', 'RET'], error='scalar_conversion', instructions=3,
            reference_error='CAST: boolean values must be exactly 0 or 1')
    add('bool_fault_preserves_acknowledged_output', ['PUSH_CONST 7', 'CONSOLE_WRITELN', 'PUSH_CONST 2', 'CAST bool', 'CAST i64', 'RET'],
        error='scalar_conversion', output=b'7\n', calls=1, instructions=4, ceiling=1, reference_error='CAST: boolean values must be exactly 0 or 1')
    fail = flow.function('fail', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'CAST bool', 'RET'], parameters=(('item', 'u8'),), result='bool')
    add('bool_fault_in_helper_keeps_caller_floor', ['PUSH_CONST 42', 'PUSH_CONST 2', 'CAST u8', 'CALL fail 1', 'POP', 'RET'], helpers=(fail,),
        error='scalar_conversion', instructions=7, reference_error='CAST: boolean values must be exactly 0 or 1')
    add('finite_cast_fuel', ['PUSH_CONST 1', 'CAST bool', 'CAST i64', 'RET'], error='instruction_limit', instructions=1,
        policy=True, reference_result=1, reference_output=b'', steps=1)
    add('finite_typed_call_frames', ['PUSH_CONST 0', 'CAST u8', 'CALL fail 1', 'CAST i64', 'RET'], helpers=(fail,),
        error='frame_limit', instructions=3, policy=True, reference_result=0, reference_output=b'', frames=1)

    contrast('integer_to_f64_excluded', ['PUSH_CONST 42', 'CAST f64', 'POP', 'PUSH_CONST 42', 'RET'])
    contrast('f64_to_i64_excluded', [floating('42.75', '4045600000000000'), 'CAST i64', 'RET'])
    contrast('f64_to_u8_excluded', [floating('2.5', '4004000000000000'), 'CAST u8', 'CAST i64', 'RET'], result=2)
    contrast('f64_to_bool_excluded', [floating('1.0', '3ff0000000000000'), 'CAST bool', 'CAST i64', 'RET'], result=1)
    contrast('text_to_u8_excluded', ['PUSH_CONST_STR S0', 'CAST u8', 'CAST i64', 'RET'], strings=('42',))

    for name, spelling in (('overflow', '1e309'), ('negative_overflow', '-1e309'), ('above_maximum', '1.7976931348623159e308'),
                           ('underflow', '1e-324'), ('negative_underflow', '-1e-324'), ('below_half_subnormal', '2.4703282292062327e-324'),
                           ('huge_nonzero_exponent', '1e' + '9' * 1000), ('huge_negative_nonzero_exponent', '1e-' + '9' * 1000)):
        reject('f64_range_' + name, ['PUSH_CONST ' + spelling, 'POP', 'PUSH_CONST 42', 'RET'])
    for name, spelling in (('nan', 'NaN'), ('positive_nan', '+nan'), ('infinity', 'inf'), ('negative_infinity', '-Infinity'),
                           ('hexfloat', '0x1.8p1'), ('underscore', '1_000.5'), ('comma', '1,5'), ('suffix', '1.0f64'),
                           ('double_dot', '1..0'), ('missing_exponent', '1e'), ('missing_signed_exponent', '1e-'),
                           ('double_sign', '1e++2'), ('missing_digits', '.e1'), ('decimal_plus_integer_unchanged', '+1'),
                           ('bool_true', 'true'), ('bool_false', 'false'), ('quoted_float', '"0.5"'), ('unicode_digit', '１.０')):
        reject('literal_' + name, ['PUSH_CONST ' + spelling, 'POP', 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
    reject('dead_float_range_checked', ['PUSH_CONST 42', 'RET', 'PUSH_CONST 1e309'])
    reject('unused_float_range_checked', ['PUSH_CONST 42', 'RET'], helpers=(flow.function('unused', ['PUSH_CONST 1e309', 'RET'], result='f64'),))
    for kind in ('bool', 'u8', 'f64'):
        seed = [floating('0.5', '3fe0000000000000')] if kind == 'f64' else ['PUSH_CONST 1', 'CAST ' + kind]
        reject(kind + '_cannot_return_as_main_integer', [*seed, 'RET'])
        reject(kind + '_console_requires_explicit_integer_bridge', [*seed, 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], ceiling=1)
        reject(kind + '_arithmetic_rejects_implicit_erasure', [*seed, 'PUSH_CONST 1', 'ADD', 'RET'])
        reject(kind + '_comparison_rejects_implicit_erasure', [*seed, *seed, 'CMP_EQ', 'RET'])
        reject(kind + '_branch_rejects_implicit_erasure', [*seed, 'JMP_IF done', 'LABEL done', 'PUSH_CONST 42', 'RET'])
        reject(kind + '_not_rejects_implicit_erasure', [*seed, 'NOT', 'RET'])
        reject(kind + '_text_conversion_excluded', [*seed, 'CAST text', 'POP', 'PUSH_CONST 42', 'RET'])
        reject(kind + '_integer_array_element_excluded', [*seed, 'NEW_ARRAY_LITERAL 1 i64', 'POP', 'PUSH_CONST 42', 'RET'])
        reject(kind + '_integer_global_value_excluded', [*seed, 'STORE_GLOBAL slot i64', 'PUSH_CONST 42', 'RET'])
        reject(kind + '_declared_array_excluded', ['NEW_ARRAY_LITERAL 0 ' + kind, 'POP', 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
        reject(kind + '_declared_global_excluded', ['LOAD_GLOBAL slot ' + kind, 'POP', 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
        reject(kind + '_declared_record_field_excluded', ['PUSH_CONST 42', 'RET'], layouts=(('box', (('value', kind),), ()),), error='manifest_unsupported')
        reject(kind + '_declared_enum_payload_excluded', ['PUSH_CONST 42', 'RET'], enums=(enums_proof.enum_layout('choice', (('Value', kind),)),), error='manifest_unsupported')
        reject(kind + '_main_signature_excluded', ['PUSH_CONST 42', 'RET'], error='manifest_intent', transform=lambda data, k=kind: data.replace(b'function main -> i64:', ('function main -> ' + k + ':').encode()))
        helper = flow.function('typed', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'RET'], parameters=(('item', kind),), result=kind)
        reject(kind + '_parameter_does_not_accept_integer', ['PUSH_CONST 1', 'CALL typed 1', 'POP', 'PUSH_CONST 42', 'RET'], helpers=(helper,))
        reject(kind + '_helper_return_does_not_accept_integer', ['PUSH_CONST 42', 'RET'], helpers=(flow.function('unused', ['PUSH_CONST 1', 'RET'], result=kind),))
    for left, right in (('bool', 'u8'), ('bool', 'f64'), ('u8', 'f64')):
        def seed(kind):
            return [floating('0.5', '3fe0000000000000')] if kind == 'f64' else ['PUSH_CONST 1', 'CAST ' + kind]
        reject(left + '_' + right + '_join_remains_distinct', ['PUSH_CONST 1', 'JMP_IF_NOT alternate', *seed(left), 'JMP joined', 'LABEL alternate', *seed(right), 'LABEL joined', 'POP', 'PUSH_CONST 42', 'RET'])
        for actual, expected in ((left, right), (right, left)):
            typed = flow.function('typed', ['STORE_LOCAL item', 'LOAD_LOCAL item', 'RET'], parameters=(('item', expected),), result=expected)
            reject(actual + '_cannot_bind_' + expected + '_parameter', [*seed(actual), 'CALL typed 1', 'POP', 'PUSH_CONST 42', 'RET'], helpers=(typed,))
        parameter_a = flow.function('first_parameter', ['STORE_LOCAL item', 'PUSH_CONST 42', 'RET'], parameters=(('item', left),))
        parameter_b = flow.function('second_parameter', ['STORE_LOCAL item', 'PUSH_CONST 42', 'RET'], parameters=(('item', right),))
        reject(left + '_' + right + '_callable_parameter_signatures_remain_distinct', ['PUSH_CONST 1', 'JMP_IF_NOT alternate', 'PUSH_FN_PTR first_parameter', 'STORE_LOCAL operation',
            'JMP joined', 'LABEL alternate', 'PUSH_FN_PTR second_parameter', 'STORE_LOCAL operation', 'LABEL joined', *seed(left), 'CALL operation 1', 'RET'], helpers=(parameter_a, parameter_b))
        a = flow.function('first', [*seed(left), 'RET'], result=left)
        b = flow.function('second', [*seed(right), 'RET'], result=right)
        reject(left + '_' + right + '_callable_signatures_remain_distinct', ['PUSH_CONST 1', 'JMP_IF_NOT alternate', 'PUSH_FN_PTR first', 'STORE_LOCAL operation', 'JMP joined',
            'LABEL alternate', 'PUSH_FN_PTR second', 'STORE_LOCAL operation', 'LABEL joined', 'CALL operation 0', 'POP', 'PUSH_CONST 42', 'RET'], helpers=(a, b))
    for kind in ('Bool', 'byte', 'boolean', 'f32', 'float', 'double'):
        reject('unadmitted_type_alias_' + kind, ['PUSH_CONST 42', 'RET'], helpers=(flow.function('unused', ['PUSH_CONST 1', 'RET'], result=kind),), error='manifest_unsupported')
    reject('float_add_width_excluded', [floating('0.5', '3fe0000000000000'), floating('0.5', '3fe0000000000000'), 'ADD f64', 'POP', 'PUSH_CONST 42', 'RET'], error='manifest_unsupported')
    reject('bool_cast_from_text_excluded', ['PUSH_CONST_STR S0', 'CAST bool', 'CAST i64', 'RET'], strings=('true',))
    reject('float_cast_from_text_excluded', ['PUSH_CONST_STR S0', 'CAST f64', 'POP', 'PUSH_CONST 42', 'RET'], strings=('0.5',))
    reject('typed_values_do_not_grant_console', ['PUSH_CONST 1', 'CAST bool', 'CAST i64', 'CONSOLE_WRITELN', 'PUSH_CONST 42', 'RET'], error='manifest_ceiling')
    reject('dead_foreign_effect_still_excluded', ['PUSH_CONST 1', 'CAST bool', 'POP', 'PUSH_CONST 42', 'RET', 'INTEROP_CALL libc puts 1 i64'], error='manifest_unsupported')
    require(len(cases) == len({case['name'] for case in cases}), 'duplicate boundary case')
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
    if case['validation'] and not case['scalar_policy']:
        require(reference is None, 'validation-only fixture ran reference')
        return 'expected_validation_refusal'
    require(reference is not None and reference['status'] == ('execution_refused' if case['reference_error'] else 'returned'), 'finite reference status differs')
    require(reference['error'] == case['reference_error'], 'finite reference error differs')
    expected_result = case['reference_result'] if case['policy'] or case['scalar_policy'] else case['result']
    expected_output = case['reference_output'] if case['reference_output'] is not None else case['output']
    require(reference['result'] == expected_result and reference['output_hex'] == expected_output.hex(), 'finite reference semantics differ')
    require(reference['source_debug'] == case['reference_source_debug'], 'reference debug metadata differs')
    require(reference['f64_literals'] == case['reference_f64_literals'], 'reference binary64 literals differ')
    if case['scalar_policy']:
        return 'finite_scalar_policy_contrast'
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
            if not case['validation'] or case['scalar_policy']:
                process, stdout, _ = baseline.run([args.reference, path], output / f'boundary-reference-{number:03d}', environment=environment)
                reference = parse_probe(process, stdout, reference=True)
            record.update(values, reference=reference)
            record['outcome'] = compare_case(case, values['candidate'], reference, values['production'])
            if case['cross_profile']:
                for name, tool, parser, status in (('previous_source_debug', args.source_debug_candidate, debug_proof.parse_production, 'admission_refused'),
                                                   ('previous_source_debug_raw', args.source_debug_conformance_candidate, debug_proof.parse_probe, 'validation_refused')):
                    process, stdout, _ = baseline.run([tool, path, str(case['ceiling'])], output / f"boundary-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    expected = 'manifest_intent' if not case['annotated'] and name == 'previous_source_debug' else 'manifest_unsupported'
                    require(previous['status'] == status and previous['error'] == expected, 'earlier source-debug profile expanded')
                    record[name] = previous
        except (AssertionError, ValueError, OSError) as error:
            record.update(outcome='failed', failure=str(error)); failures.append(case['name'])
        results.append(record)
    write_json(output / 'boundary-results.json', {'cases': results, 'failures': failures})
    return {'passed': not failures, 'cases': len(results), 'failures': failures,
            'semantic_matches': sum(item['outcome'] == 'matching_semantics' for item in results),
            'matching_runtime_refusals': sum(item['outcome'] == 'matching_runtime_refusal' for item in results),
            'finite_runtime_policy_contrasts': sum(item['outcome'] == 'finite_runtime_policy_contrast' for item in results),
            'finite_scalar_policy_contrasts': sum(item['outcome'] == 'finite_scalar_policy_contrast' for item in results),
            'expected_validation_refusals': sum(item['outcome'] == 'expected_validation_refusal' for item in results),
            'production_comparisons': sum('production' in item for item in results),
            'reference_comparisons': sum(item.get('reference') is not None for item in results),
            'f64_literal_comparisons': sum(len(item['reference']['f64_literals']) for item in results if item.get('reference') is not None),
            'source_debug_refusals': sum('previous_source_debug' in item for item in results),
            'source_debug_conformance_refusals': sum('previous_source_debug_raw' in item for item in results)}


def instruction_inventory(data):
    """Lexical spellings only; no claim of finite range, type or CFG validity."""
    result = debug_proof.instruction_inventory(data)
    result.update(profile=11, lane='raw_typed_scalars_conformance',
                  scope='profile-11 lexical coverage only; finite conversion, scalar identities, call signatures, authority and CFG require complete validation')
    if result['unsupported_format']:
        return result
    identifier = r'[A-Za-z_][A-Za-z_0-9]*'
    nominal = identifier + r'(?:::' + identifier + r')*'
    lines = data.decode('utf-8').splitlines()
    names = {match[1] for line in lines if (match := re.fullmatch(r'(?:struct|enum) (' + nominal + r')', line.strip()))}
    types = ['i32', 'i64', 'bool', 'u8', 'f64', 'text', 'array<i64>', 'array<text>', *sorted(names)]
    kind = '(?:' + '|'.join(re.escape(value) for value in types) + ')'
    header = re.compile(r'function (' + identifier + r')(?:\(([^()]*)\))? -> (' + kind + r'|void):')
    functions = {}
    for line in lines:
        match = header.fullmatch(line.strip())
        if match:
            parameters = match[2].split(', ') if match[2] else []
            if all(re.fullmatch(identifier + r': ' + kind, parameter) for parameter in parameters):
                functions[match[1]] = len(parameters)
    result['unsupported_declarations'] = [item for item in result['unsupported_declarations']
        if not (item['category'] == 'unsupported_function_shape' and header.fullmatch(item['text'].strip()) and
                header.fullmatch(item['text'].strip())[1] in functions)]
    for item in result['instructions']:
        opcode, operands = item['opcode'], item['operands']
        if opcode == 'PUSH_CONST' and len(operands) == 1 and any(c in operands[0] for c in '.eE') and float_spelling.fullmatch(operands[0]):
            item['supported_opcode'] = item['supported_form'] = True
        elif opcode == 'CAST' and len(operands) == 1 and operands[0] in {'bool', 'u8', 'f64'}:
            item['supported_opcode'] = item['supported_form'] = True
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
        if number in (4, 42) and record.get('target_verified') is not True:
            failures.append(f'complete target not independently verified: {number}')
        if number in known_raw_matches and record.get('candidate', {}).get('source_debug') != []:
            failures.append(f'non-debug complete edition gained metadata: {number}')
        if number in expected_raw_matches and number != 42 and record.get('candidate', {}).get('f64_literals') != []:
            failures.append(f'previous scalar-free edition gained floats: {number}')
    errors = dict(Counter(item['candidate']['error'] for item in records if item.get('outcome') == 'validation_refused'))
    expected_errors = {'manifest_unsupported': 19, 'manifest_edition': 9}
    if errors != expected_errors:
        failures.append('raw validation refusal inventory changed')
    return {'passed': not failures, 'failures': failures, 'matched_invocations': matched,
            'expected_matched_invocations': expected_raw_matches, 'expected_refusal_errors': expected_errors,
            'edition_pins': [{'invocation': item['invocation'], 'bytes': item.get('edition_bytes'), 'sha256': item.get('edition_sha256')}
                             for item in records if item['invocation'] in expected_raw_matches],
            'temporary_source_paths': 'Invocation4 retains its exact fresh capture and is never normalized.'}


def compare_scalar_target(target, reference):
    require((target['edition_bytes'], target['edition_sha256']) == known_raw_matches[42], 'complete scalar target changed')
    require(target['invocation'] == 42 and target['root_case'] == 'text-and-types.matrix' and target['category'] == 'root_execution',
            'scalar root identity changed')
    expected_literals = [{'function': 'demo__text__ratio', 'instruction_index': 0, 'bits_hex': '3fe0000000000000'}]
    for value in (target['candidate'], reference):
        require(value['status'] == 'returned' and value['result'] == 0 and value['error'] is None and
                value['output_hex'] == scalar_output.hex(), 'complete scalar target full-result semantics differ')
        require(value['source_debug'] == [] and value['f64_literals'] == expected_literals, 'complete scalar target metadata differs')
    require(target['candidate']['instructions'] == 37 and target['candidate']['console_calls'] == 6 and
            target['candidate']['output_bytes'] == 200, 'complete scalar target accounting differs')
    return expected_literals


def verify_complete_targets(args, output, environment, records):
    calls = baseline.strict_json((output / 'corpus-calls.json').read_text())
    for number in (4, 42):
        target = records[number]; path = output / target['retained_path']; data = path.read_bytes()
        require(len(data) == target['edition_bytes'] and digest(data) == target['edition_sha256'], 'retained complete target changed')
        process, stdout, _ = baseline.run([args.reference, path], output / f'corpus-full-reference-{number:03d}', environment=environment)
        reference = parse_probe(process, stdout, reference=True)
        if number == 4:
            source_path, metadata = debug_proof.compare_target_receipt(target, calls[53], reference)
            require(target['candidate']['f64_literals'] == reference['f64_literals'] == [], 'old debug target gained floats')
            receipt = {'invocation': 4, 'source_path': source_path, 'source_debug': metadata, 'paths_normalized': False}
        else:
            receipt = {'invocation': 42, 'f64_literals': compare_scalar_target(target, reference), 'root_case': 'text-and-types.matrix',
                       'functions': 6, 'unused_helpers_preserved': True}
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
        record['lane'] = 'raw_typed_scalars_conformance'
    raw.update(lane='raw_typed_scalars_conformance',
               unsupported_or_refused_executions=sum(item['outcome'] in {'validation_refused', 'execution_refused'} for item in records),
               refusal_errors=dict(Counter(item['candidate']['error'] for item in records if item['outcome'] in {'validation_refused', 'execution_refused'})),
               scope='Exact unchanged upstream semantics; production intent is a separate lane. Refusal is unmet coverage.',
               regression_contract=raw_regression_contract(records))
    write_json(output / 'corpus-executions.json', records); write_json(output / 'corpus-summary.json', raw)
    production_records, failures, cross = [], [], []
    for record in records:
        number = record['invocation']
        item = {key: record[key] for key in ('invocation', 'upstream_call', 'edition_sha256', 'edition_bytes', 'retained_path')}
        item['lane'] = 'production_profile11_admission'
        try:
            path = output / record['retained_path']; data = path.read_bytes()
            require(len(data) == record['edition_bytes'] and digest(data) == record['edition_sha256'], 'corpus bytes changed between lanes')
            process, stdout, _ = baseline.run([args.production_candidate, path, '1'], output / f'corpus-production-{number:03d}', environment=environment)
            candidate = parse_production(process, stdout); item['candidate'] = candidate
            require(candidate['status'] == 'admission_refused', 'unmodified corpus unexpectedly passed production admission')
            item['outcome'] = 'production_refused'
            if number == 42:
                pair = dict(item, lane='prior_source_debug_grammar')
                for name, tool, parser, status in (('previous_source_debug', args.source_debug_candidate, debug_proof.parse_production, 'admission_refused'),
                                                   ('previous_source_debug_raw', args.source_debug_conformance_candidate, debug_proof.parse_probe, 'validation_refused')):
                    process, stdout, _ = baseline.run([tool, path, '1'], output / f"corpus-{name.replace('_', '-')}-{number:03d}", environment=environment)
                    previous = parser(process, stdout)
                    require(previous['status'] == status and previous['error'] == 'manifest_unsupported', 'previous complete scalar target contract changed')
                    pair[name] = previous
                cross.append(pair)
        except (AssertionError, ValueError, OSError) as error:
            item.update(outcome='failed', failure=str(error)); failures.append(number)
        production_records.append(item)
    errors = dict(Counter(item['candidate']['error'] for item in production_records if item['outcome'] == 'production_refused'))
    expected = {'manifest_unsupported': 16, 'manifest_edition': 9, 'manifest_intent': 20}
    passed = not failures and len(production_records) == 45 and errors == expected and len(cross) == 1
    production = {'lane': 'production_profile11_admission', 'passed': passed, 'invocations': len(production_records), 'failures': failures,
                  'refusals': sum(item['outcome'] == 'production_refused' for item in production_records),
                  'refusal_errors': errors, 'expected_refusal_errors': expected, 'source_debug_corpus_comparisons': len(cross),
                  'scope': 'Production admission attempts on identical complete editions; no raw semantic coverage credit'}
    write_json(output / 'production-corpus-executions.json', production_records); write_json(output / 'production-corpus-summary.json', production)
    write_json(output / 'source-debug-corpus-executions.json', cross)
    return raw, production


def qualification(args):
    binary_names = ('candidate', 'production_candidate', 'source_debug_candidate', 'source_debug_conformance_candidate', 'reference', 'compiler', 'interpreter')
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
    tools['reference_source'] = baseline.record_tool(Path(__file__).with_name('program_typed_scalars_reference.cpp'))
    tools['script'] = baseline.record_tool(Path(__file__))
    before = baseline.inventory(args.root)
    write_json(args.output / 'tools.json', tools); write_json(args.output / 'corpus-inventory.json', before)
    write_json(args.output / 'environment-policy.json', {
        'NIL_CRASH_REPORT': '0', 'removed': ['NY_LIBRARY_PATH', 'NIL_LIBRARY_PATH', 'NY_LEX_DUMP', 'NY_PARSE_TRACE', 'NY_STDLIB_PATH', 'NIL_MEMORY_PROFILE', 'NIL_STRICT_BOUNDS'],
        'upstream_argv': 'unchanged; original capture wrappers forward all arguments',
        'raw_lane': 'explicit validate_typed_scalars host entry; only missing original main intent is relaxed',
        'production_lane': 'explicit profile11 encoder/admission/execution on the same bytes',
        'limits': {'edition_bytes': 65536, 'static_instructions': 4096, 'steps': 65536, 'frames': 16, 'output_bytes': 65536,
                   'f64_values': 'finite binary64 bit patterns in existing operand/local/frame bounds; no arena',
                   'bool_values': [0, 1], 'u8_values': [0, 255]},
        'f64_receipt_scope': 'Exact validated literal bits, including unused helpers and dead code; not a runtime trace. Private executor tests verify copy/call preservation.',
        'excluded_conversions': 'integer/text to f64 and float to integral/text remain excluded; finite general-VM contrasts are explicit policy differences',
        'earlier_proofs': '126-,77-,61-,27-,55-,87-,103-,115-,105-,149- and111-case proofs remain separate required runs',
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
    result = {'schema': 1, 'status': 'bounded_typed_scalars_checkpoint_passed' if passed else 'bounded_typed_scalars_checkpoint_failed',
              'candidate_qualified': False, 'native_execution': False, 'production_admission_waived': False, 'language_profile': 11,
              'boundary_results': boundary, 'raw_corpus': raw, 'production_corpus': production,
              'input_manifest_sha256': before['input_manifest_sha256'], 'tools': tools}
    write_json(args.output / 'summary.json', result); print(json.dumps(result, sort_keys=True))
    return 0 if passed else 1


class proof_tests(unittest.TestCase):
    def test_explicit_inventory_and_prior_proof_pins(self):
        cases = generated_cases()
        self.assertEqual(len(cases), 200)
        self.assertEqual(len({case['name'] for case in cases}), 200)
        self.assertEqual(sum(not case['error'] for case in cases), 81)
        self.assertEqual(sum(case['validation'] and not case['scalar_policy'] for case in cases), 102)
        self.assertEqual(sum(case['scalar_policy'] for case in cases), 5)
        self.assertEqual(sum(case['policy'] for case in cases), 2)
        self.assertEqual(sum(bool(case['error']) and not case['validation'] and not case['policy'] for case in cases), 10)
        self.assertEqual(sum(case['cross_profile'] for case in cases), 96)
        self.assertEqual(sum(not case['validation'] or case['scalar_policy'] for case in cases), 98)
        self.assertEqual(sum(len(case['reference_f64_literals']) for case in cases if not case['validation'] or case['scalar_policy']), 40)
        self.assertTrue(all(type(case['instructions']) is int and len(case['edition']) <= 65536 for case in cases))
        self.assertTrue(all(case['origin'] == 'independently_authored_textual_nil' for case in cases))
        self.assertEqual(len(source_pins), 21)
        self.assertEqual(len(primary_pins), 9)
        self.assertEqual(len(expected_raw_matches), 17)

    def test_float_receipt_schema_refuses_nonfinite_noncanonical_and_invalid_locations(self):
        process = {'exit_code': 0, 'timed_out': False}
        literal = {'function': 'main', 'instruction_index': 0, 'bits_hex': '8000000000000000'}
        value = {'status': 'returned', 'result': 42, 'error': None, 'output_hex': '', 'instructions': 4,
                 'console_calls': 0, 'output_bytes': 0, 'source_debug': [], 'f64_literals': [literal]}
        self.assertEqual(parse_probe(process, json.dumps(value).encode()), value)
        mutations = []
        changed = copy.deepcopy(value); del changed['f64_literals']; mutations.append(changed)
        for field, field_value in (('bits_hex', '7ff0000000000000'), ('bits_hex', 'fff8000000000001'),
                                   ('bits_hex', '3FE0000000000000'), ('bits_hex', '0'), ('instruction_index', True),
                                   ('instruction_index', -1), ('instruction_index', 4096), ('function', 'main"')):
            changed = copy.deepcopy(value); changed['f64_literals'][0][field] = field_value; mutations.append(changed)
        changed = copy.deepcopy(value); changed['f64_literals'][0]['extra'] = 1; mutations.append(changed)
        changed = copy.deepcopy(value); changed['f64_literals'].append(literal); mutations.append(changed)
        changed = copy.deepcopy(value); changed['f64_literals'] = [dict(literal, instruction_index=2), literal]; mutations.append(changed)
        changed = copy.deepcopy(value); changed['f64_literals'] = [literal, dict(literal, function='helper'), dict(literal, instruction_index=1)]; mutations.append(changed)
        changed = copy.deepcopy(value); changed.update(status='validation_refused', result=None, error='manifest_syntax', instructions=0); mutations.append(changed)
        for changed in mutations:
            with self.assertRaises(AssertionError):
                parse_probe(process, json.dumps(changed).encode())

    def test_signed_zero_and_literal_locations_are_observed_independently_of_integer_output(self):
        case = next(case for case in generated_cases() if case['name'] == 'f64_literal_negative_zero')
        candidate = {'status': 'returned', 'result': 42, 'error': None, 'output_hex': '', 'instructions': 5,
                     'console_calls': 0, 'output_bytes': 0, 'source_debug': [], 'f64_literals': case['f64_literals']}
        reference = {'schema': 1, 'status': 'returned', 'result': 42, 'error': None, 'output_hex': '',
                     'source_debug': [], 'f64_literals': case['reference_f64_literals']}
        self.assertEqual(compare_case(case, candidate, reference, candidate), 'matching_semantics')
        for field, value in (('bits_hex', '0000000000000000'), ('instruction_index', 1), ('function', 'unused')):
            changed = copy.deepcopy(candidate); changed['f64_literals'][0][field] = value
            with self.assertRaises(AssertionError):
                compare_case(case, changed, reference, changed)
            changed = copy.deepcopy(reference); changed['f64_literals'][0][field] = value
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, candidate)
        for literals in ([], [*case['f64_literals'], *case['f64_literals']]):
            with self.assertRaises(AssertionError):
                compare_case(case, dict(candidate, f64_literals=literals), reference, candidate)
        case = next(case for case in generated_cases() if case['name'] == 'f64_same_kind_join_1')
        candidate.update(instructions=9, f64_literals=case['f64_literals'])
        reference['f64_literals'] = case['reference_f64_literals']
        self.assertEqual(compare_case(case, candidate, reference, candidate), 'matching_semantics')
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, dict(reference, f64_literals=list(reversed(reference['f64_literals']))), candidate)

    def test_bool_conversion_fault_is_an_actual_matching_vm_fault(self):
        case = next(case for case in generated_cases() if case['name'] == 'bool_refuses_two')
        candidate = {'status': 'execution_refused', 'result': None, 'error': 'scalar_conversion', 'output_hex': '', 'instructions': 2,
                     'console_calls': 0, 'output_bytes': 0, 'source_debug': [], 'f64_literals': []}
        reference = {'schema': 1, 'status': 'execution_refused', 'result': None,
                     'error': 'CAST: boolean values must be exactly 0 or 1', 'output_hex': '', 'source_debug': [], 'f64_literals': []}
        self.assertEqual(compare_case(case, candidate, reference, candidate), 'matching_runtime_refusal')
        for changed in (dict(reference, error='scalar_conversion'), dict(reference, error='timeout'), dict(reference, status='returned', result=1, error=None)):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, candidate)
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, reference, dict(candidate, status='admission_refused'))

    def test_excluded_scalar_conversion_is_policy_not_matching_execution(self):
        case = next(case for case in generated_cases() if case['name'] == 'policy_f64_to_u8_excluded')
        candidate = {'status': 'validation_refused', 'result': None, 'error': 'manifest_syntax', 'output_hex': '', 'instructions': 0,
                     'console_calls': 0, 'output_bytes': 0, 'source_debug': [], 'f64_literals': []}
        production = dict(candidate, status='admission_refused')
        reference = {'schema': 1, 'status': 'returned', 'result': 2, 'error': None, 'output_hex': '', 'source_debug': [], 'f64_literals': case['reference_f64_literals']}
        self.assertEqual(compare_case(case, candidate, reference, production), 'finite_scalar_policy_contrast')
        for changed in (dict(reference, result=0), dict(reference, f64_literals=[]), dict(reference, status='execution_refused', result=None, error='timeout')):
            with self.assertRaises(AssertionError):
                compare_case(case, candidate, changed, production)
        with self.assertRaises(AssertionError):
            compare_case(case, candidate, reference, dict(production, error='manifest_intent'))

    def test_lexical_inventory_does_not_replace_type_or_range_validation(self):
        for case in generated_cases():
            if not case['validation']:
                inventory = instruction_inventory(case['edition'])
                self.assertEqual(inventory['profile'], 11)
                self.assertIn('lexical', inventory['scope'])
                self.assertTrue(all(item['supported_form'] for item in inventory['instructions']), case['name'])
                self.assertEqual(inventory['unsupported_declarations'], [], case['name'])
        for name in ('reject_f64_range_overflow', 'reject_bool_arithmetic_rejects_implicit_erasure'):
            case = next(case for case in generated_cases() if case['name'] == name)
            self.assertTrue(all(item['supported_form'] for item in instruction_inventory(case['edition'])['instructions']))
        case = next(case for case in generated_cases() if case['name'] == 'reject_literal_hexfloat')
        self.assertTrue(any(not item['supported_form'] for item in instruction_inventory(case['edition'])['instructions']))
        previous = baseline.parse_probe, baseline.instruction_inventory
        with self.assertRaises(RuntimeError):
            with corpus_adapters():
                raise RuntimeError('deliberate')
        self.assertEqual(previous, (baseline.parse_probe, baseline.instruction_inventory))

    def test_positive_corpus_contract_retains_dynamic_debug_and_new_root_obligations(self):
        records, refused = [], 0
        for number in range(45):
            if number in expected_raw_matches:
                size, identity = known_raw_matches.get(number, (3511, 'a' * 64))
                records.append({'invocation': number, 'matched': True, 'outcome': 'returned_match', 'edition_bytes': size,
                                'edition_sha256': identity, 'candidate': {'error': None, 'source_debug': [], 'f64_literals': []}, 'target_verified': number in (4, 42)})
            else:
                error = 'manifest_unsupported' if refused < 19 else 'manifest_edition'; refused += 1
                records.append({'invocation': number, 'matched': False, 'outcome': 'validation_refused', 'candidate': {'error': error}})
        self.assertTrue(raw_regression_contract(records)['passed'])
        for number in expected_raw_matches:
            changed = copy.deepcopy(records); changed[number].update(matched=False, outcome='validation_refused', candidate={'error': 'manifest_unsupported'})
            self.assertFalse(raw_regression_contract(changed)['passed'])
        for number in known_raw_matches:
            changed = copy.deepcopy(records); changed[number]['edition_sha256'] = '0' * 64
            self.assertFalse(raw_regression_contract(changed)['passed'])
        for number in (4, 42):
            changed = copy.deepcopy(records); changed[number]['target_verified'] = False
            self.assertFalse(raw_regression_contract(changed)['passed'])
        self.assertFalse(raw_regression_contract(records[:-1])['passed'])

    def test_complete_scalar_target_rejects_low_byte_alias_and_metadata_or_identity_drift(self):
        literals = [{'function': 'demo__text__ratio', 'instruction_index': 0, 'bits_hex': '3fe0000000000000'}]
        candidate = {'status': 'returned', 'result': 0, 'error': None, 'output_hex': scalar_output.hex(), 'instructions': 37,
                     'console_calls': 6, 'output_bytes': 200, 'source_debug': [], 'f64_literals': literals}
        target = {'invocation': 42, 'root_case': 'text-and-types.matrix', 'category': 'root_execution', 'candidate': candidate,
                  'edition_bytes': known_raw_matches[42][0], 'edition_sha256': known_raw_matches[42][1]}
        reference = {'schema': 1, 'status': 'returned', 'result': 0, 'error': None, 'output_hex': scalar_output.hex(), 'source_debug': [], 'f64_literals': literals}
        self.assertEqual(compare_scalar_target(target, reference), literals)
        for field, value in (('result', 256), ('instructions', 36), ('console_calls', 5), ('f64_literals', []), ('output_hex', '')):
            changed = copy.deepcopy(target); changed['candidate'][field] = value
            with self.assertRaises(AssertionError):
                compare_scalar_target(changed, reference)
        for field, value in (('edition_bytes', 2989), ('edition_sha256', '0' * 64), ('root_case', None), ('category', 'auxiliary_execution')):
            with self.assertRaises(AssertionError):
                compare_scalar_target(dict(target, **{field: value}), reference)
        with self.assertRaises(AssertionError):
            compare_scalar_target(target, dict(reference, result=256))

    def test_numeric_limits_are_value_bounds_and_metadata_cannot_supply_intent(self):
        cases = generated_cases()
        zero = next(case for case in cases if case['name'] == 'f64_literal_zero_huge_positive_exponent')
        self.assertGreater(len(zero['edition']), 60000)
        self.assertEqual(zero['f64_literals'][0]['bits_hex'], '0000000000000000')
        underflow = next(case for case in cases if case['name'] == 'reject_f64_range_underflow')
        self.assertEqual(underflow['error'], 'manifest_syntax')
        malformed = next(case for case in cases if case['name'] == 'reject_literal_missing_exponent')
        self.assertEqual(malformed['error'], 'manifest_unsupported')
        absent = next(case for case in cases if case['name'] == 'typed_values_cannot_supply_main_intent')
        self.assertEqual(absent['production_error'], 'manifest_intent')
        self.assertNotIn(b'@ai_context(', absent['edition'])
        self.assertEqual(sum('_callable_parameter_signatures_remain_distinct' in case['name'] for case in cases), 3)
        self.assertEqual(sum('_cannot_bind_' in case['name'] for case in cases), 6)

    def test_reference_environment_and_all_prior_sources_stay_pinned(self):
        for name, expected in source_pins.items():
            self.assertEqual(digest(Path(__file__).with_name(name).read_bytes()), expected)
        environment = baseline.clean_environment()
        self.assertNotIn('NIL_STRICT_BOUNDS', environment)
        self.assertEqual(environment['NIL_CRASH_REPORT'], '0')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('candidate', 'production-candidate', 'source-debug-candidate', 'source-debug-conformance-candidate', 'reference', 'compiler', 'interpreter', 'root', 'output'):
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(proof_tests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    for name in ('candidate', 'production_candidate', 'source_debug_candidate', 'source_debug_conformance_candidate', 'reference', 'compiler', 'root', 'output'):
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
