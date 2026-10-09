use code_racer_protocol::{Phase, PlayerProgress};

use super::*;

#[test]
fn only_the_host_starts_and_only_when_everyone_is_ready() {
    let clock = Clock(Instant::now());
    let mut room = lobby(&clock, &[BOB, CAROL], 8);
    room.set_ready(ALICE, true, clock.at(0)).expect("ready");
    room.set_ready(BOB, true, clock.at(0)).expect("ready");

    assert_eq!(
        error_code(room.start_countdown(BOB, "text", clock.at(1), COUNTDOWN)),
        Some(ErrorCode::NotHost)
    );
    let not_ready = room.start_countdown(ALICE, "text", clock.at(1), COUNTDOWN);
    assert_eq!(
        not_ready.map_err(|error| (error.code, error.message)),
        Err((
            ErrorCode::PlayersNotReady,
            "waiting for Carol to get ready".to_owned()
        ))
    );
    assert_eq!(room.view().phase, Phase::Lobby);

    room.set_ready(CAROL, true, clock.at(2)).expect("ready");
    room.start_countdown(ALICE, "héllo 👋", clock.at(2), COUNTDOWN)
        .expect("start");
    let view = room.view();
    assert_eq!(view.phase, Phase::Countdown);
    assert_eq!(view.text_length, 7);
    assert_eq!(
        error_code(room.start_countdown(ALICE, "again", clock.at(3), COUNTDOWN)),
        Some(ErrorCode::RaceInProgress)
    );
}

#[test]
fn the_countdown_turns_into_a_race_at_its_deadline() {
    let clock = Clock(Instant::now());
    let mut room = counting_down(&clock, &[BOB]);
    assert!(!room.advance(clock.at(2_999), TIMEOUT));
    assert_eq!(room.view().phase, Phase::Countdown);
    assert!(room.advance(clock.at(3_000), TIMEOUT));
    assert_eq!(room.view().phase, Phase::Racing);
    assert!(!room.advance(clock.at(3_050), TIMEOUT));
}

#[test]
fn the_race_ends_when_every_connected_player_finished_and_ranks_them_by_time() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.report_progress(BOB, clean(TEXT_LENGTH), clock.racing(18_000))
        .expect("bob finishes");
    assert!(!room.advance(clock.racing(18_000), TIMEOUT));
    assert_eq!(room.view().phase, Phase::Racing);

    room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(19_000))
        .expect("alice finishes");
    assert!(room.advance(clock.racing(19_000), TIMEOUT));
    let view = room.view();
    assert_eq!(view.phase, Phase::Finished);
    let standings: Vec<PlayerId> = view.standings().iter().map(|player| player.id).collect();
    assert_eq!(standings, [BOB, ALICE]);
}

#[test]
fn the_race_ends_at_the_timeout_even_if_nobody_finished() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.report_progress(BOB, clean(40), clock.racing(10_000))
        .expect("progress");
    assert!(!room.advance(clock.racing(299_999), TIMEOUT));
    assert!(room.advance(clock.racing(300_000), TIMEOUT));
    assert_eq!(room.view().phase, Phase::Finished);
    room.report_progress(BOB, clean(50), clock.racing(300_001))
        .expect("a report sent before the end is ignored");
    assert_eq!(player(&room, BOB).progress.correct, 40);
}

#[test]
fn the_race_ends_when_the_last_unfinished_player_leaves() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(20_000))
        .expect("alice finishes");
    room.leave(BOB, clock.racing(21_000));
    assert!(room.advance(clock.racing(21_000), TIMEOUT));
    assert_eq!(room.view().phase, Phase::Finished);
}

#[test]
fn the_race_ends_when_nobody_is_connected() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[BOB]);
    room.leave(ALICE, clock.racing(1_000));
    room.leave(BOB, clock.racing(1_000));
    assert!(!room.has_connected_members());
    assert!(room.advance(clock.racing(1_000), TIMEOUT));
    assert_eq!(room.view().phase, Phase::Finished);
}

#[test]
fn only_the_host_returns_a_finished_race_to_the_lobby() {
    let clock = Clock(Instant::now());
    let mut room = lobby(&clock, &[BOB], 8);
    assert_eq!(
        error_code(room.return_to_lobby(ALICE, clock.at(1))),
        Some(ErrorCode::RaceNotRunning)
    );

    let mut room = racing(&clock, &[BOB, CAROL]);
    assert_eq!(
        error_code(room.return_to_lobby(ALICE, clock.racing(1))),
        Some(ErrorCode::RaceInProgress)
    );
    room.leave(CAROL, clock.racing(1_000));
    room.report_progress(ALICE, clean(TEXT_LENGTH), clock.racing(10_000))
        .expect("alice finishes");
    room.report_progress(BOB, clean(TEXT_LENGTH), clock.racing(12_000))
        .expect("bob finishes");
    room.advance(clock.racing(12_000), TIMEOUT);
    assert_eq!(
        error_code(room.return_to_lobby(BOB, clock.racing(13_000))),
        Some(ErrorCode::NotHost)
    );

    room.return_to_lobby(ALICE, clock.racing(13_000))
        .expect("back to the lobby");
    let view = room.view();
    assert_eq!(view.phase, Phase::Lobby);
    assert_eq!(view.text_length, 0);
    assert_eq!(view.players.len(), 2);
    for player in &view.players {
        assert!(!player.ready);
        assert_eq!(player.progress, PlayerProgress::default());
    }
}

#[test]
fn counters_start_over_after_returning_to_the_lobby() {
    let clock = Clock(Instant::now());
    let mut room = racing(&clock, &[]);
    room.report_progress(
        ALICE,
        progress(TEXT_LENGTH, TEXT_LENGTH, 120, 20),
        clock.racing(30_000),
    )
    .expect("finish");
    room.advance(clock.racing(30_000), TIMEOUT);
    room.return_to_lobby(ALICE, clock.racing(31_000))
        .expect("lobby");
    room.set_ready(ALICE, true, clock.racing(31_000))
        .expect("ready");
    let rematch = clock.racing(32_000);
    room.start_countdown(ALICE, "short text", rematch, COUNTDOWN)
        .expect("rematch");
    room.report_progress(ALICE, clean(3), rematch + COUNTDOWN)
        .expect("fewer keystrokes than the previous race is fine");
}

#[test]
fn idle_time_counts_from_the_last_accepted_action() {
    let clock = Clock(Instant::now());
    let mut room = lobby(&clock, &[BOB], 8);
    room.set_ready(BOB, true, clock.at(1_000)).expect("ready");
    assert_eq!(room.idle_for(clock.at(5_000)), Duration::from_secs(4));
    assert!(
        room.start_countdown(BOB, "text", clock.at(4_000), COUNTDOWN)
            .is_err()
    );
    assert_eq!(room.idle_for(clock.at(5_000)), Duration::from_secs(4));
    assert_eq!(room.idle_for(clock.at(0)), Duration::ZERO);
}
