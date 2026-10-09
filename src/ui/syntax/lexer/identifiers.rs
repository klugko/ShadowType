use super::Lexer;
use crate::ui::syntax::{Token, first_char, is_one_of};

impl Lexer<'_> {
    pub(super) fn lifetime(&self) -> (Token, usize) {
        match self.identifier_len(1) {
            0 => (Token::Punctuation, 1),
            name => (Token::Keyword, 1 + name),
        }
    }

    pub(super) fn identifier(&self) -> (Token, usize) {
        let words = &self.syntax.words;
        let len = self.identifier_len(0);
        let word = &self.rest()[..len];
        if words.is_keyword(word) || self.is_soft_keyword(word) {
            return (Token::Keyword, len);
        }
        if words.macros && self.at(len) == "!" && self.at(len + 1) != "=" {
            return (Token::Macro, len + 1);
        }
        let token = if words.is_type(word) {
            Token::Type
        } else if self.at(len) == "(" {
            Token::Function
        } else {
            Token::Plain
        };
        (token, len)
    }

    /**
     * Whether `word` is a soft keyword such as Python's `match`: one that opens
     * a statement, is followed by a space and is not being assigned to, as in
     * `match = pattern.search(line)`.
     */
    fn is_soft_keyword(&self, word: &[String]) -> bool {
        let words = &self.syntax.words;
        let spaces = self.count_from(word.len(), " ");
        let next = word.len() + spaces;
        words.contains(words.soft_keywords, word)
            && spaces > 0
            && !matches!(self.at(next), "" | "\n")
            && !self.is_assignment_at(next)
            && self.starts_line()
    }

    fn is_assignment_at(&self, offset: usize) -> bool {
        let operator = self
            .rest()
            .iter()
            .skip(offset)
            .take_while(|grapheme| is_one_of(grapheme, "+-*/%&|^@<>"))
            .count();
        self.at(offset + operator) == "="
    }

    fn starts_line(&self) -> bool {
        self.graphemes[..self.tokens.len()]
            .iter()
            .rev()
            .take_while(|grapheme| *grapheme != "\n")
            .all(|grapheme| grapheme == " ")
    }

    fn identifier_len(&self, offset: usize) -> usize {
        if !self.is_identifier_start(self.at(offset)) {
            return 0;
        }
        1 + self
            .rest()
            .iter()
            .skip(offset + 1)
            .take_while(|grapheme| self.is_identifier_continue(grapheme))
            .count()
    }

    pub(super) fn is_identifier_start(&self, grapheme: &str) -> bool {
        first_char(grapheme).is_some_and(|c| c.is_alphabetic() || c == '_')
            || is_one_of(grapheme, self.syntax.words.identifier_symbols)
    }

    fn is_identifier_continue(&self, grapheme: &str) -> bool {
        first_char(grapheme).is_some_and(|c| c.is_alphanumeric() || c == '_')
            || is_one_of(grapheme, self.syntax.words.identifier_symbols)
    }
}
