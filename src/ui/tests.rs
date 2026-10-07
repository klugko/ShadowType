use std::time::{Duration, Instant};

use code_racer_engine::{Language, TextSource};
use code_racer_protocol::{Phase, PlayerId, PlayerProgress, PlayerView, RoomView, ServerMessage};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::{App, Overrides},
    cli::Launch,
    config::{Config, Theme},
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
        Launch::Home,
    )
}

fn screen(app: &App, width: u16, height: u16) -> String {
    screen_at(app, width, height, Instant::now())
}

fn screen_at(app: &App, width: u16, height: u16, now: Instant) -> String {
    text_of(&drawn(app, width, height, now))
}

/// A terminal of `width` by `height` with the app drawn on it at `now`.
fn drawn(app: &App, width: u16, height: u16, now: Instant) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| draw(frame, app, now)).expect("draw");
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

fn style_at(terminal: &Terminal<TestBackend>, (x, y): (u16, u16)) -> ratatui::style::Style {
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

/// An app inside `room` since `now`, the race text shown past the lobby.
fn in_room_at(room: RoomView, now: Instant) -> App {
    let mut app = app();
    app.config.multiplayer.server = "ws://127.0.0.1:9".to_owned();
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

/// The status line of a screen drawn by [`screen`].
fn status_line(screen: &str) -> &str {
    let lines: Vec<&str> = screen.lines().collect();
    lines[lines.len() - 2]
}

#[test]
fn home_looks_like_an_editor() {
    let text = screen(&app(), 100, 30);
    for expected in [
        "EXPLORER",
        "practice.toml",
        "race.toml",
        "history.log",
        "NORMAL",
        "mode",
        "words",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[test]
fn small_terminals_get_a_message_instead_of_a_broken_layout() {
    let app = app();
    let text = screen(&app, MIN_WIDTH - 1, MIN_HEIGHT);
    assert!(text.contains("Terminal too small."), "{text}");
    assert!(text.contains("80x20"));
    for (width, height) in [(1, 1), (10, 4), (MIN_WIDTH, MIN_HEIGHT - 1)] {
        screen(&app, width, height);
    }
    assert!(!screen(&app, MIN_WIDTH, MIN_HEIGHT).contains("too small"));
}

#[test]
fn the_smallest_size_gives_the_explorer_columns_to_whole_buffer_lines() {
    let mut app = app();
    for wpm in [72.0, 81.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let home = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    assert!(!home.contains("EXPLORER"), "{home}");
    assert!(
        home.contains("# capitals, commas, quotes, full stops"),
        "{home}"
    );
    for (page, line) in [
        ("history", "acc  err"),
        ("help", "edit a text value, Enter saves, Esc cancels"),
        ("config", "# e.g. ws://192.168.1.42:8080"),
    ] {
        command(&mut app, page);
        let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        assert!(text.contains(line), "{page}:\n{text}");
    }
}

#[tokio::test]
async fn the_lobby_shows_the_whole_server_address_at_the_smallest_size() {
    let mut app = in_room(Phase::Lobby);
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    for expected in [
        "# invite: code-racer join FK72AD --server ws://127.0.0.1:9",
        "server  = \"ws://127.0.0.1:9\"",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[test]
fn every_buffer_renders_in_every_theme_and_size() {
    let mut app = app();
    app.history.add(record(72.0)).expect("in memory");
    app.history.add(record(81.0)).expect("in memory");
    for theme in Theme::ALL {
        app.config.theme = theme;
        for page in [
            "practice",
            "race",
            "history",
            "config",
            "help",
            "words 25",
            "code rust",
        ] {
            command(&mut app, page);
            for (width, height) in [(80, 20), (120, 40), (200, 60)] {
                for sidebar in [true, false] {
                    app.sidebar = sidebar;
                    screen(&app, width, height);
                }
            }
            press(&mut app, KeyCode::Esc);
        }
    }
}

#[test]
fn typing_screen_shows_insert_mode_and_live_statistics() {
    let mut app = app();
    command(&mut app, "words 10");
    press(&mut app, KeyCode::Char('x'));
    let text = screen(&app, 100, 24);
    for expected in ["INSERT", "wpm", "error", "notes.md", "words 10"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

/// Types a character other than the one the session expects next.
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

#[test]
fn the_status_line_keeps_its_statistics_at_the_smallest_size() {
    let mut app = app();
    command(&mut app, "set punctuation");
    command(&mut app, "set numbers");
    command(&mut app, "words 100");
    mistake(&mut app);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let status = status_line(&text);
    for expected in ["INSERT", "notes.md", "words 100", " 1 error ", " 00:00 "] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
    assert!(status.trim_end().ends_with(" 0%"), "{status}");
}

#[tokio::test]
async fn the_race_status_line_keeps_its_statistics_at_the_smallest_size() {
    let app = in_room(Phase::Racing);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let status = status_line(&text);
    for expected in ["room FK72AD", " 0 errors ", " 00:00 "] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
    assert!(status.trim_end().ends_with(" 0%"), "{status}");
}

#[test]
fn blocked_typing_is_shown_in_the_status_line_in_the_error_colour() {
    let mut app = app();
    command(&mut app, "words 10");
    for _ in 0..code_racer_engine::ERROR_RUN_LIMIT {
        mistake(&mut app);
    }
    assert!(app.is_typing_blocked());
    let terminal = drawn(&app, MIN_WIDTH, MIN_HEIGHT, Instant::now());
    let status = MIN_HEIGHT - 2;
    let at = find(&terminal, "fix the mistake").expect("the blocked state");
    assert_eq!(at.1, status, "{}", text_of(&terminal));
    let palette = Palette::of(Theme::Editor);
    assert_eq!(style_at(&terminal, at).bg, Some(palette.error));
    assert!(status_line(&text_of(&terminal)).contains(" 10 errors "));
}

#[test]
fn solo_results_show_the_metrics() {
    let mut app = app();
    command(&mut app, "quote");
    let now = Instant::now();
    while let Some(view) = app.session_view() {
        let session = view.session;
        if session.is_finished() {
            break;
        }
        let next = session.target()[session.cursor()].clone();
        for ch in next.chars() {
            app.handle_key(
                KeyEvent::from(KeyCode::Char(ch)),
                now + Duration::from_secs(20),
            );
        }
    }
    let text = screen(&app, 100, 30);
    for expected in [
        "session complete",
        "wpm",
        "accuracy",
        "consistency",
        "personal best",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[test]
fn history_shows_records_and_a_chart() {
    let mut app = app();
    for wpm in [60.0, 64.0, 70.0, 68.0, 75.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    command(&mut app, "history");
    let text = screen(&app, 120, 40);
    for expected in ["5 sessions", "best", "75", "words 50", "┤"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[test]
fn the_oldest_session_can_be_scrolled_into_view() {
    let mut app = app();
    app.history
        .add(Record {
            mode: "words 10".to_owned(),
            ..record(50.0)
        })
        .expect("in memory");
    for wpm in [60.0, 70.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    app.resize(80, 20);
    command(&mut app, "history");
    press(&mut app, KeyCode::Char('G'));
    let text = screen(&app, 80, 20);
    let last_editor_row = text.lines().nth(17).unwrap_or_default();
    assert!(last_editor_row.contains("words 10"), "{text}");
}

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
}

#[tokio::test]
async fn lobby_lists_players_and_invite_command() {
    let app = in_room(Phase::Lobby);
    let text = screen(&app, 120, 30);
    for expected in [
        "room FK72AD",
        "code-racer join FK72AD",
        "alice",
        "host",
        "ready",
        "## players",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[tokio::test]
async fn race_shows_the_text_and_live_standings() {
    let app = in_room(Phase::Racing);
    let text = screen(&app, 120, 30);
    for expected in [
        "PLAYERS",
        "Simplicity",
        "jean",
        "alice",
        "offline",
        "━",
        "wpm",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[tokio::test]
async fn race_results_rank_players() {
    let app = in_room(Phase::Finished);
    let text = screen(&app, 120, 30);
    for expected in ["results", "0:41.2", "DNF", "you finished 1st"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[tokio::test]
async fn the_status_line_stops_when_the_race_ends_before_the_player_finishes() {
    let start = Instant::now();
    let mut app = in_room_at(room(Phase::Racing), start);
    for ch in "Simplicity".chars() {
        app.handle_key(
            KeyEvent::from(KeyCode::Char(ch)),
            start + Duration::from_secs(4),
        );
    }
    let end = start + Duration::from_secs(12);
    let mut over = room(Phase::Finished);
    over.players[0].progress.finish_ms = None;
    app.handle_network(NetworkEvent::Message(ServerMessage::Room(over)), end);

    let at_the_end = screen_at(&app, 120, 30, end);
    let later = screen_at(&app, 120, 30, end + Duration::from_secs(45));
    assert!(later.contains("DNF"), "{later}");
    assert!(status_line(&later).contains(" 00:12 "), "{later}");
    assert_eq!(status_line(&later), status_line(&at_the_end));
}

/// Types the next `count` characters of the session correctly.
fn type_prefix(app: &mut App, count: usize) {
    for _ in 0..count {
        let Some(view) = app.session_view() else {
            return;
        };
        let next = view.session.target()[view.session.cursor()].clone();
        let code = if next == "\n" {
            KeyCode::Enter
        } else {
            KeyCode::Char(next.chars().next().unwrap_or(' '))
        };
        press(app, code);
    }
}

/// Sizes the preview draws every screen at: the smallest supported and a
/// large one.
const PREVIEW_SIZES: [(u16, u16); 2] = [(MIN_WIDTH, MIN_HEIGHT), (200, 60)];

/// `app` drawn at every preview size, resized first as by the terminal.
fn shots(name: &str, app: &mut App) -> Vec<(String, String)> {
    PREVIEW_SIZES
        .iter()
        .map(|&(width, height)| {
            app.resize(width, height);
            (
                format!("{name} {width}x{height}"),
                screen(app, width, height),
            )
        })
        .collect()
}

/// Prints every screen; run with `cargo test preview_screens -- --ignored --nocapture`
/// to review the interface without a terminal.
#[tokio::test]
#[ignore = "visual preview, not an assertion"]
async fn preview_screens() {
    let mut app = app();
    for wpm in [58.0, 61.0, 66.0, 64.0, 70.0, 73.0, 71.0, 78.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    let mut screens = shots("home", &mut app);
    for page in ["race", "config", "history", "help"] {
        command(&mut app, page);
        screens.extend(shots(page, &mut app));
    }
    command(&mut app, "code rust");
    type_prefix(&mut app, 140);
    screens.extend(shots("code", &mut app));
    press(&mut app, KeyCode::Esc);
    screens.extend(shots("leaving", &mut app));
    press(&mut app, KeyCode::Esc);
    command(&mut app, "words 50");
    type_prefix(&mut app, 60);
    press(&mut app, KeyCode::Char('x'));
    screens.extend(shots("words", &mut app));
    for phase in [Phase::Lobby, Phase::Racing, Phase::Finished] {
        screens.extend(shots(&format!("{phase:?}"), &mut in_room(phase)));
    }
    for (name, text) in screens {
        println!("──── {name}\n{text}");
    }
}
