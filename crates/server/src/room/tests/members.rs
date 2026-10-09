use code_racer_protocol::{MAX_MESSAGE_BYTES, MAX_ROOM_PLAYERS, Phase, ServerMessage};

use super::*;

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

/**
 * The valid name with the longest JSON: as many bytes as allowed, and a
 * quote, which JSON escapes, at the start of each of its characters.
 */
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
fn a_player_who_left_a_running_race_cannot_rejoin_it() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.leave(BOB, clock.racing(1_000));
    assert_eq!(
        error_code(room.join(BOB, name(BOB), clock.racing(2_000))),
        Some(ErrorCode::RaceInProgress)
    );
    assert!(!player(&room, BOB).connected);
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
fn a_host_leaving_the_results_stays_listed_offline_and_hands_over_to_a_connected_member() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB, CAROL]);
    room.leave(BOB, clock.racing(1_000));
    room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(10_000))
        .expect("alice finishes first");
    room.report_progress(CAROL, clean(TEXT_LENGTH), clock.racing(12_000))
        .expect("carol finishes");
    assert!(room.advance(clock.racing(12_000), TIMEOUT));

    room.leave(ALICE, clock.racing(13_000));
    let view = room.view();
    assert_eq!(view.phase, Phase::Finished);
    assert_eq!(view.host, CAROL);
    assert_eq!(
        view.player(ALICE).map(|player| player.connected),
        Some(false)
    );
    assert_eq!(view.place_of(ALICE), Some(1));
    assert_eq!(view.place_of(CAROL), Some(2));
    assert_eq!(view.player(BOB).map(|player| player.connected), Some(false));

    room.return_to_lobby(CAROL, clock.racing(14_000))
        .expect("back to the lobby");
    let ids: Vec<PlayerId> = room.view().players.iter().map(|player| player.id).collect();
    assert_eq!(ids, [CAROL]);
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
