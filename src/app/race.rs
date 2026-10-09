//! Multiplayer: the race form and the client side of a room.

use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use code_racer_engine::{CodeLanguage, SessionOptions, TextSource, TypingSession};
use code_racer_protocol::{
    ClientMessage, ErrorCode, Phase, PlayerId, PlayerView, Progress, RoomCode, RoomView,
    ServerMessage, Username,
};

use crate::{
    app::{
        Disguise, SessionView, TextField,
        form::{Row, Step, Value},
        ink::Ink,
        practice::{Plan, code_file_name, disguised_name},
        settings,
        text_event::TextEvent,
        text_settings::{self, TextSetting},
    },
    cli::Launch,
    config::{Config, Look, Mode, Practice},
    history::Record,
    network::{self, Connection, NetworkEvent},
};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Shortest time between two presses of the same room request. A held key
/// repeats faster, and terminals without keyboard enhancement report its
/// repeats as new presses, which would trip the server's message limit.
const REQUEST_REPEAT: Duration = Duration::from_millis(150);
/// Modes a race can use: time mode is solo only.
const RACE_MODES: [Mode; 3] = [Mode::Words, Mode::Quote, Mode::Code];
/// Stands for the host of the invite when the server runs on this computer
/// and its address on the local network is unknown.
const LAN_ADDRESS_PLACEHOLDER: &str = "<your LAN address>";
/// What the host must know when the server runs on their computer.
const LOCAL_SERVER_NOTE: &str = "start the server with --host 0.0.0.0 for teammates to reach it";

/// A line of `race.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Room,
    Join,
    Text(TextSetting),
    Create,
    /// The race server address, the same setting as in `config.toml`.
    Server,
}

impl Field {
    /// The text typed to set this line, for the lines that take text.
    pub const fn text_field(self) -> Option<TextField> {
        match self {
            Self::Room => Some(TextField::RoomCode),
            Self::Server => Some(TextField::Server),
            Self::Join | Self::Text(_) | Self::Create => None,
        }
    }
}

/// Lines of the race form. The room line comes first, where the form
/// opens, as joining a room is what most players come for.
pub fn fields(settings: &Practice) -> Vec<Field> {
    [Field::Room, Field::Join]
        .into_iter()
        .chain(
            text_settings::settings(settings)
                .into_iter()
                .map(Field::Text),
        )
        .chain([Field::Create, Field::Server])
        .collect()
}

/// TOML table header printed above a field.
pub fn section(field: Field) -> Option<&'static str> {
    match field {
        Field::Room => Some("join"),
        Field::Text(TextSetting::Mode) => Some("create"),
        Field::Server => Some("multiplayer"),
        _ => None,
    }
}

