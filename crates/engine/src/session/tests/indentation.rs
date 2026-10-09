use super::*;
use crate::stats::words_per_minute;

#[test]
fn auto_indent_skips_leading_spaces() {
    let now = Instant::now();
    let mut session = auto_indented("{\n    x\n}");
    type_text(&mut session, "{\n", now);
    assert_eq!(session.cursor(), 6, "cursor sits on `x`");
    type_text(&mut session, "x\n}", now);
    let stats = session.stats(now);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(stats.correct_chars, 9);
    assert_eq!(stats.indentation, 4);
    assert_eq!(stats.keystrokes, 5, "indentation is not typed");
    assert_eq!(stats.accuracy, 100.0);
}

#[test]
fn auto_filled_indentation_counts_for_completion_but_not_for_speed() {
    let start = Instant::now();
    let mut session = auto_indented("fn f() {\n    x();\n}");
    let end = play(&mut session, "fn f() {\nx();\n}", start, 100);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(
        session.tally(),
        Tally {
            typed: 19,
            correct: 19,
            indentation: 4,
            keystrokes: 15,
            errors: 0,
        }
    );
    let stats = session.stats(end);
    assert_eq!(stats.progress, 1.0);
    let fifteen_keys = words_per_minute(15, Duration::from_millis(1_400));
    assert_eq!(stats.wpm, fifteen_keys);
    assert_eq!(stats.raw_wpm, fifteen_keys);
    assert_eq!(stats.accuracy, 100.0);
}

#[test]
fn auto_filled_indentation_does_not_count_towards_the_error_run() {
    let now = Instant::now();
    let text = format!("ab\n{}{}", " ".repeat(12), "c".repeat(20));
    let mut session = auto_indented(&text);
    type_text(&mut session, "ax\n", now);
    assert!(
        !session.is_blocked(),
        "two characters typed since the mistake"
    );
    for _ in 2..ERROR_RUN_LIMIT {
        assert!(session.type_char('c', now));
    }
    assert!(session.is_blocked());
    assert!(!session.type_char('c', now));
}

#[test]
fn backspace_undoes_auto_indent_with_its_newline() {
    let now = Instant::now();
    let mut session = auto_indented("a\n  b");
    type_text(&mut session, "a\n", now);
    assert_eq!(session.cursor(), 4);
    session.backspace(now);
    assert_eq!(session.cursor(), 1);
}

#[test]
fn wrong_newline_does_not_auto_indent() {
    let now = Instant::now();
    let mut session = auto_indented("a b\n  c");
    type_text(&mut session, "a\n", now);
    assert_eq!(session.cursor(), 2);
    assert_eq!(session.mark(1), Mark::Incorrect);
}
