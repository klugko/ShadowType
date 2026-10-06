"""Unix PTY smoke test against the real release client; no third-party packages."""
import errno
import json
import fcntl
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import tempfile
import termios
import time

root = Path(__file__).resolve().parent.parent
master, slave = pty.openpty()
original = termios.tcgetattr(slave)
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 100, 0, 0))
output = bytearray()


def collect(seconds=0.3):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if select.select([master], [], [], 0.03)[0]:
            try:
                output.extend(os.read(master, 65536))
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break


def child_terminal():
    os.setsid()
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)


with tempfile.TemporaryDirectory(prefix='code-racer-pty-') as directory:
    env = dict(os.environ, XDG_CONFIG_HOME=directory, XDG_DATA_HOME=directory, TERM='xterm-256color')
    process = subprocess.Popen([str(root / 'target/release/code-racer'), 'solo', '--mode', 'quote'], stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=child_terminal)
    try:
        collect()
        os.write(master, b'TestUser\r')
        collect()
        assert b'WPM' in output, 'typing screen missing'
        target = (root / 'texts/english.txt').read_text().rstrip()
        for character in target:
            os.write(master, character.encode())
            collect(0.005)
        collect(1.0)
        history = list(Path(directory).rglob('history.json'))
        assert history, 'completed session was not saved'
        stats = json.loads(history[0].read_text())[0]['stats']
        assert stats['accuracy'] == 100 and stats['position'] == stats['length'], stats
        os.write(master, b'r')
        collect()
        os.write(master, b'q')
        collect()
        assert process.poll() is None, 'printable q must type during a race'
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 8, 30, 0, 0))
        os.kill(process.pid, __import__('signal').SIGWINCH)
        collect()
        visible = re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', output).replace(b' ', b'')
        assert b'Terminaltoosmall' in visible, 'resize fallback missing'
        os.write(master, b'\x03')
        process.wait(timeout=5)
        assert process.returncode == 0, f'exit {process.returncode}'
        restored = termios.tcgetattr(slave)
        assert restored[3] & (termios.ICANON | termios.ECHO) == original[3] & (termios.ICANON | termios.ECHO), 'terminal flags were not restored'
        assert list(Path(directory).rglob('history.json')), 'history was not persisted'
        print('PTY: first launch, solo typing, results, restart, q input, resize, Ctrl+C, history: PASS')
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)
