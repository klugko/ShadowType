mod boundaries;
mod invariants;
mod javascript;
mod lexemes;
mod python;
mod rust;
mod sql;

use code_racer_engine::graphemes;
use rand::{RngExt, rngs::StdRng};

use super::*;

fn tokens_of(source: &str, language: CodeLanguage, needle: &str) -> Vec<Token> {
    let byte = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {source:?}"));
    let start = graphemes(&source[..byte]).len();
    let tokens = highlight(&graphemes(source), language);
    tokens[start..start + graphemes(needle).len()].to_vec()
}

fn token(source: &str, language: CodeLanguage, needle: &str) -> Token {
    let tokens = tokens_of(source, language, needle);
    assert!(
        tokens.windows(2).all(|pair| pair[0] == pair[1]),
        "{needle:?} in {source:?} is split: {tokens:?}"
    );
    tokens[0]
}

const PIECES: &[&str] = &[
    "\"", "'", "`", "\\", "/", "*", "/*", "*/", "//", "#", "-", "--", "r", "b", "f", "r#\"", "\"#",
    "\"\"\"", "'a", "0x", "1", "1e", "1usize", ".", "+", "=", "!", "(", "\n", " ", "é", "e\u{301}",
    "👍🏽", "$x", "fn", "match ", "case", "Type", "x1", "SELECT",
];

fn random_source(rng: &mut StdRng, pieces: &[&str], max_len: usize) -> String {
    let len = rng.random_range(0..max_len);
    (0..len)
        .map(|_| pieces[rng.random_range(0..pieces.len())])
        .collect()
}
