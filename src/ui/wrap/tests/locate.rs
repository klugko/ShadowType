use super::*;

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
