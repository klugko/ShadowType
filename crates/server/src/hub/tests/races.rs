use code_racer_protocol::Phase;

use super::*;

fn one_minute_races() -> ServerConfig {
    ServerConfig {
        race_timeout: Duration::from_secs(60),
        ..ServerConfig::default()
    }
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
    let mut harness = Harness::new(one_minute_races());
    harness.racing_room(0);
    harness.tick(62_999);
    assert!(harness.received(ALICE).is_empty());
    harness.tick(63_000);
    assert_eq!(harness.last_view(ALICE).phase, Phase::Finished);
}

#[test]
fn progress_arriving_after_the_race_timeout_is_ignored_and_the_results_shown_at_once() {
    let mut harness = Harness::new(one_minute_races());
    harness.racing_room(0);
    harness.send(BOB, progress(5), 63_000);
    let messages = harness.received(BOB);
    assert!(
        messages
            .iter()
            .all(|message| matches!(message, ServerMessage::Room(_))),
        "no error for a report that was on its way: {messages:?}"
    );
    let view = harness.last_view(ALICE);
    assert_eq!(view.phase, Phase::Finished);
    assert_eq!(
        view.player(BOB).map(|player| player.progress.correct),
        Some(0)
    );
}

#[test]
fn leaving_a_race_and_joining_it_again_waits_for_the_next_lobby() {
    let mut harness = Harness::new(one_minute_races());
    let code = harness.racing_room(0);
    harness.send(BOB, ClientMessage::LeaveRoom, 3_100);
    harness.send(BOB, ClientMessage::JoinRoom { code: code.clone() }, 3_200);
    assert_eq!(harness.errors(BOB), [ErrorCode::RaceInProgress]);
    harness.send(BOB, progress(3), 3_300);
    assert_eq!(harness.errors(BOB), [ErrorCode::NotInRoom]);

    harness.tick(63_000);
    harness.send(ALICE, ClientMessage::ReturnToLobby, 63_001);
    harness.send(BOB, ClientMessage::JoinRoom { code }, 63_002);
    let view = harness.last_view(BOB);
    assert_eq!(view.phase, Phase::Lobby);
    assert_eq!(view.player(BOB).map(|bob| bob.connected), Some(true));
}

#[test]
fn the_results_are_shown_before_a_return_to_the_lobby_that_beats_the_tick() {
    let mut harness = Harness::new(one_minute_races());
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
