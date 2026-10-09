use code_racer_protocol::{ErrorCode, ServerMessage};
use tokio::sync::mpsc;

use super::{requests::REQUEST_REPEAT, *};
use crate::{app::text_event::TextEvent, network::NetworkEvent};

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

/**
 * A client in the lobby over a connection that hands what it sends to
 * the test, the join request already taken.
 */
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
