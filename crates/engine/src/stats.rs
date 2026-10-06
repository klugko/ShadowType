//! Typing speed and accuracy metrics.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Characters in a "standard word" when converting characters to words.
pub const CHARS_PER_WORD: f64 = 5.0;

/// Shortest stretch of time worth its own [`Sample`]: a speed measured over a
/// few milliseconds says nothing.
const MIN_SAMPLE_WINDOW: Duration = Duration::from_millis(500);

/// Counters of a typing session, from which speed and accuracy are derived.
///
/// Solo sessions and the race server score players with these same rules.
/// Indentation filled in automatically moves the cursor and counts towards
/// completion, but nobody typed it, so it never counts towards speed or
/// accuracy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    /// Characters entered, right or wrong: the cursor position.
    pub typed: usize,
    /// Characters currently matching the text, auto-filled indentation included.
    pub correct: usize,
    /// Characters filled in by auto-indentation, all of them correct.
    pub indentation: usize,
    /// Keys pressed, corrected mistakes included.
    pub keystrokes: usize,
    /// Keystrokes that did not match the text.
    pub errors: usize,
}

impl Tally {
    /// Characters the player typed that currently match the text.
    pub fn correctly_typed(&self) -> usize {
        self.correct.saturating_sub(self.indentation)
    }

    /// Correctly typed characters per minute, divided by five.
    pub fn wpm(&self, elapsed: Duration) -> f64 {
        words_per_minute(self.correctly_typed(), elapsed)
    }

    /// Keystrokes per minute, divided by five, mistakes included.
    pub fn raw_wpm(&self, elapsed: Duration) -> f64 {
        words_per_minute(self.keystrokes, elapsed)
    }

    /// Keystrokes that matched the text, as a percentage of all keystrokes.
    pub fn accuracy(&self) -> f64 {
        percentage(self.keystrokes.saturating_sub(self.errors), self.keystrokes)
    }
}

/// A snapshot of how a session is going.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// Correctly typed characters per minute, divided by five; see [`Tally::wpm`].
    pub wpm: f64,
    /// Keystrokes per minute, divided by five, mistakes included.
    pub raw_wpm: f64,
    /// Correct keystrokes over all keystrokes, as a percentage.
    pub accuracy: f64,
    /// Keystrokes that did not match the text, including corrected ones.
    pub errors: usize,
    /// Characters of the text currently typed correctly, auto-filled indentation included.
    pub correct_chars: usize,
    /// Characters of the text currently typed incorrectly.
    pub incorrect_chars: usize,
    /// Keystrokes so far, including corrected mistakes. Auto-filled
    /// indentation is not typed, so it does not count.
    pub typed_chars: usize,
    /// Characters of `correct_chars` that auto-indentation filled in.
    pub indentation: usize,
    #[serde(with = "duration_seconds")]
    pub elapsed: Duration,
    /// Completion between 0 and 1.
    pub progress: f64,
}

impl Stats {
    /// Derives the figures of a session from its counters after `elapsed`.
    pub fn new(tally: Tally, elapsed: Duration, progress: f64) -> Self {
        Self {
            wpm: tally.wpm(elapsed),
            raw_wpm: tally.raw_wpm(elapsed),
            accuracy: tally.accuracy(),
            errors: tally.errors,
            correct_chars: tally.correct,
            incorrect_chars: tally.typed.saturating_sub(tally.correct),
            typed_chars: tally.keystrokes,
            indentation: tally.indentation,
            elapsed,
            progress,
        }
    }
}

/// Speed and errors during about one second of a session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Position of the sample, from 1. Every sample covers one second except
    /// the last, which covers whatever remains, up to a second and a half.
    pub second: u32,
    /// Speed from the start of the session up to the end of this sample, by
    /// the same rule as [`Stats::wpm`], so the last sample matches it.
    pub wpm: f64,
    /// Speed of all keystrokes made during this sample alone.
    pub raw_wpm: f64,
    pub errors: u32,
}

/// Converts a number of characters typed in `elapsed` into words per minute.
pub fn words_per_minute(chars: usize, elapsed: Duration) -> f64 {
    let minutes = elapsed.as_secs_f64() / 60.0;
    if minutes <= 0.0 {
        0.0
    } else {
        chars as f64 / CHARS_PER_WORD / minutes
    }
}

/// Percentage of `correct` among `total`, 100 when nothing was attempted.
pub fn percentage(correct: usize, total: usize) -> f64 {
    if total == 0 {
        100.0
    } else {
        correct as f64 / total as f64 * 100.0
    }
}

/// How steady the per-second raw speed was, from 0 (erratic) to 100 (constant).
///
/// Fewer than two samples leave nothing to compare and count as constant.
pub fn consistency(samples: &[Sample]) -> f64 {
    if samples.len() < 2 {
        return 100.0;
    }
    let count = samples.len() as f64;
    let speeds = samples.iter().map(|sample| sample.raw_wpm);
    let mean = speeds.clone().sum::<f64>() / count;
    if mean <= 0.0 {
        return 0.0;
    }
    let variance = speeds.map(|speed| (speed - mean).powi(2)).sum::<f64>() / count;
    let variation = variance.sqrt() / mean;
    (100.0 * (1.0 - variation)).clamp(0.0, 100.0)
}

