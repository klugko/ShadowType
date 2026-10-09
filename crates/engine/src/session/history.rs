use std::{iter::Peekable, slice, time::Duration};

use super::grading::Judgement;
use crate::stats::{Sample, sample_ends, words_per_minute};

#[derive(Debug, Clone, Copy)]
struct Keystroke {
    at: Duration,
    correct: bool,
}

/**
 * How many characters were correctly typed right after an input, on the
 * basis of [`Tally::correctly_typed`](crate::Tally::correctly_typed).
 */
#[derive(Debug, Clone, Copy)]
struct Checkpoint {
    at: Duration,
    correctly_typed: usize,
}

#[derive(Debug, Clone, Default)]
pub(super) struct History {
    keystrokes: Vec<Keystroke>,
    /**
     * First keystroke of the last character while it is only the beginning
     * of the expected one.
     */
    pending_since: Option<usize>,
    checkpoints: Vec<Checkpoint>,
}

impl History {
    pub(super) fn keystrokes(&self) -> usize {
        self.keystrokes.len()
    }

    pub(super) fn errors(&self) -> usize {
        self.keystrokes.iter().filter(|k| !k.correct).count()
    }

    pub(super) fn record_keystroke(&mut self, judgement: Judgement, at: Duration) {
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
    pub(super) fn abandon_pending(&mut self) {
        if let Some(first) = self.pending_since.take() {
            for keystroke in &mut self.keystrokes[first..] {
                keystroke.correct = false;
            }
        }
    }

    pub(super) fn record_checkpoint(&mut self, at: Duration, correctly_typed: usize) {
        self.checkpoints.push(Checkpoint {
            at,
            correctly_typed,
        });
    }

    pub(super) fn samples(&self, elapsed: Duration) -> Vec<Sample> {
        let mut replay = Replay::new(&self.keystrokes, &self.checkpoints);
        sample_ends(elapsed)
            .into_iter()
            .zip(1..)
            .map(|(end, second)| replay.sample(second, end))
            .collect()
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
