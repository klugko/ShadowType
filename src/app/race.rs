//! Multiplayer: the race form and the client side of a room.

use std::time::{Duration, Instant};

use code_racer_engine::{CodeLanguage, SessionOptions, Stats, TextSource, TypingSession};
use code_racer_protocol::{
    ClientMessage, ErrorCode, Phase, PlayerId, PlayerView, Progress, RoomCode, RoomView,
    ServerMessage, Username,
};

use crate::{
    app::{
        form::{Row, Step, Value},
        practice::{Plan, code_file_name},
        text_settings::{self, TextSetting},
    },
    config::{Mode, Practice},
    history::Record,
    network::{Connection, NetworkEvent},
};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Modes a race can use: time mode is solo only.
const RACE_MODES: [Mode; 3] = [Mode::Words, Mode::Quote, Mode::Code];
/// [`Record::mode`] of races.
const RACE_RECORD_MODE: &str = "race";

/// A line of `race.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Room,
    Join,
    Text(TextSetting),
    Create,
}

pub fn fields(settings: &Practice) -> Vec<Field> {
    [Field::Room, Field::Join]
        .into_iter()
        .chain(
            text_settings::settings(settings)
                .into_iter()
                .map(Field::Text),
        )
        .chain([Field::Create])
        .collect()
}

/// TOML table header printed above a field.
pub fn section(field: Field) -> Option<&'static str> {
    match field {
        Field::Room => Some("join"),
        Field::Text(TextSetting::Mode) => Some("create"),
        _ => None,
    }
}

pub fn row(settings: &Practice, room_code: &str, field: Field) -> Row {
    match field {
        Field::Room => Row::new("room", Value::Text(room_code.to_owned()))
            .hint("code shared by the host, Enter to type it"),
        Field::Join => Row::action("join room"),
        Field::Text(setting) => text_settings::row(settings, setting, &RACE_MODES),
        Field::Create => Row::action("create room"),
    }
}

pub fn adjust(settings: &mut Practice, field: Field, step: Step) {
    if let Field::Text(setting) = field {
        text_settings::adjust(settings, setting, step, &RACE_MODES);
    }
}

/// Why the player connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Create(TextSource),
    Join(RoomCode),
}

/// Where the player is in the life of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Connecting,
    Joining,
    Lobby,
    Countdown(Duration),
    Racing,
    Finished,
}

/// What the application should do after a network event.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Nothing,
    Entered(RoomCode),
    /// The countdown of a race began: the player should see its text.
    Starting,
    Failure(String),
    Closed(String),
    /// The player's own race is over, with this record to keep.
    Finished(Record),
}

/// What the player asks of the room from the lobby or the results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomRequest {
    ToggleReady,
    Start,
    /// Back to the lobby for another race, host only.
    Again,
}

/// The race currently typed by the player.
#[derive(Debug)]
pub struct LiveRace {
    pub session: TypingSession,
    pub countdown_ends: Instant,
    recorded: bool,
}

/// Connection to the race server and local view of the room.
#[derive(Debug)]
pub struct RaceClient {
    connection: Connection,
    pub server: String,
    pub player: Option<PlayerId>,
    pub room: Option<RoomView>,
    pub race: Option<LiveRace>,
    intent: Option<Intent>,
    reported: Option<(Instant, Progress)>,
    /// Whether a room request is still unanswered. The server answers each
    /// with the new room view or an error, and only then can the next one
    /// be decided: a held key would otherwise flood the server, and
    /// toggling twice would ask for the same state twice.
    awaiting_answer: bool,
}

impl RaceClient {
    pub fn connect(server: String, username: Username, intent: Intent) -> Self {
        let connection = Connection::open(server.clone(), username);
        Self::over(connection, server, intent)
    }

    pub(super) fn over(connection: Connection, server: String, intent: Intent) -> Self {
        Self {
            connection,
            server,
            player: None,
            room: None,
            race: None,
            intent: Some(intent),
            reported: None,
            awaiting_answer: false,
        }
    }

    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    pub fn stage(&self, now: Instant) -> Stage {
        let Some(room) = &self.room else {
            return if self.player.is_some() {
                Stage::Joining
            } else {
                Stage::Connecting
            };
        };
        match room.phase {
            Phase::Lobby => Stage::Lobby,
            Phase::Countdown => Stage::Countdown(self.countdown_left(now)),
            Phase::Racing => Stage::Racing,
            Phase::Finished => Stage::Finished,
        }
    }

