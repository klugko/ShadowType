//! The hub is the single owner of every room and connected player.
//!
//! It runs as one task fed with [`Command`]s, so no state is shared between
//! connections. [`Hub`] holds the logic and is driven synchronously with
//! explicit instants; [`run`] is the thin async loop around it.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use code_racer_engine::TextSource;
use code_racer_protocol::{
    ClientMessage, ErrorCode, PlayerId, Progress, RACE_WORD_COUNTS, RoomCode, ServerError,
    ServerMessage, Username, is_raceable,
};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use tokio::{
    sync::mpsc::{self, error::TrySendError},
    time::{self, MissedTickBehavior},
};
use tracing::warn;

use crate::{config::ServerConfig, room::Room};

/// How often rooms advance and coalesced progress is broadcast (20 Hz).
const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// What connections tell the hub.
#[derive(Debug)]
pub enum Command {
    /// A player completed the handshake. Messages for them go to `outbox`.
    Connected {
        id: PlayerId,
        name: Username,
        outbox: mpsc::Sender<ServerMessage>,
    },
    Message {
        id: PlayerId,
        message: ClientMessage,
    },
    Disconnected {
        id: PlayerId,
    },
}

#[derive(Debug)]
struct Player {
    name: Username,
    outbox: mpsc::Sender<ServerMessage>,
    room: Option<RoomCode>,
}

#[derive(Debug)]
pub struct Hub {
    config: ServerConfig,
    players: HashMap<PlayerId, Player>,
    rooms: HashMap<RoomCode, Room>,
    /// Rooms whose progress changed since the last broadcast.
    dirty: HashSet<RoomCode>,
    /// Players whose outbox refused a message, dropped once the current command is handled.
    stuck: Vec<PlayerId>,
    rng: StdRng,
}

impl Hub {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            players: HashMap::new(),
            rooms: HashMap::new(),
            dirty: HashSet::new(),
            stuck: Vec::new(),
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    pub fn handle(&mut self, command: Command, now: Instant) {
        match command {
            Command::Connected { id, name, outbox } => {
                self.players.insert(
                    id,
                    Player {
                        name,
                        outbox,
                        room: None,
                    },
                );
            }
            Command::Message { id, message } => self.handle_message(id, message, now),
            Command::Disconnected { id } => self.disconnect(id, now),
        }
        self.drop_stuck_players(now);
    }

    /// Advances every room, broadcasts the rooms that changed and closes idle ones.
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

    fn handle_message(&mut self, id: PlayerId, message: ClientMessage, now: Instant) {
        if !self.players.contains_key(&id) {
            return;
        }
        let result = match message {
            ClientMessage::Hello { .. } => Err(ServerError::new(
                ErrorCode::InvalidMessage,
                "the handshake is already done",
            )),
            ClientMessage::CreateRoom { text } => self.create_room(id, text, now),
            ClientMessage::JoinRoom { code } => self.join_room(id, code, now),
            ClientMessage::LeaveRoom => self.leave_room(id, now),
            ClientMessage::SetReady { ready } => self.set_ready(id, ready, now),
            ClientMessage::StartRace => self.start_race(id, now),
            ClientMessage::Progress(progress) => self.report_progress(id, progress, now),
            ClientMessage::ReturnToLobby => self.return_to_lobby(id, now),
        };
        if let Err(error) = result {
            self.send(id, ServerMessage::Error(error));
        }
    }

    fn create_room(
        &mut self,
        id: PlayerId,
        text: TextSource,
        now: Instant,
    ) -> Result<(), ServerError> {
        if !is_raceable(&text) {
            return Err(ServerError::new(
                ErrorCode::InvalidSettings,
                format!(
                    "races need {} to {} words",
                    RACE_WORD_COUNTS.start(),
                    RACE_WORD_COUNTS.end()
                ),
            ));
        }
        if self.rooms.len() >= self.config.max_rooms {
            return Err(ServerError::new(
                ErrorCode::ServerFull,
                "the server cannot host more rooms, try again later",
            ));
        }
        let name = self.name_of(id)?;
        let code = self.unused_code();
        let room = Room::new(code.clone(), id, name, text, self.config.max_players, now);
        self.rooms.insert(code.clone(), room);
        self.move_player(id, code, now);
        Ok(())
    }

