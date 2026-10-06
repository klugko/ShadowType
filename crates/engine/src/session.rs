//! A typing session: keystrokes compared against a target text, grapheme by grapheme.

use std::time::{Duration, Instant};

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use crate::stats::{Sample, Stats, percentage, words_per_minute};

/// How many characters may be typed past an uncorrected mistake before input
/// is refused. Mistakes have to be fixed for a session to complete, so this
/// keeps a typo from silently ruining the rest of the line.
pub const ERROR_RUN_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionOptions {
    /// Ends the session after this much time, however much has been typed.
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
    /// The time limit was reached.
    TimeUp,
}

/// State of one character of the target text.
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

#[derive(Debug, Clone, Copy)]
struct Keystroke {
    at: Duration,
    correct: bool,
}

/// Tracks what a player types against a target text.
///
/// Texts are compared grapheme by grapheme after Unicode NFC normalisation, so
/// `é` matches whether it was typed precomposed or as `e` followed by a
/// combining accent, and an emoji made of several code points is a single
/// character. All time-dependent methods take the current instant explicitly.
#[derive(Debug, Clone)]
pub struct TypingSession {
    target: Vec<String>,
    entries: Vec<Entry>,
    keystrokes: Vec<Keystroke>,
    auto_filled: usize,
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
            keystrokes: Vec::new(),
            auto_filled: 0,
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

    /// Types one character. Returns whether the input was accepted.
    ///
    /// Combining characters merge into the previous character. Input is refused
    /// once the session is over, past the end of the text, or when the cursor
    /// is [`ERROR_RUN_LIMIT`] characters past an uncorrected mistake.
    pub fn type_char(&mut self, ch: char, now: Instant) -> bool {
        self.update(now);
        if self.is_finished() || (ch.is_control() && ch != '\n') {
            return false;
        }
        if self.merge_into_last(ch, now) {
            return true;
        }
        if self.cursor() >= self.target.len() || self.is_blocked() {
            return false;
        }
        self.start(now);
        let expected = &self.target[self.cursor()];
        let text = ch.to_string();
        let correct = same_text(&text, expected);
        let acceptable = correct || is_partial(&text, expected);
        self.record_keystroke(acceptable, now);
        self.entries.push(Entry {
            text,
            correct,
            auto: false,
        });
        if correct && ch == '\n' && self.options.auto_indent {
            self.fill_indentation();
        }
        self.complete_if_done(now);
        true
    }

    /// Removes the last typed character, or the whole automatic indentation
    /// together with the newline that produced it.
    pub fn backspace(&mut self, now: Instant) -> bool {
        self.update(now);
        !self.is_finished() && self.pop_entry()
    }

    /// Removes the last typed word and the blanks that follow it.
    pub fn delete_word(&mut self, now: Instant) -> bool {
        self.update(now);
        if self.is_finished() {
            return false;
        }
        let mut removed_any = false;
        let mut removed_word = false;
        while let Some(last) = self.entries.last() {
            let blank = last.text.chars().all(char::is_whitespace);
            if blank && removed_word {
                break;
            }
            removed_word |= !blank;
            removed_any |= self.pop_entry();
        }
        removed_any
    }

