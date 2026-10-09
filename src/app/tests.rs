use std::time::{Duration, Instant};

use code_racer_engine::{CodeLanguage, Language, Status};
use code_racer_protocol::Phase;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    test_support::{session, type_remaining_at},
    *,
};
use crate::{
    cli::Launch,
    config::{Config, Mode, Practice, Theme},
    history::History,
    persist::scratch::TempDir,
};

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

#[test]
fn starts_in_the_explorer_on_the_practice_buffer() {
    let app = app();
    assert_eq!(app.focus, Focus::Explorer);
    assert_eq!(app.buffer, Buffer::Practice);
    assert_eq!(app.editor_mode(), EditorMode::Normal);
    assert_eq!(app.entries(), Buffer::FILES.to_vec());
}

#[test]
fn explorer_previews_buffers_and_enter_focuses_the_editor() {
    let mut app = app();
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.buffer, Buffer::Race);
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.buffer, Buffer::Help);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.buffer, Buffer::Help, "stays on the last entry");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Editor);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Explorer);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Editor);
}

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
fn keys_right_after_the_time_runs_out_are_ignored() {
    let mut app = app();
    command(&mut app, "time 15");
    let start = Instant::now();
    press_at(&mut app, KeyCode::Char('x'), start);
    let end = start + Duration::from_secs(16);
    app.tick(end);
    let in_flight = end + Duration::from_millis(50);
    for code in [
        KeyCode::Char('q'),
        KeyCode::Char('e'),
        KeyCode::Char(':'),
        KeyCode::Tab,
        KeyCode::Enter,
    ] {
        press_at(&mut app, code, in_flight);
    }
    assert!(!app.should_quit());
    assert!(app.solo().is_some_and(|run| run.result.is_some()));
    assert_eq!((app.buffer, app.focus), (Buffer::Session, Focus::Editor));
    assert!(app.prompt.is_none());
    press_at(&mut app, KeyCode::Char('r'), end + AFTER_QUIET);
    assert!(
        app.solo().is_some_and(|run| run.result.is_none()),
        "r restarts once the quiet period is over"
    );
}

#[test]
fn a_reflex_enter_after_the_last_character_keeps_the_results() {
    let mut app = app();
    command(&mut app, "code rust");
    let now = Instant::now();
    type_remaining_at(&mut app, now);
    press_at(&mut app, KeyCode::Enter, now + Duration::from_millis(100));
    assert!(app.solo().is_some_and(|run| run.result.is_some()));
    press_with(&mut app, 'c', KeyModifiers::CONTROL);
    assert!(app.should_quit(), "Ctrl+C is never ignored");
}

#[test]
fn keys_right_after_losing_the_connection_keep_the_reason_on_screen() {
    let now = Instant::now();
    let mut app = room::racing(now);
    let settings = app.config.race;
    app.handle_network(
        crate::network::NetworkEvent::Closed {
            reason: "connection lost".to_owned(),
        },
        now,
    );
    let in_flight = now + Duration::from_millis(50);
    for code in [
        KeyCode::Char('s'),
        KeyCode::Char('l'),
        KeyCode::Char('q'),
        KeyCode::Enter,
    ] {
        press_at(&mut app, code, in_flight);
    }
    assert!(!app.should_quit());
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Race);
    assert_eq!(app.config.race, settings);
    assert_eq!(app.message(), Some(&Message::error("connection lost")));
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
fn leaving_a_race_takes_two_escapes_until_the_room_is_back_in_the_lobby() {
    for start in [room::counting_down, room::racing, room::showing_results] {
        let now = Instant::now();
        let mut app = start(now);
        let first = now + AFTER_QUIET;
        press_at(&mut app, KeyCode::Esc, first);
        assert!(app.race().is_some());
        assert_eq!(
            app.message(),
            Some(&Message::info("press Esc again to leave FK72AD"))
        );
        press_at(&mut app, KeyCode::Esc, first + Duration::from_secs(1));
        assert!(app.activity.is_none());
        assert_eq!(app.buffer, Buffer::Race);
        assert_eq!(app.message(), Some(&Message::info("left FK72AD")));
    }
}

#[test]
fn a_lobby_is_left_with_one_escape() {
    let now = Instant::now();
    let mut app = room::joined(now);
    press_at(&mut app, KeyCode::Esc, now);
    assert!(app.activity.is_none());
}

#[test]
fn typing_blocked_by_a_mistake_says_how_to_go_on() {
    let mut app = app();
    command(&mut app, "words 10");
    type_text(&mut app, &"#".repeat(code_racer_engine::ERROR_RUN_LIMIT));
    assert!(app.is_typing_blocked());
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(session(&app).cursor(), code_racer_engine::ERROR_RUN_LIMIT);
    assert_eq!(
        app.message(),
        Some(&Message::info("fix the mistake first: Backspace or Ctrl+W"))
    );
    press(&mut app, KeyCode::Backspace);
    assert!(!app.is_typing_blocked());
    assert_eq!(app.message(), None);
}

