# code-racer

Typing practice that looks like your code editor, with private races against your team on the LAN.

At a glance it is a terminal editor: an explorer, tabs, line numbers, a status line and a Vim command
line. The file you are "editing" is the text you are typing: what is ahead of the cursor is ghost text,
what you typed becomes real, syntax-highlighted code.

```text
 EXPLORER              │ main.rs ● │                                               code-racer
 ▾ code-racer          │  1  pub fn parse_config(
     practice.toml     │  2      text: &str,
     race.toml         │  3  ) -> Result<HashMap<String, String>, String> {
     history.log       │  4      let mut config = HashMap::new();
     config.toml       │  5      for (index, raw) in text.lines().enumerate() {
     help.md           │  6          let line = raw.trim();
                       │  7          if line.is_empty() || line.starts_with('#') {
 ▾ session             │  8              continue;
   ● main.rs           │  9          }
                       │ 10          let (key, value) = line
 RECORDS               │ 11              .split_once('=')
   best     78 wpm     │~
   last 10  68 wpm     │~
 INSERT  main.rs  code · rust                        84 wpm  97%  2 errors  00:18   41%
-- INSERT --  Esc Esc abandon  Ctrl+R restart  Ctrl+W delete word
```

During a race the standings sit in a panel under the text, like an IDE's terminal panel:

```text
                       │  1  Simplicity is prerequisite for reliability.
                       │ PLAYERS · FK72AD ──────────────────────────────────────────────
                       │  1 jean          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100%   92 wpm  ✓ 0:41.2
                       │  2 alice         ━━━━━━━━━━━━━━━━━━━━━───────  75%   87 wpm
                       │  3 bob           ━━━━━━━━━───────────────────  33%   79 wpm  offline
```

## Features

- **Solo practice**: words (10/25/50/100), time (15/30/60/120 s), quotes, code, or any file of yours
  (`:e src/lib.rs`).
- **Text engine**: 1,000 common English and French words, sentences with capitals, commas, quotes,
  parentheses and French typography when punctuation is on, numbers (integers, years, decimals) when
  numbers are on, public-domain quotes, and 100+ real code snippets in Rust, Python, TypeScript,
  JavaScript and SQL.
- **Precise typing engine**: Unicode grapheme comparison with NFC normalisation (`é` typed with a dead
  key or as `e` + accent both count), auto-indentation in code, mistakes must be fixed to finish, and
  input stops 10 characters after an uncorrected mistake.
- **Statistics**: WPM, raw WPM, accuracy, errors, correct and incorrect characters, consistency, time,
  progress, a WPM chart after each session, history and personal records.
- **Races** (TypeRacer style): private rooms with short codes such as `FK72AD`, ready checks, a
  server-driven countdown, live standings, server-measured finishing times and a final ranking.
- **Editor look**: explorer, tab line, line numbers, cursor line, `~` past the end of the buffer,
  lualine-like status line, Vim modes (NORMAL, INSERT, COMMAND) and `:` commands with Tab completion.
- **Themes**: `editor` (true colour), `dark` (the 16 ANSI colours on a black background), `mono` (no colour).
- **Robust terminal handling**: the terminal is restored on exit, on error and on panic; resizing is
  handled, the explorer makes room for the buffer below 100 columns until `Ctrl+B` says otherwise,
  small terminals get a clear message instead of a broken layout; no CPU use while idle.

## Installation

You need a stable Rust toolchain (1.88 or later) and a C compiler with its linker.

**Fedora**

```bash
sudo dnf install rust cargo gcc
```

**Ubuntu, Debian**

