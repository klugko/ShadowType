use std::time::{Duration, Instant};

use code_racer_engine::{Language, TextSource};
use code_racer_protocol::{
    MAX_ROOM_PLAYERS, Phase, PlayerId, PlayerProgress, PlayerView, RoomView, ServerMessage,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Position,
    style::{Color, Style},
};

use super::*;
use crate::{
    app::{
        App, Overrides,
        test_support::{type_next, type_remaining_at},
    },
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

/// A terminal of `width` by `height` with the app drawn on it at `now`.
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
    in_room_on("ws://127.0.0.1:9", room, now)
}

/// An app inside `room` of the race server at `server` since `now`.
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
    assert!(home.contains("# 10 · 25 · 50 · 100"), "{home}");
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
    let mut app = in_room_on("ws://10.0.0.9:8080", room(Phase::Lobby), Instant::now());
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    for expected in [
        "# invite: code-racer join FK72AD --server ws://10.0.0.9:8080",
        "server  = \"ws://10.0.0.9:8080\"",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    assert!(!text.contains("--host 0.0.0.0"), "{text}");
}

#[tokio::test]
async fn the_lobby_never_invites_teammates_to_their_own_computer() {
    let mut app = in_room(Phase::Lobby);
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let invite = text
        .lines()
        .find(|line| line.contains("# invite: code-racer join FK72AD --server ws://"))
        .unwrap_or_else(|| panic!("no invite:\n{text}"));
    assert!(!invite.contains("127.0.0.1"), "{invite}");
    assert!(invite.trim_end().ends_with(":9"), "{invite}");
    assert!(
        text.contains("# start the server with --host 0.0.0.0 for teammates to reach it"),
        "{text}"
    );
    assert!(text.contains("server  = \"ws://127.0.0.1:9\""), "{text}");
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
    mistake(&mut app);
    let text = screen(&app, 100, 24);
    for expected in ["INSERT", "notes.md", "words 10", "-- INSERT --"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    let status = status_line(&text);
    for expected in [" - wpm ", " 0% ", " 1 error ", " 00:00 "] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
}

/// The styles of a character typed right, one typed wrong, the cursor and a
/// character still to type, in that order on the cursor line, in `theme`.
fn typing_cells(theme: Theme) -> [Style; 4] {
    let mut app = app();
    app.config.theme = theme;
    app.config.animations = false;
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    command(&mut app, "words 10");
    type_prefix(&mut app, 1);
    mistake(&mut app);
    let terminal = drawn(&app, MIN_WIDTH, MIN_HEIGHT, Instant::now());
    let first_row = 1;
    let text = 5;
    [0, 1, 2, 3].map(|offset| style_at(&terminal, (text + offset, first_row)))
}

#[test]
fn typed_text_shows_right_wrong_cursor_and_pending_characters_apart() {
    let palette = Palette::of(Theme::Editor);
    let [right, wrong, cursor, pending] = typing_cells(Theme::Editor);
    assert_eq!(right.fg, Some(palette.strong), "typed right");
    assert_eq!(wrong.fg, Some(palette.error), "typed wrong");
    assert!(wrong.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(
        (cursor.fg, cursor.bg),
        (Some(palette.on_accent), Some(palette.accent)),
        "the cursor"
    );
    assert_eq!(pending.fg, palette.pending.fg, "still to type");
    assert_ne!(pending.fg, right.fg);
    assert_eq!(pending.bg, Some(palette.highlight), "on the cursor line");
}

#[test]
fn the_mono_theme_shows_typed_text_apart_without_colour() {
    let cells = typing_cells(Theme::Mono);
    let [right, wrong, cursor, pending] = cells;
    for style in cells {
        for colour in [style.fg, style.bg] {
            assert!(matches!(colour, None | Some(Color::Reset)), "{style:?}");
        }
    }
    assert!(
        cursor.add_modifier.contains(Modifier::REVERSED),
        "{cursor:?}"
    );
    assert!(
        !wrong.add_modifier.contains(Modifier::REVERSED),
        "a mistake never looks like the cursor: {wrong:?}"
    );
    assert!(
        wrong.add_modifier.contains(Modifier::UNDERLINED),
        "{wrong:?}"
    );
    assert!(pending.add_modifier.contains(Modifier::DIM), "{pending:?}");
    assert!(
        !pending.add_modifier.contains(Modifier::BOLD),
        "no bold, which some terminals let win over dim: {pending:?}"
    );
    assert!(!right.add_modifier.contains(Modifier::DIM), "{right:?}");
}

#[test]
fn the_typing_view_scrolls_to_keep_context_above_the_cursor() {
    let directory = crate::persist::scratch::TempDir::new();
    let path = directory.join("lines.txt");
    let lines: Vec<String> = (1..=40).map(|line| format!("line {line:02}")).collect();
    std::fs::write(&path, lines.join("\n")).expect("write");
    let mut app = app();
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    command(&mut app, &format!("e {}", path.display()));
    type_prefix(&mut app, 25 * "line 01\n".len());

    let terminal = drawn(&app, MIN_WIDTH, MIN_HEIGHT, Instant::now());
    let text = text_of(&terminal);
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[1].starts_with(" 24  line 24"), "{text}");
    assert!(
        rows[3].starts_with(" 26  line 26"),
        "two lines of context:\n{text}"
    );
    let palette = Palette::of(Theme::Editor);
    assert_eq!(style_at(&terminal, (5, 3)).bg, Some(palette.accent));
    assert_eq!(style_at(&terminal, (1, 3)).fg, Some(palette.strong));
    assert_eq!(style_at(&terminal, (1, 2)).fg, Some(palette.faint));
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
    app.history
        .add(Record {
            mode: "quote".to_owned(),
            ..record(10.0)
        })
        .expect("in memory");
    command(&mut app, "quote");
    let length = app.session_view().expect("a quote").session.target().len();
    let end = type_whole_text(&mut app, Duration::from_secs(20));

    let text = screen_at(&app, 100, 30, end + Duration::from_secs(30));
    let wpm = code_racer_engine::words_per_minute(length, Duration::from_secs(20));
    for expected in [
        "session complete".to_owned(),
        format!("wpm         = {wpm:.1}"),
        "accuracy    = 100.0%".to_owned(),
        "errors      = 0".to_owned(),
        "time        = 0:20.0".to_owned(),
        "new personal best, previous 10.0 wpm".to_owned(),
        "## wpm over time".to_owned(),
    ] {
        assert!(text.contains(&expected), "missing {expected}:\n{text}");
    }
    assert_eq!(chart_end(&text), Some(97), "the chart spans the text");
    let status = status_line(&text);
    assert!(status.contains(&format!(" {wpm:.0} wpm ")), "{status}");
    assert!(
        status.contains(" 00:20 ") && status.ends_with(" 100% "),
        "{status}"
    );
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
    assert_eq!(chart_end(&text), Some(117), "two columns short of the edge");
}

/// The last column the chart of a screen draws in.
fn chart_end(screen: &str) -> Option<usize> {
    screen
        .lines()
        .filter(|line| line.contains(['┤', '┼']))
        .filter_map(|line| line.trim_end().chars().count().checked_sub(1))
        .max()
}

#[test]
fn history_draws_every_line_of_its_layout_and_no_other() {
    use crate::app::history_log::{CHART_SESSIONS, lines};
    for sessions in [0, 1, 2, CHART_SESSIONS + 1] {
        let mut app = app();
        for _ in 0..sessions {
            app.history.add(record(70.0)).expect("in memory");
        }
        command(&mut app, "history");
        let count = lines(sessions).len();
        let height = u16::try_from(count + 4)
            .expect("a small screen")
            .max(MIN_HEIGHT);
        let text = screen(&app, 120, height);
        let numbered = |number: usize| {
            text.lines().any(|line| {
                line.split('│')
                    .nth(1)
                    .is_some_and(|editor| editor.trim_start().starts_with(&format!("{number}  ")))
            })
        };
        assert!(
            numbered(count),
            "line {count} of {sessions} sessions:\n{text}"
        );
        assert!(!numbered(count + 1), "{sessions} sessions:\n{text}");
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

/// Relative luminance of a true colour, as WCAG defines it.
fn luminance(color: Color) -> Option<f64> {
    let Color::Rgb(red, green, blue) = color else {
        return None;
    };
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue))
}

/// WCAG contrast ratio of two true colours, from 1 to 21.
fn contrast(first: Color, second: Color) -> Option<f64> {
    let (first, second) = (luminance(first)?, luminance(second)?);
    Some((first.max(second) + 0.05) / (first.min(second) + 0.05))
}

#[test]
fn command_line_hints_are_readable_in_every_theme() {
    let mut app = app();
    for theme in Theme::ALL {
        app.config.theme = theme;
        let palette = Palette::of(theme);
        let terminal = drawn(&app, 100, 24, Instant::now());
        let hints = 23;
        let (key, _) = find(&terminal, "j/k move").expect("the hints");
        let key_style = style_at(&terminal, (key, hints));
        let action_style = style_at(&terminal, (key + 4, hints));
        let action = action_style.fg.expect("a colour");
        assert_eq!(key_style.fg, Some(palette.strong), "{theme}");
        assert!(key_style.add_modifier.contains(Modifier::BOLD), "{theme}");
        assert!(
            !action_style.add_modifier.contains(Modifier::DIM),
            "{theme}"
        );
        if !palette.mono {
            assert_ne!(action, palette.faint, "{theme}");
            assert_ne!(action, palette.muted, "{theme}");
        }
        if let Some(ratio) = contrast(action, palette.background) {
            assert!(ratio >= 4.5, "{theme}: {ratio:.2}");
        }
    }
}

#[test]
fn text_still_to_type_is_readable_on_the_cursor_line() {
    for theme in [Theme::Editor, Theme::VsCode] {
        let palette = Palette::of(theme);
        let pending = palette.pending.fg.expect("a colour");
        let line = palette.cursorline.bg.expect("a current line");
        for (place, background) in [("cursor line", line), ("buffer", palette.background)] {
            let ratio = contrast(pending, background).expect("true colours");
            assert!(ratio >= 4.5, "{theme} {place}: {ratio:.2}");
        }
        let typed = contrast(palette.strong, pending).expect("true colours");
        assert!(
            typed >= 2.0,
            "{theme}: typed text stands out from the rest: {typed:.2}"
        );
    }
}

#[test]
fn code_still_to_type_stays_readable_and_apart_from_typed_code() {
    use crate::ui::syntax::Token;
    let tokens = [
        Token::Keyword,
        Token::Type,
        Token::Function,
        Token::String,
        Token::Number,
        Token::Punctuation,
        Token::Plain,
    ];
    for theme in [Theme::Editor, Theme::VsCode] {
        let palette = Palette::of(theme);
        let ghost = palette.ghost.expect("syntax colours");
        let line = palette.cursorline.bg.expect("a current line");
        for token in tokens {
            let (dim, lit) = (ghost.of(token), palette.lit.of(token));
            let readable = contrast(dim, line).expect("true colours");
            assert!(readable >= 3.0, "{theme} {token:?}: {readable:.2}");
            let apart = contrast(lit, dim).expect("true colours");
            assert!(apart >= 1.6, "{theme} {token:?}: {apart:.2}");
        }
    }
}

#[test]
fn the_vscode_theme_has_a_blue_status_bar_and_no_tildes() {
    let mut app = app();
    app.config.theme = Theme::VsCode;
    let terminal = drawn(&app, 120, 30, Instant::now());
    let palette = Palette::of(Theme::VsCode);
    let status = 28;
    for column in [0, 60, 119] {
        let style = style_at(&terminal, (column, status));
        assert!(style.bg.is_some(), "column {column}: {style:?}");
    }
    assert_eq!(
        style_at(&terminal, (60, status)).bg,
        palette.status.bg,
        "the bar between its items"
    );
    assert_eq!(palette.status.bg, Some(Color::Rgb(0, 122, 204)));
    assert!(!text_of(&terminal).contains('~'), "{}", text_of(&terminal));
    let ratio = contrast(Color::Rgb(255, 255, 255), Color::Rgb(0, 122, 204)).expect("rgb");
    assert!(ratio >= 4.5, "white on the bar: {ratio:.2}");
}

#[test]
fn files_have_icons_in_the_explorer_and_the_tabs() {
    use crate::config::Icons;
    let mut app = app();
    command(&mut app, "set look=commit");
    command(&mut app, "words 10");
    type_prefix(&mut app, 1);
    let text = screen(&app, 120, 30);
    for expected in [
        "§ practice.toml",
        "¶ help.md",
        "≡ history.log",
        "± COMMIT_EDITMSG",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    let tab = text.lines().next().unwrap_or_default();
    assert!(tab.contains("± COMMIT_EDITMSG ●"), "{tab}");
    app.config.icons = Icons::Nerd;
    let text = screen(&app, 120, 30);
    assert!(text.contains("\u{e615} practice.toml"), "{text}");
    assert!(text.contains("\u{f07c} code-racer"), "{text}");
    app.config.icons = Icons::None;
    let text = screen(&app, 120, 30);
    assert!(text.contains("     practice.toml"), "{text}");
}

#[test]
fn mono_tells_every_typing_state_apart_without_colour() {
    let palette = Palette::of(Theme::Mono);
    let styles = [
        ("cursor", palette.cursor),
        ("mistake", palette.mistake),
        ("pending", palette.pending),
        ("selection", palette.selection),
    ];
    for (name, style) in styles {
        assert_eq!(style.fg, None, "{name} uses no colour");
        assert_eq!(style.bg, None, "{name} uses no colour");
    }
    for (index, (name, style)) in styles.iter().enumerate() {
        for (other, other_style) in &styles[index + 1..] {
            assert_ne!(
                style.add_modifier, other_style.add_modifier,
                "{name} and {other} look the same"
            );
        }
    }
    assert!(!palette.mistake.add_modifier.contains(Modifier::REVERSED));
    assert!(palette.cursor.add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn the_dark_theme_draws_on_black_with_visible_selections() {
    let mut app = app();
    app.config.theme = Theme::Dark;
    let palette = Palette::of(Theme::Dark);
    let terminal = drawn(&app, 120, 30, Instant::now());
    let blank = find(&terminal, "~").expect("an empty line");
    assert_eq!(style_at(&terminal, blank).bg, Some(Color::Black));
    let entry = find(&terminal, "   § practice.toml").expect("the explorer entry");
    let selected = style_at(&terminal, (entry.0 + 5, entry.1));
    assert_eq!(selected.bg, Some(palette.highlight));
    assert_ne!(palette.highlight, palette.background);

    press(&mut app, KeyCode::Enter);
    let terminal = drawn(&app, 120, 30, Instant::now());
    let line = find(&terminal, "mode ").expect("the selected line");
    let style = style_at(&terminal, line);
    assert!(style.add_modifier.contains(Modifier::BOLD), "{style:?}");
    assert_eq!(
        style.bg,
        Some(Color::Black),
        "no grey that hides ghost text"
    );
}

#[test]
fn the_mono_theme_shows_which_side_has_the_focus_without_colour() {
    let mut app = app();
    app.config.theme = Theme::Mono;
    let selected_entry = |app: &App| {
        let terminal = drawn(app, 120, 30, Instant::now());
        let entry = find(&terminal, "   § practice.toml").expect("the explorer entry");
        style_at(&terminal, (entry.0 + 5, entry.1))
    };
    let in_explorer = selected_entry(&app);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, crate::app::Focus::Editor);
    let in_editor = selected_entry(&app);
    assert!(
        in_explorer.add_modifier.contains(Modifier::REVERSED),
        "{in_explorer:?}"
    );
    assert_ne!(in_explorer, in_editor);
}

/// Where the terminal cursor is left once `app` is drawn.
fn cursor_of(app: &App, width: u16, height: u16) -> (Position, Terminal<TestBackend>) {
    let mut terminal = drawn(app, width, height, Instant::now());
    let position = terminal.get_cursor_position().expect("cursor");
    (position, terminal)
}

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
    for ch in "FK7".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    let (cursor, terminal) = cursor_of(&app, MIN_WIDTH, MIN_HEIGHT);
    let (x, y) = find(&terminal, "room        = \"FK7\"").expect("the line");
    assert_eq!(cursor, Position::new(x + 18, y), "after \"FK7\"");
}

/// Types `text` key by key.
fn type_keys(app: &mut App, text: &str) {
    for ch in text.chars() {
        press(app, KeyCode::Char(ch));
    }
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

#[test]
fn a_value_longer_than_its_field_scrolls_to_keep_the_cursor_in_view() {
    let mut app = app();
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let host = "a-very-long-host-name".repeat(4);
    type_keys(&mut app, &format!("ws://{host}.example.com:8080"));
    let (cursor, terminal) = cursor_of(&app, MIN_WIDTH, MIN_HEIGHT);
    let text = text_of(&terminal);
    let (x, y) = find(&terminal, "example.com:8080\"").expect("the end of the value");
    assert_eq!(cursor, Position::new(x + 16, y), "just after it:\n{text}");
    let row = text.lines().nth(usize::from(y)).unwrap_or_default();
    assert!(row.contains("server     = \"…"), "the start is cut: {row}");
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
        "▾ session",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    let status = status_line(&text);
    assert!(status.trim_end().ends_with(" markdown"), "{status}");
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
async fn the_standings_always_show_the_player_even_last_of_a_full_room() {
    let mut full = room(Phase::Racing);
    full.players = (1..=8_u32)
        .map(|n| player(n.into(), &format!("racer{n}"), 10 * n, None))
        .collect();
    let mut app = in_room_at(full, Instant::now());
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let standings: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.contains("PLAYERS"))
        .skip(1)
        .take_while(|line| !line.contains("INSERT"))
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert!(
        standings.len() < 8,
        "the panel cannot show everyone:\n{text}"
    );
    let last = standings.last().expect("standings");
    assert!(
        last.trim_start().starts_with("8 racer1"),
        "own row with its real place:\n{text}"
    );
    assert!(standings[0].trim_start().starts_with("1 racer8"), "{text}");
}

/// A room of `size` players in `phase`, the client joined last, made by
/// `player` from each id.
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

#[tokio::test]
async fn a_full_lobby_keeps_the_players_row_and_what_to_do_next_in_view() {
    for size in [8, MAX_ROOM_PLAYERS] {
        let lobby = full_room(Phase::Lobby, size, |id| PlayerView {
            ready: id != 1,
            ..player(id, &racer(id), 0, None)
        });
        let mut app = in_room_at(lobby, Instant::now());
        app.resize(MIN_WIDTH, MIN_HEIGHT);
        let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        for expected in [
            "· jean            host, you   not ready",
            "more",
            "# waiting for 1 player to get ready",
            "r  toggle ready",
            "s  start the race",
        ] {
            assert!(
                text.contains(expected),
                "{size}: missing {expected}:\n{text}"
            );
        }
    }
}

#[tokio::test]
async fn full_results_keep_the_players_place_and_what_to_do_next_in_view() {
    let results = full_room(Phase::Finished, MAX_ROOM_PLAYERS, |id| {
        let time = if id == 1 { 90_000 } else { 40_000 + id * 100 };
        player(id, &racer(id), 120, Some(time))
    });
    let mut app = in_room_at(results, Instant::now());
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    for expected in [
        " 32  jean ",
        "more",
        "you finished 32nd of 32",
        "r  back to the lobby",
        "Esc  leave",
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
        type_next(app, Instant::now());
    }
}

/// Types the whole text correctly, its first character now and the others
/// `duration` later. Returns when the text ended.
fn type_whole_text(app: &mut App, duration: Duration) -> Instant {
    let start = Instant::now();
    type_next(app, start);
    let end = start + duration;
    type_remaining_at(app, end);
    end
}

/// Where the things a click acts on lie once `app` is drawn at `now`.
fn hits_of(app: &App, width: u16, height: u16, now: Instant) -> crate::app::mouse::Hits {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    let mut hits = crate::app::mouse::Hits::default();
    terminal
        .draw(|frame| {
            hits = draw(frame, app, now);
        })
        .expect("draw");
    hits
}

#[test]
fn what_the_screen_shows_is_where_a_click_lands() {
    use crate::app::{Buffer, mouse::Target};
    let mut app = app();
    app.history.add(record(70.0)).expect("in memory");
    command(&mut app, "words 10");
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let now = end + Duration::from_secs(5);
    app.handle_key(KeyEvent::from(KeyCode::Char('e')), now);
    assert_eq!(
        app.buffer,
        Buffer::Practice,
        "the results open the settings"
    );
    let terminal = drawn(&app, 120, 30, now);
    let hits = hits_of(&app, 120, 30, now);
    let (x, y) = find(&terminal, "history.log").expect("the entry");
    assert_eq!(hits.at(x, y), Some(Target::Entry(Buffer::History)));
    let (x, y) = find(&terminal, "notes.md").expect("the session tab");
    assert_eq!(hits.at(x, y), Some(Target::Tab(Buffer::Session)));
    let (_, folder) = find(&terminal, "▾ session").expect("the session folder");
    assert_eq!(hits.at(5, folder + 1), Some(Target::Entry(Buffer::Session)));
    let (x, y) = find(&terminal, "language    =").expect("a form line");
    assert_eq!(hits.at(x, y), Some(Target::FormLine(1)));
    let (x, y) = find(&terminal, " NORMAL ").expect("the mode");
    assert_eq!(hits.at(x, y), Some(Target::Mode));
    let (x, y) = find(&terminal, "~").expect("the end of the buffer");
    assert_eq!(hits.at(x, y), Some(Target::Editor));
}

#[test]
fn the_keys_under_the_results_can_be_clicked() {
    use crate::app::mouse::Target;
    let mut app = app();
    command(&mut app, "words 10");
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let later = end + Duration::from_secs(5);
    let terminal = drawn(&app, 100, 30, later);
    let hits = hits_of(&app, 100, 30, later);
    let (x, y) = find(&terminal, "r  new text").expect("the keys");
    assert_eq!(
        hits.at(x + 3, y),
        Some(Target::Key {
            code: KeyCode::Char('r'),
            times: 1
        })
    );
}

#[test]
fn the_command_palette_finds_and_runs_a_command() {
    let mut app = app();
    app.handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let text = screen(&app, 100, 30);
    for expected in ["commands", "Start a session", "COMMAND", "Enter run"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    type_keys(&mut app, "hist");
    let text = screen(&app, 100, 30);
    assert!(text.contains("Open history.log"), "{text}");
    assert!(!text.contains("Start a session"), "{text}");
    press(&mut app, KeyCode::Enter);
    assert!(app.palette.is_none());
    assert_eq!(app.buffer, crate::app::Buffer::History);
}

#[test]
fn f1_opens_the_palette_even_while_typing_and_esc_closes_it() {
    let mut app = app();
    command(&mut app, "words 10");
    type_prefix(&mut app, 2);
    press(&mut app, KeyCode::F(1));
    assert!(app.palette.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.palette.is_none());
    assert!(app.is_typing(), "Esc closed the palette, not the session");
}

#[test]
fn the_mascot_lives_at_the_bottom_of_the_explorer_when_there_is_room() {
    let mut app = app();
    app.resize(120, 30);
    let text = screen(&app, 120, 30);
    let sidebar: Vec<String> = text
        .lines()
        .map(|line| line.chars().take(usize::from(SIDEBAR_WIDTH)).collect())
        .collect();
    let body = sidebar.len() - 2;
    let ghost = sidebar[..body]
        .iter()
        .rposition(|line| line.contains('█'))
        .expect("the ghost");
    assert!(ghost >= body - 3, "at the bottom:\n{text}");
    assert!(chrome::shows_mascot(&app));
    app.config.mascot = false;
    assert!(!screen(&app, 120, 30).contains('█'));
    app.config.mascot = true;
    app.history.add(record(70.0)).expect("in memory");
    app.resize(120, MIN_HEIGHT);
    assert!(
        !chrome::shows_mascot(&app),
        "no room under the explorer and the records"
    );
    assert!(!screen(&app, 120, MIN_HEIGHT).contains('█'));
}

#[test]
fn nothing_moves_once_the_mascot_is_asleep_or_animations_are_off() {
    let mut app = app();
    app.resize(120, 30);
    let start = Instant::now();
    assert_eq!(frame_period(&app, start), Some(MASCOT_FRAME));
    let napping = start + crate::app::mascot::NAP_AFTER + Duration::from_secs(1);
    assert_eq!(
        frame_period(&app, napping),
        None,
        "asleep, no processor time"
    );
    assert!(screen_at(&app, 120, 30, napping).contains('Z'), "it snores");
    app.config.animations = false;
    assert_eq!(frame_period(&app, start), None);
}

#[test]
fn typing_redraws_often_while_the_ink_dries() {
    let mut app = app();
    command(&mut app, "words 10");
    let now = Instant::now();
    type_next(&mut app, now);
    assert_eq!(frame_period(&app, now), Some(TEXT_FRAME));
}

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

#[test]
fn the_results_name_the_characters_missed() {
    let mut app = app();
    command(&mut app, "words 10");
    mistake(&mut app);
    press(&mut app, KeyCode::Backspace);
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let text = screen_at(&app, 100, 30, end + Duration::from_secs(5));
    assert!(text.contains("missed      = { "), "{text}");
}

#[test]
fn the_results_count_up_when_they_come_on_screen() {
    let mut app = app();
    command(&mut app, "words 10");
    let end = type_whole_text(&mut app, Duration::from_secs(10));
    let early = screen_at(&app, 100, 30, end);
    assert!(early.contains("wpm         = 0.0"), "{early}");
    app.config.animations = false;
    let still = screen_at(&app, 100, 30, end);
    assert!(!still.contains("wpm         = 0.0"), "{still}");
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
    for _ in 0..code_racer_engine::ERROR_RUN_LIMIT {
        press(&mut app, KeyCode::Char('x'));
    }
    screens.extend(shots("blocked", &mut app));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    command(&mut app, "quote");
    type_whole_text(&mut app, Duration::from_secs(20));
    screens.extend(shots("results", &mut app));
    for phase in [Phase::Lobby, Phase::Racing, Phase::Finished] {
        screens.extend(shots(&format!("{phase:?}"), &mut in_room(phase)));
    }
    let mut full = room(Phase::Racing);
    full.players = (1..=8_u32)
        .map(|n| player(n.into(), &format!("racer{n}"), 10 * n, None))
        .collect();
    screens.extend(shots("full room", &mut in_room_at(full, Instant::now())));
    let lobby = full_room(Phase::Lobby, MAX_ROOM_PLAYERS, |id| {
        player(id, &racer(id), 0, None)
    });
    screens.extend(shots("full lobby", &mut in_room_at(lobby, Instant::now())));
    let results = full_room(Phase::Finished, MAX_ROOM_PLAYERS, |id| {
        player(id, &racer(id), 120, Some(40_000 + id * 100))
    });
    screens.extend(shots(
        "full results",
        &mut in_room_at(results, Instant::now()),
    ));
    screens.extend(long_input_shots());
    screens.extend(look_shots());
    screens.extend(vscode_shots());
    for (name, text) in screens {
        println!("──── {name}\n{text}");
    }
}

/// A code session in the VS Code theme.
fn vscode_shots() -> Vec<(String, String)> {
    let mut vscode = app();
    vscode.config.theme = Theme::VsCode;
    command(&mut vscode, "code typescript");
    type_prefix(&mut vscode, 40);
    shots("vscode", &mut vscode)
}

/// The command palette, then a words session in every look.
fn look_shots() -> Vec<(String, String)> {
    let mut app = app();
    for wpm in [58.0, 61.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    app.handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let mut screens = shots("palette", &mut app);
    type_keys(&mut app, "cod");
    screens.extend(shots("palette code", &mut app));
    press(&mut app, KeyCode::Esc);
    for look in crate::config::Look::DISGUISES {
        command(&mut app, &format!("set look={look}"));
        command(&mut app, "words 25");
        type_prefix(&mut app, 70);
        screens.extend(shots(&format!("look {look}"), &mut app));
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Esc);
    }
    screens
}

/// A server address and then a command too long for their line, being typed.
fn long_input_shots() -> Vec<(String, String)> {
    let mut app = app();
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    type_keys(
        &mut app,
        "/a-very-long-path/to/a/race/server/behind/a/proxy",
    );
    let mut screens = shots("long value", &mut app);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char(':'));
    type_keys(&mut app, &format!("e {}", "/a-very-long-path".repeat(6)));
    screens.extend(shots("long command", &mut app));
    screens
}
