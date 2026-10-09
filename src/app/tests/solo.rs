use code_racer_engine::{ERROR_RUN_LIMIT, Status};

use super::*;
use crate::{config::Mode, persist::scratch::TempDir};

#[test]
fn every_printable_key_is_text_while_typing() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    assert!(app.is_typing());
    assert_eq!(app.editor_mode(), EditorMode::Insert);
    type_text(&mut app, "q:?");
    assert!(!app.should_quit());
    assert!(app.prompt.is_none());
    assert_eq!(session(&app).cursor(), 3);
}

#[test]
fn completing_a_session_records_it_and_shows_results() {
    let mut app = app();
    command(&mut app, "quote");
    assert_eq!(app.config.practice.mode, Mode::Quote);
    let now = Instant::now();
    type_remaining_at(&mut app, now);
    let run = app.solo().expect("solo run");
    assert_eq!(run.session().status(), Status::Completed);
    assert!(run.result.is_some());
    assert_eq!(app.history.records().len(), 1);
    assert_eq!(app.editor_mode(), EditorMode::Normal);
    press_at(&mut app, KeyCode::Char('r'), now + AFTER_QUIET);
    assert!(
        app.solo().is_some_and(|run| run.result.is_none()),
        "r restarts"
    );
    press_at(&mut app, KeyCode::Esc, now + AFTER_QUIET);
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Practice);
    assert_eq!(app.message(), Some(&Message::info("session abandoned")));
}

#[test]
fn code_sessions_complete_with_auto_indentation() {
    let mut app = app();
    command(&mut app, "code python");
    assert_eq!(app.config.practice.code_language.name(), "python");
    type_remaining(&mut app);
    assert_eq!(session(&app).status(), Status::Completed);
    assert_eq!(session(&app).stats(Instant::now()).errors, 0);
}

#[test]
fn timed_sessions_end_on_the_clock() {
    let mut app = app();
    command(&mut app, "time 15");
    let start = Instant::now();
    app.handle_key(KeyEvent::from(KeyCode::Char('x')), start);
    app.tick(start + Duration::from_secs(16));
    assert_eq!(session(&app).status(), Status::TimeUp);
    assert_eq!(app.history.records()[0].mode, "time 15");
}

#[test]
fn abandoning_a_started_session_takes_two_escapes() {
    let mut app = app();
    let now = Instant::now();
    press_at(&mut app, KeyCode::Char('s'), now);
    press_at(&mut app, KeyCode::Char('x'), now);
    press_at(&mut app, KeyCode::Esc, now);
    assert!(app.solo().is_some());
    assert_eq!(
        app.message(),
        Some(&Message::info("press Esc again to abandon the session"))
    );
    press_at(&mut app, KeyCode::Esc, now + Duration::from_secs(3));
    assert!(app.solo().is_some(), "too late: the first press expired");
    press_at(&mut app, KeyCode::Char('y'), now + Duration::from_secs(3));
    press_at(&mut app, KeyCode::Esc, now + Duration::from_secs(4));
    assert!(app.solo().is_some(), "a key in between cancels leaving");
    press_at(&mut app, KeyCode::Esc, now + Duration::from_secs(5));
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Practice);
    assert_eq!(app.message(), Some(&Message::info("session abandoned")));
}

#[test]
fn typing_blocked_by_a_mistake_says_how_to_go_on() {
    let mut app = app();
    command(&mut app, "words 10");
    type_text(&mut app, &"#".repeat(ERROR_RUN_LIMIT));
    assert!(app.is_typing_blocked());
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(session(&app).cursor(), ERROR_RUN_LIMIT);
    assert_eq!(
        app.message(),
        Some(&Message::info("fix the mistake first: Backspace or Ctrl+W"))
    );
    press(&mut app, KeyCode::Backspace);
    assert!(!app.is_typing_blocked());
    assert_eq!(app.message(), None);
}

#[test]
fn a_mistake_left_at_the_end_of_the_text_says_how_to_finish() {
    let directory = TempDir::new();
    let path = directory.join("short.txt");
    std::fs::write(&path, "say hello").expect("write");
    let mut app = app();
    command(&mut app, &format!("e {}", path.display()));
    type_text(&mut app, "say hellp");
    assert!(app.is_typing_blocked(), "the badge shows at once");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.message(),
        Some(&Message::info("fix the mistake first: Backspace or Ctrl+W"))
    );
    press(&mut app, KeyCode::Backspace);
    type_text(&mut app, "o");
    assert_eq!(session(&app).status(), Status::Completed);
}

#[test]
fn pasting_is_refused_while_typing() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    app.handle_paste("the whole text", Instant::now());
    assert_eq!(session(&app).cursor(), 0);
    assert!(app.message().is_some_and(Message::is_error));
}