#[test]
fn a_message_that_arrives_under_the_command_line_waits_until_it_is_seen() {
    let now = Instant::now();
    let mut app = room::joined(now);
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "hist");
    app.handle_network(
        crate::network::NetworkEvent::Closed {
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
        crate::network::NetworkEvent::Closed {
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

fn released(code: KeyCode) -> KeyEvent {
    KeyEvent {
        kind: crossterm::event::KeyEventKind::Release,
        ..KeyEvent::from(code)
    }
}

#[test]
fn key_releases_are_ignored() {
    let mut app = app();
    app.handle_key(released(KeyCode::Char(':')), Instant::now());
    assert!(app.prompt.is_none(), "a release opens nothing");
    press(&mut app, KeyCode::Char('s'));
    let next = session(&app).target()[0].clone();
    for ch in next.chars() {
        press(&mut app, KeyCode::Char(ch));
        app.handle_key(released(KeyCode::Char(ch)), Instant::now());
    }
    assert_eq!(session(&app).cursor(), 1, "typed once");
}

#[test]
fn releasing_the_ready_key_sends_nothing() {
    use code_racer_protocol::{ClientMessage, PlayerProgress, ServerMessage};
    let now = Instant::now();
    let (mut app, mut sent) = room::joined_over_loopback(now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::JoinRoom { .. })
    ));
    press_at(&mut app, KeyCode::Char('r'), now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::SetReady { .. })
    ));
    let answer = room::view(Phase::Lobby, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(answer), now);
    app.handle_key(
        released(KeyCode::Char('r')),
        now + Duration::from_millis(200),
    );
    assert!(sent.try_recv().is_err(), "the release is not a press");
}

#[test]
fn quitting_in_a_room_leaves_it() {
    use code_racer_protocol::ClientMessage;
    for quit in [KeyCode::Char('q'), KeyCode::Char('c')] {
        let now = Instant::now();
        let (mut app, mut sent) = room::joined_over_loopback(now);
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMessage::JoinRoom { .. })
        ));
        if quit == KeyCode::Char('c') {
            press_with(&mut app, 'c', KeyModifiers::CONTROL);
        } else {
            press(&mut app, quit);
        }
        assert!(app.should_quit());
        let connection = app.finish();
        assert!(connection.is_some(), "the goodbye still has to be sent");
        assert_eq!(sent.try_recv(), Ok(ClientMessage::LeaveRoom));
    }
    assert!(self::app().finish().is_none(), "no room, nothing to send");
}

#[test]
fn control_c_always_quits() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    press_with(&mut app, 'c', KeyModifiers::CONTROL);
    assert!(app.should_quit());
}

#[test]
fn commands_change_settings_and_report_errors() {
    let mut app = app();
    command(&mut app, "words 10");
    assert_eq!(session(&app).target().concat().split(' ').count(), 10);
    press(&mut app, KeyCode::Esc);
    command(&mut app, "set theme=mono");
    assert_eq!(app.config.theme, Theme::Mono);
    command(&mut app, "set punctuation");
    assert!(app.config.practice.punctuation);
    command(&mut app, "lang french");
    assert_eq!(app.config.practice.language, Language::French);
    command(&mut app, "frobnicate");
    assert!(
        app.message()
            .is_some_and(|message| message.is_error() && message.text.starts_with("E492"))
    );
    command(&mut app, "q");
    assert!(app.should_quit());
}

#[test]
fn tab_completes_command_names() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "hi");
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        app.prompt.as_ref().map(|prompt| prompt.input.value()),
        Some("history")
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.buffer, Buffer::History);
    assert!(app.prompt.is_none());
}

#[test]
fn backspace_on_an_empty_command_line_cancels_it() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    assert_eq!(app.editor_mode(), EditorMode::Command);
    press(&mut app, KeyCode::Backspace);
    assert!(app.prompt.is_none());
}

#[test]
fn pasting_is_refused_while_typing() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    app.handle_paste("the whole text", Instant::now());
    assert_eq!(session(&app).cursor(), 0);
    assert!(app.message().is_some_and(Message::is_error));
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
fn lang_with_a_programming_language_only_sets_the_code_language() {
    let mut app = room::joined(Instant::now());
    command(&mut app, "lang python");
    assert!(app.race().is_some(), "still in the room");
    assert_eq!(app.config.practice.code_language, CodeLanguage::Python);
    assert_eq!(app.config.race.code_language, CodeLanguage::Python);
    assert_eq!(
        app.message(),
        Some(&Message::info("code language set to python"))
    );
}

#[test]
fn server_errors_are_shown_and_a_closed_room_is_left() {
    use code_racer_protocol::{ErrorCode, ServerMessage};
    let now = Instant::now();
    let mut app = room::racing(now);
    room::deliver(
        &mut app,
        ServerMessage::error(ErrorCode::InvalidProgress, "progress rejected"),
        now,
    );
    assert!(app.race().is_some(), "a rejected message keeps the race");
    assert_eq!(app.message(), Some(&Message::error("progress rejected")));
    room::deliver(
        &mut app,
        ServerMessage::error(ErrorCode::RoomNotFound, "room FK72AD closed"),
        now,
    );
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Race);
    assert_eq!(
        app.message(),
        Some(&Message::error("progress rejected")),
        "an error stays until a key dismisses it"
    );
    press_at(&mut app, KeyCode::Char('j'), now + AFTER_QUIET);
    assert_eq!(app.message(), Some(&Message::error("room FK72AD closed")));
}