    pub fn me(&self) -> Option<&PlayerView> {
        self.room.as_ref()?.player(self.player?)
    }

    pub fn is_host(&self) -> bool {
        self.player
            .zip(self.room.as_ref())
            .is_some_and(|(player, room)| room.is_host(player))
    }

    pub fn phase(&self) -> Option<Phase> {
        self.room.as_ref().map(|room| room.phase)
    }

    /// Whether the room counts down or races, whoever is still typing.
    pub fn is_live(&self) -> bool {
        matches!(self.phase(), Some(Phase::Countdown | Phase::Racing))
    }

    /// Whether the race text takes what the player types.
    pub fn accepts_typing(&self) -> bool {
        self.phase() == Some(Phase::Racing)
            && self
                .race
                .as_ref()
                .is_some_and(|race| !race.session.is_finished())
    }

    /// Whether the player is in a race they have not finished: the keyboard
    /// belongs to the race text, already during the countdown so that early
    /// keys are not taken as commands.
    pub fn is_player_racing(&self) -> bool {
        self.phase() == Some(Phase::Countdown) || self.accepts_typing()
    }

    pub fn session_mut(&mut self) -> Option<&mut TypingSession> {
        self.race.as_mut().map(|race| &mut race.session)
    }

    /// File name shown in the editor for the race text.
    pub fn title(&self) -> String {
        match self.room.as_ref().map(|room| room.text) {
            Some(TextSource::Code { language }) => code_file_name(language),
            Some(_) => format!("{}.md", self.room_label()),
            None => "race.md".to_owned(),
        }
    }

    pub fn room_label(&self) -> String {
        self.room
            .as_ref()
            .map_or_else(|| "race".to_owned(), |room| room.code.to_string())
    }

    pub fn syntax(&self) -> Option<CodeLanguage> {
        match self.room.as_ref()?.text {
            TextSource::Code { language } => Some(language),
            _ => None,
        }
    }

    pub fn text_label(&self) -> String {
        self.room
            .as_ref()
            .map(|room| Plan::Text(room.text).label())
            .unwrap_or_default()
    }

    pub fn handle(&mut self, event: NetworkEvent, now: Instant) -> Outcome {
        match event {
            NetworkEvent::Connected(player) => {
                self.player = Some(player);
                self.send_intent();
                Outcome::Nothing
            }
            NetworkEvent::Message(message) => self.handle_message(message, now),
            NetworkEvent::Closed { reason } => Outcome::Closed(reason),
        }
    }

    /// Sends the player's progress when [`progress_due`] says so.
    pub fn report_progress(&mut self, now: Instant) {
        let Some(race) = &self.race else {
            return;
        };
        if self.phase() != Some(Phase::Racing) {
            return;
        }
        let progress = progress(race.session.cursor(), &race.session.stats(now));
        let due = progress_due(self.reported, progress, race.session.is_finished(), now);
        if due && self.connection.send(ClientMessage::Progress(progress)) {
            self.reported = Some((now, progress));
        }
    }

