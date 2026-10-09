//! Typing helpers shared by the tests of the app and of its views.
//!
//! Each keystroke is checked as it is typed: a key the text refuses or
//! judges wrong fails the test at once, where a loop typing "until the
//! text is done" would spin forever.

use std::time::Instant;

use code_racer_engine::{Mark, TypingSession};
use crossterm::event::{KeyCode, KeyEvent};

use super::App;

pub(crate) fn session(app: &App) -> &TypingSession {
    app.session_view().expect("a session").session
}

/// Types the character the text expects next, at `at`, and checks that it
/// was taken and judged right.
pub(crate) fn type_next(app: &mut App, at: Instant) {
    let before = session(app).cursor();
    let next = session(app)
        .target()
        .get(before)
        .expect("text left to type")
        .clone();
    for key in keys_for(&next) {
        app.handle_key(KeyEvent::from(key), at);
    }
    assert_eq!(
        session(app).mark(before),
        Mark::Correct,
        "{next:?} at {before} was refused or judged wrong"
    );
}

/// Every round moves the cursor on, or fails, so the loop always ends.
pub(crate) fn type_remaining_at(app: &mut App, at: Instant) {
    while !session(app).is_finished() {
        type_next(app, at);
    }
}

fn keys_for(grapheme: &str) -> Vec<KeyCode> {
    match grapheme {
        "\n" => vec![KeyCode::Enter],
        text => text.chars().map(KeyCode::Char).collect(),
    }
}
