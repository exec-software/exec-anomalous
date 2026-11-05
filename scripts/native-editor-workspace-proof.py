#!/usr/bin/env python3
"""Literal copied-packet, physical-input, composed-pixel and cold-media oracle.

The older pair's wire/media helpers are independently specified host functions.
Its lifecycle verifier and VM policy are deliberately not reused: this profile
has three owners, four live directional routes and complete desktop frames.
"""
import argparse
import dataclasses
import importlib.util
import json
from pathlib import Path
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time

spec = importlib.util.spec_from_file_location('native_pair_oracle', Path(__file__).with_name('native-interactive-proof.py'))
base = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = base
spec.loader.exec_module(base)
require, fnv, uint, fields = base.require, base.fnv, base.uint, base.fields
observation, wire, mount_ready = base.observation, base.wire, base.mount_ready
prefix = 'kernel.native_editor_workspace_test.'
startup_events = (*base.boot_events,
    ('workspace_pointer.ready', dict(available=True, generation=1)))
ready_index = len(startup_events)
disk_identity = (0x414e4f4d414c4f55, 0x52454d4f54450001)
limit = 1024 * 1024
pairs = ((0, 1), (1, 0), (1, 2), (2, 1))
schemas = {
    'ready': set('tasks phase proof_mode descriptor_bytes owned_frames owned_leases stage_frames connection free_before free_live'.split()),
    'started': {'role', 'cpu_clean'},
    'packet': set('channel sequence producer consumer length hash'.split()),
    'frame': set('sequence hash width height rows'.split()),
    'retired': set('role faulted closed_routes discarded survivors'.split()),
    'channel': set('channel sends deliveries acks'.split()),
    'passed': set('phase proof_mode starts calls switches timer_ticks inputs input_losses frames rows denials bad_copies invalid closed discarded exits faults resources_restored authority_revoked cpu_restored device_poisoned free_frames_before free_frames_after zeroed_bytes cpu_captures cpu_clean_checks user_irqs'.split()),
}
booleans = {'resources_restored', 'authority_revoked', 'cpu_restored', 'device_poisoned', 'faulted', 'cpu_clean'}


def action(action_id, expected, opcode, first=0, second=0):
    require(all(uint(v) for v in (action_id, expected, opcode, first, second)) and action_id > 0 and expected > 0, 'action correlation')
    require(1 <= opcode <= 12, 'action opcode')
    require((opcode == 1 and first <= 255 and second == 0)
            or (opcode == 4 and first <= 3 and second <= 1)
            or opcode == 5 or (opcode not in (1, 4, 5) and first == second == 0), 'action operands')
    data = bytearray(64)
    data[:8] = b'ANEA\x01\x00\x40\x00'
    struct.pack_into('<3Q', data, 8, 1, action_id, expected)
    data[32] = opcode
    struct.pack_into('<2Q', data, 40, first, second)
    return bytes(data)


def message(state, action_id=0, expected=0, outcome=0, error=0):
    require(all(uint(v) for v in (action_id, expected, outcome, error)), 'message integers')
    require((action_id == expected == outcome == error == 0)
            or (action_id > 0 and expected > 0 and 1 <= outcome <= 6), 'message correlation')
    require((outcome != 6 and error == 0) or (outcome == 6 and 1 <= error <= 5), 'message error tag')
    data = bytearray(192)
    data[:8] = b'ANER\x01\x00\xc0\x00'
    data[8:11] = bytes([2 if action_id else 1, outcome, error])
    struct.pack_into('<3Q', data, 16, 1, action_id, expected)
    data[64:] = state.encode()
    return bytes(data)


def held_release(document, revision, later, request=1, version=1):
    require(all(uint(v) and v > 0 for v in (document, revision, later, request, version)) and later > revision, 'held save correlation')
    data = bytearray(128)
    data[:8] = b'ANDH\x01\x00\x80\x00'
    struct.pack_into('<6Q', data, 8, document, 1, request, version, revision, later)
    return bytes(data)


