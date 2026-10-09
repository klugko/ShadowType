use code_racer_engine::Language;

use super::*;
use crate::config::Theme;

#[test]
fn commands_change_settings_and_report_errors() {
    let mut app = app();
    command(&mut app, "words 10");
    assert_eq!(session(&app).target().concat().split(' ').count(), 10);
    press(&mut app, KeyCode::Esc);
    command(&mut app, "set theme=mono");
    assert_eq!(app.config.theme, Theme::Mono);
    command(&mut app, "set punctuation");
    assert!(app.config.practice.punctuation);
    command(&mut app, "lang french");
    assert_eq!(app.config.practice.language, Language::French);
    command(&mut app, "frobnicate");
    assert!(
        app.message()
            .is_some_and(|message| message.is_error() && message.text.starts_with("E492"))
    );
    command(&mut app, "q");
    assert!(app.should_quit());
}

#[test]
fn tab_completes_command_names() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "hi");
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        app.prompt.as_ref().map(|prompt| prompt.input.value()),
        Some("history")
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.buffer, Buffer::History);
    assert!(app.prompt.is_none());
}

#[test]
fn backspace_on_an_empty_command_line_cancels_it() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    assert_eq!(app.editor_mode(), EditorMode::Command);
    press(&mut app, KeyCode::Backspace);
    assert!(app.prompt.is_none());
}
