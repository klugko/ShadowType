//! Rooms fed with server messages instead of a real server.

use code_racer_engine::{Language, TextSource};
use code_racer_protocol::{
    ClientMessage, Phase, PlayerId, PlayerProgress, PlayerView, RoomView, ServerMessage,
};
use tokio::sync::mpsc;

use super::*;
use crate::{
    app::race::Intent,
    network::{Connection, NetworkEvent},
};

pub const ME: PlayerId = PlayerId(1);
pub const TEXT: &str = "Simplicity is prerequisite.";

/// The room with the player and a rival, `progress` being the player's.
pub fn view(phase: Phase, progress: PlayerProgress) -> RoomView {
    let player = |id, name: &str, progress| PlayerView {
        id,
        name: name.parse().expect("name"),
        ready: true,
        connected: true,
        progress,
    };
    RoomView {
        code: "FK72AD".parse().expect("code"),
        host: ME,
        text: TextSource::Quote {
            language: Language::English,
        },
        text_length: 27,
        phase,
        max_players: 8,
        players: vec![
            player(ME, "jean", progress),
            player(PlayerId(2), "alice", PlayerProgress::default()),
        ],
    }
}

pub fn deliver(app: &mut App, message: ServerMessage, at: Instant) {
    app.handle_network(NetworkEvent::Message(message), at);
}

/// In the lobby of room FK72AD, hosting it.
pub fn joined(at: Instant) -> App {
    let mut app = app();
    command(&mut app, "join FK72AD");
    app.handle_network(NetworkEvent::Connected(ME), at);
    deliver(
        &mut app,
        ServerMessage::Room(view(Phase::Lobby, PlayerProgress::default())),
        at,
    );
    app
}

pub fn counting_down(at: Instant) -> App {
    let mut app = joined(at);
    deliver(
        &mut app,
        ServerMessage::Countdown {
            text: TEXT.to_owned(),
            duration_ms: 3_000,
        },
        at,
    );
    deliver(
        &mut app,
        ServerMessage::Room(view(Phase::Countdown, PlayerProgress::default())),
        at,
    );
    app
}

/// In the lobby over a connection that hands what is sent to the test.
pub fn joined_over_loopback(at: Instant) -> (App, mpsc::Receiver<ClientMessage>) {
    let (connection, sent) = Connection::loopback();
    let code = "FK72AD".parse().expect("code");
    let client = RaceClient::over(connection, "ws://test".to_owned(), Intent::Join(code));
    let mut app = app();
    app.activity = Some(Activity::Race(Box::new(client)));
    app.open(Buffer::Session);
    app.handle_network(NetworkEvent::Connected(ME), at);
    deliver(
        &mut app,
        ServerMessage::Room(view(Phase::Lobby, PlayerProgress::default())),
        at,
    );
    (app, sent)
}

pub fn racing(at: Instant) -> App {
    let mut app = counting_down(at);
    deliver(
        &mut app,
        ServerMessage::Room(view(Phase::Racing, PlayerProgress::default())),
        at,
    );
    app
}

/// The results of a race the player did not finish.
pub fn showing_results(at: Instant) -> App {
    let mut app = racing(at);
    deliver(
        &mut app,
        ServerMessage::Room(view(Phase::Finished, PlayerProgress::default())),
        at,
    );
    app
}
