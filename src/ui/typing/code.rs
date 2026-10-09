use code_racer_engine::CodeLanguage;

use crate::ui::{
    syntax::{self, Token},
    wrap::VisualLine,
};

const DEFAULT_INDENT: usize = 4;

/// The syntax class and bracket depth of every character of code.
pub(super) struct Syntax {
    pub(super) tokens: Vec<Token>,
    pub(super) depths: Vec<Option<usize>>,
}

impl Syntax {
    pub(super) fn of(target: &[String], language: CodeLanguage) -> Self {
        let tokens = syntax::highlight(target, language);
        let depths = bracket_depths(target, &tokens);
        Self { tokens, depths }
    }
}

/**
 * How deep each bracket of code is nested, `None` for other characters.
 * Brackets in strings and comments are not punctuation, so they do not count.
 */
pub(super) fn bracket_depths(target: &[String], tokens: &[Token]) -> Vec<Option<usize>> {
    let mut depth: usize = 0;
    target
        .iter()
        .zip(tokens)
        .map(|(grapheme, token)| {
            if *token != Token::Punctuation {
                return None;
            }
            match grapheme.as_str() {
                "(" | "[" | "{" => {
                    depth += 1;
                    Some(depth - 1)
                }
                ")" | "]" | "}" => {
                    depth = depth.saturating_sub(1);
                    Some(depth)
                }
                _ => None,
            }
        })
        .collect()
}

/**
 * The width of one level of indentation in code: the smallest indentation
 * of its lines, or [`DEFAULT_INDENT`] when none is indented.
 */
pub(super) fn indent_unit(target: &[String], lines: &[VisualLine]) -> usize {
    lines
        .iter()
        .filter(|line| line.number.is_some())
        .map(|line| leading_spaces(target, line))
        .filter(|spaces| *spaces > 0)
        .min()
        .unwrap_or(DEFAULT_INDENT)
}

/// Zero for the rows that continue a wrapped line.
pub(super) fn leading_spaces(target: &[String], line: &VisualLine) -> usize {
    if line.number.is_none() {
        return 0;
    }
    target[line.start..line.end]
        .iter()
        .take_while(|grapheme| *grapheme == " ")
        .count()
}
