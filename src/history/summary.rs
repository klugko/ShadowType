use std::{collections::BTreeSet, time::Duration};

use chrono::NaiveDate;

use super::{History, RECENT_SESSIONS, Record};

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

impl History {
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

    /// What was practised on `day`: how many sessions, for how long.
    pub fn day(&self, day: NaiveDate) -> (usize, Duration) {
        let records: Vec<&Record> = self
            .records
            .iter()
            .filter(|record| record.date.date_naive() == day)
            .collect();
        let seconds: f64 = records.iter().map(|record| record.duration).sum();
        (records.len(), Duration::from_secs_f64(seconds.max(0.0)))
    }

    /**
     * How many days in a row, up to `today`, have at least one session.
     * A streak still counts on a day not practised yet when the day
     * before was.
     */
    pub fn streak(&self, today: NaiveDate) -> usize {
        let days: BTreeSet<NaiveDate> = self
            .records
            .iter()
            .map(|record| record.date.date_naive())
            .collect();
        let start = if days.contains(&today) {
            Some(today)
        } else {
            today
                .pred_opt()
                .filter(|yesterday| days.contains(yesterday))
        };
        std::iter::successors(start, |day| day.pred_opt().filter(|day| days.contains(day))).count()
    }

    /// Best speed ever reached in this mode and language.
    pub fn personal_best(&self, mode: &str, language: &str) -> Option<f64> {
        self.records
            .iter()
            .filter(|record| record.mode == mode && record.language == language)
            .map(|record| record.wpm)
            .reduce(f64::max)
    }
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
