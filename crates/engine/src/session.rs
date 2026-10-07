//! A typing session: keystrokes compared against a target text, grapheme by grapheme.

use std::{
    iter::Peekable,
    slice,
    time::{Duration, Instant},
};

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    indentation::indentation_run,
    normalize::keyboard_form,
    stats::{Sample, Stats, Tally, sample_ends, words_per_minute},
};

/// Longest run of characters, counted from the first uncorrected mistake and
/// including it, that can be typed before further input is refused. Mistakes
/// have to be fixed for a session to complete, so this keeps a typo from
/// silently ruining the rest of the line.
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

    /// Whether erasing this entry removes a line break: a typed newline, or
    /// auto-filled indentation that goes away with the newline before it.
    fn ends_line(&self) -> bool {
        self.auto || self.text == "\n"
    }
}

#[derive(Debug, Clone, Copy)]
struct Keystroke {
    at: Duration,
    correct: bool,
}

/// How many characters were correctly typed right after an input, on the
/// basis of [`Tally::correctly_typed`].
#[derive(Debug, Clone, Copy)]
struct Checkpoint {
    at: Duration,
    correctly_typed: usize,
}

/// How some input compares with the character expected at its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judgement {
    Correct,
    /// The beginning of the expected character, such as `e` on the way to `é`.
    Partial,
    Wrong,
}

/// Tracks what a player types against a target text.
///
/// Texts are compared grapheme by grapheme after Unicode NFC normalisation, so
/// `é` matches whether it was typed precomposed or as `e` followed by a
/// combining accent, and an emoji made of several code points is a single
/// character. All time-dependent methods take the current instant explicitly.
///
/// A keystroke that only begins the expected character, such as `e` for `é`,
/// counts as correct while the accent may still follow, and becomes an error
/// once the player moves on, erases it or runs out of time. Errors therefore
/// never decrease, which race servers rely on to validate progress.
#[derive(Debug, Clone)]
pub struct TypingSession {
    target: Vec<String>,
    entries: Vec<Entry>,
    keystrokes: Vec<Keystroke>,
    /// First keystroke of the last character while it is only the beginning
    /// of the expected one. Those keystrokes count as correct until the
    /// character is completed, and become errors if it is left unfinished.
    pending_since: Option<usize>,
    checkpoints: Vec<Checkpoint>,
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
            pending_since: None,
            checkpoints: Vec::new(),
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
    /// The character is first folded like [`normalize`](crate::normalize())
    /// folds texts, so a typed no-break space, `’` or `…` stands for the
    /// space, `'` or `...` of the text, and characters that display as
    /// nothing are refused. Combining characters merge into the previous
    /// character. Input is refused once the session is over, past the end of
    /// the text, or once [`ERROR_RUN_LIMIT`] characters have been typed from
    /// the first uncorrected mistake onward.
    pub fn type_char(&mut self, ch: char, now: Instant) -> bool {
        self.update(now);
        if ch.is_control() && ch != '\n' {
            return false;
        }
        let mut accepted = false;
        for key in keyboard_form(ch).chars() {
            accepted |= self.enter(key, now);
        }
        accepted
    }

    /// Removes the last typed character, or the whole automatic indentation
    /// together with the newline that produced it.
    pub fn backspace(&mut self, now: Instant) -> bool {
        self.erase(now, Self::pop_entry)
    }

    /// Removes the last typed word and the blanks that follow it, like
    /// Ctrl+Backspace in an editor: a line break is removed on its own, never
    /// together with the line before it.
    pub fn delete_word(&mut self, now: Instant) -> bool {
        self.erase(now, Self::pop_word)
    }

