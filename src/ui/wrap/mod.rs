//! Soft wrapping of the text being typed into rows of terminal cells.

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
mod tests {
    use code_racer_engine::graphemes;
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    use super::*;

    fn rows(text: &str, width: u16) -> Vec<String> {
        let graphemes = graphemes(text);
        wrap(&graphemes, width)
            .iter()
            .map(|line| graphemes[line.start..line.end].concat())
            .collect()
    }

    fn numbers(text: &str, width: u16) -> Vec<Option<usize>> {
        wrap(&graphemes(text), width)
            .iter()
            .map(|line| line.number)
            .collect()
    }

    fn position(text: &str, width: u16, index: usize) -> (usize, usize) {
        let graphemes = graphemes(text);
        locate(&graphemes, &wrap(&graphemes, width), index)
    }

    #[test]
    fn empty_text_is_one_empty_first_line() {
        assert_eq!(
            wrap(&[], 20),
            [VisualLine {
                start: 0,
                end: 0,
                number: Some(1)
            }]
        );
        assert_eq!(position("", 20, 0), (0, 0));
    }

    #[test]
    fn words_that_do_not_fit_move_to_the_next_row_with_their_space() {
        assert_eq!(
            rows("the quick brown fox jumps", 10),
            ["the quick ", "brown fox ", "jumps"]
        );
        assert_eq!(
            numbers("the quick brown fox jumps", 10),
            [Some(1), None, None]
        );
    }

    #[test]
    fn a_word_ending_at_the_edge_moves_so_its_space_stays_visible() {
        assert_eq!(rows("abc defg hi", 8), ["abc ", "defg hi"]);
    }

