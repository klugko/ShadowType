# Code Racer

A Rust terminal typing trainer with an editor appearance and private realtime races.

```text
 code-racer                                             SoloRace
 main.rs

   1 │ fn calculate_speed(chars: usize, seconds: f64) -> f64 {
   2 │     let minutes = seconds / 60.0;
   3 │     chars as f64 / 5.0 / minutes
   4 │ }

 ──────────────────────────────────────────────────────────────
 INSERT │ rust │ 84.1 WPM │ raw 86.2 │ 97.5% │ errors: 2 │ 00:18
```

## Features

- Words (10/25/50/100), timed (15/30/60/120 seconds), quote and code sessions.
- English and French; Rust, Python, TypeScript and SQL snippets. JavaScript is an alias for the TypeScript snippet.
- Grapheme-aware comparison/backspace, editor gutters, current character cursor, correct/error/remaining styles and viewport scrolling.
- Live WPM, raw WPM, accuracy, errors, elapsed time and progress.
- Private six-character rooms, up to eight players by default, ready checks, server-controlled countdown, realtime progress and final ranking.
- Host promotion on disconnect, disconnected racers marked offline, race timeout and abandoned-room cleanup.
- Local TOML settings, JSON history and personal best/average/accuracy/training time.
- Editor, dark and monochrome themes. Terminal restoration on normal exit, errors and unwinding panics.

## Installation

Install a current stable Rust toolchain (Linux, Fedora, Ubuntu or Windows):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows use the installer at https://rustup.rs and the MSVC build tools. Fedora packages are also supported:

```bash
sudo dnf install rust cargo rustfmt clippy
```

Ubuntu users can install build tools with `sudo apt install build-essential` and Rust via rustup.

From the repository root:

```bash
cargo build --release --workspace
cargo install --path .
cargo install --path crates/server
code-racer
```

The client package lives at the workspace root so `cargo run` and `cargo install --path .` work directly. There is no separate client manifest.

## Usage

```bash
cargo run
code-racer --help
code-racer solo
code-racer solo --mode words --language french --words 25
code-racer solo --mode time --seconds 60
code-racer solo --mode quote --language french
code-racer solo --mode code --language rust
code-racer multiplayer
code-racer create
code-racer join FK72AD --server ws://192.168.1.42:8080
```

On first launch, choose a username and press Enter. Direct create/join commands open their respective forms; Enter connects. CLI flags override settings for that invocation.

## Solo mode

Choose **Solo** from Home (`s`). Use `j/k` to choose a setting and `h/l` to change it. Enter starts immediately. The timer starts with the first keystroke; timed sessions stop at their deadline. Other sessions finish at the end of the text. Errors can be corrected with Backspace; reaching the end still finishes if errors remain. Corrected mistakes remain in cumulative accuracy/error statistics.

WPM is current correctly matched graphemes / 5 / elapsed minutes. Raw WPM counts all printable input attempts, including corrected input. Accuracy counts correct keystrokes / total keystrokes; a scalar extending a combining grapheme is correct if the current grapheme is a prefix of the expected one. Backspace is not an input attempt. Empty sessions have 0 WPM and 100% accuracy. In races elapsed time starts at the server's start, including hesitation before typing.

Code preserves literal indentation and punctuation. Enter types a newline; Tab inserts four spaces. Settings select the default natural language; use the solo config or CLI to select code languages.

## Multiplayer

1. Start the server.
2. First client: Multiplayer → Create room → share its displayed code.
3. Second client: Multiplayer → Join room → enter the code → Enter.
4. Each player presses `r` (or Enter) to become ready.
5. Host presses `s` after everyone is ready.
6. A server countdown starts, then both clients type the identical text.
7. Progress is sent every 100 ms. When everyone finishes/disconnects or the timeout expires, results appear.
8. Host presses `r` or `l` to return everyone to the lobby; toggle ready again for another race.

Finished racers keep watching other players. Ranking places finishers first by server-measured completion time, then unfinished racers by progress. Unfinished participants are marked DNF. Esc leaves the room. There is no automatic reconnection; rejoin a waiting room after connection loss.

## Running your own server

```bash
cargo run -p code-racer-server -- --host 127.0.0.1 --port 8080
# or after installation
code-racer-server --host 0.0.0.0 --port 8080
code-racer-server --max-players 8 --room-ttl 1800 --race-timeout 180
```

Defaults: localhost:8080, 8 players, 30-minute inactivity expiry, 180-second racing timeout (after the countdown). Rooms are in memory; restarting the server removes them. Logs go to the server's stdout, never to the client's TUI.

## LAN multiplayer

On machine A:

```bash
code-racer-server --host 0.0.0.0 --port 8080
```