pub fn row(config: &Config, room_code: &str, field: Field) -> Row {
    match field {
        Field::Room => {
            Row::new("room", Value::Text(room_code.to_owned())).hint("Enter to type the code")
        }
        Field::Join => Row::action("join room"),
        Field::Text(setting) => text_settings::row(&config.race, setting, &RACE_MODES),
        Field::Create => Row::action("create room"),
        Field::Server => settings::row(config, settings::Field::Server),
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

impl From<Intent> for Launch {
    fn from(intent: Intent) -> Self {
        match intent {
            Intent::Create(text) => Self::Create(text),
            Intent::Join(code) => Self::Join(code),
        }
    }
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
struct LiveRace {
    session: TypingSession,
    ink: Ink,
    countdown_ends: Instant,
    /// When the room finished the race, which stops the clock of a text
    /// the player had not finished.
    ended_at: Option<Instant>,
    recorded: bool,
}

/// How teammates join the room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /// What they run, such as
    /// `code-racer join FK72AD --server ws://192.168.1.42:8080`.
    pub command: String,
    /// What the host must check for the command to work, when the server
    /// runs on their computer.
    pub note: Option<&'static str>,
}

/// The server address to give teammates.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedServer {
    address: String,
    note: Option<&'static str>,
}

impl SharedServer {
    /// How teammates reach `server`. An address only this computer reaches,
    /// such as `ws://127.0.0.1:8080`, would send them to their own computer,
    /// so this computer's LAN address from `lan_address`, or a placeholder
    /// for it, takes its place.
    fn of(server: &str, lan_address: impl FnOnce() -> Option<IpAddr>) -> Self {
        if !network::is_local_only(server) {
            return Self {
                address: server.to_owned(),
                note: None,
            };
        }
        let host = match lan_address() {
            Some(IpAddr::V6(address)) => format!("[{address}]"),
            Some(IpAddr::V4(address)) => address.to_string(),
            None => LAN_ADDRESS_PLACEHOLDER.to_owned(),
        };
        Self {
            address: network::with_host(server, &host).unwrap_or_else(|| server.to_owned()),
            note: Some(LOCAL_SERVER_NOTE),
        }
    }
}

/// Connection to the race server and local view of the room.
#[derive(Debug)]
pub struct RaceClient {
    connection: Connection,
    pub server: String,
    shared_server: SharedServer,
    pub player: Option<PlayerId>,
    pub room: Option<RoomView>,
    race: Option<LiveRace>,
    intent: Option<Intent>,
    reported: Option<(Instant, Progress)>,
    /// Whether a room request is still unanswered. The server answers each
    /// with the new room view or an error, and only then can the next one
    /// be decided: a held key would otherwise flood the server, and
    /// toggling twice would ask for the same state twice.
    awaiting_answer: bool,
    last_press: Option<(RoomRequest, Instant)>,
}

impl RaceClient {
    pub fn connect(server: String, username: Username, intent: Intent) -> Self {
        let connection = Connection::open(server.clone(), username);
        Self::over(connection, server, intent)
    }

    pub(super) fn over(connection: Connection, server: String, intent: Intent) -> Self {
        Self {
            connection,
            shared_server: SharedServer::of(&server, network::lan_address),
            server,
            player: None,
            room: None,
            race: None,
            intent: Some(intent),
            reported: None,
            awaiting_answer: false,
            last_press: None,
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

    /// Whether the room shows a race: its countdown, the race or its results.
    pub fn shows_a_race(&self) -> bool {
        self.is_live() || self.phase() == Some(Phase::Finished)
    }

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

    /// The race text, from its countdown to the results.
    pub fn session_view(&self) -> Option<SessionView<'_>> {
        self.race.as_ref().map(|race| SessionView {
            session: &race.session,
            syntax: self.syntax(),
            attribution: None,
            stopped_at: race.ended_at,
            ink: Some(&race.ink),
            disguise: None,
        })
    }

    /// File name shown in the editor for the race text: prose is in a file
    /// named after the room, unless `disguise` makes it another kind of file.
    pub fn title(&self, disguise: Disguise<'_>) -> String {
        match self.room.as_ref().map(|room| room.text) {
            Some(TextSource::Code { language }) => code_file_name(language),
            Some(_) if disguise.look != Look::Notes => disguised_name(disguise),
            Some(_) => format!("{}.md", self.room_label()),
            None => "race.md".to_owned(),
        }
    }

    /// How teammates join the room, once in one.
    pub fn invite(&self) -> Option<Invite> {
        let room = self.room.as_ref()?;
        Some(Invite {
            command: format!(
                "code-racer join {} --server {}",
                room.code, self.shared_server.address
            ),
            note: self.shared_server.note,
        })
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

    /// What the room races on, once in a room.
    pub fn plan(&self) -> Option<Plan> {
        self.room.as_ref().map(|room| Plan::Text(room.text))
    }

    pub fn text_label(&self) -> String {
        self.plan().map(|plan| plan.label()).unwrap_or_default()
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

    /// Hands `event` to the race text, which takes keys only while the race
    /// is on and the player's text unfinished, then reports the progress.
    /// Returns whether the text took the key.
    pub fn text_event(&mut self, event: TextEvent, now: Instant) -> bool {
        let open = event == TextEvent::Tick || self.accepts_typing();
        let taken = match &mut self.race {
            Some(race) if open => race.ink.apply(event, &mut race.session, now),
            _ => false,
        };
        self.report_progress(now);
        taken
    }

    /// Sends the player's progress when [`progress_due`] says so.
    fn report_progress(&mut self, now: Instant) {
        let Some(race) = &self.race else {
            return;
        };
        if self.phase() != Some(Phase::Racing) {
            return;
        }
        let progress = Progress::from(race.session.tally());
        let due = progress_due(self.reported, progress, race.session.is_finished(), now);
        if due && self.connection.send(ClientMessage::Progress(progress)) {
            self.reported = Some((now, progress));
        }
    }

    /// Sends `request` pressed at `now`, unless the previous one is still
    /// unanswered or the key is only repeating. Returns why it cannot be
    /// made, if it cannot.
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

    /// Records a press of `request` and tells whether it repeats the previous
    /// press within [`REQUEST_REPEAT`]. Every press restarts the delay, so a
    /// key held down acts once.
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

    /// Leaves the room and hands back the connection, which still has the
    /// goodbye to send.
    pub fn leave(self) -> Connection {
        self.connection.send(ClientMessage::LeaveRoom);
        self.connection
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
            ink: Ink::default(),
            countdown_ends: now + Duration::from_millis(duration_ms.into()),
            ended_at: None,
            recorded: false,
        });
        self.reported = None;
    }

    fn apply_room(&mut self, room: RoomView, now: Instant) -> Outcome {
        let entered = (self.room.is_none()).then(|| room.code.clone());
        let phase = room.phase;
        self.room = Some(room);
        match (phase, &mut self.race) {
            (Phase::Lobby, _) => self.race = None,
            (Phase::Racing, Some(race)) => race.session.start(now),
            (Phase::Finished, Some(race)) => {
                race.ended_at.get_or_insert(now);
            }
            _ => {}
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
        let stats = self.session_view()?.stats(now);
        let race = self.race.as_mut()?;
        if race.recorded || stats.keystrokes == 0 {
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
            ..Record::from_stats(Record::RACE_MODE.to_owned(), language, &stats)
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

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

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
        assert_eq!(fields(&settings).len(), 9);
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

    /// A client in the lobby over a connection that hands what it sends to
    /// the test, the join request already taken.
    fn in_lobby() -> (RaceClient, mpsc::Receiver<ClientMessage>, Instant) {
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
        (client, sent, now)
    }

    #[test]
    fn the_invite_gives_teammates_an_address_they_can_reach() {
        let lan: IpAddr = [192, 168, 1, 42].into();
        let shared = SharedServer::of("ws://127.0.0.1:8080", || Some(lan));
        assert_eq!(shared.address, "ws://192.168.1.42:8080");
        assert_eq!(shared.note, Some(LOCAL_SERVER_NOTE));

        let unknown = SharedServer::of("ws://localhost:9000", || None);
        assert_eq!(
            unknown.address,
            format!("ws://{LAN_ADDRESS_PLACEHOLDER}:9000")
        );
        assert_eq!(unknown.note, Some(LOCAL_SERVER_NOTE));

        let probed = std::cell::Cell::new(false);
        let remote = SharedServer::of("ws://10.0.0.9:8080", || {
            probed.set(true);
            None
        });
        assert_eq!(remote.address, "ws://10.0.0.9:8080");
        assert_eq!(remote.note, None);
        assert!(!probed.get(), "no need to look for this computer's address");
    }

    #[test]
    fn the_invite_names_the_room() {
        let (client, _sent, _) = in_lobby();
        assert_eq!(
            client.invite(),
            Some(Invite {
                command: "code-racer join FK72AD --server ws://test".to_owned(),
                note: None,
            })
        );
    }

    #[test]
    fn the_race_text_takes_keys_only_once_the_race_is_on() {
        let (mut client, mut sent, now) = in_lobby();
        let countdown = ServerMessage::Countdown {
            text: "go".to_owned(),
            duration_ms: 3_000,
        };
        client.handle(NetworkEvent::Message(countdown), now);
        let mut room = lobby(true);
        room.phase = Phase::Countdown;
        client.handle(
            NetworkEvent::Message(ServerMessage::Room(room.clone())),
            now,
        );
        for event in [
            TextEvent::Typed('g'),
            TextEvent::Backspace,
            TextEvent::DeleteWord,
        ] {
            assert!(!client.text_event(event, now), "{event:?}");
        }
        assert_eq!(
            client.session_view().map(|view| view.session.cursor()),
            Some(0)
        );
        assert!(sent.try_recv().is_err(), "nothing to report yet");

        room.phase = Phase::Racing;
        client.handle(NetworkEvent::Message(ServerMessage::Room(room)), now);
        assert!(client.text_event(TextEvent::Typed('g'), now));
        assert!(matches!(sent.try_recv(), Ok(ClientMessage::Progress(_))));
    }

    #[test]
    fn one_room_request_at_a_time() {
        let (mut client, mut sent, now) = in_lobby();
        for press in 0..3 {
            let at = now + REQUEST_REPEAT * press;
            client
                .request(RoomRequest::ToggleReady, at)
                .expect("allowed");
        }
        assert_eq!(sent.try_recv(), Ok(ClientMessage::SetReady { ready: true }));
        assert!(sent.try_recv().is_err(), "held until the server answers");

        let later = now + REQUEST_REPEAT * 4;
        client.handle(
            NetworkEvent::Message(ServerMessage::Room(lobby(true))),
            later,
        );
        client
            .request(RoomRequest::ToggleReady, later)
            .expect("allowed");
        assert_eq!(
            sent.try_recv(),
            Ok(ClientMessage::SetReady { ready: false })
        );

        let refusal = ServerMessage::error(ErrorCode::InvalidMessage, "no");
        client.handle(NetworkEvent::Message(refusal), later);
        client
            .request(RoomRequest::Start, later)
            .expect("everyone is ready");
        assert_eq!(
            sent.try_recv(),
            Ok(ClientMessage::StartRace),
            "an error answers too"
        );
    }

    #[test]
    fn a_request_pressed_again_and_again_is_a_held_key() {
        let (mut client, mut sent, now) = in_lobby();
        let mut at = now;
        for _ in 0..30 {
            client
                .request(RoomRequest::ToggleReady, at)
                .expect("allowed");
            client.handle(NetworkEvent::Message(ServerMessage::Room(lobby(true))), at);
            at += Duration::from_millis(30);
        }
        assert_eq!(sent.try_recv(), Ok(ClientMessage::SetReady { ready: true }));
        assert!(sent.try_recv().is_err(), "one request for the whole hold");

        client.request(RoomRequest::Start, at).expect("allowed");
        assert_eq!(
            sent.try_recv(),
            Ok(ClientMessage::StartRace),
            "another request is a new press"
        );
        client.handle(NetworkEvent::Message(ServerMessage::Room(lobby(true))), at);
        client
            .request(RoomRequest::ToggleReady, at + REQUEST_REPEAT)
            .expect("allowed");
        assert_eq!(
            sent.try_recv(),
            Ok(ClientMessage::SetReady { ready: false }),
            "the same request after the delay is a new press"
        );
    }
}
