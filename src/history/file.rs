use std::{io, path::Path};

use chrono::{DateTime, Local};
use serde::Deserialize;

use super::{Record, keep_newest};
use crate::persist;

/**
 * Appends `records` to the file and returns everything it then holds.
 *
 * The file is read again under the lock, rather than rewritten from memory,
 * so that the results another instance saved since start-up are kept.
 */
pub(super) fn append_to_file(path: &Path, records: &[Record]) -> io::Result<Vec<Record>> {
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

pub(super) fn parse_records(contents: &str) -> Result<Vec<Record>, String> {
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

/**
 * Development builds of 0.2 migrated code sessions to `code <language>`,
 * which never matches the `code` key that sessions are recorded under.
 */
fn current_mode(mode: String) -> String {
    if mode.starts_with("code ") {
        Record::CODE_MODE.to_owned()
    } else {
        mode
    }
}

pub(super) fn legacy_mode(mode: &str) -> String {
    match mode.split_once('/') {
        Some((kind @ ("words" | "time"), amount)) => format!("{kind} {amount}"),
        Some(("quote", _)) => "quote".to_owned(),
        Some(("code", _)) => Record::CODE_MODE.to_owned(),
        _ if mode == "multiplayer" => Record::RACE_MODE.to_owned(),
        _ => mode.replace('/', " "),
    }
}