    /// Ends the session if its time limit has passed.
    pub fn update(&mut self, now: Instant) {
        let (Some(started_at), Some(limit)) = (self.started_at, self.options.time_limit) else {
            return;
        };
        if self.finished_at.is_none() && now.saturating_duration_since(started_at) >= limit {
            self.finished_at = Some(started_at + limit);
            self.timed_out = true;
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

    /// Index of the next character to type.
    pub fn cursor(&self) -> usize {
        self.entries.len()
    }

    /// Characters of the text that have not been typed yet.
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

    /// What was typed at `index`, if anything.
    pub fn typed(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|entry| entry.text.as_str())
    }

    /// Whether input is refused until the first mistake is corrected.
    pub fn is_blocked(&self) -> bool {
        self.entries
            .iter()
            .position(|entry| !entry.correct)
            .is_some_and(|first_error| self.cursor() - first_error >= ERROR_RUN_LIMIT)
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        self.started_at.map_or(Duration::ZERO, |started_at| {
            self.finished_at
                .unwrap_or(now)
                .saturating_duration_since(started_at)
        })
    }

    /// Time left before the limit, for timed sessions.
    pub fn time_left(&self, now: Instant) -> Option<Duration> {
        self.options
            .time_limit
            .map(|limit| limit.saturating_sub(self.elapsed(now)))
    }

    pub fn stats(&self, now: Instant) -> Stats {
        let elapsed = self.elapsed(now);
        let correct_chars = self.entries.iter().filter(|entry| entry.correct).count();
        let keystrokes = self.keystrokes.len();
        let correct_keystrokes = self.keystrokes.iter().filter(|k| k.correct).count();
        let typed_chars = keystrokes + self.auto_filled;
        Stats {
            wpm: words_per_minute(correct_chars, elapsed),
            raw_wpm: words_per_minute(typed_chars, elapsed),
            accuracy: percentage(correct_keystrokes, keystrokes),
            errors: keystrokes - correct_keystrokes,
            correct_chars,
            incorrect_chars: self.entries.len() - correct_chars,
            typed_chars,
            elapsed,
            progress: self.progress(elapsed),
        }
    }

    /// Speed and errors for every whole second elapsed so far.
    pub fn samples(&self, now: Instant) -> Vec<Sample> {
        let seconds = self.elapsed(now).as_secs();
        let mut keystrokes = self.keystrokes.iter().peekable();
        let mut correct_so_far = 0;
        (1..=seconds)
            .map(|second| {
                let end = Duration::from_secs(second);
                let mut in_second = 0;
                let mut errors = 0;
                while let Some(keystroke) = keystrokes.next_if(|k| k.at < end) {
                    in_second += 1;
                    if keystroke.correct {
                        correct_so_far += 1;
                    } else {
                        errors += 1;
                    }
                }
                Sample {
                    second: u32::try_from(second).unwrap_or(u32::MAX),
                    wpm: words_per_minute(correct_so_far, end),
                    raw_wpm: words_per_minute(in_second, Duration::from_secs(1)),
                    errors,
                }
            })
            .collect()
    }

    fn progress(&self, elapsed: Duration) -> f64 {
        let ratio = match self.options.time_limit {
            Some(limit) if !limit.is_zero() => elapsed.as_secs_f64() / limit.as_secs_f64(),
            _ if self.target.is_empty() => 0.0,
            _ => self.cursor() as f64 / self.target.len() as f64,
        };
        ratio.clamp(0.0, 1.0)
    }

    /// Merges a combining character into the last typed grapheme.
    fn merge_into_last(&mut self, ch: char, now: Instant) -> bool {
        if ch.is_ascii() {
            return false;
        }
        let Some(index) = self.cursor().checked_sub(1) else {
            return false;
        };
        let entry = &self.entries[index];
        let mut merged = entry.text.clone();
        merged.push(ch);
        if entry.auto || merged.graphemes(true).count() != 1 {
            return false;
        }
        let expected = &self.target[index];
        let correct = same_text(&merged, expected);
        let acceptable = correct || is_partial(&merged, expected);
        self.record_keystroke(acceptable, now);
        self.entries[index] = Entry {
            text: merged,
            correct,
            auto: false,
        };
        self.complete_if_done(now);
        true
    }

    fn record_keystroke(&mut self, correct: bool, now: Instant) {
        let at = self.elapsed(now);
        self.keystrokes.push(Keystroke { at, correct });
    }

    fn fill_indentation(&mut self) {
        while self.target.get(self.cursor()).is_some_and(|g| g == " ") {
            self.entries.push(Entry {
                text: " ".to_owned(),
                correct: true,
                auto: true,
            });
            self.auto_filled += 1;
        }
    }

    fn pop_entry(&mut self) -> bool {
        let Some(removed) = self.entries.pop() else {
            return false;
        };
        if removed.auto {
            while self.entries.last().is_some_and(|entry| entry.auto) {
                self.entries.pop();
            }
            self.entries.pop();
        }
        true
    }

    fn complete_if_done(&mut self, now: Instant) {
        let done =
            self.cursor() == self.target.len() && self.entries.iter().all(|entry| entry.correct);
        if done && self.finished_at.is_none() {
            self.finished_at = Some(now);
        }
    }
}

/// Splits `text` into NFC-normalised extended grapheme clusters.
pub fn graphemes(text: &str) -> Vec<String> {
    let composed: String = text.nfc().collect();
    composed.graphemes(true).map(str::to_owned).collect()
}

/// Number of characters a player has to type for `text`.
pub fn grapheme_count(text: &str) -> usize {
    let composed: String = text.nfc().collect();
    composed.graphemes(true).count()
}

fn same_text(typed: &str, expected: &str) -> bool {
    typed.nfc().eq(expected.nfc())
}

/// Whether `typed` is the beginning of `expected`, such as `e` for `é`.
fn is_partial(typed: &str, expected: &str) -> bool {
    let typed: String = typed.nfd().collect();
    let expected: String = expected.nfd().collect();
    expected.len() > typed.len() && expected.starts_with(&typed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    fn type_text(session: &mut TypingSession, text: &str, now: Instant) {
        for ch in text.chars() {
            session.type_char(ch, now);
        }
    }

    #[test]
    fn completes_when_everything_is_correct() {
        let start = Instant::now();
        let mut session = TypingSession::new("hello world", SessionOptions::default());
        assert_eq!(session.status(), Status::NotStarted);
        type_text(&mut session, "hello world", at(start, 6_000));
        assert_eq!(session.status(), Status::Completed);
        assert!(!session.type_char('!', at(start, 7_000)));
    }

    #[test]
    fn computes_wpm_raw_and_accuracy() {
        let start = Instant::now();
        let mut session = TypingSession::new("abcde abcde", SessionOptions::default());
        session.start(start);
        type_text(&mut session, "abx", at(start, 1_000));
        session.backspace(at(start, 1_500));
        type_text(&mut session, "cde abcde", at(start, 12_000));
        let stats = session.stats(at(start, 12_000));
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(stats.correct_chars, 11);
        assert_eq!(stats.typed_chars, 12);
        assert_eq!(stats.errors, 1);
        assert!((stats.wpm - 11.0).abs() < 1e-9);
        assert!((stats.raw_wpm - 12.0).abs() < 1e-9);
        assert!((stats.accuracy - 11.0 / 12.0 * 100.0).abs() < 1e-9);
        assert_eq!(stats.progress, 1.0);
    }

    #[test]
    fn empty_session_reports_neutral_stats() {
        let stats = TypingSession::new("abc", SessionOptions::default()).stats(Instant::now());
        assert_eq!(stats.wpm, 0.0);
        assert_eq!(stats.raw_wpm, 0.0);
        assert_eq!(stats.accuracy, 100.0);
        assert_eq!(stats.elapsed, Duration::ZERO);
    }

    #[test]
    fn mistakes_must_be_fixed_to_complete() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab", SessionOptions::default());
        type_text(&mut session, "xb", now);
        assert_eq!(session.status(), Status::Running);
        assert_eq!(session.mark(0), Mark::Incorrect);
        assert_eq!(session.mark(1), Mark::Correct);
        assert!(!session.type_char('c', now), "no input past the end");
        session.backspace(now);
        session.backspace(now);
        type_text(&mut session, "ab", now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(session.stats(now).errors, 1);
    }

    #[test]
    fn input_is_blocked_after_a_long_uncorrected_run() {
        let now = Instant::now();
        let text = "a".repeat(30);
        let mut session = TypingSession::new(&text, SessionOptions::default());
        session.type_char('x', now);
        for _ in 1..ERROR_RUN_LIMIT {
            assert!(session.type_char('a', now));
        }
        assert!(session.is_blocked());
        assert!(!session.type_char('a', now));
        assert_eq!(session.stats(now).typed_chars, ERROR_RUN_LIMIT);
        session.backspace(now);
        assert!(!session.is_blocked());
    }

    #[test]
    fn graphemes_are_single_characters() {
        let now = Instant::now();
        let mut session = TypingSession::new("é👩‍💻x", SessionOptions::default());
        assert_eq!(session.target().len(), 3);
        type_text(&mut session, "é👩‍💻", now);
        assert_eq!(session.cursor(), 2);
        assert_eq!(session.mark(1), Mark::Correct);
        session.backspace(now);
        assert_eq!(session.cursor(), 1);
        assert_eq!(session.typed(0), Some("é"));
    }

    #[test]
    fn decomposed_input_matches_precomposed_text() {
        let now = Instant::now();
        let mut session = TypingSession::new("café", SessionOptions::default());
        type_text(&mut session, "cafe\u{301}", now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(session.stats(now).accuracy, 100.0);
    }

    #[test]
    fn combining_mark_on_a_wrong_base_is_an_error() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab", SessionOptions::default());
        type_text(&mut session, "a\u{301}", now);
        assert_eq!(session.cursor(), 1);
        assert_eq!(session.mark(0), Mark::Incorrect);
        assert_eq!(session.stats(now).errors, 1);
    }

    #[test]
    fn auto_indent_skips_leading_spaces() {
        let now = Instant::now();
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new("{\n    x\n}", options);
        type_text(&mut session, "{\n", now);
        assert_eq!(session.cursor(), 6, "cursor sits on `x`");
        type_text(&mut session, "x\n}", now);
        let stats = session.stats(now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(stats.typed_chars, 9);
        assert_eq!(stats.accuracy, 100.0);
    }

    #[test]
    fn backspace_undoes_auto_indent_with_its_newline() {
        let now = Instant::now();
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new("a\n  b", options);
        type_text(&mut session, "a\n", now);
        assert_eq!(session.cursor(), 4);
        session.backspace(now);
        assert_eq!(session.cursor(), 1);
    }

    #[test]
    fn wrong_newline_does_not_auto_indent() {
        let now = Instant::now();
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new("a b\n  c", options);
        type_text(&mut session, "a\n", now);
        assert_eq!(session.cursor(), 2);
        assert_eq!(session.mark(1), Mark::Incorrect);
    }

    #[test]
    fn delete_word_removes_the_last_word() {
        let now = Instant::now();
        let mut session = TypingSession::new("one two three", SessionOptions::default());
        type_text(&mut session, "one two", now);
        assert!(session.delete_word(now));
        assert_eq!(session.cursor(), 4);
        assert!(session.delete_word(now));
        assert_eq!(session.cursor(), 0);
        assert!(!session.delete_word(now));
    }

    #[test]
    fn time_limit_freezes_the_clock_at_the_limit() {
        let start = Instant::now();
        let options = SessionOptions {
            time_limit: Some(Duration::from_secs(15)),
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new("some words to type", options);
        session.type_char('s', start);
        assert_eq!(
            session.time_left(at(start, 5_000)),
            Some(Duration::from_secs(10))
        );
        assert!(!session.type_char('o', at(start, 15_001)));
        assert_eq!(session.status(), Status::TimeUp);
        assert_eq!(session.elapsed(at(start, 60_000)), Duration::from_secs(15));
        assert_eq!(session.stats(at(start, 60_000)).progress, 1.0);
    }

    #[test]
    fn extend_appends_to_the_target() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab", SessionOptions::default());
        session.extend(" cd");
        type_text(&mut session, "ab c", now);
        assert_eq!(session.remaining(), 1);
        assert_eq!(session.status(), Status::Running);
    }

    #[test]
    fn samples_cover_whole_seconds() {
        let start = Instant::now();
        let mut session = TypingSession::new(&"a".repeat(40), SessionOptions::default());
        session.start(start);
        for i in 0..10 {
            session.type_char('a', at(start, 100 * i));
        }
        session.type_char('x', at(start, 1_500));
        for i in 0..5 {
            session.type_char('a', at(start, 2_000 + 100 * i));
        }
        let samples = session.samples(at(start, 3_200));
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0].raw_wpm, 120.0);
        assert_eq!(samples[1].errors, 1);
        assert_eq!(samples[2].raw_wpm, 60.0);
        assert_eq!(samples[2].wpm, words_per_minute(15, Duration::from_secs(3)));
    }

    #[test]
    fn control_characters_are_ignored() {
        let now = Instant::now();
        let mut session = TypingSession::new("a", SessionOptions::default());
        assert!(!session.type_char('\u{7}', now));
        assert_eq!(session.status(), Status::NotStarted);
    }

    #[test]
    fn grapheme_count_matches_session_length() {
        let text = "naïve 👩‍💻 cafe\u{301}";
        assert_eq!(
            grapheme_count(text),
            TypingSession::new(text, SessionOptions::default())
                .target()
                .len()
        );
    }
}
