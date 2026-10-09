/*!
 * Results of finished sessions, stored in `history.json` as a JSON array of
 * records, oldest first. Files written by code-racer 0.1 are understood and
 * migrated on the next save.
 */

mod file;
mod summary;
#[cfg(test)]
mod tests;

use std::{
    io,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local};
use code_racer_engine::Stats;
use serde::{Deserialize, Serialize};

use crate::persist::{self, Loaded, Recovered};
use file::{append_to_file, parse_records};
pub use summary::Summary;

/// Records kept on disk; the oldest ones are dropped first.
pub const MAX_RECORDS: usize = 2000;

/// Sessions averaged into [`Summary::recent_wpm`].
pub const RECENT_SESSIONS: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub date: DateTime<Local>,
    /**
     * What was practised: `words 50`, `time 30`, `quote`, `code`, `file` or
     * `race`.
     */
    pub mode: String,
    /**
     * The natural language of the text, or the programming language of a
     * `code` session (`rust`), a `file` (`text` when unknown) or a race on code.
     */
    pub language: String,
    /// Length of the session, in seconds.
    pub duration: f64,
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub errors: usize,
    /// Characters of the text that were typed, right or wrong.
    pub text_length: usize,
}

impl Record {
    /// [`Record::mode`] of code sessions, whatever their programming language.
    pub const CODE_MODE: &'static str = "code";
    /// [`Record::mode`] of races, whatever their text.
    pub const RACE_MODE: &'static str = "race";

    /// The record of a session that just ended, dated now.
    pub fn from_stats(mode: String, language: String, stats: &Stats) -> Self {
        Self {
            date: Local::now(),
            mode,
            language,
            duration: stats.elapsed.as_secs_f64(),
            wpm: stats.wpm,
            raw_wpm: stats.raw_wpm,
            accuracy: stats.accuracy,
            errors: stats.errors,
            text_length: stats.correct_chars + stats.incorrect_chars,
        }
    }

    /**
     * JSON cannot store NaN or infinities, which would make the whole file
     * unreadable, and negative amounts are meaningless: both become zero, so
     * that no summary can come out as NaN.
     */
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

#[derive(Debug, Clone, PartialEq)]
pub struct History {
    records: Vec<Record>,
    /// How many of the newest records are not in the file yet.
    unsaved: usize,
    path: Option<PathBuf>,
}

impl History {
    /**
     * Reads the history saved at `path`, which later additions are written to.
     *
     * A missing file gives an empty history. An invalid one is moved aside,
     * explained in the warning, and replaced by an empty history. A file
     * that can be neither used nor moved aside gives an empty history that
     * stays in memory, so that the file is never overwritten.
     */
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

    /**
     * Appends a record and saves it, along with the records saved meanwhile
     * by other running instances, which then show up here too.
     *
     * On a write error the record stays in memory, is saved with the next
     * one, and the error is returned.
     */
    pub fn add(&mut self, record: Record) -> io::Result<()> {
        self.records.push(record.sanitized());
        self.unsaved = (self.unsaved + 1).min(MAX_RECORDS);
        keep_newest(&mut self.records);
        self.save()
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