    /// Sends `request`, unless the previous one is still unanswered. Returns
    /// why it cannot be made, if it cannot.
    pub fn request(&mut self, request: RoomRequest) -> Result<(), String> {
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

    pub fn leave(&self) {
        self.connection.send(ClientMessage::LeaveRoom);
    }

    fn countdown_left(&self, now: Instant) -> Duration {
        self.race.as_ref().map_or(Duration::ZERO, |race| {
            race.countdown_ends.saturating_duration_since(now)
        })
    }

    fn send_intent(&mut self) {
        let message = match self.intent.take() {
            Some(Intent::Create(text)) => ClientMessage::CreateRoom { text },
            Some(Intent::Join(code)) => ClientMessage::JoinRoom { code },
            None => return,
        };
        self.connection.send(message);
    }

    fn handle_message(&mut self, message: ServerMessage, now: Instant) -> Outcome {
        if matches!(message, ServerMessage::Room(_) | ServerMessage::Error(_)) {
            self.awaiting_answer = false;
        }
        match message {
            ServerMessage::Room(room) => self.apply_room(room, now),
            ServerMessage::Countdown { text, duration_ms } => {
                self.begin_countdown(&text, duration_ms, now);
                Outcome::Starting
            }
            ServerMessage::Error(error) if self.room.is_none() => Outcome::Closed(error.message),
            ServerMessage::Error(error) if error.code == ErrorCode::RoomNotFound => {
                Outcome::Closed(error.message)
            }
            ServerMessage::Error(error) => Outcome::Failure(error.message),
            ServerMessage::Welcome { .. } => Outcome::Nothing,
        }
    }

    fn begin_countdown(&mut self, text: &str, duration_ms: u32, now: Instant) {
        let options = SessionOptions {
            auto_indent: self.syntax().is_some(),
            ..SessionOptions::default()
        };
        self.race = Some(LiveRace {
            session: TypingSession::new(text, options),
            countdown_ends: now + Duration::from_millis(duration_ms.into()),
            recorded: false,
        });
        self.reported = None;
    }

    fn apply_room(&mut self, room: RoomView, now: Instant) -> Outcome {
        let entered = (self.room.is_none()).then(|| room.code.clone());
        let phase = room.phase;
        self.room = Some(room);
        match phase {
            Phase::Lobby => self.race = None,
            Phase::Racing => {
                if let Some(race) = &mut self.race {
                    race.session.start(now);
                }
            }
            Phase::Countdown | Phase::Finished => {}
        }
        if let Some(record) = self.conclude(now) {
            return Outcome::Finished(record);
        }
        entered.map_or(Outcome::Nothing, Outcome::Entered)
    }

    /// Whether the player's race is over: the server timed their finish,
    /// or the race ended, finished or not.
    fn is_over_for_me(&self) -> bool {
        self.phase() == Some(Phase::Finished)
            || self.me().is_some_and(|me| me.progress.is_finished())
    }

    /// The record of the race once it is over for the player, and only
    /// once: as soon as the server timed their finish, so that leaving
    /// before the slowest player finishes keeps it. It takes the server's
    /// figures where it has them, as in the standings, and the local ones
    /// otherwise. A race where nothing was typed is not recorded.
    fn conclude(&mut self, now: Instant) -> Option<Record> {
        if !self.is_over_for_me() {
            return None;
        }
        let progress = self.me()?.progress;
        let language = Plan::Text(self.room.as_ref()?.text).language_label();
        let race = self.race.as_mut()?;
        let stats = race.session.stats(now);
        if race.recorded || stats.typed_chars == 0 {
            return None;
        }
        race.recorded = true;
        let duration = progress
            .finish_ms
            .map_or(stats.elapsed.as_secs_f64(), |ms| ms as f64 / 1000.0);
        Some(Record {
            duration,
            wpm: progress.wpm,
            accuracy: progress.accuracy,
            ..Record::from_stats(RACE_RECORD_MODE.to_owned(), language, &stats)
        })
    }
}

/// Whether `progress` should be sent: the first report at once, then only
/// a changed one, at most every [`PROGRESS_INTERVAL`] but at once when the
/// text is complete, so that the server times the finish exactly.
fn progress_due(
    last: Option<(Instant, Progress)>,
    progress: Progress,
    complete: bool,
    now: Instant,
) -> bool {
    last.is_none_or(|(at, sent)| {
        sent != progress && (complete || now.duration_since(at) >= PROGRESS_INTERVAL)
    })
}

fn progress(cursor: usize, stats: &Stats) -> Progress {
    let clamp = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    Progress {
        typed: clamp(cursor),
        correct: clamp(stats.correct_chars),
        indentation: clamp(stats.indentation),
        keystrokes: clamp(stats.typed_chars),
        errors: clamp(stats.errors),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_form_cycles_only_race_modes() {
        let mut settings = Practice::default();
        let mut seen = Vec::new();
        for _ in 0..3 {
            adjust(&mut settings, Field::Text(TextSetting::Mode), Step::Next);
            seen.push(settings.mode);
        }
        assert!(!seen.contains(&Mode::Time));
        assert_eq!(settings.mode, Mode::Words);
        assert_eq!(fields(&settings).len(), 8);
    }

    #[test]
    fn race_words_cycle_through_raceable_presets() {
        let mut settings = Practice::default().for_race();
        let words = Field::Text(TextSetting::Words);
        for _ in 0..Practice::WORD_COUNT_PRESETS.len() {
            adjust(&mut settings, words, Step::Next);
            assert!(code_racer_protocol::is_raceable(
                &settings.race_text_source()
            ));
        }
        assert_eq!(section(Field::Text(TextSetting::Mode)), Some("create"));
    }

    fn typed(count: u32) -> Progress {
        Progress {
            typed: count,
            correct: count,
            keystrokes: count,
            ..Progress::default()
        }
    }

    #[test]
    fn progress_is_sent_at_once_then_throttled() {
        let start = Instant::now();
        let soon = start + PROGRESS_INTERVAL / 2;
        let later = start + PROGRESS_INTERVAL;
        assert!(progress_due(None, typed(0), false, start), "first report");
        let last = Some((start, typed(1)));
        assert!(!progress_due(last, typed(1), false, later), "unchanged");
        assert!(!progress_due(last, typed(2), false, soon), "too soon");
        assert!(progress_due(last, typed(2), false, later));
        assert!(progress_due(last, typed(2), true, soon), "completion");
        assert!(!progress_due(last, typed(1), true, soon), "completion sent");
    }

    fn lobby(ready: bool) -> RoomView {
        let me = code_racer_protocol::PlayerView {
            id: PlayerId(1),
            name: "jean".parse().expect("name"),
            ready,
            connected: true,
            progress: code_racer_protocol::PlayerProgress::default(),
        };
        RoomView {
            code: "FK72AD".parse().expect("code"),
            host: PlayerId(1),
            text: TextSource::Quote {
                language: code_racer_engine::Language::English,
            },
            text_length: 0,
            phase: Phase::Lobby,
            max_players: 8,
            players: vec![me],
        }
    }

    #[test]
    fn one_room_request_at_a_time() {
        let (connection, mut sent) = Connection::loopback();
        let code: RoomCode = "FK72AD".parse().expect("code");
        let mut client = RaceClient::over(connection, "ws://test".to_owned(), Intent::Join(code));
        let now = Instant::now();
        client.handle(NetworkEvent::Connected(PlayerId(1)), now);
        client.handle(
            NetworkEvent::Message(ServerMessage::Room(lobby(false))),
            now,
        );
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMessage::JoinRoom { .. })
        ));

