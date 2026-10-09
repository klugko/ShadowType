use std::{fs, time::Duration};

use super::*;
use crate::persist::scratch::TempDir;

#[test]
fn a_day_counts_its_sessions_and_their_time() {
    let history = practised_on(&["2026-05-01", "2026-05-01", "2026-05-02"]);
    let (sessions, time) = history.day(date("2026-05-01"));
    assert_eq!((sessions, time), (2, Duration::from_secs(60)));
    assert_eq!(history.day(date("2026-05-03")).0, 0);
}

#[test]
fn a_streak_counts_the_days_in_a_row() {
    let history = practised_on(&["2026-05-01", "2026-05-03", "2026-05-04", "2026-05-05"]);
    assert_eq!(history.streak(date("2026-05-05")), 3);
    assert_eq!(
        history.streak(date("2026-05-06")),
        3,
        "today may still come"
    );
    assert_eq!(history.streak(date("2026-05-07")), 0, "a day was missed");
    assert_eq!(History::in_memory().streak(date("2026-05-07")), 0);
}

#[test]
fn empty_history_summary_is_all_zeros() {
    let summary = History::in_memory().summary();
    assert_eq!(summary, Summary::default());
    assert!(!summary.average_wpm.is_nan() && !summary.recent_wpm.is_nan());
}

#[test]
fn summary_aggregates_speed_accuracy_and_time() {
    let mut history = with_speeds([10.0, 20.0, 30.0, 40.0, 50.0, 60.0]);
    history
        .add(Record {
            duration: 90.0,
            accuracy: 99.5,
            ..record("quote", "french", 35.0, 0.0)
        })
        .expect("add");

    let summary = history.summary();

    assert_eq!(summary.sessions, 7);
    assert_eq!(summary.best_wpm, 60.0);
    assert_eq!(summary.average_wpm, 35.0);
    assert_eq!(summary.best_accuracy, 99.5);
    assert_eq!(summary.total_time, Duration::from_secs(6 * 30 + 90));
}

#[test]
fn recent_speed_only_counts_the_last_ten_sessions() {
    let history = with_speeds([1000.0, 1000.0].into_iter().chain([50.0; RECENT_SESSIONS]));
    let summary = history.summary();
    assert_eq!(summary.recent_wpm, 50.0);
    assert!(summary.average_wpm > 50.0);
}

#[test]
fn personal_best_matches_mode_and_language() {
    let mut history = History::in_memory();
    for (mode, language, wpm) in [
        ("words 50", "english", 70.0),
        ("words 50", "english", 82.0),
        ("words 50", "french", 95.0),
        ("words 25", "english", 99.0),
    ] {
        history.add(record(mode, language, wpm, 95.0)).expect("add");
    }

    assert_eq!(history.personal_best("words 50", "english"), Some(82.0));
    assert_eq!(history.personal_best("words 50", "french"), Some(95.0));
    assert_eq!(history.personal_best("code", "rust"), None);
}

#[test]
fn hand_edited_numbers_keep_the_summary_finite() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let records = [
        Record {
            duration: -30.0,
            accuracy: -5.0,
            ..record("words 50", "english", -50.0, 0.0)
        },
        Record {
            duration: 30.0,
            ..record("words 50", "english", 80.0, 96.0)
        },
    ];
    fs::write(&path, serde_json::to_vec(&records).expect("json")).expect("write");

    let summary = History::load(&path).value.summary();

    assert_eq!(summary.average_wpm, 40.0);
    assert_eq!(summary.best_wpm, 80.0);
    assert_eq!(summary.best_accuracy, 96.0);
    assert_eq!(summary.total_time, Duration::from_secs(30));
}
