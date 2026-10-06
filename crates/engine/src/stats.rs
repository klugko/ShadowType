//! Typing speed and accuracy metrics.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Characters in a "standard word" when converting characters to words.
pub const CHARS_PER_WORD: f64 = 5.0;

/// A snapshot of how a session is going.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// Correctly typed characters per minute, divided by five.
    pub wpm: f64,
    /// Every typed character per minute, divided by five, mistakes included.
    pub raw_wpm: f64,
    /// Correct keystrokes over all keystrokes, as a percentage.
    pub accuracy: f64,
    /// Keystrokes that did not match the text, including corrected ones.
    pub errors: usize,
    /// Characters of the text currently typed correctly.
    pub correct_chars: usize,
    /// Characters of the text currently typed incorrectly.
    pub incorrect_chars: usize,
    /// Characters entered so far, including corrected mistakes.
    pub typed_chars: usize,
    #[serde(with = "duration_seconds")]
    pub elapsed: Duration,
    /// Completion between 0 and 1.
    pub progress: f64,
}

/// Speed and errors during one second of a session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub second: u32,
    /// Speed from the start of the session up to the end of this second.
    pub wpm: f64,
    /// Speed of all keystrokes made during this second alone.
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
pub fn consistency(samples: &[Sample]) -> f64 {
    if samples.is_empty() {
        return 0.0;
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
    fn consistency_rewards_steady_speed() {
        let steady: Vec<Sample> = (1..=5).map(|second| sample(second, 80.0)).collect();
        let erratic: Vec<Sample> = [20.0, 140.0, 30.0, 150.0, 10.0]
            .into_iter()
            .zip(1..)
            .map(|(speed, second)| sample(second, speed))
            .collect();
        assert_eq!(consistency(&steady), 100.0);
        assert!(consistency(&erratic) < 40.0);
        assert_eq!(consistency(&[]), 0.0);
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
