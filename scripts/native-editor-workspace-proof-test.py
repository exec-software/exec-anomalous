#!/usr/bin/env python3
"""Pure host-oracle rejection tests; no compilation, guest model or VM."""
import copy
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('native_workspace_oracle', Path(__file__).with_name('native-editor-workspace-proof.py'))
proof = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = proof
spec.loader.exec_module(proof)


def record(event, **values):
    return dict(schema=1, event=event, level='info', **values)


def valid_trace():
    dynamic = dict(regions=9, usable_bytes=81920000, free_frames=20000,
        unavailable_frames=1028576, pitch=4096, bpp=32)
    records = [record(event, **{k: dynamic[k] if v is int else v for k, v in fields.items()})
               for event, fields in proof.startup_events]
    records.append(record(proof.prefix+'ready', tasks=3, phase=1, proof_mode=0,
        descriptor_bytes=192, owned_frames=144, owned_leases=18, stage_frames=576,
        connection=1, free_before=20000, free_live=19280))
    for role in range(3):
        records.append(record(proof.prefix+'started', role=role, cpu_clean=True))
    packets = [[], [], [], []]
    def packet(channel, data):
        producer, consumer = proof.pairs[channel]
        item = dict(channel=channel, sequence=len(packets[channel])+1, producer=producer,
            consumer=consumer, length=len(data), hash=proof.fnv(data))
        packets[channel].append(item)
        records.append(record(proof.prefix+'packet', **item))
    state = proof.observation(text=b'n', version=2)
    packet(3, proof.mount_ready(2, 2))
    packet(2, proof.wire(1, 0, 1, document=2))
    packet(3, proof.wire(2, 1, 1, version=2, text=b'n', document=2))
    packet(1, proof.message(state))
    packet(0, proof.action(1, 1, 12))
    packet(1, proof.message(state.next(feedback=13, closed=1), 1, 1, 4))
    frames = [dict(sequence=i, hash=i*100, width=1024, height=768, rows=768) for i in range(1, 4)]
    records += [record(proof.prefix+'frame', **f) for f in frames]
    for index, role in enumerate((1, 2, 0)):
        records.append(record(proof.prefix+'retired', role=role, faulted=False,
            closed_routes=4 if index == 0 else 0, discarded=0, survivors=2-index))
    for channel in range(4):
        count = len(packets[channel])
        records.append(record(proof.prefix+'channel', channel=channel, sends=count, deliveries=count, acks=count))
    records.append(record(proof.prefix+'passed', phase=1, proof_mode=0, starts=3,
        calls=3000, switches=40, timer_ticks=9, inputs=2, input_losses=0,
        frames=3, rows=2304, denials=11, bad_copies=0, invalid=0, closed=2,
        discarded=0, exits=3, faults=0, resources_restored=True, authority_revoked=True,
        cpu_restored=True, device_poisoned=False, free_frames_before=20000,
        free_frames_after=20000, zeroed_bytes=102400, cpu_captures=3012,
        cpu_clean_checks=3012, user_irqs=12))
    return records, frames, packets


