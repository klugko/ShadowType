use code_racer_protocol::Phase;

use super::*;
use crate::hub::describe;

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