```bash
sudo apt install build-essential
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**macOS**

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Windows**: install Rust from <https://rustup.rs> with the MSVC build tools, then use Windows Terminal.

Then, from the repository:

```bash
cargo install --path .                 # the code-racer client
cargo install --path crates/server     # the code-racer-server race server
```

Both binaries land in `~/.cargo/bin`. To try without installing: `cargo run` (client) and
`cargo run -p code-racer-server` (server).

## Usage

```bash
code-racer                                        # open the editor
code-racer solo                                   # start typing right away
code-racer solo --mode words --words 25 --punctuation --numbers
code-racer solo --mode time --seconds 60 --language french
code-racer solo --mode quote --language french
code-racer solo --mode code --language rust       # also python, typescript, javascript, sql
code-racer solo --file src/main.rs                # practise on your own code
code-racer multiplayer                            # open race.toml
code-racer create --words 30                      # create a room
code-racer join FK72AD                            # join a room
code-racer history
code-racer --server ws://192.168.1.42:8080 join FK72AD
code-racer --theme mono
code-racer --help
```

On first launch code-racer asks for the name other racers will see (Esc skips it until you race). It
is saved in `config.toml` and can be changed in the `config.toml` buffer or with `:set username=Jean`.

Flags apply to this run only: `--theme`, `--server` and the `solo` options are never written to
`config.toml`, unless you set the same setting inside the app, even to the value the flag gave it.

## Solo mode

Open `practice.toml` in the explorer (or press `s` anywhere to start immediately). `j`/`k` select a
setting, `h`/`l` change it, Enter on `▶ start session` starts. Settings are saved for next time.

The clock starts with the first keystroke. A session ends when the whole text is typed correctly, or
when the timer runs out in time mode. Mistakes stay in your statistics even once corrected. In code,
Enter goes to the next line and the indentation is filled in for you. `Ctrl+R` restarts with a new
text. Once you have started typing, `Esc` twice abandons the session, so a stray `Esc` costs
nothing; before the first keystroke a single `Esc` closes it. Keys typed right after the end of a
text are ignored for a moment, so that a word typed on the run is not taken as commands. When a
mistake is left uncorrected for 10 characters, input stops and the status line says so until it is
fixed.

| Metric      | Definition                                              |
| ----------- | ------------------------------------------------------- |
| WPM         | correctly typed characters / 5 / elapsed minutes        |
| raw WPM     | every keystroke / 5 / elapsed minutes                   |
| accuracy    | correct keystrokes / all keystrokes × 100               |
| errors      | wrong keystrokes, corrected or not                      |
| consistency | how steady your per-second speed was, from 0 to 100 %  |

The indentation that code mode fills in for you counts towards your progress but never towards
speed or accuracy, in solo sessions and races alike.

## Multiplayer

1. Someone runs a race server (see below).
2. The host opens `race.toml` (`m`), picks the text under `[create]` and selects `▶ create room`
   (or presses `c`, or runs `code-racer create`). The room code appears, e.g. `FK72AD`.
3. Teammates type the code in the `room` line of `race.toml` and press Enter
   (or run `code-racer join FK72AD`, or type `:join FK72AD`).
4. Everyone presses `r` to get ready. The host presses `s`.
5. The server announces the text and starts the countdown; everyone starts on the server's signal.
6. Standings update live. When everyone has finished, left, or the race times out, results appear.
7. The host presses `r` to go back to the lobby for another race. `Esc` leaves the room (twice during
   a race, so a stray Esc never costs you a race).

The server is the authority: it chooses the text, starts the race, measures every finishing time,
validates progress reports and computes speeds. A client cannot declare itself finished.

## Running your own server

```bash
code-racer-server                                  # 127.0.0.1:8080
code-racer-server --host 0.0.0.0 --port 8080       # reachable from the LAN
code-racer-server --max-players 8 --room-ttl 1800 --race-timeout 300 --countdown 3
code-racer-server --max-connections-per-address 0  # behind a proxy that hides client addresses
RUST_LOG=debug code-racer-server                   # more logs, on stderr
```

| Option           | Default     | Meaning                                         |
| ---------------- | ----------- | ----------------------------------------------- |
| `--host`         | `127.0.0.1` | address to listen on, `0.0.0.0` for the LAN     |
| `--port`         | `8080`      | TCP port                                        |
| `--max-players`  | `8`         | players per room                                |
| `--room-ttl`     | `1800`      | seconds of inactivity before a room is closed   |
| `--race-timeout` | `300`       | seconds after which a race ends anyway          |
| `--countdown`    | `3`         | seconds between the start and the first keystroke |
| `--max-connections-per-address` | `16` | connections one IP may hold, `0` for no limit |

Rooms live in memory. Lobby players who disconnect are removed; racers who disconnect are shown
offline and the race goes on. When the host leaves, the next player becomes host.

### Docker

```bash
docker build -t code-racer-server .
docker run -p 8080:8080 code-racer-server
```

The image is a two-stage build that runs the server as an unprivileged user. CI publishes it to
`ghcr.io/klugko/code-racer-server` on every push to `main`.

## LAN multiplayer

On the machine that hosts the server (here `192.168.1.42`):

```bash
code-racer-server --host 0.0.0.0 --port 8080
sudo firewall-cmd --add-port=8080/tcp      # Fedora, if the firewall is on
```

On every player's machine:

```bash
code-racer --server ws://192.168.1.42:8080
```

or set it once with `:set server=ws://192.168.1.42:8080` (saved in `config.toml`). `ws://` is fine on
a trusted LAN. The client also speaks `wss://`, so the server can later sit behind a TLS reverse proxy
such as `wss://race.example.com`.

