# code-racer

[![CI](https://github.com/klugko/ShadowType/actions/workflows/ci.yml/badge.svg)](https://github.com/klugko/ShadowType/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A typing trainer that looks like your code editor, with private races against your teammates on
the local network.

Most typing trainers look like a game. This one looks like a terminal editor: an explorer, tabs,
line numbers, a status line and a Vim command line. The file you seem to be editing is the text you
are typing. Everything ahead of the cursor is ghost text, and what you type turns into real,
syntax-highlighted code. Press `F12` and even the speed counter goes away.

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

When the team wants to know who types fastest, someone starts the server and everyone joins the
same room. The standings sit under the text, where an IDE keeps its terminal panel:

```text
                       │  1  Simplicity is prerequisite for reliability.
                       │ PLAYERS · FK72AD ──────────────────────────────────────────────
                       │  1 jean          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100%   92 wpm  ✓ 0:41.2
                       │  2 alice         ━━━━━━━━━━━━━━━━━━━━━───────  75%   87 wpm
                       │  3 bob           ━━━━━━━━━───────────────────  33%   79 wpm  offline
```

## Installing

code-racer is built from source. You need Rust 1.88 or newer and a C toolchain, which `ring`, the
cryptography behind `wss://`, compiles against.

On Fedora, `sudo dnf install rust cargo gcc` is enough. On Ubuntu or Debian, install
`build-essential`, on macOS run `xcode-select --install`, then get Rust from
[rustup.rs](https://rustup.rs). On Windows, rustup sets up the MSVC build tools for you; use
Windows Terminal to play. Then, from a clone of this repository:

```bash
cargo install --path .               # code-racer, the client
cargo install --path crates/server   # code-racer-server, only needed to host races
```

Both land in `~/.cargo/bin`. To try it without installing anything, use `cargo run`.

## Practising on your own

Run `code-racer`. The first time, it asks for the name other racers will see; `Esc` puts that off
until your first race. Then open `practice.toml` in the explorer, or press `s` to start straight
away with your last settings. In the settings buffers, `j` and `k` move between lines, `h` and `l`
change a value, and Enter on `▶ start session` starts. A mouse click works too. You can type:

- **words**: 10, 25, 50 or 100 of the thousand most common English or French words. Turn on
  punctuation for capitals, commas, quotes, parentheses and French spacing, and numbers for
  integers, years and decimals.
- **time**: as many words as you can in 15, 30, 60 or 120 seconds.
- **quote**: a public-domain passage, with its author.
- **code**: one of more than a hundred real snippets in Rust, Python, TypeScript, JavaScript or SQL.
  Enter moves to the next line and fills in the indentation for you.
- **a file of your own**: `code-racer solo --file src/lib.rs`, or `:e src/lib.rs` from inside.

Prose does not have to look like prose. The `look` setting dresses it up as a file you could
plausibly have open at work:

| Look      | Shown as         | What it adds                                                 |
| --------- | ---------------- | ------------------------------------------------------------ |
| `notes`   | `notes.md`       | nothing, the text as it comes                                |
| `todo`    | `TODO.md`        | a checkbox before each row, ticked once the row is typed     |
| `commit`  | `COMMIT_EDITMSG` | git's template under the message                             |
| `docs`    | `lib.rs`, …      | comment markers and a function, in your code language        |
| `log`     | `server.log`     | a timestamp and a level before each row, stamped as you type |
| `mail`    | `draft.eml`      | headers, a greeting and your name                            |
| `shuffle` |                  | another of these for every text                              |

The clock starts on your first keystroke. A session ends once the whole text is typed correctly,
or when the timer runs out in time mode. Mistakes stay in your statistics even after you fix them,
and you have to fix them to finish: after 10 characters typed past an uncorrected mistake, input
stops until you go back. Characters are compared as graphemes after Unicode normalisation, so an
`é` typed with a dead key and one typed as `e` plus a combining accent count the same.

`Ctrl+R` restarts with a new text. Once you have started typing, abandoning takes `Esc` twice, and
keys typed in the instant after a text ends are ignored, so a stray key never costs you anything.

Every result shows your speed, accuracy, a chart of your speed second by second, the characters you
missed most and how the session compares with your recent average. `code-racer history` keeps your
personal records, today's practice and your streak of days. Solo sessions and races are scored
the same way, and indentation filled in for you counts towards progress, never towards speed:

| Metric      | How it is computed                                     |
| ----------- | ------------------------------------------------------ |
| WPM         | correctly typed characters / 5 / minutes elapsed       |
| raw WPM     | every keystroke / 5 / minutes elapsed                  |
| accuracy    | correct keystrokes / all keystrokes × 100              |
| errors      | wrong keystrokes, whether you fixed them or not        |
| consistency | how steady your speed was from one second to the next  |

## Racing your team

1. Someone runs a race server, often on their own machine (see [below](#hosting-a-race)).
2. The host opens `race.toml` (`m`), picks the text under `[create]` and selects `▶ create room`,
   or presses `c`. A room code such as `FK72AD` appears, with the command teammates should run,
   pointing at the host's LAN address rather than at their own `127.0.0.1`.
3. Teammates type the code on the `room` line of `race.toml`, or run `code-racer join FK72AD`.
4. Everyone presses `r` when ready, then the host presses `s`.
5. The server sends the text and counts down. Standings update live while you race.
6. Once everyone has finished or left, or the race times out, the results appear and the host
   presses `r` for another round.

`Esc` leaves the room. From the countdown to the results it has to be pressed twice, so a stray
key never costs you a race. If the host leaves, the next player to have joined takes over.

The server is the referee. It picks the text, starts the race, times every finish and recomputes
speed and accuracy from the counters each client reports. A client cannot declare itself finished,
type faster than about 360 WPM or take back a keystroke it already reported.

### Hosting a race

On the machine that hosts it, here `192.168.1.42`:

```bash
code-racer-server --host 0.0.0.0     # listens on port 8080; open it in your firewall if needed
```

Players either pass `--server ws://192.168.1.42:8080` or save it once with `:set server=...`.
Room sizes, timeouts, Docker and the server's limits are in the [server's README](crates/server/README.md).

## The editor around it

There are four themes: `editor`, the default, in true colour; `vscode`, with the colours of VS
Code's Dark+ and its blue status bar; `dark`, the 16 ANSI colours on black; and `mono`. Files in
the explorer and the tabs get icons, drawn with symbols every font has, or with a Nerd Font if you
set `icons = "nerd"`. Code is highlighted before you type it, dimmed, and lights up as you go, with
brackets coloured by depth and indentation guides.

A small pixel-art ghost lives under the explorer. It floats about, types along with you, startles at
mistakes, cheers when you finish and naps after 45 seconds without a key. The cursor leaves a short
trail as it moves, like the smooth cursor of a modern editor. All of this can be switched off.

Discreet mode (`F12`) goes further: the status line shows `Ln, Col`, `UTF-8` and `LF` instead of
your speed, the records and the mascot disappear, and the explorer is named after the directory you
are in. Below 80×20 you get a clear message rather than a broken layout, and the terminal is
restored however the app exits, panics included.

## Keys

| Where             | Keys                                | Does                                            |
| ----------------- | ----------------------------------- | ----------------------------------------------- |
| anywhere          | `Ctrl+P` or `F1`                    | command palette: type a few letters of an action |
|                   | `Ctrl+B`                            | show or hide the explorer                       |
|                   | `F12`                               | discreet mode: an editor and nothing else       |
|                   | `Ctrl+C`                            | quit, leaving the terminal as it was            |
| normal mode       | `:`, `?`                            | command line (Tab completes), help              |
|                   | `s`, `m`, `c`, `q`                  | solo session, race.toml, create a room, quit    |
|                   | `Tab`                               | switch between the explorer and the editor      |
| explorer          | `j` `k` `g` `G`, Enter              | move, open                                      |
| settings buffers  | `j` `k`, `h` `l`, Enter, `i`        | move, change a value, select, edit a text value |
| typing            | Backspace, `Ctrl+W`, `Alt+Backspace` | delete a character, delete a word              |
|                   | `Ctrl+R`, `Esc` `Esc`               | restart, abandon                                |
| results           | `r`, `e`, `Esc`                     | new text, settings, close                       |
| room              | `r` or Space, `s`, `Esc`            | ready, start (host), leave                      |
| history, help     | `j` `k`, `Ctrl+D` `Ctrl+U`, `g` `G` | scroll                                          |

Files, tabs, settings and on-screen keys can be clicked; `:set nomouse` leaves the mouse alone.

## Commands

| Command                                  | Does                                                  |
| ---------------------------------------- | ----------------------------------------------------- |
| `:solo`, `:words 50`, `:time 60`         | start a session                                       |
| `:quote`, `:code rust`                   | start a quote or code session                         |
| `:lang french`, `:lang rust`             | language of words and quotes, or of code              |
| `:e path/to/file`                        | practise on a file                                    |
| `:set punctuation`, `:set nonumbers`     | turn an option on or off                              |
| `:set look=commit`                       | `notes`, `todo`, `commit`, `docs`, `log`, `mail`, `shuffle` |
| `:set theme=vscode`                      | `editor`, `dark`, `mono` or `vscode`                  |
| `:set icons=nerd`                        | `unicode`, `nerd` (needs a Nerd Font) or `none`       |
| `:set nomascot`, `notrail`, `noanimations`, `discreet` | interface options                       |
| `:set server=URL`, `:set username=NAME`  | multiplayer settings                                  |
| `:create`, `:join CODE`                  | create or join a room                                 |
| `:history`, `:config`, `:race`, `:help`  | open a buffer                                         |
| `:q`                                     | quit                                                  |

Most of it works from the shell too:

```bash
code-racer solo --mode words --words 25 --punctuation --numbers
code-racer solo --mode time --seconds 60 --language french
code-racer --server ws://192.168.1.42:8080 join FK72AD
```

Flags apply to that run only, and never reach `config.toml` unless you change the setting in-app.

## Configuration

Everything you change inside the app is saved to `config.toml`, and every key is optional:

```toml
username = "Jean"
theme = "editor"          # editor, dark, mono, vscode
icons = "unicode"         # unicode, nerd, none
look = "notes"            # notes, todo, commit, docs, log, mail, shuffle
mascot = true             # the little ghost under the explorer
animations = true         # cursor trail and breathing, mascot, results counting up
trail = true              # false lets typed text glow instead of the cursor trailing
mouse = true
discreet = false          # what F12 toggles
default_mode = "words"    # words, time, quote, code
language = "english"      # english, french
code_language = "rust"    # rust, python, typescript, javascript, sql
word_count = 50
duration = 30             # seconds, in time mode
punctuation = false
numbers = false

[race]                    # what race.toml and `code-racer create` start from
default_mode = "words"    # words, quote, code
language = "english"
word_count = 25           # 5 to 200

[multiplayer]
server = "ws://127.0.0.1:8080"
```

| File             | Linux                          | Windows                         |
| ---------------- | ------------------------------ | ------------------------------- |
| `config.toml`    | `~/.config/code-racer/`        | `%APPDATA%\code-racer\config\`  |
| `history.json`   | `~/.local/share/code-racer/`   | `%LOCALAPPDATA%\code-racer\data\` |
| `code-racer.log` | `~/.local/state/code-racer/`   | next to `history.json`          |

The app edits `config.toml` in place and only writes the settings you changed, so comments,
unknown keys and edits made in the meantime survive. History is appended under a file lock, so two
instances never lose each other's results. Files are written atomically; one that cannot be parsed
is moved aside to a `.bak` with a warning, and one that cannot be read is left alone.

## How it is built

A Cargo workspace: the client at the root, three library crates beside it.

| Crate                                        | What it does                                                         |
| -------------------------------------------- | -------------------------------------------------------------------- |
| `code-racer` (`src/`)                        | the terminal client: event loop, app state, rendering, files, network |
| [`code-racer-engine`](crates/engine)         | texts to type, the typing session and its statistics                 |
| [`code-racer-protocol`](crates/protocol)     | the messages client and server exchange over WebSocket               |
| [`code-racer-server`](crates/server)         | the race server                                                      |

A few decisions shape the code:

- **Nothing runs while nothing happens.** The client sleeps until a key, a network message or,
  during a session, a 100 ms tick. It redraws only after a change, or every 40 to 100 ms while
  something moves, so it can sit open all day without using any CPU.
- **State and drawing are separate.** `src/app` holds the state and never draws; `src/ui` draws and
  never changes it. Every screen can be rendered in a test, in every theme and at every size.
- **The engine never reads the clock.** Time is passed in, so typing rules and statistics are
  tested to the millisecond, and the server scores races with exactly the same code as the client.
- **One task owns every room.** Connections talk to it over channels, so the server never holds a
  lock across an `.await`, and the room rules are a plain state machine tested with simulated time.

## Working on it

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace && python3 tests/e2e/solo.py && python3 tests/e2e/multiplayer.py   # Linux
cargo test preview_screens -- --ignored --nocapture   # print every screen to your terminal
```

The tests go from pure functions up to real binaries: every room rule with simulated time,
WebSocket tests over real sockets, every screen in every theme, two clients racing through a real
server, and the release binaries driven through pseudo-terminals in `tests/e2e/`. CI runs them on
Linux and Windows, again with Fedora's own Rust packages, checks the minimum Rust version, then
builds, smoke-tests and publishes the server image from `main`.

## Roadmap

- Rejoining a running race after a network drop.
- More languages (Malagasy, German, Spanish), each just a word list, a quote file and an enum
  variant ([how](crates/engine/README.md#adding-a-language)).
- Spectators, replays and a local leaderboard.
- Custom word lists and themes loaded from files.

## License

[MIT](LICENSE)
