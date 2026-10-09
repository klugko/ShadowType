use std::{fs, path::Path};

use code_racer_engine::{Language, WORD_COUNTS};
use code_racer_protocol::RACE_WORD_COUNTS;

use super::*;

#[test]
fn documented_example_is_understood() {
    let dir = TempDir::new();
    let loaded = load(&write_config(&dir, DOCUMENTED_EXAMPLE));

    assert_eq!(
        loaded,
        Loaded::clean(Config {
            username: "Jean".to_owned(),
            theme: Theme::Dark,
            practice: Practice {
                mode: Mode::Words,
                language: Language::French,
                word_count: 50,
                ..Practice::default()
            },
            race: Practice::default().for_race(),
            multiplayer: Multiplayer {
                server: "ws://127.0.0.1:8080".to_owned(),
            },
            ..Config::default()
        })
    );
    assert_eq!(
        loaded.value.username().map(String::from),
        Some("Jean".to_owned())
    );
}

#[test]
fn discovered_paths_use_the_documented_file_names() {
    let Some(paths) = Paths::discover() else {
        return;
    };
    let name = |path: &Path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    };
    assert_eq!(name(&paths.config_file).as_deref(), Some("config.toml"));
    assert_eq!(name(&paths.history_file).as_deref(), Some("history.json"));
    assert_eq!(name(&paths.log_file).as_deref(), Some("code-racer.log"));
}

#[test]
fn missing_keys_default_and_unknown_keys_are_ignored() {
    let dir = TempDir::new();
    let contents = "editor = \"vim\"\ndefault_mode = \"code\"\n[multiplayer]\nretries = 3\n";
    let loaded = load(&write_config(&dir, contents));

    assert_eq!(loaded.warning, None);
    assert_eq!(
        loaded.value,
        Config {
            practice: Practice {
                mode: Mode::Code,
                ..Practice::default()
            },
            ..Config::default()
        }
    );
}

#[test]
fn missing_file_gives_defaults_without_creating_it() {
    let dir = TempDir::new();
    let path = dir.join("config.toml");
    assert_eq!(load(&path), Loaded::clean(Config::default()));
    assert!(!path.exists());
}

#[test]
fn invalid_toml_is_backed_up_and_replaced_by_defaults() {
    let dir = TempDir::new();
    let path = write_config(&dir, "username = \"Jean\"\ntheme = \n");

    let loaded = load(&path);

    assert_eq!(loaded.value, Config::default());
    let warning = loaded.warning.expect("warning");
    let backup = dir.join("config.toml.bak");
    assert!(
        warning.starts_with("config.toml was invalid (line 2: "),
        "{warning}"
    );
    assert!(
        warning.ends_with(&format!(
            "); defaults loaded, backup at {}",
            backup.display()
        )),
        "{warning}"
    );
    assert_eq!(
        fs::read_to_string(backup).expect("backup"),
        "username = \"Jean\"\ntheme = \n"
    );
    assert!(!path.exists());
}

#[test]
fn unknown_theme_names_the_offending_line() {
    let dir = TempDir::new();
    let path = write_config(&dir, "username = \"Jean\"\n\ntheme = \"solarized\"\n");

    let warning = load(&path).warning.expect("warning");

    assert!(warning.contains("(line 3: "), "{warning}");
    assert!(warning.contains("solarized"), "{warning}");
    assert!(!warning.contains('\n'), "{warning}");
}

#[test]
fn out_of_range_counts_are_clamped_on_load() {
    let dir = TempDir::new();
    let path = write_config(&dir, "word_count = 9000\nduration = 1\n");

    let practice = load(&path).value.practice;

    assert_eq!(practice.word_count, *WORD_COUNTS.end());
    assert_eq!(practice.duration, *Practice::DURATION_LIMITS.start());
}

#[test]
fn race_settings_are_kept_apart_from_solo_settings() {
    let dir = TempDir::new();
    let contents = "default_mode = \"time\"\nword_count = 10\n\
                    [race]\ndefault_mode = \"quote\"\nlanguage = \"french\"\n";

    let config = load(&write_config(&dir, contents)).value;

    assert_eq!(
        (config.practice.mode, config.practice.word_count),
        (Mode::Time, 10)
    );
    assert_eq!(
        config.race,
        Practice {
            mode: Mode::Quote,
            language: Language::French,
            ..Practice::default().for_race()
        }
    );
}

#[test]
fn hand_edited_race_settings_are_made_raceable() {
    let dir = TempDir::new();
    let contents = "[race]\ndefault_mode = \"time\"\nword_count = 1000\nduration = 1\n";

    let race = load(&write_config(&dir, contents)).value.race;

    assert_eq!(race.mode, Mode::Words);
    assert_eq!(race.word_count, *RACE_WORD_COUNTS.end());
    assert_eq!(race.duration, *Practice::DURATION_LIMITS.start());
    assert!(
        race.text_source()
            .is_some_and(|text| code_racer_protocol::is_raceable(&text))
    );
}

#[test]
fn missing_race_settings_default_to_the_solo_defaults_made_raceable() {
    let dir = TempDir::new();
    let path = write_config(&dir, DOCUMENTED_EXAMPLE);

    assert_eq!(load(&path).value.race, Practice::default().for_race());
}

#[cfg(unix)]
#[test]
fn a_file_left_in_place_is_not_writable() {
    let dir = TempDir::new();
    let path = write_config(&dir, DOCUMENTED_EXAMPLE);
    if !crate::persist::scratch::make_unreadable(&path) {
        return;
    }
    let loaded = load_config(&path);
    assert_eq!(loaded.value, None);
    assert!(loaded.warning.is_some());
    crate::persist::scratch::read_unreadable(&path);
}

#[test]
fn the_new_settings_default_to_on_and_load_from_their_keys() {
    let dir = TempDir::new();
    assert_eq!(Config::default().look, Look::Notes);
    assert!(Config::default().mascot && Config::default().animations);
    assert!(Config::default().mouse);
    let contents = "look = \"log\"
mascot = false
animations = false
mouse = false
";
    let loaded = load(&write_config(&dir, contents)).value;
    assert_eq!(loaded.look, Look::Log);
    assert!(!loaded.mascot && !loaded.animations && !loaded.mouse);
}
