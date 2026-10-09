//! What the screen and the results need to know about the keys typed that the
//! typing engine does not keep: when each character was typed (for drying ink
//! and log stamps), where the cursor went (for its trail), and what was missed.

use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use code_racer_engine::{Mark, TypingSession};

use super::text_event::TextEvent;

/// How long typed text takes to settle into its final colour.
pub const DRYING_TIME: Duration = Duration::from_millis(450);
/// How long the trail of the cursor takes to fade.
pub const TRAIL_TIME: Duration = Duration::from_millis(320);
/// Most moves of the cursor kept for its trail: far more than fit in
/// [`TRAIL_TIME`] at any typing speed.
const TRAIL_MOVES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Move {
    at: Instant,
    from: usize,
    to: usize,
}

impl Move {
    /// How far along this move `position` lies, from just above 0 where
    /// the cursor left to 1 next to where it arrived; `None` off its path.
    fn along(self, position: usize) -> Option<f64> {
        let (length, distance) = if self.from < self.to {
            let on_path = (self.from..self.to).contains(&position);
            (self.to - self.from, on_path.then(|| self.to - 1 - position))
        } else {
            let on_path = (self.to + 1..=self.from).contains(&position);
            (self.from - self.to, on_path.then(|| position - self.to - 1))
        };
        Some(1.0 - distance? as f64 / length as f64)
    }
}

/// The keys typed in one text.
#[derive(Debug, Clone, Default)]
pub struct Ink {
    /// When each character before the cursor was typed, by position.
    typed_at: Vec<Instant>,
    /// When the first character was typed, by the monotonic and the wall clock.
    started: Option<(Instant, DateTime<Local>)>,
    last_key: Option<Instant>,
    last_mistake: Option<Instant>,
    /// The last moves of the cursor, the latest last.
    moves: VecDeque<Move>,
    /// How many times each expected character was typed wrong.
    missed: BTreeMap<String, u32>,
}

impl Ink {
    /// Hands `event` to `session`, noting what it changed. Returns whether
    /// the session took the key, like [`TextEvent::apply_to`].
    pub fn apply(&mut self, event: TextEvent, session: &mut TypingSession, now: Instant) -> bool {
        if event == TextEvent::Tick {
            return event.apply_to(session, now);
        }
        let before = session.cursor();
        let errors = session.tally().errors;
        let taken = event.apply_to(session, now);
        self.last_key = Some(now);
        let after = session.cursor();
        if after != before {
            if self.moves.len() == TRAIL_MOVES {
                self.moves.pop_front();
            }
            self.moves.push_back(Move {
                at: now,
                from: before,
                to: after,
            });
        }
        self.typed_at.truncate(before.min(after));
        if after > before {
            self.started.get_or_insert_with(|| (now, Local::now()));
            self.typed_at.resize(after, now);
        }
        if session.tally().errors > errors {
            self.last_mistake = Some(now);
            if let Some(expected) = missed_at(session, before) {
                *self.missed.entry(expected).or_default() += 1;
            }
        }
        taken
    }

    pub fn typed_at(&self, position: usize) -> Option<Instant> {
        self.typed_at.get(position).copied()
    }

    pub fn wall_time(&self, position: usize) -> Option<DateTime<Local>> {
        let (start, wall) = self.started?;
        let at = self.typed_at(position)?;
        let offset = chrono::Duration::from_std(at.saturating_duration_since(start)).ok()?;
        Some(wall + offset)
    }

    pub fn last_key(&self) -> Option<Instant> {
        self.last_key
    }

    pub fn last_mistake(&self) -> Option<Instant> {
        self.last_mistake
    }

    /// Whether some text typed lately is still drying at `now`.
    pub fn is_wet(&self, now: Instant) -> bool {
        self.last_key
            .is_some_and(|at| now.saturating_duration_since(at) < DRYING_TIME)
    }

    /// How much of the cursor's trail lies on `position` at `now`, from 0
    /// to 1: the most right behind the cursor, where it just was, fading
    /// out along the way it came and as time passes. A jump, such as a word
    /// erased or a line indented, leaves a streak all along it.
    pub fn trail(&self, position: usize, now: Instant) -> f64 {
        self.moves
            .iter()
            .rev()
            .map_while(|step| {
                let age = now.saturating_duration_since(step.at);
                (age < TRAIL_TIME).then(|| {
                    let fade = 1.0 - age.as_secs_f64() / TRAIL_TIME.as_secs_f64();
                    step.along(position)
                        .map_or(0.0, |along| along * fade * fade)
                })
            })
            .fold(0.0, f64::max)
    }

    pub fn is_trailing(&self, now: Instant) -> bool {
        self.moves
            .back()
            .is_some_and(|step| now.saturating_duration_since(step.at) < TRAIL_TIME)
    }

    pub fn most_missed(&self, count: usize) -> Vec<(String, u32)> {
        let mut missed: Vec<(String, u32)> = self
            .missed
            .iter()
            .map(|(expected, times)| (expected.clone(), *times))
            .collect();
        missed.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        missed.truncate(count);
        missed
    }
}

