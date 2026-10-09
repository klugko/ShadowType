use super::Lexer;
use crate::ui::syntax::is_one_of;

#[derive(Debug, Clone, Copy)]
struct Literal {
    quote: &'static str,
    repeat: usize,
    hashes: usize,
    escapes: bool,
    multiline: bool,
}

const CHAR_LITERAL: Literal = Literal {
    quote: "'",
    repeat: 1,
    hashes: 0,
    escapes: true,
    multiline: false,
};

impl Lexer<'_> {
    pub(super) fn string_len(&self) -> Option<usize> {
        let strings = &self.syntax.strings;
        let prefix = self
            .rest()
            .iter()
            .take(2)
            .take_while(|grapheme| is_one_of(grapheme, strings.prefixes))
            .count();
        let raw = strings.raw_strings && self.rest()[..prefix].iter().any(|g| is_one_of(g, "rR"));
        let hashes = if raw { self.count_from(prefix, "#") } else { 0 };
        let open = prefix + hashes;
        if strings.char_literals && hashes == 0 && self.at(open) == "'" {
            return self.char_literal_len(open);
        }
        let quote = strings
            .quotes
            .iter()
            .find(|quote| self.at(open) == quote.mark)?;
        let triple = strings.triple_quotes
            && self.at(open + 1) == quote.mark
            && self.at(open + 2) == quote.mark;
        let literal = Literal {
            quote: quote.mark,
            repeat: if triple { 3 } else { 1 },
            hashes,
            escapes: strings.escapes && !raw,
            multiline: quote.multiline || triple,
        };
        Some(self.literal_end(open + literal.repeat, literal))
    }

    fn char_literal_len(&self, open: usize) -> Option<usize> {
        if self.at(open + 1) == "\\" {
            return Some(self.literal_end(open + 1, CHAR_LITERAL));
        }
        let single = !matches!(self.at(open + 1), "'" | "\n" | "");
        (single && self.at(open + 2) == "'").then_some(open + 3)
    }

    fn literal_end(&self, start: usize, literal: Literal) -> usize {
        let mut offset = start;
        while offset < self.rest().len() {
            let grapheme = self.at(offset);
            if literal.escapes && grapheme == "\\" {
                offset += 2;
            } else if grapheme == "\n" && !literal.multiline {
                return offset;
            } else if self.closes(offset, literal) {
                return offset + literal.repeat + literal.hashes;
            } else {
                offset += 1;
            }
        }
        self.rest().len()
    }

    fn closes(&self, offset: usize, literal: Literal) -> bool {
        (0..literal.repeat).all(|index| self.at(offset + index) == literal.quote)
            && (0..literal.hashes).all(|index| self.at(offset + literal.repeat + index) == "#")
    }
}