def inspect_media(path, expected_epoch, expected_version, expected_text, expected_previous):
    """Independent raw-sector parser with this profile's separately fixed identity."""
    data = base.bounded_read(path, 1024*1024)
    blank = base.blank_disk()
    require(len(data) == len(blank) and data[:4096] == blank[:4096]
            and data[6656:] == blank[6656:], 'disk extent guard modified')
    sectors = [data[4096+i*512:4608+i*512] for i in range(5)]
    for index, sector in enumerate(sectors):
        magic = b'ANDURHDR' if index == 0 else b'ANDURBOD' if index % 2 else b'ANDURCMT'
        require(sector[:8] == magic and struct.unpack_from('<HH', sector, 8) == (1, 512), 'sector header')
        require(struct.unpack_from('<2Q', sector, 16) == disk_identity, 'disk identity')
        require(struct.unpack_from('<I', sector, 508)[0] == base.crc32c(sector[:508]), 'sector CRC')
    require(struct.unpack_from('<I', sectors[0], 12)[0] == 5 and not any(sectors[0][32:508]), 'header geometry')
    generations = []
    for slot in range(2):
        body, commit = sectors[1+2*slot:3+2*slot]
        require(not any(body[12:16]) and not any(commit[12:16]), 'reserved common bytes')
        index, sequence, epoch, version = struct.unpack_from('<4Q', body, 32)
        require(index == slot and sequence == epoch+version and version > 0
                and sequence > 0 and slot == (sequence-1) % 2, 'slot generation')
        require(body[32:64] == commit[32:64] and commit[64:68] == body[508:512]
                and not any(commit[68:508]), 'commit binding')
        length = body[64]
        require(length <= 31 and not any(body[65+length:508]), 'body bounds')
        text = body[65:65+length]
        require(all(32 <= byte <= 126 for byte in text), 'body text')
        generations.append((sequence, epoch, version, text))
    old, new = sorted(generations)
    require(new[0] == old[0]+1, 'nonadjacent committed generations')
    require((new[1] == old[1]+1 and new[2:] == old[2:])
            or (new[1] == old[1] and new[2] == old[2]+1), 'illegal committed transition')
    require(new[1:] == (expected_epoch, expected_version, expected_text), 'recovered media differs')
    require(old[1:] == expected_previous, 'preceding committed snapshot differs')
    return dict(sha256=base.digest(path), epoch=new[1], version=new[2], text_hex=new[3].hex())


def validate_prefix(records):
    for index, record in enumerate(records):
        if index < ready_index:
            event, expected = startup_events[index]
            fields(record, event, expected)
            continue
        require(type(record) is dict and type(record.get('event')) is str and record['event'].startswith(prefix), 'unexpected native workspace event')
        suffix = record['event'][len(prefix):]
        require(suffix in schemas, 'unknown native workspace event')
        expected = {key: record.get(key) if key in booleans else int for key in schemas[suffix]}
        fields(record, prefix + suffix, expected)
        require(all(type(record[key]) is bool for key in schemas[suffix] & booleans), 'typed boolean evidence')
        if suffix == 'packet':
            channel = record['channel']
            require(channel < 4 and (record['producer'], record['consumer']) == pairs[channel], 'wrong packet route owners')
            require(record['length'] == (64 if channel == 0 else 192 if channel == 1 else 128)
                    and record['sequence'] > 0, 'wrong packet geometry')
        elif suffix == 'frame':
            require((record['width'], record['height'], record['rows']) == (1024, 768, 768)
                    and record['sequence'] > 0, 'incomplete desktop frame')
        elif suffix == 'retired':
            require(record['role'] < 3 and not record['faulted'] and record['survivors'] < 3, 'unexpected role fault/retirement')
        elif suffix == 'started':
            require(record['role'] < 3 and record['cpu_clean'], 'initial CPU ownership')
    if len(records) > ready_index:
        require(records[ready_index]['event'] == prefix + 'ready', 'exact initial ready position')
    for index, record in enumerate(records[ready_index+1:], ready_index+1):
        require(record['event'] != prefix + 'ready', 'duplicate ready')
        if record['event'] == prefix + 'passed':
            require(index == len(records)-1, 'event after terminal completion')
        if record['event'] == prefix + 'channel':
            require(all(r['event'] in (prefix+'channel', prefix+'passed') for r in records[index:]), 'activity after channel teardown')
    if len(records) >= 5:
        memory, allocator = records[3:5]
        require(allocator['free_frames'] * 4096 == memory['usable_bytes']
                and allocator['free_frames'] + allocator['unavailable_frames'] == 1 << 20,
                'boot memory conservation')