#[test]
fn the_countdown_brings_the_player_back_to_the_race_text() {
    use code_racer_protocol::{PlayerProgress, ServerMessage};
    let now = Instant::now();
    let mut app = room::joined(now);
    press(&mut app, KeyCode::Char('?'));
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "wq");
    room::deliver(
        &mut app,
        ServerMessage::Countdown {
            text: room::TEXT.to_owned(),
            duration_ms: 3_000,
        },
        now,
    );
    assert!(app.prompt.is_none(), "the command line is cancelled");
    assert_eq!((app.buffer, app.focus), (Buffer::Session, Focus::Editor));
    let counting_down = room::view(Phase::Countdown, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(counting_down), now);
    assert!(app.is_typing());
    let start = now + Duration::from_secs(3);
    let racing = room::view(Phase::Racing, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(racing), start);
    type_text_at(&mut app, "Simp", start);
    assert_eq!(session(&app).cursor(), 4);
    assert!(!app.should_quit());
}

#[test]
fn the_countdown_cancels_a_field_being_typed() {
    use code_racer_protocol::ServerMessage;
    let now = Instant::now();
    let mut app = room::joined(now);
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Enter);
    assert!(app.editing.is_some());
    room::deliver(
        &mut app,
        ServerMessage::Countdown {
            text: room::TEXT.to_owned(),
            duration_ms: 3_000,
        },
        now,
    );
    assert!(app.editing.is_none());
    assert_eq!(app.buffer, Buffer::Session);
}

#[test]
fn entering_a_room_cancels_a_field_left_open_while_connecting() {
    use code_racer_protocol::{PlayerProgress, ServerMessage};
    let now = Instant::now();
    let mut app = app();
    command(&mut app, "create");
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Enter);
    assert!(app.editing.is_some());
    app.handle_network(crate::network::NetworkEvent::Connected(room::ME), now);
    let lobby = room::view(Phase::Lobby, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(lobby), now);
    assert_eq!(app.buffer, Buffer::Session);
    assert!(app.editing.is_none());
    assert_eq!(app.editor_mode(), EditorMode::Normal);
}

#[test]
fn a_race_is_recorded_once_as_soon_as_the_server_times_the_finish() {
    use code_racer_protocol::{PlayerProgress, ServerMessage};
    let now = Instant::now();
    let mut app = room::racing(now);
    let typed_at = now + Duration::from_secs(10);
    type_remaining_at(&mut app, typed_at);
    assert!(
        app.history.records().is_empty(),
        "not timed by the server yet"
    );
    let finished = PlayerProgress {
        typed: 27,
        correct: 27,
        errors: 0,
        wpm: 32.4,
        accuracy: 100.0,
        finish_ms: Some(10_000),
    };
    let racing = room::view(Phase::Racing, finished);
    room::deliver(&mut app, ServerMessage::Room(racing), typed_at);
    let records = app.history.records();
    assert_eq!(records.len(), 1, "recorded while others still race");
    assert_eq!(
        (records[0].mode.as_str(), records[0].language.as_str()),
        ("race", "english")
    );
    assert_eq!((records[0].duration, records[0].wpm), (10.0, 32.4));
    assert_eq!(records[0].text_length, room::TEXT.len());
    let over = room::view(Phase::Finished, finished);
    room::deliver(&mut app, ServerMessage::Room(over), typed_at);
    assert_eq!(app.history.records().len(), 1, "never twice");
    press_at(&mut app, KeyCode::Esc, typed_at + AFTER_QUIET);
    assert!(app.race().is_some(), "the results are worth a second Esc");
    press_at(&mut app, KeyCode::Esc, typed_at + AFTER_QUIET);
    assert!(app.activity.is_none());
}

#[test]
fn a_race_where_nothing_was_typed_is_not_recorded() {
    use code_racer_protocol::{PlayerProgress, ServerMessage};
    let now = Instant::now();
    let mut app = room::racing(now);
    let over = room::view(Phase::Finished, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(over), now);
    assert!(app.history.records().is_empty());
    assert!(app.race().is_some(), "the results are shown");
}

#[test]
fn the_clock_of_an_unfinished_race_stops_when_the_race_ends() {
    use code_racer_protocol::{PlayerProgress, ServerMessage};
    let start = Instant::now();
    let mut app = room::racing(start);
    type_text_at(&mut app, "Simp", start + Duration::from_secs(2));
    let end = start + Duration::from_secs(10);
    let over = room::view(Phase::Finished, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(over), end);

    let view = app.session_view().expect("the race text");
    let later = end + Duration::from_secs(50);
    assert_eq!(view.stats(later), view.stats(end), "frozen");
    assert_eq!(view.stats(later).elapsed, Duration::from_secs(10));
    assert_eq!(app.history.records()[0].duration, 10.0);
}

