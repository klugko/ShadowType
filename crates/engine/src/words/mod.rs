//! Endless, sentence-aware stream of words with optional punctuation and numbers.

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
        let starts_sentence = self.words_left_in_sentence == 0;
        if starts_sentence {
            self.words_left_in_sentence = self.rng.random_range(SENTENCE_LENGTH);
        }
        self.words_left_in_sentence -= 1;
        let ends_sentence = closes_phrase || self.words_left_in_sentence == 0;
        if ends_sentence {
            self.words_left_in_sentence = 0;
        }

        let word = if self.options.numbers && self.rng.random_bool(NUMBER_PROBABILITY) {
            self.number()
        } else {
            self.vocabulary_word(starts_sentence && self.options.punctuation)
        };

        if !self.options.punctuation {
            return word;
        }
        if ends_sentence {
            let word = if starts_sentence {
                capitalize(&word)
            } else {
                word
            };
            self.end_sentence(word)
        } else if starts_sentence {
            self.open_sentence(&word)
        } else {
            self.decorate(word)
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: WordOptions = WordOptions {
        punctuation: false,
        numbers: false,
    };
    const PUNCTUATED: WordOptions = WordOptions {
        punctuation: true,
        numbers: false,
    };
    const NUMBERS: WordOptions = WordOptions {
        punctuation: false,
        numbers: true,
    };

    fn sample(language: Language, options: WordOptions, seed: u64) -> String {
        WordStream::new(language, options, seed).phrase(400)
    }

    #[test]
    fn phrase_has_exactly_the_requested_number_of_words() {
        for count in [1, 10, 25, 50, 100] {
            let text = WordStream::new(Language::English, PLAIN, 7).phrase(count);
            assert_eq!(text.split(' ').count(), count);
        }
    }

    #[test]
    fn same_seed_gives_same_text() {
        for options in [PLAIN, PUNCTUATED, NUMBERS] {
            assert_eq!(
                sample(Language::French, options, 42),
                sample(Language::French, options, 42)
            );
        }
        assert_ne!(
            sample(Language::English, PLAIN, 1),
            sample(Language::English, PLAIN, 2)
        );
    }

    #[test]
    fn plain_words_come_from_the_vocabulary() {
        let vocabulary = corpus::words(Language::French);
        let text = sample(Language::French, PLAIN, 3);
        assert!(
            text.split(' ')
                .all(|word| vocabulary.iter().any(|v| v == word))
        );
    }

    #[test]
    fn words_are_never_repeated_back_to_back() {
        let text = sample(Language::English, PLAIN, 11);
        let words: Vec<&str> = text.split(' ').collect();
        assert!(words.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn numbers_option_mixes_digits_in() {
        let text = sample(Language::English, NUMBERS, 5);
        let numbers = text
            .split(' ')
            .filter(|word| word.chars().all(|c| c.is_ascii_digit()))
            .count();
        assert!(
            (20..=90).contains(&numbers),
            "unexpected ratio: {numbers}/400"
        );
        assert!(!sample(Language::English, PLAIN, 5).contains(|c: char| c.is_ascii_digit()));
    }

    /**
     * Whether a sentence may start with `first`: an ASCII capital, easy to
     * type on every layout, or a digit.
     */
    fn opens_sentence(first: char) -> bool {
        first.is_ascii_uppercase() || first.is_ascii_digit()
    }

    #[test]
    fn punctuated_phrases_are_proper_sentences_in_every_language() {
        for language in Language::ALL {
            for numbers in [false, true] {
                let options = WordOptions {
                    punctuation: true,
                    numbers,
                };
                for seed in 0..20 {
                    let text = WordStream::new(language, options, seed).phrase(60);
                    let first = text.chars().next().expect("non-empty");
                    assert!(opens_sentence(first), "{text}");
                    assert!(text.ends_with(['.', '?', '!']), "{text}");
                    for sentence_end in [". ", "? ", "! "] {
                        for (index, _) in text.match_indices(sentence_end) {
                            let next = text[index + 2..].chars().next().expect("next word");
                            assert!(opens_sentence(next), "{text}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sentence_openers_start_with_an_ascii_letter_even_in_french() {
        let mut stream = WordStream::new(Language::French, PUNCTUATED, 0);
        for _ in 0..2_000 {
            let word = stream.vocabulary_word(true);
            assert!(starts_with_ascii_letter(&word), "{word}");
        }
    }

    #[test]
    fn plain_text_has_no_punctuation_or_capitals() {
        let text = sample(Language::English, PLAIN, 9);
        assert!(
            text.chars()
                .all(|c| c == ' ' || c.is_lowercase() || !c.is_alphabetic())
        );
        assert!(!text.contains([',', '.', '?', '!', '"', '(']));
    }

    #[test]
    fn french_puts_a_space_before_high_punctuation() {
        let text = sample(Language::French, PUNCTUATED, 13);
        for mark in ['?', '!', ';', ':'] {
            for (index, _) in text.match_indices(mark) {
                assert_eq!(&text[index - 1..index], " ", "{mark} in {text}");
            }
        }
        let english = sample(Language::English, PUNCTUATED, 13);
        assert!(!english.contains(" ?") && !english.contains(" !"));
    }

    fn has_decimal(text: &str, separator: char) -> bool {
        let chars: Vec<char> = text.chars().collect();
        chars.windows(3).any(|window| {
            window[0].is_ascii_digit() && window[1] == separator && window[2].is_ascii_digit()
        })
    }

    #[test]
    fn decimals_use_the_separator_of_the_language() {
        let options = WordOptions {
            punctuation: true,
            numbers: true,
        };
        let french: Vec<String> = (0..5)
            .map(|seed| sample(Language::French, options, seed))
            .collect();
        let english: Vec<String> = (0..5)
            .map(|seed| sample(Language::English, options, seed))
            .collect();
        assert!(french.iter().any(|text| has_decimal(text, ',')));
        assert!(french.iter().all(|text| !has_decimal(text, '.')));
        assert!(english.iter().any(|text| has_decimal(text, '.')));
        assert!(english.iter().all(|text| !has_decimal(text, ',')));
    }

    #[test]
    fn stream_keeps_producing_words() {
        let mut stream = WordStream::new(Language::English, PUNCTUATED, 0);
        assert!((0..1000).all(|_| !stream.next_word(false).is_empty()));
    }

    #[test]
    fn capitalize_handles_unicode() {
        assert_eq!(capitalize("été"), "Été");
        assert_eq!(capitalize(""), "");
    }
}
