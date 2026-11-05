#!/usr/bin/env python3
"""Observe one built image in an isolated VirtualBox VM; not a semantic verifier.

Every run retains its boot disk, VM configuration, serial output, screenshot and
command receipts. No existing VirtualBox registration or disk is adopted. Run
under the parent's memory-limited scope; this script does not configure the host.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import struct
import subprocess
import sys
import time
import uuid
import xml.etree.ElementTree as etree


serial_limit = 64 * 1024
command_limit = 64 * 1024
image_limit = 128 * 1024 * 1024
png_limit = 4 * 1024 * 1024
vbox_log_limit = 8 * 1024 * 1024
command_timeout = 20


class proof_error(Exception):
    pass


class terminated(BaseException):
    def __init__(self, signum):
        self.signum = signum


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def bounded_read(path, limit):
    with path.open('rb') as stream:
        value = stream.read(limit + 1)
    if len(value) > limit:
        raise proof_error(f'{path.name} exceeds {limit} bytes')
    return value


def observe_transcript(data, expected):
    """Only complete JSON lines can trigger capture; no lifecycle is qualified."""
    if len(data) > serial_limit:
        raise proof_error('serial output exceeded its bound')
    complete = data.rsplit(b'\n', 1)[0] if b'\n' in data else b''
    try:
        lines = complete.decode('utf-8').splitlines()
    except UnicodeDecodeError as error:
        raise proof_error('serial output is not UTF-8') from error
    observed = 0
    entries = 0
    for line in lines:
        if not line.startswith('{'):
            continue
        try:
            record = json.loads(line)
        except (ValueError, TypeError) as error:
            raise proof_error('malformed complete serial JSON record') from error
        if not isinstance(record, dict):
            raise proof_error('serial JSON record is not an object')
        if record.get('level') in ('error', 'fatal'):
            raise proof_error('guest reported an error/fatal record')
        entries += record.get('event') == 'boot.kernel.enter'
        if record.get('event') == expected:
            if record.get('schema') != 1 or record.get('level') != 'info':
                raise proof_error('expected event has an invalid envelope')
            observed += 1
    if entries > 1 or observed > 1:
        raise proof_error('duplicate boot or expected event')
    return observed == 1


def png_dimensions(data):
    # The independent frame verifier must decode pixels. This only bounds the
    # transport and identifies the screenshot geometry, not its semantic content.
    if (len(data) < 33 or len(data) > png_limit
            or data[:8] != b'\x89PNG\r\n\x1a\n'
            or data[8:16] != b'\x00\x00\x00\rIHDR'):
        raise proof_error('screenshot has no bounded PNG header')
    width, height = struct.unpack('>II', data[16:24])
    if (width, height) != (1024, 768):
        raise proof_error(f'unexpected screenshot dimensions {width}x{height}')
    return width, height


def process_sample(private_home, vm_uuid):
    """Inspect only this harness and matching VirtualBox processes, never all envs."""
    samples = []
    for path in Path('/proc').iterdir():
        if not path.name.isdecimal():
            continue
        try:
            comm = (path / 'comm').read_text().strip()
            own = int(path.name) == os.getpid()
            if not own and comm not in {
                    'VBoxSVC', 'VBoxHeadless', 'VirtualBoxVM', 'VBoxManage',
                    'VBoxXPCOMIPCD'}:
                continue
            cmdline = bounded_read(path / 'cmdline', 64 * 1024).split(b'\0')
            matches = vm_uuid.encode() in cmdline
            if not matches and not own:
                try:
                    environment = bounded_read(path / 'environ', 128 * 1024)
                    matches = (b'VBOX_USER_HOME=' + os.fsencode(private_home)) in environment.split(b'\0')
                except (OSError, proof_error):
                    pass
            if not own and not matches:
                continue
            status = {}
            for line in (path / 'status').read_text().splitlines():
                key, _, value = line.partition(':')
                if key in ('PPid', 'VmRSS', 'VmSize', 'VmSwap', 'Threads'):
                    status[key] = value.strip()
            samples.append({
                'pid': int(path.name), 'name': comm, 'status': status,
                'cgroup': (path / 'cgroup').read_text().splitlines(),
            })
        except (OSError, proof_error):
            continue
    return samples


class proof_run:
    def __init__(self, arguments):
        self.arguments = arguments
        self.output = arguments.output.absolute()
        # Refuse an existing path, including an existing or dangling symlink.
        self.output.mkdir(mode=0o700, parents=True, exist_ok=False)
        self.output = self.output.resolve(strict=True)
        self.private_home = self.output / 'vbox-home'
        self.private_home.mkdir(mode=0o700)
        self.machines = self.output / 'machines'
        self.machines.mkdir(mode=0o700)
        self.commands = self.output / 'commands'
        self.commands.mkdir(mode=0o700)
        self.vm_uuid = str(uuid.uuid4())
        self.vm_name = 'Anomalous Proof ' + self.vm_uuid
        # XPCOM chooses its service by this socket ID, not VBOX_USER_HOME.
        # /tmp/.vbox-<ID>-ipc/ipcd is 66 ASCII bytes, below Linux's 108-byte limit.
        self.ipc_socket_id = 'anomalous-' + self.vm_uuid
        self.config = self.machines / self.vm_name / (self.vm_name + '.vbox')
        self.serial = self.output / 'serial.log'
        self.serial.touch(mode=0o600, exist_ok=False)
        self.environment = dict(
            os.environ, VBOX_USER_HOME=str(self.private_home),
            VBOX_IPC_SOCKETID=self.ipc_socket_id,
        )
        self.command_index = 0
        self.last_sample = 0.0
        self.created = False
        self.receipt = {
            'schema': 1, 'result': 'pending', 'semantic_qualified': False,
            'expected_event': arguments.expect_event, 'event_observed': False,
            'vm_uuid': self.vm_uuid, 'vm_name': self.vm_name,
            'vbox_user_home': str(self.private_home), 'memory_mib': arguments.memory,
            'vbox_ipc_socket_id': self.ipc_socket_id,
            'timeout_seconds': arguments.timeout,
            'cleanup_powered_off': False,
        }
        self.write_receipt()

    def write_receipt(self):
        staging = self.output / 'receipt.json.part'
        staging.write_text(json.dumps(self.receipt, indent=2) + '\n')
        staging.replace(self.output / 'receipt.json')

    def sample(self, force=False):
        if force or time.monotonic() - self.last_sample >= 1:
            self.last_sample = time.monotonic()
            with (self.output / 'process-cgroups.jsonl').open('a') as stream:
                stream.write(json.dumps({
                    'monotonic_seconds': self.last_sample,
                    'processes': process_sample(self.private_home, self.vm_uuid),
                }) + '\n')

    def limits(self):
        if self.serial.stat().st_size > serial_limit:
            raise proof_error('serial output exceeded its bound')
        for path in self.machines.rglob('VBox.log*'):
            if path.is_file() and path.stat().st_size > vbox_log_limit:
                raise proof_error('VirtualBox diagnostic log exceeded its bound')

    def command(self, *arguments, cleanup=False):
        self.command_index += 1
        prefix = self.commands / f'{self.command_index:03d}'
        stdout_path = prefix.with_suffix('.stdout')
        stderr_path = prefix.with_suffix('.stderr')
        argv = ['VBoxManage', *map(str, arguments)]
        started = time.monotonic()
        child = None
        record = {'argv': argv, 'timeout_seconds': command_timeout}
        try:
            with stdout_path.open('xb') as stdout, stderr_path.open('xb') as stderr:
                child = subprocess.Popen(
                    argv, env=self.environment, stdin=subprocess.DEVNULL,
                    stdout=stdout, stderr=stderr, start_new_session=True,
                )
                while child.poll() is None:
                    self.sample()
                    if not cleanup:
                        self.limits()
                    if (stdout_path.stat().st_size > command_limit
                            or stderr_path.stat().st_size > command_limit):
                        raise proof_error('VBoxManage diagnostic output exceeded its bound')
                    if time.monotonic() - started >= command_timeout:
                        raise proof_error('VBoxManage command timed out')
                    time.sleep(0.05)
                record['exit_code'] = child.returncode
            stdout = bounded_read(stdout_path, command_limit)
            bounded_read(stderr_path, command_limit)
            if child.returncode:
                raise proof_error(f'VBoxManage {arguments[0]} failed; inspect {stderr_path}')
            return stdout.decode('utf-8', errors='strict')
        finally:
            if child is not None and child.poll() is None:
                # Only this invocation's process group; never kill by name.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                try:
                    child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    record['unreaped'] = True
            record['elapsed_seconds'] = time.monotonic() - started
            prefix.with_suffix('.json').write_text(json.dumps(record, indent=2) + '\n')

    def guard_owner(self):
        if not self.config.resolve().is_relative_to(self.machines):
            raise proof_error('VM configuration escaped this run')
        tree = etree.fromstring(bounded_read(self.config, 1024 * 1024))
        machines = [node for node in tree.iter()
                    if node.tag.rsplit('}', 1)[-1] == 'Machine']
        if (len(machines) != 1
                or machines[0].get('uuid', '').strip('{}') != self.vm_uuid
                or machines[0].get('name') != self.vm_name):
            raise proof_error('VM configuration does not match the owned UUID/name')

    def state(self, cleanup=False):
        self.guard_owner()
        output = self.command('showvminfo', self.vm_uuid, '--machinereadable', cleanup=cleanup)
        match = re.search(r'^VMState="([a-z]+)"$', output, re.MULTILINE)
        if match is None:
            raise proof_error('VirtualBox returned no machine state')
        return match[1]

    def prepare(self):
        image = self.arguments.image.resolve(strict=True)
        info = image.stat()
        if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= image_limit:
            raise proof_error('input image must be a regular file of at most 128 MiB')
        original_hash = digest(image)
        raw = self.output / 'boot.img'
        shutil.copyfile(image, raw)
        if digest(raw) != original_hash or digest(image) != original_hash:
            raise proof_error('input image changed while being copied')
        self.receipt['image_source'] = str(image)
        self.receipt['image_sha256'] = original_hash
        self.receipt['vbox_version'] = self.command('--version').strip()
        self.command('createvm', '--name', self.vm_name, '--uuid', self.vm_uuid,
                     '--basefolder', self.machines, '--ostype', 'Other_64', '--register')
        self.created = True
        self.guard_owner()
        self.command(
            'modifyvm', self.vm_uuid, '--firmware', 'bios', '--chipset', 'piix3',
            '--memory', self.arguments.memory, '--cpus', '1', '--vram', '32',
            '--graphicscontroller', 'vmsvga', '--accelerate3d', 'off',
            '--boot1', 'disk', '--boot2', 'none', '--boot3', 'none', '--boot4', 'none',
            '--nic1', 'none', '--nic2', 'none', '--nic3', 'none', '--nic4', 'none',
            '--nic5', 'none', '--nic6', 'none', '--nic7', 'none', '--nic8', 'none',
            '--usb-ohci', 'off', '--usb-ehci', 'off', '--usb-xhci', 'off',
            '--audio-enabled', 'off', '--clipboard-mode', 'disabled',
            '--drag-and-drop', 'disabled', '--vrde', 'off',
            '--uart1', '0x3F8', '4', '--uart-mode1', 'file', self.serial,
        )
        self.command('storagectl', self.vm_uuid, '--name', 'Proof SATA', '--add', 'sata',
                     '--controller', 'IntelAhci', '--portcount', '1',
                     '--hostiocache', 'off', '--bootable', 'on')
        self.command('convertfromraw', raw, self.output / 'boot.vdi', '--format', 'VDI')
        self.command('storageattach', self.vm_uuid, '--storagectl', 'Proof SATA',
                     '--port', '0', '--device', '0', '--type', 'hdd',
                     '--medium', self.output / 'boot.vdi', '--mtype', 'normal')
        self.receipt['configured_state'] = self.state()
        if self.receipt['configured_state'] != 'poweroff':
            raise proof_error('new VM was not powered off before startup')
        self.write_receipt()

    def run(self):
        self.prepare()
        self.guard_owner()
        self.command('startvm', self.vm_uuid, '--type', 'headless')
        self.sample(force=True)
        deadline = time.monotonic() + self.arguments.timeout
        while True:
            self.limits()
            self.sample()
            if observe_transcript(bounded_read(self.serial, serial_limit), self.arguments.expect_event):
                self.receipt['event_observed'] = True
                break
            if time.monotonic() >= deadline:
                raise proof_error('guest event deadline expired')
            if self.state() not in ('running', 'starting'):
                raise proof_error('VM stopped before the expected event')
            time.sleep(0.1)
        self.guard_owner()
        screenshot = self.output / 'screen.png'
        self.command('controlvm', self.vm_uuid, 'screenshotpng', screenshot, '0')
        width, height = png_dimensions(bounded_read(screenshot, png_limit))
        self.receipt['screenshot'] = {
            'path': 'screen.png', 'sha256': digest(screenshot),
            'width': width, 'height': height, 'pixels_semantically_verified': False,
        }

    def cleanup(self):
        # Registration/configuration can succeed before a CLI is interrupted.
        # Use the private config's exact UUID/name guard even in that case.
        if not self.config.exists():
            return
        self.guard_owner()
        state = self.state(cleanup=True)
        if state != 'poweroff':
            if state == 'aborted':
                raise proof_error('VM aborted; clean poweroff was not established')
            self.command('controlvm', self.vm_uuid, 'poweroff', cleanup=True)
        deadline = time.monotonic() + 10
        while self.state(cleanup=True) != 'poweroff':
            if time.monotonic() >= deadline:
                raise proof_error('VM did not reach poweroff during cleanup')
            time.sleep(0.1)
        self.receipt['cleanup_powered_off'] = True
        self.sample(force=True)


def parse_arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', type=Path, required=True, help='Already-built raw boot image.')
    parser.add_argument('--output', type=Path, required=True, help='New private evidence directory; must not exist.')
    parser.add_argument('--expect-event', required=True, help='Exact schema-1 info event that triggers capture.')
    parser.add_argument('--timeout', type=int, default=30, help='Guest event deadline, 1..300 seconds; CLI calls separately bounded to 20 seconds.')
    parser.add_argument('--memory', type=int, default=512, help='Guest MiB, 96..512 (default 512).')
    arguments = parser.parse_args(argv)
    if not 1 <= arguments.timeout <= 300 or not 96 <= arguments.memory <= 512:
        parser.error('timeout must be 1..300 seconds and memory 96..512 MiB')
    if not re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', arguments.expect_event):
        parser.error('expect-event must be a bounded event identifier')
    return arguments


def main(argv=None):
    arguments = parse_arguments(argv)
    os.umask(0o077)
    try:
        run = proof_run(arguments)
    except OSError as error:
        print(f'VirtualBox proof setup rejected: {error}', file=sys.stderr)
        return 1
    exit_code = 1

    def stop(signum, _frame):
        raise terminated(signum)

    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, stop)
    try:
        run.run()
        exit_code = 0
    except terminated as error:
        exit_code = 128 + error.signum
        run.receipt['error'] = f'interrupted by signal {error.signum}'
    except Exception as error:
        run.receipt['error'] = str(error)
    finally:
        # A second catchable termination cannot interrupt bounded cleanup.
        # SIGKILL/host failure cannot be handled; the retained UUID/config allow
        # the parent to identify and stop exactly this run if recovery is needed.
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(signum, signal.SIG_IGN)
        try:
            run.cleanup()
            run.limits()
            if run.receipt['event_observed']:
                if not observe_transcript(bounded_read(run.serial, serial_limit), arguments.expect_event):
                    raise proof_error('expected event disappeared from the final serial log')
        except Exception as error:
            exit_code = exit_code or 1
            run.receipt['cleanup_error'] = str(error)
        run.receipt['serial_sha256'] = digest(run.serial)
        run.receipt['result'] = 'event_observed' if exit_code == 0 else 'failed'
        run.receipt['exit_code'] = exit_code
        run.write_receipt()
    print(json.dumps(run.receipt, indent=2))
    return exit_code


if __name__ == '__main__':
    sys.exit(main())
