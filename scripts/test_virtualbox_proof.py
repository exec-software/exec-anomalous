#!/usr/bin/env python3
"""Safety and evidence tests; all VirtualBox operations are mocked.

Run with: python3 -B -m unittest discover -s scripts -p test_virtualbox_proof.py
"""

import argparse
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import signal
import struct
import tempfile
import unittest
from unittest import mock


spec = importlib.util.spec_from_file_location(
    'virtualbox_proof', Path(__file__).with_name('virtualbox-proof.py'))
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)

event = 'kernel.native_relaunch_test.passed'
ready = json.dumps({'schema': 1, 'event': event, 'level': 'info'}).encode() + b'\n'
png = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR' + struct.pack('>II', 1024, 768) + b'\x08\x02\0\0\0\0\0\0\0'


class fake_run(proof.proof_run):
    """Emulate CLI-visible transitions, including failures after guest startup."""
    def __init__(self, arguments):
        super().__init__(arguments)
        self.calls = []
        self.vm_state = 'poweroff'
        self.fail_after_start = None
        self.fail_poweroff = False
        self.truncate_after_poweroff = False

    def sample(self, force=False):
        pass

    def command(self, *arguments, cleanup=False):
        self.calls.append(tuple(map(str, arguments)))
        operation = arguments[0]
        if operation == '--version':
            return 'mock-7.2.6\n'
        if operation == 'createvm':
            self.config.parent.mkdir(parents=True)
            self.config.write_text(
                f'<VirtualBox><Machine uuid="{{{self.vm_uuid}}}" '
                f'name="{self.vm_name}"/></VirtualBox>')
        elif operation == 'convertfromraw':
            Path(arguments[2]).write_bytes(b'mock VDI')
        elif operation == 'showvminfo':
            return f'VMState="{self.vm_state}"\n'
        elif operation == 'startvm':
            self.vm_state = 'running'
            self.serial.write_bytes(ready)
            if self.fail_after_start is not None:
                raise self.fail_after_start
        elif operation == 'controlvm':
            if arguments[2] == 'screenshotpng':
                Path(arguments[3]).write_bytes(png)
            elif arguments[2] == 'poweroff':
                if self.fail_poweroff:
                    raise proof.proof_error('mock poweroff failure')
                self.vm_state = 'poweroff'
                if self.truncate_after_poweroff:
                    self.serial.write_bytes(b'')
            else:
                raise AssertionError(arguments)
        elif operation not in ('modifyvm', 'storagectl', 'storageattach'):
            raise AssertionError(arguments)
        return ''


class transcript_tests(unittest.TestCase):
    def test_complete_exact_event_is_required(self):
        self.assertFalse(proof.observe_transcript(ready[:-1], event))
        self.assertFalse(proof.observe_transcript(ready, event + '.other'))
        self.assertFalse(proof.observe_transcript(b'console: ' + ready, event))
        self.assertTrue(proof.observe_transcript(b'boot banner\n' + ready, event))

    def test_duplicate_boot_and_guest_errors_are_not_success(self):
        boot = b'{"schema":1,"event":"boot.kernel.enter","level":"info"}\n'
        for value in [ready + ready, boot + boot + ready,
                      ready + b'{"event":"failure","level":"fatal"}\n',
                      ready.replace(b'"info"', b'"warning"'),
                      b'{bad json}\n' + ready,
                      b'\xff\n' + ready]:
            with self.subTest(value=value):
                with self.assertRaises(proof.proof_error):
                    proof.observe_transcript(value, event)

    def test_serial_and_png_transport_bounds(self):
        with self.assertRaises(proof.proof_error):
            proof.observe_transcript(b' ' * (proof.serial_limit + 1), event)
        self.assertEqual(proof.png_dimensions(png), (1024, 768))
        for value in [png[:32], b'bad signature' + png,
                      png[:16] + struct.pack('>II', 640, 480) + png[24:]]:
            with self.assertRaises(proof.proof_error):
                proof.png_dimensions(value)


