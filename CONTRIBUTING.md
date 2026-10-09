# Contributing

See [docs/architecture.md](docs/architecture.md) for how the workspace fits together.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace
python3 tests/e2e/solo.py                               # Linux, after the release build
python3 tests/e2e/multiplayer.py
cargo test preview_screens -- --ignored --nocapture     # print every screen to the terminal
```

Lints are strict and shared by the workspace: `unsafe_code` is forbidden, and `unwrap`, `dbg!` and
`todo!` are flagged. CI treats every warning as an error.

## Testing strategy

About 640 tests, from pure functions up to real binaries:

| Layer        | What is covered |
| ------------ | --------------- |
| **engine**   | WPM, raw WPM, accuracy and consistency formulas, zero division, graphemes, combining accents, emoji, NFC, auto-indentation, mistake blocking, time limits, sentence generation, punctuation, numbers, French spacing, determinism, corpus integrity |
| **protocol** | room codes, usernames, round trip of every message, wire format, ranking, maximum message size |
| **server**   | every room rule with simulated time, hub routing and broadcasting, WebSocket integration over real sockets (full race, version mismatch, invalid messages, rate limiting, disconnections) |
| **client**   | state machine driven by key presses, commands, forms, config and history files, CLI parsing, network client, rendering of every screen in every theme and size, two full clients racing through a real server |
| **end to end** | release binaries in pseudo-terminals: first launch, typing, results, history, resize, `Ctrl+C`, `SIGTERM` and `SIGHUP` restoring the terminal, a two-player race against a real server |

## Continuous integration

Every push and pull request runs, in [`ci.yml`](.github/workflows/ci.yml):

1. **Quality** on Linux and Windows: format, Clippy, tests, release build, end-to-end tests (Linux),
   and the binaries uploaded as artifacts.
2. **Fedora**: the test suite and `cargo install` with Fedora's own Rust packages.
3. **MSRV**: `cargo check` with the `rust-version` declared in `Cargo.toml`.
4. **Container**: build, smoke-test, and on `main`, publish the server image to GHCR.

## Adding a language

A natural language takes a word list (one word per line) and a quote file (entries separated by
lines containing only `%`) in [`crates/engine/corpus/`](crates/engine/corpus), a variant of
`Language` in [`language.rs`](crates/engine/src/language.rs), and its entry in
[`corpus.rs`](crates/engine/src/corpus.rs). The `bundled_corpora_are_well_formed` test catches
malformed entries.

## Workflow

1. Branch from `main`.
2. Keep the four checks above green (`fmt`, `clippy`, `test`, release build).
3. Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `refactor:`…).
4. Comments explain *why*, not *what*.

