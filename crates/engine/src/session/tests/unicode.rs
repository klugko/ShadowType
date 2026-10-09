use super::*;

#[test]
fn graphemes_are_single_characters() {
    let now = Instant::now();
    let mut session = TypingSession::new("é👩‍💻x", SessionOptions::default());
    assert_eq!(session.target().len(), 3);
    type_text(&mut session, "é👩‍💻", now);
    assert_eq!(session.cursor(), 2);
    assert_eq!(session.mark(1), Mark::Correct);
    session.backspace(now);
    assert_eq!(session.cursor(), 1);
    assert_eq!(session.mark(0), Mark::Correct);
}

#[test]
fn decomposed_input_matches_precomposed_text() {
    let now = Instant::now();
    let mut session = TypingSession::new("café", SessionOptions::default());
    type_text(&mut session, "cafe\u{301}", now);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.stats(now).accuracy, 100.0);
    assert_eq!(session.stats(now).errors, 0);
}

#[test]
fn a_bare_letter_for_an_accented_one_is_an_error_once_the_player_moves_on() {
    let now = Instant::now();
    let mut session = TypingSession::new("été ok", SessionOptions::default());
    type_text(&mut session, "ete o", now);
    let stats = session.stats(now);
    assert_eq!(stats.errors, 2);
    assert!(stats.accuracy < 100.0);
}

#[test]
fn retyping_a_missing_accent_keeps_the_error() {
    let now = Instant::now();
    let mut session = TypingSession::new("été ok", SessionOptions::default());
    type_text(&mut session, "ete ok", now);
    for _ in 0..6 {
        session.backspace(now);
    }
    type_text(&mut session, "été ok", now);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.stats(now).errors, 2);
}

#[test]
fn erasing_an_unfinished_accent_counts_it_as_an_error() {
    let now = Instant::now();
    let mut session = TypingSession::new("é", SessionOptions::default());
    session.type_char('e', now);
    assert_eq!(session.stats(now).errors, 0, "the accent may still come");
    session.backspace(now);
    assert_eq!(session.stats(now).errors, 1);
}

#[test]
fn an_accent_still_missing_at_the_time_limit_is_an_error() {
    let start = Instant::now();
    let options = SessionOptions {
        time_limit: Some(Duration::from_secs(5)),
        ..SessionOptions::default()
    };
    let mut session = TypingSession::new("café", options);
    type_text(&mut session, "cafe", start);
    session.update(at(start, 5_000));
    assert_eq!(session.status(), Status::TimeUp);
    assert_eq!(session.stats(at(start, 5_000)).errors, 1);
}

#[test]
fn a_wrong_accent_is_a_single_error() {
    let now = Instant::now();
    let mut session = TypingSession::new("é", SessionOptions::default());
    type_text(&mut session, "e\u{300}", now);
    assert_eq!(session.mark(0), Mark::Incorrect);
    assert_eq!(session.stats(now).errors, 1);
    session.backspace(now);
    assert_eq!(session.stats(now).errors, 1);
}

#[test]
fn errors_never_decrease() {
    let now = Instant::now();
    let mut session = TypingSession::new("été où ça", SessionOptions::default());
    let mut highest = 0;
    let mut check = |session: &TypingSession| {
        let errors = session.stats(now).errors;
        assert!(errors >= highest, "{errors} < {highest}");
        highest = errors;
    };
    for ch in "e\u{301}tex o\u{300}".chars() {
        session.type_char(ch, now);
        check(&session);
    }
    while session.backspace(now) {
        check(&session);
    }
    for ch in "ete ou c".chars() {
        session.type_char(ch, now);
        check(&session);
    }
    assert!(session.delete_word(now));
    check(&session);
    assert_eq!(highest, 9);
}

#[test]
fn an_emoji_typed_code_point_by_code_point_has_no_errors() {
    let now = Instant::now();
    let mut session = TypingSession::new("👩‍💻!", SessionOptions::default());
    type_text(&mut session, "👩\u{200D}💻!", now);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.stats(now).errors, 0);
}

#[test]
fn combining_mark_on_a_wrong_base_is_an_error() {
    let now = Instant::now();
    let mut session = TypingSession::new("ab", SessionOptions::default());
    type_text(&mut session, "a\u{301}", now);
    assert_eq!(session.cursor(), 1);
    assert_eq!(session.mark(0), Mark::Incorrect);
    assert_eq!(session.stats(now).errors, 1);
}

#[test]
fn grapheme_count_matches_session_length() {
    let text = "naïve 👩‍💻 cafe\u{301}";
    assert_eq!(
        grapheme_count(text),
        TypingSession::new(text, SessionOptions::default())
            .target()
            .len()
    );
}