class harness_tests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.image = self.root / 'input.img'
        self.image.write_bytes(b'bounded test image')
        self.arguments = argparse.Namespace(
            image=self.image, output=self.root / 'run', expect_event=event,
            timeout=1, memory=512,
        )

    def invoke(self, run):
        argv = ['--image', str(self.image), '--output', str(self.arguments.output),
                '--expect-event', event, '--timeout', '1']
        # No signal handler or process-global umask is changed by these tests.
        with mock.patch.object(proof, 'proof_run', return_value=run), \
                mock.patch.object(proof.signal, 'signal'), \
                mock.patch.object(proof.os, 'umask'), \
                contextlib.redirect_stdout(io.StringIO()):
            return proof.main(argv)

    def test_existing_output_or_symlink_is_never_reused(self):
        self.arguments.output.mkdir()
        marker = self.arguments.output / 'preserve'
        marker.write_text('existing evidence')
        with self.assertRaises(FileExistsError):
            proof.proof_run(self.arguments)
        self.assertEqual(marker.read_text(), 'existing evidence')
        alternate = self.root / 'alternate'
        alternate.symlink_to(self.arguments.output, target_is_directory=True)
        self.arguments.output = alternate
        with self.assertRaises(FileExistsError):
            proof.proof_run(self.arguments)

    def test_each_run_overrides_inherited_config_and_ipc_namespace(self):
        inherited = {'VBOX_USER_HOME': '/existing/config',
                     'VBOX_IPC_SOCKETID': 'existing-service'}
        with mock.patch.dict(proof.os.environ, inherited):
            first = proof.proof_run(self.arguments)
            second_arguments = argparse.Namespace(**vars(self.arguments))
            second_arguments.output = self.root / 'second-run'
            second = proof.proof_run(second_arguments)
            self.assertNotEqual(first.ipc_socket_id, second.ipc_socket_id)
            self.assertNotEqual(first.private_home, second.private_home)
            for run in (first, second):
                self.assertEqual(run.environment['VBOX_USER_HOME'], str(run.private_home))
                self.assertEqual(run.environment['VBOX_IPC_SOCKETID'], run.ipc_socket_id)
                self.assertNotEqual(run.ipc_socket_id, inherited['VBOX_IPC_SOCKETID'])
                self.assertRegex(run.ipc_socket_id, r'^anomalous-[0-9a-f-]{36}$')
                socket_path = f'/tmp/.vbox-{run.ipc_socket_id}-ipc/ipcd'.encode('ascii')
                self.assertLess(len(socket_path), 108)
                receipt = json.loads((run.output / 'receipt.json').read_text())
                self.assertEqual(receipt['vbox_ipc_socket_id'], run.ipc_socket_id)
            for key, value in inherited.items():
                self.assertEqual(proof.os.environ[key], value)

    def test_guard_rejects_foreign_uuid_without_poweroff(self):
        run = fake_run(self.arguments)
        run.prepare()
        run.config.write_text('<VirtualBox><Machine uuid="foreign" name="foreign"/></VirtualBox>')
        calls = list(run.calls)
        with self.assertRaises(proof.proof_error):
            run.cleanup()
        self.assertEqual(calls, run.calls)

    def test_guard_rejects_configuration_symlink_escape(self):
        run = fake_run(self.arguments)
        run.prepare()
        foreign = self.root / 'foreign.vbox'
        run.config.replace(foreign)
        run.config.symlink_to(foreign)
        with self.assertRaises(proof.proof_error):
            run.cleanup()

    def test_success_preserves_evidence_and_does_not_claim_semantics(self):
        run = fake_run(self.arguments)
        self.assertEqual(self.invoke(run), 0)
        self.assertTrue(run.receipt['cleanup_powered_off'])
        self.assertEqual(run.receipt['result'], 'event_observed')
        self.assertFalse(run.receipt['semantic_qualified'])
        self.assertFalse(run.receipt['screenshot']['pixels_semantically_verified'])
        self.assertEqual(run.environment['VBOX_USER_HOME'], str(run.private_home))
        self.assertNotEqual(run.vm_name, 'Anomalous Dev [Threshold]')
        for name in ['receipt.json', 'serial.log', 'screen.png', 'boot.img', 'boot.vdi']:
            self.assertTrue((run.output / name).is_file())
        self.assertTrue(run.config.is_file())
        self.assertFalse(any(call[0] in ('unregistervm', 'closemedium') for call in run.calls))
        self.assertEqual(sum(call[0] == 'convertfromraw' for call in run.calls), 1)
        for call in run.calls:
            if call[0] in ('modifyvm', 'storagectl', 'storageattach', 'startvm', 'controlvm', 'showvminfo'):
                self.assertEqual(call[1], run.vm_uuid)

    def test_startup_error_still_powers_off_created_guest(self):
        run = fake_run(self.arguments)
        run.fail_after_start = proof.proof_error('mock CLI failed after VM started')
        self.assertEqual(self.invoke(run), 1)
        self.assertTrue(run.receipt['cleanup_powered_off'])
        self.assertEqual(run.vm_state, 'poweroff')

    def test_termination_still_powers_off_created_guest(self):
        run = fake_run(self.arguments)
        run.fail_after_start = proof.terminated(signal.SIGTERM)
        self.assertEqual(self.invoke(run), 128 + signal.SIGTERM)
        self.assertTrue(run.receipt['cleanup_powered_off'])
        self.assertIn('signal', run.receipt['error'])

    def test_failed_cleanup_cannot_report_success(self):
        run = fake_run(self.arguments)
        run.fail_poweroff = True
        self.assertEqual(self.invoke(run), 1)
        self.assertEqual(run.receipt['result'], 'failed')
        self.assertFalse(run.receipt['cleanup_powered_off'])

    def test_truncated_final_serial_cannot_report_success(self):
        run = fake_run(self.arguments)
        run.truncate_after_poweroff = True
        self.assertEqual(self.invoke(run), 1)
        self.assertEqual(run.receipt['result'], 'failed')
        self.assertIn('disappeared', run.receipt['cleanup_error'])

    def test_cli_deadline_kills_only_its_new_process_group(self):
        run = proof.proof_run(self.arguments)
        child = mock.Mock(pid=123456)
        child.poll.return_value = None
        child.wait.return_value = -signal.SIGKILL
        with mock.patch.object(proof.subprocess, 'Popen', return_value=child) as spawn, \
                mock.patch.object(proof.os, 'killpg') as kill, \
                mock.patch.object(proof.time, 'monotonic', side_effect=[0, 21, 22]), \
                mock.patch.object(run, 'sample'):
            with self.assertRaisesRegex(proof.proof_error, 'timed out'):
                run.command('showvminfo', run.vm_uuid, '--machinereadable')
        kill.assert_called_once_with(child.pid, signal.SIGKILL)
        self.assertTrue(spawn.call_args.kwargs['start_new_session'])
        self.assertEqual(spawn.call_args.kwargs['env']['VBOX_USER_HOME'], str(run.private_home))
        self.assertEqual(spawn.call_args.kwargs['env']['VBOX_IPC_SOCKETID'], run.ipc_socket_id)
        child.wait.assert_called_once_with(timeout=2)


if __name__ == '__main__':
    unittest.main()
