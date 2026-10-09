use code_racer_engine::{CodeLanguage, Mark, SessionOptions, TypingSession};
use code_racer_protocol::Phase;

use super::*;

#[test]
fn progress_is_refused_outside_a_race() {
    let clock = Clock(Instant::now());
    let mut room = lobby(&clock, &[BOB], 8);
    assert_eq!(
        error_code(room.report_progress(BOB, clean(1), clock.at(1))),
        Some(ErrorCode::RaceNotRunning)
    );

    let mut room = counting_down(&clock, &[BOB]);
    assert_eq!(
        error_code(room.report_progress(BOB, clean(1), clock.at(2_999))),
        Some(ErrorCode::RaceNotRunning)
    );
    assert_eq!(
        error_code(room.report_progress(STRANGER, clean(1), clock.racing(1))),
        Some(ErrorCode::NotInRoom)
    );
}

#[test]
fn progress_after_the_deadline_starts_the_race_without_waiting_for_a_tick() {
    let clock = Clock(Instant::now());
    let mut room = counting_down(&clock, &[BOB]);
    room.report_progress(BOB, clean(5), clock.racing(500))
        .expect("progress");
    assert_eq!(room.view().phase, Phase::Racing);
    assert_eq!(player(&room, BOB).progress.correct, 5);
}

#[test]
fn inconsistent_counters_are_rejected_without_changing_anything() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    let accepted = progress(20, 18, 25, 4);
    room.report_progress(BOB, accepted, clock.racing(10_000))
        .expect("valid progress");
    let before = player(&room, BOB);

    let invalid = [
        progress(TEXT_LENGTH + 1, 20, 200, 4),
        progress(20, 21, 30, 4),
        progress(20, 18, 30, 31),
        progress(31, 18, 30, 4),
        progress(20, 18, 24, 4),
        progress(20, 18, 30, 3),
    ];
    for counters in invalid {
        let result = room.report_progress(BOB, counters, clock.racing(11_000));
        assert_eq!(
            error_code(result),
            Some(ErrorCode::InvalidProgress),
            "{counters:?}"
        );
        assert_eq!(player(&room, BOB), before, "{counters:?}");
    }
}

#[test]
fn typing_faster_than_thirty_characters_per_second_is_implausible() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    assert_eq!(
        error_code(room.report_progress(BOB, clean(6), clock.racing(0))),
        Some(ErrorCode::InvalidProgress)
    );
    room.report_progress(BOB, clean(5), clock.racing(0))
        .expect("a short burst");
    assert_eq!(
        error_code(room.report_progress(BOB, clean(36), clock.racing(1_000))),
        Some(ErrorCode::InvalidProgress)
    );
    room.report_progress(BOB, clean(35), clock.racing(1_000))
        .expect("thirty characters per second and a burst");
    assert_eq!(player(&room, BOB).progress.correct, 35);
}

#[test]
fn a_short_race_cannot_be_finished_instantly() {
    let clock = Clock(Instant::now());
    let text = "a".repeat(30);
    let mut room = racing_on(&clock, &text);
    assert_eq!(
        error_code(room.report_progress(ALICE, clean(30), clock.racing(10))),
        Some(ErrorCode::InvalidProgress)
    );
    room.report_progress(ALICE, clean(30), clock.racing(1_000))
        .expect("thirty characters in a second");
    assert_eq!(player(&room, ALICE).progress.finish_ms, Some(1_000));
}

#[test]
fn speed_and_accuracy_come_from_the_server_clock() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.report_progress(BOB, progress(52, 50, 55, 5), clock.racing(12_000))
        .expect("progress");
    let progress = player(&room, BOB).progress;
    assert_eq!(progress.typed, 52);
    assert_eq!(progress.correct, 50);
    assert_eq!(progress.errors, 5);
    assert!((progress.wpm - 50.0).abs() < 1e-9, "{}", progress.wpm);
    assert!((progress.accuracy - 50.0 / 55.0 * 100.0).abs() < 1e-9);
    assert_eq!(progress.finish_ms, None);
}

