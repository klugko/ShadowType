/*!
 * Syntax highlighting of code snippets: one hand-written lexer driven by a
 * small table per language, aimed at idiomatic snippets rather than full
 * grammars. It is total: any input, half-typed or malformed, gets exactly one
 * token per grapheme.
 */

mod languages;
mod lexer;

use code_racer_engine::CodeLanguage;

use lexer::Lexer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Plain,
    Keyword,
    Type,
    Function,
    Macro,
    String,
    Number,
    Comment,
    Punctuation,
}

pub fn highlight(graphemes: &[String], language: CodeLanguage) -> Vec<Token> {
    Lexer::new(graphemes, Syntax::of(language)).run()
}

#[derive(Debug)]
struct Syntax {
    line_comment: &'static str,
    block_comments: bool,
    strings: Strings,
    words: Words,
}

#[derive(Debug)]
struct Strings {
    quotes: &'static [Quote],
    triple_quotes: bool,
    escapes: bool,
    /// Letters that may prefix a string literal, such as `b` in `b"bytes"`.
    prefixes: &'static str,
    /// Whether an `r` prefix makes a Rust raw string: no escapes, optional `#` fences.
    raw_strings: bool,
    /// Whether `'` starts a Rust character literal or lifetime instead of a string.
    char_literals: bool,
}

#[derive(Debug, Clone, Copy)]
struct Quote {
    mark: &'static str,
    multiline: bool,
}

#[derive(Debug)]
struct Words {
    keywords: &'static [&'static [&'static str]],
    /// Keywords only where they open a statement, like Python's `match`.
    soft_keywords: &'static [&'static str],
    types: &'static [&'static str],
    /// Keyword and type lists are lowercase and matched case-insensitively.
    ignore_case: bool,
    capitalized_types: bool,
    macros: bool,
    identifier_symbols: &'static str,
}

impl Words {
    fn is_keyword(&self, word: &[String]) -> bool {
        self.keywords.iter().any(|list| self.contains(list, word))
    }

    fn is_type(&self, word: &[String]) -> bool {
        self.contains(self.types, word)
            || (self.capitalized_types
                && word
                    .first()
                    .and_then(|g| first_char(g))
                    .is_some_and(char::is_uppercase))
    }

    fn contains(&self, sorted: &[&str], word: &[String]) -> bool {
        let bytes = || word.iter().flat_map(|grapheme| grapheme.bytes());
        sorted
            .binary_search_by(|candidate| {
                if self.ignore_case {
                    candidate
                        .bytes()
                        .cmp(bytes().map(|byte| byte.to_ascii_lowercase()))
                } else {
                    candidate.bytes().cmp(bytes())
                }
            })
            .is_ok()
    }
}

fn first_char(grapheme: &str) -> Option<char> {
    grapheme.chars().next()
}

fn starts_with_digit(grapheme: &str) -> bool {
    first_char(grapheme).is_some_and(|c| c.is_ascii_digit())
}

fn is_char(grapheme: &str, expected: char) -> bool {
    let mut chars = grapheme.chars();
    chars.next() == Some(expected) && chars.next().is_none()
}

fn is_one_of(grapheme: &str, set: &str) -> bool {
    let mut chars = grapheme.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if set.contains(c))
}

#[cfg(test)]
mod tests;