#[test]
fn starting_something_else_in_a_room_is_refused() {
    let directory = TempDir::new();
    let file = directory.join("notes.txt");
    std::fs::write(&file, "some text").expect("write");
    let now = Instant::now();
    let mut app = room::joined(now);
    let practice = app.config.practice;
    let refused = Some(Message::error("leave room FK72AD first with Esc"));
    press(&mut app, KeyCode::Char('?'));
    for key in ['s', 'c'] {
        press(&mut app, KeyCode::Char(key));
        assert!(app.race().is_some(), "{key}");
        assert_eq!(app.message(), refused.as_ref(), "{key}");
    }
    let edit = format!("e {}", file.display());
    for line in [
        "solo",
        "words 10",
        "quote",
        "code rust",
        "create",
        "join ABCDEF",
        &edit,
    ] {
        command(&mut app, line);
        assert!(app.race().is_some(), "{line}");
        assert_eq!(app.message(), refused.as_ref(), "{line}");
    }
    assert_eq!(
        app.config.practice, practice,
        "refused commands change nothing"
    );
    assert_eq!(
        app.room_code, "FK72AD",
        "the room line keeps the room joined"
    );
}

#[test]
fn holding_the_ready_key_sends_one_request() {
    use code_racer_protocol::{ClientMessage, PlayerProgress, ServerMessage};
    use crossterm::event::KeyEventKind;
    let now = Instant::now();
    let (mut app, mut sent) = room::joined_over_loopback(now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::JoinRoom { .. })
    ));
    let ready = KeyEvent::from(KeyCode::Char('r'));
    let held = KeyEvent {
        kind: KeyEventKind::Repeat,
        ..ready
    };
    app.handle_key(ready, now);
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::SetReady { .. })
    ));
    let answer = room::view(Phase::Lobby, PlayerProgress::default());
    room::deliver(&mut app, ServerMessage::Room(answer.clone()), now);
    app.handle_key(held, now);
    app.handle_key(held, now);
    assert!(sent.try_recv().is_err(), "auto-repeat is ignored");
    let mut at = now;
    for _ in 0..20 {
        at += Duration::from_millis(30);
        app.handle_key(ready, at);
        room::deliver(&mut app, ServerMessage::Room(answer.clone()), at);
    }
    assert!(
        sent.try_recv().is_err(),
        "repeats reported as presses are ignored too"
    );
    app.handle_key(ready, at + Duration::from_millis(200));
    assert!(matches!(
        sent.try_recv(),
        Ok(ClientMessage::SetReady { .. })
    ));
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

#[test]
fn altgr_characters_are_typed_but_shortcuts_are_not() {
    let directory = TempDir::new();
    let path = directory.join("snippet.txt");
    std::fs::write(&path, "@a").expect("write");
    let mut app = app();
    command(&mut app, &format!("e {}", path.display()));
    press_with(&mut app, 'a', KeyModifiers::CONTROL);
    assert_eq!(session(&app).cursor(), 0, "Ctrl+A is not text");
    press_with(&mut app, '@', KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(session(&app).cursor(), 1, "AltGr+0 types @ on AZERTY");
}

#[test]
fn altgr_characters_reach_the_command_line_and_fields() {
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    press_with(&mut app, '@', altgr);
    press_with(&mut app, '\\', altgr);
    assert_eq!(
        app.prompt.as_ref().map(|prompt| prompt.input.value()),
        Some("@\\")
    );
    press(&mut app, KeyCode::Esc);
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Char('i'));
    press_with(&mut app, 'u', KeyModifiers::CONTROL);
    press_with(&mut app, '[', altgr);
    assert_eq!(
        app.editing.as_ref().map(|edit| edit.input.value()),
        Some("[")
    );
}

#[test]
fn control_h_erases_one_character_in_the_session() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    type_text(&mut app, "xy");
    press_with(&mut app, 'h', KeyModifiers::CONTROL);
    assert_eq!(session(&app).cursor(), 1);
}

#[test]
fn practice_form_edits_the_settings() {
    let mut app = app();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.config.practice.mode, Mode::Time);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.config.practice.duration, 60);
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    assert!(app.is_typing(), "Enter on the last line starts the session");
}

#[test]
fn invalid_room_codes_are_rejected_before_connecting() {
    let mut app = app();
    press(&mut app, KeyCode::Char('m'));
    assert_eq!(app.buffer, Buffer::Race);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(&app.editing, Some(edit) if edit.field == TextField::RoomCode));
    type_text(&mut app, "AB0");
    press(&mut app, KeyCode::Enter);
    assert!(app.message().is_some_and(Message::is_error));
    assert!(app.activity.is_none());
    press(&mut app, KeyCode::Esc);
    assert!(app.editing.is_none());
}

#[test]
fn settings_changes_the_theme_in_place() {
    let mut app = app();
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.config.theme, Theme::Dark);
}

