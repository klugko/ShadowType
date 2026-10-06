"""End-to-end Unix test: real server and two real terminal clients."""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import socket
import struct
import subprocess
import tempfile
import termios
import time

root = Path(__file__).resolve().parent.parent
clients = []
server = None


def start_client(directory, name, command):
    config = Path(directory) / name / 'code-racer'
    config.mkdir(parents=True)
    (config / 'config.toml').write_text(f'username = "{name}"\n')
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
    def controlling_terminal():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    env = dict(os.environ, XDG_CONFIG_HOME=str(config.parent), XDG_DATA_HOME=str(config.parent), TERM='xterm-256color')
    process = subprocess.Popen([str(root / 'target/release/code-racer'), '--server', url, *command], stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=controlling_terminal)
    client = (process, master, slave, bytearray(), config.parent)
    clients.append(client)
    return client


def pump(seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        readable, _, _ = select.select([c[1] for c in clients], [], [], 0.01)
        for client in clients:
            if client[1] in readable:
                client[3].extend(os.read(client[1], 65536))


def visible(client):
    # Replay cursor moves rather than concatenating differential terminal output.
    cells = [[' '] * 120 for _ in range(30)]
    row = column = 0
    text = client[3].decode('utf8', 'replace')
    for match in re.finditer(r'\x1b\[[0-?]*[ -/]*[@-~]|[^\x1b]|\x1b.', text):
        token = match.group()
        if token.startswith('\x1b['):
            if token[-1] in 'Hf':
                coordinates = token[2:-1].split(';')
                row = int(coordinates[0] or 1) - 1
                column = int(coordinates[1] or 1) - 1 if len(coordinates) > 1 else 0
            elif token == '\x1b[2J':
                cells = [[' '] * 120 for _ in range(30)]
        elif token == '\n':
            row += 1
            column = 0
        elif token == '\r':
            column = 0
        elif not token.startswith('\x1b') and token.isprintable():
            if 0 <= row < 30 and 0 <= column < 120:
                cells[row][column] = token
            column += 1
    return '\n'.join(''.join(line) for line in cells)


with tempfile.TemporaryDirectory(prefix='code-racer-multi-') as directory:
    port_socket = socket.socket()
    port_socket.bind(('127.0.0.1', 0))
    port = port_socket.getsockname()[1]
    port_socket.close()
    url = f'ws://127.0.0.1:{port}'
    try:
        server = subprocess.Popen([str(root / 'target/release/code-racer-server'), '--port', str(port)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.3)
        alice = start_client(directory, 'Alice', ['create'])
        pump(0.3)
        os.write(alice[1], b'\r')
        pump(0.5)
        match = re.search(r'Room: ([A-Z2-9]{6})', visible(alice))
        assert match, visible(alice)[-1000:]
        bob = start_client(directory, 'Bob', ['join', match.group(1)])
        pump(0.3)
        os.write(bob[1], b'\r')
        pump(0.5)
        assert 'Bob' in visible(alice) and 'Alice' in visible(bob), 'both participants missing'
        os.write(alice[1], b'r')
        os.write(bob[1], b'r')
        pump(0.3)
        os.write(alice[1], b's')
        pump(0.5)
        for c in clients:
            assert 'Countdown' in visible(c), visible(c)
        pump(3.2)
        for c in clients:
            assert 'MultiplayerRace' in visible(c), visible(c)
        target = (root / 'texts/english.txt').read_text().rstrip()
        for character in target:
            for c in clients:
                os.write(c[1], character.encode())
            pump(0.005)
        pump(0.7)
        for c in clients:
            files = list(c[4].rglob('history.json'))
            assert files, 'no saved multiplayer result'
            record = json.loads(files[0].read_text())[0]
            assert record['mode'] == 'multiplayer' and record['stats']['accuracy'] == 100, record
            assert 'Results' in visible(c), 'results screen missing'
        os.write(alice[1], b'r')
        pump(0.5)
        for c in clients:
            assert 'Lobby' in visible(c), 'race again did not return to lobby'
            os.write(c[1], b'\x03')
            c[0].wait(timeout=5)
            assert c[0].returncode == 0
        print('Two PTYs: create, join, ready, countdown, actual typing, results, saved stats, race again: PASS')
    finally:
        for process, master, slave, _, _ in clients:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)
        if server is not None:
            server.send_signal(signal.SIGINT)
            server.wait(timeout=5)
