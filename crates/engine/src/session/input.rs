use std::{iter, time::Instant};

use unicode_segmentation::UnicodeSegmentation;

use super::{
    Entry, TypingSession,
    grading::{Judgement, judge},
};
use crate::{indentation::indentation_run, normalize::keyboard_form};

impl TypingSession {
    /**
     * Types one character. Returns whether the input was accepted.
     *
     * The character is first folded like [`normalize`](crate::normalize())
     * folds texts, so a typed no-break space, `’` or `…` stands for the
     * space, `'` or `...` of the text, and characters that display as
     * nothing are refused. Combining characters merge into the previous
     * character. Input is refused once the session is over, past the end of
     * the text, or once [`ERROR_RUN_LIMIT`](super::ERROR_RUN_LIMIT)
     * characters have been typed from the first uncorrected mistake onward.
     */
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

    /**
     * Removes the last typed character, or the whole automatic indentation
     * together with the newline that produced it.
     */
    pub fn backspace(&mut self, now: Instant) -> bool {
        self.erase(now, Self::pop_entry)
    }

    /**
     * Removes the last typed word and the blanks that follow it, like
     * Ctrl+Backspace in an editor: a line break is removed on its own, never
     * together with the line before it.
     */
    pub fn delete_word(&mut self, now: Instant) -> bool {
        self.erase(now, Self::pop_word)
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
        self.history.abandon_pending();
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

    fn fill_indentation(&mut self) {
        let width = indentation_run(&self.target, self.cursor());
        self.entries
            .extend(iter::repeat_n(Entry::auto_space(), width));
    }

    fn complete_if_done(&mut self, now: Instant) {
        let done =
            self.cursor() == self.target.len() && self.entries.iter().all(|entry| entry.correct);
        if done && self.finished_at.is_none() {
            self.finished_at = Some(now);
        }
    }

    /// Applies a deletion, which leaves any character in progress unfinished.
    fn erase(&mut self, now: Instant, remove: fn(&mut Self)) -> bool {
        self.update(now);
        if self.is_finished() || self.entries.is_empty() {
            return false;
        }
        self.history.abandon_pending();
        remove(self);
        self.record_checkpoint(now);
        true
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

    fn record_keystroke(&mut self, judgement: Judgement, now: Instant) {
        let at = self.elapsed(now);
        self.history.record_keystroke(judgement, at);
    }

    fn record_checkpoint(&mut self, now: Instant) {
        let at = self.elapsed(now);
        let correctly_typed = self.tally().correctly_typed();
        self.history.record_checkpoint(at, correctly_typed);
    }
}