class oracle_tests(unittest.TestCase):
    def verify(self, records, frames=None, packets=None):
        if frames is None:
            _, frames, packets = valid_trace()
        return proof.validate_complete(records, 1, 0, frames, packets)

    def test_literal_wire_envelopes(self):
        expected = bytearray(64)
        expected[:8] = bytes((65, 78, 69, 65, 1, 0, 64, 0))
        expected[8], expected[16], expected[24], expected[32], expected[40] = 1, 9, 7, 1, 110
        self.assertEqual(proof.action(9, 7, 1, 110), expected)
        state = proof.observation(text=b'n', sequence=8, revision=2, anchor=1,
            head=1, undo=1, dirty=1, feedback=1)
        value = proof.message(state, 9, 7, 1)
        self.assertEqual(value[:16], b'ANER\x01\x00\xc0\x00\x02\x01'+bytes(6))
        self.assertEqual(struct.unpack_from('<3Q', value, 16), (1, 9, 7))
        self.assertEqual(value[40:64], bytes(24))
        self.assertEqual(value[64:], state.encode())
        held = proof.held_release(1, 6, 7)
        self.assertEqual(held[:8], b'ANDH\x01\x00\x80\x00')
        self.assertEqual(struct.unpack_from('<6Q', held, 8), (1, 1, 1, 1, 6, 7))
        self.assertEqual(held[56:], bytes(72))

    def test_expected_desktop_history_retains_pointer_and_observation_publications(self):
        history = proof.desktop_history()
        history.observation(None)
        self.assertEqual(history.encode(), b'')
        initial = proof.observation()
        edited = initial.next(text=b'n', revision=2, anchor=1, head=1, undo=1, dirty=1, feedback=1)
        closed = edited.next(feedback=13, closed=1)
        history.observation(initial)
        history.pointer(528, 384)
        history.pointer(512, 384)
        before_edit = b'anui '+initial.encode().hex().encode()+b'\npointer 528 384\npointer 512 384\n'
        self.assertEqual(history.encode(), before_edit)
        history.observation(edited)
        history.observation(closed)
        self.assertEqual(history.encode(), before_edit+b'anui '+edited.encode().hex().encode()
                         +b'\nanui '+closed.encode().hex().encode()+b'\n')
        for x, y in [(1024, 384), (512, 768), (-1, 0), (True, 0)]:
            with self.assertRaises(ValueError):
                history.pointer(x, y)
        with self.assertRaises(ValueError):
            history.observation(None)

    def test_complete_three_owner_trace_is_required(self):
        records, frames, packets = valid_trace()
        self.verify(records, frames, packets)
        for length in range(len(records)):
            with self.assertRaises((ValueError, KeyError, IndexError)):
                self.verify(records[:length])

    def test_every_evidence_field_is_strict(self):
        records, _, _ = valid_trace()
        for index, event in enumerate(records):
            for key in event:
                changed = copy.deepcopy(records)
                del changed[index][key]
                with self.assertRaises(ValueError, msg=f'missing {index}.{key}'):
                    self.verify(changed)
            changed = copy.deepcopy(records)
            changed[index]['unreviewed'] = 0
            with self.assertRaises(ValueError):
                self.verify(changed)
            changed = copy.deepcopy(records)
            changed[index]['level'] = 'debug'
            with self.assertRaises(ValueError):
                self.verify(changed)

    def test_pointer_ready_prelude_is_exact_and_does_not_change_pair_boot(self):
        records, _, _ = valid_trace()
        self.assertEqual(len(proof.base.boot_events), 8)
        self.assertEqual(proof.startup_events[:-1], tuple(proof.base.boot_events))
        pointer = len(proof.base.boot_events)
        self.assertEqual(records[pointer], record('workspace_pointer.ready', available=True, generation=1))
        mutations = [records[:pointer]+records[pointer+1:],
            records[:pointer]+[records[pointer]]+records[pointer:],
            records[:pointer]+[records[pointer+1], records[pointer]]+records[pointer+2:],
            records[:pointer-1]+[records[pointer], records[pointer-1]]+records[pointer+1:]]
        for key, value in [('available', False), ('available', 1), ('generation', 2), ('generation', True)]:
            changed = copy.deepcopy(records)
            changed[pointer][key] = value
            mutations.append(changed)
        for changed in mutations:
            with self.assertRaises(ValueError):
                self.verify(changed)

    def test_route_identity_shape_and_literal_packet_hash_are_bound(self):
        records, _, _ = valid_trace()
        index = next(i for i, r in enumerate(records) if r['event'] == proof.prefix+'packet' and r['channel'] == 1)
        for key, value in [('channel', 0), ('producer', 2), ('consumer', 1), ('sequence', 2), ('length', 128), ('hash', 0)]:
            changed = copy.deepcopy(records)
            changed[index][key] = value
            with self.assertRaises(ValueError, msg=key):
                self.verify(changed)

    def test_owners_frames_cpu_and_allocator_cannot_be_partial(self):
        records, _, _ = valid_trace()
        mutations = [('free_frames_after', 19999), ('cpu_captures', 3013), ('cpu_clean_checks', 3011),
            ('user_irqs', 13), ('zeroed_bytes', 65536), ('rows', 2303), ('starts', 2),
            ('exits', 2), ('faults', 1), ('input_losses', 1), ('resources_restored', False),
            ('denials', 10), ('bad_copies', 1), ('invalid', 1),
            ('authority_revoked', False), ('cpu_restored', False), ('device_poisoned', True)]
        for key, value in mutations:
            changed = copy.deepcopy(records)
            changed[-1][key] = value
            with self.assertRaises(ValueError, msg=key):
                self.verify(changed)
        for suffix, key, value in [('started', 'role', 0), ('started', 'cpu_clean', False),
            ('frame', 'rows', 767), ('retired', 'discarded', 1), ('retired', 'faulted', True),
            ('retired', 'survivors', 0), ('channel', 'acks', 0)]:
            changed = copy.deepcopy(records)
            indices = [i for i, r in enumerate(records) if r['event'] == proof.prefix+suffix]
            index = indices[1] if suffix == 'started' and key == 'role' else indices[0]
            changed[index][key] = value
            with self.assertRaises(ValueError, msg=f'{suffix}.{key}'):
                self.verify(changed)

    def test_unknown_duplicate_out_of_order_and_post_terminal_records_reject(self):
        records, _, _ = valid_trace()
        ready = proof.ready_index
        mutations = [records + [records[-1]], records[:ready+1]+[records[ready]]+records[ready+1:],
            records[:ready+2]+[record(proof.prefix+'unreviewed')]+records[ready+2:],
            records[:-1]+[records[ready+4]]+[records[-1]]]
        for changed in mutations:
            with self.assertRaises(ValueError):
                self.verify(changed)

    def test_independent_pixel_oracle_detects_geometry_and_every_glyph_row_and_column(self):
        pixels = bytearray(1024*768*3)
        def paint(sx, sy, color):
            x = 216+(sx*592+127)//128
            y = 216+(sy*360+47)//48
            pixels[(y*1024+x)*3:(y*1024+x)*3+3] = bytes(color)
        for baseline, rows, color in [(0, (14, 17, 17, 31, 17, 17, 17), (144, 205, 255)),
                (8, (0, 0, 22, 25, 17, 17, 17), (236, 241, 247))]:
            for row, bits in enumerate(rows):
                for col in range(5):
                    paint(col, row+baseline, color if bits & (1 << (4-col)) else (18, 24, 34))
        state = proof.observation(text=b'n', head=1)
        proof.independent_pixels(pixels, state)
        with self.assertRaises(ValueError):
            proof.independent_pixels(pixels[:-1], state)
        for baseline in (0, 8):
            for row in range(7):
                for col in range(5):
                    with self.subTest(glyph_y=baseline, row=row, column=col):
                        changed = bytearray(pixels)
                        x = 216+(col*592+127)//128
                        y = 216+((row+baseline)*360+47)//48
                        changed[(y*1024+x)*3] ^= 1
                        with self.assertRaises(ValueError):
                            proof.independent_pixels(changed, state)

    def test_media_identity_and_extent_are_independent_of_old_pair_identity(self):
        data = proof.base.blank_disk()
        def sector(magic):
            value = bytearray(512)
            value[:8] = magic
            struct.pack_into('<HH', value, 8, 1, 512)
            struct.pack_into('<QQ', value, 16, 0x414e4f4d414c4f55, 0x52454d4f54450001)
            return value
        def write(index, value):
            struct.pack_into('<I', value, 508, proof.base.crc32c(value[:508]))
            data[4096+index*512:4608+index*512] = value
        header = sector(b'ANDURHDR')
        struct.pack_into('<I', header, 12, 5)
        write(0, header)
        for epoch, version, text in [(1, 1, b''), (1, 2, b'n')]:
            sequence = epoch+version
            slot = (sequence-1) % 2
            body = sector(b'ANDURBOD')
            struct.pack_into('<4Q', body, 32, slot, sequence, epoch, version)
            body[64] = len(text)
            body[65:65+len(text)] = text
            write(1+2*slot, body)
            commit = sector(b'ANDURCMT')
            commit[32:64] = body[32:64]
            commit[64:68] = body[508:512]
            write(2+2*slot, commit)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'fixture.raw'
            path.write_bytes(data)
            self.assertEqual(proof.inspect_media(path, 1, 2, b'n', (1, 1, b''))['text_hex'], '6e')
            for offset in [0, 4095, 6656, len(data)-1]:
                changed = bytearray(data)
                changed[offset] ^= 1
                path.write_bytes(changed)
                with self.assertRaises(ValueError):
                    proof.inspect_media(path, 1, 2, b'n', (1, 1, b''))
            # A correctly checksummed old-profile header must still fail identity.
            changed = bytearray(data)
            struct.pack_into('<2Q', changed, 4096+16, 0xa110, 0xd001)
            struct.pack_into('<I', changed, 4096+508, proof.base.crc32c(changed[4096:4096+508]))
            path.write_bytes(changed)
            with self.assertRaisesRegex(ValueError, 'disk identity'):
                proof.inspect_media(path, 1, 2, b'n', (1, 1, b''))


if __name__ == '__main__':
    unittest.main()
