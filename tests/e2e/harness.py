"""Runs the real release binaries in pseudo-terminals and rebuilds their screen.

Only the standard library is used, so the tests run anywhere Python 3 and a
Unix pseudo-terminal are available.
"""

import codecs
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from contextlib import contextmanager
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CLIENT = ROOT / "target" / "release" / "code-racer"
SERVER = ROOT / "target" / "release" / "code-racer-server"

ESCAPE = r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b[^\[]"
TOKEN = re.compile(ESCAPE + r"|[^\x1b]", re.DOTALL)
COMPLETE_ESCAPE = re.compile(ESCAPE)
LISTENING = re.compile(r"race server listening.*?address=(\S+)")


class Screen:
    """A minimal terminal emulator: enough for cursor-addressed TUI output."""

    def __init__(self, rows, columns):
        self.rows = rows
        self.columns = columns
        self.clear()
        self.decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
        self.pending = ""

    def clear(self):
        self.cells = [[" "] * self.columns for _ in range(self.rows)]
        self.row = 0
        self.column = 0

    def feed(self, data):
        text = self.pending + self.decoder.decode(data)
        start = text.rfind("\x1b")
        if start != -1 and not COMPLETE_ESCAPE.match(text, start):
            text, self.pending = text[:start], text[start:]
        else:
            self.pending = ""
        for token in TOKEN.findall(text):
            self._apply(token)

    def _apply(self, token):
        if token.startswith("\x1b["):
            self._control(token[2:-1], token[-1])
        elif token == "\r":
            self.column = 0
        elif token == "\n":
            self.row = min(self.row + 1, self.rows - 1)
        elif not token.startswith("\x1b") and token.isprintable():
            if 0 <= self.row < self.rows and 0 <= self.column < self.columns:
                self.cells[self.row][self.column] = token
            self.column += 1

    def _control(self, parameters, command):
        if command in "Hf":
            row, _, column = parameters.partition(";")
            self.row = int(row or 1) - 1
            self.column = int(column or 1) - 1
        elif command == "J" and parameters == "2":
            self.clear()

    def text(self):
        return "\n".join("".join(line).rstrip() for line in self.cells)


class Terminal:
    """A program attached to its own pseudo-terminal."""

    def __init__(self, arguments, environment, rows=30, columns=120):
        self.master, self.slave = pty.openpty()
        self.screen = Screen(rows, columns)
        self._set_size(rows, columns)
        self.original_mode = termios.tcgetattr(self.slave)
        self.process = subprocess.Popen(
            arguments,
            stdin=self.slave,
            stdout=self.slave,
            stderr=self.slave,
            env=environment,
            preexec_fn=self._take_controlling_terminal,
        )

    def _take_controlling_terminal(self):
        os.setsid()
        fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

    def _set_size(self, rows, columns):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))

    def resize(self, rows, columns):
        self._set_size(rows, columns)
        self.screen = Screen(rows, columns)
        os.kill(self.process.pid, signal.SIGWINCH)

    def send(self, keys, delay=0.0):
        for key in keys:
            os.write(self.master, key.encode())
            if delay:
                self.read(delay)

    def read(self, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.master], [], [], 0.02)
            if ready:
                try:
                    self.screen.feed(os.read(self.master, 65536))
                except OSError:
                    return

    def wait_for(self, pattern, timeout=10.0):
        """Waits until the screen matches `pattern` and returns the match."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.read(0.05)
            match = re.search(pattern, self.screen.text())
            if match:
                return match
        raise AssertionError(f"screen never showed {pattern!r}:\n{self.screen.text()}")

    def mode_restored(self):
        flags = termios.ICANON | termios.ECHO
        return termios.tcgetattr(self.slave)[3] & flags == self.original_mode[3] & flags

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait()
        os.close(self.master)
        os.close(self.slave)


def environment(home, username=None):
    """Isolated configuration and data directories, optionally with a username."""
    config = Path(home) / "config" / "code-racer"
    config.mkdir(parents=True, exist_ok=True)
    if username:
        (config / "config.toml").write_text(f'username = "{username}"\n')
    return dict(
        os.environ,
        XDG_CONFIG_HOME=str(Path(home) / "config"),
        XDG_DATA_HOME=str(Path(home) / "data"),
        XDG_STATE_HOME=str(Path(home) / "state"),
        TERM="xterm-256color",
    )


def history(home):
    files = list((Path(home) / "data").rglob("history.json"))
    if not files:
        return []
    return json.loads(files[0].read_text())


@contextmanager
def race_server(*arguments, timeout=10.0):
    """Runs the race server on a port picked by the system and yields its URL.

    The server's log goes to a temporary file that is printed when the block
    fails. Leaving the block normally checks that Ctrl+C stops the server
    cleanly.
    """
    with tempfile.NamedTemporaryFile(prefix="code-racer-server-", suffix=".log") as log_file:
        log = Path(log_file.name)
        server = subprocess.Popen(
            [str(SERVER), "--port", "0", *arguments],
            stdout=subprocess.DEVNULL,
            stderr=log_file,
        )
        try:
            yield f"ws://{listening_address(server, log, timeout)}"
            interrupt(server)
        except BaseException:
            print(f"--- server log ---\n{log.read_text(errors='replace')}", file=sys.stderr)
            raise
        finally:
            if server.poll() is None:
                server.kill()
                server.wait()


def listening_address(server, log, timeout):
    """Waits for the line the server logs once its socket is bound."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        match = LISTENING.search(COMPLETE_ESCAPE.sub("", log.read_text(errors="replace")))
        if match:
            return match.group(1)
        if server.poll() is not None:
            raise AssertionError(f"the server exited with status {server.returncode} before listening")
        time.sleep(0.02)
    raise AssertionError(f"the server was not listening after {timeout} s")


def interrupt(server):
    assert server.poll() is None, f"the server stopped early with status {server.returncode}"
    server.send_signal(signal.SIGINT)
    assert server.wait(timeout=5) == 0, "the server shuts down cleanly on Ctrl+C"
