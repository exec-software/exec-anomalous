#!/usr/bin/env python3
"""Physical-keyboard/cold-media proof for the experimental native editor pair.

Host observations are literal ANUI records, not guest-model replay. The raster
converter shares the published renderer (and is deliberately not an independent
font oracle); every complete guest frame hash and stable captured pixel must
match. The media parser below is independent of the guest codec/store.
"""
import argparse
import dataclasses
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import struct
import subprocess
import tempfile
import time

event_prefix = 'kernel.native_interactive_test.'
diagnostic_limit = 256 * 1024
u64_max = (1 << 64) - 1
common_fields = {'schema', 'event', 'level'}
boot_events = [
    ('boot.kernel.enter', dict(arch='x86_64')),
    ('kernel.fault_path.ready', dict(vectors=32, tss=True, double_fault_ist=1, nmi_ist=2, nmi_masked=True)),
    ('boot.omen.accepted', dict(version=2, boot_drive=128)),
    ('boot.memory.map', dict(regions=int, usable_bytes=int)),
    ('kernel.memory.ready', dict(frame_size=4096, address_limit=1 << 32, total_frames=1 << 20,
                               free_frames=int, unavailable_frames=int, data_owned_frames=0, table_owned_frames=0)),
    ('boot.display.ready', dict(width=1024, height=768, pitch=int, bpp=int)),
    ('boot.ready', dict(control_abi='0.1', framebuffer=True)),
    ('kernel.interrupts.ready', dict(backend='legacy_pc', vector_base=32, vector_count=16,
                                   pit_input_hz=1193182, pit_divisor=11932, timebase_generation=1,
                                   initial_master_mask=255, initial_slave_mask=255, master_mask=252,
                                   slave_mask=255, input_available=True, input_source_generation=1,
                                   input_queue_capacity=64, nmi_masked=True, if_enabled=True)),
]
event_fields = {
    'ready': set('tasks phase proof_mode descriptor_bytes arena_bytes data_frames table_frames leases x y width height'.split()),
    'packet': set('channel sequence length hash'.split()),
    'frame': {'sequence', 'hash'},
    'channel': set('channel sends deliveries acks'.split()),
    'passed': set('phase proof_mode starts calls switches timer_ticks inputs input_losses frames rows denials bad_copies invalid closed discarded exits faults zeroed_bytes resources_restored authority_revoked device_poisoned'.split()),
}
bool_fields = {'resources_restored', 'authority_revoked', 'device_poisoned'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fnv(data):
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return value


def bounded_read(path, limit):
    with path.open('rb') as stream:
        data = stream.read(limit+1)
    require(len(data) <= limit, f'{path.name} exceeds its byte limit')
    return data


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


def uint(value):
    return type(value) is int and 0 <= value <= u64_max


def fields(record, event, expected):
    require(type(record) is dict and set(record) == common_fields | set(expected), f'fields for {event}')
    require(type(record['schema']) is int and record['schema'] == 1
            and record['event'] == event and record['level'] == 'info', f'header for {event}')
    for key, value in expected.items():
        actual = record[key]
        if value is int:
            require(uint(actual), f'{event}.{key} must be u64')
        else:
            require(type(actual) is type(value) and actual == value, f'{event}.{key} mismatch')


def validate_prefix(records):
    """Every complete line is checked even while the guest is still running."""
    for index, record in enumerate(records):
        if index < len(boot_events):
            event, expected = boot_events[index]
            fields(record, event, expected)
            continue
        require(type(record) is dict and type(record.get('event')) is str, 'missing event')
        event = record['event']
        require(event.startswith(event_prefix), 'unexpected event after boot prerequisites')
        suffix = event[len(event_prefix):]
        require(suffix in event_fields, 'unknown interactive event')
        expected = {key: (False if key == 'device_poisoned' else True)
                    if key in bool_fields else int for key in event_fields[suffix]}
        fields(record, event, expected)
        if suffix == 'packet':
            require(record['channel'] < 4 and record['sequence'] > 0 and 0 < record['length'] <= 4096,
                    'invalid packet geometry')
        elif suffix == 'frame':
            require(record['sequence'] > 0, 'zero frame sequence')
        elif suffix == 'channel':
            require(record['channel'] < 4, 'invalid teardown channel')
    if len(records) >= 5:
        memory, allocator = records[3:5]
        require(0 < memory['regions'] <= 128 and allocator['free_frames'] > 0
                and allocator['free_frames'] * 4096 == memory['usable_bytes']
                and allocator['free_frames'] + allocator['unavailable_frames'] == 1 << 20,
                'boot memory accounting')
    if len(records) >= 6:
        display = records[5]
        require(display['bpp'] in (24, 32) and display['pitch'] >= 1024 * (display['bpp'] // 8),
                'boot display geometry')
    if len(records) > 8:
        require(records[8]['event'] == event_prefix+'ready', 'ready must immediately follow boot')
    for index, record in enumerate(records[9:], 9):
        suffix = record['event'][len(event_prefix):]
        require(suffix != 'ready', 'duplicate ready')
        if suffix == 'channel':
            require(index >= 9 and all(r['event'] not in (event_prefix+'packet', event_prefix+'frame')
                                      for r in records[index+1:]), 'event after teardown begins')
        if suffix == 'passed':
            require(index == len(records)-1, 'event after completion')


def transcript(path, complete=False):
    if not path.exists():
        return []
    data = bounded_read(path, diagnostic_limit)
    if complete:
        require(data.endswith(b'\n'), 'unterminated final serial record')
    lines = data.rsplit(b'\n', 1)[0].splitlines() if b'\n' in data else []
    records = []
    for line in lines:
        require(line.startswith(b'{'), 'non-JSON serial line')
        record = json.loads(line, object_pairs_hook=unique_fields,
                            parse_constant=lambda value: require(False, f'nonfinite JSON {value}'))
        records.append(record)
    validate_prefix(records)
    return records


def validate_complete(records, phase, mode, frames, packets, inputs):
    validate_prefix(records)
    require(len(records) >= 14, 'incomplete lifecycle')
    require([r['event'] for r in records[-5:]] == [event_prefix+'channel']*4 + [event_prefix+'passed'],
            'exact teardown tail required')
    require([r['channel'] for r in records[-5:-1]] == list(range(4)), 'teardown channel order')
    require(all(r['event'] in (event_prefix+'packet', event_prefix+'frame') for r in records[9:-5]),
            'unexpected lifecycle record')
    expected_ready = dict(tasks=2, phase=phase, proof_mode=mode, descriptor_bytes=192,
                          arena_bytes=32768, data_frames=48, table_frames=18, leases=12,
                          x=32, y=32, width=384, height=144)
    fields(records[8], event_prefix+'ready', expected_ready)
    actual_frames = [{k: r[k] for k in event_fields['frame']}
                     for r in records if r['event'] == event_prefix+'frame']
    require(actual_frames == frames, 'complete frame trace mismatch')
    require([f['sequence'] for f in frames] == list(range(1, len(frames)+1)), 'host frame sequence')
    seen_requests = 0
    seen_replies = 0
    seen_frames = 0
    for record in records[9:-5]:
        if record['event'] == event_prefix+'frame':
            seen_frames += 1
        elif record['channel'] == 0:
            require(record['sequence'] == seen_frames, 'audit must follow its committed frame')
        elif record['channel'] == 2:
            seen_requests += 1
            require(seen_replies == seen_requests, 'request overtook preceding reply')
        elif record['channel'] == 3:
            seen_replies += 1
            require(seen_replies <= seen_requests + 1, 'reply precedes corresponding request')
    for channel in range(4):
        actual = [{k: r[k] for k in event_fields['packet']}
                  for r in records if r['event'] == event_prefix+'packet' and r['channel'] == channel]
        require(actual == packets[channel], f'channel {channel} packet mismatch')
        require([r['sequence'] for r in actual] == list(range(1, len(actual)+1)), 'packet sequence gap')
        require(all(r['length'] == 128 for r in actual), 'unexpected fixture packet length')
        fields(records[-5+channel], event_prefix+'channel', dict(channel=channel,
               sends=len(actual), deliveries=len(actual), acks=len(actual)))
    require(not packets[1], 'reserved reverse route used')
    result = records[-1]
    expected = dict(phase=phase, proof_mode=mode, starts=2, input_losses=0,
                    frames=len(frames), rows=144*len(frames), exits=2, faults=0,
                    zeroed_bytes=65536, discarded=0, closed=1,
                    denials=6, bad_copies=1, invalid=3,
                    resources_restored=True, authority_revoked=True, device_poisoned=False)
    for key, value in expected.items():
        require(type(result[key]) is type(value) and result[key] == value, f'completion {key}')
    minimum_calls = (2 + result['rows'] + result['frames'] + result['inputs']
                     + 3*sum(len(channel) for channel in packets) + result['switches']
                     + result['denials'] + result['bad_copies'] + result['invalid']
                     + result['closed'] + result['exits'])
    require(result['switches'] > 0 and result['calls'] >= minimum_calls,
            'call total cannot account for reported operations')
    # Ctrl and Q make records cause terminal close. There is no subsequent
    # gate15 read, so their two release records are intentionally not consumed.
    require(result['inputs'] == inputs-2, 'physical input count')
    return result


@dataclasses.dataclass
class observation:
    text: bytes = b''
    sequence: int = 1
    revision: int = 1
    version: int = 1
    pending_request: int = 0
    pending_revision: int = 0
    anchor: int = 0
    head: int = 0
    undo: int = 0
    dirty: int = 0
    outcome: int = 0
    feedback: int = 0
    armed: int = 0
    closed: int = 0

    def encode(self):
        require(type(self.text) is bytes and len(self.text) <= 31
                and all(32 <= b <= 126 for b in self.text), 'host text profile')
        require(all(uint(v) for v in (self.sequence, self.revision, self.version,
                                     self.pending_request, self.pending_revision))
                and self.sequence > 0 and self.revision > 0 and self.version > 0, 'host counters')
        require(all(type(v) is int for v in (self.anchor, self.head, self.undo, self.dirty,
                                            self.outcome, self.feedback, self.armed, self.closed)), 'host field types')
        require(0 <= self.anchor <= len(self.text) and 0 <= self.head <= len(self.text)
                and self.undo in (0, 1) and self.dirty in (0, 1) and self.outcome in (0, 1, 2)
                and self.feedback in (*range(14), *range(16, 22))
                and self.armed in (0, 1) and self.closed in (0, 1) and not (self.armed and self.closed),
                'host semantic fields')
        require((self.outcome == 0 and self.pending_request == self.pending_revision == 0)
                or (self.outcome != 0 and self.pending_request > 0
                    and 0 < self.pending_revision <= self.revision), 'host pending binding')
        require(self.outcome != 2 or self.dirty == 1, 'unknown outcome cannot be clean')
        result = bytearray(128)
        result[:8] = b'ANUI\x01\x00\x80\x00'
        struct.pack_into('<5Q', result, 8, self.sequence, self.revision, self.version,
                         self.pending_request, self.pending_revision)
        result[48:57] = bytes([len(self.text), self.anchor, self.head, self.undo,
                              self.dirty, self.outcome, self.feedback, self.armed, self.closed])
        result[64:64+len(self.text)] = self.text
        return bytes(result)

    def next(self, **changes):
        return dataclasses.replace(self, sequence=self.sequence + 1, armed=0, **changes)


def wire(kind, generation, request, base=0, version=0, revision=0, text=b'', document=1):
    require(all(uint(v) for v in (kind, document, generation, request, base, version, revision))
            and 1 <= kind <= 8 and document > 0 and request > 0, 'host document integers')
    require(type(text) is bytes and len(text) <= 31 and all(32 <= b <= 126 for b in text),
            'host document text')
    require((kind == 1 and generation == base == version == revision == 0 and not text)
            or (kind in (3, 7, 8) and generation > 0 and base == version == revision == 0 and not text)
            or (kind in (2, 4) and generation > 0 and version > 0 and base == revision == 0)
            or (kind == 5 and generation > 0 and base > 0 and version == 0 and revision > 0)
            or (kind == 6 and generation > 0 and base > 0 and version == base+1 and revision > 0),
            'host document operation fields')
    data = bytearray(128)
    data[:8] = b'ANDC\x01\x00\x80\x00'
    data[8] = kind
    struct.pack_into('<6Q', data, 16, document, generation, request, base, version, revision)
    data[64] = len(text)
    data[65:65+len(text)] = text
    return bytes(data)


def mount_ready(epoch, version):
    require(uint(epoch) and uint(version) and epoch > 0 and version > 0
            and epoch+version <= u64_max, 'host mount identity/version')
    data = bytearray(128)
    data[:8] = b'ANDM\x01\x00\x80\x00'
    struct.pack_into('<3Q', data, 8, epoch, version, epoch+version)
    return bytes(data)


def fixture(phase, mode):
    require(type(phase) is int and type(mode) is int and phase in (0, 1) and mode in (0, 1, 2), 'invalid fixture')
    data = bytearray(64)
    data[:8] = b'ANDF\x01\x00\x40\x00'
    struct.pack_into('<2Q', data, 8, phase, mode)
    return data


def blank_disk():
    data = bytearray(((i*37 ^ (i >> 9) ^ 0xa7) & 255) for i in range(1024*1024))
    data[4096:6656] = bytes(2560)
    return data


def crc32c(data):
    value = 0xffffffff
    for byte in data:
        value ^= byte
        for _ in range(8):
            value = (value >> 1) ^ (0x82f63b78 if value & 1 else 0)
    return value ^ 0xffffffff


def inspect_media(path, expected_epoch, expected_version, expected_text, expected_previous):
    data = bounded_read(path, 1024*1024)
    blank = blank_disk()
    require(len(data) == len(blank) and data[:4096] == blank[:4096]
            and data[6656:] == blank[6656:], 'disk extent guard modified')
    sectors = [data[4096+i*512:4608+i*512] for i in range(5)]
    for i, sector in enumerate(sectors):
        magic = b'ANDURHDR' if i == 0 else b'ANDURBOD' if i % 2 else b'ANDURCMT'
        require(sector[:8] == magic and struct.unpack_from('<HH', sector, 8) == (1, 512), 'sector header')
        require(struct.unpack_from('<2Q', sector, 16) == (0xa110, 0xd001), 'disk identity')
        require(struct.unpack_from('<I', sector, 508)[0] == crc32c(sector[:508]), 'sector CRC')
    require(struct.unpack_from('<I', sectors[0], 12)[0] == 5 and not any(sectors[0][32:508]), 'header geometry')
    generations = []
    for slot in range(2):
        body, commit = sectors[1+2*slot:3+2*slot]
        require(not any(body[12:16]) and not any(commit[12:16]), 'reserved common bytes')
        index, sequence, epoch, version = struct.unpack_from('<4Q', body, 32)
        require(index == slot and sequence == epoch+version and version > 0
                and sequence > 0 and slot == (sequence-1) % 2, 'slot generation')
        require(body[32:64] == commit[32:64]
                and commit[64:68] == body[508:512] and not any(commit[68:508]), 'commit binding')
        length = body[64]
        require(length <= 31 and not any(body[65+length:508]), 'body bounds')
        text = body[65:65+length]
        require(all(32 <= b <= 126 for b in text), 'body text')
        generations.append((sequence, epoch, version, text))
    generations.sort()
    old, new = generations
    require(new[0] == old[0]+1, 'nonadjacent committed generations')
    require((new[1] == old[1]+1 and new[2:] == old[2:])
            or (new[1] == old[1] and new[2] == old[2]+1), 'illegal committed transition')
    require(new[1:] == (expected_epoch, expected_version, expected_text), f'recovered media differs: {new}')
    require(old[1:] == expected_previous, f'preceding committed snapshot differs: {old}')
    return {'sha256': digest(path), 'epoch': new[1], 'version': new[2], 'text_hex': new[3].hex()}


class vm:
    def __init__(self, root, image, disk, directory, phase, mode, memory):
        self.root, self.directory = root, directory
        directory.mkdir()
        self.phase, self.mode = phase, mode
        self.serial = directory/'serial.jsonl'
        self.frames = []
        self.packets = [[], [], [], []]
        self.outside = None
        self.inputs = 0
        self.monitor = None
        self.process = None
        self.stderr = (directory/'qemu.stderr').open('wb')
        (directory/'fixture.bin').write_bytes(fixture(phase, mode))
        shutil.copyfile(image, directory/'boot.img')
        # /tmp keeps the monitor socket below sockaddr_un's pathname limit.
        self.socket_directory = tempfile.TemporaryDirectory(prefix='anomalous-interactive-')
        monitor_path = Path(self.socket_directory.name)/'monitor'
        command = ['qemu-system-x86_64', '-machine', 'q35', '-cpu', 'Nehalem', '-m', f'{memory}M',
                   '-drive', f'format=raw,file={directory / "boot.img"}',
                   '-device', 'isa-ide,id=proof_ide,iobase=0x170,iobase2=0x376,irq=15',
                   '-drive', f'if=none,id=proof_data,file={disk},format=raw,cache=writeback,cache.no-flush=off,werror=report,rerror=report',
                   '-device', 'ide-hd,drive=proof_data,bus=proof_ide.0,unit=0,logical_block_size=512,physical_block_size=512',
                   '-fw_cfg', f'name=opt/anomalous/interactive,file={directory / "fixture.bin"}',
                   '-no-reboot', '-display', 'none', '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
                   '-serial', f'file:{self.serial}', '-monitor', f'unix:{monitor_path},server=on,wait=off']
        (directory/'command.json').write_text(json.dumps(command, indent=2)+'\n')
        try:
            self.process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=self.stderr)
            self.wait(lambda: monitor_path.exists(), 'monitor socket')
            self.monitor = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.monitor.settimeout(10)
            self.monitor.connect(str(monitor_path))
            self.prompt()
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.monitor:
            self.monitor.close()
        if self.process and self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.stderr.close()
        self.socket_directory.cleanup()

    def prompt(self, allow_eof=False):
        data = bytearray()
        while not data.endswith(b'(qemu) '):
            part = self.monitor.recv(4096)
            if not part and allow_eof:
                return bytes(data)  # Provisional only; finish requires exact frame/exit33.
            require(part, 'monitor disconnected')
            data.extend(part)
            require(len(data) < diagnostic_limit, 'monitor output overflow')
        return bytes(data)

    def command(self, command, allow_eof=False):
        self.monitor.sendall(command.encode()+b'\n')
        response = self.prompt(allow_eof)
        (self.directory/'monitor.log').open('ab').write(response)
        require(b'Error' not in response and b'unknown command' not in response, 'monitor command failed')

    def wait(self, predicate, label, timeout=25):
        deadline = time.monotonic()+timeout
        while not predicate():
            require((self.directory/'qemu.stderr').stat().st_size <= diagnostic_limit, 'stderr overflow')
            transcript(self.serial)
            if self.process.poll() is not None:
                # Exit can race the first predicate check; reread the evidence
                # before declaring failure (including the expected-exit wait).
                require(predicate(), f'guest exited waiting for {label}: {self.process.returncode}')
                return
            require(time.monotonic() < deadline, f'timeout waiting for {label}')
            time.sleep(.02)

    def events(self, suffix):
        return [r for r in transcript(self.serial) if r['event'] == event_prefix+suffix]

    def packet(self, channel, data):
        self.packets[channel].append({'channel': channel, 'sequence': len(self.packets[channel])+1,
                                      'length': len(data), 'hash': fnv(data)})

    def observe(self, state, audit=True, capture=True):
        data = state.encode()
        (self.directory/f'expected-{state.sequence:03}.anui').write_bytes(data)
        rgb = subprocess.run([str(self.root/'target/debug/xtask'), 'native-interactive-render'],
                             input=data, capture_output=True, check=True, timeout=10).stdout
        require(len(rgb) == 128*48*3, 'host raster length')
        enlarged = bytearray()
        for y in range(48):
            row = b''.join(rgb[(y*128+x)*3:(y*128+x+1)*3]*3 for x in range(128))
            enlarged.extend(row*3)
        self.frames.append({'sequence': len(self.frames)+1, 'hash': fnv(enlarged)})
        if audit:
            self.packet(0, data)
        self.wait(lambda: len(self.events('frame')) >= len(self.frames), 'frame')
        actual = self.events('frame')[len(self.frames)-1]
        require({k: actual.get(k) for k in ('sequence', 'hash')} == self.frames[-1],
                f'frame {state.sequence} mismatch: {actual}; expected {self.frames[-1]}')
        if audit:
            # A frame commit precedes audit submission. Wait for the exact audit
            # record too, so the host cannot type the newer edit before the
            # intended pending observation has entered its real copied route.
            self.wait(lambda: len([r for r in self.events('packet') if r['channel'] == 0])
                      >= len(self.packets[0]), 'matching UI audit')
            actual = [r for r in self.events('packet') if r['channel'] == 0][len(self.packets[0])-1]
            require({k: actual[k] for k in event_fields['packet']} == self.packets[0][-1],
                    'UI audit differs from literal observation')
        if capture:
            self.capture(state.sequence, enlarged)

    def capture(self, sequence, expected):
        path = self.directory/f'frame-{sequence:03}.ppm'
        self.command(f'screendump {path}')
        raw = bounded_read(path, 1024*768*3+64)
        header = b'P6\n1024 768\n255\n'
        require(raw.startswith(header) and len(raw) == len(header)+1024*768*3, 'capture geometry')
        pixels = bytearray(raw[len(header):])
        patch = bytearray()
        for y in range(32, 176):
            start = (y*1024+32)*3
            patch.extend(pixels[start:start+1152])
            pixels[start:start+1152] = bytes(1152)
        require(patch == expected, f'capture {sequence} differs from complete expected frame')
        outside = hashlib.sha256(pixels).hexdigest()
        if self.outside is None:
            self.outside = outside
        require(outside == self.outside, 'display changed outside granted rectangle')

    def key(self, key, terminal=False):
        self.command(f'sendkey {key} 10', allow_eof=terminal)
        # Release must precede a subsequent chord; both input records belong to
        # this command even when a press has already published its frame.
        time.sleep(.04)
        self.inputs += 2*len(key.split('-'))

    def finish(self, last_state, audit=True):
        if audit and last_state.outcome == 0:
            # Application correlation is independent of the route sequence.
            # This client deliberately chooses its next transport number for close.
            request = len(self.packets[2])+1
            self.packet(2, wire(7, 1, request, document=self.phase+1))
            self.packet(3, wire(8, 1, request, document=self.phase+1))
        self.key('ctrl-q', terminal=True)
        closed = last_state.next(feedback=13, closed=1)
        self.observe(closed, audit=audit, capture=False)
        self.wait(lambda: self.process.poll() is not None, 'clean guest exit')
        require(self.process.returncode == 33, f'bad debug exit {self.process.returncode}')
        require((self.directory/'qemu.stderr').stat().st_size <= diagnostic_limit, 'stderr overflow')
        records = transcript(self.serial, complete=True)
        result = validate_complete(records, self.phase, self.mode, self.frames, self.packets, self.inputs)
        (self.directory/'verified.json').write_text(json.dumps(result, indent=2)+'\n')
        return result


def run_case(root, image, base, memory, mode, text):
    name = f'{memory}m-mode{mode}'
    disk = base/f'{name}.raw'
    disk.write_bytes(blank_disk())
    machine = vm(root, image, disk, base/(name+'-initial'), 0, mode, memory)
    try:
        machine.packet(3, mount_ready(1, 1))
        machine.packet(2, wire(1, 0, 1))
        machine.packet(3, wire(2, 1, 1, version=1))
        state = observation()
        machine.observe(state)
        # Host entropy is delivered only through actual PS/2 input after boot.
        for byte in text:
            machine.key(chr(byte))
            state = state.next(text=state.text+bytes([byte]), revision=state.revision+1,
                               anchor=state.head+1, head=state.head+1, undo=1, dirty=1,
                               feedback=1 if state.undo == 0 else 2)
            machine.observe(state)
        saved_text, saved_revision = state.text, state.revision
        machine.key('ctrl-s')
        state = state.next(pending_request=1, pending_revision=state.revision, outcome=1, feedback=8)
        machine.packet(2, wire(5, 1, 1, base=1, revision=saved_revision, text=saved_text))
        machine.observe(state, capture=mode == 1)
        if mode == 1:
            machine.key('x')
            state = state.next(text=state.text+b'x', revision=state.revision+1,
                               anchor=state.head+1, head=state.head+1, feedback=2)
            machine.observe(state, capture=False)
            machine.packet(3, wire(6, 1, 1, base=1, version=2, revision=saved_revision, text=saved_text))
            state = state.next(version=2, pending_request=0, pending_revision=0, outcome=0, feedback=9)
            machine.observe(state)
            machine.key('ctrl-s')
            state = state.next(pending_request=2, pending_revision=state.revision, outcome=1, feedback=8)
            machine.packet(2, wire(5, 1, 2, base=2, revision=state.revision, text=state.text))
            machine.observe(state, capture=False)
            machine.packet(3, wire(6, 1, 2, base=2, version=3, revision=state.revision, text=state.text))
            state = state.next(version=3, pending_request=0, pending_revision=0, outcome=0, dirty=0, feedback=9)
            machine.observe(state)
            committed_text, committed_version = state.text, 3
            # Client close handshake request is separately specified below.
            machine.finish(state)
        else:
            state = state.next(outcome=2, feedback=11)
            machine.observe(state, audit=False)
            machine.key('x')
            state = state.next(text=state.text+b'x', revision=state.revision+1,
                               anchor=state.head+1, head=state.head+1, feedback=2)
            machine.observe(state, audit=False)
            machine.key('ctrl-q')
            state = dataclasses.replace(state.next(feedback=12), armed=1)
            machine.observe(state, audit=False)
            machine.finish(state, audit=False)
            committed_text, committed_version = saved_text, 2
    finally:
        machine.close()
    predecessor = (1, 2, saved_text) if mode == 1 else (1, 1, b'')
    first = inspect_media(disk, 1, committed_version, committed_text, predecessor)
    shutil.copyfile(disk, base/f'{name}-pre-cold.raw')
    cold = vm(root, image, disk, base/(name+'-cold'), 1, 0, memory)
    try:
        cold.packet(3, mount_ready(2, committed_version))
        cold.packet(2, wire(1, 0, 1, document=2))
        cold.packet(3, wire(2, 1, 1, version=committed_version, text=committed_text, document=2))
        state = observation(text=committed_text, version=committed_version)
        cold.observe(state)
        cold.finish(state)
    finally:
        cold.close()
    second = inspect_media(disk, 2, committed_version, committed_text,
                           (1, committed_version, committed_text))
    return {'memory_mib': memory, 'mode': mode, 'initial': first, 'cold': second}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', type=Path, required=True)
    parser.add_argument('--memory', type=int, choices=(96, 320), action='append')
    parser.add_argument('--mode', type=int, choices=(1, 2), action='append')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    image = args.image.resolve()
    image_hash = digest(image)
    base = Path(tempfile.mkdtemp(prefix='native-interactive-proof-', dir=root/'build'))
    print(f'Interactive proof evidence: {base}', flush=True)
    text = ('n'+secrets.token_hex(3)).encode()
    (base/'challenge.txt').write_bytes(text)
    results = []
    try:
        for memory in args.memory or (96, 320):
            for mode in args.mode or (1, 2):
                results.append(run_case(root, image, base, memory, mode, text))
        require(digest(image) == image_hash, 'inspected image changed')
        for copy in base.glob('*/boot.img'):
            require(digest(copy) == image_hash, 'boot image copy changed')
        (base/'receipt.json').write_text(json.dumps(dict(image_sha256=image_hash, cases=results,
             cold_answer_supplied=False, text_delivered_by='physical PS/2', extent_guard_unchanged=True), indent=2)+'\n')
    except BaseException as error:
        (base/'failure.txt').write_text(f'{type(error).__name__}: {error}\n')
        raise
    print(f'Interactive editor physical saves and cold recovery passed: {base}', flush=True)


if __name__ == '__main__':
    main()
