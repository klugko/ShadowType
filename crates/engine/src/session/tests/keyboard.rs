use super::*;

fn completes_without_errors(text: &str, typed: &str) {
    let now = Instant::now();
    let mut session = TypingSession::new(text, SessionOptions::default());
    type_text(&mut session, typed, now);
    assert_eq!(
        session.status(),
        Status::Completed,
        "{typed:?} for {text:?}"
    );
    assert_eq!(session.stats(now).errors, 0, "{typed:?} for {text:?}");
}

#[test]
fn typed_spaces_of_any_kind_match_the_plain_space_of_the_text() {
    completes_without_errors("Vraiment ?", "Vraiment\u{a0}?");
    completes_without_errors("Vraiment ?", "Vraiment\u{202f}?");
}

#[test]
fn typed_typographic_characters_match_their_plain_form_in_the_text() {
    completes_without_errors("l'\u{e9}t\u{e9}", "l\u{2019}\u{e9}t\u{e9}");
    completes_without_errors("\"a\" - b", "\u{ab}a\u{bb} \u{2013} b");
    completes_without_errors("a...", "a\u{2026}");
    completes_without_errors("a -> b", "a \u{2192} b");
}

#[test]
fn typed_invisible_characters_are_refused() {
    let now = Instant::now();
    let mut session = TypingSession::new("ab", SessionOptions::default());
    session.type_char('a', now);
    assert!(!session.type_char('\u{200b}', now));
    assert_eq!(session.cursor(), 1);
    assert_eq!(session.stats(now).errors, 0);
}

#[test]
fn control_characters_are_ignored() {
    let now = Instant::now();
    let mut session = TypingSession::new("a", SessionOptions::default());
    assert!(!session.type_char('\u{7}', now));
    assert_eq!(session.status(), Status::NotStarted);
}
