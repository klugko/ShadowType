use std::ops::RangeInclusive;

use clap::ValueEnum;
use code_racer_engine::{CodeLanguage, Language, TextSource, WORD_COUNTS, WordOptions};
use code_racer_protocol::RACE_WORD_COUNTS;
use serde::{Deserialize, Serialize};

use super::adopt;

/// What a practice session asks to type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A fixed number of common words.
    #[default]
    Words,
    /// As many words as possible before the timer runs out, solo only.
    Time,
    /// A passage of literature.
    Quote,
    /// A snippet of source code.
    Code,
}

impl Mode {
    pub const ALL: [Self; 4] = [Self::Words, Self::Time, Self::Quote, Self::Code];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Words => "words",
            Self::Time => "time",
            Self::Quote => "quote",
            Self::Code => "code",
        }
    }
}

/// Settings of the last solo session or race, reused as the next defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Practice {
    #[serde(rename = "default_mode")]
    pub mode: Mode,
    pub language: Language,
    pub code_language: CodeLanguage,
    pub word_count: u16,
    /// Length of a time session, in seconds.
    pub duration: u16,
    pub punctuation: bool,
    pub numbers: bool,
}

impl Practice {
    /**
     * Word counts offered by the forms, solo and in races. Other counts in
     * [`WORD_COUNTS`], or [`RACE_WORD_COUNTS`] for races, can be set by hand.
     */
    pub const WORD_COUNT_PRESETS: [u16; 4] = [10, 25, 50, 100];
    /// Time session lengths offered by the forms, in seconds.
    pub const DURATION_PRESETS: [u16; 4] = [15, 30, 60, 120];
    /// Accepted time session lengths, in seconds.
    pub const DURATION_LIMITS: RangeInclusive<u16> = 5..=600;

    /// Brings hand-edited counts back into the supported ranges.
    pub fn sanitized(self) -> Self {
        Self {
            word_count: clamp_to(self.word_count, &WORD_COUNTS),
            duration: clamp_to(self.duration, &Self::DURATION_LIMITS),
            ..self
        }
    }

    /**
     * The same settings for a race: time mode becomes words mode, which
     * races can use, and the word count is brought into [`RACE_WORD_COUNTS`].
     */
    pub fn for_race(self) -> Self {
        Self {
            mode: match self.mode {
                Mode::Time => Mode::Words,
                mode => mode,
            },
            word_count: clamp_to(self.word_count, &RACE_WORD_COUNTS),
            ..self
        }
    }

    /**
     * The finite text these settings describe, `None` in time mode where
     * words keep coming until the timer runs out.
     */
    pub fn text_source(&self) -> Option<TextSource> {
        (self.mode != Mode::Time).then(|| self.text_in(self.mode))
    }

    /**
     * The text of a race created with these settings, which
     * [`Practice::for_race`] makes raceable.
     */
    pub fn race_text_source(self) -> TextSource {
        let race = self.for_race();
        race.text_in(race.mode)
    }

    /// The text of `mode`, where a timed session types words.
    fn text_in(&self, mode: Mode) -> TextSource {
        match mode {
            Mode::Words | Mode::Time => TextSource::Words {
                language: self.language,
                count: self.word_count,
                options: self.word_options(),
            },
            Mode::Quote => TextSource::Quote {
                language: self.language,
            },
            Mode::Code => TextSource::Code {
                language: self.code_language,
            },
        }
    }

    pub const fn word_options(&self) -> WordOptions {
        WordOptions {
            punctuation: self.punctuation,
            numbers: self.numbers,
        }
    }

    /// See [`Config::adopt_changes`](super::Config::adopt_changes).
    pub(super) fn adopt_changes(&mut self, before: &Self, after: &Self) {
        let Self {
            mode,
            language,
            code_language,
            word_count,
            duration,
            punctuation,
            numbers,
        } = after;
        adopt(&mut self.mode, &before.mode, mode);
        adopt(&mut self.language, &before.language, language);
        adopt(
            &mut self.code_language,
            &before.code_language,
            code_language,
        );
        adopt(&mut self.word_count, &before.word_count, word_count);
        adopt(&mut self.duration, &before.duration, duration);
        adopt(&mut self.punctuation, &before.punctuation, punctuation);
        adopt(&mut self.numbers, &before.numbers, numbers);
    }
}

impl Default for Practice {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            language: Language::default(),
            code_language: CodeLanguage::default(),
            word_count: 50,
            duration: 30,
            punctuation: false,
            numbers: false,
        }
    }
}

fn clamp_to(value: u16, range: &RangeInclusive<u16>) -> u16 {
    value.clamp(*range.start(), *range.end())
}
