#!/usr/bin/env python3
"""End-to-end Linux shutdown checks against real CLI/daemon binaries."""
import argparse
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

parser = argparse.ArgumentParser()
parser.add_argument('--cli', required=True)
parser.add_argument('--daemon', required=True)
options = parser.parse_args()
CLI, DAEMON = str(Path(options.cli).resolve()), str(Path(options.daemon).resolve())

WRITER = r'''
import os, pathlib, signal, subprocess, sys, time
home=pathlib.Path.home()
mode=sys.argv[1]
count=int(sys.argv[2])
child=None
def finish(*_):
    if child is not None: child.wait(timeout=3)
    time.sleep(.1)
    os.write(1, (mode+' TERM OUT\n').encode())
    os.write(2, (mode+' TERM ERR\n').encode())
    sys.exit(0)
signal.signal(signal.SIGTERM, signal.SIG_IGN if mode=='stubborn' else finish)
if mode=='parent':
    child=subprocess.Popen([sys.executable,__file__,'leaf','0'])
    while not (home/'leaf.ready').exists(): time.sleep(.01)
for i in range(count):
    os.write(1, ('OUT %06d '%i+'x'*110+'\n').encode())
    os.write(2, ('ERR %06d '%i+'y'*110+'\n').encode())
(home/(mode+'.ready')).write_text(str(os.getpid()))
while True: time.sleep(1)
'''


class Shutdown(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='pm3-shutdown-')
        self.home = Path(self.temp.name)
        self.env = {k: v for k, v in os.environ.items() if not k.startswith('PM3_')}
        self.env['HOME'] = str(self.home)
        self.output = (self.home/'daemon.log').open('w')
        self.daemon = subprocess.Popen([DAEMON], cwd=self.home, env=self.env,
                                       stdout=self.output, stderr=subprocess.STDOUT)
        self.addCleanup(self.cleanup)
        self.writer = self.home/'writer.py'
        self.writer.write_text(WRITER)
        self.wait_for(lambda: (self.home/'.pm3/config.proc').exists())
        self.cli('ping')

    def cleanup(self):
        # These PIDs come only from fixtures created in this test's private HOME.
        if self.daemon.poll() is None:
            self.daemon.kill()
            self.daemon.wait(timeout=5)
        for marker in self.home.glob('*.ready'):
            try:
                pid = int(marker.read_text())
                if str(self.writer).encode() in Path(f'/proc/{pid}/cmdline').read_bytes():
                    os.kill(pid, signal.SIGKILL)
            except (ProcessLookupError, FileNotFoundError): pass
        self.output.close()
        self.temp.cleanup()

    def wait_for(self, predicate):
        deadline = time.monotonic()+15
        while time.monotonic() < deadline:
            if predicate(): return
            if self.daemon.poll() is not None:
                self.fail((self.home/'daemon.log').read_text())
            time.sleep(.02)
        self.fail('fixture timed out: '+(self.home/'daemon.log').read_text())

    def cli(self, *args):
        result = subprocess.run([CLI, *args], env=self.env, cwd=self.home,
                                text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
        return result.stdout

    def start(self, mode, count, limit=10):
        self.cli('start', sys.executable, '--name', 'probe', '--max-size', str(limit),
                 '--', '-u', str(self.writer), mode, str(count))
        self.wait_for(lambda: (self.home/(mode+'.ready')).exists())

    def stop_daemon(self, sig=signal.SIGTERM, expected=0):
        started = time.monotonic()
        self.daemon.send_signal(sig)
        self.assertEqual(self.daemon.wait(timeout=25), expected,
                         (self.home/'daemon.log').read_text())
        return time.monotonic()-started

    def log(self, stream):
        folder = self.home/'.pm3/processes/probe'
        return b''.join(p.read_bytes() for p in
                        [folder/(stream+'.old.log'), folder/(stream+'.log')] if p.exists())

    def assert_logs(self, count, mode, tail=True):
        for stream, label, char in [('stdout', 'OUT', 'x'), ('stderr', 'ERR', 'y')]:
            expected = ''.join(f'{label} {i:06d} '+char*110+'\n' for i in range(count))
            if tail: expected += mode+' TERM '+label+'\n'
            self.assertEqual(self.log(stream), expected.encode())

    def test_term_drains_backlog_and_signal_handler_output(self):
        self.start('normal', 20000)
        self.stop_daemon()
        self.assert_logs(20000, 'normal')

    def test_int_drains_small_unflushed_buffer(self):
        self.start('normal', 3)
        self.stop_daemon(signal.SIGINT)
        self.assert_logs(3, 'normal')

    def test_rotation_is_complete_before_exit(self):
        self.start('normal', 12000, limit=1)
        self.stop_daemon()
        self.assert_logs(12000, 'normal')

    def test_descendant_pipes_are_drained(self):
        self.start('parent', 0)
        self.stop_daemon()
        for stream, label in [('stdout', 'OUT'), ('stderr', 'ERR')]:
            self.assertEqual(self.log(stream), f'leaf TERM {label}\nparent TERM {label}\n'.encode())

    def test_stubborn_process_is_bounded_and_logs_are_kept(self):
        self.start('stubborn', 5)
        elapsed = self.stop_daemon()
        self.assertGreaterEqual(elapsed, 9)
        self.assertLess(elapsed, 20)
        self.assert_logs(5, 'stubborn', tail=False)

    def test_write_failure_is_reported_as_failed_shutdown(self):
        folder = self.home/'.pm3/processes/probe'
        folder.mkdir(parents=True)
        (folder/'stdout.log').symlink_to('/dev/full')
        self.start('normal', 3)
        self.stop_daemon(expected=1)
        self.assertIn('shutdown error', (self.home/'daemon.log').read_text())


if __name__ == '__main__':
    unittest.main(argv=[sys.argv[0]], verbosity=2)
