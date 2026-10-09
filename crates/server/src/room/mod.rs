/*!
 * A room and its races as a pure state machine.
 *
 * Nothing here performs I/O or reads the clock: every operation receives the
 * current [`Instant`], which keeps the rules deterministic and testable. The
 * hub owns the rooms and turns their state into messages.
 */

use std::time::{Duration, Instant};

use code_racer_engine::{Indentation, TextSource, grapheme_count};
use code_racer_protocol::{
    ErrorCode, Phase, PlayerId, PlayerProgress, PlayerView, Progress, RoomCode, RoomView,
    ServerError, Username,
};

const MAX_CHARS_PER_SECOND: f64 = 30.0;
const SPEED_BURST_CHARS: f64 = 5.0;

#[derive(Debug, Clone)]
struct Race {
    length: u32,
    indentation: Indentation,
    announced_at: Instant,
    countdown: Duration,
}

impl Race {
    fn countdown_over(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.announced_at) >= self.countdown
    }

    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.announced_at)
            .saturating_sub(self.countdown)
    }
}

#[derive(Debug, Clone)]
enum Stage {
    Lobby,
    Countdown(Race),
    Racing(Race),
    Finished(Race),
}

impl Stage {
    fn phase(&self) -> Phase {
        match self {
            Self::Lobby => Phase::Lobby,
            Self::Countdown(_) => Phase::Countdown,
            Self::Racing(_) => Phase::Racing,
            Self::Finished(_) => Phase::Finished,
        }
    }

    fn race(&self) -> Option<&Race> {
        match self {
            Self::Lobby => None,
            Self::Countdown(race) | Self::Racing(race) | Self::Finished(race) => Some(race),
        }
    }
}

#[derive(Debug)]
struct Member {
    id: PlayerId,
    name: Username,
    ready: bool,
    connected: bool,
    progress: PlayerProgress,
    counters: Progress,
}

impl Member {
    fn new(id: PlayerId, name: Username) -> Self {
        Self {
            id,
            name,
            ready: false,
            connected: true,
            progress: PlayerProgress::default(),
            counters: Progress::default(),
        }
    }

    fn reset(&mut self) {
        self.ready = false;
        self.progress = PlayerProgress::default();
        self.counters = Progress::default();
    }

    fn record(&mut self, counters: Progress, race: &Race, now: Instant) {
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

    fn view(&self) -> PlayerView {
        PlayerView {
            id: self.id,
            name: self.name.clone(),
            ready: self.ready,
            connected: self.connected,
            progress: self.progress,
        }
    }
}

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

    pub fn start_countdown(
        &mut self,
        by: PlayerId,
        text: &str,
        now: Instant,
        countdown: Duration,
    ) -> Result<(), ServerError> {
        self.require_host(by, "start the race")?;
        self.require_lobby()?;
        let waiting: Vec<&str> = self
            .members
            .iter()
            .filter(|member| !member.ready)
            .map(|member| member.name.as_str())
            .collect();
        if !waiting.is_empty() {
            return Err(ServerError::new(
                ErrorCode::PlayersNotReady,
                format!("waiting for {} to get ready", waiting.join(", ")),
            ));
        }
        let length = u32::try_from(grapheme_count(text)).unwrap_or(u32::MAX);
        self.stage = Stage::Countdown(Race {
            length,
            indentation: Indentation::of(text),
            announced_at: now,
            countdown,
        });
        self.last_activity = now;
        Ok(())
    }

    /**
     * Records a player's typing counters and derives their speed, accuracy
     * and finishing time from the server clock.
     *
     * Reports of players who already finished are ignored, and so are those
     * arriving after the race ended, which were on their way when it did.
     * Implausible reports are rejected and leave the room unchanged.
     */
    pub fn report_progress(
        &mut self,
        by: PlayerId,
        counters: Progress,
        now: Instant,
    ) -> Result<(), ServerError> {
        let index = self.member_index(by)?;
        self.start_race_if_due(now);
        let race = match &self.stage {
            Stage::Racing(race) => race,
            Stage::Finished(_) => return Ok(()),
            Stage::Lobby | Stage::Countdown(_) => {
                return Err(ServerError::new(
                    ErrorCode::RaceNotRunning,
                    format!("room {} is not racing", self.code),
                ));
            }
        };
        let member = &mut self.members[index];
        if member.progress.is_finished() {
            return Ok(());
        }
        check_progress(member.counters, counters, race, now).map_err(|reason| {
            ServerError::new(
                ErrorCode::InvalidProgress,
                format!("progress rejected: {reason}"),
            )
        })?;
        member.record(counters, race, now);
        self.last_activity = now;
        Ok(())
    }