    #[test]
    fn words_longer_than_a_row_are_broken() {
        assert_eq!(rows("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(rows("ab abcdefghij", 4), ["ab ", "abcd", "efgh", "ij"]);
    }

    #[test]
    fn newlines_end_their_row_and_take_a_cell() {
        assert_eq!(rows("ab\ncd", 10), ["ab\n", "cd"]);
        assert_eq!(numbers("ab\ncd", 10), [Some(1), Some(2)]);
        assert_eq!(rows("abcd\nef", 4), ["abcd", "\n", "ef"]);
        assert_eq!(numbers("abcd\nef", 4), [Some(1), None, Some(2)]);
    }

    #[test]
    fn continuation_rows_are_unnumbered() {
        let text = "aaa bbb\nccc ddd eee\n\nfff";
        assert_eq!(
            rows(text, 8),
            ["aaa bbb\n", "ccc ddd ", "eee\n", "\n", "fff"]
        );
        assert_eq!(numbers(text, 8), [Some(1), Some(2), None, Some(3), Some(4)]);
    }

    #[test]
    fn trailing_newline_opens_an_empty_numbered_row() {
        assert_eq!(rows("ab\n", 10), ["ab\n", ""]);
        assert_eq!(numbers("ab\n", 10), [Some(1), Some(2)]);
        assert_eq!(position("ab\n", 10, 3), (1, 0));
    }

    #[test]
    fn indentation_is_kept_on_its_row() {
        let text = "fn main() {\n    let total = 1;\n}";
        assert_eq!(
            rows(text, 14),
            ["fn main() {\n", "    let total ", "= 1;\n", "}"]
        );
    }

    #[test]
    fn indentation_is_not_left_alone_before_a_word_that_has_to_break() {
        let text = "{\n    println!(\"{total}\");\n}";
        assert_eq!(
            rows(text, 16),
            ["{\n", "    println!(\"{t", "otal}\");\n", "}"]
        );
        assert_eq!(rows("   ", 2), ["  ", " "]);
        assert_eq!(rows("  \n  x", 8), ["  \n", "  x"]);
    }

    #[test]
    fn indentation_gets_its_own_row_rather_than_breaking_a_word_that_fits() {
        assert_eq!(
            rows("x\n        abcdefghij", 12),
            ["x\n", "        ", "abcdefghij"]
        );
        assert_eq!(
            rows("x\n        abcdefghijkl\ny", 12),
            ["x\n", "        ", "abcdefghijkl", "\n", "y"]
        );
        assert_eq!(
            numbers("x\n        abcdefghij", 12),
            [Some(1), Some(2), None]
        );
    }

    #[test]
    fn detached_closing_marks_stay_with_their_word() {
        assert_eq!(
            rows("il dit bonjour ? oui", 15),
            ["il dit ", "bonjour ? oui"]
        );
        assert_eq!(rows("un « mot » ici", 10), ["un « ", "mot » ici"]);
    }

    #[test]
    fn wide_graphemes_take_two_cells() {
        assert_eq!(rows("日本語テキスト", 5), ["日本", "語テ", "キス", "ト"]);
        assert_eq!(position("日本語テキスト", 5, 3), (1, 2));
    }

    #[test]
    fn a_grapheme_wider_than_the_row_gets_a_row_of_its_own() {
        assert_eq!(rows("a日b", 1), ["a", "日", "b", ""]);
        assert_eq!(position("a日b", 1, 2), (2, 0));
    }

    #[test]
    fn zero_width_graphemes_take_one_cell() {
        assert_eq!(position("a\u{200b}b", 10, 2), (0, 2));
    }

    #[test]
    fn width_one_puts_every_grapheme_on_its_own_row() {
        assert_eq!(rows("ab c\nd", 1), ["a", "b", " ", "c", "\n", "d", ""]);
        assert_eq!(rows("ab", 0), ["a", "b", ""]);
        assert_eq!(position("ab c\nd", 1, 5), (5, 0));
    }

    #[test]
    fn a_full_last_row_leaves_the_end_cursor_on_an_empty_row() {
        assert_eq!(rows("abcd efgh", 4), ["abcd", " ", "efgh", ""]);
        assert_eq!(numbers("abcd efgh", 4), [Some(1), None, None, None]);
        assert_eq!(position("abcd efgh", 4, 9), (3, 0));
        assert_eq!(rows("ab\ncd", 2), ["ab", "\n", "cd", ""]);
        assert_eq!(rows("abc", 4), ["abc"]);
        assert_eq!(position("abc", 4, 3), (0, 3));
    }

    #[test]
    fn locate_finds_rows_and_cell_columns() {
        let text = "the quick brown fox";
        assert_eq!(position(text, 10, 0), (0, 0));
        assert_eq!(position(text, 10, 9), (0, 9));
        assert_eq!(position(text, 10, 10), (1, 0));
        assert_eq!(position(text, 10, 19), (1, 9));
        assert_eq!(position(text, 10, 99), (1, 9));
    }

    #[test]
    fn locate_puts_the_position_after_a_newline_on_the_next_row() {
        let text = "ab\n\ncd";
        assert_eq!(position(text, 10, 2), (0, 2));
        assert_eq!(position(text, 10, 3), (1, 0));
        assert_eq!(position(text, 10, 4), (2, 0));
        assert_eq!(position(text, 10, 6), (2, 2));
    }

    #[test]
    fn locate_tolerates_missing_lines() {
        let graphemes = graphemes("abc");
        assert_eq!(locate(&graphemes, &[], 2), (0, 2));
    }

    const PIECES: [&str; 18] = [
        "a",
        "word",
        "longerword",
        "averyveryverylongword",
        " ",
        "  ",
        "\n",
        "\n    ",
        "日",
        "é",
        "e\u{301}",
        "\u{200b}",
        "👍🏽",
        " ?",
        "!",
        ".",
        "»",
        "\u{a0}",
    ];

    fn random_text(rng: &mut StdRng) -> String {
        let len = rng.random_range(0..40);
        (0..len)
            .map(|_| PIECES[rng.random_range(0..PIECES.len())])
            .collect()
    }

    fn row_width(width: u16) -> usize {
        usize::from(width.max(1))
    }

    fn assert_rows_tile_the_text(graphemes: &[String], lines: &[VisualLine], context: &str) {
        assert_eq!(lines.first().map(|line| line.start), Some(0), "{context}");
        assert_eq!(
            lines.last().map(|line| line.end),
            Some(graphemes.len()),
            "{context}"
        );
        for (row, pair) in lines.windows(2).enumerate() {
            assert_eq!(pair[0].end, pair[1].start, "{context}: gap after row {row}");
            assert!(pair[0].start < pair[0].end, "{context}: empty row {row}");
        }
    }

    fn assert_rows_fit(graphemes: &[String], width: u16, lines: &[VisualLine], context: &str) {
        for (row, line) in lines.iter().enumerate() {
            let content = &graphemes[line.start..line.end];
            assert!(
                cells(content) <= row_width(width) || content.len() == 1,
                "{context}: row {row} too wide"
            );
            let inner = content.iter().rev().skip(1);
            assert!(
                inner.clone().all(|grapheme| !is_newline(grapheme)),
                "{context}: newline inside row {row}"
            );
        }
    }

    fn assert_logical_numbering(graphemes: &[String], lines: &[VisualLine], context: &str) {
        let mut expected = 1;
        for (row, line) in lines.iter().enumerate() {
            let starts_line = row == 0 || graphemes[line.start - 1] == "\n";
            assert_eq!(
                line.number,
                starts_line.then_some(expected),
                "{context}: row {row}"
            );
            expected += usize::from(starts_line);
        }
    }

    fn word_ranges(graphemes: &[String]) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut start = 0;
        while start < graphemes.len() {
            let len = count_while(&graphemes[start..], is_word_part);
            if len > 0 {
                ranges.push((start, start + len));
            }
            start += len.max(1);
        }
        ranges
    }

    fn row_of(lines: &[VisualLine], index: usize) -> usize {
        lines.partition_point(|line| line.end <= index)
    }

    fn assert_fitting_words_stay_whole(
        graphemes: &[String],
        width: u16,
        lines: &[VisualLine],
        context: &str,
    ) {
        for (start, end) in word_ranges(graphemes) {
            if cells(&graphemes[start..end]) <= row_width(width) {
                assert_eq!(
                    row_of(lines, start),
                    row_of(lines, end - 1),
                    "{context}: word {start}..{end} was broken"
                );
            }
        }
    }

    fn assert_early_breaks_fall_between_words(
        graphemes: &[String],
        width: u16,
        lines: &[VisualLine],
        context: &str,
    ) {
        for (row, line) in lines.iter().enumerate() {
            let Some(next) = graphemes.get(line.end) else {
                continue;
            };
            let last = &graphemes[line.end - 1];
            let room_left =
                cells(&graphemes[line.start..line.end]) + cell_width(next) <= row_width(width);
            if room_left && !is_newline(last) {
                assert!(
                    is_space(last) && !is_space(next),
                    "{context}: row {row} ends early inside a word or a run of spaces"
                );
            }
        }
    }

    fn assert_end_cursor_has_room(
        graphemes: &[String],
        width: u16,
        lines: &[VisualLine],
        context: &str,
    ) {
        let (row, column) = locate(graphemes, lines, graphemes.len());
        assert_eq!(row + 1, lines.len(), "{context}");
        assert!(
            column < row_width(width),
            "{context}: end cursor off the row"
        );
    }

    fn assert_locate_matches_rows(
        graphemes: &[String],
        width: u16,
        lines: &[VisualLine],
        context: &str,
    ) {
        for index in 0..graphemes.len() {
            let (row, column) = locate(graphemes, lines, index);
            let line = lines[row];
            assert!(
                (line.start..line.end).contains(&index),
                "{context}: index {index} outside row {row}"
            );
            assert_eq!(column, cells(&graphemes[line.start..index]), "{context}");
            assert!(
                column == 0 || column + cell_width(&graphemes[index]) <= row_width(width),
                "{context}: index {index} drawn past the edge"
            );
        }
    }

    #[test]
    fn random_layouts_satisfy_every_invariant() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..2_000 {
            let graphemes = graphemes(&random_text(&mut rng));
            for width in 0..=14 {
                let lines = wrap(&graphemes, width);
                let context = format!("{:?} at width {width}", graphemes.concat());
                assert_rows_tile_the_text(&graphemes, &lines, &context);
                assert_rows_fit(&graphemes, width, &lines, &context);
                assert_logical_numbering(&graphemes, &lines, &context);
                assert_fitting_words_stay_whole(&graphemes, width, &lines, &context);
                assert_early_breaks_fall_between_words(&graphemes, width, &lines, &context);
                assert_end_cursor_has_room(&graphemes, width, &lines, &context);
                assert_locate_matches_rows(&graphemes, width, &lines, &context);
            }
        }
    }
}