    fn join_room(&mut self, id: PlayerId, code: RoomCode, now: Instant) -> Result<(), ServerError> {
        if self.room_of(id) == Some(&code) {
            self.send_view(id, &code);
            return Ok(());
        }
        let name = self.name_of(id)?;
        let room = self.rooms.get_mut(&code).ok_or_else(|| {
            ServerError::new(ErrorCode::RoomNotFound, format!("room {code} not found"))
        })?;
        room.join(id, name, now)?;
        self.move_player(id, code, now);
        Ok(())
    }

    fn leave_room(&mut self, id: PlayerId, now: Instant) -> Result<(), ServerError> {
        self.current_room(id, now)?;
        self.leave_current_room(id, now);
        Ok(())
    }

    fn set_ready(&mut self, id: PlayerId, ready: bool, now: Instant) -> Result<(), ServerError> {
        let (code, room) = self.current_room(id, now)?;
        room.set_ready(id, ready, now)?;
        self.broadcast_view(&code);
        Ok(())
    }

    fn start_race(&mut self, id: PlayerId, now: Instant) -> Result<(), ServerError> {
        let seed = self.rng.random();
        let countdown = self.config.countdown;
        let (code, room) = self.current_room(id, now)?;
        let text = room.text_source().generate(seed).text;
        room.start_countdown(id, &text, now, countdown)?;
        let duration_ms = u32::try_from(countdown.as_millis()).unwrap_or(u32::MAX);
        self.broadcast(&code, &ServerMessage::Countdown { text, duration_ms });
        self.broadcast_view(&code);
        Ok(())
    }

    fn report_progress(
        &mut self,
        id: PlayerId,
        progress: Progress,
        now: Instant,
    ) -> Result<(), ServerError> {
        let (code, room) = self.current_room(id, now)?;
        room.report_progress(id, progress, now)?;
        self.dirty.insert(code);
        Ok(())
    }

    fn return_to_lobby(&mut self, id: PlayerId, now: Instant) -> Result<(), ServerError> {
        let (code, room) = self.current_room(id, now)?;
        room.return_to_lobby(id, now)?;
        self.broadcast_view(&code);
        Ok(())
    }

    fn disconnect(&mut self, id: PlayerId, now: Instant) {
        if let Some(Player {
            room: Some(code), ..
        }) = self.players.remove(&id)
        {
            self.remove_member(id, &code, now);
        }
    }

    /// Puts a player who was just added to the room `code` there, leaving their previous room.
    fn move_player(&mut self, id: PlayerId, code: RoomCode, now: Instant) {
        self.leave_current_room(id, now);
        if let Some(player) = self.players.get_mut(&id) {
            player.room = Some(code.clone());
        }
        self.broadcast_view(&code);
    }

    fn leave_current_room(&mut self, id: PlayerId, now: Instant) {
        if let Some(code) = self
            .players
            .get_mut(&id)
            .and_then(|player| player.room.take())
        {
            self.remove_member(id, &code, now);
        }
    }

