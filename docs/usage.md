# Usage

## Installation

You need a stable Rust toolchain (1.88 or later) and a C compiler with its linker.

| Platform       | Prerequisites                                                                              |
| -------------- | ------------------------------------------------------------------------------------------ |
| Fedora         | `sudo dnf install rust cargo gcc`                                                          |
| Ubuntu, Debian | `sudo apt install build-essential`, then Rust from <https://rustup.rs>                     |
| macOS          | `xcode-select --install`, then Rust from <https://rustup.rs>                               |
| Windows        | Rust from <https://rustup.rs> with the MSVC build tools; use Windows Terminal              |

Then, from the repository:

```bash
cargo install --path .                 # code-racer, the client
cargo install --path crates/server     # code-racer-server, the race server
```

Both binaries land in `~/.cargo/bin`. To try without installing: `cargo run` (client) and
`cargo run -p code-racer-server` (server).

## Command line

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

On first launch code-racer asks for the name other racers will see (`Esc` skips it until you race).
It is saved in `config.toml` and can be changed later with `:set username=Jean`.

> [!NOTE]
> Command-line flags apply to the current run only. `--theme`, `--server` and the `solo` options are
> never written to `config.toml` unless you change the same setting inside the app.

## Solo sessions

Open `practice.toml` in the explorer, or press `s` anywhere to start immediately. `j`/`k` select a
setting, `h`/`l` change it, Enter on `▶ start session` starts. With the mouse, click a setting to
select it and click again to change it. Settings are remembered.

The `look` setting chooses what prose is dressed as:

| Look      | File             | What it adds                                                 |
| --------- | ---------------- | ------------------------------------------------------------ |
| `notes`   | `notes.md`       | nothing: the text as it comes                                |
| `todo`    | `TODO.md`        | a checkbox before each row, ticked once the row is typed     |
| `commit`  | `COMMIT_EDITMSG` | git's template under the message                             |
| `docs`    | `lib.rs`, …      | comment markers and a function, in the code language         |
| `log`     | `server.log`     | a timestamp and a level before each row, stamped as you type |
| `mail`    | `draft.eml`      | headers, a greeting and your name                            |
| `shuffle` |                  | another of these for every text                              |

Session rules:

- The clock starts on the first keystroke. A session ends when the whole text is typed correctly, or
  when the timer runs out in time mode.
- Mistakes count in your statistics even once corrected. After 10 characters past an uncorrected
  mistake, input stops until it is fixed.
- In code, Enter moves to the next line and fills in the indentation. Auto-filled indentation counts
  towards progress but never towards speed or accuracy, solo and in races.
- `Ctrl+R` restarts with a new text. Once you have started typing, `Esc` **twice** abandons, so a
  stray `Esc` costs nothing.
- Keys typed right after the end of a text are briefly ignored, so a word typed on the run is not
  interpreted as commands.

## Metrics

| Metric      | Definition                                                               |
| ----------- | ------------------------------------------------------------------------ |
| WPM         | correctly typed characters / 5 / elapsed minutes                         |
| raw WPM     | every keystroke / 5 / elapsed minutes                                    |
| accuracy    | correct keystrokes / all keystrokes × 100                                |
| errors      | wrong keystrokes, corrected or not                                       |
| consistency | 100 × (1 − coefficient of variation of per-second raw WPM), from 0 to 100 |

## Racing

1. Someone [runs a race server](server.md).
2. The host opens `race.toml` (`m`), picks the text under `[create]` and selects `▶ create room`
   (or presses `c`, or runs `code-racer create`). The room code appears, e.g. `FK72AD`, along with
   the exact command teammates should run. When the server runs on the host's machine, that command
   uses its LAN address rather than `127.0.0.1`.
3. Teammates enter the code in the `room` line of `race.toml` and press Enter
   (or run `code-racer join FK72AD`, or type `:join FK72AD`).
4. Everyone presses `r` to get ready; the host presses `s`.
5. The server announces the text and runs the countdown; everyone starts on its signal.
6. Standings update live. Results appear when everyone has finished or left, or the race times out.
7. The host presses `r` to return to the lobby. `Esc` leaves the room (twice between the countdown
   and the results, so a stray `Esc` never costs you a race).

Lobby players who disconnect are removed; racers who disconnect are shown `offline` and the race goes
on. When the host leaves, the next player becomes host.


## Reference

### Keybindings

