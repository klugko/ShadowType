use super::*;

#[test]
fn the_terminal_cursor_follows_the_value_being_typed() {
    let mut app = app();
    command(&mut app, "config");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Left);
    let (cursor, terminal) = cursor_of(&app, 120, 30);
    let (x, y) = find(&terminal, "username   = \"jean\"").expect("the line");
    assert_eq!(cursor, Position::new(x + 16, y), "after \"je\"");

    press(&mut app, KeyCode::Esc);
    command(&mut app, "race");
    press(&mut app, KeyCode::Enter);
    type_keys(&mut app, "FK7");
    let (cursor, terminal) = cursor_of(&app, MIN_WIDTH, MIN_HEIGHT);
    let (x, y) = find(&terminal, "room        = \"FK7\"").expect("the line");
    assert_eq!(cursor, Position::new(x + 18, y), "after \"FK7\"");
}

#[test]
fn a_value_longer_than_its_field_scrolls_to_keep_the_cursor_in_view() {
    let mut app = app();
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    press_ctrl(&mut app, 'u');
    let host = "a-very-long-host-name".repeat(4);
    type_keys(&mut app, &format!("ws://{host}.example.com:8080"));
    let (cursor, terminal) = cursor_of(&app, MIN_WIDTH, MIN_HEIGHT);
    let text = text_of(&terminal);
    let (x, y) = find(&terminal, "example.com:8080\"").expect("the end of the value");
    assert_eq!(cursor, Position::new(x + 16, y), "just after it:\n{text}");
    let row = text.lines().nth(usize::from(y)).unwrap_or_default();
    assert!(row.contains("server     = \"…"), "the start is cut: {row}");
}