#[test]
fn scrolling_stops_with_the_last_line_at_the_bottom() {
    let mut app = app();
    app.resize(80, 20);
    let rows = app.viewport.editor_rows();
    assert_eq!(rows, 17);
    command(&mut app, "help");
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.help_scroll, help::LINES.len() - rows);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_scroll, help::LINES.len() - rows);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help_scroll, help::LINES.len() - rows - 1);
    app.resize(200, 100);
    assert_eq!(app.help_scroll, 0, "everything fits");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn history_scrolls_to_its_oldest_session() {
    let mut app = app();
    let stats = code_racer_engine::Stats::default();
    for _ in 0..3 {
        let record =
            crate::history::Record::from_stats("quote".to_owned(), "english".to_owned(), &stats);
        app.history.add(record).expect("in memory");
    }
    app.resize(80, 20);
    command(&mut app, "history");
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(
        app.history_scroll,
        history_log::lines(3).len() - app.viewport.editor_rows()
    );
}

#[test]
fn the_explorer_has_the_focus_only_while_it_is_shown() {
    let mut app = app();
    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    assert!(!app.sidebar);
    assert_eq!(app.focus, Focus::Editor);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.buffer, Buffer::Practice, "j moves in the form");
    press(&mut app, KeyCode::Esc);
    assert!(app.sidebar, "Esc brings the explorer back");
    assert_eq!(app.focus, Focus::Explorer);
    command(&mut app, "set nosidebar");
    assert_eq!((app.sidebar, app.focus), (false, Focus::Editor));
    press(&mut app, KeyCode::Tab);
    assert_eq!((app.sidebar, app.focus), (true, Focus::Explorer));
}

#[test]
fn the_explorer_makes_room_on_narrow_terminals_until_the_user_decides() {
    let mut app = app();
    assert_eq!(app.focus, Focus::Explorer);
    app.resize(80, 20);
    assert_eq!((app.sidebar, app.focus), (false, Focus::Editor));
    app.resize(120, 30);
    assert!(app.sidebar, "shown again once there is room");

    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    app.resize(200, 60);
    assert!(!app.sidebar, "hidden by the user, whatever the width");
    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    app.resize(80, 20);
    assert!(app.sidebar, "shown by the user, whatever the width");
}

#[test]
fn on_a_narrow_terminal_the_explorer_shows_only_while_it_has_the_focus() {
    let mut app = app();
    app.resize(80, 20);
    press(&mut app, KeyCode::Char('?'));
    for (reveal, back) in [
        (KeyCode::Esc, KeyCode::Enter),
        (KeyCode::Char('h'), KeyCode::Char('l')),
        (KeyCode::Left, KeyCode::Right),
        (KeyCode::Tab, KeyCode::Tab),
        (KeyCode::Esc, KeyCode::Char('?')),
    ] {
        press(&mut app, reveal);
        assert_eq!(
            (app.sidebar, app.focus),
            (true, Focus::Explorer),
            "{reveal}"
        );
        press(&mut app, back);
        assert_eq!((app.sidebar, app.focus), (false, Focus::Editor), "{back}");
    }

    press_with(&mut app, 'b', KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Enter);
    assert!(app.sidebar, "pinned by the user");
    command(&mut app, "set nosidebar");
    press(&mut app, KeyCode::Esc);
    assert!(app.sidebar, "shown while it has the focus");
    press(&mut app, KeyCode::Enter);
    assert!(!app.sidebar, "hidden again, as the user wants it");
}

#[test]
fn returning_to_a_visible_explorer_keeps_it_automatic() {
    let mut app = app();
    app.resize(120, 30);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Explorer);
    app.resize(80, 20);
    assert!(!app.sidebar, "still hides itself on a narrow terminal");
}

#[test]
fn home_on_a_narrow_terminal_focuses_the_practice_form() {
    let mut app = app_with(Config::default(), Launch::Home);
    app.resize(80, 20);
    type_text(&mut app, "Ada");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        (app.buffer, app.focus, app.sidebar),
        (Buffer::Practice, Focus::Editor, false)
    );
}

/// Settings saved to a real `config.toml`, read back as the next run would.
mod saving {
    use std::path::Path;

    use super::*;
    use crate::config::tests::load;

    fn app_saving_to(path: &Path, config: Config, overrides: &Overrides, launch: Launch) -> App {
        App::new(
            config,
            overrides,
            Some(path.to_owned()),
            History::in_memory(),
            Vec::new(),
            launch,
        )
    }

    fn reload(path: &Path) -> Config {
        load(path).value
    }