| Where            | Keys                                | Action                                            |
| ---------------- | ----------------------------------- | ------------------------------------------------- |
| everywhere       | `Ctrl+P`, `F1`                      | command palette                                   |
|                  | `Ctrl+C`                            | quit, the terminal is restored                    |
|                  | `Ctrl+B`                            | show or hide the explorer                         |
|                  | `F12`                               | discreet mode on or off                           |
| command palette  | letters, `↑` `↓`, Enter             | find, select, run (`Esc` closes)                  |
| mouse            | click, wheel                        | open, select, press the key shown; scroll         |
| normal mode      | `:`                                 | command line (Tab completes)                      |
|                  | `?`                                 | help                                              |
|                  | `s` / `m` / `c` / `q`               | solo session / race.toml / create room / quit     |
|                  | `Tab`                               | switch between explorer and editor                |
| explorer         | `j` `k` `g` `G`, Enter              | move, open                                        |
| settings buffers | `j` `k`, `h` `l`, Enter             | move, change a value, select                      |
|                  | `i`                                 | edit a text value (Enter saves, `Esc` cancels)    |
| typing           | any key                             | type                                              |
|                  | Backspace, `Ctrl+W`                 | delete a character, a word (also `Alt+Backspace`) |
|                  | Enter                               | new line (code is auto-indented)                  |
|                  | `Ctrl+R`, `Esc` `Esc`               | restart, abandon                                  |
| results          | `r` / `e` / `Esc`                   | new text / settings / close                       |
| room             | `r` or Space, `s`, `Esc`            | ready, start (host), leave (twice in a race)      |
| history, help    | `j` `k`, `Ctrl+D` `Ctrl+U`, `g` `G` | scroll                                            |


### Commands

| Command                                                                               | Effect                                                      |
| ------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `:solo`                                                                               | start a session with the current settings                   |
| `:words 50`, `:time 60`                                                               | words or time session                                       |
| `:quote`, `:code rust`                                                                | quote or code session                                       |
| `:lang french`, `:lang rust`                                                          | language of words and quotes, or of code                    |
| `:e path/to/file`                                                                     | practise on a file                                          |
| `:set punctuation`, `:set nonumbers`                                                  | toggle options                                              |
| `:set look=commit`                                                                    | `notes`, `todo`, `commit`, `docs`, `log`, `mail`, `shuffle` |
| `:set theme=vscode`                                                                   | `editor`, `dark`, `mono` or `vscode`                        |
| `:set icons=nerd`                                                                     | file icons: `unicode`, `nerd` or `none`                     |
| `:set nomascot`, `:set notrail`, `:set noanimations`, `:set nomouse`, `:set discreet` | interface options                                           |
| `:set server=URL`, `:set username=NAME`                                               | multiplayer settings                                        |
| `:create`, `:join CODE`                                                               | create or join a room                                       |
| `:history`, `:config`, `:race`, `:help`                                               | open a buffer                                               |
| `:q`                                                                                  | quit                                                        |


### Configuration (`config.toml`)

Every key is optional; missing keys take the defaults.

```toml
username = "Jean"
theme = "editor"          # editor, dark, mono, vscode
icons = "unicode"         # unicode, nerd (needs a Nerd Font), none
look = "notes"            # notes, todo, commit, docs, log, mail, shuffle
mascot = true             # the ghost under the explorer
animations = true         # cursor trail, cursor breathing, mascot, results counting up
trail = true              # the cursor's trail; false lets typed text glow instead
mouse = true              # false leaves the mouse to the terminal, to select text
discreet = false          # true: an editor and nothing else (F12)
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


### Files and data

| File             | Linux                        | Windows                           | Purpose                              |
| ---------------- | ---------------------------- | --------------------------------- | ------------------------------------ |
| `config.toml`    | `~/.config/code-racer/`      | `%APPDATA%\code-racer\config\`    | settings                             |
| `history.json`   | `~/.local/share/code-racer/` | `%LOCALAPPDATA%\code-racer\data\` | results of every session             |
| `code-racer.log` | `~/.local/state/code-racer/` | `%LOCALAPPDATA%\code-racer\data\` | logs (never written to the terminal) |

macOS keeps all three in `~/Library/Application Support/code-racer/`.

How these files are treated:

- **Edits are surgical.** Settings changed in the app update `config.toml` in place: comments and
  unknown keys survive, and only the changed keys are written onto what the file holds at that
  moment, so hand edits and other running instances are not overwritten.
- **History is concurrency-safe.** It is appended under a file lock, so two instances never lose each
  other's results.
- **Writes are atomic.** A file that cannot be parsed is moved aside to a `.bak` file with a warning;
  one that cannot be read is left untouched and code-racer runs on defaults.


## Troubleshooting

| Symptom | Fix |
| --- | --- |
| Teammates cannot connect | Start the server with `--host 0.0.0.0`, open the port in the firewall, and give them the host's LAN address, not `127.0.0.1`. |
| "server speaks protocol vX, client vY" | Client and server come from different versions; rebuild both from the same commit. |
| Icons show as boxes or question marks | Use `:set icons=unicode`, or install a Nerd Font and select it in your terminal. |
| You cannot select text with the mouse | `:set nomouse` leaves the mouse to the terminal. |
| Colours look wrong | Your terminal may lack true colour: try `:set theme=dark` or `:set theme=mono`. |
| Settings were reset | `config.toml` could not be parsed; the original is next to it as a `.bak` file. |
| Something else | Check `code-racer.log`, or run the server with `RUST_LOG=debug`. |

