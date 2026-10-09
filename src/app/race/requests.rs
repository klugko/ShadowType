use std::time::{Duration, Instant};

use code_racer_protocol::ClientMessage;

use super::RaceClient;

/**
 * Shortest time between two presses of the same room request. A held key
 * repeats faster, and terminals without keyboard enhancement report its
 * repeats as new presses, which would trip the server's message limit.
 */
pub(super) const REQUEST_REPEAT: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomRequest {
    ToggleReady,
    Start,
    /// Back to the lobby for another race, host only.
    Again,
}

impl RaceClient {
    /**
     * Sends `request` pressed at `now`, unless the previous one is still
     * unanswered or the key is only repeating. Returns why it cannot be
     * made, if it cannot.
     */
    pub fn request(&mut self, request: RoomRequest, now: Instant) -> Result<(), String> {
        if self.repeats_last_press(request, now) {
            return Ok(());
        }
        let message = match request {
            RoomRequest::ToggleReady => ClientMessage::SetReady {
                ready: !self.me().is_some_and(|me| me.ready),
            },
            RoomRequest::Start => {
                self.check_start()?;
                ClientMessage::StartRace
            }
            RoomRequest::Again if self.is_host() => ClientMessage::ReturnToLobby,
            RoomRequest::Again => {
                return Err("waiting for the host to start another race".to_owned());
            }
        };
        if !self.awaiting_answer {
            self.awaiting_answer = self.connection.send(message);
        }
        Ok(())
    }

    /**
     * Records a press of `request` and tells whether it repeats the previous
     * press within [`REQUEST_REPEAT`]. Every press restarts the delay, so a
     * key held down acts once.
     */
    fn repeats_last_press(&mut self, request: RoomRequest, now: Instant) -> bool {
        let repeats = self.last_press.is_some_and(|(last, at)| {
            last == request && now.saturating_duration_since(at) < REQUEST_REPEAT
        });
        self.last_press = Some((request, now));
        repeats
    }

    fn check_start(&self) -> Result<(), String> {
        let room = self.room.as_ref().ok_or("not in a room")?;
        if !self.is_host() {
            return Err("only the host can start the race".to_owned());
        }
        if !room.everyone_ready() {
            return Err("waiting for every player to be ready".to_owned());
        }
        Ok(())
    }
}
