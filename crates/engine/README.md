# code-racer-engine

Everything about typing that needs neither a terminal nor a network: the texts to type, the session
that compares keystrokes with them, and the statistics that come out of it. The client runs it for
solo sessions and races, and the race server runs the very same code to score players, so a race
can never disagree with practice about what 80 WPM means.

The engine never reads the clock. Every call that depends on time takes an `Instant`, which makes
every rule deterministic and lets the tests step through a session to the millisecond.

```rust
use std::time::{Duration, Instant};

use code_racer_engine::{Language, SessionOptions, TextSource, TypingSession, WordOptions};

let source = TextSource::Words {
    language: Language::English,
    count: 25,
    options: WordOptions { punctuation: true, numbers: false },
};
let text = source.generate(42).text;

let start = Instant::now();
let mut session = TypingSession::new(&text, SessionOptions::default());
for (index, ch) in text.chars().enumerate() {
    session.type_char(ch, start + Duration::from_millis(150 * index as u64));
}

let stats = session.stats(start + Duration::from_secs(30));
println!("{:.0} wpm, {:.0}% accuracy", stats.wpm, stats.accuracy);
```

## Texts

A `TextSource` describes a text rather than holding it: so many words of a language, a quote, or a
code snippet. `generate(seed)` turns it into the actual text, and the same seed always gives the
same text. That is how the server can pick a seed and know exactly what every player is typing.

**Words** come from lists of about a thousand common words per language, most frequent first.
Without options they are just words. With punctuation, the stream is cut into sentences of 4 to 12
words: the first is capitalised, the last ends with a full stop, a question mark or an exclamation
mark, and the ones in between sometimes get a comma, quotes or parentheses. French gets its own
typography, with a space before `;`, `:`, `!` and `?`. With numbers, about one word in eight
becomes an integer, a year or a decimal, written with a comma in French.

**Quotes** are public-domain passages with their author and work. **Code** snippets are real-world
functions and queries in Rust, Python, TypeScript, JavaScript and SQL.

All of it lives in [`corpus/`](corpus) and is compiled into the binary with `include_str!`, so there
are no data files to install or lose. Word lists hold one word per line. Quotes and snippets are
separated by lines holding a single `%`, as in fortune files, and a quote can end with a `-- `
attribution line.

## Making any text typeable

Whatever the source, a file of the player's included, the text goes through `normalize` first, so
that every character in it can be typed on an ordinary keyboard:

- line endings become `\n`, tabs become spaces up to the next multiple of four columns;
- curly quotes and guillemets become `'` or `"`, dashes become `-`, odd spaces become a space,
  box drawing becomes `-`, `|` or `+`, and symbols such as `→` or `≤` are spelled `->` and `<=`;
- characters no key produces, such as zero-width spaces, bidirectional controls or the soft
  hyphen, are dropped, while letters of every script, accents and emoji stay;
- trailing whitespace and blank lines at either end go, and the result is in Unicode NFC.

## The typing session

`TypingSession` compares input with the text grapheme by grapheme: what the player sees as one
character is one character, whether it is `é`, a flag or a family emoji. Input is normalised the
same way, so an accent typed with a dead key and one typed as a combining mark both match.

A few rules make it behave like a careful editor rather than a game:

- Mistakes have to be fixed for the session to complete. After `ERROR_RUN_LIMIT` (10) characters
  typed from an uncorrected mistake, further input is refused until the player goes back.
- With `auto_indent`, Enter fills in the next line's leading spaces, as a code editor would. Those
  characters count towards progress, but never as keystrokes, so they cannot inflate speed or
  accuracy. `Indentation` tells the server which characters of a text are filled in this way.
- With a `time_limit`, the session ends when the time is up, however much was typed, and an accent
  still waiting for its letter at that moment counts as a mistake.

## Statistics

| Measure     | Definition                                                              |
| ----------- | ----------------------------------------------------------------------- |
| WPM         | characters typed correctly / 5 / minutes elapsed                        |
| raw WPM     | keystrokes / 5 / minutes elapsed                                        |
| accuracy    | keystrokes that matched / all keystrokes × 100                          |
| errors      | keystrokes that did not match, corrected or not                         |
| consistency | 100 × (1 − coefficient of variation of the per-second raw WPM), 0–100   |

With no time elapsed, speeds are 0 rather than infinite, and accuracy with no keystrokes is 100%.
`samples` gives one `Sample` per second of the session for the chart shown after it. A remainder
shorter than half a second joins the previous window, because a speed measured over a few
milliseconds means nothing.

A `Tally` holds the raw counters behind all this. It is also what a client sends the race server,
which recomputes the speeds itself rather than trusting any.

## Adding a language

A natural language needs a word list in `corpus/words/`, a quote file in `corpus/quotes/`, a
variant of `Language` with its names in [`language.rs`](src/language.rs), and the matching entries
in [`corpus.rs`](src/corpus.rs). If the language puts a space before high punctuation or writes
decimals with a comma, say so next to French in `language.rs`. A programming language works the
same way with a snippet file in `corpus/code/` and a variant of `CodeLanguage`.

The `bundled_corpora_are_well_formed` test then checks the new files: at least 200 words, none
with whitespace in it, at least one quote, and snippets with their tabs already expanded.