def transcript(path, complete=False):
    if not path.exists():
        return []
    data = base.bounded_read(path, limit)
    if complete:
        require(data.endswith(b'\n'), 'incomplete final serial line')
    lines = data.rsplit(b'\n', 1)[0].splitlines() if b'\n' in data else []
    records = [json.loads(line, object_pairs_hook=base.unique_fields,
                         parse_constant=lambda _: require(False, 'nonfinite JSON')) for line in lines]
    validate_prefix(records)
    return records


def validate_complete(records, phase, mode, frames, packets):
    validate_prefix(records)
    require(len(records) >= ready_index+9, 'incomplete lifecycle')
    fields(records[ready_index], prefix+'ready', dict(tasks=3, phase=phase, proof_mode=mode,
        descriptor_bytes=192, owned_frames=144, owned_leases=18, stage_frames=576, connection=1,
        free_before=records[4]['free_frames'], free_live=records[4]['free_frames']-720))
    started = [r['role'] for r in records if r['event'] == prefix+'started']
    require(len(started) == 3 and sorted(started) == [0, 1, 2], 'three exact clean starts')
    actual_frames = [{k: r[k] for k in schemas['frame']} for r in records if r['event'] == prefix+'frame']
    require(actual_frames == frames and [r['sequence'] for r in frames] == list(range(1, len(frames)+1)), 'exact full-frame trace')
    require([r['event'] for r in records[-5:]] == [prefix+'channel']*4+[prefix+'passed'], 'exact teardown tail')
    for channel in range(4):
        actual = [{k: r[k] for k in schemas['packet']} for r in records if r['event'] == prefix+'packet' and r['channel'] == channel]
        require(actual == packets[channel], f'channel {channel} literal packet trace')
        require([p['sequence'] for p in actual] == list(range(1, len(actual)+1)), 'transport sequence gap')
        fields(records[-5+channel], prefix+'channel', dict(channel=channel, sends=len(actual), deliveries=len(actual), acks=len(actual)))
    retired = [r for r in records if r['event'] == prefix+'retired']
    require(len(retired) == 3 and sorted(r['role'] for r in retired) == [0, 1, 2], 'all owners retired once')
    require([r['survivors'] for r in retired] == [2, 1, 0], 'retirement cardinality')
    require(all(r['discarded'] == 0 for r in retired), 'unexpected discarded copied packet')
    if mode == 2:
        require(retired[0]['role'] == 2 and retired[0]['closed_routes'] == 2, 'service loss must spare workspace/editor routes')
        cut = records.index(retired[0])
        require(any(r['event'] == prefix+'packet' and r['channel'] == 1 for r in records[cut+1:]), 'no observation after service loss')
        require(any(r['event'] == prefix+'frame' for r in records[cut+1:]), 'no presentation after service loss')
    result = records[-1]
    for key, value in dict(phase=phase, proof_mode=mode, starts=3, input_losses=0,
            frames=len(frames), rows=768*len(frames), exits=3, faults=0, discarded=0,
            denials=11, bad_copies=0, invalid=0,
            zeroed_bytes=102400, resources_restored=True, authority_revoked=True,
            cpu_restored=True, device_poisoned=False).items():
        require(type(result[key]) is type(value) and result[key] == value, f'completion {key}')
    require(result['free_frames_before'] == result['free_frames_after'] == records[4]['free_frames'], 'exact allocator restoration')
    require(result['cpu_captures'] == result['cpu_clean_checks'] == result['calls'] + result['user_irqs'] + result['faults'], 'CPU capture/clean relation')
    require(result['switches'] > 0 and result['timer_ticks'] > 0 and result['inputs'] > 0, 'no scheduling/input evidence')
    return result


def independent_pixels(rgb, state):
    """Literal 5x7 A and n with independently fixed native viewport geometry."""
    require(len(rgb) == 1024*768*3, 'pixel oracle frame length')
    if state is None or state.closed:
        return
    def sample(sx, sy):
        x = 216 + (sx*592 + 127)//128
        y = 216 + (sy*360 + 47)//48
        offset = (y*1024+x)*3
        return rgb[offset:offset+3]
    for row, bits in enumerate((14, 17, 17, 31, 17, 17, 17)):
        for col in range(5):
            expected = bytes((144, 205, 255) if bits & (1 << (4-col)) else (18, 24, 34))
            require(sample(col, row) == expected, 'independent A glyph/viewport mismatch')
    if state.text.startswith(b'n') and state.head > 0 and state.head <= 20:
        for row, bits in enumerate((0, 0, 22, 25, 17, 17, 17)):
            for col in range(5):
                expected = bytes((236, 241, 247) if bits & (1 << (4-col)) else (18, 24, 34))
                require(sample(col, row+8) == expected, 'independent typed n glyph mismatch')