    /**
     * Brings everyone back to the lobby after a race, forgetting the players
     * who left during it.
     */
    pub fn return_to_lobby(&mut self, by: PlayerId, now: Instant) -> Result<(), ServerError> {
        self.require_host(by, "return to the lobby")?;
        match self.stage {
            Stage::Finished(_) => {}
            Stage::Lobby => {
                return Err(ServerError::new(
                    ErrorCode::RaceNotRunning,
                    format!("room {} is already in the lobby", self.code),
                ));
            }
            Stage::Countdown(_) | Stage::Racing(_) => {
                return Err(ServerError::new(
                    ErrorCode::RaceInProgress,
                    format!("the race in room {} is not over yet", self.code),
                ));
            }
        }
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

    pub fn connected_members(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.members
            .iter()
            .filter(|member| member.connected)
            .map(|member| member.id)
    }

    pub fn has_connected_members(&self) -> bool {
        self.connected_members().next().is_some()
    }

    pub fn idle_for(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.last_activity)
    }

    fn start_race_if_due(&mut self, now: Instant) -> bool {
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

    fn everyone_finished(&self) -> bool {
        self.members
            .iter()
            .filter(|member| member.connected)
            .all(|member| member.progress.is_finished())
    }

    fn promote_next_host(&mut self) {
        let successor = self.connected_members().next();
        self.host = successor.unwrap_or(self.host);
    }

    fn position(&self, id: PlayerId) -> Option<usize> {
        self.members.iter().position(|member| member.id == id)
    }

    fn member_index(&self, id: PlayerId) -> Result<usize, ServerError> {
        self.position(id).ok_or_else(|| {
            ServerError::new(
                ErrorCode::NotInRoom,
                format!("you are not in room {}", self.code),
            )
        })
    }

    fn require_host(&self, id: PlayerId, action: &str) -> Result<(), ServerError> {
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

    fn require_lobby(&self) -> Result<(), ServerError> {
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
}

/**
 * Checks a report against the previous one and the race text. Auto-filled
 * indentation needs no keystroke, so it is exempt from the keystroke and
 * speed checks but must match indentation the text actually has.
 */
fn check_progress(
    previous: Progress,
    next: Progress,
    race: &Race,
    now: Instant,
) -> Result<(), &'static str> {
    let elapsed = race.elapsed(now);
    let speed_limit = elapsed.as_secs_f64() * MAX_CHARS_PER_SECOND + SPEED_BURST_CHARS;
    let counts = next.tally();
    let rules = [
        (next.typed <= race.length, "typed past the end of the text"),
        (
            next.correct <= next.typed,
            "more correct characters than typed ones",
        ),
        (
            next.indentation <= next.correct,
            "more indentation than correct characters",
        ),
        (
            counts.indentation <= race.indentation.within(counts.typed),
            "more indentation than the text has",
        ),
        (
            next.errors <= next.keystrokes,
            "more errors than keystrokes",
        ),
        (
            next.typed <= next.keystrokes.saturating_add(next.indentation),
            "more characters than keystrokes",
        ),
        (
            next.keystrokes >= previous.keystrokes,
            "keystrokes cannot decrease",
        ),
        (next.errors >= previous.errors, "errors cannot decrease"),
        (
            counts.correctly_typed() as f64 <= speed_limit,
            "faster than humanly possible",
        ),
    ];
    match rules.into_iter().find(|(holds, _)| !holds) {
        Some((_, broken)) => Err(broken),
        None => Ok(()),
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
