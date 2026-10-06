"""Solo session in a real terminal: first launch, typing, results, resize, Ctrl+C."""

import signal
import tempfile
from pathlib import Path

from harness import CLIENT, Terminal, environment, history

TEXT = "Typing fast is fun.\nfn main() {\n    run();\n}\n"
TYPED = "Typing fast is fun.\rfn main() {\rrun();\r}"


def main():
    with tempfile.TemporaryDirectory(prefix="code-racer-solo-") as home:
        exercise = Path(home) / "exercise.txt"
        exercise.write_text(TEXT)
        terminal = Terminal([str(CLIENT), "solo", "--file", str(exercise)], environment(home))
        try:
            run(terminal, home)
        finally:
            terminal.close()
        for stop in (signal.SIGTERM, signal.SIGHUP):
            stops_cleanly(stop, home)
    print("solo: first launch, typing with auto-indent, results, history, restart, resize, Ctrl+C, SIGTERM, SIGHUP: PASS")


def run(terminal, home):
    terminal.wait_for(r"config\.toml")
    terminal.send("Tester\r")
    terminal.wait_for(r"INSERT .*exercise\.txt")

    terminal.send(TYPED, delay=0.01)
    terminal.wait_for(r"session complete")
    records = history(home)
    assert len(records) == 1, records
    assert records[0]["mode"] == "file" and records[0]["accuracy"] == 100, records[0]

    terminal.send("r")
    terminal.wait_for(r"INSERT")
    terminal.send("q")
    terminal.wait_for(r"1 error")
    assert terminal.process.poll() is None, "q is text while typing"

    terminal.resize(8, 40)
    terminal.wait_for(r"Terminal too small")
    terminal.resize(30, 120)
    terminal.wait_for(r"INSERT")

    terminal.send("\x03")
    assert terminal.process.wait(timeout=5) == 0, "Ctrl+C exits cleanly"
    assert terminal.mode_restored(), "the terminal is back in cooked mode"


def stops_cleanly(stop, home):
    """A signal from outside quits like :q does, restoring the terminal."""
    terminal = Terminal([str(CLIENT), "history"], environment(home))
    try:
        terminal.wait_for(r"NORMAL .*history\.log")
        terminal.process.send_signal(stop)
        assert terminal.process.wait(timeout=5) == 0, f"{stop.name} exits cleanly"
        assert terminal.mode_restored(), f"the terminal is restored after {stop.name}"
    finally:
        terminal.close()


if __name__ == "__main__":
    main()