    fn remove_member(&mut self, id: PlayerId, code: &RoomCode, now: Instant) {
        let Some(room) = self.rooms.get_mut(code) else {
            return;
        };
        room.leave(id, now);
        room.advance(now, self.config.race_timeout);
        if room.has_connected_members() {
            self.broadcast_view(code);
        } else {
            self.rooms.remove(code);
            self.dirty.remove(code);
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

    fn broadcast_view(&mut self, code: &RoomCode) {
        if let Some(room) = self.rooms.get(code) {
            let view = ServerMessage::Room(room.view());
            self.broadcast(code, &view);
            self.dirty.remove(code);
        }
    }

    fn broadcast(&mut self, code: &RoomCode, message: &ServerMessage) {
        let Some(room) = self.rooms.get(code) else {
            return;
        };
        for id in room.connected_members() {
            if !deliver(&self.players, id, message.clone()) {
                self.stuck.push(id);
            }
        }
    }

    fn send_view(&mut self, id: PlayerId, code: &RoomCode) {
        if let Some(view) = self.rooms.get(code).map(Room::view) {
            self.send(id, ServerMessage::Room(view));
        }
    }

    fn send(&mut self, id: PlayerId, message: ServerMessage) {
        if !deliver(&self.players, id, message) {
            self.stuck.push(id);
        }
    }

    fn drop_stuck_players(&mut self, now: Instant) {
        while let Some(id) = self.stuck.pop() {
            self.disconnect(id, now);
        }
    }

    /// The room of a player, first brought up to date with the clock so that
    /// no request acts on a countdown or a race that is already over.
    fn current_room(
        &mut self,
        id: PlayerId,
        now: Instant,
    ) -> Result<(RoomCode, &mut Room), ServerError> {
        let not_in_room = || ServerError::new(ErrorCode::NotInRoom, "you are not in a room");
        let code = self.room_of(id).cloned().ok_or_else(not_in_room)?;
        self.advance_room(&code, now);
        let room = self.rooms.get_mut(&code).ok_or_else(not_in_room)?;
        Ok((code, room))
    }

    fn advance_room(&mut self, code: &RoomCode, now: Instant) {
        let race_timeout = self.config.race_timeout;
        let changed = self
            .rooms
            .get_mut(code)
            .is_some_and(|room| room.advance(now, race_timeout));
        if changed {
            self.broadcast_view(code);
        }
    }

    fn room_of(&self, id: PlayerId) -> Option<&RoomCode> {
        self.players.get(&id)?.room.as_ref()
    }

    fn name_of(&self, id: PlayerId) -> Result<Username, ServerError> {
        self.players
            .get(&id)
            .map(|player| player.name.clone())
            .ok_or_else(|| ServerError::new(ErrorCode::HandshakeRequired, "say hello first"))
    }

    fn unused_code(&mut self) -> RoomCode {
        loop {
            let code = RoomCode::random(&mut self.rng);
            if !self.rooms.contains_key(&code) {
                return code;
            }
        }
    }
}

/// Runs the hub until every connection and the server dropped their command senders.
pub async fn run(mut hub: Hub, mut commands: mpsc::Receiver<Command>) {
    let mut ticker = time::interval(TICK_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(command) => hub.handle(command, Instant::now()),
                None => return,
            },
            _ = ticker.tick() => hub.tick(Instant::now()),
        }
    }
}

