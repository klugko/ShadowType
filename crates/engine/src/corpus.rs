//! Word lists, quotes and code snippets bundled into the binary.
//!
//! Word lists hold one word per line. Quotes and snippets use the fortune
//! format: entries are separated by a line containing only `%`. A quote ends
//! with an attribution line starting with `-- `.

use std::sync::LazyLock;

use crate::{
    language::{CodeLanguage, Language},
    normalize::normalize,
};

/// A passage of literature together with its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    pub text: String,
    pub source: Option<String>,
}

struct LanguageCorpus {
    words: Vec<String>,
    quotes: Vec<Quote>,
}

static ENGLISH: LazyLock<LanguageCorpus> = LazyLock::new(|| LanguageCorpus {
    words: parse_words(include_str!("../corpus/words/english.txt")),
    quotes: parse_quotes(include_str!("../corpus/quotes/english.txt")),
});

static FRENCH: LazyLock<LanguageCorpus> = LazyLock::new(|| LanguageCorpus {
    words: parse_words(include_str!("../corpus/words/french.txt")),
    quotes: parse_quotes(include_str!("../corpus/quotes/french.txt")),
});

static RUST: LazyLock<Vec<String>> =
    LazyLock::new(|| parse_snippets(include_str!("../corpus/code/rust.txt")));
static PYTHON: LazyLock<Vec<String>> =
    LazyLock::new(|| parse_snippets(include_str!("../corpus/code/python.txt")));
static TYPESCRIPT: LazyLock<Vec<String>> =
    LazyLock::new(|| parse_snippets(include_str!("../corpus/code/typescript.txt")));
static JAVASCRIPT: LazyLock<Vec<String>> =
    LazyLock::new(|| parse_snippets(include_str!("../corpus/code/javascript.txt")));
static SQL: LazyLock<Vec<String>> =
    LazyLock::new(|| parse_snippets(include_str!("../corpus/code/sql.txt")));

fn corpus(language: Language) -> &'static LanguageCorpus {
    match language {
        Language::English => &ENGLISH,
        Language::French => &FRENCH,
    }
}

/// Common words of `language`, most frequent first.
pub fn words(language: Language) -> &'static [String] {
    &corpus(language).words
}

pub fn quotes(language: Language) -> &'static [Quote] {
    &corpus(language).quotes
}

pub fn snippets(language: CodeLanguage) -> &'static [String] {
    match language {
        CodeLanguage::Rust => &RUST,
        CodeLanguage::Python => &PYTHON,
        CodeLanguage::TypeScript => &TYPESCRIPT,
        CodeLanguage::JavaScript => &JAVASCRIPT,
        CodeLanguage::Sql => &SQL,
    }
}

fn parse_words(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(normalize)
        .collect()
}

fn parse_quotes(raw: &str) -> Vec<Quote> {
    entries(raw)
        .into_iter()
        .filter_map(|entry| {
            let mut lines: Vec<&str> = entry.lines().collect();
            let source = lines
                .last()
                .and_then(|line| line.strip_prefix("-- "))
                .map(|source| source.trim().to_owned());
            if source.is_some() {
                lines.pop();
            }
            let text = normalize(&lines.join(" "));
            (!text.is_empty()).then_some(Quote { text, source })
        })
        .collect()
}

fn parse_snippets(raw: &str) -> Vec<String> {
    entries(raw)
        .into_iter()
        .map(|entry| normalize(&entry))
        .filter(|snippet| !snippet.is_empty())
        .collect()
}

fn entries(raw: &str) -> Vec<String> {
    let mut entries = vec![String::new()];
    for line in raw.lines() {
        if line.trim_end() == "%" {
            entries.push(String::new());
        } else if let Some(entry) = entries.last_mut() {
            entry.push_str(line);
            entry.push('\n');
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_split_on_percent_lines_with_any_line_ending() {
        let raw = "first\r\nline\r\n%\r\nsecond\r\n%\nthird % not a separator\n";
        let parsed = parse_snippets(raw);
        assert_eq!(parsed, ["first\nline", "second", "third % not a separator"]);
    }

    #[test]
    fn quotes_keep_their_attribution() {
        let raw = "It was the best of times.\n-- Charles Dickens\n%\nNo source here.\n";
        let parsed = parse_quotes(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "It was the best of times.");
        assert_eq!(parsed[0].source.as_deref(), Some("Charles Dickens"));
        assert_eq!(parsed[1].source, None);
    }

    #[test]
    fn empty_entries_are_skipped() {
        assert_eq!(parse_snippets("%\n\n%\ncode\n%\n"), ["code"]);
        assert!(parse_quotes("%\n-- Nobody\n").is_empty());
    }

    #[test]
    fn bundled_corpora_are_well_formed() {
        for language in Language::ALL {
            let words = words(language);
            assert!(words.len() >= 200, "{language}: too few words");
            assert!(
                words.iter().all(|word| !word.contains(char::is_whitespace)),
                "{language}: words must not contain whitespace"
            );
            assert!(!quotes(language).is_empty(), "{language}: no quotes");
        }
        for language in CodeLanguage::ALL {
            let snippets = snippets(language);
            assert!(!snippets.is_empty(), "{language}: no snippets");
            assert!(
                snippets.iter().all(|snippet| !snippet.contains('\t')),
                "{language}: tabs must be expanded"
            );
        }
    }
}
