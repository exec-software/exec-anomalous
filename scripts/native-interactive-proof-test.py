#!/usr/bin/env python3
"""Pure oracle rejection tests. Imports no guest implementation and starts no VM."""
import copy
import dataclasses
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('interactive_proof', Path(__file__).with_name('native-interactive-proof.py'))
proof = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = proof
spec.loader.exec_module(proof)


def record(event, **fields):
    return dict(schema=1, event=event, level='info', **fields)


def valid_trace():
    dynamic = dict(regions=9, usable_bytes=81920000, free_frames=20000,
                   unavailable_frames=1028576, pitch=4096, bpp=32)
    records = [record(event, **{k: dynamic[k] if v is int else v for k, v in fields.items()})
               for event, fields in proof.boot_events]
    records.append(record(proof.event_prefix+'ready', tasks=2, phase=1, proof_mode=0,
                          descriptor_bytes=192, arena_bytes=32768, data_frames=48,
                          table_frames=18, leases=12, x=32, y=32, width=384, height=144))
    packets = [[], [], [], []]

    def packet(channel, data):
        value = dict(channel=channel, sequence=len(packets[channel])+1, length=len(data), hash=proof.fnv(data))
        packets[channel].append(value)
        records.append(record(proof.event_prefix+'packet', **value))

    packet(3, proof.mount_ready(2, 2))
    packet(2, proof.wire(1, 0, 1, document=2))
    packet(3, proof.wire(2, 1, 1, version=2, text=b'a', document=2))
    frames = [dict(sequence=1, hash=0x123456789abcdef0)]
    records.append(record(proof.event_prefix+'frame', **frames[0]))
    packet(0, proof.observation(text=b'a', version=2).encode())
    for channel in range(4):
        count = len(packets[channel])
        records.append(record(proof.event_prefix+'channel', channel=channel,
                              sends=count, deliveries=count, acks=count))
    records.append(record(proof.event_prefix+'passed', phase=1, proof_mode=0, starts=2,
                          calls=300, switches=8, timer_ticks=0, inputs=2, input_losses=0,
                          frames=1, rows=144, denials=6, bad_copies=1, invalid=3,
                          closed=1, discarded=0, exits=2, faults=0, zeroed_bytes=65536,
                          resources_restored=True, authority_revoked=True, device_poisoned=False))
    return records, frames, packets


def media(previous, current):
    """Literal test-image construction independent of the guest disk codec."""
    data = proof.blank_disk()

    def sector(magic):
        value = bytearray(512)
        value[:8] = magic
        struct.pack_into('<HH', value, 8, 1, 512)
        struct.pack_into('<QQ', value, 16, 0xa110, 0xd001)
        return value

    def write(index, value):
        struct.pack_into('<I', value, 508, proof.crc32c(value[:508]))
        data[4096+index*512:4608+index*512] = value

    header = sector(b'ANDURHDR')
    struct.pack_into('<I', header, 12, 5)
    write(0, header)
    for epoch, version, text in (previous, current):
        sequence = epoch+version
        slot = (sequence-1) % 2
        body = sector(b'ANDURBOD')
        struct.pack_into('<QQQQ', body, 32, slot, sequence, epoch, version)
        body[64] = len(text)
        body[65:65+len(text)] = text
        write(1+2*slot, body)
        commit = sector(b'ANDURCMT')
        commit[32:64] = body[32:64]
        commit[64:68] = body[508:512]
        write(2+2*slot, commit)
    return data