/// Queues a message for a player. Returns false when their connection is gone
/// or so far behind that its outbox is full.
fn deliver(players: &HashMap<PlayerId, Player>, id: PlayerId, message: ServerMessage) -> bool {
    let Some(player) = players.get(&id) else {
        return true;
    };
    match player.outbox.try_send(message) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            warn!(player = %id, "dropping a client that stopped reading its messages");
            false
        }
        Err(TrySendError::Closed(_)) => false,
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
    use code_racer_engine::{Language, WordOptions};
    use code_racer_protocol::{Phase, RoomView};

    use super::*;

    const ALICE: PlayerId = PlayerId(1);
    const BOB: PlayerId = PlayerId(2);
    const CAROL: PlayerId = PlayerId(3);
    const INBOX: usize = 64;

    struct Harness {
        hub: Hub,
        inboxes: HashMap<PlayerId, mpsc::Receiver<ServerMessage>>,
        start: Instant,
    }

    impl Harness {
        fn new(config: ServerConfig) -> Self {
            Self {
                hub: Hub::new(config),
                inboxes: HashMap::new(),
                start: Instant::now(),
            }
        }

        fn at(&self, millis: u64) -> Instant {
            self.start + Duration::from_millis(millis)
        }

        fn connect(&mut self, id: PlayerId, name: &str, capacity: usize) {
            let (outbox, inbox) = mpsc::channel(capacity);
            let name = name.parse().expect("valid name");
            self.hub
                .handle(Command::Connected { id, name, outbox }, self.at(0));
            self.inboxes.insert(id, inbox);
        }

        fn send(&mut self, id: PlayerId, message: ClientMessage, millis: u64) {
            let now = self.at(millis);
            self.hub.handle(Command::Message { id, message }, now);
        }

        fn tick(&mut self, millis: u64) {
            let now = self.at(millis);
            self.hub.tick(now);
        }

        fn received(&mut self, id: PlayerId) -> Vec<ServerMessage> {
            let inbox = self.inboxes.get_mut(&id).expect("connected player");
            std::iter::from_fn(|| inbox.try_recv().ok()).collect()
        }

        fn last_view(&mut self, id: PlayerId) -> RoomView {
            self.received(id)
                .into_iter()
                .rev()
                .find_map(|message| match message {
                    ServerMessage::Room(view) => Some(view),
                    _ => None,
                })
                .expect("a room view")
        }

        fn errors(&mut self, id: PlayerId) -> Vec<ErrorCode> {
            self.received(id)
                .into_iter()
                .filter_map(|message| match message {
                    ServerMessage::Error(error) => Some(error.code),
                    _ => None,
                })
                .collect()
        }

        /// Alice creates a room that Bob joins, then both get ready.
        fn ready_room(&mut self) -> RoomCode {
            self.connect(ALICE, "Alice", INBOX);
            self.connect(BOB, "Bob", INBOX);
            self.send(ALICE, create(words(10)), 0);
            let code = self.last_view(ALICE).code;
            self.send(BOB, ClientMessage::JoinRoom { code: code.clone() }, 0);
            self.send(ALICE, ClientMessage::SetReady { ready: true }, 0);
            self.send(BOB, ClientMessage::SetReady { ready: true }, 0);
            self.received(ALICE);
            self.received(BOB);
            code
        }

        /// A ready room where the race started at `millis` and is now running.
        fn racing_room(&mut self, millis: u64) -> RoomCode {
            let code = self.ready_room();
            self.send(ALICE, ClientMessage::StartRace, millis);
            self.tick(millis + 3_000);
            self.received(ALICE);
            self.received(BOB);
            code
        }
    }

    fn words(count: u16) -> TextSource {
        TextSource::Words {
            language: Language::French,
            count,
            options: WordOptions::default(),
        }
    }

    fn create(text: TextSource) -> ClientMessage {
        ClientMessage::CreateRoom { text }
    }

    fn progress(correct: u32) -> ClientMessage {
        ClientMessage::Progress(Progress {
            typed: correct,
            correct,
            indentation: 0,
            keystrokes: correct,
            errors: 0,
        })
    }

    #[test]
    fn creating_a_room_makes_its_creator_host() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.connect(ALICE, "Alice", 8);
        harness.send(ALICE, create(words(10)), 0);
        let view = harness.last_view(ALICE);
        assert_eq!(view.host, ALICE);
        assert_eq!(view.phase, Phase::Lobby);
        assert_eq!(view.max_players, 8);
        assert_eq!(view.text, words(10));
    }

    #[test]
    fn rooms_need_raceable_settings_and_free_capacity() {
        let config = ServerConfig {
            max_rooms: 1,
            ..ServerConfig::default()
        };
        let mut harness = Harness::new(config);
        harness.connect(ALICE, "Alice", 8);
        harness.connect(BOB, "Bob", 8);
        harness.send(ALICE, create(words(4)), 0);
        assert_eq!(harness.errors(ALICE), [ErrorCode::InvalidSettings]);

        harness.send(ALICE, create(words(5)), 0);
        harness.send(BOB, create(words(5)), 0);
        assert_eq!(harness.errors(BOB), [ErrorCode::ServerFull]);
    }

    #[test]
    fn creating_a_room_while_in_one_leaves_the_previous_room() {
        let mut harness = Harness::new(ServerConfig::default());
        let first = harness.ready_room();
        harness.send(ALICE, create(words(20)), 1);
        let second = harness.last_view(ALICE);
        assert_ne!(second.code, first);
        assert_eq!(second.players.len(), 1);

        let first_view = harness.last_view(BOB);
        assert_eq!(first_view.code, first);
        assert_eq!(first_view.host, BOB);
        assert_eq!(first_view.player(ALICE), None);
    }

    #[test]
    fn joining_broadcasts_to_every_member() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.connect(ALICE, "Alice", 8);
        harness.connect(BOB, "Bob", 8);
        harness.send(ALICE, create(words(10)), 0);
        let code = harness.last_view(ALICE).code;
        harness.send(BOB, ClientMessage::JoinRoom { code }, 0);
        assert_eq!(harness.last_view(ALICE).players.len(), 2);
        assert_eq!(harness.last_view(BOB).players.len(), 2);
    }

    #[test]
    fn joining_an_unknown_room_fails_without_leaving_the_current_one() {
        let mut harness = Harness::new(ServerConfig::default());
        let code = harness.ready_room();
        let unknown: RoomCode = "ZZZZZZ".parse().expect("valid code");
        harness.send(BOB, ClientMessage::JoinRoom { code: unknown }, 1);
        assert_eq!(harness.errors(BOB), [ErrorCode::RoomNotFound]);
        assert!(harness.received(ALICE).is_empty());
        harness.send(BOB, ClientMessage::SetReady { ready: false }, 2);
        assert_eq!(harness.last_view(ALICE).code, code);
    }

    #[test]
    fn room_messages_outside_a_room_are_refused() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.connect(ALICE, "Alice", 16);
        for message in [
            ClientMessage::LeaveRoom,
            ClientMessage::SetReady { ready: true },
            ClientMessage::StartRace,
            progress(1),
            ClientMessage::ReturnToLobby,
        ] {
            harness.send(ALICE, message, 0);
        }
        assert_eq!(harness.errors(ALICE), [ErrorCode::NotInRoom; 5]);
    }

    #[test]
    fn a_second_hello_is_an_invalid_message() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.connect(ALICE, "Alice", 8);
        let hello = ClientMessage::Hello {
            version: code_racer_protocol::PROTOCOL_VERSION,
            username: "Mallory".parse().expect("valid name"),
        };
        harness.send(ALICE, hello, 0);
        assert_eq!(harness.errors(ALICE), [ErrorCode::InvalidMessage]);
    }

    #[test]
    fn errors_only_reach_the_requester() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.ready_room();
        harness.send(BOB, ClientMessage::StartRace, 1);
        assert_eq!(harness.errors(BOB), [ErrorCode::NotHost]);
        assert!(harness.received(ALICE).is_empty());
    }

    #[test]
    fn the_race_text_is_announced_before_the_countdown_view() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.ready_room();
        harness.send(ALICE, ClientMessage::StartRace, 1);
        for id in [ALICE, BOB] {
            let messages = harness.received(id);
            let [
                ServerMessage::Countdown { text, duration_ms },
                ServerMessage::Room(view),
            ] = messages.as_slice()
            else {
                panic!("unexpected messages {messages:?}");
            };
            assert_eq!(*duration_ms, 3_000);
            assert_eq!(text.split(' ').count(), 10);
            assert_eq!(view.phase, Phase::Countdown);
            assert_eq!(
                view.text_length as usize,
                code_racer_engine::grapheme_count(text)
            );
        }
    }

    #[test]
    fn ticks_start_the_race_after_the_countdown() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.ready_room();
        harness.send(ALICE, ClientMessage::StartRace, 1_000);
        harness.received(ALICE);
        harness.tick(3_999);
        assert!(harness.received(ALICE).is_empty());
        harness.tick(4_000);
        assert_eq!(harness.last_view(ALICE).phase, Phase::Racing);
        assert_eq!(harness.last_view(BOB).phase, Phase::Racing);
    }

    #[test]
    fn progress_is_broadcast_once_per_tick() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.racing_room(0);
        harness.send(ALICE, progress(3), 3_100);
        harness.send(BOB, progress(4), 3_110);
        harness.send(ALICE, progress(6), 3_120);
        assert!(harness.received(ALICE).is_empty());
        assert!(harness.received(BOB).is_empty());

        harness.tick(3_150);
        let messages = harness.received(BOB);
        assert_eq!(messages.len(), 1);
        let ServerMessage::Room(view) = &messages[0] else {
            panic!("expected a room view, got {messages:?}");
        };
        assert_eq!(
            view.player(ALICE).map(|player| player.progress.correct),
            Some(6)
        );
        assert_eq!(
            view.player(BOB).map(|player| player.progress.correct),
            Some(4)
        );

        harness.tick(3_200);
        assert!(harness.received(BOB).is_empty());
    }

    #[test]
    fn rejected_progress_is_reported_to_the_sender_only() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.racing_room(0);
        harness.send(BOB, progress(5_000), 3_100);
        assert_eq!(harness.errors(BOB), [ErrorCode::InvalidProgress]);
        harness.tick(3_150);
        assert!(harness.received(ALICE).is_empty());
    }

    #[test]
    fn finishing_players_end_the_race_on_the_next_tick() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.ready_room();
        harness.send(ALICE, ClientMessage::StartRace, 0);
        let length = match harness.received(ALICE).first() {
            Some(ServerMessage::Countdown { text, .. }) => code_racer_engine::grapheme_count(text),
            other => panic!("expected the countdown, got {other:?}"),
        };
        let length = u32::try_from(length).expect("short text");
        harness.tick(3_000);
        harness.send(BOB, progress(length), 13_000);
        harness.send(ALICE, progress(length), 14_000);
        harness.tick(14_050);
        let view = harness.last_view(ALICE);
        assert_eq!(view.phase, Phase::Finished);
        assert_eq!(view.place_of(BOB), Some(1));
        assert_eq!(
            view.player(BOB)
                .and_then(|player| player.progress.finish_ms),
            Some(10_000)
        );
    }

    #[test]
    fn races_time_out() {
        let config = ServerConfig {
            race_timeout: Duration::from_secs(60),
            ..ServerConfig::default()
        };
        let mut harness = Harness::new(config);
        harness.racing_room(0);
        harness.tick(62_999);
        assert!(harness.received(ALICE).is_empty());
        harness.tick(63_000);
        assert_eq!(harness.last_view(ALICE).phase, Phase::Finished);
    }

    #[test]
    fn progress_after_the_race_timeout_is_refused_and_the_results_shown_without_a_tick() {
        let config = ServerConfig {
            race_timeout: Duration::from_secs(60),
            ..ServerConfig::default()
        };
        let mut harness = Harness::new(config);
        harness.racing_room(0);
        harness.send(BOB, progress(5), 63_000);
        assert_eq!(harness.errors(BOB), [ErrorCode::RaceNotRunning]);
        let view = harness.last_view(ALICE);
        assert_eq!(view.phase, Phase::Finished);
        assert_eq!(
            view.player(BOB).map(|player| player.progress.correct),
            Some(0)
        );
    }

    #[test]
    fn the_results_are_shown_before_a_return_to_the_lobby_that_beats_the_tick() {
        let config = ServerConfig {
            race_timeout: Duration::from_secs(60),
            ..ServerConfig::default()
        };
        let mut harness = Harness::new(config);
        harness.racing_room(0);
        harness.send(ALICE, ClientMessage::ReturnToLobby, 63_000);
        let phases: Vec<Phase> = harness
            .received(BOB)
            .into_iter()
            .map(|message| match message {
                ServerMessage::Room(view) => view.phase,
                other => panic!("expected room views, got {other:?}"),
            })
            .collect();
        assert_eq!(phases, [Phase::Finished, Phase::Lobby]);
    }

    #[test]
    fn rejoining_ones_own_room_only_resends_the_view_to_the_requester() {
        let mut harness = Harness::new(ServerConfig::default());
        let code = harness.ready_room();
        harness.send(BOB, ClientMessage::JoinRoom { code: code.clone() }, 1);
        assert_eq!(harness.last_view(BOB).code, code);
        assert!(harness.received(ALICE).is_empty());
    }

    #[test]
    fn a_client_that_stops_reading_mid_race_is_shown_offline() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.racing_room(0);
        for millis in (3_050..).step_by(50).take(INBOX + 1) {
            harness.received(ALICE);
            harness.send(ALICE, progress(1), millis);
            harness.tick(millis);
        }
        let view = harness.last_view(ALICE);
        assert_eq!(view.phase, Phase::Racing);
        assert_eq!(view.player(BOB).map(|player| player.connected), Some(false));
        assert_eq!(harness.received(BOB).len(), INBOX);
    }

    #[test]
    fn disconnecting_in_the_lobby_removes_the_player_and_hands_over_the_host() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.ready_room();
        harness
            .hub
            .handle(Command::Disconnected { id: ALICE }, harness.at(1));
        let view = harness.last_view(BOB);
        assert_eq!(view.players.len(), 1);
        assert_eq!(view.host, BOB);
    }

    #[test]
    fn disconnecting_mid_race_keeps_the_player_listed_offline() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.racing_room(0);
        harness.send(BOB, progress(10), 4_000);
        harness
            .hub
            .handle(Command::Disconnected { id: BOB }, harness.at(5_000));
        let view = harness.last_view(ALICE);
        let bob = view.player(BOB).expect("still listed");
        assert!(!bob.connected);
        assert_eq!(bob.progress.correct, 10);
        assert_eq!(view.phase, Phase::Racing);
    }

    #[test]
    fn the_last_member_leaving_closes_the_room() {
        let mut harness = Harness::new(ServerConfig::default());
        let code = harness.ready_room();
        harness.send(BOB, ClientMessage::LeaveRoom, 1);
        harness
            .hub
            .handle(Command::Disconnected { id: ALICE }, harness.at(2));
        harness.send(BOB, ClientMessage::JoinRoom { code }, 3);
        assert_eq!(harness.errors(BOB), [ErrorCode::RoomNotFound]);
    }

    #[test]
    fn returning_to_the_lobby_forgets_players_who_left_mid_race() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.connect(CAROL, "Carol", INBOX);
        let code = harness.ready_room();
        harness.send(CAROL, ClientMessage::JoinRoom { code }, 1);
        harness.send(CAROL, ClientMessage::SetReady { ready: true }, 1);
        harness.send(ALICE, ClientMessage::StartRace, 2);
        harness.tick(3_002);
        harness
            .hub
            .handle(Command::Disconnected { id: CAROL }, harness.at(4_000));
        harness.send(BOB, ClientMessage::LeaveRoom, 4_000);
        harness.send(ALICE, ClientMessage::ReturnToLobby, 4_001);
        assert_eq!(harness.errors(ALICE), [ErrorCode::RaceInProgress]);

        harness.tick(400_000);
        harness.send(ALICE, ClientMessage::ReturnToLobby, 400_001);
        let view = harness.last_view(ALICE);
        assert_eq!(view.phase, Phase::Lobby);
        let ids: Vec<PlayerId> = view.players.iter().map(|player| player.id).collect();
        assert_eq!(ids, [ALICE]);
    }

    #[test]
    fn idle_rooms_are_closed_and_their_members_told() {
        let config = ServerConfig {
            room_ttl: Duration::from_secs(600),
            ..ServerConfig::default()
        };
        let mut harness = Harness::new(config);
        harness.ready_room();
        harness.tick(599_999);
        assert!(harness.received(ALICE).is_empty());
        harness.tick(600_000);
        for id in [ALICE, BOB] {
            let messages = harness.received(id);
            let [ServerMessage::Error(error)] = messages.as_slice() else {
                panic!("expected a closing notice, got {messages:?}");
            };
            assert_eq!(error.code, ErrorCode::RoomNotFound);
            assert!(
                error
                    .message
                    .ends_with("closed after 10 minutes of inactivity")
            );
        }
        harness.send(ALICE, ClientMessage::SetReady { ready: false }, 600_001);
        assert_eq!(harness.errors(ALICE), [ErrorCode::NotInRoom]);
    }

    #[test]
    fn a_client_whose_outbox_overflows_is_dropped() {
        let mut harness = Harness::new(ServerConfig::default());
        harness.connect(ALICE, "Alice", INBOX);
        harness.connect(BOB, "Bob", 2);
        harness.send(ALICE, create(words(10)), 0);
        let code = harness.last_view(ALICE).code;
        harness.send(BOB, ClientMessage::JoinRoom { code }, 0);
        harness.send(ALICE, ClientMessage::SetReady { ready: true }, 1);
        harness.send(ALICE, ClientMessage::SetReady { ready: false }, 2);

        let view = harness.last_view(ALICE);
        assert_eq!(view.players.len(), 1);
        assert_eq!(view.player(BOB), None);
        harness.send(BOB, ClientMessage::LeaveRoom, 3);
        assert_eq!(harness.received(BOB).len(), 2);
    }

    #[test]
    fn closing_notices_use_readable_durations() {
        assert_eq!(describe(Duration::from_secs(1_800)), "30 minutes");
        assert_eq!(describe(Duration::from_secs(60)), "1 minute");
        assert_eq!(describe(Duration::from_secs(45)), "45 seconds");
        assert_eq!(describe(Duration::from_secs(1)), "1 second");
    }
}