    /// Ends the session if its time limit has passed.
    pub fn update(&mut self, now: Instant) {
        let (Some(started_at), Some(limit)) = (self.started_at, self.options.time_limit) else {
            return;
        };
        if self.finished_at.is_none() && now.saturating_duration_since(started_at) >= limit {
            self.finished_at = Some(started_at + limit);
            self.timed_out = true;
            self.abandon_pending();
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

    /// Whether input is refused until the first mistake is corrected.
    pub fn is_blocked(&self) -> bool {
        self.first_mistake()
            .is_some_and(|first| self.cursor() - first >= ERROR_RUN_LIMIT)
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

    /// The counters every speed and accuracy figure is derived from.
    pub fn tally(&self) -> Tally {
        Tally {
            typed: self.cursor(),
            correct: self.entries.iter().filter(|entry| entry.correct).count(),
            indentation: self.entries.iter().filter(|entry| entry.auto).count(),
            keystrokes: self.keystrokes.len(),
            errors: self.keystrokes.iter().filter(|k| !k.correct).count(),
        }
    }

    pub fn stats(&self, now: Instant) -> Stats {
        let elapsed = self.elapsed(now);
        Stats::new(self.tally(), elapsed, self.progress(elapsed))
    }

    /// Speed and errors second by second, from the start of the session to `now`.
    pub fn samples(&self, now: Instant) -> Vec<Sample> {
        let mut replay = Replay::new(&self.keystrokes, &self.checkpoints);
        sample_ends(self.elapsed(now))
            .into_iter()
            .zip(1..)
            .map(|(end, second)| replay.sample(second, end))
            .collect()
    }

    /// Share of the time limit used, or of the text typed correctly up to the
    /// first mistake, so that it reaches 1 only once the session is over.
    fn progress(&self, elapsed: Duration) -> f64 {
        let ratio = match self.options.time_limit {
            Some(limit) if !limit.is_zero() => elapsed.as_secs_f64() / limit.as_secs_f64(),
            _ if self.target.is_empty() => 0.0,
            _ => self.correct_prefix() as f64 / self.target.len() as f64,
        };
        ratio.clamp(0.0, 1.0)
    }

    /// Characters typed correctly before the first uncorrected mistake.
    fn correct_prefix(&self) -> usize {
        self.first_mistake().unwrap_or(self.cursor())
    }

    fn first_mistake(&self) -> Option<usize> {
        self.entries.iter().position(|entry| !entry.correct)
    }

    /// Enters one character already in the form of the text.
    fn enter(&mut self, ch: char, now: Instant) -> bool {
        if self.is_finished() {
            return false;
        }
        let accepted = self.merge_into_last(ch, now) || self.push_char(ch, now);
        if accepted {
            self.record_checkpoint(now);
            self.complete_if_done(now);
        }
        accepted
    }

    fn push_char(&mut self, ch: char, now: Instant) -> bool {
        if self.cursor() >= self.target.len() || self.is_blocked() {
            return false;
        }
        self.start(now);
        self.abandon_pending();
        let text = ch.to_string();
        let judgement = judge(&text, &self.target[self.cursor()]);
        let correct = judgement == Judgement::Correct;
        self.record_keystroke(judgement, now);
        self.entries.push(Entry {
            text,
            correct,
            auto: false,
        });
        if correct && ch == '\n' && self.options.auto_indent {
            self.fill_indentation();
        }
        true
    }

    /// Applies a deletion, which leaves any character in progress unfinished.
    fn erase(&mut self, now: Instant, remove: fn(&mut Self)) -> bool {
        self.update(now);
        if self.is_finished() || self.entries.is_empty() {
            return false;
        }
        self.abandon_pending();
        remove(self);
        self.record_checkpoint(now);
        true
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
        let judgement = judge(&merged, &self.target[index]);
        self.record_keystroke(judgement, now);
        self.entries[index] = Entry {
            text: merged,
            correct: judgement == Judgement::Correct,
            auto: false,
        };
        true
    }

    fn record_keystroke(&mut self, judgement: Judgement, now: Instant) {
        let at = self.elapsed(now);
        let index = self.keystrokes.len();
        self.keystrokes.push(Keystroke {
            at,
            correct: judgement != Judgement::Wrong,
        });
        if judgement == Judgement::Partial {
            self.pending_since.get_or_insert(index);
        } else {
            self.pending_since = None;
        }
    }

    /// Turns the keystrokes of a character left unfinished into errors.
    fn abandon_pending(&mut self) {
        if let Some(first) = self.pending_since.take() {
            for keystroke in &mut self.keystrokes[first..] {
                keystroke.correct = false;
            }
        }
    }

    fn record_checkpoint(&mut self, now: Instant) {
        let checkpoint = Checkpoint {
            at: self.elapsed(now),
            correctly_typed: self.tally().correctly_typed(),
        };
        self.checkpoints.push(checkpoint);
    }

    fn fill_indentation(&mut self) {
        let width = indentation_run(&self.target, self.cursor());
        self.entries
            .extend(std::iter::repeat_n(Entry::auto_space(), width));
    }

    fn pop_entry(&mut self) {
        let removed = self.entries.pop();
        if removed.is_some_and(|entry| entry.auto) {
            while self.entries.last().is_some_and(|entry| entry.auto) {
                self.entries.pop();
            }
            self.entries.pop();
        }
    }

    fn pop_word(&mut self) {
        let mut removed_word = false;
        while let Some(last) = self.entries.last() {
            let blank = last.is_blank();
            if blank && removed_word {
                break;
            }
            let ends_line = last.ends_line();
            removed_word |= !blank;
            self.pop_entry();
            if ends_line {
                break;
            }
        }
    }

    fn complete_if_done(&mut self, now: Instant) {
        let done =
            self.cursor() == self.target.len() && self.entries.iter().all(|entry| entry.correct);
        if done && self.finished_at.is_none() {
            self.finished_at = Some(now);
        }
    }
}

/// Walks through the history of a session one sample window after the other.
struct Replay<'a> {
    keystrokes: Peekable<slice::Iter<'a, Keystroke>>,
    checkpoints: Peekable<slice::Iter<'a, Checkpoint>>,
    correctly_typed: usize,
    window_start: Duration,
}

impl<'a> Replay<'a> {
    fn new(keystrokes: &'a [Keystroke], checkpoints: &'a [Checkpoint]) -> Self {
        Self {
            keystrokes: keystrokes.iter().peekable(),
            checkpoints: checkpoints.iter().peekable(),
            correctly_typed: 0,
            window_start: Duration::ZERO,
        }
    }

    /// Summarises what happened since the previous window, up to `end` included.
    fn sample(&mut self, second: u32, end: Duration) -> Sample {
        let mut keystrokes = 0;
        let mut errors = 0;
        while let Some(keystroke) = self.keystrokes.next_if(|k| k.at <= end) {
            keystrokes += 1;
            errors += u32::from(!keystroke.correct);
        }
        while let Some(checkpoint) = self.checkpoints.next_if(|c| c.at <= end) {
            self.correctly_typed = checkpoint.correctly_typed;
        }
        let window = end - self.window_start;
        self.window_start = end;
        Sample {
            second,
            wpm: words_per_minute(self.correctly_typed, end),
            raw_wpm: words_per_minute(keystrokes, window),
            errors,
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

fn judge(typed: &str, expected: &str) -> Judgement {
    if same_text(typed, expected) {
        Judgement::Correct
    } else if is_partial(typed, expected) {
        Judgement::Partial
    } else {
        Judgement::Wrong
    }
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
    use crate::stats::consistency;

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
    fn progress_stays_below_one_until_the_text_is_correct() {
        let now = Instant::now();
        let mut session = TypingSession::new("abcd", SessionOptions::default());
        type_text(&mut session, "abxd", now);
        assert_eq!(session.status(), Status::Running);
        assert_eq!(session.stats(now).progress, 0.5, "up to the first mistake");
        session.backspace(now);
        session.backspace(now);
        type_text(&mut session, "cd", now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(session.stats(now).progress, 1.0);
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
        assert_eq!(session.mark(0), Mark::Correct);
    }

    #[test]
    fn decomposed_input_matches_precomposed_text() {
        let now = Instant::now();
        let mut session = TypingSession::new("café", SessionOptions::default());
        type_text(&mut session, "cafe\u{301}", now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(session.stats(now).accuracy, 100.0);
        assert_eq!(session.stats(now).errors, 0);
    }

    #[test]
    fn a_bare_letter_for_an_accented_one_is_an_error_once_the_player_moves_on() {
        let now = Instant::now();
        let mut session = TypingSession::new("été ok", SessionOptions::default());
        type_text(&mut session, "ete o", now);
        let stats = session.stats(now);
        assert_eq!(stats.errors, 2);
        assert!(stats.accuracy < 100.0);
    }

    #[test]
    fn retyping_a_missing_accent_keeps_the_error() {
        let now = Instant::now();
        let mut session = TypingSession::new("été ok", SessionOptions::default());
        type_text(&mut session, "ete ok", now);
        for _ in 0..6 {
            session.backspace(now);
        }
        type_text(&mut session, "été ok", now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(session.stats(now).errors, 2);
    }

    #[test]
    fn erasing_an_unfinished_accent_counts_it_as_an_error() {
        let now = Instant::now();
        let mut session = TypingSession::new("é", SessionOptions::default());
        session.type_char('e', now);
        assert_eq!(session.stats(now).errors, 0, "the accent may still come");
        session.backspace(now);
        assert_eq!(session.stats(now).errors, 1);
    }

    #[test]
    fn an_accent_still_missing_at_the_time_limit_is_an_error() {
        let start = Instant::now();
        let options = SessionOptions {
            time_limit: Some(Duration::from_secs(5)),
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new("café", options);
        type_text(&mut session, "cafe", start);
        session.update(at(start, 5_000));
        assert_eq!(session.status(), Status::TimeUp);
        assert_eq!(session.stats(at(start, 5_000)).errors, 1);
    }

    #[test]
    fn a_wrong_accent_is_a_single_error() {
        let now = Instant::now();
        let mut session = TypingSession::new("é", SessionOptions::default());
        type_text(&mut session, "e\u{300}", now);
        assert_eq!(session.mark(0), Mark::Incorrect);
        assert_eq!(session.stats(now).errors, 1);
        session.backspace(now);
        assert_eq!(session.stats(now).errors, 1);
    }

    #[test]
    fn errors_never_decrease() {
        let now = Instant::now();
        let mut session = TypingSession::new("été où ça", SessionOptions::default());
        let mut highest = 0;
        let mut check = |session: &TypingSession| {
            let errors = session.stats(now).errors;
            assert!(errors >= highest, "{errors} < {highest}");
            highest = errors;
        };
        for ch in "e\u{301}tex o\u{300}".chars() {
            session.type_char(ch, now);
            check(&session);
        }
        while session.backspace(now) {
            check(&session);
        }
        for ch in "ete ou c".chars() {
            session.type_char(ch, now);
            check(&session);
        }
        assert!(session.delete_word(now));
        check(&session);
        assert_eq!(highest, 9);
    }

    #[test]
    fn an_emoji_typed_code_point_by_code_point_has_no_errors() {
        let now = Instant::now();
        let mut session = TypingSession::new("👩‍💻!", SessionOptions::default());
        type_text(&mut session, "👩\u{200D}💻!", now);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(session.stats(now).errors, 0);
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
        assert_eq!(stats.correct_chars, 9);
        assert_eq!(stats.indentation, 4);
        assert_eq!(stats.typed_chars, 5, "indentation is not typed");
        assert_eq!(stats.accuracy, 100.0);
    }

    /// Types `keys` one every `interval_ms` from `start`, `\u{8}` standing for
    /// Backspace, and returns the instant of the last key.
    fn play(session: &mut TypingSession, keys: &str, start: Instant, interval_ms: u64) -> Instant {
        let mut now = start;
        for (index, key) in (0..).zip(keys.chars()) {
            now = at(start, index * interval_ms);
            if key == '\u{8}' {
                session.backspace(now);
            } else {
                session.type_char(key, now);
            }
        }
        now
    }

    fn auto_indented(text: &str) -> TypingSession {
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        TypingSession::new(text, options)
    }

    #[test]
    fn auto_filled_indentation_counts_for_completion_but_not_for_speed() {
        let start = Instant::now();
        let mut session = auto_indented("fn f() {\n    x();\n}");
        let end = play(&mut session, "fn f() {\nx();\n}", start, 100);
        assert_eq!(session.status(), Status::Completed);
        assert_eq!(
            session.tally(),
            Tally {
                typed: 19,
                correct: 19,
                indentation: 4,
                keystrokes: 15,
                errors: 0,
            }
        );
        let stats = session.stats(end);
        assert_eq!(stats.progress, 1.0);
        let fifteen_keys = words_per_minute(15, Duration::from_millis(1_400));
        assert_eq!(stats.wpm, fifteen_keys);
        assert_eq!(stats.raw_wpm, fifteen_keys);
        assert_eq!(stats.accuracy, 100.0);
    }

    #[test]
    fn the_last_sample_matches_the_headline_and_samples_add_up_to_its_errors() {
        let start = Instant::now();
        let mut session = auto_indented("if ok {\n    go();\n}");
        let end = play(&mut session, "if oj\u{8}k {\ngo();\n}", start, 200);
        assert_eq!(session.status(), Status::Completed);
        let stats = session.stats(end);
        let samples = session.samples(end);
        assert_eq!(samples.len(), 3);
        assert_eq!(samples.last().map(|sample| sample.wpm), Some(stats.wpm));
        let errors: u32 = samples.iter().map(|sample| sample.errors).sum();
        assert_eq!(errors as usize, stats.errors);
        assert_eq!(stats.errors, 1);
    }

    #[test]
    fn deleted_characters_do_not_count_towards_the_samples() {
        let start = Instant::now();
        let mut session = TypingSession::new("ab cd", SessionOptions::default());
        play(&mut session, "ab\u{8}\u{8}ab", start, 100);
        let second = at(start, 1_000);
        let two_characters = words_per_minute(2, Duration::from_secs(1));
        assert_eq!(session.stats(second).wpm, two_characters);
        let samples = session.samples(second);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].wpm, two_characters);
        assert_eq!(
            samples[0].raw_wpm,
            words_per_minute(4, Duration::from_secs(1))
        );
    }

    #[test]
    fn a_session_shorter_than_a_second_has_one_steady_sample() {
        let start = Instant::now();
        let mut session = TypingSession::new("hello", SessionOptions::default());
        let end = play(&mut session, "hello", start, 150);
        let samples = session.samples(end);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].wpm, session.stats(end).wpm);
        assert_eq!(consistency(&samples), 100.0);
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
    fn delete_word_after_an_auto_indented_newline_keeps_the_previous_line() {
        let now = Instant::now();
        let mut session = auto_indented("fn main() {\n    x\n}");
        type_text(&mut session, "fn main() {\n", now);
        assert_eq!(session.cursor(), 16);
        assert!(session.delete_word(now));
        assert_eq!(
            session.cursor(),
            11,
            "only the line break and its indentation go"
        );
        assert_eq!(session.mark(10), Mark::Correct, "the brace stays typed");
    }

    #[test]
    fn delete_word_never_crosses_a_line_break() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab\n  cd ef", SessionOptions::default());
        type_text(&mut session, "ab\n  cd", now);
        assert!(session.delete_word(now));
        assert_eq!(
            session.cursor(),
            5,
            "the word, not the indentation before it"
        );
        assert!(session.delete_word(now));
        assert_eq!(
            session.cursor(),
            2,
            "the typed indentation and its line break"
        );
        assert!(session.delete_word(now));
        assert_eq!(session.cursor(), 0);
    }

    #[test]
    fn delete_word_right_after_a_newline_removes_only_the_newline() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab\ncd", SessionOptions::default());
        type_text(&mut session, "ab\n", now);
        assert!(session.delete_word(now));
        assert_eq!(session.cursor(), 2);
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
    fn samples_cover_seconds_and_the_last_one_takes_a_short_remainder() {
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
        let end = at(start, 3_200);
        let samples = session.samples(end);
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0].raw_wpm, 120.0);
        assert_eq!(samples[1].errors, 1);
        assert_eq!(
            samples[2].raw_wpm,
            words_per_minute(4, Duration::from_millis(1_200))
        );
        assert_eq!(
            samples[2].wpm,
            words_per_minute(15, Duration::from_millis(3_200))
        );
        assert_eq!(samples[2].wpm, session.stats(end).wpm);
    }

    fn completes_without_errors(text: &str, typed: &str) {
        let now = Instant::now();
        let mut session = TypingSession::new(text, SessionOptions::default());
        type_text(&mut session, typed, now);
        assert_eq!(
            session.status(),
            Status::Completed,
            "{typed:?} for {text:?}"
        );
        assert_eq!(session.stats(now).errors, 0, "{typed:?} for {text:?}");
    }

    #[test]
    fn typed_spaces_of_any_kind_match_the_plain_space_of_the_text() {
        completes_without_errors("Vraiment ?", "Vraiment\u{a0}?");
        completes_without_errors("Vraiment ?", "Vraiment\u{202f}?");
    }

    #[test]
    fn typed_typographic_characters_match_their_plain_form_in_the_text() {
        completes_without_errors("l'\u{e9}t\u{e9}", "l\u{2019}\u{e9}t\u{e9}");
        completes_without_errors("\"a\" - b", "\u{ab}a\u{bb} \u{2013} b");
        completes_without_errors("a...", "a\u{2026}");
        completes_without_errors("a -> b", "a \u{2192} b");
    }

    #[test]
    fn typed_invisible_characters_are_refused() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab", SessionOptions::default());
        session.type_char('a', now);
        assert!(!session.type_char('\u{200b}', now));
        assert_eq!(session.cursor(), 1);
        assert_eq!(session.stats(now).errors, 0);
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
