use std::time::{Duration, Instant};

use code_racer_engine::{CodeLanguage, Language, Status};
use code_racer_protocol::Phase;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
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
    App::new(config, None, History::in_memory(), launch)
}

fn app() -> App {
    app_with(configured("jean"), Launch::Home)
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code), Instant::now());
}

fn press_with(app: &mut App, ch: char, modifiers: KeyModifiers) {
    app.handle_key(KeyEvent::new(KeyCode::Char(ch), modifiers), Instant::now());
}

fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        let code = if ch == '\n' {
            KeyCode::Enter
        } else {
            KeyCode::Char(ch)
        };
        press(app, code);
    }
}

fn command(app: &mut App, line: &str) {
    press(app, KeyCode::Char(':'));
    type_text(app, line);
    press(app, KeyCode::Enter);
}

/// Types whatever the session expects next until it is complete.
fn type_remaining(app: &mut App) {
    while let Some(view) = app.session_view() {
        let session = view.session;
        if session.is_finished() || session.cursor() >= session.target().len() {
            break;
        }
        let next = session.target()[session.cursor()].clone();
        type_text(app, &next);
    }
}

fn session(app: &App) -> &code_racer_engine::TypingSession {
    app.session_view().expect("a session").session
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
        app.message.as_ref().is_some_and(Message::is_error),
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
    type_remaining(&mut app);
    let run = app.solo().expect("solo run");
    assert_eq!(run.session.status(), Status::Completed);
    assert!(run.result.is_some());
    assert_eq!(app.history.records().len(), 1);
    assert_eq!(app.editor_mode(), EditorMode::Normal);
    press(&mut app, KeyCode::Char('r'));
    assert!(
        app.solo().is_some_and(|run| run.result.is_none()),
        "r restarts"
    );
    press(&mut app, KeyCode::Esc);
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Practice);
    assert_eq!(app.message, Some(Message::info("session abandoned")));
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
        app.message
            .as_ref()
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
    app.handle_paste("the whole text");
    assert_eq!(session(&app).cursor(), 0);
    assert!(app.message.as_ref().is_some_and(Message::is_error));
}

#[test]
fn errors_stay_on_screen_while_typing() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'));
    app.handle_paste("the whole text");
    type_text(&mut app, "ab");
    assert!(app.message.as_ref().is_some_and(Message::is_error));
    assert_eq!(session(&app).cursor(), 2);
}

#[test]
fn lang_with_a_programming_language_only_sets_the_code_language() {
    let mut app = room::joined(Instant::now());
    command(&mut app, "lang python");
    assert!(app.race().is_some(), "still in the room");
    assert_eq!(app.config.practice.code_language, CodeLanguage::Python);
    assert_eq!(app.race_settings.code_language, CodeLanguage::Python);
    assert_eq!(
        app.message,
        Some(Message::info("code language set to python"))
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
    assert_eq!(app.message, Some(Message::error("progress rejected")));
    room::deliver(
        &mut app,
        ServerMessage::error(ErrorCode::RoomNotFound, "room FK72AD closed"),
        now,
    );
    assert!(app.activity.is_none());
    assert_eq!(app.buffer, Buffer::Race);
    assert_eq!(app.message, Some(Message::error("room FK72AD closed")));
}

#[test]
fn a_failed_save_is_the_message_left_on_screen() {
    let directory = TempDir::new();
    let not_a_directory = directory.join("file");
    std::fs::write(&not_a_directory, "").expect("write");
    let mut app = App::new(
        configured("jean"),
        Some(not_a_directory.join("config.toml")),
        History::in_memory(),
        Launch::Home,
    );
    let failed_save = |app: &App| {
        app.message
            .as_ref()
            .is_some_and(|message| message.is_error() && message.text.contains("cannot save"))
    };
    command(&mut app, "lang french");
    assert!(failed_save(&app), "{:?}", app.message);
    command(&mut app, "set username=Ada");
    assert!(failed_save(&app), "{:?}", app.message);
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
    assert!(app.message.as_ref().is_some_and(Message::is_error));
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
fn scrolling_is_clamped_to_the_content() {
    let mut app = app();
    command(&mut app, "help");
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.help_scroll, help::LINES.len() - 1);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.help_scroll, help::LINES.len() - 2);
}

/// Rooms fed with server messages instead of a real server.
mod room {
    use code_racer_engine::TextSource;
    use code_racer_protocol::{PlayerId, PlayerProgress, PlayerView, RoomView, ServerMessage};

    use super::*;
    use crate::network::NetworkEvent;

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

    pub fn racing(at: Instant) -> App {
        let mut app = counting_down(at);
        deliver(
            &mut app,
            ServerMessage::Room(view(Phase::Racing, PlayerProgress::default())),
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

    const WAIT: Duration = Duration::from_secs(5);

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
        app.race_settings.word_count = 5;
        app
    }

    async fn pump_until(app: &mut App, done: impl Fn(&App) -> bool) {
        let waited = timeout(WAIT, async {
            while !done(app) {
                let Some(connection) = app.connection_mut() else {
                    panic!("connection closed: {:?}", app.message);
                };
                let event = connection.next_event().await.expect("event");
                app.handle_network(event, Instant::now());
                app.tick(Instant::now());
            }
        })
        .await;
        assert!(waited.is_ok(), "timed out, message: {:?}", app.message);
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
            alice.message.as_ref().is_some_and(Message::is_error),
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
        type_remaining(&mut alice);
        type_remaining(&mut bob);
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
            "alice",
            "alice typed first"
        );
        assert_eq!(alice.history.records().len(), 1);
        assert_eq!(bob.history.records()[0].mode, "race");

        press(&mut bob, KeyCode::Char('r'));
        assert_eq!(phase(&bob), Some(Phase::Finished), "only the host restarts");
        press(&mut alice, KeyCode::Char('r'));
        both(
            pump_until(&mut alice, |app| phase(app) == Some(Phase::Lobby)),
            pump_until(&mut bob, |app| phase(app) == Some(Phase::Lobby)),
        )
        .await;

        press(&mut bob, KeyCode::Esc);
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
            app.message
                .as_ref()
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
        assert!(app.message.as_ref().is_some_and(Message::is_error));
    }

    #[tokio::test]
    async fn an_unreachable_server_is_reported() {
        let mut app = player("dave", "ws://127.0.0.1:9");
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
        assert!(app.message.as_ref().is_some_and(Message::is_error));
    }
}
