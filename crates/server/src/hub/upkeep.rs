use std::time::{Duration, Instant};

use code_racer_protocol::{ErrorCode, RoomCode, ServerMessage};

use super::Hub;

impl Hub {
    pub fn tick(&mut self, now: Instant) {
        let race_timeout = self.config.race_timeout;
        for (code, room) in &mut self.rooms {
            if room.advance(now, race_timeout) {
                self.dirty.insert(code.clone());
            }
        }
        for code in std::mem::take(&mut self.dirty) {
            self.broadcast_view(&code);
        }
        self.close_idle_rooms(now);
        self.drop_stuck_players(now);
    }

    pub(super) fn advance_room(&mut self, code: &RoomCode, now: Instant) {
        let race_timeout = self.config.race_timeout;
        let changed = self
            .rooms
            .get_mut(code)
            .is_some_and(|room| room.advance(now, race_timeout));
        if changed {
            self.broadcast_view(code);
        }
    }

    fn close_idle_rooms(&mut self, now: Instant) {
        let ttl = self.config.room_ttl;
        let idle: Vec<RoomCode> = self
            .rooms
            .iter()
            .filter(|(_, room)| room.idle_for(now) >= ttl)
            .map(|(code, _)| code.clone())
            .collect();
        for code in idle {
            self.close_room(&code);
        }
    }

    fn close_room(&mut self, code: &RoomCode) {
        let notice = ServerMessage::error(
            ErrorCode::RoomNotFound,
            format!(
                "room {code} closed after {} of inactivity",
                describe(self.config.room_ttl)
            ),
        );
        self.broadcast(code, &notice);
        if let Some(room) = self.rooms.remove(code) {
            for id in room.connected_members() {
                if let Some(player) = self.players.get_mut(&id) {
                    player.room = None;
                }
            }
        }
        self.dirty.remove(code);
    }
}

fn describe(duration: Duration) -> String {
    let (count, unit) = match duration.as_secs() {
        seconds @ 0..60 => (seconds, "second"),
        seconds => (seconds / 60, "minute"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_notices_use_readable_durations() {
        assert_eq!(describe(Duration::from_secs(1_800)), "30 minutes");
        assert_eq!(describe(Duration::from_secs(60)), "1 minute");
        assert_eq!(describe(Duration::from_secs(45)), "45 seconds");
        assert_eq!(describe(Duration::from_secs(1)), "1 second");
    }
}