#[test]
fn reported_indentation_must_be_indentation_of_the_text() {
    let clock = Clock(Instant::now());
    let mut room = racing_on(&clock, "{\n    x\n}");
    let invalid = [
        indented(6, 6, 5, 2),
        indented(3, 3, 2, 2),
        indented(6, 3, 4, 3),
        indented(6, 6, 4, 1),
    ];
    for counters in invalid {
        assert_eq!(
            error_code(room.report_progress(ALICE, counters, clock.racing(5_000))),
            Some(ErrorCode::InvalidProgress),
            "{counters:?}"
        );
    }
    room.report_progress(ALICE, indented(6, 6, 4, 2), clock.racing(5_000))
        .expect("the newline filled in four spaces");
    assert_eq!(player(&room, ALICE).progress.typed, 6);
}

#[test]
fn auto_filled_indentation_counts_for_finishing_but_not_for_speed_or_accuracy() {
    let clock = Clock(Instant::now());
    let mut room = racing_on(&clock, "{\n    x\n}");
    let finished = Progress {
        errors: 1,
        ..indented(9, 9, 4, 6)
    };
    room.report_progress(ALICE, finished, clock.racing(2_000))
        .expect("finish");
    let progress = player(&room, ALICE).progress;
    assert_eq!(progress.finish_ms, Some(2_000));
    assert_eq!(
        progress.wpm,
        code_racer_engine::words_per_minute(5, Duration::from_secs(2))
    );
    assert_eq!(progress.accuracy, 5.0 / 6.0 * 100.0);
}

#[test]
fn a_code_race_is_scored_like_the_session_that_typed_it() {
    let clock = Clock(Instant::now());
    let snippet = TextSource::Code {
        language: CodeLanguage::Python,
    }
    .generate(3)
    .text;
    let mut room = racing_on(&clock, &snippet);
    let options = SessionOptions {
        auto_indent: true,
        ..SessionOptions::default()
    };
    let mut session = TypingSession::new(&snippet, options);
    session.start(clock.racing(0));
    let mut now = clock.racing(0);
    let mut keys = 0;
    while let Some(expected) = session.target().get(session.cursor()).cloned() {
        let before = session.cursor();
        if keys == 3 {
            session.type_char('#', now);
            session.backspace(now);
        }
        keys += 1;
        now = clock.racing(keys * 100);
        for ch in expected.chars() {
            assert!(session.type_char(ch, now), "{ch:?} refused at {before}");
        }
        assert_eq!(
            session.mark(before),
            Mark::Correct,
            "{expected:?} at {before}"
        );
        room.report_progress(ALICE, Progress::from(session.tally()), now)
            .expect("an honest report");
    }
    let stats = session.stats(now);
    let progress = player(&room, ALICE).progress;
    assert_eq!(progress.finish_ms, Some(keys * 100));
    assert_eq!(progress.wpm, stats.wpm);
    assert_eq!(progress.accuracy, stats.accuracy);
    assert!(stats.indentation > 0, "the snippet is indented");
    assert!(stats.accuracy < 100.0);
}

#[test]
fn a_player_finishes_when_the_whole_text_is_correct_and_further_reports_are_ignored() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.report_progress(BOB, progress(TEXT_LENGTH, 99, 100, 1), clock.racing(15_000))
        .expect("one mistake left");
    assert_eq!(player(&room, BOB).progress.finish_ms, None);

    room.report_progress(
        BOB,
        progress(TEXT_LENGTH, TEXT_LENGTH, 102, 1),
        clock.racing(20_250),
    )
    .expect("finish");
    let finished = player(&room, BOB);
    assert_eq!(finished.progress.finish_ms, Some(20_250));

    room.report_progress(BOB, progress(0, 0, 0, 0), clock.racing(21_000))
        .expect("ignored");
    assert_eq!(player(&room, BOB), finished);
}
