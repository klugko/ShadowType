use unicode_width::UnicodeWidthStr;

/**
 * Marks that belong to the word before them even when a space separates them,
 * as in French typography (`vraiment ?`): they never start a row on their own.
 */
const CLOSING_MARKS: [&str; 5] = ["?", "!", ";", ":", "»"];

/// One row on screen: the graphemes `start..end` of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualLine {
    pub start: usize,
    pub end: usize,
    /// 1-based number of the logical line, on its first row only.
    pub number: Option<usize>,
}

/**
 * Lays `graphemes` out in rows of at most `width` cells, like an editor with
 * `wrap` and `linebreak` set.
 *
 * A word moves to the next row when it does not fit together with the spaces
 * (or the newline) that follow it, so those stay at the end of its row. Only a
 * word wider than a whole row is broken; it starts on a fresh row, right after
 * the indentation when it opens a line. The indentation stays with the first
 * word of its line unless that would break a word that fits on a row by
 * itself. A `"\n"` grapheme takes one cell and ends its row. Every grapheme
 * takes at least one cell so the cursor stays visible on zero-width
 * characters; one wider than `width` gets a row of its own. A `width` of zero
 * is treated as one.
 *
 * The rows cover every grapheme exactly once, in order. The last row always
 * has a free cell for the cursor after the last grapheme: the text ends on an
 * empty row of its own after a final newline or a full row, so there is always
 * at least one row.
 */
pub fn wrap(graphemes: &[String], width: u16) -> Vec<VisualLine> {
    let mut rows = Rows::new(usize::from(width.max(1)));
    let mut start = 0;
    while start < graphemes.len() {
        let end = chunk_end(graphemes, start);
        rows.place(&graphemes[start..end], start);
        start = end;
    }
    rows.finish(graphemes.len())
}

/**
 * Row and cell column of the grapheme at `index` in `lines`, as produced by
 * [`wrap`] for the same `graphemes`.
 *
 * `graphemes.len()` is the position just after the last grapheme, where the
 * cursor sits once everything is typed; larger indexes are clamped to it.
 */
pub fn locate(graphemes: &[String], lines: &[VisualLine], index: usize) -> (usize, usize) {
    let index = index.min(graphemes.len());
    let row = lines
        .partition_point(|line| line.end <= index)
        .min(lines.len().saturating_sub(1));
    let start = lines.get(row).map_or(0, |line| line.start).min(index);
    (row, cells(&graphemes[start..index]))
}

struct Rows {
    width: usize,
    lines: Vec<VisualLine>,
    start: usize,
    used: usize,
    number: Option<usize>,
    logical_lines: usize,
}

impl Rows {
    fn new(width: usize) -> Self {
        Self {
            width,
            lines: Vec::new(),
            start: 0,
            used: 0,
            number: Some(1),
            logical_lines: 1,
        }
    }

    fn place(&mut self, chunk: &[String], offset: usize) {
        let indentation = self.detachable_indentation(chunk);
        if indentation > 0 {
            self.fill(&chunk[..indentation], offset);
        }
        self.fill(&chunk[indentation..], offset + indentation);
        if chunk.last().is_some_and(|grapheme| is_newline(grapheme)) {
            self.close_row(offset + chunk.len());
            self.logical_lines += 1;
            self.number = Some(self.logical_lines);
        }
    }

    /**
     * Length of the indentation that has to be laid out apart from the word
     * after it: zero unless the pair is wider than a row while the word alone
     * is not.
     */
    fn detachable_indentation(&self, chunk: &[String]) -> usize {
        let indentation = count_while(chunk, is_space);
        let word = count_while(&chunk[indentation..], is_word_part);
        let detach = word > 0
            && cells(chunk) > self.width
            && cells(&chunk[indentation..indentation + word]) <= self.width;
        if detach { indentation } else { 0 }
    }

    fn fill(&mut self, piece: &[String], offset: usize) {
        let needed = cells(piece);
        if self.used > 0 && self.used + needed > self.width {
            self.close_row(offset);
        }
        if self.used + needed <= self.width {
            self.used += needed;
        } else {
            self.break_word(piece, offset);
        }
    }

    fn break_word(&mut self, piece: &[String], offset: usize) {
        for (index, grapheme) in (offset..).zip(piece) {
            let needed = cell_width(grapheme);
            if self.used > 0 && self.used + needed > self.width {
                self.close_row(index);
            }
            self.used += needed;
        }
    }

    fn close_row(&mut self, end: usize) {
        self.lines.push(VisualLine {
            start: self.start,
            end,
            number: self.number.take(),
        });
        self.start = end;
        self.used = 0;
    }

    fn finish(mut self, len: usize) -> Vec<VisualLine> {
        if self.used >= self.width {
            self.close_row(len);
        }
        self.close_row(len);
        self.lines
    }
}

/**
 * End of the unit that wraps as a whole: a word with the indentation before
 * it, the spaces after it, any closing marks detached from it by a space, and
 * a newline that ends it.
 */
fn chunk_end(graphemes: &[String], start: usize) -> usize {
    let mut end = start;
    loop {
        let word = count_while(&graphemes[end..], is_word_part);
        let spaces = count_while(&graphemes[end + word..], is_space);
        end += word + spaces;
        if graphemes
            .get(end)
            .is_some_and(|grapheme| is_newline(grapheme))
        {
            return end + 1;
        }
        let indentation = word == 0;
        let closing_mark = spaces > 0 && starts_with_closing_mark(&graphemes[end..]);
        if end == graphemes.len() || !(indentation || closing_mark) {
            return end;
        }
    }
}

fn starts_with_closing_mark(graphemes: &[String]) -> bool {
    let detached = graphemes
        .get(1)
        .is_none_or(|next| is_space(next) || is_newline(next));
    detached
        && graphemes
            .first()
            .is_some_and(|mark| CLOSING_MARKS.contains(&mark.as_str()))
}

fn count_while(graphemes: &[String], predicate: impl Fn(&str) -> bool) -> usize {
    graphemes
        .iter()
        .take_while(|grapheme| predicate(grapheme))
        .count()
}

fn cells(graphemes: &[String]) -> usize {
    graphemes.iter().map(|grapheme| cell_width(grapheme)).sum()
}

fn cell_width(grapheme: &str) -> usize {
    grapheme.width().max(1)
}

fn is_space(grapheme: &str) -> bool {
    grapheme == " "
}

fn is_newline(grapheme: &str) -> bool {
    grapheme == "\n"
}

fn is_word_part(grapheme: &str) -> bool {
    !is_space(grapheme) && !is_newline(grapheme)
}

#[cfg(test)]
mod tests;
