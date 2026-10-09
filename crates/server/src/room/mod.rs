/*!
 * A room and its races as a pure state machine.
 *
 * Nothing here performs I/O or reads the clock: every operation receives the
 * current [`Instant`], which keeps the rules deterministic and testable. The
 * hub owns the rooms and turns their state into messages.
 */

mod members;
mod progress;
mod race;

use std::time::{Duration, Instant};

use code_racer_engine::TextSource;
use code_racer_protocol::{PlayerId, RoomCode, RoomView, Username};

use members::Member;
use race::Stage;

#[derive(Debug)]
pub struct Room {
    code: RoomCode,
    host: PlayerId,
    text: TextSource,
    max_players: u8,
    stage: Stage,
    members: Vec<Member>,
    last_activity: Instant,
}

impl Room {
    pub fn new(
        code: RoomCode,
        host: PlayerId,
        name: Username,
        text: TextSource,
        max_players: u8,
        now: Instant,
    ) -> Self {
        Self {
            code,
            host,
            text,
            max_players,
            stage: Stage::Lobby,
            members: vec![Member::new(host, name)],
            last_activity: now,
        }
    }

    pub fn text_source(&self) -> TextSource {
        self.text
    }

    pub fn view(&self) -> RoomView {
        RoomView {
            code: self.code.clone(),
            host: self.host,
            text: self.text,
            text_length: self.stage.race().map_or(0, |race| race.length),
            phase: self.stage.phase(),
            max_players: self.max_players,
            players: self.members.iter().map(Member::view).collect(),
        }
    }

    pub fn idle_for(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.last_activity)
    }
}

#[cfg(test)]
mod tests;
