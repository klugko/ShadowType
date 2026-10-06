//! What every member of a room sees about it.

use std::ops::RangeInclusive;

use code_racer_engine::TextSource;
use serde::{Deserialize, Serialize};

use crate::ids::{PlayerId, RoomCode, Username};

/// Word counts accepted for a race.
pub const RACE_WORD_COUNTS: RangeInclusive<u16> = 5..=200;

/// Whether a text can be raced on: every quote and snippet can, words within [`RACE_WORD_COUNTS`].
pub fn is_raceable(text: &TextSource) -> bool {
    match text {
        TextSource::Words { count, .. } => RACE_WORD_COUNTS.contains(count),
        TextSource::Quote { .. } | TextSource::Code { .. } => true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Players gather and get ready.
    Lobby,
    /// The text is known and the race starts when the countdown ends.
    Countdown,
    Racing,
    /// Everyone finished, left, or the race timed out.
    Finished,
}

/// Complete state of a room, sent to its members whenever it changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomView {
    pub code: RoomCode,
    pub host: PlayerId,
    pub text: TextSource,
    /// Characters in the current race text, zero before the first race.
    pub text_length: u32,
    pub phase: Phase,
    pub max_players: u8,
    /// Players in the order they joined.
    pub players: Vec<PlayerView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: PlayerId,
    pub name: Username,
    pub ready: bool,
    /// False once the player lost their connection during a race.
    pub connected: bool,
    pub progress: PlayerProgress,
}

/// A player's race progress as validated and timed by the server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerProgress {
    pub typed: u32,
    pub correct: u32,
    pub errors: u32,
    pub wpm: f64,
    pub accuracy: f64,
    /// Race time measured by the server, once the whole text is typed.
    pub finish_ms: Option<u64>,
}

impl PlayerProgress {
    pub fn is_finished(&self) -> bool {
        self.finish_ms.is_some()
    }

    /// Share of the text typed correctly, between 0 and 1.
    pub fn fraction(&self, text_length: u32) -> f64 {
        if text_length == 0 {
            0.0
        } else {
            (f64::from(self.correct) / f64::from(text_length)).clamp(0.0, 1.0)
        }
    }
}

impl RoomView {
    pub fn player(&self, id: PlayerId) -> Option<&PlayerView> {
        self.players.iter().find(|player| player.id == id)
    }

    pub fn is_host(&self, id: PlayerId) -> bool {
        self.host == id
    }

    pub fn everyone_ready(&self) -> bool {
        self.players.iter().all(|player| player.ready)
    }

    /// Players from first to last place.
    ///
    /// Finishers come first, fastest first. Everyone else is ranked by how
    /// much of the text they typed correctly; ties keep the joining order.
    pub fn standings(&self) -> Vec<&PlayerView> {
        let mut standings: Vec<&PlayerView> = self.players.iter().collect();
        standings.sort_by_key(|player| {
            let progress = player.progress;
            (
                progress.finish_ms.is_none(),
                progress.finish_ms.unwrap_or(u64::MAX),
                std::cmp::Reverse(progress.correct),
            )
        });
        standings
    }

    /// 1-based place of a player in [`Self::standings`].
    pub fn place_of(&self, id: PlayerId) -> Option<usize> {
        self.standings()
            .iter()
            .position(|player| player.id == id)
            .map(|index| index + 1)
    }
}

#[cfg(test)]
mod tests {
    use code_racer_engine::{CodeLanguage, Language, WordOptions};

    use super::*;

    fn player(id: u64, name: &str, correct: u32, finish_ms: Option<u64>) -> PlayerView {
        PlayerView {
            id: PlayerId(id),
            name: name.parse().expect("valid name"),
            ready: true,
            connected: true,
            progress: PlayerProgress {
                typed: correct,
                correct,
                finish_ms,
                ..PlayerProgress::default()
            },
        }
    }

    fn room(players: Vec<PlayerView>) -> RoomView {
        RoomView {
            code: "ABC234".parse().expect("valid code"),
            host: PlayerId(1),
            text: TextSource::Quote {
                language: Language::English,
            },
            text_length: 100,
            phase: Phase::Finished,
            max_players: 8,
            players,
        }
    }

    #[test]
    fn finishers_rank_by_time_then_others_by_progress() {
        let room = room(vec![
            player(1, "slow", 100, Some(9_000)),
            player(2, "behind", 40, None),
            player(3, "fast", 100, Some(7_500)),
            player(4, "close", 90, None),
            player(5, "tied", 40, None),
        ]);
        let names: Vec<&str> = room
            .standings()
            .iter()
            .map(|player| player.name.as_str())
            .collect();
        assert_eq!(names, ["fast", "slow", "close", "behind", "tied"]);
        assert_eq!(room.place_of(PlayerId(4)), Some(3));
        assert_eq!(room.place_of(PlayerId(42)), None);
    }

    #[test]
    fn fraction_is_bounded() {
        let progress = PlayerProgress {
            correct: 50,
            ..PlayerProgress::default()
        };
        assert_eq!(progress.fraction(100), 0.5);
        assert_eq!(progress.fraction(0), 0.0);
        assert_eq!(progress.fraction(10), 1.0);
    }

    #[test]
    fn race_texts_are_bounded() {
        let words = |count| TextSource::Words {
            language: Language::French,
            count,
            options: WordOptions::default(),
        };
        assert!(is_raceable(&words(30)));
        assert!(!is_raceable(&words(4)));
        assert!(!is_raceable(&words(201)));
        assert!(is_raceable(&TextSource::Code {
            language: CodeLanguage::Sql
        }));
    }
}
