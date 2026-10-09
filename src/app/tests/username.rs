use super::*;
use crate::config::Practice;

#[test]
fn first_launch_asks_for_a_username_then_continues() {
    let launch = Launch::Solo {
        practice: Practice::default(),
        file: None,
    };
    let mut app = app_with(Config::default(), launch);
    assert_eq!(app.buffer, Buffer::Settings);
    assert_eq!(app.editor_mode(), EditorMode::Insert);
    press(&mut app, KeyCode::Enter);
    assert!(
        app.message().is_some_and(Message::is_error),
        "empty name refused"
    );
    assert!(app.editing.is_some());
    type_text(&mut app, "Ada");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.config.username, "Ada");
    assert!(app.editing.is_none());
    assert!(app.solo().is_some(), "the solo launch resumes");
    assert!(app.is_typing());
}

#[test]
fn escape_skips_the_first_name_and_the_launch_goes_on() {
    let launch = Launch::Solo {
        practice: Practice::default(),
        file: None,
    };
    let mut app = app_with(Config::default(), launch);
    press(&mut app, KeyCode::Esc);
    assert!(app.editing.is_none());
    assert!(app.solo().is_some(), "a solo session needs no name");
}

#[test]
fn a_race_launched_without_a_name_says_it_needs_one() {
    let launch = Launch::Join("FK72AD".parse().expect("code"));
    let mut app = app_with(Config::default(), launch);
    press(&mut app, KeyCode::Esc);
    assert!(app.activity.is_none());
    assert!(
        app.message()
            .is_some_and(|message| message.is_error() && message.text.contains("username"))
    );
    command(&mut app, "set username=Ada");
    assert!(
        app.activity.is_none(),
        "the skipped race does not come back"
    );
}

#[test]
fn creating_a_room_without_a_name_goes_on_once_it_is_set() {
    let mut app = app_with(Config::default(), Launch::Home);
    press(&mut app, KeyCode::Esc);
    assert_eq!((app.buffer, app.focus), (Buffer::Practice, Focus::Explorer));
    press(&mut app, KeyCode::Char('c'));
    assert!(matches!(&app.editing, Some(edit) if edit.field == TextField::Username));
    type_text(&mut app, "Ada");
    press(&mut app, KeyCode::Enter);
    assert!(app.race().is_some(), "the room is being created");
}