/// The character the player was expected to type when a keystroke went
/// wrong at `position`: the one there, or the one before it when moving on
/// left an accent unfinished there.
fn missed_at(session: &TypingSession, position: usize) -> Option<String> {
    let wrong = |index: usize| session.mark(index) == Mark::Incorrect;
    let index = if position < session.cursor() && wrong(position) {
        position
    } else {
        position.checked_sub(1).filter(|index| wrong(*index))?
    };
    session.target().get(index).cloned()
}

#[cfg(test)]
mod tests {
    use code_racer_engine::SessionOptions;

    use super::*;

    fn typed(ink: &mut Ink, session: &mut TypingSession, text: &str, now: Instant) {
        for ch in text.chars() {
            ink.apply(TextEvent::Typed(ch), session, now);
        }
    }

    #[test]
    fn every_typed_character_keeps_when_it_was_typed() {
        let start = Instant::now();
        let later = start + Duration::from_secs(1);
        let mut session = TypingSession::new("abc def", SessionOptions::default());
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "ab", start);
        typed(&mut ink, &mut session, "c", later);
        assert_eq!(ink.typed_at(1), Some(start));
        assert_eq!(ink.typed_at(2), Some(later));
        assert_eq!(ink.typed_at(3), None);
        ink.apply(TextEvent::Backspace, &mut session, later);
        assert_eq!(ink.typed_at(2), None, "erased characters forget it");
        assert!(ink.is_wet(later));
        assert!(!ink.is_wet(later + DRYING_TIME));
    }

    #[test]
    fn auto_indentation_is_stamped_with_the_newline() {
        let now = Instant::now();
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new("a\n    b", options);
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "a\n", now);
        assert_eq!(session.cursor(), 6);
        assert_eq!(ink.typed_at(5), Some(now));
    }

    #[test]
    fn mistakes_count_the_character_expected() {
        let now = Instant::now();
        let mut session = TypingSession::new("the then", SessionOptions::default());
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "tge", now);
        ink.apply(TextEvent::Backspace, &mut session, now);
        ink.apply(TextEvent::Backspace, &mut session, now);
        typed(&mut ink, &mut session, "hw", now);
        assert_eq!(
            ink.most_missed(5),
            [("e".to_owned(), 1), ("h".to_owned(), 1)]
        );
        assert_eq!(ink.last_mistake(), Some(now));
        assert_eq!(ink.most_missed(1).len(), 1);
    }

    #[test]
    fn a_refused_key_is_no_mistake() {
        let now = Instant::now();
        let mut session = TypingSession::new("ab", SessionOptions::default());
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "ab", now);
        assert!(!ink.apply(TextEvent::Typed('c'), &mut session, now));
        assert!(ink.most_missed(5).is_empty());
        assert_eq!(ink.last_mistake(), None);
    }

    #[test]
    fn the_cursor_leaves_a_trail_that_fades_behind_it() {
        let start = Instant::now();
        let mut session = TypingSession::new("abcdef", SessionOptions::default());
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "a", start);
        typed(&mut ink, &mut session, "b", start + TRAIL_TIME / 4);
        let now = start + TRAIL_TIME / 4;
        let (older, newer) = (ink.trail(0, now), ink.trail(1, now));
        assert!(newer > older && older > 0.0, "{older} then {newer}");
        assert_eq!(ink.trail(2, now), 0.0, "nothing ahead of the cursor");
        assert!(ink.is_trailing(now));
        let later = start + TRAIL_TIME / 4 + TRAIL_TIME;
        assert_eq!(ink.trail(1, later), 0.0, "faded");
        assert!(!ink.is_trailing(later));
    }

    #[test]
    fn a_word_erased_at_once_leaves_a_streak_to_the_cursor() {
        let now = Instant::now();
        let mut session = TypingSession::new("one two three", SessionOptions::default());
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "one two", now);
        let later = now + TRAIL_TIME;
        ink.apply(TextEvent::DeleteWord, &mut session, later);
        assert_eq!(session.cursor(), 4);
        let streak: Vec<f64> = (5..=7).map(|position| ink.trail(position, later)).collect();
        assert!(streak.iter().all(|strength| *strength > 0.0), "{streak:?}");
        assert!(streak[0] > streak[2], "brightest by the cursor: {streak:?}");
        assert_eq!(ink.trail(4, later), 0.0, "the cursor itself");
    }

    #[test]
    fn wall_times_follow_the_typing() {
        let start = Instant::now();
        let mut session = TypingSession::new("ab", SessionOptions::default());
        let mut ink = Ink::default();
        typed(&mut ink, &mut session, "a", start);
        typed(&mut ink, &mut session, "b", start + Duration::from_secs(2));
        let first = ink.wall_time(0).expect("typed");
        let second = ink.wall_time(1).expect("typed");
        assert_eq!((second - first).num_seconds(), 2);
    }
}
