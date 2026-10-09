use std::time::{Duration, Instant};

use code_racer_engine::{Indentation, grapheme_count};
use code_racer_protocol::{ErrorCode, Phase, PlayerId, ServerError};

use super::{Room, members::Member};

#[derive(Debug, Clone)]
pub(super) struct Race {
    pub(super) length: u32,
    pub(super) indentation: Indentation,
    announced_at: Instant,
    countdown: Duration,
}

impl Race {
    fn announce(text: &str, now: Instant, countdown: Duration) -> Self {
        Self {
            length: u32::try_from(grapheme_count(text)).unwrap_or(u32::MAX),
            indentation: Indentation::of(text),
            announced_at: now,
            countdown,
        }
    }

    fn countdown_over(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.announced_at) >= self.countdown
    }

    pub(super) fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.announced_at)
            .saturating_sub(self.countdown)
    }
}

#[derive(Debug, Clone)]
pub(super) enum Stage {
    Lobby,
    Countdown(Race),
    Racing(Race),
    Finished(Race),
}

impl Stage {
    pub(super) fn phase(&self) -> Phase {
        match self {
            Self::Lobby => Phase::Lobby,
            Self::Countdown(_) => Phase::Countdown,
            Self::Racing(_) => Phase::Racing,
            Self::Finished(_) => Phase::Finished,
        }
    }

    pub(super) fn race(&self) -> Option<&Race> {
        match self {
            Self::Lobby => None,
            Self::Countdown(race) | Self::Racing(race) | Self::Finished(race) => Some(race),
        }
    }
}

impl Room {
    pub fn start_countdown(
        &mut self,
        by: PlayerId,
        text: &str,
        now: Instant,
        countdown: Duration,
    ) -> Result<(), ServerError> {
        self.require_host(by, "start the race")?;
        self.require_lobby()?;
        self.require_everyone_ready()?;
        self.stage = Stage::Countdown(Race::announce(text, now, countdown));
        self.last_activity = now;
        Ok(())
    }

    /**
     * Brings everyone back to the lobby after a race, forgetting the players
     * who left during it.
     */
    pub fn return_to_lobby(&mut self, by: PlayerId, now: Instant) -> Result<(), ServerError> {
        self.require_host(by, "return to the lobby")?;
        self.require_finished()?;
        self.stage = Stage::Lobby;
        self.members.retain(|member| member.connected);
        self.members.iter_mut().for_each(Member::reset);
        self.last_activity = now;
        Ok(())
    }

    /**
     * Lets time move the race along: the countdown ends, then the race ends
     * once every connected player finished, nobody is left or the timeout
     * passed. Returns whether the phase changed.
     */
    pub fn advance(&mut self, now: Instant, race_timeout: Duration) -> bool {
        let started = self.start_race_if_due(now);
        let finished = self.finish_race_if_due(now, race_timeout);
        started || finished
    }

    pub(super) fn start_race_if_due(&mut self, now: Instant) -> bool {
        match &self.stage {
            Stage::Countdown(race) if race.countdown_over(now) => {
                self.stage = Stage::Racing(race.clone());
                true
            }
            _ => false,
        }
    }

    fn finish_race_if_due(&mut self, now: Instant, race_timeout: Duration) -> bool {
        match &self.stage {
            Stage::Racing(race)
                if race.elapsed(now) >= race_timeout || self.everyone_finished() =>
            {
                self.stage = Stage::Finished(race.clone());
                true
            }
            _ => false,
        }
    }

    pub(super) fn require_lobby(&self) -> Result<(), ServerError> {
        match self.stage {
            Stage::Lobby => Ok(()),
            _ => Err(ServerError::new(
                ErrorCode::RaceInProgress,
                format!(
                    "room {} is not in the lobby, wait for the next race",
                    self.code
                ),
            )),
        }
    }

    fn require_finished(&self) -> Result<(), ServerError> {
        match self.stage {
            Stage::Finished(_) => Ok(()),
            Stage::Lobby => Err(ServerError::new(
                ErrorCode::RaceNotRunning,
                format!("room {} is already in the lobby", self.code),
            )),
            Stage::Countdown(_) | Stage::Racing(_) => Err(ServerError::new(
                ErrorCode::RaceInProgress,
                format!("the race in room {} is not over yet", self.code),
            )),
        }
    }
}
