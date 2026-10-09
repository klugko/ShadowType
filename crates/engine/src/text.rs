use std::ops::RangeInclusive;

use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};

use crate::{
    corpus,
    language::{CodeLanguage, Language},
    words::{WordOptions, WordStream},
};

/// Word counts accepted for a words session.
pub const WORD_COUNTS: RangeInclusive<u16> = 1..=500;

/// A finite text to type: generated words, a quote or a code snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextSource {
    Words {
        language: Language,
        count: u16,
        #[serde(flatten)]
        options: WordOptions,
    },
    Quote {
        language: Language,
    },
    Code {
        language: CodeLanguage,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedText {
    pub text: String,
    /// Author and work of a quote.
    pub attribution: Option<String>,
}

impl TextSource {
    /// The same seed always yields the same text.
    pub fn generate(&self, seed: u64) -> GeneratedText {
        match *self {
            Self::Words {
                language,
                count,
                options,
            } => GeneratedText {
                text: WordStream::new(language, options, seed).phrase(usize::from(count)),
                attribution: None,
            },
            Self::Quote { language } => {
                let quotes = corpus::quotes(language);
                let quote = &quotes[pick(quotes.len(), seed)];
                GeneratedText {
                    text: quote.text.clone(),
                    attribution: quote.source.clone(),
                }
            }
            Self::Code { language } => {
                let snippets = corpus::snippets(language);
                GeneratedText {
                    text: snippets[pick(snippets.len(), seed)].clone(),
                    attribution: None,
                }
            }
        }
    }
}

fn pick(len: usize, seed: u64) -> usize {
    StdRng::seed_from_u64(seed).random_range(0..len.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_source_respects_count_and_options() {
        let source = TextSource::Words {
            language: Language::English,
            count: 25,
            options: WordOptions {
                punctuation: true,
                numbers: true,
            },
        };
        let generated = source.generate(99);
        assert_eq!(generated.text.split(' ').count(), 25);
        assert!(generated.text.ends_with(['.', '?', '!']));
        assert_eq!(generated.attribution, None);
    }

    #[test]
    fn quote_comes_from_the_bundled_collection() {
        let generated = TextSource::Quote {
            language: Language::French,
        }
        .generate(4);
        let quotes = corpus::quotes(Language::French);
        assert!(quotes.iter().any(|quote| quote.text == generated.text));
    }

    #[test]
    fn code_snippets_keep_their_layout() {
        for language in CodeLanguage::ALL {
            let snippets = corpus::snippets(language);
            let generated = TextSource::Code { language }.generate(1);
            assert!(snippets.contains(&generated.text));
            assert!(
                generated.text.contains('\n'),
                "{language} snippet is one line"
            );
        }
    }

    #[test]
    fn different_seeds_reach_different_quotes() {
        let source = TextSource::Quote {
            language: Language::English,
        };
        let distinct: std::collections::HashSet<String> =
            (0..50).map(|seed| source.generate(seed).text).collect();
        assert!(distinct.len() > 5);
    }

    #[test]
    fn serializes_with_a_kind_tag() {
        let source = TextSource::Words {
            language: Language::French,
            count: 30,
            options: WordOptions::default(),
        };
        let json = serde_json::to_value(source).expect("serialize");
        assert_eq!(json["kind"], "words");
        assert_eq!(json["language"], "french");
        assert_eq!(json["punctuation"], false);
        let back: TextSource = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, source);
    }
}
