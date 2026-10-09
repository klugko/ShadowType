mod migration;
mod storage;
mod summary;

use chrono::{DateTime, Local, NaiveDate};

use super::*;

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

/// A history with a 30-second session on each of `days`.
fn practised_on(days: &[&str]) -> History {
    let mut history = History::in_memory();
    for day in days {
        let date = DateTime::parse_from_rfc3339(&format!("{day}T12:00:00+00:00"))
            .expect("valid date")
            .with_timezone(&Local);
        history
            .add(Record {
                date,
                ..record("words 50", "english", 60.0, 95.0)
            })
            .expect("in-memory add");
    }
    history
}

fn date(day: &str) -> NaiveDate {
    let noon = DateTime::parse_from_rfc3339(&format!("{day}T12:00:00+00:00"))
        .expect("valid date")
        .with_timezone(&Local);
    noon.date_naive()
}
