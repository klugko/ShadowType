use super::*;

#[test]
fn command_line_shows_the_command_and_errors() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    press(&mut app, KeyCode::Char('w'));
    let text = screen(&app, 100, 24);
    assert!(text.contains("COMMAND"));
    assert!(text.lines().any(|line| line.starts_with(":w")));
    press(&mut app, KeyCode::Esc);
    command(&mut app, "nope");
    assert!(screen(&app, 100, 24).contains("E492: Not an editor command: nope"));
    let app = App::new(
        Config::default(),
        &Overrides::default(),
        None,
        History::in_memory(),
        vec!["E:\\notes.txt is empty".to_owned()],
        Launch::Home,
    );
    let text = screen(&app, 100, 24);
    assert!(
        text.lines()
            .any(|line| line.starts_with("E: E:\\notes.txt is empty")),
        "an error is never taken for a numbered one:\n{text}"
    );
}

#[test]
fn a_long_warning_keeps_its_backup_path_on_the_smallest_screen() {
    let backup = "/home/ada/.config/code-racer/config.toml.bak";
    let warning = format!(
        "config.toml was invalid (line 2: invalid string, expected `\"`); defaults loaded, backup at {backup}"
    );
    let app = App::new(
        Config::default(),
        &Overrides::default(),
        None,
        History::in_memory(),
        vec![warning],
        Launch::Home,
    );
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    assert!(text.contains(backup), "{text}");
    assert!(text.contains("INSERT"), "the status line stays:\n{text}");
}

/// The command line of a terminal.
fn last_line(terminal: &Terminal<TestBackend>) -> String {
    text_of(terminal)
        .lines()
        .last()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_command_longer_than_the_line_scrolls_to_keep_the_cursor_in_view() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    type_keys(
        &mut app,
        "e /home/someone/projects/a-rather-long-directory-name/src/some/module/file_name.rs",
    );
    let (cursor, terminal) = cursor_of(&app, MIN_WIDTH, MIN_HEIGHT);
    let line = last_line(&terminal);
    assert!(line.starts_with(":…"), "the start is cut: {line}");
    let (x, y) = find(&terminal, "file_name.rs").expect("the end of the command");
    assert_eq!(cursor, Position::new(x + 12, y), "just after it: {line}");

    press(&mut app, KeyCode::Home);
    let (cursor, terminal) = cursor_of(&app, MIN_WIDTH, MIN_HEIGHT);
    let line = last_line(&terminal);
    assert!(line.starts_with(":e /home/someone"), "{line}");
    assert_eq!(cursor, Position::new(1, MIN_HEIGHT - 1));
}
