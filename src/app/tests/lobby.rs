use code_racer_engine::CodeLanguage;
use code_racer_protocol::{ClientMessage, Phase, PlayerProgress, ServerMessage};

use super::*;
use crate::{network::NetworkEvent, persist::scratch::TempDir};

#[test]
fn a_lobby_is_left_with_one_escape() {
    let now = Instant::now();
    let mut app = room::joined(now);
    press_at(&mut app, KeyCode::Esc, now);
    assert!(app.activity.is_none());
}

#[test]
fn releasing_the_ready_key_sends_nothing() {
    let now = Instant::now();
    let (mut app, mut sent) = room::joined_over_loopback(now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::JoinRoom { .. })
    ));
    press_at(&mut app, KeyCode::Char('r'), now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::SetReady { .. })
    ));
    let answer = room::view(Phase::Lobby, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(answer), now);
    app.handle_key(
        released(KeyCode::Char('r')),
        now + Duration::from_millis(200),
    );
    assert!(sent.try_recv().is_err(), "the release is not a press");
}

#[test]
fn quitting_in_a_room_leaves_it() {
    for quit in [KeyCode::Char('q'), KeyCode::Char('c')] {
        let now = Instant::now();
        let (mut app, mut sent) = room::joined_over_loopback(now);
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMessage::JoinRoom { .. })
        ));
        if quit == KeyCode::Char('c') {
            press_with(&mut app, 'c', KeyModifiers::CONTROL);
        } else {
            press(&mut app, quit);
        }
        assert!(app.should_quit());
        let connection = app.finish();
        assert!(connection.is_some(), "the goodbye still has to be sent");
        assert_eq!(sent.try_recv(), Ok(ClientMessage::LeaveRoom));
    }
    assert!(self::app().finish().is_none(), "no room, nothing to send");
}

#[test]
fn lang_with_a_programming_language_only_sets_the_code_language() {
    let mut app = room::joined(Instant::now());
    command(&mut app, "lang python");
    assert!(app.race().is_some(), "still in the room");
    assert_eq!(app.config.practice.code_language, CodeLanguage::Python);
    assert_eq!(app.config.race.code_language, CodeLanguage::Python);
    assert_eq!(
        app.message(),
        Some(&Message::info("code language set to python"))
    );
}

#[test]
fn entering_a_room_cancels_a_field_left_open_while_connecting() {
    let now = Instant::now();
    let mut app = app();
    command(&mut app, "create");
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Enter);
    assert!(app.editing.is_some());
    app.handle_network(NetworkEvent::Connected(room::ME), now);
    let lobby = room::view(Phase::Lobby, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(lobby), now);
    assert_eq!(app.buffer, Buffer::Session);
    assert!(app.editing.is_none());
    assert_eq!(app.editor_mode(), EditorMode::Normal);
}

#[test]
fn starting_something_else_in_a_room_is_refused() {
    let directory = TempDir::new();
    let file = directory.join("notes.txt");
    std::fs::write(&file, "some text").expect("write");
    let now = Instant::now();
    let mut app = room::joined(now);
    let practice = app.config.practice;
    let refused = Some(Message::error("leave room FK72AD first with Esc"));
    press(&mut app, KeyCode::Char('?'));
    for key in ['s', 'c'] {
        press(&mut app, KeyCode::Char(key));
        assert!(app.race().is_some(), "{key}");
        assert_eq!(app.message(), refused.as_ref(), "{key}");
    }
    let edit = format!("e {}", file.display());
    for line in [
        "solo",
        "words 10",
        "quote",
        "code rust",
        "create",
        "join ABCDEF",
        &edit,
    ] {
        command(&mut app, line);
        assert!(app.race().is_some(), "{line}");
        assert_eq!(app.message(), refused.as_ref(), "{line}");
    }
    assert_eq!(
        app.config.practice, practice,
        "refused commands change nothing"
    );
    assert_eq!(
        app.room_code, "FK72AD",
        "the room line keeps the room joined"
    );
}

#[test]
fn holding_the_ready_key_sends_one_request() {
    let now = Instant::now();
    let (mut app, mut sent) = room::joined_over_loopback(now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::JoinRoom { .. })
    ));
    let ready = KeyEvent::from(KeyCode::Char('r'));
    let held = KeyEvent {
        kind: KeyEventKind::Repeat,
        ..ready
    };
    app.handle_key(ready, now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::SetReady { .. })
    ));
    let answer = room::view(Phase::Lobby, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(answer.clone()), now);
    app.handle_key(held, now);
    app.handle_key(held, now);
    assert!(sent.try_recv().is_err(), "auto-repeat is ignored");
    let mut at = now;
    for _ in 0..20 {
        at += Duration::from_millis(30);
        app.handle_key(ready, at);
        room::deliver(&mut app, ServerMessage::Room(answer.clone()), at);
    }
    assert!(
        sent.try_recv().is_err(),
        "repeats reported as presses are ignored too"
    );
    app.handle_key(ready, at + Duration::from_millis(200));
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::SetReady { .. })
    ));
}
