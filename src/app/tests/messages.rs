use super::*;
use crate::{network::NetworkEvent, persist::scratch::TempDir};

#[test]
fn a_message_that_arrives_under_the_command_line_waits_until_it_is_seen() {
    let now = Instant::now();
    let mut app = room::joined(now);
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "hist");
    app.handle_network(
        NetworkEvent::Closed {
            reason: "room FK72AD closed after 30 minutes without activity".to_owned(),
        },
        now,
    );
    let closed = Message::error("room FK72AD closed after 30 minutes without activity");
    press(&mut app, KeyCode::Esc);
    assert!(app.prompt.is_none());
    assert_eq!(app.message(), Some(&closed), "shown once the line closes");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.message(), None, "dismissed once seen");

    let mut app = room::joined(now);
    command(&mut app, "lang french");
    press(&mut app, KeyCode::Char(':'));
    app.handle_network(
        NetworkEvent::Closed {
            reason: "connection lost".to_owned(),
        },
        now,
    );
    type_text(&mut app, "lang rust");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.message(), Some(&Message::error("connection lost")));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        app.message(),
        Some(&Message::info("code language set to rust"))
    );
}

#[test]
fn every_warning_of_the_start_up_is_shown_in_turn() {
    let warnings = vec![
        "config.toml was invalid (line 2); defaults loaded, backup at /home/ada/config.toml.bak"
            .to_owned(),
        "history.json was invalid (line 1); history started afresh".to_owned(),
    ];
    let mut app = App::new(
        Config::default(),
        &Overrides::default(),
        None,
        History::in_memory(),
        warnings.clone(),
        Launch::Home,
    );
    let mut shown = Vec::new();
    for key in "Ada".chars() {
        shown.extend(app.message().map(|message| message.text.clone()));
        press(&mut app, KeyCode::Char(key));
    }
    assert_eq!(
        shown,
        [
            format!("E: {}", warnings[0]),
            format!("E: {}", warnings[1]),
            "welcome! choose the name other racers will see, then Enter (Esc skips)".to_owned(),
        ]
    );
    assert_eq!(app.message(), None);
}

#[test]
fn errors_stay_on_screen_while_typing() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    app.handle_paste("the whole text", Instant::now());
    type_text(&mut app, "ab");
    assert!(app.message().is_some_and(Message::is_error));
    assert_eq!(session(&app).cursor(), 2);
}

#[test]
fn a_failed_save_is_the_message_left_on_screen() {
    let directory = TempDir::new();
    let not_a_directory = directory.join("file");
    std::fs::write(&not_a_directory, "").expect("write");
    let mut app = App::new(
        configured("jean"),
        &Overrides::default(),
        Some(not_a_directory.join("config.toml")),
        History::in_memory(),
        Vec::new(),
        Launch::Home,
    );
    let failed_save = |app: &App| {
        app.message()
            .is_some_and(|message| message.is_error() && message.text.contains("cannot save"))
    };
    command(&mut app, "lang french");
    assert!(failed_save(&app), "{:?}", app.message());
    command(&mut app, "set username=Ada");
    assert!(failed_save(&app), "{:?}", app.message());
}
