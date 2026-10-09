mod command_line;
mod explorer;
mod forms;
mod keys;
mod lobby;
mod messages;
mod multiplayer;
mod quiet_period;
mod races;
mod room;
mod saving;
mod scrolling;
mod solo;
mod username;

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{
    test_support::{session, type_remaining_at},
    *,
};
use crate::{cli::Launch, config::Config, history::History};

fn configured(username: &str) -> Config {
    Config {
        username: username.to_owned(),
        ..Config::default()
    }
}

fn app_with(config: Config, launch: Launch) -> App {
    App::new(
        config,
        &Overrides::default(),
        None,
        History::in_memory(),
        Vec::new(),
        launch,
    )
}

fn app() -> App {
    app_with(configured("jean"), Launch::Home)
}

/// Comfortably longer than the quiet period that follows the end of typing.
const AFTER_QUIET: Duration = Duration::from_secs(1);

fn press(app: &mut App, code: KeyCode) {
    press_at(app, code, Instant::now());
}

fn press_at(app: &mut App, code: KeyCode, at: Instant) {
    app.handle_key(KeyEvent::from(code), at);
}

fn press_with(app: &mut App, ch: char, modifiers: KeyModifiers) {
    app.handle_key(KeyEvent::new(KeyCode::Char(ch), modifiers), Instant::now());
}

fn released(code: KeyCode) -> KeyEvent {
    KeyEvent {
        kind: KeyEventKind::Release,
        ..KeyEvent::from(code)
    }
}

fn type_text(app: &mut App, text: &str) {
    type_text_at(app, text, Instant::now());
}

fn type_text_at(app: &mut App, text: &str, at: Instant) {
    for ch in text.chars() {
        let code = if ch == '\n' {
            KeyCode::Enter
        } else {
            KeyCode::Char(ch)
        };
        press_at(app, code, at);
    }
}

fn command(app: &mut App, line: &str) {
    press(app, KeyCode::Char(':'));
    type_text(app, line);
    press(app, KeyCode::Enter);
}

fn type_remaining(app: &mut App) {
    type_remaining_at(app, Instant::now());
}
