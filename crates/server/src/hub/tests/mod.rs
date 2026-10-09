mod departures;
mod races;
mod rooms;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use code_racer_engine::{Language, TextSource, WordOptions};
use code_racer_protocol::{
    ClientMessage, ErrorCode, PlayerId, Progress, RoomCode, RoomView, ServerMessage,
};
use tokio::sync::mpsc;

use super::{Command, Hub};
use crate::config::ServerConfig;

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

    fn disconnect(&mut self, id: PlayerId, millis: u64) {
        let now = self.at(millis);
        self.hub.handle(Command::Disconnected { id }, now);
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
