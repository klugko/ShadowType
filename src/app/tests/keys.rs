use super::*;
use crate::persist::scratch::TempDir;

#[test]
fn key_releases_are_ignored() {
    let mut app = app();
    app.handle_key(released(KeyCode::Char(':')), Instant::now());
    assert!(app.prompt.is_none(), "a release opens nothing");
    press(&mut app, KeyCode::Char('s'));
    let next = session(&app).target()[0].clone();
    for ch in next.chars() {
        press(&mut app, KeyCode::Char(ch));
        app.handle_key(released(KeyCode::Char(ch)), Instant::now());
    }
    assert_eq!(session(&app).cursor(), 1, "typed once");
}

#[test]
fn control_c_always_quits() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    press_with(&mut app, 'c', KeyModifiers::CONTROL);
    assert!(app.should_quit());
}

#[test]
fn altgr_characters_are_typed_but_shortcuts_are_not() {
    let directory = TempDir::new();
    let path = directory.join("snippet.txt");
    std::fs::write(&path, "@a").expect("write");
    let mut app = app();
    command(&mut app, &format!("e {}", path.display()));
    press_with(&mut app, 'a', KeyModifiers::CONTROL);
    assert_eq!(session(&app).cursor(), 0, "Ctrl+A is not text");
    press_with(&mut app, '@', KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(session(&app).cursor(), 1, "AltGr+0 types @ on AZERTY");
}

#[test]
fn altgr_characters_reach_the_command_line_and_fields() {
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    press_with(&mut app, '@', altgr);
    press_with(&mut app, '\\', altgr);
    assert_eq!(
        app.prompt.as_ref().map(|prompt| prompt.input.value()),
        Some("@\\")
    );
    press(&mut app, KeyCode::Esc);
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Char('i'));
    press_with(&mut app, 'u', KeyModifiers::CONTROL);
    press_with(&mut app, '[', altgr);
    assert_eq!(
        app.editing.as_ref().map(|edit| edit.input.value()),
        Some("[")
    );
}

#[test]
fn control_h_erases_one_character_in_the_session() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    type_text(&mut app, "xy");
    press_with(&mut app, 'h', KeyModifiers::CONTROL);
    assert_eq!(session(&app).cursor(), 1);
}
