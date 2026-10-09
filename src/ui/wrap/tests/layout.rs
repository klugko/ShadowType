use super::*;

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
