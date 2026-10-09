//! Settings saved to a real `config.toml`, read back as the next run would.

use std::path::Path;

use code_racer_engine::{CodeLanguage, Language};

use super::*;
use crate::{
    config::{Mode, Practice, Theme, tests::load},
    persist::scratch::TempDir,
};

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
