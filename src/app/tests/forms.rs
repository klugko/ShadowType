use super::*;
use crate::config::{Mode, Theme};

#[test]
fn practice_form_edits_the_settings() {
    let mut app = app();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.config.practice.mode, Mode::Time);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.config.practice.duration, 60);
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    assert!(app.is_typing(), "Enter on the last line starts the session");
}

#[test]
fn invalid_room_codes_are_rejected_before_connecting() {
    let mut app = app();
    press(&mut app, KeyCode::Char('m'));
    assert_eq!(app.buffer, Buffer::Race);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(&app.editing, Some(edit) if edit.field == TextField::RoomCode));
    type_text(&mut app, "AB0");
    press(&mut app, KeyCode::Enter);
    assert!(app.message().is_some_and(Message::is_error));
    assert!(app.activity.is_none());
    press(&mut app, KeyCode::Esc);
    assert!(app.editing.is_none());
}

#[test]
fn settings_changes_the_theme_in_place() {
    let mut app = app();
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.config.theme, Theme::Dark);
}
