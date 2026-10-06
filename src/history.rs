//! Results of finished sessions, stored in `history.json`.
//!
//! The file is a JSON array of records, oldest first. Files written by
//! code-racer 0.1 are understood and migrated on the next save. Several
//! running instances can share the file: each result is appended to what the
//! file holds at that moment, not to what it held at start-up.

use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::persist::{self, Loaded, Recovered};

/// Records kept on disk; the oldest ones are dropped first.
pub const MAX_RECORDS: usize = 2000;

/// Sessions averaged into [`Summary::recent_wpm`].
pub const RECENT_SESSIONS: usize = 10;

/// [`Record::mode`] of code sessions, whatever their programming language.
const CODE_MODE: &str = "code";

/// One finished solo session or race.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub date: DateTime<Local>,
    /// What was practised: `words 50`, `time 30`, `quote`, `code`, `file` or
    /// `race`.
    pub mode: String,
    /// The natural language of the text, or the programming language of a
    /// `code` session (`rust`), a `file` (`text` when unknown) or a race on code.
    pub language: String,
    /// Length of the session, in seconds.
    pub duration: f64,
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub errors: usize,
    /// Characters in the text that was typed.
    pub text_length: usize,
}

/// Totals over the whole history.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    pub sessions: usize,
    pub best_wpm: f64,
    pub average_wpm: f64,
    /// Average speed of the last [`RECENT_SESSIONS`] sessions.
    pub recent_wpm: f64,
    pub best_accuracy: f64,
    pub total_time: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct History {
    records: Vec<Record>,
    /// How many of the newest records are not in the file yet.
    unsaved: usize,
    path: Option<PathBuf>,
}

impl History {
    /// Reads the history saved at `path`, which later additions are written to.
    ///
    /// A missing file gives an empty history. An invalid one is moved aside,
    /// explained in the warning, and replaced by an empty history. A file
    /// that can be neither used nor moved aside gives an empty history that
    /// stays in memory, so that the file is never overwritten.
    pub fn load(path: &Path) -> Loaded<Self> {
        persist::read_or_recover(path, "starting a new history", parse_records).map(|found| {
            match found {
                Recovered::Parsed(records) => Self::saved_at(path, records),
                Recovered::Absent => Self::saved_at(path, Vec::new()),
                Recovered::LeftInPlace => Self::in_memory(),
            }
        })
    }

    fn saved_at(path: &Path, records: Vec<Record>) -> Self {
        Self {
            records,
            unsaved: 0,
            path: Some(path.to_owned()),
        }
    }

    /// A history that lives only as long as the program and never touches the disk.
    pub fn in_memory() -> Self {
        Self {
            records: Vec::new(),
            unsaved: 0,
            path: None,
        }
    }

    /// Every record, oldest first.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Appends a record and saves it, along with the records saved meanwhile
    /// by other running instances, which then show up here too.
    ///
    /// On a write error the record stays in memory, is saved with the next
    /// one, and the error is returned.
    pub fn add(&mut self, record: Record) -> io::Result<()> {
        self.records.push(record.sanitized());
        self.unsaved = (self.unsaved + 1).min(MAX_RECORDS);
        keep_newest(&mut self.records);
        self.save()
    }

    pub fn summary(&self) -> Summary {
        let speeds = self.records.iter().map(|record| record.wpm);
        Summary {
            sessions: self.records.len(),
            best_wpm: maximum(speeds.clone()),
            average_wpm: mean(speeds),
            recent_wpm: mean(
                self.records
                    .iter()
                    .rev()
                    .take(RECENT_SESSIONS)
                    .map(|record| record.wpm),
            ),
            best_accuracy: maximum(self.records.iter().map(|record| record.accuracy)),
            total_time: total_time(&self.records),
        }
    }

    /// Best speed ever reached in this mode and language.
    pub fn personal_best(&self, mode: &str, language: &str) -> Option<f64> {
        self.records
            .iter()
            .filter(|record| record.mode == mode && record.language == language)
            .map(|record| record.wpm)
            .reduce(f64::max)
    }

    fn save(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let unsaved = &self.records[self.records.len().saturating_sub(self.unsaved)..];
        self.records = append_to_file(path, unsaved)?;
        self.unsaved = 0;
        Ok(())
    }
}

/// Appends `records` to the file and returns everything it then holds.
///
/// The file is read again under the lock, rather than rewritten from memory,
/// so that the results another instance saved since start-up are kept.
fn append_to_file(path: &Path, records: &[Record]) -> io::Result<Vec<Record>> {
    let _lock = persist::lock(path)?;
    let mut saved = match persist::read_existing(path)? {
        Some(contents) => {
            parse_records(&contents).map_err(|problem| persist::invalid_contents(&problem))?
        }
        None => Vec::new(),
    };
    saved.extend_from_slice(records);
    keep_newest(&mut saved);
    let json = serde_json::to_vec_pretty(&saved).map_err(io::Error::other)?;
    persist::write_atomically(path, &json)?;
    Ok(saved)
}