class oracle_tests(unittest.TestCase):
    def check(self, records, frames=None, packets=None):
        _, expected_frames, expected_packets = valid_trace()
        return proof.validate_complete(records, 1, 0, frames or expected_frames,
                                       packets or expected_packets, 4)

    def test_valid_trace_and_literal_wire_offsets(self):
        records, frames, packets = valid_trace()
        self.check(records, frames, packets)
        value = proof.observation(text=b'bc', sequence=9, revision=5, version=2,
                                  pending_request=3, pending_revision=4, anchor=2, head=1,
                                  undo=1, dirty=1, outcome=1, feedback=2)
        encoded = value.encode()
        self.assertEqual(encoded[:8], b'ANUI\x01\x00\x80\x00')
        self.assertEqual(struct.unpack_from('<5Q', encoded, 8), (9, 5, 2, 3, 4))
        self.assertEqual(encoded[48:57], bytes([2, 2, 1, 1, 1, 1, 2, 0, 0]))
        self.assertEqual(encoded[57:64], bytes(7))
        self.assertEqual(encoded[64:], b'bc'+bytes(62))
        request = proof.wire(5, 7, 3, base=2, revision=4, text=b'B')
        self.assertEqual(struct.unpack_from('<6Q', request, 16), (1, 7, 3, 2, 0, 4))
        self.assertEqual(request[64:68], b'\x01B\x00\x00')

    def test_extra_missing_duplicate_and_reordered_records_reject(self):
        valid, _, _ = valid_trace()
        for index in range(len(valid)):
            with self.subTest(kind='missing', index=index):
                with self.assertRaises(ValueError):
                    self.check(valid[:index]+valid[index+1:])
            with self.subTest(kind='duplicate', index=index):
                with self.assertRaises(ValueError):
                    self.check(valid[:index]+[valid[index]]+valid[index:])
        for a, b in [(0, 1), (9, 10), (12, 13), (14, 15), (17, 18)]:
            changed = copy.deepcopy(valid)
            changed[a], changed[b] = changed[b], changed[a]
            with self.assertRaises(ValueError):
                self.check(changed)
        changed = copy.deepcopy(valid)
        changed.insert(12, record('kernel.unrelated.success'))
        with self.assertRaises(ValueError):
            self.check(changed)

    def test_fieldsets_numeric_types_and_wrong_hashes_reject(self):
        valid, _, _ = valid_trace()
        for index, source in enumerate(valid):
            changed = copy.deepcopy(valid)
            changed[index]['unexpected'] = 1
            with self.assertRaises(ValueError):
                self.check(changed)
            for field, value in source.items():
                if type(value) is not int:
                    continue
                for wrong in [True, float(value), str(value), -1, 1 << 64]:
                    changed = copy.deepcopy(valid)
                    changed[index][field] = wrong
                    with self.subTest(index=index, field=field, wrong=wrong):
                        with self.assertRaises(ValueError):
                            self.check(changed)
        for index in (9, 12, 13):
            changed = copy.deepcopy(valid)
            changed[index]['hash'] ^= 1
            with self.assertRaises(ValueError):
                self.check(changed)

    def test_channel_ack_loss_and_undetected_discard_reject(self):
        valid, _, _ = valid_trace()
        for index in (14, 16, 17):
            for key in ('sends', 'deliveries', 'acks'):
                changed = copy.deepcopy(valid)
                changed[index][key] -= 1
                with self.assertRaises(ValueError):
                    self.check(changed)
        for key in ('discarded', 'denials', 'invalid', 'bad_copies', 'closed', 'zeroed_bytes'):
            changed = copy.deepcopy(valid)
            changed[-1][key] += 1
            with self.assertRaises(ValueError):
                self.check(changed)

    def test_serial_partial_tail_duplicate_fields_and_nonfinite_values_reject(self):
        valid, _, _ = valid_trace()
        text = ''.join(json.dumps(record)+'\n' for record in valid)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'serial'
            path.write_text(text)
            self.assertEqual(proof.transcript(path, complete=True), valid)
            for changed in [text.rstrip('\n'), text+'{', text.replace('"schema": 1', '"schema": 1, "schema": 1', 1),
                            text.replace('"calls": 300', '"calls": NaN')]:
                path.write_text(changed)
                with self.assertRaises(ValueError):
                    proof.transcript(path, complete=True)

    def test_host_observation_rejects_false_clean_unknown_and_bad_pending(self):
        baseline = proof.observation(text=b'a')
        for values in [dict(sequence=True), dict(anchor=2), dict(text=b'\n'), dict(feedback=14),
                       dict(outcome=1), dict(pending_request=1), dict(armed=1, closed=1),
                       dict(outcome=2, pending_request=1, pending_revision=1),
                       dict(outcome=1, pending_request=1, pending_revision=2), dict(dirty=True)]:
            with self.assertRaises(ValueError):
                dataclasses.replace(baseline, **values).encode()
        for args in [(1, 1, 1), (5, 1, 1), (7, 1, True), (9, 1, 1)]:
            with self.assertRaises(ValueError):
                proof.wire(*args)
        for text in (b'x'*32, b'\n', b'\xff'):
            with self.assertRaises(ValueError):
                proof.wire(5, 1, 1, base=1, revision=2, text=text)
        with self.assertRaises(ValueError):
            proof.wire(6, 1, 1, base=1, version=3, revision=2, text=b'a')

    def test_provisioning_contains_only_phase_and_mode_without_answer(self):
        for phase in (0, 1):
            for mode in (0, 1, 2):
                data = proof.fixture(phase, mode)
                self.assertEqual(len(data), 64)
                self.assertEqual(data[:8], b'ANDF\x01\x00\x40\x00')
                self.assertEqual(struct.unpack_from('<2Q', data, 8), (phase, mode))
                self.assertEqual(data[24:], bytes(40))
        for phase, mode in [(True, 0), (0, False), (2, 0), (0, 3)]:
            with self.assertRaises(ValueError):
                proof.fixture(phase, mode)

    def test_media_requires_both_known_snapshots_and_extent_integrity(self):
        old, new = (1, 2, b'B'), (1, 3, b'C')
        valid = media(old, new)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'disk'
            path.write_bytes(valid)
            self.assertEqual(proof.inspect_media(path, *new, old)['text_hex'], '43')
            # Even recomputing every checksum cannot hide an incorrect previous
            # committed text, wrong identity, illegal transition or recovered text.
            mutations = [media((1, 2, b'X'), new), media(old, (1, 3, b'X')),
                         media((0, 3, b'C'), new)]
            for offset in (0, 4095, 4096, 4096+65+512, 6656, len(valid)-1):
                changed = bytearray(valid)
                changed[offset] ^= 1
                mutations.append(changed)
            for changed in mutations:
                path.write_bytes(changed)
                with self.assertRaises(ValueError):
                    proof.inspect_media(path, *new, old)


if __name__ == '__main__':
    unittest.main()
