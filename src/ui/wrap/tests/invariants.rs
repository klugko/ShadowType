use rand::{RngExt, SeedableRng, rngs::StdRng};

use super::*;

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
