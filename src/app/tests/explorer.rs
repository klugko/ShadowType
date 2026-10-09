use super::*;

#[test]
fn starts_in_the_explorer_on_the_practice_buffer() {
    let app = app();
    assert_eq!(app.focus, Focus::Explorer);
    assert_eq!(app.buffer, Buffer::Practice);
    assert_eq!(app.editor_mode(), EditorMode::Normal);
    assert_eq!(app.entries(), Buffer::FILES.to_vec());
}

#[test]
fn explorer_previews_buffers_and_enter_focuses_the_editor() {
    let mut app = app();
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.buffer, Buffer::Race);
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.buffer, Buffer::Help);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.buffer, Buffer::Help, "stays on the last entry");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Editor);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Explorer);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Editor);
}

#[test]
fn the_explorer_has_the_focus_only_while_it_is_shown() {
    let mut app = app();
    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    assert!(!app.sidebar);
    assert_eq!(app.focus, Focus::Editor);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.buffer, Buffer::Practice, "j moves in the form");
    press(&mut app, KeyCode::Esc);
    assert!(app.sidebar, "Esc brings the explorer back");
    assert_eq!(app.focus, Focus::Explorer);
    command(&mut app, "set nosidebar");
    assert_eq!((app.sidebar, app.focus), (false, Focus::Editor));
    press(&mut app, KeyCode::Tab);
    assert_eq!((app.sidebar, app.focus), (true, Focus::Explorer));
}

#[test]
fn the_explorer_makes_room_on_narrow_terminals_until_the_user_decides() {
    let mut app = app();
    assert_eq!(app.focus, Focus::Explorer);
    app.resize(80, 20);
    assert_eq!((app.sidebar, app.focus), (false, Focus::Editor));
    app.resize(120, 30);
    assert!(app.sidebar, "shown again once there is room");

    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    app.resize(200, 60);
    assert!(!app.sidebar, "hidden by the user, whatever the width");
    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    app.resize(80, 20);
    assert!(app.sidebar, "shown by the user, whatever the width");
}

#[test]
fn on_a_narrow_terminal_the_explorer_shows_only_while_it_has_the_focus() {
    let mut app = app();
    app.resize(80, 20);
    press(&mut app, KeyCode::Char('?'));
    for (reveal, back) in [
        (KeyCode::Esc, KeyCode::Enter),
        (KeyCode::Char('h'), KeyCode::Char('l')),
        (KeyCode::Left, KeyCode::Right),
        (KeyCode::Tab, KeyCode::Tab),
        (KeyCode::Esc, KeyCode::Char('?')),
    ] {
        press(&mut app, reveal);
        assert_eq!(
            (app.sidebar, app.focus),
            (true, Focus::Explorer),
            "{reveal}"
        );
        press(&mut app, back);
        assert_eq!((app.sidebar, app.focus), (false, Focus::Editor), "{back}");
    }

    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Enter);
    assert!(app.sidebar, "pinned by the user");
    command(&mut app, "set nosidebar");
    press(&mut app, KeyCode::Esc);
    assert!(app.sidebar, "shown while it has the focus");
    press(&mut app, KeyCode::Enter);
    assert!(!app.sidebar, "hidden again, as the user wants it");
}

#[test]
fn returning_to_a_visible_explorer_keeps_it_automatic() {
    let mut app = app();
    app.resize(120, 30);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Explorer);
    app.resize(80, 20);
    assert!(!app.sidebar, "still hides itself on a narrow terminal");
}

#[test]
fn home_on_a_narrow_terminal_focuses_the_practice_form() {
    let mut app = app_with(Config::default(), Launch::Home);
    app.resize(80, 20);
    type_text(&mut app, "Ada");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        (app.buffer, app.focus, app.sidebar),
        (Buffer::Practice, Focus::Editor, false)
    );
}
