mod animation;
mod command_line;
mod forms;
mod history;
mod layout;
mod looks;
mod mouse;
mod multiplayer;
mod palette;
mod preview;
mod results;
mod status;
mod themes;
mod typing;

use std::time::{Duration, Instant};

use code_racer_engine::{Language, TextSource};
use code_racer_protocol::{Phase, PlayerId, PlayerProgress, PlayerView, RoomView, ServerMessage};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend, layout::Position, style::Style};

use super::*;
use crate::{
    app::{
        App, Overrides,
        test_support::{type_next, type_remaining_at},
    },
    cli::Launch,
    config::Config,
    history::{History, Record},
    network::NetworkEvent,
};

fn app() -> App {
    let config = Config {
        username: "jean".to_owned(),
        ..Config::default()
    };
    App::new(
        config,
        &Overrides::default(),
        None,
        History::in_memory(),
        Vec::new(),
        Launch::Home,
    )
}

fn screen(app: &App, width: u16, height: u16) -> String {
    screen_at(app, width, height, Instant::now())
}

fn screen_at(app: &App, width: u16, height: u16, now: Instant) -> String {
    text_of(&drawn(app, width, height, now))
}

fn drawn(app: &App, width: u16, height: u16, now: Instant) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| {
            draw(frame, app, now);
        })
        .expect("draw");
    terminal
}

fn text_of(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

/// The cell where `needle` starts on screen, for text one column per character.
fn find(terminal: &Terminal<TestBackend>, needle: &str) -> Option<(u16, u16)> {
    text_of(terminal).lines().enumerate().find_map(|(y, line)| {
        let start = line.find(needle)?;
        let x = line[..start].chars().count();
        Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?))
    })
}

fn style_at(terminal: &Terminal<TestBackend>, (x, y): (u16, u16)) -> Style {
    terminal.backend().buffer()[(x, y)].style()
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code), Instant::now());
}

fn command(app: &mut App, line: &str) {
    press(app, KeyCode::Char(':'));
    for ch in line.chars() {
        press(app, KeyCode::Char(ch));
    }
    press(app, KeyCode::Enter);
}

fn type_keys(app: &mut App, text: &str) {
    for ch in text.chars() {
        press(app, KeyCode::Char(ch));
    }
}

/// Types the next `count` characters of the session correctly.
fn type_prefix(app: &mut App, count: usize) {
    for _ in 0..count {
        type_next(app, Instant::now());
    }
}

/**
 * Types the whole text correctly, its first character now and the others
 * `duration` later. Returns when the text ended.
 */
fn type_whole_text(app: &mut App, duration: Duration) -> Instant {
    let start = Instant::now();
    type_next(app, start);
    let end = start + duration;
    type_remaining_at(app, end);
    end
}

fn mistake(app: &mut App) {
    let expected = app
        .session_view()
        .map(|view| view.session.target()[view.session.cursor()].clone());
    let wrong = if expected.as_deref() == Some("x") {
        'y'
    } else {
        'x'
    };
    press(app, KeyCode::Char(wrong));
}

fn cursor_of(app: &App, width: u16, height: u16) -> (Position, Terminal<TestBackend>) {
    let mut terminal = drawn(app, width, height, Instant::now());
    let position = terminal.get_cursor_position().expect("cursor");
    (position, terminal)
}

/// The line just above the command line.
fn status_line(screen: &str) -> &str {
    let lines: Vec<&str> = screen.lines().collect();
    lines[lines.len() - 2]
}

/// The last column the chart of a screen draws in.
fn chart_end(screen: &str) -> Option<usize> {
    screen
        .lines()
        .filter(|line| line.contains(['┤', '┼']))
        .filter_map(|line| line.trim_end().chars().count().checked_sub(1))
        .max()
}

fn record(wpm: f64) -> Record {
    Record {
        date: chrono::Local::now(),
        mode: "words 50".to_owned(),
        language: "english".to_owned(),
        duration: 42.0,
        wpm,
        raw_wpm: wpm + 4.0,
        accuracy: 97.5,
        errors: 3,
        text_length: 250,
    }
}

fn player(id: u64, name: &str, correct: u32, finish_ms: Option<u64>) -> PlayerView {
    PlayerView {
        id: PlayerId(id),
        name: name.parse().expect("name"),
        ready: true,
        connected: id != 3,
        progress: PlayerProgress {
            typed: correct,
            correct,
            errors: 1,
            wpm: 70.0 + f64::from(correct) / 10.0,
            accuracy: 98.0,
            finish_ms,
        },
    }
}

fn room(phase: Phase) -> RoomView {
    RoomView {
        code: "FK72AD".parse().expect("code"),
        host: PlayerId(1),
        text: TextSource::Quote {
            language: Language::English,
        },
        text_length: 120,
        phase,
        max_players: 8,
        players: vec![
            player(1, "jean", 120, Some(41_200)),
            player(2, "alice", 90, None),
            player(3, "bob", 40, None),
        ],
    }
}

/// An app inside a room, fed with server messages instead of a real server.
fn in_room(phase: Phase) -> App {
    in_room_at(room(phase), Instant::now())
}

fn in_room_at(room: RoomView, now: Instant) -> App {
    in_room_on("ws://127.0.0.1:9", room, now)
}

fn in_room_on(server: &str, room: RoomView, now: Instant) -> App {
    let mut app = app();
    app.config.multiplayer.server = server.to_owned();
    command(&mut app, "join FK72AD");
    app.handle_network(NetworkEvent::Connected(PlayerId(1)), now);
    if room.phase != Phase::Lobby {
        app.handle_network(
            NetworkEvent::Message(ServerMessage::Countdown {
                text: "Simplicity is prerequisite for reliability.".to_owned(),
                duration_ms: 3_000,
            }),
            now,
        );
    }
    app.handle_network(NetworkEvent::Message(ServerMessage::Room(room)), now);
    app
}

/// A room of `size` players in `phase`, the client joined last.
fn full_room(phase: Phase, size: u8, player: impl Fn(u64) -> PlayerView) -> RoomView {
    let others = 2..=u64::from(size);
    RoomView {
        max_players: size,
        players: others.chain([1]).map(player).collect(),
        ..room(phase)
    }
}

fn racer(id: u64) -> String {
    if id == 1 {
        "jean".to_owned()
    } else {
        format!("racer{id}")
    }
}
