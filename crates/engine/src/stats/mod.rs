use std::time::Duration;

/// Characters in a "standard word" when converting characters to words.
pub const CHARS_PER_WORD: f64 = 5.0;

/**
 * Shortest stretch of time worth its own [`Sample`]: a speed measured over a
 * few milliseconds says nothing.
 */
const MIN_SAMPLE_WINDOW: Duration = Duration::from_millis(500);

/**
 * Counters of a typing session, from which speed and accuracy are derived.
 *
 * Solo sessions and the race server score players with these same rules.
 * Indentation filled in automatically moves the cursor and counts towards
 * completion, but nobody typed it, so it never counts towards speed or
 * accuracy.
 */
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

    pub fn wpm(&self, elapsed: Duration) -> f64 {
        words_per_minute(self.correctly_typed(), elapsed)
    }

    pub fn raw_wpm(&self, elapsed: Duration) -> f64 {
        words_per_minute(self.keystrokes, elapsed)
    }

    pub fn accuracy(&self) -> f64 {
        percentage(self.keystrokes.saturating_sub(self.errors), self.keystrokes)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    /// Keystrokes that did not match the text, including corrected ones.
    pub errors: usize,
    /// Characters of the text currently typed correctly, auto-filled indentation included.
    pub correct_chars: usize,
    pub incorrect_chars: usize,
    /**
     * Keys pressed so far, corrected mistakes included and auto-filled
     * indentation left out.
     */
    pub keystrokes: usize,
    /// Characters of `correct_chars` that auto-indentation filled in.
    pub indentation: usize,
    pub elapsed: Duration,
    /**
     * Completion between 0 and 1: the share of the time limit used, or the
     * [`completion`] of the text. It reaches 1 only once the session is over.
     */
    pub progress: f64,
}

impl Stats {
    pub fn new(tally: Tally, elapsed: Duration, progress: f64) -> Self {
        Self {
            wpm: tally.wpm(elapsed),
            raw_wpm: tally.raw_wpm(elapsed),
            accuracy: tally.accuracy(),
            errors: tally.errors,
            correct_chars: tally.correct,
            incorrect_chars: tally.typed.saturating_sub(tally.correct),
            keystrokes: tally.keystrokes,
            indentation: tally.indentation,
            elapsed,
            progress,
        }
    }
}

/// Speed and errors during about one second of a session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /**
     * Position of the sample, from 1. Every sample covers one second except
     * the last, which covers whatever remains, up to a second and a half.
     */
    pub second: u32,
    /**
     * Speed from the start of the session up to the end of this sample, by
     * the same rule as [`Stats::wpm`], so the last sample matches it.
     */
    pub wpm: f64,
    /// Speed of all keystrokes made during this sample alone.
    pub raw_wpm: f64,
    pub errors: u32,
}

/// Words of [`CHARS_PER_WORD`] characters per minute, 0 when no time has elapsed.
pub fn words_per_minute(chars: usize, elapsed: Duration) -> f64 {
    let minutes = elapsed.as_secs_f64() / 60.0;
    if minutes <= 0.0 {
        0.0
    } else {
        chars as f64 / CHARS_PER_WORD / minutes
    }
}

/**
 * Share of a text of `length` characters currently typed correctly,
 * auto-filled indentation included, between 0 and 1. Solo sessions and race
 * standings both use it, and it reaches 1 only once the whole text is correct.
 */
pub fn completion(correct: usize, length: usize) -> f64 {
    if length == 0 {
        0.0
    } else {
        (correct as f64 / length as f64).clamp(0.0, 1.0)
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

/**
 * How steady the per-second raw speed was, from 0 (erratic) to 100 (constant).
 *
 * Fewer than two samples leave nothing to compare and count as constant.
 */
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

/**
 * Ends of the windows that samples cover over `elapsed`: one per second, the
 * last window taking whatever remains. A remainder shorter than
 * [`MIN_SAMPLE_WINDOW`] stretches the previous window instead of standing alone.
 */
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

#[cfg(test)]
mod tests;
