//! Endless, sentence-aware stream of words with optional punctuation and numbers.

#[cfg(test)]
mod tests;

use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};

use crate::{corpus, language::Language};

const SENTENCE_LENGTH: std::ops::RangeInclusive<usize> = 4..=12;
const NUMBER_PROBABILITY: f64 = 0.12;
const PICK_ATTEMPTS: usize = 16;

/// Extra characters mixed into generated words.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WordOptions {
    /// Capitalised sentences, commas, quotes, parentheses and sentence endings.
    #[serde(default)]
    pub punctuation: bool,
    #[serde(default)]
    pub numbers: bool,
}

/**
 * Generates words from a language's vocabulary, one at a time.
 *
 * With punctuation enabled the stream is organised in sentences: the first
 * word is capitalised, the last one carries a full stop, a question mark or an
 * exclamation mark, and words in between are occasionally decorated with
 * commas, quotes or parentheses. French spacing rules are respected. The
 * output is fully determined by the seed.
 */
#[derive(Debug)]
pub struct WordStream {
    language: Language,
    vocabulary: &'static [String],
    options: WordOptions,
    rng: StdRng,
    previous: Option<&'static str>,
    words_left_in_sentence: usize,
}

impl WordStream {
    pub fn new(language: Language, options: WordOptions, seed: u64) -> Self {
        Self {
            language,
            vocabulary: corpus::words(language),
            options,
            rng: StdRng::seed_from_u64(seed),
            previous: None,
            words_left_in_sentence: 0,
        }
    }

    /**
     * Generates `count` words separated by spaces.
     *
     * With punctuation enabled the text always ends a sentence, so phrases can
     * be appended to each other.
     */
    pub fn phrase(&mut self, count: usize) -> String {
        (0..count)
            .map(|index| self.next_word(index + 1 == count))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn next_word(&mut self, closes_phrase: bool) -> String {
        let (starts_sentence, ends_sentence) = self.advance_sentence(closes_phrase);
        let word = if self.options.numbers && self.rng.random_bool(NUMBER_PROBABILITY) {
            self.number()
        } else {
            self.vocabulary_word(starts_sentence && self.options.punctuation)
        };
        if !self.options.punctuation {
            return word;
        }
        match (starts_sentence, ends_sentence) {
            (true, true) => self.end_sentence(capitalize(&word)),
            (false, true) => self.end_sentence(word),
            (true, false) => self.open_sentence(&word),
            (false, false) => self.decorate(word),
        }
    }

    /// Counts the next word into its sentence: whether it starts it, and whether it ends it.
    fn advance_sentence(&mut self, closes_phrase: bool) -> (bool, bool) {
        let starts_sentence = self.words_left_in_sentence == 0;
        if starts_sentence {
            self.words_left_in_sentence = self.rng.random_range(SENTENCE_LENGTH);
        }
        self.words_left_in_sentence -= 1;
        let ends_sentence = closes_phrase || self.words_left_in_sentence == 0;
        if ends_sentence {
            self.words_left_in_sentence = 0;
        }
        (starts_sentence, ends_sentence)
    }

    /**
     * Picks a random word, avoiding immediate repetitions. Sentence openers
     * must start with an ASCII letter so that their capital is easy to type.
     */
    fn vocabulary_word(&mut self, opens_sentence: bool) -> String {
        if self.vocabulary.is_empty() {
            return String::new();
        }
        let mut word = self.random_word();
        for _ in 1..PICK_ATTEMPTS {
            let acceptable =
                Some(word) != self.previous && (!opens_sentence || starts_with_ascii_letter(word));
            if acceptable {
                break;
            }
            word = self.random_word();
        }
        self.previous = Some(word);
        word.to_owned()
    }

    fn random_word(&mut self) -> &'static str {
        let vocabulary: &'static [String] = self.vocabulary;
        &vocabulary[self.rng.random_range(0..vocabulary.len())]
    }

    fn number(&mut self) -> String {
        let decimals_allowed = self.options.punctuation;
        match self.rng.random_range(0..100) {
            0..45 => self.rng.random_range(0..100u32).to_string(),
            45..70 => self.rng.random_range(100..10_000u32).to_string(),
            70..85 => self.rng.random_range(1950..=2030u32).to_string(),
            _ if decimals_allowed => {
                let whole = self.rng.random_range(0..100u32);
                let fraction = self.rng.random_range(0..100u32);
                let separator = self.language.decimal_separator();
                format!("{whole}{separator}{fraction:02}")
            }
            _ => self.rng.random_range(10..1000u32).to_string(),
        }
    }

    fn end_sentence(&mut self, word: String) -> String {
        let mark = match self.rng.random_range(0..100) {
            0..76 => ".",
            76..90 => "?",
            _ => "!",
        };
        self.attach(word, mark)
    }

    fn open_sentence(&mut self, word: &str) -> String {
        let word = capitalize(word);
        if self.rng.random_bool(0.08) {
            self.attach(word, ",")
        } else {
            word
        }
    }

    fn decorate(&mut self, word: String) -> String {
        match self.rng.random_range(0..100) {
            0..12 => self.attach(word, ","),
            12..14 => self.attach(word, ";"),
            14..16 => self.attach(word, ":"),
            16..19 => format!("\"{word}\""),
            19..22 => format!("({word})"),
            _ => word,
        }
    }

    fn attach(&self, word: String, mark: &str) -> String {
        let spaced =
            matches!(mark, "?" | "!" | ";" | ":") && self.language.spaces_high_punctuation();
        if spaced {
            format!("{word} {mark}")
        } else {
            format!("{word}{mark}")
        }
    }
}

fn starts_with_ascii_letter(word: &str) -> bool {
    word.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
