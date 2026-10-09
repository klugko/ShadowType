use super::*;

#[test]
fn delete_word_removes_the_last_word() {
    let now = Instant::now();
    let mut session = TypingSession::new("one two three", SessionOptions::default());
    type_text(&mut session, "one two", now);
    assert!(session.delete_word(now));
    assert_eq!(session.cursor(), 4);
    assert!(session.delete_word(now));
    assert_eq!(session.cursor(), 0);
    assert!(!session.delete_word(now));
}

#[test]
fn delete_word_after_an_auto_indented_newline_keeps_the_previous_line() {
    let now = Instant::now();
    let mut session = auto_indented("fn main() {\n    x\n}");
    type_text(&mut session, "fn main() {\n", now);
    assert_eq!(session.cursor(), 16);
    assert!(session.delete_word(now));
    assert_eq!(
        session.cursor(),
        11,
        "only the line break and its indentation go"
    );
    assert_eq!(session.mark(10), Mark::Correct, "the brace stays typed");
}

#[test]
fn delete_word_never_crosses_a_line_break() {
    let now = Instant::now();
    let mut session = TypingSession::new("ab\n  cd ef", SessionOptions::default());
    type_text(&mut session, "ab\n  cd", now);
    assert!(session.delete_word(now));
    assert_eq!(
        session.cursor(),
        5,
        "the word, not the indentation before it"
    );
    assert!(session.delete_word(now));
    assert_eq!(
        session.cursor(),
        2,
        "the typed indentation and its line break"
    );
    assert!(session.delete_word(now));
    assert_eq!(session.cursor(), 0);
}

#[test]
fn delete_word_right_after_a_newline_removes_only_the_newline() {
    let now = Instant::now();
    let mut session = TypingSession::new("ab\ncd", SessionOptions::default());
    type_text(&mut session, "ab\n", now);
    assert!(session.delete_word(now));
    assert_eq!(session.cursor(), 2);
}
