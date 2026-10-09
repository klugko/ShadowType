use super::*;

#[test]
fn completes_when_everything_is_correct() {
    let start = Instant::now();
    let mut session = TypingSession::new("hello world", SessionOptions::default());
    assert_eq!(session.status(), Status::NotStarted);
    type_text(&mut session, "hello world", at(start, 6_000));
    assert_eq!(session.status(), Status::Completed);
    assert!(!session.type_char('!', at(start, 7_000)));
}

#[test]
fn computes_wpm_raw_and_accuracy() {
    let start = Instant::now();
    let mut session = TypingSession::new("abcde abcde", SessionOptions::default());
    session.start(start);
    type_text(&mut session, "abx", at(start, 1_000));
    session.backspace(at(start, 1_500));
    type_text(&mut session, "cde abcde", at(start, 12_000));
    let stats = session.stats(at(start, 12_000));
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(stats.correct_chars, 11);
    assert_eq!(stats.keystrokes, 12);
    assert_eq!(stats.errors, 1);
    assert!((stats.wpm - 11.0).abs() < 1e-9);
    assert!((stats.raw_wpm - 12.0).abs() < 1e-9);
    assert!((stats.accuracy - 11.0 / 12.0 * 100.0).abs() < 1e-9);
    assert_eq!(stats.progress, 1.0);
}

#[test]
fn empty_session_reports_neutral_stats() {
    let stats = TypingSession::new("abc", SessionOptions::default()).stats(Instant::now());
    assert_eq!(stats.wpm, 0.0);
    assert_eq!(stats.raw_wpm, 0.0);
    assert_eq!(stats.accuracy, 100.0);
    assert_eq!(stats.elapsed, Duration::ZERO);
}

#[test]
fn mistakes_must_be_fixed_to_complete() {
    let now = Instant::now();
    let mut session = TypingSession::new("ab", SessionOptions::default());
    type_text(&mut session, "xb", now);
    assert_eq!(session.status(), Status::Running);
    assert_eq!(session.mark(0), Mark::Incorrect);
    assert_eq!(session.mark(1), Mark::Correct);
    assert!(!session.type_char('c', now), "no input past the end");
    session.backspace(now);
    session.backspace(now);
    type_text(&mut session, "ab", now);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.stats(now).errors, 1);
}

#[test]
fn progress_is_the_share_of_the_text_typed_correctly() {
    let now = Instant::now();
    let mut session = TypingSession::new("abcd", SessionOptions::default());
    type_text(&mut session, "abxd", now);
    assert_eq!(session.status(), Status::Running);
    assert_eq!(
        session.stats(now).progress,
        0.75,
        "the mistake does not count"
    );
    session.backspace(now);
    session.backspace(now);
    type_text(&mut session, "cd", now);
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.stats(now).progress, 1.0);
}

#[test]
fn input_is_blocked_after_a_long_uncorrected_run() {
    let now = Instant::now();
    let text = "a".repeat(30);
    let mut session = TypingSession::new(&text, SessionOptions::default());
    session.type_char('x', now);
    for _ in 1..ERROR_RUN_LIMIT {
        assert!(session.type_char('a', now));
    }
    assert!(session.is_blocked());
    assert!(!session.type_char('a', now));
    assert_eq!(session.stats(now).keystrokes, ERROR_RUN_LIMIT);
    session.backspace(now);
    assert!(!session.is_blocked());
}

#[test]
fn input_is_blocked_at_the_end_of_a_text_with_a_mistake_left() {
    let now = Instant::now();
    let mut session = TypingSession::new("say hello", SessionOptions::default());
    type_text(&mut session, "say hellp", now);
    assert!(session.is_blocked(), "nothing left to type but a mistake");
    assert!(!session.type_char('o', now));
    assert!(!session.is_finished());
    session.backspace(now);
    assert!(!session.is_blocked());
    assert!(session.type_char('o', now));
    assert_eq!(session.status(), Status::Completed);
}

#[test]
fn time_limit_freezes_the_clock_at_the_limit() {
    let start = Instant::now();
    let options = SessionOptions {
        time_limit: Some(Duration::from_secs(15)),
        ..SessionOptions::default()
    };
    let mut session = TypingSession::new("some words to type", options);
    session.type_char('s', start);
    assert_eq!(
        session.time_left(at(start, 5_000)),
        Some(Duration::from_secs(10))
    );
    assert!(!session.type_char('o', at(start, 15_001)));
    assert_eq!(session.status(), Status::TimeUp);
    assert_eq!(session.elapsed(at(start, 60_000)), Duration::from_secs(15));
    assert_eq!(session.stats(at(start, 60_000)).progress, 1.0);
}

#[test]
fn extend_appends_to_the_target() {
    let now = Instant::now();
    let mut session = TypingSession::new("ab", SessionOptions::default());
    session.extend(" cd");
    type_text(&mut session, "ab c", now);
    assert_eq!(session.remaining(), 1);
    assert_eq!(session.status(), Status::Running);
}