impl Record {
    /// JSON cannot store NaN or infinities, which would make the whole file
    /// unreadable, and negative amounts are meaningless: both become zero, so
    /// that no summary can come out as NaN.
    fn sanitized(self) -> Self {
        Self {
            duration: non_negative(self.duration),
            wpm: non_negative(self.wpm),
            raw_wpm: non_negative(self.raw_wpm),
            accuracy: non_negative(self.accuracy),
            ..self
        }
    }
}

fn non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn keep_newest(records: &mut Vec<Record>) {
    let excess = records.len().saturating_sub(MAX_RECORDS);
    records.drain(..excess);
}

fn maximum(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, f64::max)
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0_u32), |(sum, count), value| (sum + value, count + 1));
    if count == 0 {
        0.0
    } else {
        sum / f64::from(count)
    }
}

fn total_time(records: &[Record]) -> Duration {
    let seconds: f64 = records.iter().map(|record| record.duration).sum();
    Duration::try_from_secs_f64(seconds).unwrap_or_default()
}

fn parse_records(contents: &str) -> Result<Vec<Record>, String> {
    let stored: Vec<StoredRecord> =
        serde_json::from_str(contents).map_err(|error| error.to_string())?;
    let mut records: Vec<Record> = stored
        .into_iter()
        .map(|stored| Record::from(stored).sanitized())
        .collect();
    keep_newest(&mut records);
    Ok(records)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredRecord {
    Current(Record),
    Legacy(LegacyRecord),
}

/// A record as written by code-racer 0.1.
#[derive(Deserialize)]
struct LegacyRecord {
    date: DateTime<Local>,
    /// `words/50`, `time/30`, `quote/50`, `code/50` or `multiplayer`.
    mode: String,
    /// A natural language, or the programming language in code mode.
    language: String,
    stats: LegacyStats,
}

#[derive(Deserialize)]
struct LegacyStats {
    wpm: f64,
    raw_wpm: f64,
    accuracy: f64,
    errors: usize,
    length: usize,
    /// Seconds.
    elapsed: f64,
}

impl From<StoredRecord> for Record {
    fn from(stored: StoredRecord) -> Self {
        match stored {
            StoredRecord::Current(record) => Self {
                mode: current_mode(record.mode),
                ..record
            },
            StoredRecord::Legacy(legacy) => Self {
                mode: legacy_mode(&legacy.mode),
                date: legacy.date,
                language: legacy.language,
                duration: legacy.stats.elapsed,
                wpm: legacy.stats.wpm,
                raw_wpm: legacy.stats.raw_wpm,
                accuracy: legacy.stats.accuracy,
                errors: legacy.stats.errors,
                text_length: legacy.stats.length,
            },
        }
    }
}

/// Development builds of 0.2 migrated code sessions to `code <language>`,
/// which never matches the `code` key that sessions are recorded under.
fn current_mode(mode: String) -> String {
    if mode.starts_with("code ") {
        CODE_MODE.to_owned()
    } else {
        mode
    }
}

fn legacy_mode(mode: &str) -> String {
    match mode.split_once('/') {
        Some((kind @ ("words" | "time"), amount)) => format!("{kind} {amount}"),
        Some(("quote", _)) => "quote".to_owned(),
        Some(("code", _)) => CODE_MODE.to_owned(),
        _ if mode == "multiplayer" => "race".to_owned(),
        _ => mode.replace('/', " "),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::persist::scratch::{TempDir, occupy_every_backup};

    fn record(mode: &str, language: &str, wpm: f64, accuracy: f64) -> Record {
        Record {
            date: DateTime::parse_from_rfc3339("2026-03-14T15:09:26+01:00")
                .expect("valid date")
                .with_timezone(&Local),
            mode: mode.to_owned(),
            language: language.to_owned(),
            duration: 30.0,
            wpm,
            raw_wpm: wpm + 5.0,
            accuracy,
            errors: 2,
            text_length: 250,
        }
    }

    fn with_speeds(speeds: impl IntoIterator<Item = f64>) -> History {
        let mut history = History::in_memory();
        for wpm in speeds {
            history
                .add(record("words 50", "english", wpm, 95.0))
                .expect("in-memory add");
        }
        history
    }

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

        let saved: Vec<Record> = serde_json::from_str(&fs::read_to_string(&path).expect("read"))
            .expect("current format");
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].mode, "words 25");
        assert_eq!(saved[0].duration, 30.5);
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
}