    #[test]
    fn the_first_name_is_saved() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut app = app_saving_to(
            &path,
            Config::default(),
            &Overrides::default(),
            Launch::Home,
        );
        type_text(&mut app, "Ada");
        press(&mut app, KeyCode::Enter);
        assert_eq!(reload(&path).username, "Ada");
        assert_eq!((app.buffer, app.focus), (Buffer::Practice, Focus::Explorer));
    }

    #[test]
    fn commands_save_their_settings() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut app = app_saving_to(
            &path,
            configured("jean"),
            &Overrides::default(),
            Launch::Home,
        );
        for line in [
            "set theme=mono",
            "set punctuation",
            "lang french",
            "lang python",
        ] {
            command(&mut app, line);
        }
        for session in ["time 60", "words 25"] {
            command(&mut app, session);
            press(&mut app, KeyCode::Esc);
        }
        let saved = reload(&path);
        assert_eq!(saved.theme, Theme::Mono);
        assert!(saved.practice.punctuation && saved.race.punctuation);
        assert_eq!(
            (saved.practice.language, saved.race.language),
            (Language::French, Language::French)
        );
        assert_eq!(saved.race.code_language, CodeLanguage::Python);
        assert_eq!(
            (
                saved.practice.mode,
                saved.practice.word_count,
                saved.practice.duration
            ),
            (Mode::Words, 25, 60)
        );
    }

    #[test]
    fn forms_save_their_settings_for_the_next_run() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut app = app_saving_to(
            &path,
            configured("jean"),
            &Overrides::default(),
            Launch::Home,
        );
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('l'));
        command(&mut app, "config");
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Enter);
        command(&mut app, "race");
        for code in [KeyCode::Char('j'), KeyCode::Char('j'), KeyCode::Char('l')] {
            press(&mut app, code);
        }
        let saved = reload(&path);
        assert_eq!(saved.practice.mode, Mode::Time);
        assert_eq!(saved.theme, Theme::Dark);
        assert_eq!(saved.race.mode, Mode::Quote);
        let next_run = app_with(saved, Launch::Home);
        assert_eq!(next_run.config.race.mode, Mode::Quote);
    }

    #[test]
    fn flags_of_this_run_are_saved_only_once_changed_in_the_app() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let overrides = Overrides {
            theme: Some(Theme::Mono),
            server: Some("ws://10.0.0.9:8080".to_owned()),
        };
        let launch = Launch::Solo {
            practice: Practice {
                mode: Mode::Code,
                ..Practice::default()
            },
            file: None,
        };
        let mut app = app_saving_to(&path, configured("jean"), &overrides, launch);
        assert_eq!(app.config.theme, Theme::Mono);
        assert_eq!(app.config.practice.mode, Mode::Code);
        press(&mut app, KeyCode::Esc);
        command(&mut app, "set numbers");
        let saved = reload(&path);
        assert_eq!(saved.theme, Theme::Editor);
        assert_eq!(saved.multiplayer, Config::default().multiplayer);
        assert_eq!(saved.practice.mode, Mode::Words);
        assert!(saved.practice.numbers);
        command(&mut app, "set theme=dark");
        assert_eq!(reload(&path).theme, Theme::Dark);
    }

    #[test]
    fn a_flag_chosen_again_in_the_app_is_saved() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let overrides = Overrides {
            theme: Some(Theme::Mono),
            server: None,
        };
        let launch = Launch::Solo {
            practice: Practice {
                mode: Mode::Code,
                ..Practice::default()
            },
            file: None,
        };
        let mut app = app_saving_to(&path, configured("jean"), &overrides, launch);
        press(&mut app, KeyCode::Esc);
        command(&mut app, "set theme=mono");
        assert_eq!(reload(&path).theme, Theme::Mono);
        press(&mut app, KeyCode::Esc);
        command(&mut app, "code");
        assert_eq!(reload(&path).practice.mode, Mode::Code);
    }

    #[test]
    fn the_server_line_of_race_toml_is_edited_in_place() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut app = app_saving_to(
            &path,
            configured("jean"),
            &Overrides::default(),
            Launch::Home,
        );
        command(&mut app, "race");
        press(&mut app, KeyCode::Char('G'));
        press(&mut app, KeyCode::Enter);
        assert!(matches!(&app.editing, Some(edit) if edit.field == TextField::Server));
        press_with(&mut app, 'u', KeyModifiers::CONTROL);
        type_text(&mut app, "10.0.0.9:8080");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.config.multiplayer.server, "ws://10.0.0.9:8080");
        assert_eq!(reload(&path).multiplayer.server, "ws://10.0.0.9:8080");
    }

    #[test]
    fn lines_without_a_value_save_nothing() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut app = app_saving_to(
            &path,
            configured("jean"),
            &Overrides::default(),
            Launch::Home,
        );
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('G'));
        press(&mut app, KeyCode::Char('l'));
        command(&mut app, "race");
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        assert!(!path.exists(), "nothing to save");
        assert_eq!(app.message(), None);
    }

    #[test]
    fn settings_edited_by_hand_while_running_are_kept() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut app = app_saving_to(
            &path,
            configured("jean"),
            &Overrides::default(),
            Launch::Home,
        );
        std::fs::write(
            &path,
            "username = \"jean\"\ntheme = \"dark\"\n[multiplayer]\nserver = \"ws://10.0.0.5:8080\"\n",
        )
        .expect("edit by hand");
        command(&mut app, "set punctuation");
        let saved = reload(&path);
        assert_eq!(saved.theme, Theme::Dark);
        assert_eq!(saved.multiplayer.server, "ws://10.0.0.5:8080");
        assert!(saved.practice.punctuation);
    }
}

/// Rooms fed with server messages instead of a real server.
mod room {
    use code_racer_engine::TextSource;
    use code_racer_protocol::{PlayerId, PlayerProgress, PlayerView, RoomView, ServerMessage};

    use code_racer_protocol::ClientMessage;
    use tokio::sync::mpsc;

    use super::*;
    use crate::{
        app::race::Intent,
        network::{Connection, NetworkEvent},
    };

