use super::*;
use crate::network::NetworkEvent;

#[test]
fn keys_right_after_the_time_runs_out_are_ignored() {
    let mut app = app();
    command(&mut app, "time 15");
    let start = Instant::now();
    press_at(&mut app, KeyCode::Char('x'), start);
    let end = start + Duration::from_secs(16);
    app.tick(end);
    let in_flight = end + Duration::from_millis(50);
    for code in [
        KeyCode::Char('q'),
        KeyCode::Char('e'),
        KeyCode::Char(':'),
        KeyCode::Tab,
        KeyCode::Enter,
    ] {
        press_at(&mut app, code, in_flight);
    }
    assert!(!app.should_quit());
    assert!(app.solo().is_some_and(|run| run.result.is_some()));
    assert_eq!((app.buffer, app.focus), (Buffer::Session, Focus::Editor));
    assert!(app.prompt.is_none());
    press_at(&mut app, KeyCode::Char('r'), end + AFTER_QUIET);
    assert!(
        app.solo().is_some_and(|run| run.result.is_none()),
        "r restarts once the quiet period is over"
    );
}

#[test]
fn a_reflex_enter_after_the_last_character_keeps_the_results() {
    let mut app = app();
    command(&mut app, "code rust");
    let now = Instant::now();
    type_remaining_at(&mut app, now);
    press_at(&mut app, KeyCode::Enter, now + Duration::from_millis(100));
    assert!(app.solo().is_some_and(|run| run.result.is_some()));
    press_with(&mut app, 'c', KeyModifiers::CONTROL);
    assert!(app.should_quit(), "Ctrl+C is never ignored");
}

#[test]
fn keys_right_after_losing_the_connection_keep_the_reason_on_screen() {
    let now = Instant::now();
    let mut app = room::racing(now);
    let settings = app.config.race;
    app.handle_network(
        NetworkEvent::Closed {
            reason: "connection lost".to_owned(),
        },
        now,
    );
    let in_flight = now + Duration::from_millis(50);
    for code in [
        KeyCode::Char('s'),
        KeyCode::Char('l'),
        KeyCode::Char('q'),
        KeyCode::Enter,
    ] {
        press_at(&mut app, code, in_flight);
    }
    assert!(!app.should_quit());
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Race);
    assert_eq!(app.config.race, settings);
    assert_eq!(app.message(), Some(&Message::error("connection lost")));
}
