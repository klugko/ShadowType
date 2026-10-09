use std::fs;

use chrono::DateTime;

use super::*;
use crate::{history::file::legacy_mode, persist::scratch::TempDir};

#[test]
fn version_one_entries_are_migrated() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let legacy_stats = r#"{"wpm":62.5,"raw_wpm":70.0,"accuracy":96.0,"errors":4,
        "correct":240,"attempts":250,"position":250,"length":250,"elapsed":48.0}"#;
    let entries: Vec<String> = [
        ("words/50", "english"),
        ("time/30", "french"),
        ("quote/50", "english"),
        ("code/50", "rust"),
        ("multiplayer", "english"),
    ]
    .iter()
    .map(|(mode, language)| {
        format!(
            r#"{{"date":"2025-05-01T10:00:00.123456+00:00","mode":"{mode}","language":"{language}","stats":{legacy_stats}}}"#
        )
    })
    .collect();
    fs::write(&path, format!("[{}]", entries.join(","))).expect("write");

    let loaded = History::load(&path);

    assert_eq!(loaded.warning, None);
    let modes: Vec<&str> = loaded
        .value
        .records()
        .iter()
        .map(|record| record.mode.as_str())
        .collect();
    assert_eq!(modes, ["words 50", "time 30", "quote", "code", "race"]);
    let first = &loaded.value.records()[0];
    assert_eq!(
        (first.wpm, first.raw_wpm, first.accuracy, first.errors),
        (62.5, 70.0, 96.0, 4)
    );
    assert_eq!((first.duration, first.text_length), (48.0, 250));
    assert_eq!(
        first.date,
        DateTime::parse_from_rfc3339("2025-05-01T10:00:00.123456Z").expect("date")
    );
}

#[test]
fn migrated_history_is_saved_in_the_current_format() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let legacy = r#"[{"date":"2025-05-01T10:00:00+00:00","mode":"words/25","language":"french",
        "stats":{"wpm":50.0,"raw_wpm":55.0,"accuracy":93.0,"errors":7,"length":140,"elapsed":30.5}}]"#;
    fs::write(&path, legacy).expect("write");

    let mut history = History::load(&path).value;
    history
        .add(record("words 25", "french", 52.0, 94.0))
        .expect("add");

    let saved: Vec<Record> =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("current format");
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].mode, "words 25");
    assert_eq!(saved[0].duration, 30.5);
}

#[test]
fn unknown_legacy_modes_lose_their_slash() {
    assert_eq!(legacy_mode("zen/0"), "zen 0");
    assert_eq!(legacy_mode("multiplayer"), "race");
}

#[test]
fn migrated_code_sessions_count_towards_the_personal_best() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let legacy = r#"[{"date":"2025-05-01T10:00:00+00:00","mode":"code/100","language":"rust",
        "stats":{"wpm":71.0,"raw_wpm":75.0,"accuracy":97.0,"errors":3,"length":300,"elapsed":51.0}}]"#;
    fs::write(&path, legacy).expect("write");

    let history = History::load(&path).value;

    assert_eq!(history.records()[0].mode, "code");
    assert_eq!(history.records()[0].language, "rust");
    assert_eq!(history.personal_best("code", "rust"), Some(71.0));
}

#[test]
fn code_sessions_saved_with_their_language_in_the_mode_are_normalised() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let saved = [record("code python", "python", 58.0, 94.0)];
    fs::write(&path, serde_json::to_vec(&saved).expect("json")).expect("write");

    let history = History::load(&path).value;

    assert_eq!(history.personal_best("code", "python"), Some(58.0));
}
