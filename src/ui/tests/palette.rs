use crossterm::event::KeyModifiers;

use super::*;

#[test]
fn the_command_palette_finds_and_runs_a_command() {
    let mut app = app();
    app.handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let text = screen(&app, 100, 30);
    for expected in ["commands", "Start a session", "COMMAND", "Enter run"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    type_keys(&mut app, "hist");
    let text = screen(&app, 100, 30);
    assert!(text.contains("Open history.log"), "{text}");
    assert!(!text.contains("Start a session"), "{text}");
    press(&mut app, KeyCode::Enter);
    assert!(app.palette.is_none());
    assert_eq!(app.buffer, crate::app::Buffer::History);
}

#[test]
fn f1_opens_the_palette_even_while_typing_and_esc_closes_it() {
    let mut app = app();
    command(&mut app, "words 10");
    type_prefix(&mut app, 2);
    press(&mut app, KeyCode::F(1));
    assert!(app.palette.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.palette.is_none());
    assert!(app.is_typing(), "Esc closed the palette, not the session");
}
