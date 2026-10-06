//! A room and its races as a pure state machine.
//!
//! Nothing here performs I/O or reads the clock: every operation receives the
//! current [`Instant`], which keeps the rules deterministic and testable. The
//! hub owns the rooms and turns their state into messages.

use std::time::{Duration, Instant};

use code_racer_engine::{Indentation, TextSource, grapheme_count};
use code_racer_protocol::{
    ErrorCode, Phase, PlayerId, PlayerProgress, PlayerView, Progress, RoomCode, RoomView,
    ServerError, Username,
};

/// Fastest typing the server believes, in characters per second (about 360 WPM).
const MAX_CHARS_PER_SECOND: f64 = 30.0;
/// Characters a report may run ahead of that pace, for short bursts of fast
/// typing. No allowance is needed for network delays: the server times a race
/// from its own start, which comes before any client can type.
const SPEED_BURST_CHARS: f64 = 5.0;

#[derive(Debug, Clone)]
struct Race {
    length: u32,
    /// What auto-indentation fills in, to check the indentation players report.
    indentation: Indentation,
    announced_at: Instant,
    countdown: Duration,
}

impl Race {
    fn countdown_over(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.announced_at) >= self.countdown
    }

    /// Time since the end of the countdown, as measured by the server.
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
    /// Counters of the last accepted report, which later reports may not undo.
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

/// A group of players racing on the same texts, one race at a time.
#[derive(Debug)]
pub struct Room {
    code: RoomCode,
    host: PlayerId,
    text: TextSource,
    max_players: u8,
    stage: Stage,
    /// In joining order, which decides who becomes host when the host leaves.
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