class desktop_history:
    """Host expectations only; each entry is one desktop publication after demo."""
    def __init__(self):
        self.entries = []

    def observation(self, state):
        if state is None:
            require(not self.entries, 'demo must be the first expected desktop')
        else:
            self.entries.append(b'anui '+state.encode().hex().encode()+b'\n')

    def pointer(self, x, y):
        require(uint(x) and uint(y) and x < 1024 and y < 768, 'expected pointer coordinate')
        self.entries.append(f'pointer {x} {y}\n'.encode())

    def encode(self):
        require(len(self.entries) <= 256, 'expected desktop history too long')
        return b''.join(self.entries)


class vm(base.vm):
    def __init__(self, root, image, disk, directory, phase, mode, memory, backend):
        self.root, self.directory = root, directory
        directory.mkdir()
        self.phase, self.mode = phase, mode
        self.serial = directory/'serial.jsonl'
        self.frames, self.packets = [], [[], [], [], []]
        self.inputs, self.action_id = 0, 0
        self.pointer = None
        self.history = desktop_history()
        self.monitor = self.process = None
        self.stderr = (directory/'qemu.stderr').open('wb')
        (directory/'fixture.bin').write_bytes(base.fixture(phase, mode))
        shutil.copyfile(image, directory/'boot.img')
        self.socket_directory = tempfile.TemporaryDirectory(prefix='anomalous-editor-workspace-')
        monitor_path = Path(self.socket_directory.name)/'monitor'
        command = ['qemu-system-x86_64', '-machine', 'q35', '-accel', backend,
            '-cpu', 'Nehalem' if backend == 'tcg' else 'host', '-m', f'{memory}M',
            '-drive', f'format=raw,file={directory/"boot.img"}',
            '-device', 'isa-ide,id=proof_ide,iobase=0x170,iobase2=0x376,irq=15',
            '-drive', f'if=none,id=proof_data,file={disk},format=raw,cache=writeback,cache.no-flush=off,werror=report,rerror=report',
            '-device', 'ide-hd,drive=proof_data,bus=proof_ide.0,unit=0,logical_block_size=512,physical_block_size=512',
            '-fw_cfg', f'name=opt/anomalous/interactive,file={directory/"fixture.bin"}',
            '-no-reboot', '-display', 'none', '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
            '-serial', f'file:{self.serial}', '-monitor', f'unix:{monitor_path},server=on,wait=off']
        (directory/'command.json').write_text(json.dumps(command, indent=2)+'\n')
        try:
            self.process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=self.stderr)
            self.wait(lambda: monitor_path.exists(), 'monitor')
            self.monitor = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.monitor.settimeout(10)
            self.monitor.connect(str(monitor_path))
            self.prompt()
        except BaseException:
            self.close()
            raise

    def wait(self, predicate, label, timeout=45):
        deadline = time.monotonic()+timeout
        while not predicate():
            require((self.directory/'qemu.stderr').stat().st_size <= limit, 'stderr overflow')
            transcript(self.serial)
            if self.process.poll() is not None:
                require(predicate(), f'guest exited awaiting {label}: {self.process.returncode}')
                return
            require(time.monotonic() < deadline, f'timeout awaiting {label}')
            time.sleep(.02)

    def events(self, suffix):
        return [r for r in transcript(self.serial) if r['event'] == prefix+suffix]

    def packet(self, channel, data):
        producer, consumer = pairs[channel]
        self.packets[channel].append(dict(channel=channel, sequence=len(self.packets[channel])+1,
            producer=producer, consumer=consumer, length=len(data), hash=fnv(data)))

    def observe(self, state, action_id=0, expected=0, outcome=0, capture=True, packet=True):
        if state is not None and packet:
            self.packet(1, message(state, action_id, expected, outcome))
        if packet:
            self.history.observation(state)
        data = state.encode() if state is not None else b''
        command = [str(self.root/'target/debug/xtask'), 'native-editor-workspace-render']
        history = self.history.encode()
        rendered = subprocess.run(command, input=history, capture_output=True, check=True, timeout=15)
        rgb = rendered.stdout
        require(len(rgb) == 1024*768*3, 'full desktop raster length')
        frame = dict(sequence=len(self.frames)+1, hash=fnv(rgb), width=1024, height=768, rows=768)
        self.frames.append(frame)
        (self.directory/f'expected-{frame["sequence"]:03}.anui').write_bytes(data)
        (self.directory/f'expected-{frame["sequence"]:03}.history').write_bytes(history)
        self.wait(lambda: len(self.events('frame')) >= len(self.frames), 'complete desktop frame')
        actual = self.events('frame')[len(self.frames)-1]
        require({k: actual[k] for k in schemas['frame']} == frame, f'frame {frame["sequence"]} mismatch: {actual}, expected {frame}')
        if state is not None and packet:
            self.wait(lambda: len([r for r in self.events('packet') if r['channel'] == 1]) >= len(self.packets[1]), 'matching ANER')
            actual = [r for r in self.events('packet') if r['channel'] == 1][len(self.packets[1])-1]
            require({k: actual[k] for k in schemas['packet']} == self.packets[1][-1], 'ANER differs from literal semantic state')
        if capture:
            path = self.directory/f'frame-{frame["sequence"]:03}.ppm'
            self.command(f'screendump {path}')
            raw = base.bounded_read(path, 1024*768*3+64)
            require(raw == b'P6\n1024 768\n255\n'+rgb, 'captured composed desktop differs')
            independent_pixels(raw[len(b'P6\n1024 768\n255\n'):], state)

    def input_action(self, key, previous, following, opcode, first=0, second=0, outcome=1, capture=True, terminal=False):
        self.action_id += 1
        self.packet(0, action(self.action_id, previous.sequence, opcode, first, second))
        self.key(key, terminal=terminal)
        self.observe(following, self.action_id, previous.sequence, outcome, capture=capture)

    def pointer_motion(self, state, dx):
        self.command(f'mouse_move {dx} 0')
        x, y = self.pointer or (512, 384)
        self.pointer = (x+dx, y)
        self.history.pointer(*self.pointer)
        self.observe(state, packet=False)

    def finish(self, state):
        self.input_action('ctrl-q', state, state.next(feedback=13, closed=1), 12,
            outcome=4, capture=False, terminal=True)
        self.wait(lambda: self.process.poll() is not None, 'complete cleanup')
        require(self.process.returncode == 33, f'debug exit {self.process.returncode}')
        require(base.bounded_read(self.directory/'qemu.stderr', limit) == b'', 'unexpected QEMU stderr')
        result = validate_complete(transcript(self.serial, True), self.phase, self.mode, self.frames, self.packets)
        (self.directory/'verified.json').write_text(json.dumps(result, indent=2)+'\n')
        return result


