use std::fs;

use code_racer_engine::{CodeLanguage, Language};

use super::*;
use crate::persist::scratch::occupy_every_backup;

#[test]
fn documented_example_survives_a_save() {
    let dir = TempDir::new();
    let path = write_config(&dir, DOCUMENTED_EXAMPLE);
    let changed = Config {
        theme: Theme::Mono,
        ..load(&path).value
    };

    save(&path, &changed).expect("save");

    assert_eq!(load(&path), Loaded::clean(changed));
}

#[test]
fn saving_keeps_comments_spelling_and_unknown_keys() {
    let dir = TempDir::new();
    let path = write_config(
        &dir,
        "# code-racer settings\n\
         username = 'Jean'  # shown to other racers\n\
         theme = \"dark\"     # editor, dark or mono\n\
         default_mode = \"words\"\n\
         future_key = true\n\
         \n\
         [multiplayer]\n\
         # the LAN server\n\
         server = \"ws://127.0.0.1:8080\"\n\
         retries = 3\n",
    );
    let config = Config {
        theme: Theme::Mono,
        ..load(&path).value
    };

    save(&path, &config).expect("save");

    let saved = fs::read_to_string(&path).expect("read");
    for line in [
        "# code-racer settings",
        "username = 'Jean'  # shown to other racers",
        "theme = \"mono\"     # editor, dark or mono",
        "future_key = true",
        "# the LAN server",
        "retries = 3",
    ] {
        assert!(
            saved.lines().any(|saved| saved == line),
            "{line} in {saved}"
        );
    }
    assert_eq!(load(&path), Loaded::clean(config));
}

#[test]
fn saved_settings_load_back_identically() {
    let dir = TempDir::new();
    let path = dir.join("settings/config.toml");
    let config = Config {
        username: "Élodie".to_owned(),
        theme: Theme::Mono,
        icons: Icons::Nerd,
        look: Look::Commit,
        mascot: false,
        animations: false,
        trail: false,
        mouse: false,
        discreet: true,
        practice: Practice {
            mode: Mode::Time,
            language: Language::French,
            code_language: CodeLanguage::Sql,
            word_count: 25,
            duration: 120,
            punctuation: true,
            numbers: true,
        },
        race: Practice {
            mode: Mode::Code,
            language: Language::English,
            code_language: CodeLanguage::Rust,
            word_count: 200,
            duration: 30,
            punctuation: false,
            numbers: true,
        },
        multiplayer: Multiplayer {
            server: "ws://192.168.1.20:9000".to_owned(),
        },
    };

    save(&path, &config).expect("save");

    assert_eq!(load(&path), Loaded::clean(config));
}

#[test]
fn saved_file_keeps_the_documented_key_names() {
    let dir = TempDir::new();
    let path = dir.join("config.toml");
    let config = Config {
        username: "Jean".to_owned(),
        ..Config::default()
    };
    save(&path, &config).expect("save");
    let saved = fs::read_to_string(&path).expect("read");

    for line in [
        "default_mode = \"words\"",
        "word_count = 50",
        "theme = \"editor\"",
        "[race]",
        "[multiplayer]",
    ] {
        assert!(
            saved.lines().any(|saved_line| saved_line == line),
            "{line} in {saved}"
        );
    }
}

#[test]
fn invalid_file_that_cannot_be_backed_up_is_never_overwritten() {
    let dir = TempDir::new();
    let path = write_config(&dir, "username = \"Jean\"\ntheme = \"solarized\"\n");
    occupy_every_backup(&path);

    let loaded = load(&path);
    let saved = save(&path, &loaded.value);

    assert_eq!(loaded.value, Config::default());
    assert!(loaded.warning.is_some());
    let error = saved.expect_err("the invalid file is kept");
    assert!(
        error
            .to_string()
            .starts_with("the file is invalid (line 2: "),
        "{error}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read"),
        "username = \"Jean\"\ntheme = \"solarized\"\n"
    );
}

#[cfg(unix)]
#[test]
fn file_that_cannot_be_read_is_never_overwritten() {
    let dir = TempDir::new();
    let path = write_config(&dir, DOCUMENTED_EXAMPLE);
    if !crate::persist::scratch::make_unreadable(&path) {
        return;
    }

    let loaded = load(&path);
    let saved = save(&path, &loaded.value);

    assert_eq!(loaded.value, Config::default());
    let warning = loaded.warning.expect("warning");
    assert!(
        warning.starts_with("cannot read config.toml ("),
        "{warning}"
    );
    assert!(saved.is_err());
    assert_eq!(
        crate::persist::scratch::read_unreadable(&path),
        DOCUMENTED_EXAMPLE
    );
}
