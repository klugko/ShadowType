use std::time::{Duration, Instant};

use code_racer_protocol::{
    ErrorCode, PlayerId, PlayerProgress, PlayerView, Progress, ServerError, Username,
};

use super::{
    Room,
    race::{Race, Stage},
};

#[derive(Debug)]
pub(super) struct Member {
    id: PlayerId,
    name: Username,
    ready: bool,
    pub(super) connected: bool,
    pub(super) progress: PlayerProgress,
    pub(super) counters: Progress,
}

impl Member {
    pub(super) fn new(id: PlayerId, name: Username) -> Self {
        Self {
            id,
            name,
            ready: false,
            connected: true,
            progress: PlayerProgress::default(),
            counters: Progress::default(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.ready = false;
        self.progress = PlayerProgress::default();
        self.counters = Progress::default();
    }

    pub(super) fn record(&mut self, counters: Progress, race: &Race, now: Instant) {
        let elapsed = race.elapsed(now);
        let tally = counters.tally();
        self.progress = PlayerProgress {
            typed: counters.typed,
            correct: counters.correct,
            errors: counters.errors,
            wpm: tally.wpm(elapsed),
            accuracy: tally.accuracy(),
            finish_ms: (counters.correct == race.length).then(|| millis(elapsed)),
        };
        self.counters = counters;
    }

    pub(super) fn view(&self) -> PlayerView {
        PlayerView {
            id: self.id,
            name: self.name.clone(),
            ready: self.ready,
            connected: self.connected,
            progress: self.progress,
        }
    }
}

impl Room {
    /**
     * Adds a player to the lobby. Joining a room one is already in changes
     * nothing, while a player who left after a race was announced stays
     * listed offline and may only come back once the room is in the lobby
     * again.
     */
    pub fn join(&mut self, id: PlayerId, name: Username, now: Instant) -> Result<(), ServerError> {
        let already_in = self
            .members
            .iter()
            .any(|member| member.id == id && member.connected);
        if already_in {
            return Ok(());
        }
        self.require_lobby()?;
        if self.members.len() >= usize::from(self.max_players) {
            return Err(ServerError::new(
                ErrorCode::RoomFull,
                format!("room {} is full", self.code),
            ));
        }
        self.members.push(Member::new(id, name));
        self.last_activity = now;
        Ok(())
    }

    /**
     * Removes a player from the lobby. Once a race is announced, they stay
     * listed as disconnected until the room returns to the lobby, so that
     * the standings and the results stay complete.
     */
    pub fn leave(&mut self, id: PlayerId, now: Instant) {
        let Some(index) = self.position(id) else {
            return;
        };
        match self.stage {
            Stage::Lobby => {
                self.members.remove(index);
            }
            Stage::Countdown(_) | Stage::Racing(_) | Stage::Finished(_) => {
                self.members[index].connected = false;
            }
        }
        if self.host == id {
            self.promote_next_host();
        }
        self.last_activity = now;
    }

    pub fn set_ready(
        &mut self,
        id: PlayerId,
        ready: bool,
        now: Instant,
    ) -> Result<(), ServerError> {
        let index = self.member_index(id)?;
        self.require_lobby()?;
        self.members[index].ready = ready;
        self.last_activity = now;
        Ok(())
    }

    pub fn connected_members(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.members
            .iter()
            .filter(|member| member.connected)
            .map(|member| member.id)
    }

    pub fn has_connected_members(&self) -> bool {
        self.connected_members().next().is_some()
    }

    pub(super) fn everyone_finished(&self) -> bool {
        self.members
            .iter()
            .filter(|member| member.connected)
            .all(|member| member.progress.is_finished())
    }

    pub(super) fn require_everyone_ready(&self) -> Result<(), ServerError> {
        let waiting: Vec<&str> = self
            .members
            .iter()
            .filter(|member| !member.ready)
            .map(|member| member.name.as_str())
            .collect();
        if waiting.is_empty() {
            Ok(())
        } else {
            Err(ServerError::new(
                ErrorCode::PlayersNotReady,
                format!("waiting for {} to get ready", waiting.join(", ")),
            ))
        }
    }

    pub(super) fn require_host(&self, id: PlayerId, action: &str) -> Result<(), ServerError> {
        self.member_index(id)?;
        if id == self.host {
            Ok(())
        } else {
            Err(ServerError::new(
                ErrorCode::NotHost,
                format!("only the host can {action}"),
            ))
        }
    }

    pub(super) fn member_index(&self, id: PlayerId) -> Result<usize, ServerError> {
        self.position(id).ok_or_else(|| {
            ServerError::new(
                ErrorCode::NotInRoom,
                format!("you are not in room {}", self.code),
            )
        })
    }

    fn position(&self, id: PlayerId) -> Option<usize> {
        self.members.iter().position(|member| member.id == id)
    }

    fn promote_next_host(&mut self) {
        let successor = self.connected_members().next();
        self.host = successor.unwrap_or(self.host);
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