def run_case(root, image, output, backend, memory, mode, text):
    name = f'{backend}-{memory}m-mode{mode}'
    disk = output/f'{name}.raw'
    disk.write_bytes(base.blank_disk())
    machine = vm(root, image, disk, output/(name+'-initial'), 0, mode, memory, backend)
    try:
        machine.packet(3, mount_ready(1, 1))
        machine.packet(2, wire(1, 0, 1))
        machine.packet(3, wire(2, 1, 1, version=1))
        # The demo has no input barrier before native open. Its exact complete
        # readback hash remains required; capture the stable native window next.
        machine.observe(None, capture=False)
        state = observation()
        machine.observe(state)
        machine.pointer_motion(state, 16)
        machine.pointer_motion(state, -16)
        for byte in text:
            following = state.next(text=state.text+bytes([byte]), revision=state.revision+1,
                anchor=state.head+1, head=state.head+1, undo=1, dirty=1,
                feedback=1 if state.undo == 0 else 2)
            machine.input_action(chr(byte), state, following, 1, byte)
            state = following
        saved_text, saved_revision = state.text, state.revision
        following = state.next(pending_request=1, pending_revision=state.revision, outcome=1, feedback=8)
        machine.packet(2, wire(5, 1, 1, base=1, revision=saved_revision, text=saved_text))
        machine.input_action('ctrl-s', state, following, 11, outcome=2, capture=mode == 1)
        state = following
        if mode == 1:
            following = state.next(text=state.text+b'x', revision=state.revision+1,
                anchor=state.head+1, head=state.head+1, feedback=2)
            machine.input_action('x', state, following, 1, ord('x'), capture=False)
            state = following
            machine.packet(2, held_release(1, saved_revision, state.revision))
            machine.packet(3, wire(6, 1, 1, base=1, version=2, revision=saved_revision, text=saved_text))
            state = state.next(version=2, pending_request=0, pending_revision=0, outcome=0, feedback=9)
            machine.observe(state)
            following = state.next(pending_request=2, pending_revision=state.revision, outcome=1, feedback=8)
            machine.packet(2, wire(5, 1, 2, base=2, revision=state.revision, text=state.text))
            machine.input_action('ctrl-s', state, following, 11, outcome=2, capture=False)
            state = following
            machine.packet(3, wire(6, 1, 2, base=2, version=3, revision=state.revision, text=state.text))
            state = state.next(version=3, pending_request=0, pending_revision=0, outcome=0, dirty=0, feedback=9)
            machine.observe(state)
            committed_text, committed_version = state.text, 3
            machine.finish(state)
        else:
            state = state.next(outcome=2, feedback=11)
            machine.observe(state)
            following = state.next(text=state.text+b'x', revision=state.revision+1,
                anchor=state.head+1, head=state.head+1, feedback=2)
            machine.input_action('x', state, following, 1, ord('x'))
            state = following
            following = dataclasses.replace(state.next(feedback=12), armed=1)
            machine.input_action('ctrl-q', state, following, 12, outcome=3)
            state = following
            machine.finish(state)
            committed_text, committed_version = saved_text, 2
    finally:
        machine.close()
    predecessor = (1, 2, saved_text) if mode == 1 else (1, 1, b'')
    initial = inspect_media(disk, 1, committed_version, committed_text, predecessor)
    shutil.copyfile(disk, output/f'{name}-pre-cold.raw')
    cold = vm(root, image, disk, output/(name+'-cold'), 1, 0, memory, backend)
    try:
        cold.packet(3, mount_ready(2, committed_version))
        cold.packet(2, wire(1, 0, 1, document=2))
        cold.packet(3, wire(2, 1, 1, version=committed_version, text=committed_text, document=2))
        # Native open may supersede this transient demo before an HMP capture.
        # Retain its exact full-frame readback hash, then capture native content.
        cold.observe(None, capture=False)
        state = observation(text=committed_text, version=committed_version)
        cold.observe(state)
        cold.finish(state)
    finally:
        cold.close()
    recovered = inspect_media(disk, 2, committed_version, committed_text, (1, committed_version, committed_text))
    return dict(backend=backend, memory_mib=memory, mode=mode, initial=initial, cold=recovered)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', type=Path, required=True)
    parser.add_argument('--backend', choices=('tcg', 'kvm'), action='append')
    parser.add_argument('--memory', type=int, choices=(96, 320), action='append')
    parser.add_argument('--mode', type=int, choices=(1, 2), action='append')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    image = args.image.resolve()
    image_hash = base.digest(image)
    output = Path(tempfile.mkdtemp(prefix='native-editor-workspace-proof-', dir=root/'build'))
    text = ('n'+secrets.token_hex(2)).encode()
    (output/'challenge.txt').write_bytes(text)
    print(f'Native editor workspace evidence: {output}', flush=True)
    try:
        results = [run_case(root, image, output, backend, memory, mode, text)
                   for backend in args.backend or ('tcg', 'kvm')
                   for memory in args.memory or (96, 320) for mode in args.mode or (1, 2)]
        require(base.digest(image) == image_hash, 'source image changed during qualification')
        require(all(base.digest(copy) == image_hash for copy in output.glob('*/boot.img')), 'boot copy changed')
        (output/'receipt.json').write_text(json.dumps(dict(image_sha256=image_hash, cases=results,
            cold_answer_supplied=False, input='physical PS/2 keyboard and pointer', complete_desktop_frames=True), indent=2)+'\n')
    except BaseException as error:
        (output/'failure.txt').write_text(f'{type(error).__name__}: {error}\n')
        raise
    print(f'Native editor workspace qualification passed: {output}', flush=True)


if __name__ == '__main__':
    main()