On machines B and C (replace the IP with machine A's LAN address):

```bash
code-racer --server ws://192.168.1.42:8080
```

Open TCP 8080 in the host firewall if necessary. On Fedora:

```bash
sudo firewall-cmd --add-port=8080/tcp
```

The server binds localhost by default. Plain `ws://` suits trusted LANs. The client supports `wss://`; put a TLS WebSocket reverse proxy in front of the server for Internet use. No code received from clients is executed. This is a lightweight private-race server, not a hardened public competition service.

## Keybindings

| Context | Keys | Action |
| --- | --- | --- |
| Menus | j/k or arrows, Enter | Navigate/select |
| Home | s, m, ? | Solo, multiplayer, help |
| Solo config | h/l or left/right | Change option |
| Multiplayer | c, j | Create/join |
| Lobby | r or Enter, s | Ready, host start |
| Typing | printable keys, Backspace | Type/correct |
| Code | Enter, Tab | Newline, four spaces |
| Results | r, l | Retry solo / host returns to lobby |
| History | j/k | Scroll |
| Outside typing | Esc, q | Home/leave, quit |
| Typing | Esc | Abandon session/leave room |
| Everywhere | Ctrl+C | Exit and restore terminal |

Settings starts with an editable username. Enter validates/saves it and moves to Theme. Then j/k selects Theme/Language and l changes values; Esc returns home. Minimum terminal size: 80×20. A small-terminal message replaces the view until resized.

## Configuration

Linux: `~/.config/code-racer/config.toml`, history at `~/.local/share/code-racer/history.json`. `XDG_CONFIG_HOME` / `XDG_DATA_HOME` are respected. Windows uses the platform configuration/data directories chosen by `directories` (under AppData).

```toml
username = "Jean"
language = "french"
theme = "editor"
default_mode = "words"
word_count = 50

[multiplayer]
server = "ws://127.0.0.1:8080"
```

Settings are saved when edited; history is saved when a solo session completes or a multiplayer result becomes available. Abandoned sessions are omitted. Writes use a temporary file and recoverable `.bak` backup. Invalid persisted files produce an error before terminal mode is entered and are preserved. Run one client per profile to avoid concurrent history writes; multiple testing clients can use different XDG directories.

## Development

```bash
rustup component add rustfmt clippy
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace
python3 tests/terminal_smoke.py  # Unix only, after release build
```

## Architecture

- Root package `code-racer`: client source under `crates/client/src`.
- `app.rs`: explicit state machine, input handling and lifecycle; `ui.rs`: rendering only.
- `typing.rs`: UI-independent grapheme typing engine and statistics.
- `storage.rs`: platform settings/history; `network.rs`: bounded channels and WebSocket IO.
- `crates/protocol`: versioned tagged JSON enums, snapshots, validation, ranking and bundled texts.
- `crates/server`: connection-bound UUID identity, rooms and authoritative timing.

The server uses a shared hub with short synchronous operations under a Tokio mutex; no network awaits occur under its lock. Bounded per-client outbound queues avoid unbounded buffering. Snapshots replace the entire room view, simplifying synchronization and future protocol evolution. Messages/frames are limited to 8 KiB, clients to 100 incoming messages per second, rooms to 1024. Usernames must contain 1–24 printable graphemes. IDs are assigned by the server; clients cannot choose another player's ID. Progress bounds and numeric relationships are validated; host-only operations are enforced. Position, correct counts and attempts are still self-reported: minimum anti-cheat, not proof of actual typing.

## Testing

Unit tests verify WPM/accuracy, correction accounting, graphemes/combining characters/emoji, time limits, navigation, renderer resize safety, persistence, room validation/limits/cleanup and protocol roundtrip. Two actual WebSocket clients test create → join → ready → countdown → race → progress → finish → ranking → reset, non-host and invalid-progress rejection, malformed JSON, disconnection, host promotion and timeout. The Unix PTY smoke test types a full quote using the actual release client, verifies results/restart/history/resize and checks terminal flags after Ctrl+C.

GitHub Actions runs format, Clippy, tests and release builds on Linux and Windows, plus the PTY test on Linux. A separate job builds and starts the Docker image, verifies a WebSocket handshake and publishes the validated image to GHCR on `main`. Downloadable client/server artifacts are attached to each successful CI run.

## Docker server

```bash
docker build -t code-racer-server .
docker run --rm -p 8080:8080 code-racer-server
# CI-published image (registry visibility/access may require authentication)
docker run --rm -p 8080:8080 ghcr.io/klugko/code-racer-server:latest
```

Multi-stage build; the final Debian image runs only the server as a non-root user. Publication to GHCR provides a deployable image; a continuously running public endpoint requires a hosting machine and is not provisioned by this repository.

## Roadmap / current limits

- More languages, corpora and quote variety; current words sample a compact bundled corpus.
- Automatic reconnection, spectators, replay and custom text import.
- Full Unicode normalization/IME paste support; composed and decomposed forms currently compare literally.
- Rich syntax highlighting; current code mode preserves layout and uses typing-state colors.
- Public-server authentication, per-IP connection limits and stronger anti-cheat.
- Multi-process safe persistence and advanced historical charts.
