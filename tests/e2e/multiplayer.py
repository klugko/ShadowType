"""A real server and two real terminal clients race against each other."""

import signal
import subprocess
import tempfile
import time

from harness import CLIENT, SERVER, Terminal, environment, free_port, history

SIDEBAR_AND_GUTTER = 29


def main():
    port = free_port()
    url = f"ws://127.0.0.1:{port}"
    server = subprocess.Popen(
        [str(SERVER), "--port", str(port), "--countdown", "2"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    terminals = []
    try:
        time.sleep(0.3)
        with tempfile.TemporaryDirectory(prefix="code-racer-race-") as home:
            race(url, home, terminals)
    finally:
        for terminal in terminals:
            terminal.close()
        server.send_signal(signal.SIGINT)
        assert server.wait(timeout=5) == 0, "the server shuts down cleanly"
    print("multiplayer: create, join, ready, countdown, typing, standings, results, history, lobby again: PASS")


def race(url, home, terminals):
    alice_home, bob_home = f"{home}/alice", f"{home}/bob"
    alice = Terminal(
        [str(CLIENT), "--server", url, "create", "--mode", "words", "--words", "5"],
        environment(alice_home, "alice"),
    )
    terminals.append(alice)
    code = alice.wait_for(r"room ([A-Z2-9]{6})").group(1)

    bob = Terminal([str(CLIENT), "--server", url, "join", code], environment(bob_home, "bob"))
    terminals.append(bob)
    for terminal in (alice, bob):
        terminal.wait_for(r"alice[\s\S]*bob")

    alice.send("s")
    alice.wait_for(r"E: waiting for every player")
    for terminal in (alice, bob):
        terminal.send("r")
    alice.wait_for(r"everyone is ready")
    alice.send("s")
    for terminal in (alice, bob):
        terminal.wait_for(r"starting in")
    for terminal in (alice, bob):
        terminal.wait_for(rf"PLAYERS · {code}")

    text = race_text(alice)
    assert text == race_text(bob), "both players type the same text"
    for terminal in (alice, bob):
        terminal.send(text, delay=0.03)
    for terminal in (alice, bob):
        terminal.wait_for(r"results · room")
        terminal.wait_for(r"you finished (1st|2nd) of 2")

    for player_home in (alice_home, bob_home):
        records = history(player_home)
        assert len(records) == 1 and records[0]["mode"] == "race", records

    alice.send("r")
    for terminal in (alice, bob):
        terminal.wait_for(r"toggle ready")
        terminal.send("\x03")
        assert terminal.process.wait(timeout=5) == 0
        assert terminal.mode_restored()


def race_text(terminal):
    """The race text: five words fit on the first row of the editor."""
    first_row = terminal.screen.text().splitlines()[1]
    return first_row[SIDEBAR_AND_GUTTER:].strip()


if __name__ == "__main__":
    main()
