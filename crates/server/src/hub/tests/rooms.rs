use code_racer_protocol::Phase;

use super::*;

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
fn rooms_never_hold_more_players_than_the_protocol_allows() {
    let config = ServerConfig {
        max_players: u8::MAX,
        ..ServerConfig::default()
    };
    let mut harness = Harness::new(config);
    harness.connect(ALICE, "Alice", 8);
    harness.send(ALICE, create(words(10)), 0);
    assert_eq!(
        harness.last_view(ALICE).max_players,
        code_racer_protocol::MAX_ROOM_PLAYERS
    );
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
fn rejoining_ones_own_room_only_resends_the_view_to_the_requester() {
    let mut harness = Harness::new(ServerConfig::default());
    let code = harness.ready_room();
    harness.send(BOB, ClientMessage::JoinRoom { code: code.clone() }, 1);
    assert_eq!(harness.last_view(BOB).code, code);
    assert!(harness.received(ALICE).is_empty());
}