## Keybindings

| Where                  | Keys                    | Action                                     |
| ---------------------- | ----------------------- | ------------------------------------------ |
| everywhere             | `Ctrl+C`                | quit, the terminal is restored             |
|                        | `Ctrl+B`                | show or hide the explorer                  |
| normal mode            | `:`                     | command line (Tab completes)               |
|                        | `?`                     | help                                       |
|                        | `s` / `m` / `c` / `q`   | solo session / race.toml / create room / quit |
|                        | `Tab`                   | switch between explorer and editor         |
| explorer               | `j` `k` `g` `G`, Enter  | move, open                                 |
| settings buffers       | `j` `k`, `h` `l`, Enter | move, change a value, select               |
|                        | `i`                     | edit a text value (Enter saves, Esc cancels) |
| typing                 | any key                 | type                                       |
|                        | Backspace, `Ctrl+W`     | delete a character, a word (also `Alt+Backspace`) |
|                        | Enter                   | new line (code is auto-indented)           |
|                        | `Ctrl+R`, `Esc` `Esc`   | restart, abandon                           |
| results                | `r` / `e` / `Esc`       | new text / settings / close                |
| room                   | `r` or Space, `s`, `Esc`| ready, start (host), leave                 |
| history, help          | `j` `k`, `Ctrl+D` `Ctrl+U`, `g` `G` | scroll                         |

### Commands

| Command                         | Effect                                           |
| ------------------------------- | ------------------------------------------------ |
| `:solo`                         | start a session with the current settings        |
| `:words 50`, `:time 60`         | words or time session                            |
| `:quote`, `:code rust`          | quote or code session                            |
| `:lang french`, `:lang rust`    | language of words and quotes, or of code         |
| `:e path/to/file`               | practise on a file                               |
| `:set punctuation`, `:set nonumbers` | toggle options                              |
| `:set theme=mono`               | `editor`, `dark` or `mono`                       |
| `:set server=URL`, `:set username=NAME` | multiplayer settings                     |
| `:create`, `:join CODE`         | create or join a room                            |
| `:history`, `:config`, `:race`, `:help` | open a buffer                            |
| `:q`                            | quit                                             |

## Configuration

