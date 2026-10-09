mod grading;
mod history;
mod input;
#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

pub use self::grading::{grapheme_count, graphemes};
use self::history::History;
use crate::stats::{Sample, Stats, Tally, completion};

/**
 * Longest run of characters, counted from the first uncorrected mistake and
 * including it, that can be typed before further input is refused. Mistakes
 * have to be fixed for a session to complete, so this keeps a typo from
 * silently ruining the rest of the line. Indentation filled in
 * automatically is not typed, so it does not count.
 */
pub const ERROR_RUN_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionOptions {
    pub time_limit: Option<Duration>,
    /// Fills the indentation of a line automatically after a newline, like a code editor.
    pub auto_indent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Waiting for the first keystroke, or for [`TypingSession::start`].
    NotStarted,
    Running,
    /// The whole text was typed without uncorrected mistakes.
    Completed,
    TimeUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Pending,
    Correct,
    Incorrect,
}

#[derive(Debug, Clone)]
struct Entry {
    text: String,
    correct: bool,
    auto: bool,
}

impl Entry {
    fn auto_space() -> Self {
        Self {
            text: " ".to_owned(),
            correct: true,
            auto: true,
        }
    }

    fn is_blank(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }

    /**
     * Whether erasing this entry removes a line break: a typed newline, or
     * auto-filled indentation that goes away with the newline before it.
     */
    fn ends_line(&self) -> bool {
        self.auto || self.text == "\n"
    }
}

/**
 * Tracks what a player types against a target text.
 *
 * Texts are compared grapheme by grapheme after Unicode NFC normalisation, so
 * `é` matches whether it was typed precomposed or as `e` followed by a
 * combining accent, and an emoji made of several code points is a single
 * character.
 *
 * A keystroke that only begins the expected character, such as `e` for `é`,
 * counts as correct while the accent may still follow, and becomes an error
 * once the player moves on, erases it or runs out of time. Errors therefore
 * never decrease, which race servers rely on to validate progress.
 */
#[derive(Debug, Clone)]
pub struct TypingSession {
    target: Vec<String>,
    entries: Vec<Entry>,
    history: History,
    options: SessionOptions,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
    timed_out: bool,
}

impl TypingSession {
    pub fn new(text: &str, options: SessionOptions) -> Self {
        Self {
            target: graphemes(text),
            entries: Vec::new(),
            history: History::default(),
            options,
            started_at: None,
            finished_at: None,
            timed_out: false,
        }
    }

    /// Appends more text to type, typically to keep a timed session going.
    pub fn extend(&mut self, text: &str) {
        self.target.extend(graphemes(text));
    }

    /// Starts the clock. Sessions also start on their first keystroke.
    pub fn start(&mut self, now: Instant) {
        self.started_at.get_or_insert(now);
    }

    /// Ends the session if its time limit has passed.
    pub fn update(&mut self, now: Instant) {
        let (Some(started_at), Some(limit)) = (self.started_at, self.options.time_limit) else {
            return;
        };
        if self.finished_at.is_none() && now.saturating_duration_since(started_at) >= limit {
            self.finished_at = Some(started_at + limit);
            self.timed_out = true;
            self.history.abandon_pending();
        }
    }

    pub fn status(&self) -> Status {
        match (self.started_at, self.finished_at) {
            (_, Some(_)) if self.timed_out => Status::TimeUp,
            (_, Some(_)) => Status::Completed,
            (Some(_), None) => Status::Running,
            (None, None) => Status::NotStarted,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished_at.is_some()
    }

    pub fn options(&self) -> SessionOptions {
        self.options
    }

    /// The target text, one grapheme per element.
    pub fn target(&self) -> &[String] {
        &self.target
    }

    pub fn cursor(&self) -> usize {
        self.entries.len()
    }

    pub fn remaining(&self) -> usize {
        self.target.len() - self.cursor()
    }

    pub fn mark(&self, index: usize) -> Mark {
        match self.entries.get(index) {
            Some(entry) if entry.correct => Mark::Correct,
            Some(_) => Mark::Incorrect,
            None => Mark::Pending,
        }
    }

    /**
     * Whether input is refused until the first mistake is corrected: after
     * [`ERROR_RUN_LIMIT`] characters typed from it, or at the end of the
     * text, which is not complete while a mistake is left.
     */
    pub fn is_blocked(&self) -> bool {
        self.first_mistake().is_some_and(|first| {
            self.typed_from(first) >= ERROR_RUN_LIMIT || self.cursor() == self.target.len()
        })
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        self.started_at.map_or(Duration::ZERO, |started_at| {
            self.finished_at
                .unwrap_or(now)
                .saturating_duration_since(started_at)
        })
    }

    pub fn time_left(&self, now: Instant) -> Option<Duration> {
        self.options
            .time_limit
            .map(|limit| limit.saturating_sub(self.elapsed(now)))
    }

    pub fn tally(&self) -> Tally {
        Tally {
            typed: self.cursor(),
            correct: self.entries.iter().filter(|entry| entry.correct).count(),
            indentation: self.entries.iter().filter(|entry| entry.auto).count(),
            keystrokes: self.history.keystrokes(),
            errors: self.history.errors(),
        }
    }

    pub fn stats(&self, now: Instant) -> Stats {
        let elapsed = self.elapsed(now);
        Stats::new(self.tally(), elapsed, self.progress(elapsed))
    }

    pub fn samples(&self, now: Instant) -> Vec<Sample> {
        self.history.samples(self.elapsed(now))
    }

    /**
     * Share of the time limit used, or [`completion`] of the text, so that
     * it reaches 1 only once the session is over.
     */
    fn progress(&self, elapsed: Duration) -> f64 {
        match self.options.time_limit {
            Some(limit) if !limit.is_zero() => {
                (elapsed.as_secs_f64() / limit.as_secs_f64()).clamp(0.0, 1.0)
            }
            _ => completion(self.tally().correct, self.target.len()),
        }
    }

    fn first_mistake(&self) -> Option<usize> {
        self.entries.iter().position(|entry| !entry.correct)
    }

    /**
     * Characters the player typed from `index` on, leaving out the
     * indentation filled in for them.
     */
    fn typed_from(&self, index: usize) -> usize {
        self.entries[index..]
            .iter()
            .filter(|entry| !entry.auto)
            .count()
    }
}
