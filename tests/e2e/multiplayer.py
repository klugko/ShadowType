"""A real server and two real terminal clients race against each other."""

import re
import tempfile

from harness import CLIENT, Terminal, environment, history, race_server

EDITOR_LINE_ONE = re.compile(r"^(?:.*│)? *1  (\S.*)$", re.MULTILINE)
QUIET_PERIOD = 0.7  # seconds during which keys are ignored once a player finished


def main():
    with race_server("--countdown", "2") as url, tempfile.TemporaryDirectory(prefix="code-racer-race-") as home:
        terminals = []
        try:
            race(url, home, terminals)
        finally:
            for terminal in terminals:
                terminal.close()
    print("multiplayer: create, join, ready, countdown, typing, standings, results, history, lobby again: PASS")


def race(url, home, terminals):
    """Alice hosts and types the whole text before Bob starts, so she finishes first."""
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
    alice.send(text, delay=0.03)
    bob.send(text, delay=0.03)
    for terminal, place in ((alice, "1st"), (bob, "2nd")):
        terminal.wait_for(r"results · room")
        terminal.wait_for(rf"you finished {place} of 2")

    for player_home in (alice_home, bob_home):
        records = history(player_home)
        assert len(records) == 1 and records[0]["mode"] == "race", records

    alice.read(QUIET_PERIOD)
    alice.send("r")
    for terminal in (alice, bob):
        terminal.wait_for(r"toggle ready")
        terminal.send("\x03")
        assert terminal.process.wait(timeout=5) == 0
        assert terminal.mode_restored()


def race_text(terminal):
    """The race text, read from the editor row numbered 1.

    The row is found by its gutter (the explorer border when the explorer is
    shown, then the right-aligned number and two spaces) rather than by a fixed
    column, so a wider explorer or gutter does not break it. Five words fit on
    that row.
    """
    screen = terminal.screen.text()
    match = EDITOR_LINE_ONE.search(screen)
    assert match, f"no editor row numbered 1 on the screen:\n{screen}"
    return match.group(1)


if __name__ == "__main__":
    main()