        for _ in 0..3 {
            client.request(RoomRequest::ToggleReady).expect("allowed");
        }
        assert_eq!(sent.try_recv(), Ok(ClientMessage::SetReady { ready: true }));
        assert!(sent.try_recv().is_err(), "held until the server answers");

        client.handle(NetworkEvent::Message(ServerMessage::Room(lobby(true))), now);
        client.request(RoomRequest::ToggleReady).expect("allowed");
        assert_eq!(
            sent.try_recv(),
            Ok(ClientMessage::SetReady { ready: false })
        );

        let refusal = ServerMessage::error(ErrorCode::InvalidMessage, "no");
        client.handle(NetworkEvent::Message(refusal), now);
        client
            .request(RoomRequest::Start)
            .expect("everyone is ready");
        assert_eq!(
            sent.try_recv(),
            Ok(ClientMessage::StartRace),
            "an error answers too"
        );
    }

    #[test]
    fn progress_converts_session_counters() {
        let stats = Stats {
            correct_chars: 9,
            typed_chars: 7,
            indentation: 4,
            errors: 3,
            ..Stats::default()
        };
        assert_eq!(
            progress(10, &stats),
            Progress {
                typed: 10,
                correct: 9,
                indentation: 4,
                keystrokes: 7,
                errors: 3
            }
        );
    }
}