    /// Adds a player to the lobby. Joining a room one is already in changes nothing.
    pub fn join(&mut self, id: PlayerId, name: Username, now: Instant) -> Result<(), ServerError> {
        if self.position(id).is_some() {
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

    /// Removes a player, except during a race where they stay listed as
    /// disconnected so that the standings remain complete.
    pub fn leave(&mut self, id: PlayerId, now: Instant) {
        let Some(index) = self.position(id) else {
            return;
        };
        match self.stage {
            Stage::Lobby | Stage::Finished(_) => {
                self.members.remove(index);
            }
            Stage::Countdown(_) | Stage::Racing(_) => self.members[index].connected = false,
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

    /// Announces a race on `text`, which starts once `countdown` has elapsed.
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

    /// Records a player's typing counters and derives their speed, accuracy
    /// and finishing time from the server clock.
    ///
    /// Reports of players who already finished are ignored. Implausible
    /// reports are rejected and leave the room unchanged.
    pub fn report_progress(
        &mut self,
        by: PlayerId,
        counters: Progress,
        now: Instant,
    ) -> Result<(), ServerError> {
        let index = self.member_index(by)?;
        self.start_race_if_due(now);
        let Stage::Racing(race) = &self.stage else {
            return Err(ServerError::new(
                ErrorCode::RaceNotRunning,
                format!("room {} is not racing", self.code),
            ));
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

    /// Brings everyone back to the lobby after a race, forgetting the players
    /// who left during it.
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

    /// Lets time move the race along: the countdown ends, then the race ends
    /// once every connected player finished, nobody is left or the timeout
    /// passed. Returns whether the phase changed.
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

/// Checks a report against the previous one and the race text. Auto-filled
/// indentation needs no keystroke, so it is exempt from the keystroke and
/// speed checks but must match indentation the text actually has.
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
mod tests {
    use code_racer_engine::{CodeLanguage, Language, SessionOptions, Tally, TypingSession};
    use code_racer_protocol::{MAX_MESSAGE_BYTES, MAX_ROOM_PLAYERS, ServerMessage};

    use super::*;

    const ALICE: PlayerId = PlayerId(1);
    const BOB: PlayerId = PlayerId(2);
    const CAROL: PlayerId = PlayerId(3);
    const STRANGER: PlayerId = PlayerId(99);
    const COUNTDOWN: Duration = Duration::from_secs(3);
    const TIMEOUT: Duration = Duration::from_secs(300);
    const TEXT_LENGTH: u32 = 100;

    struct Clock(Instant);

    impl Clock {
        fn at(&self, millis: u64) -> Instant {
            self.0 + Duration::from_millis(millis)
        }

        /// Instant `millis` after the end of the countdown started at `at(0)`.
        fn racing(&self, millis: u64) -> Instant {
            self.at(millis) + COUNTDOWN
        }
    }

    fn name(id: PlayerId) -> Username {
        match id {
            ALICE => "Alice",
            BOB => "Bob",
            CAROL => "Carol",
            _ => "Stranger",
        }
        .parse()
        .expect("valid name")
    }

    fn lobby(clock: &Clock, players: &[PlayerId], max_players: u8) -> Room {
        let code = "ABC234".parse().expect("valid code");
        let text = TextSource::Quote {
            language: Language::English,
        };
        let mut room = Room::new(code, ALICE, name(ALICE), text, max_players, clock.at(0));
        for &id in players {
            room.join(id, name(id), clock.at(0)).expect("join");
        }
        room
    }

    fn counting_down(clock: &Clock, players: &[PlayerId]) -> Room {
        counting_down_on(clock, players, &"a".repeat(TEXT_LENGTH as usize))
    }

    fn counting_down_on(clock: &Clock, players: &[PlayerId], text: &str) -> Room {
        let mut room = lobby(clock, players, 8);
        for id in std::iter::once(ALICE).chain(players.iter().copied()) {
            room.set_ready(id, true, clock.at(0)).expect("ready");
        }
        room.start_countdown(ALICE, text, clock.at(0), COUNTDOWN)
            .expect("start");
        room
    }

    fn racing(clock: &Clock, players: &[PlayerId]) -> Room {
        let mut room = counting_down(clock, players);
        assert!(room.advance(clock.racing(0), TIMEOUT));
        room
    }

    fn racing_on(clock: &Clock, text: &str) -> Room {
        let mut room = counting_down_on(clock, &[], text);
        assert!(room.advance(clock.racing(0), TIMEOUT));
        room
    }

    fn progress(typed: u32, correct: u32, keystrokes: u32, errors: u32) -> Progress {
        Progress {
            typed,
            correct,
            indentation: 0,
            keystrokes,
            errors,
        }
    }

    fn indented(typed: u32, correct: u32, indentation: u32, keystrokes: u32) -> Progress {
        Progress {
            indentation,
            ..progress(typed, correct, keystrokes, 0)
        }
    }

    fn clean(characters: u32) -> Progress {
        progress(characters, characters, characters, 0)
    }

    fn player(room: &Room, id: PlayerId) -> PlayerView {
        room.view().player(id).cloned().expect("member")
    }

    fn error_code<T>(result: Result<T, ServerError>) -> Option<ErrorCode> {
        result.err().map(|error| error.code)
    }

    #[test]
    fn a_new_room_is_a_lobby_hosted_by_its_creator() {
        let clock = Clock(Instant::now());
        let view = lobby(&clock, &[], 8).view();
        assert_eq!(view.host, ALICE);
        assert_eq!(view.phase, Phase::Lobby);
        assert_eq!(view.text_length, 0);
        assert_eq!(view.players.len(), 1);
        assert!(!view.players[0].ready);
        assert!(view.players[0].connected);
    }

    #[test]
    fn players_join_in_order_until_the_room_is_full() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 2);
        let refused = room.join(CAROL, name(CAROL), clock.at(1));
        assert_eq!(
            refused.map_err(|error| (error.code, error.message)),
            Err((ErrorCode::RoomFull, "room ABC234 is full".to_owned()))
        );
        let ids: Vec<PlayerId> = room.view().players.iter().map(|player| player.id).collect();
        assert_eq!(ids, [ALICE, BOB]);
    }

    #[test]
    fn joining_twice_keeps_a_single_entry() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 8);
        assert!(room.join(BOB, name(BOB), clock.at(1)).is_ok());
        assert_eq!(room.view().players.len(), 2);
    }

    #[test]
    fn nobody_joins_once_a_race_is_announced_or_until_back_in_the_lobby() {
        let clock = Clock(Instant::now());
        let mut room = counting_down(&clock, &[BOB]);
        assert_eq!(
            error_code(room.join(CAROL, name(CAROL), clock.at(1))),
            Some(ErrorCode::RaceInProgress)
        );
        room.advance(clock.racing(0), TIMEOUT);
        room.leave(BOB, clock.racing(1));
        room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(10_000))
            .expect("finish");
        assert!(room.advance(clock.racing(10_000), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Finished);
        assert_eq!(
            error_code(room.join(CAROL, name(CAROL), clock.racing(10_001))),
            Some(ErrorCode::RaceInProgress)
        );
    }

    #[test]
    fn readiness_only_changes_in_the_lobby() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 8);
        room.set_ready(BOB, true, clock.at(1)).expect("ready");
        assert!(player(&room, BOB).ready);
        room.set_ready(BOB, false, clock.at(2)).expect("not ready");
        assert!(!player(&room, BOB).ready);
        assert_eq!(
            error_code(room.set_ready(STRANGER, true, clock.at(3))),
            Some(ErrorCode::NotInRoom)
        );

        let mut room = counting_down(&clock, &[BOB]);
        assert_eq!(
            error_code(room.set_ready(BOB, false, clock.at(1))),
            Some(ErrorCode::RaceInProgress)
        );
        assert!(player(&room, BOB).ready);
    }

    #[test]
    fn only_the_host_starts_and_only_when_everyone_is_ready() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB, CAROL], 8);
        room.set_ready(ALICE, true, clock.at(0)).expect("ready");
        room.set_ready(BOB, true, clock.at(0)).expect("ready");

        assert_eq!(
            error_code(room.start_countdown(BOB, "text", clock.at(1), COUNTDOWN)),
            Some(ErrorCode::NotHost)
        );
        let not_ready = room.start_countdown(ALICE, "text", clock.at(1), COUNTDOWN);
        assert_eq!(
            not_ready.map_err(|error| (error.code, error.message)),
            Err((
                ErrorCode::PlayersNotReady,
                "waiting for Carol to get ready".to_owned()
            ))
        );
        assert_eq!(room.view().phase, Phase::Lobby);

        room.set_ready(CAROL, true, clock.at(2)).expect("ready");
        room.start_countdown(ALICE, "héllo 👋", clock.at(2), COUNTDOWN)
            .expect("start");
        let view = room.view();
        assert_eq!(view.phase, Phase::Countdown);
        assert_eq!(view.text_length, 7);
        assert_eq!(
            error_code(room.start_countdown(ALICE, "again", clock.at(3), COUNTDOWN)),
            Some(ErrorCode::RaceInProgress)
        );
    }

    /// The valid name with the longest JSON: as many bytes as allowed, and a
    /// quote, which JSON escapes, at the start of each of its characters.
    fn widest_name() -> Username {
        let marks = (Username::MAX_BYTES - Username::MAX_LENGTH) / '\u{301}'.len_utf8();
        let quotes = "\"".repeat(Username::MAX_LENGTH - 1);
        format!("\"{}{quotes}", "\u{301}".repeat(marks))
            .parse()
            .expect("the widest valid name")
    }

    #[test]
    fn a_full_room_of_the_longest_names_still_fits_in_every_message() {
        let clock = Clock(Instant::now());
        let code = "ABC234".parse().expect("valid code");
        let text = TextSource::Quote {
            language: Language::English,
        };
        let host = PlayerId(0);
        let mut room = Room::new(
            code,
            host,
            widest_name(),
            text,
            MAX_ROOM_PLAYERS,
            clock.at(0),
        );
        for id in 1..u64::from(MAX_ROOM_PLAYERS) {
            room.join(PlayerId(id), widest_name(), clock.at(0))
                .expect("room for everyone");
        }
        let waiting = room
            .start_countdown(host, "text", clock.at(1), COUNTDOWN)
            .expect_err("nobody is ready");
        assert_eq!(waiting.code, ErrorCode::PlayersNotReady);
        for message in [
            ServerMessage::Error(waiting),
            ServerMessage::Room(room.view()),
        ] {
            assert!(message.to_json().len() <= MAX_MESSAGE_BYTES);
        }
    }

    #[test]
    fn the_countdown_turns_into_a_race_at_its_deadline() {
        let clock = Clock(Instant::now());
        let mut room = counting_down(&clock, &[BOB]);
        assert!(!room.advance(clock.at(2_999), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Countdown);
        assert!(room.advance(clock.at(3_000), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Racing);
        assert!(!room.advance(clock.at(3_050), TIMEOUT));
    }

    #[test]
    fn progress_is_refused_outside_a_race() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 8);
        assert_eq!(
            error_code(room.report_progress(BOB, clean(1), clock.at(1))),
            Some(ErrorCode::RaceNotRunning)
        );

        let mut room = counting_down(&clock, &[BOB]);
        assert_eq!(
            error_code(room.report_progress(BOB, clean(1), clock.at(2_999))),
            Some(ErrorCode::RaceNotRunning)
        );
        assert_eq!(
            error_code(room.report_progress(STRANGER, clean(1), clock.racing(1))),
            Some(ErrorCode::NotInRoom)
        );
    }

    #[test]
    fn progress_after_the_deadline_starts_the_race_without_waiting_for_a_tick() {
        let clock = Clock(Instant::now());
        let mut room = counting_down(&clock, &[BOB]);
        room.report_progress(BOB, clean(5), clock.racing(500))
            .expect("progress");
        assert_eq!(room.view().phase, Phase::Racing);
        assert_eq!(player(&room, BOB).progress.correct, 5);
    }

    #[test]
    fn inconsistent_counters_are_rejected_without_changing_anything() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        let accepted = progress(20, 18, 25, 4);
        room.report_progress(BOB, accepted, clock.racing(10_000))
            .expect("valid progress");
        let before = player(&room, BOB);

        let invalid = [
            progress(TEXT_LENGTH + 1, 20, 200, 4),
            progress(20, 21, 30, 4),
            progress(20, 18, 30, 31),
            progress(31, 18, 30, 4),
            progress(20, 18, 24, 4),
            progress(20, 18, 30, 3),
        ];
        for counters in invalid {
            let result = room.report_progress(BOB, counters, clock.racing(11_000));
            assert_eq!(
                error_code(result),
                Some(ErrorCode::InvalidProgress),
                "{counters:?}"
            );
            assert_eq!(player(&room, BOB), before, "{counters:?}");
        }
    }

    #[test]
    fn typing_faster_than_thirty_characters_per_second_is_implausible() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        assert_eq!(
            error_code(room.report_progress(BOB, clean(6), clock.racing(0))),
            Some(ErrorCode::InvalidProgress)
        );
        room.report_progress(BOB, clean(5), clock.racing(0))
            .expect("a short burst");
        assert_eq!(
            error_code(room.report_progress(BOB, clean(36), clock.racing(1_000))),
            Some(ErrorCode::InvalidProgress)
        );
        room.report_progress(BOB, clean(35), clock.racing(1_000))
            .expect("thirty characters per second and a burst");
        assert_eq!(player(&room, BOB).progress.correct, 35);
    }

    #[test]
    fn a_short_race_cannot_be_finished_instantly() {
        let clock = Clock(Instant::now());
        let text = "a".repeat(30);
        let mut room = racing_on(&clock, &text);
        assert_eq!(
            error_code(room.report_progress(ALICE, clean(30), clock.racing(10))),
            Some(ErrorCode::InvalidProgress)
        );
        room.report_progress(ALICE, clean(30), clock.racing(1_000))
            .expect("thirty characters in a second");
        assert_eq!(player(&room, ALICE).progress.finish_ms, Some(1_000));
    }

    #[test]
    fn speed_and_accuracy_come_from_the_server_clock() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        room.report_progress(BOB, progress(52, 50, 55, 5), clock.racing(12_000))
            .expect("progress");
        let progress = player(&room, BOB).progress;
        assert_eq!(progress.typed, 52);
        assert_eq!(progress.correct, 50);
        assert_eq!(progress.errors, 5);
        assert!((progress.wpm - 50.0).abs() < 1e-9, "{}", progress.wpm);
        assert!((progress.accuracy - 50.0 / 55.0 * 100.0).abs() < 1e-9);
        assert_eq!(progress.finish_ms, None);
    }

    #[test]
    fn reported_indentation_must_be_indentation_of_the_text() {
        let clock = Clock(Instant::now());
        let mut room = racing_on(&clock, "{\n    x\n}");
        let invalid = [
            indented(6, 6, 5, 2),
            indented(3, 3, 2, 2),
            indented(6, 3, 4, 3),
            indented(6, 6, 4, 1),
        ];
        for counters in invalid {
            assert_eq!(
                error_code(room.report_progress(ALICE, counters, clock.racing(5_000))),
                Some(ErrorCode::InvalidProgress),
                "{counters:?}"
            );
        }
        room.report_progress(ALICE, indented(6, 6, 4, 2), clock.racing(5_000))
            .expect("the newline filled in four spaces");
        assert_eq!(player(&room, ALICE).progress.typed, 6);
    }

    #[test]
    fn auto_filled_indentation_counts_for_finishing_but_not_for_speed_or_accuracy() {
        let clock = Clock(Instant::now());
        let mut room = racing_on(&clock, "{\n    x\n}");
        let finished = Progress {
            errors: 1,
            ..indented(9, 9, 4, 6)
        };
        room.report_progress(ALICE, finished, clock.racing(2_000))
            .expect("finish");
        let progress = player(&room, ALICE).progress;
        assert_eq!(progress.finish_ms, Some(2_000));
        assert_eq!(
            progress.wpm,
            code_racer_engine::words_per_minute(5, Duration::from_secs(2))
        );
        assert_eq!(progress.accuracy, 5.0 / 6.0 * 100.0);
    }

    #[test]
    fn a_code_race_is_scored_like_the_session_that_typed_it() {
        let clock = Clock(Instant::now());
        let snippet = TextSource::Code {
            language: CodeLanguage::Python,
        }
        .generate(3)
        .text;
        let mut room = racing_on(&clock, &snippet);
        let options = SessionOptions {
            auto_indent: true,
            ..SessionOptions::default()
        };
        let mut session = TypingSession::new(&snippet, options);
        session.start(clock.racing(0));
        let mut now = clock.racing(0);
        let mut keys = 0;
        while let Some(expected) = session.target().get(session.cursor()).cloned() {
            if keys == 3 {
                session.type_char('#', now);
                session.backspace(now);
            }
            keys += 1;
            now = clock.racing(keys * 100);
            expected.chars().for_each(|ch| {
                session.type_char(ch, now);
            });
            let tally = session.tally();
            room.report_progress(ALICE, reported(tally), now)
                .expect("an honest report");
        }
        let stats = session.stats(now);
        let progress = player(&room, ALICE).progress;
        assert_eq!(progress.finish_ms, Some(keys * 100));
        assert_eq!(progress.wpm, stats.wpm);
        assert_eq!(progress.accuracy, stats.accuracy);
        assert!(stats.indentation > 0, "the snippet is indented");
        assert!(stats.accuracy < 100.0);
    }

    fn reported(tally: Tally) -> Progress {
        let count = |value: usize| u32::try_from(value).expect("small counter");
        Progress {
            typed: count(tally.typed),
            correct: count(tally.correct),
            indentation: count(tally.indentation),
            keystrokes: count(tally.keystrokes),
            errors: count(tally.errors),
        }
    }

    #[test]
    fn a_player_finishes_when_the_whole_text_is_correct_and_further_reports_are_ignored() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        room.report_progress(BOB, progress(TEXT_LENGTH, 99, 100, 1), clock.racing(15_000))
            .expect("one mistake left");
        assert_eq!(player(&room, BOB).progress.finish_ms, None);

        room.report_progress(
            BOB,
            progress(TEXT_LENGTH, TEXT_LENGTH, 102, 1),
            clock.racing(20_250),
        )
        .expect("finish");
        let finished = player(&room, BOB);
        assert_eq!(finished.progress.finish_ms, Some(20_250));

        room.report_progress(BOB, progress(0, 0, 0, 0), clock.racing(21_000))
            .expect("ignored");
        assert_eq!(player(&room, BOB), finished);
    }

    #[test]
    fn the_race_ends_when_every_connected_player_finished_and_ranks_them_by_time() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        room.report_progress(BOB, clean(TEXT_LENGTH), clock.racing(18_000))
            .expect("bob finishes");
        assert!(!room.advance(clock.racing(18_000), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Racing);

        room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(19_000))
            .expect("alice finishes");
        assert!(room.advance(clock.racing(19_000), TIMEOUT));
        let view = room.view();
        assert_eq!(view.phase, Phase::Finished);
        let standings: Vec<PlayerId> = view.standings().iter().map(|player| player.id).collect();
        assert_eq!(standings, [BOB, ALICE]);
    }

    #[test]
    fn the_race_ends_at_the_timeout_even_if_nobody_finished() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        room.report_progress(BOB, clean(40), clock.racing(10_000))
            .expect("progress");
        assert!(!room.advance(clock.racing(299_999), TIMEOUT));
        assert!(room.advance(clock.racing(300_000), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Finished);
        assert_eq!(
            error_code(room.report_progress(BOB, clean(50), clock.racing(300_001))),
            Some(ErrorCode::RaceNotRunning)
        );
    }

    #[test]
    fn leaving_the_lobby_removes_the_player() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 8);
        room.leave(BOB, clock.at(1));
        assert_eq!(room.view().player(BOB), None);
        room.leave(STRANGER, clock.at(2));
        assert_eq!(room.view().players.len(), 1);
    }

    #[test]
    fn leaving_during_a_race_keeps_the_player_listed_as_disconnected() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB, CAROL]);
        room.report_progress(BOB, clean(30), clock.racing(5_000))
            .expect("progress");
        room.leave(BOB, clock.racing(6_000));
        let bob = player(&room, BOB);
        assert!(!bob.connected);
        assert_eq!(bob.progress.correct, 30);
        assert!(room.has_connected_members());
        assert_eq!(room.connected_members().collect::<Vec<_>>(), [ALICE, CAROL]);
    }

    #[test]
    fn leaving_during_the_countdown_hands_over_the_host_and_the_race_still_starts() {
        let clock = Clock(Instant::now());
        let mut room = counting_down(&clock, &[BOB]);
        room.leave(ALICE, clock.at(1_000));
        let view = room.view();
        assert_eq!(view.host, BOB);
        assert_eq!(
            view.player(ALICE).map(|player| player.connected),
            Some(false)
        );
        assert!(room.advance(clock.racing(0), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Racing);
    }

    #[test]
    fn a_host_leaving_the_results_hands_over_to_a_connected_member_not_an_offline_one() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB, CAROL]);
        room.leave(BOB, clock.racing(1_000));
        for id in [ALICE, CAROL] {
            room.report_progress(id, clean(TEXT_LENGTH), clock.racing(10_000))
                .expect("finish");
        }
        assert!(room.advance(clock.racing(10_000), TIMEOUT));

        room.leave(ALICE, clock.racing(11_000));
        let view = room.view();
        assert_eq!(view.phase, Phase::Finished);
        assert_eq!(view.host, CAROL);
        assert_eq!(view.player(ALICE), None);
        assert_eq!(view.player(BOB).map(|player| player.connected), Some(false));
    }

    #[test]
    fn the_race_ends_when_the_last_unfinished_player_leaves() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(20_000))
            .expect("alice finishes");
        room.leave(BOB, clock.racing(21_000));
        assert!(room.advance(clock.racing(21_000), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Finished);
    }

    #[test]
    fn the_race_ends_when_nobody_is_connected() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB]);
        room.leave(ALICE, clock.racing(1_000));
        room.leave(BOB, clock.racing(1_000));
        assert!(!room.has_connected_members());
        assert!(room.advance(clock.racing(1_000), TIMEOUT));
        assert_eq!(room.view().phase, Phase::Finished);
    }

    #[test]
    fn a_departing_host_hands_over_to_the_first_connected_member() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[BOB, CAROL]);
        room.leave(BOB, clock.racing(1_000));
        room.leave(ALICE, clock.racing(2_000));
        assert_eq!(room.view().host, CAROL);

        let mut room = lobby(&clock, &[BOB, CAROL], 8);
        room.leave(ALICE, clock.at(1));
        assert_eq!(room.view().host, BOB);
    }

    #[test]
    fn only_the_host_returns_a_finished_race_to_the_lobby() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 8);
        assert_eq!(
            error_code(room.return_to_lobby(ALICE, clock.at(1))),
            Some(ErrorCode::RaceNotRunning)
        );

        let mut room = racing(&clock, &[BOB, CAROL]);
        assert_eq!(
            error_code(room.return_to_lobby(ALICE, clock.racing(1))),
            Some(ErrorCode::RaceInProgress)
        );
        room.leave(CAROL, clock.racing(1_000));
        room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(10_000))
            .expect("alice finishes");
        room.report_progress(BOB, clean(TEXT_LENGTH), clock.racing(12_000))
            .expect("bob finishes");
        room.advance(clock.racing(12_000), TIMEOUT);
        assert_eq!(
            error_code(room.return_to_lobby(BOB, clock.racing(13_000))),
            Some(ErrorCode::NotHost)
        );

        room.return_to_lobby(ALICE, clock.racing(13_000))
            .expect("back to the lobby");
        let view = room.view();
        assert_eq!(view.phase, Phase::Lobby);
        assert_eq!(view.text_length, 0);
        assert_eq!(view.players.len(), 2);
        for player in &view.players {
            assert!(!player.ready);
            assert_eq!(player.progress, PlayerProgress::default());
        }
    }

    #[test]
    fn counters_start_over_after_returning_to_the_lobby() {
        let clock = Clock(Instant::now());
        let mut room = racing(&clock, &[]);
        room.report_progress(
            ALICE,
            progress(TEXT_LENGTH, TEXT_LENGTH, 120, 20),
            clock.racing(30_000),
        )
        .expect("finish");
        room.advance(clock.racing(30_000), TIMEOUT);
        room.return_to_lobby(ALICE, clock.racing(31_000))
            .expect("lobby");
        room.set_ready(ALICE, true, clock.racing(31_000))
            .expect("ready");
        let rematch = clock.racing(32_000);
        room.start_countdown(ALICE, "short text", rematch, COUNTDOWN)
            .expect("rematch");
        room.report_progress(ALICE, clean(3), rematch + COUNTDOWN)
            .expect("fewer keystrokes than the previous race is fine");
    }

    #[test]
    fn idle_time_counts_from_the_last_accepted_action() {
        let clock = Clock(Instant::now());
        let mut room = lobby(&clock, &[BOB], 8);
        room.set_ready(BOB, true, clock.at(1_000)).expect("ready");
        assert_eq!(room.idle_for(clock.at(5_000)), Duration::from_secs(4));
        assert!(
            room.start_countdown(BOB, "text", clock.at(4_000), COUNTDOWN)
                .is_err()
        );
        assert_eq!(room.idle_for(clock.at(5_000)), Duration::from_secs(4));
        assert_eq!(room.idle_for(clock.at(0)), Duration::ZERO);
    }
}