`config.toml` lives in the platform configuration directory (`~/.config/code-racer/` on Linux,
`%APPDATA%\code-racer\config\` on Windows). Every key is optional:

```toml
username = "Jean"
theme = "editor"          # editor, dark, mono
default_mode = "words"    # words, time, quote, code
language = "french"       # english, french
code_language = "rust"    # rust, python, typescript, javascript, sql
word_count = 50
duration = 30             # seconds, time mode
punctuation = false
numbers = false

[race]                    # the race.toml settings, used by `create` too
default_mode = "words"    # words, quote, code
language = "english"
word_count = 25           # 5 to 200

[multiplayer]
server = "ws://127.0.0.1:8080"
```

If `config.toml` cannot be read, code-racer starts with the defaults and leaves the file untouched
for the whole run.

Settings changed inside the app update `config.toml` in place, so your comments and unknown keys
survive. Only the settings you changed are written, onto what the file holds at that moment, so an
edit made by hand or by another running instance is kept. History is kept in `history.json` (`~/.local/share/code-racer/` on Linux) and appended to
under a lock, so two running instances never lose each other's results. Logs go to `code-racer.log`
(`~/.local/state/code-racer/`), never to the terminal. Files are written atomically; a file that cannot
be parsed is moved aside to a `.bak` file with a warning, and one that cannot be read is left alone.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace
python3 tests/e2e/solo.py          # Linux, after the release build
python3 tests/e2e/multiplayer.py
cargo test preview_screens -- --ignored --nocapture   # print every screen
```

## Architecture

```text
Cargo.toml          workspace + the code-racer client package
src/                client
  main.rs           startup: command line, files, logging
  runtime.rs        event loop: keyboard, network and clock events, redraw on change
  terminal.rs       raw mode, alternate screen, restoration on exit and panic
  app/              state machine, independent of rendering
    mod.rs          App: buffers, focus, modes, viewport, what the screen needs
    keys.rs         key handling by context      actions.rs        what users can do
    events.rs       network, clock, paste, quiet period after typing
    practice.rs     solo plans and sessions      race.rs           room client and race rules
    text_settings.rs  lines shared by practice.toml and race.toml
    changes.rs      settings changes and saving  saved_config.rs   saved vs run-only settings
    command.rs      `:` commands                 form.rs, input.rs, settings.rs, help.rs, history_log.rs
  ui/               rendering only
    chrome.rs       explorer, tab line, status line, command line
    editor.rs       buffer with line numbers     typing.rs         ghost text and typed text
    views/          practice/race/config forms, session, room, history, help
    format.rs, wrap.rs, syntax.rs, chart.rs, theme.rs
  config.rs, history.rs, persist.rs, cli.rs, network.rs, logging.rs
crates/engine       texts and typing, no terminal, no network
  corpus/           word lists, quotes, code snippets (embedded at compile time)
  language.rs       natural and programming languages, one table of names each
  normalize.rs      turns any text into characters a keyboard can type
  words.rs          sentence-aware word generator (punctuation, numbers)
  text.rs           TextSource: words, quote or code, seeded
  session.rs        TypingSession: graphemes, mistakes, auto-indent, timing
  indentation.rs    which characters auto-indent fills in
  stats.rs          Tally, WPM, accuracy, consistency, per-second samples
crates/protocol     WebSocket messages, validated RoomCode and Username, room views and ranking
crates/server       race server
  room.rs           pure room state machine (lobby, countdown, race, results)
  hub.rs            single task owning every room, no locks
  connection.rs     one task per socket: handshake, limits, rate limiting
  peers.rs          connections held by each address
tests/e2e/          real binaries in pseudo-terminals
```

Design choices:

- The client is event-driven: it sleeps until a key, a network message or (only while a session runs)
  a 100 ms clock tick arrives, and redraws only after a change.
- The typing engine and the text generator are plain Rust with injected time, so they are fully
  tested without a terminal.
- The protocol is versioned (`Hello` / `Welcome` handshake, currently version 3), tagged JSON, and
  identifiers are validated when they are decoded. Messages are limited to 16 KiB, and a test proves
  that the largest possible room view still fits.
- The server keeps every room in one hub task fed by channels, so no lock is ever held across an
  `.await`. Room logic is a pure state machine tested with simulated time.
- Anti-cheat basics: progress cannot exceed the text, counters cannot go backwards, speed above about
  360 WPM is refused, finishing times are measured by the server, and speed and accuracy are computed
  by the server with the same engine code as the client.

## Testing

- **engine**: WPM, raw WPM, accuracy and consistency formulas, zero division, graphemes, combining
  accents, emoji, NFC, auto-indentation, mistake blocking, time limits, the word generator's sentences,
  punctuation, numbers, French spacing, determinism, corpus integrity.
- **protocol**: room codes, usernames, every message round trip, wire format, ranking.
- **server**: room state machine (every rule, with simulated time), hub routing and broadcasting, and
  WebSocket integration tests with real sockets (full race, version mismatch, invalid messages, rate
  limiting, disconnections).
- **client**: state machine driven by key presses, commands, forms, configuration and history files,
  command line parsing, network client, rendering of every screen in every theme and size, and two
  complete clients racing through a real server.
- **end to end**: the release binaries in pseudo-terminals: first launch, typing, results, history,
  resize, Ctrl+C, SIGTERM and SIGHUP restoring the terminal, and a two-player race against a real
  server.

CI runs formatting, Clippy, tests and release builds on Linux and Windows, the test suite with
Fedora's own Rust packages, a check with the minimum supported Rust version, the end-to-end tests on
Linux, then builds, smoke-tests and publishes the server image.

## Roadmap

- Reconnecting to a running race after a network drop.
- More languages (Malagasy, German, Spanish): one word list, one quote file and one enum variant each.
- Spectators, replays and a local leaderboard.
- Custom word lists and themes from files.