/// Ends of the windows that samples cover over `elapsed`: one per second, the
/// last window taking whatever remains. A remainder shorter than
/// [`MIN_SAMPLE_WINDOW`] stretches the previous window instead of standing alone.
pub(crate) fn sample_ends(elapsed: Duration) -> Vec<Duration> {
    let whole_seconds = elapsed.as_secs();
    let remainder = elapsed - Duration::from_secs(whole_seconds);
    let mut ends: Vec<Duration> = (1..=whole_seconds).map(Duration::from_secs).collect();
    match ends.last_mut() {
        Some(last) if remainder < MIN_SAMPLE_WINDOW => *last = elapsed,
        _ if !remainder.is_zero() => ends.push(elapsed),
        _ => {}
    }
    ends
}

mod duration_seconds {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(duration.as_secs_f64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let seconds = f64::deserialize(deserializer)?;
        Ok(Duration::try_from_secs_f64(seconds).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpm_uses_five_characters_per_word() {
        assert_eq!(words_per_minute(300, Duration::from_secs(60)), 60.0);
        assert_eq!(words_per_minute(50, Duration::from_secs(10)), 60.0);
    }

    #[test]
    fn wpm_is_zero_without_elapsed_time() {
        assert_eq!(words_per_minute(42, Duration::ZERO), 0.0);
    }

    #[test]
    fn percentage_defaults_to_perfect() {
        assert_eq!(percentage(0, 0), 100.0);
        assert_eq!(percentage(3, 4), 75.0);
    }

    #[test]
    fn indentation_counts_for_neither_speed_nor_accuracy() {
        let tally = Tally {
            typed: 30,
            correct: 28,
            indentation: 8,
            keystrokes: 25,
            errors: 5,
        };
        let minute = Duration::from_secs(60);
        assert_eq!(tally.correctly_typed(), 20);
        assert_eq!(tally.wpm(minute), 4.0);
        assert_eq!(tally.raw_wpm(minute), 5.0);
        assert_eq!(tally.accuracy(), 80.0);
    }

    #[test]
    fn stats_are_derived_from_the_tally() {
        let tally = Tally {
            typed: 12,
            correct: 10,
            indentation: 4,
            keystrokes: 9,
            errors: 1,
        };
        let elapsed = Duration::from_secs(6);
        let stats = Stats::new(tally, elapsed, 0.5);
        assert_eq!(stats.wpm, tally.wpm(elapsed));
        assert_eq!(stats.raw_wpm, tally.raw_wpm(elapsed));
        assert_eq!(stats.accuracy, tally.accuracy());
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.correct_chars, 10);
        assert_eq!(stats.incorrect_chars, 2);
        assert_eq!(stats.typed_chars, 9);
        assert_eq!(stats.indentation, 4);
        assert_eq!(stats.progress, 0.5);
    }

    #[test]
    fn an_empty_tally_is_neutral() {
        let tally = Tally::default();
        assert_eq!(tally.wpm(Duration::from_secs(10)), 0.0);
        assert_eq!(tally.accuracy(), 100.0);
    }

    #[test]
    fn samples_cover_seconds_and_fold_short_remainders() {
        let millis = Duration::from_millis;
        assert!(sample_ends(Duration::ZERO).is_empty());
        assert_eq!(sample_ends(millis(300)), [millis(300)]);
        assert_eq!(sample_ends(millis(2_000)), [millis(1_000), millis(2_000)]);
        assert_eq!(sample_ends(millis(2_499)), [millis(1_000), millis(2_499)]);
        assert_eq!(
            sample_ends(millis(2_500)),
            [millis(1_000), millis(2_000), millis(2_500)]
        );
    }

    #[test]
    fn consistency_rewards_steady_speed() {
        let steady: Vec<Sample> = (1..=5).map(|second| sample(second, 80.0)).collect();
        let erratic: Vec<Sample> = [20.0, 140.0, 30.0, 150.0, 10.0]
            .into_iter()
            .zip(1..)
            .map(|(speed, second)| sample(second, speed))
            .collect();
        assert_eq!(consistency(&steady), 100.0);
        assert!(consistency(&erratic) < 40.0);
    }

    #[test]
    fn fewer_than_two_samples_are_consistent() {
        assert_eq!(consistency(&[]), 100.0);
        assert_eq!(consistency(&[sample(1, 70.0)]), 100.0);
    }

    #[test]
    fn stats_round_trip_with_fractional_seconds() {
        let stats = Stats {
            wpm: 71.5,
            elapsed: Duration::from_millis(12_340),
            ..Stats::default()
        };
        let json = serde_json::to_string(&stats).expect("serialize");
        let back: Stats = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, stats);
    }

    fn sample(second: u32, raw_wpm: f64) -> Sample {
        Sample {
            second,
            wpm: raw_wpm,
            raw_wpm,
            errors: 0,
        }
    }
}
