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
