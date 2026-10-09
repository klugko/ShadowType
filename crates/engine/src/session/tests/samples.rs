use super::*;
use crate::stats::{consistency, words_per_minute};

#[test]
fn the_last_sample_matches_the_headline_and_samples_add_up_to_its_errors() {
    let start = Instant::now();
    let mut session = auto_indented("if ok {\n    go();\n}");
    let end = play(&mut session, "if oj\u{8}k {\ngo();\n}", start, 200);
    assert_eq!(session.status(), Status::Completed);
    let stats = session.stats(end);
    let samples = session.samples(end);
    assert_eq!(samples.len(), 3);
    assert_eq!(samples.last().map(|sample| sample.wpm), Some(stats.wpm));
    let errors: u32 = samples.iter().map(|sample| sample.errors).sum();
    assert_eq!(errors as usize, stats.errors);
    assert_eq!(stats.errors, 1);
}

#[test]
fn deleted_characters_do_not_count_towards_the_samples() {
    let start = Instant::now();
    let mut session = TypingSession::new("ab cd", SessionOptions::default());
    play(&mut session, "ab\u{8}\u{8}ab", start, 100);
    let second = at(start, 1_000);
    let two_characters = words_per_minute(2, Duration::from_secs(1));
    assert_eq!(session.stats(second).wpm, two_characters);
    let samples = session.samples(second);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].wpm, two_characters);
    assert_eq!(
        samples[0].raw_wpm,
        words_per_minute(4, Duration::from_secs(1))
    );
}

#[test]
fn a_session_shorter_than_a_second_has_one_steady_sample() {
    let start = Instant::now();
    let mut session = TypingSession::new("hello", SessionOptions::default());
    let end = play(&mut session, "hello", start, 150);
    let samples = session.samples(end);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].wpm, session.stats(end).wpm);
    assert_eq!(consistency(&samples), 100.0);
}

#[test]
fn samples_cover_seconds_and_the_last_one_takes_a_short_remainder() {
    let start = Instant::now();
    let mut session = TypingSession::new(&"a".repeat(40), SessionOptions::default());
    session.start(start);
    for i in 0..10 {
        session.type_char('a', at(start, 100 * i));
    }
    session.type_char('x', at(start, 1_500));
    for i in 0..5 {
        session.type_char('a', at(start, 2_000 + 100 * i));
    }
    let end = at(start, 3_200);
    let samples = session.samples(end);
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].raw_wpm, 120.0);
    assert_eq!(samples[1].errors, 1);
    assert_eq!(
        samples[2].raw_wpm,
        words_per_minute(4, Duration::from_millis(1_200))
    );
    assert_eq!(
        samples[2].wpm,
        words_per_minute(15, Duration::from_millis(3_200))
    );
    assert_eq!(samples[2].wpm, session.stats(end).wpm);
}