    pub const ME: PlayerId = PlayerId(1);
    pub const TEXT: &str = "Simplicity is prerequisite.";

    /// The room with the player and a rival, `progress` being the player's.
    pub fn view(phase: Phase, progress: PlayerProgress) -> RoomView {
        let player = |id, name: &str, progress| PlayerView {
            id,
            name: name.parse().expect("name"),
            ready: true,
            connected: true,
            progress,
        };
        RoomView {
            code: "FK72AD".parse().expect("code"),
            host: ME,
            text: TextSource::Quote {
                language: Language::English,
            },
            text_length: 27,
            phase,
            max_players: 8,
            players: vec![
                player(ME, "jean", progress),
                player(PlayerId(2), "alice", PlayerProgress::default()),
            ],
        }
    }

    pub fn deliver(app: &mut App, message: ServerMessage, at: Instant) {
        app.handle_network(NetworkEvent::Message(message), at);
    }

    /// In the lobby of room FK72AD, hosting it.
    pub fn joined(at: Instant) -> App {
        let mut app = app();
        command(&mut app, "join FK72AD");
        app.handle_network(NetworkEvent::Connected(ME), at);
        deliver(
            &mut app,
            ServerMessage::Room(view(Phase::Lobby, PlayerProgress::default())),
            at,
        );
        app
    }

    pub fn counting_down(at: Instant) -> App {
        let mut app = joined(at);
        deliver(
            &mut app,
            ServerMessage::Countdown {
                text: TEXT.to_owned(),
                duration_ms: 3_000,
            },
            at,
        );
        deliver(
            &mut app,
            ServerMessage::Room(view(Phase::Countdown, PlayerProgress::default())),
            at,
        );
        app
    }

    /// In the lobby over a connection that hands what is sent to the test.
    pub fn joined_over_loopback(at: Instant) -> (App, mpsc::Receiver<ClientMessage>) {
        let (connection, sent) = Connection::loopback();
        let code = "FK72AD".parse().expect("code");
        let client = RaceClient::over(connection, "ws://test".to_owned(), Intent::Join(code));
        let mut app = app();
        app.activity = Some(Activity::Race(Box::new(client)));
        app.open(Buffer::Session);
        app.handle_network(NetworkEvent::Connected(ME), at);
        deliver(
            &mut app,
            ServerMessage::Room(view(Phase::Lobby, PlayerProgress::default())),
            at,
        );
        (app, sent)
    }

    pub fn racing(at: Instant) -> App {
        let mut app = counting_down(at);
        deliver(
            &mut app,
            ServerMessage::Room(view(Phase::Racing, PlayerProgress::default())),
            at,
        );
        app
    }

    /// The results of a race the player did not finish.
    pub fn showing_results(at: Instant) -> App {
        let mut app = racing(at);
        deliver(
            &mut app,
            ServerMessage::Room(view(Phase::Finished, PlayerProgress::default())),
            at,
        );
        app
    }
}

mod multiplayer {
    use std::future::Future;

    use code_racer_server::ServerConfig;
    use tokio::{net::TcpListener, sync::oneshot, time::timeout};

    use super::*;

    /// Longer than the client's connection and handshake timeouts, five seconds
    /// each, so that a test fails on its assertions rather than on this limit.
    const WAIT: Duration = Duration::from_secs(15);

    async fn server() -> (String, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("ws://{}", listener.local_addr().expect("address"));
        let config = ServerConfig {
            countdown: Duration::from_millis(300),
            ..ServerConfig::default()
        };
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(code_racer_server::serve(listener, config, async {
            let _ = stopped.await;
        }));
        (url, stop)
    }

    fn player(name: &str, url: &str) -> App {
        let mut config = configured(name);
        config.multiplayer.server = url.to_owned();
        let mut app = app_with(config, Launch::Home);
        app.config.race.word_count = 5;
        app
    }

    async fn pump_until(app: &mut App, done: impl Fn(&App) -> bool) {
        let waited = timeout(WAIT, async {
            while !done(app) {
                let Some(connection) = app.connection_mut() else {
                    panic!("connection closed: {:?}", app.message());
                };
                let event = connection.next_event().await.expect("event");
                app.handle_network(event, Instant::now());
                app.tick(Instant::now());
            }
        })
        .await;
        assert!(waited.is_ok(), "timed out, message: {:?}", app.message());
    }

    fn phase(app: &App) -> Option<Phase> {
        app.race()?.room.as_ref().map(|room| room.phase)
    }

    fn players(app: &App) -> usize {
        app.race()
            .and_then(|client| client.room.as_ref())
            .map_or(0, |room| room.players.len())
    }

    /// The server refuses progress faster than 30 characters per second plus
    /// a burst of 5: how long typing `length` characters takes at least.
    fn believable_typing_time(length: usize) -> Duration {
        Duration::from_secs_f64(length.saturating_sub(5) as f64 / 30.0 + 0.05)
    }

    async fn both<F: Future<Output = ()>>(first: F, second: impl Future<Output = ()>) {
        tokio::join!(first, second);
    }

