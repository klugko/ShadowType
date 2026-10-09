# code-racer

[![CI](https://github.com/klugko/ShadowType/actions/workflows/ci.yml/badge.svg)](https://github.com/klugko/ShadowType/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](Cargo.toml)

**Typing practice that looks like your code editor, with private races against your team on the LAN.**

code-racer is a terminal typing trainer dressed as a code editor: an explorer, tabs, line numbers, a
status line and a Vim command line. The file you are "editing" is the text you are typing. What is
ahead of the cursor is ghost text; what you have typed becomes real, syntax-highlighted code.

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

During a race, live standings sit in a panel under the text:

```text
                       │  1  Simplicity is prerequisite for reliability.
                       │ PLAYERS · FK72AD ──────────────────────────────────────────────
                       │  1 jean          ━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100%   92 wpm  ✓ 0:41.2
                       │  2 alice         ━━━━━━━━━━━━━━━━━━━━━───────  75%   87 wpm
                       │  3 bob           ━━━━━━━━━───────────────────  33%   79 wpm  offline
```

## Features

- **Practice** on words, timed runs, quotes, 100+ real snippets in Rust, Python, TypeScript,
  JavaScript and SQL, or your own files. English and French, with optional punctuation and numbers.
- **Accurate engine**: Unicode graphemes with NFC normalisation, auto-indentation in code, and WPM,
  accuracy, consistency, a speed chart, records and streaks.
- **LAN races** in private rooms (`FK72AD`) with live standings. The server is authoritative: it
  times every racer and computes every score.
- **Looks like an editor**: Vim modes, `:` commands, a command palette, mouse support, four themes,
  Nerd Font icons, and a discreet mode (`F12`) that hides anything that isn't an editor.
- **Good terminal citizen**: no CPU use while idle, and the terminal is always restored, even on
  panic or `SIGTERM`.

## Quick start

Requires Rust **1.88+** and a C linker.

```bash
git clone https://github.com/klugko/ShadowType.git && cd ShadowType
cargo install --path .                 # code-racer, the client
code-racer solo                        # start typing
```

To race, one person runs the server and everyone points at it:

```bash
cargo install --path crates/server
code-racer-server --host 0.0.0.0                       # on the host, e.g. 192.168.1.42
code-racer --server ws://192.168.1.42:8080 create      # host: prints a room code
code-racer --server ws://192.168.1.42:8080 join FK72AD # teammates
```

Or run the server with Docker: `docker run -p 8080:8080 ghcr.io/klugko/code-racer-server`.

Press `?` in the app for help, or `Ctrl+P` for the command palette.

## Documentation

| Guide                                       | Contents                                                        |
| ------------------------------------------- | --------------------------------------------------------------- |
| [Usage](docs/usage.md)                      | CLI, solo sessions, metrics, races, keys, commands, config, troubleshooting |
| [Race server](docs/server.md)               | options, LAN setup, Docker, TLS, hardening                      |
| [Architecture](docs/architecture.md)        | crates, data flow, design decisions and their trade-offs        |
| [Contributing](CONTRIBUTING.md)             | checks, testing strategy, CI, adding a language                 |

## Roadmap

- [ ] Reconnect to a running race after a network drop
- [ ] More languages (Malagasy, German, Spanish)
- [ ] Spectators, replays and a local leaderboard
- [ ] Custom word lists and themes loaded from files

## License

[MIT](LICENSE) © 2026 klugko
