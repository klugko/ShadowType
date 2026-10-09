use code_racer_protocol::{ErrorCode, Phase, PlayerProgress, ServerMessage};

use super::*;

#[test]
fn leaving_a_race_takes_two_escapes_until_the_room_is_back_in_the_lobby() {
    for start in [room::counting_down, room::racing, room::showing_results] {
        let now = Instant::now();
        let mut app = start(now);
        let first = now + AFTER_QUIET;
        press_at(&mut app, KeyCode::Esc, first);
        assert!(app.race().is_some());
        assert_eq!(
            app.message(),
            Some(&Message::info("press Esc again to leave FK72AD"))
        );
        press_at(&mut app, KeyCode::Esc, first + Duration::from_secs(1));
        assert!(app.activity.is_none());
        assert_eq!(app.buffer, Buffer::Race);
        assert_eq!(app.message(), Some(&Message::info("left FK72AD")));
    }
}

#[test]
fn server_errors_are_shown_and_a_closed_room_is_left() {
    let now = Instant::now();
    let mut app = room::racing(now);
    room::deliver(
        &mut app,
        ServerMessage::error(ErrorCode::InvalidProgress, "progress rejected"),
        now,
    );
    assert!(app.race().is_some(), "a rejected message keeps the race");
    assert_eq!(app.message(), Some(&Message::error("progress rejected")));
    room::deliver(
        &mut app,
        ServerMessage::error(ErrorCode::RoomNotFound, "room FK72AD closed"),
        now,
    );
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Race);
    assert_eq!(
        app.message(),
        Some(&Message::error("progress rejected")),
        "an error stays until a key dismisses it"
    );
    press_at(&mut app, KeyCode::Char('j'), now + AFTER_QUIET);
    assert_eq!(app.message(), Some(&Message::error("room FK72AD closed")));
}

#[test]
fn the_countdown_brings_the_player_back_to_the_race_text() {
    let now = Instant::now();
    let mut app = room::joined(now);
    press(&mut app, KeyCode::Char('?'));
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "wq");
    room::deliver(
        &mut app,
        ServerMessage::Countdown {
            text: room::TEXT.to_owned(),
            duration_ms: 3_000,
        },
        now,
    );
    assert!(app.prompt.is_none(), "the command line is cancelled");
    assert_eq!((app.buffer, app.focus), (Buffer::Session, Focus::Editor));
    let counting_down = room::view(Phase::Countdown, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(counting_down), now);
    assert!(app.is_typing());
    let start = now + Duration::from_secs(3);
    let racing = room::view(Phase::Racing, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(racing), start);
    type_text_at(&mut app, "Simp", start);
    assert_eq!(session(&app).cursor(), 4);
    assert!(!app.should_quit());
}

#[test]
fn the_countdown_cancels_a_field_being_typed() {
    let now = Instant::now();
    let mut app = room::joined(now);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Enter);
    assert!(app.editing.is_some());
    room::deliver(
        &mut app,
        ServerMessage::Countdown {
            text: room::TEXT.to_owned(),
            duration_ms: 3_000,
        },
        now,
    );
    assert!(app.editing.is_none());
    assert_eq!(app.buffer, Buffer::Session);
}

#[test]
fn a_race_is_recorded_once_as_soon_as_the_server_times_the_finish() {
    let now = Instant::now();
    let mut app = room::racing(now);
    let typed_at = now + Duration::from_secs(10);
    type_remaining_at(&mut app, typed_at);
    assert!(
        app.history.records().is_empty(),
        "not timed by the server yet"
    );
    let finished = PlayerProgress {
        typed: 27,
        correct: 27,
        errors: 0,
        wpm: 32.4,
        accuracy: 100.0,
        finish_ms: Some(10_000),
    };
    let racing = room::view(Phase::Racing, finished);
    room::deliver(&mut app, ServerMessage::Room(racing), typed_at);
    let records = app.history.records();
    assert_eq!(records.len(), 1, "recorded while others still race");
    assert_eq!(
        (records[0].mode.as_str(), records[0].language.as_str()),
        ("race", "english")
    );
    assert_eq!((records[0].duration, records[0].wpm), (10.0, 32.4));
    assert_eq!(records[0].text_length, room::TEXT.len());
    let over = room::view(Phase::Finished, finished);
    room::deliver(&mut app, ServerMessage::Room(over), typed_at);
    assert_eq!(app.history.records().len(), 1, "never twice");
    press_at(&mut app, KeyCode::Esc, typed_at + AFTER_QUIET);
    assert!(app.race().is_some(), "the results are worth a second Esc");
    press_at(&mut app, KeyCode::Esc, typed_at + AFTER_QUIET);
    assert!(app.activity.is_none());
}

#[test]
fn a_race_where_nothing_was_typed_is_not_recorded() {
    let now = Instant::now();
    let mut app = room::racing(now);
    let over = room::view(Phase::Finished, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(over), now);
    assert!(app.history.records().is_empty());
    assert!(app.race().is_some(), "the results are shown");
}

#[test]
fn the_clock_of_an_unfinished_race_stops_when_the_race_ends() {
    let start = Instant::now();
    let mut app = room::racing(start);
    type_text_at(&mut app, "Simp", start + Duration::from_secs(2));
    let end = start + Duration::from_secs(10);
    let over = room::view(Phase::Finished, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(over), end);

    let view = app.session_view().expect("the race text");
    let later = end + Duration::from_secs(50);
    assert_eq!(view.stats(later), view.stats(end), "frozen");
    assert_eq!(view.stats(later).elapsed, Duration::from_secs(10));
    assert_eq!(app.history.records()[0].duration, 10.0);
}