    #[tokio::test]
    async fn two_players_race_through_a_real_server() {
        let (url, stop) = server().await;
        let mut alice = player("alice", &url);
        let mut bob = player("bob", &url);

        command(&mut alice, "create");
        pump_until(&mut alice, |app| phase(app) == Some(Phase::Lobby)).await;
        assert_eq!(alice.buffer, Buffer::Session);
        let code = alice.room_code.clone();

        command(&mut bob, &format!("join {code}"));
        pump_until(&mut bob, |app| players(app) == 2).await;
        pump_until(&mut alice, |app| players(app) == 2).await;

        press(&mut alice, KeyCode::Char('s'));
        assert!(
            alice.message().is_some_and(Message::is_error),
            "nobody is ready yet"
        );
        press(&mut alice, KeyCode::Char('r'));
        press(&mut bob, KeyCode::Char('r'));
        let everyone_ready = |app: &App| {
            app.race()
                .and_then(|client| client.room.as_ref())
                .is_some_and(|room| room.players.len() == 2 && room.everyone_ready())
        };
        pump_until(&mut alice, everyone_ready).await;
        press(&mut alice, KeyCode::Char('s'));

        both(
            pump_until(&mut alice, |app| phase(app) == Some(Phase::Racing)),
            pump_until(&mut bob, |app| phase(app) == Some(Phase::Racing)),
        )
        .await;
        assert!(alice.is_typing() && bob.is_typing());
        assert_eq!(
            session(&alice).target().concat(),
            session(&bob).target().concat(),
            "everyone types the same text"
        );

        let length = session(&alice).target().len();
        tokio::time::sleep(believable_typing_time(length)).await;
        type_remaining(&mut bob);
        pump_until(&mut bob, |app| {
            app.race()
                .and_then(RaceClient::me)
                .is_some_and(|me| me.progress.is_finished())
        })
        .await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        type_remaining(&mut alice);
        both(
            pump_until(&mut alice, |app| phase(app) == Some(Phase::Finished)),
            pump_until(&mut bob, |app| phase(app) == Some(Phase::Finished)),
        )
        .await;

        let room = alice
            .race()
            .and_then(|client| client.room.clone())
            .expect("room");
        assert!(
            room.players
                .iter()
                .all(|player| player.progress.is_finished())
        );
        assert_eq!(
            room.standings()[0].name.as_str(),
            "bob",
            "bob finished first, although alice joined first"
        );
        assert_eq!(alice.history.records().len(), 1);
        assert_eq!(bob.history.records()[0].mode, "race");

        let later = Instant::now() + AFTER_QUIET;
        press_at(&mut bob, KeyCode::Char('r'), later);
        assert_eq!(
            bob.message(),
            Some(&Message::info("waiting for the host to start another race")),
            "only the host restarts"
        );
        press_at(&mut alice, KeyCode::Char('r'), later);
        both(
            pump_until(&mut alice, |app| phase(app) == Some(Phase::Lobby)),
            pump_until(&mut bob, |app| phase(app) == Some(Phase::Lobby)),
        )
        .await;

        press_at(&mut bob, KeyCode::Esc, later);
        assert!(bob.activity.is_none());
        assert_eq!(bob.buffer, Buffer::Race);
        pump_until(&mut alice, |app| players(app) == 1).await;
        let _ = stop.send(());
    }

    #[tokio::test]
    async fn joining_an_unknown_room_reports_the_error() {
        let (url, stop) = server().await;
        let mut app = player("carol", &url);
        command(&mut app, "join ABCDEF");
        let waited = timeout(WAIT, async {
            while let Some(connection) = app.connection_mut() {
                let event = connection.next_event().await.expect("event");
                app.handle_network(event, Instant::now());
            }
        })
        .await;
        assert!(waited.is_ok());
        assert!(
            app.message()
                .is_some_and(|message| message.is_error() && message.text.contains("ABCDEF"))
        );
        assert_eq!(app.buffer, Buffer::Race);
        let _ = stop.send(());
    }

    #[tokio::test]
    async fn a_secure_address_without_tls_is_reported_without_crashing() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (plain_socket, _) = listener.accept().await.expect("accept");
            drop(plain_socket);
        });
        let mut app = player("erin", &format!("wss://{address}"));
        command(&mut app, "create");
        let waited = timeout(WAIT, async {
            while let Some(connection) = app.connection_mut() {
                let event = connection
                    .next_event()
                    .await
                    .expect("an explicit close event");
                app.handle_network(event, Instant::now());
            }
        })
        .await;
        assert!(waited.is_ok());
        assert!(app.message().is_some_and(Message::is_error));
    }

    #[tokio::test]
    async fn an_unreachable_server_is_reported() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        drop(listener);
        let mut app = player("dave", &format!("ws://{address}"));
        command(&mut app, "create");
        let waited = timeout(WAIT, async {
            while let Some(connection) = app.connection_mut() {
                let event = connection.next_event().await.expect("event");
                app.handle_network(event, Instant::now());
            }
        })
        .await;
        assert!(waited.is_ok());
        assert!(app.activity.is_none());
        assert!(app.message().is_some_and(Message::is_error));
    }
}
