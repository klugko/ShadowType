mod identifiers;
mod literals;

use std::iter;

use super::{Syntax, Token, first_char, is_char, is_one_of, starts_with_digit};

pub(super) struct Lexer<'a> {
    graphemes: &'a [String],
    syntax: &'static Syntax,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    pub(super) fn new(graphemes: &'a [String], syntax: &'static Syntax) -> Self {
        Self {
            graphemes,
            syntax,
            tokens: Vec::with_capacity(graphemes.len()),
        }
    }

    pub(super) fn run(mut self) -> Vec<Token> {
        while self.tokens.len() < self.graphemes.len() {
            let (token, len) = self.lexeme();
            let len = len.clamp(1, self.rest().len());
            self.tokens.extend(iter::repeat_n(token, len));
        }
        self.tokens
    }

    fn rest(&self) -> &[String] {
        &self.graphemes[self.tokens.len()..]
    }

    fn at(&self, offset: usize) -> &str {
        self.rest().get(offset).map_or("", String::as_str)
    }

    fn lexeme(&self) -> (Token, usize) {
        if self.starts_with(self.syntax.line_comment) {
            return (Token::Comment, self.line_comment_len());
        }
        if self.syntax.block_comments && self.starts_with("/*") {
            return (Token::Comment, self.block_comment_len());
        }
        if let Some(len) = self.string_len() {
            return (Token::String, len);
        }
        if self.syntax.strings.char_literals && self.at(0) == "'" {
            return self.lifetime();
        }
        if starts_with_digit(self.at(0)) {
            return (Token::Number, self.number_len());
        }
        if self.is_identifier_start(self.at(0)) {
            return self.identifier();
        }
        let punctuation = first_char(self.at(0)).is_some_and(|c| c.is_ascii_punctuation());
        let token = if punctuation {
            Token::Punctuation
        } else {
            Token::Plain
        };
        (token, 1)
    }

    fn starts_with(&self, pattern: &str) -> bool {
        !pattern.is_empty()
            && pattern
                .chars()
                .enumerate()
                .all(|(offset, expected)| is_char(self.at(offset), expected))
    }

    fn count_from(&self, offset: usize, expected: &str) -> usize {
        self.rest()
            .iter()
            .skip(offset)
            .take_while(|grapheme| *grapheme == expected)
            .count()
    }

    fn line_comment_len(&self) -> usize {
        let rest = self.rest();
        rest.iter()
            .position(|grapheme| grapheme == "\n")
            .unwrap_or(rest.len())
    }

    fn block_comment_len(&self) -> usize {
        (2..self.rest().len())
            .find(|&offset| self.at(offset) == "*" && self.at(offset + 1) == "/")
            .map_or(self.rest().len(), |offset| offset + 2)
    }

    fn number_len(&self) -> usize {
        let hex = self.at(0) == "0" && is_one_of(self.at(1), "xX");
        let mut len = 1;
        let mut fraction = false;
        loop {
            let grapheme = self.at(len);
            let continues = match grapheme {
                "." => !fraction && !hex && starts_with_digit(self.at(len + 1)),
                "+" | "-" => !hex && self.is_exponent_marker(len - 1),
                _ => first_char(grapheme).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_'),
            };
            if !continues {
                return len;
            }
            fraction |= grapheme == ".";
            len += 1;
        }
    }

    /**
     * Whether the `e` at `offset` starts an exponent, as in `2.5e-3`, rather
     * than ending a suffix, as in `1usize-1`.
     */
    fn is_exponent_marker(&self, offset: usize) -> bool {
        is_one_of(self.at(offset), "eE")
            && offset
                .checked_sub(1)
                .is_some_and(|before| starts_with_digit(self.at(before)))
    }
}
