use std::{fs, io};

use chrono::DateTime;

use super::*;
use crate::persist::scratch::{TempDir, occupy_every_backup};

#[test]
fn records_survive_a_reload_oldest_first() {
    let dir = TempDir::new();
    let path = dir.join("data/history.json");
    let mut history = History::load(&path).value;
    history
        .add(record("words 50", "english", 61.5, 97.0))
        .expect("add");
    history
        .add(record("code", "rust", 48.0, 91.0))
        .expect("add");

    let reloaded = History::load(&path);

    assert_eq!(reloaded.warning, None);
    assert_eq!(reloaded.value.records(), history.records());
    assert_eq!(reloaded.value.records()[0].mode, "words 50");
}

#[test]
fn dates_are_stored_as_rfc3339() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let mut history = History::load(&path).value;
    history
        .add(record("quote", "french", 70.0, 99.0))
        .expect("add");

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
    let date = json[0]["date"].as_str().expect("date string");

    assert_eq!(
        DateTime::parse_from_rfc3339(date).expect("rfc 3339"),
        history.records()[0].date
    );
}

#[test]
fn missing_file_is_an_empty_history() {
    let dir = TempDir::new();
    let loaded = History::load(&dir.join("history.json"));
    assert_eq!(loaded.warning, None);
    assert!(loaded.value.records().is_empty());
}

#[test]
fn in_memory_history_never_writes() {
    let mut history = History::in_memory();
    history
        .add(record("time 30", "english", 80.0, 96.0))
        .expect("add");
    assert_eq!(history.records().len(), 1);
    assert_eq!(history.path, None);
}

#[test]
fn only_the_newest_records_are_kept() {
    let history = with_speeds((0..MAX_RECORDS + 5).map(|index| index as f64));
    assert_eq!(history.records().len(), MAX_RECORDS);
    assert_eq!(history.records()[0].wpm, 5.0);
    assert_eq!(
        history.records().last().map(|record| record.wpm),
        Some((MAX_RECORDS + 4) as f64)
    );
}

#[test]
fn oversized_files_are_trimmed_on_load() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let records: Vec<Record> = (0..MAX_RECORDS + 3)
        .map(|index| record("quote", "english", index as f64, 90.0))
        .collect();
    fs::write(&path, serde_json::to_vec(&records).expect("json")).expect("write");

    let history = History::load(&path).value;

    assert_eq!(history.records().len(), MAX_RECORDS);
    assert_eq!(history.records()[0].wpm, 3.0);
}

#[test]
fn non_finite_numbers_are_stored_as_zero() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let mut history = History::load(&path).value;
    history
        .add(Record {
            raw_wpm: f64::INFINITY,
            accuracy: f64::NAN,
            ..record("words 10", "english", 40.0, 0.0)
        })
        .expect("add");

    let reloaded = History::load(&path);

    assert_eq!(reloaded.warning, None);
    let saved = &reloaded.value.records()[0];
    assert_eq!((saved.wpm, saved.raw_wpm, saved.accuracy), (40.0, 0.0, 0.0));
}

#[test]
fn failed_save_keeps_the_record_in_memory() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let mut history = History::load(&path).value;
    fs::create_dir(&path).expect("put a directory in the way");

    let saved = history.add(record("quote", "english", 66.0, 97.0));

    assert!(saved.is_err());
    assert_eq!(history.records().len(), 1);
    assert_eq!(history.personal_best("quote", "english"), Some(66.0));
}

#[test]
fn two_running_instances_keep_each_others_results() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let mut practising = History::load(&path).value;
    let mut racing = History::load(&path).value;

    practising
        .add(record("words 50", "english", 61.0, 97.0))
        .expect("add");
    racing
        .add(record("race", "french", 72.0, 95.0))
        .expect("add");
    practising
        .add(record("quote", "english", 55.0, 99.0))
        .expect("add");

    let speeds = |history: &History| -> Vec<f64> {
        history.records().iter().map(|record| record.wpm).collect()
    };
    let reloaded = History::load(&path).value;
    assert_eq!(speeds(&reloaded), [61.0, 72.0, 55.0]);
    assert_eq!(speeds(&practising), [61.0, 72.0, 55.0]);
    assert_eq!(racing.personal_best("words 50", "english"), Some(61.0));
}

#[test]
fn a_result_that_could_not_be_saved_is_saved_with_the_next_one() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let mut history = History::load(&path).value;
    fs::create_dir(&path).expect("put a directory in the way");
    assert!(history.add(record("quote", "english", 40.0, 90.0)).is_err());
    fs::remove_dir(&path).expect("clear the way");

    history
        .add(record("quote", "english", 50.0, 92.0))
        .expect("add");

    let saved: Vec<f64> = History::load(&path)
        .value
        .records()
        .iter()
        .map(|record| record.wpm)
        .collect();
    assert_eq!(saved, [40.0, 50.0]);
    assert_eq!(history.records().len(), 2);
}

#[test]
fn a_file_that_became_invalid_meanwhile_is_not_overwritten() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let mut history = History::load(&path).value;
    fs::write(&path, "[{\"wpm\": 1").expect("break the file");

    let error = history
        .add(record("quote", "english", 66.0, 97.0))
        .expect_err("the broken file is kept");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().starts_with("the file is invalid ("),
        "{error}"
    );
    assert_eq!(fs::read_to_string(&path).expect("read"), "[{\"wpm\": 1");
    assert_eq!(history.personal_best("quote", "english"), Some(66.0));
}

#[test]
fn invalid_file_is_moved_aside_and_a_new_history_starts() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    fs::write(&path, r#"[{"date":"yesterday"}]"#).expect("write");

    let loaded = History::load(&path);

    let warning = loaded.warning.expect("warning");
    assert!(
        warning.starts_with("history.json was invalid ("),
        "{warning}"
    );
    assert!(
        warning.contains("starting a new history, backup at"),
        "{warning}"
    );
    assert!(dir.join("history.json.bak").exists());

    let mut history = loaded.value;
    assert!(history.records().is_empty());
    history
        .add(record("quote", "english", 66.0, 97.0))
        .expect("add");
    assert_eq!(History::load(&path).value.records().len(), 1);
}

#[test]
fn invalid_file_that_cannot_be_backed_up_is_never_overwritten() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    fs::write(&path, "[{\"wpm\": 120").expect("write");
    occupy_every_backup(&path);

    let loaded = History::load(&path);
    let mut history = loaded.value;
    history
        .add(record("quote", "english", 66.0, 97.0))
        .expect("add in memory");

    assert!(loaded.warning.is_some());
    assert_eq!(history.records().len(), 1);
    assert_eq!(fs::read_to_string(&path).expect("read"), "[{\"wpm\": 120");
}

#[cfg(unix)]
#[test]
fn file_that_cannot_be_read_is_never_overwritten() {
    let dir = TempDir::new();
    let path = dir.join("history.json");
    let saved = serde_json::to_string(&[record("quote", "english", 90.0, 99.0)]).expect("json");
    fs::write(&path, &saved).expect("write");
    if !crate::persist::scratch::make_unreadable(&path) {
        return;
    }

    let loaded = History::load(&path);
    let mut history = loaded.value;
    history
        .add(record("quote", "english", 66.0, 97.0))
        .expect("add in memory");

    let warning = loaded.warning.expect("warning");
    assert!(
        warning.starts_with("cannot read history.json ("),
        "{warning}"
    );
    assert_eq!(crate::persist::scratch::read_unreadable(&path), saved);
}
