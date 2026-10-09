use super::*;

#[test]
fn a_look_names_the_file_and_its_type() {
    let mut app = app();
    command(&mut app, "set look=commit");
    command(&mut app, "words 10");
    let text = screen(&app, 100, 30);
    assert!(text.contains("COMMIT_EDITMSG"), "{text}");
    assert!(text.contains("# On branch main"), "{text}");
    assert!(status_line(&text).trim_end().ends_with('%'), "{text}");
    press(&mut app, KeyCode::Esc);
    command(&mut app, "set look=mail");
    command(&mut app, "quote");
    let text = screen(&app, 100, 30);
    assert!(text.contains("draft.eml") && text.contains("From: jean <jean@localhost>"));
}

#[test]
fn discreet_mode_shows_an_editor_and_nothing_else() {
    let mut app = app();
    app.history.add(record(70.0)).expect("in memory");
    app.resize(120, 30);
    press(&mut app, KeyCode::F(12));
    assert!(app.config.discreet);
    command(&mut app, "words 10");
    type_prefix(&mut app, 3);
    let text = screen(&app, 120, 30);
    let status = status_line(&text);
    for expected in ["Ln 1, Col 4", "UTF-8", "markdown", "⎇ main"] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
    for hidden in ["wpm", "RECORDS", "code-racer", "words 10", "Esc Esc", "█"] {
        assert!(!text.contains(hidden), "{hidden} shows:\n{text}");
    }
    press(&mut app, KeyCode::F(12));
    assert!(!app.config.discreet);
    assert!(status_line(&screen(&app, 120, 30)).contains("wpm"));
}
