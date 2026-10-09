use code_racer_engine::corpus;
use rand::SeedableRng;

use super::*;

#[test]
fn word_lists_are_sorted_for_binary_search() {
    for language in CodeLanguage::ALL {
        let words = &Syntax::of(language).words;
        let lists = words
            .keywords
            .iter()
            .chain([&words.soft_keywords, &words.types]);
        for list in lists {
            assert!(
                list.windows(2).all(|pair| pair[0] < pair[1]),
                "{language}: {list:?}"
            );
            if words.ignore_case {
                assert!(
                    list.iter().all(|word| *word == word.to_lowercase()),
                    "{language}"
                );
            }
        }
    }
}

#[test]
fn every_bundled_snippet_gets_one_token_per_grapheme() {
    for language in CodeLanguage::ALL {
        for snippet in corpus::snippets(language) {
            let graphemes = graphemes(snippet);
            let tokens = highlight(&graphemes, language);
            assert_eq!(tokens.len(), graphemes.len(), "{language}: {snippet}");
            assert!(tokens.contains(&Token::Keyword), "{language}: {snippet}");
        }
    }
}

fn random_char(rng: &mut StdRng) -> char {
    let ranges = [
        0..0x80,
        0x80..0x800,
        0x300..0x370,
        0x1F300..0x1FAFF,
        0..0x11_0000,
    ];
    let range = ranges[rng.random_range(0..ranges.len())].clone();
    char::from_u32(rng.random_range(range)).unwrap_or('\u{FFFD}')
}

#[test]
fn arbitrary_input_never_panics_and_keeps_one_token_per_grapheme() {
    let mut rng = StdRng::seed_from_u64(42);
    for round in 0..4_000 {
        let source = if round % 2 == 0 {
            random_source(&mut rng, PIECES, 30)
        } else {
            (0..rng.random_range(0..30))
                .map(|_| random_char(&mut rng))
                .collect()
        };
        let graphemes = graphemes(&source);
        for language in CodeLanguage::ALL {
            assert_eq!(
                highlight(&graphemes, language).len(),
                graphemes.len(),
                "{source:?}"
            );
        }
    }
}

#[test]
fn whitespace_is_never_highlighted_as_code() {
    let mut rng = StdRng::seed_from_u64(3);
    for _ in 0..2_000 {
        let graphemes = graphemes(&random_source(&mut rng, PIECES, 30));
        for language in CodeLanguage::ALL {
            let tokens = highlight(&graphemes, language);
            for (grapheme, token) in graphemes.iter().zip(&tokens) {
                let blank = grapheme == " " || grapheme == "\n";
                assert!(
                    !blank || matches!(token, Token::Plain | Token::String | Token::Comment),
                    "{language}: {:?} gives {token:?}",
                    graphemes.concat()
                );
            }
        }
    }
}
