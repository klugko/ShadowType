//! Everything about typing that does not depend on a terminal or a network:
//! the texts to type, the session that compares keystrokes against them and
//! the statistics derived from it.

pub mod corpus;
pub mod language;
pub mod normalize;
pub mod session;
pub mod stats;
pub mod text;
pub mod words;

pub use language::{CodeLanguage, Language, UnknownLanguage};
pub use normalize::normalize;
pub use session::{
    ERROR_RUN_LIMIT, Mark, SessionOptions, Status, TypingSession, grapheme_count, graphemes,
};
pub use stats::{Sample, Stats, consistency, percentage, words_per_minute};
pub use text::{GeneratedText, TextSource, WORD_COUNTS};
pub use words::{WordOptions, WordStream};
