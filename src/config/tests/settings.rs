use clap::ValueEnum;

use super::*;

#[test]
fn only_changed_settings_are_adopted() {
    let kept = Config {
        theme: Theme::Dark,
        ..Config::default()
    };
    let before = Config {
        theme: Theme::Mono,
        practice: Practice {
            mode: Mode::Code,
            ..Practice::default()
        },
        ..Config::default()
    };
    let after = Config {
        username: "Ada".to_owned(),
        practice: Practice {
            word_count: 25,
            ..before.practice
        },
        ..before.clone()
    };
    let mut adopted = kept.clone();
    adopted.adopt_changes(&before, &after);
    assert_eq!(adopted.username, "Ada");
    assert_eq!(adopted.theme, Theme::Dark, "unchanged since before");
    assert_eq!(adopted.practice.mode, Mode::Words, "unchanged since before");
    assert_eq!(adopted.practice.word_count, 25);
}

#[test]
fn username_is_only_given_when_valid() {
    let named = |username: &str| Config {
        username: username.to_owned(),
        ..Config::default()
    };
    assert_eq!(named("").username(), None);
    assert_eq!(named("   ").username(), None);
    assert_eq!(named("Bob\u{7}").username(), None);
    assert_eq!(named(&"x".repeat(25)).username(), None);
    assert_eq!(
        named("  Jean ").username().map(String::from),
        Some("Jean".to_owned())
    );
}

#[test]
fn names_match_the_serialized_and_command_line_forms() {
    for theme in Theme::ALL {
        let value = theme.to_possible_value().expect("possible value");
        assert_eq!(value.get_name(), theme.name());
        assert_eq!(Theme::from_str(theme.name(), true), Ok(theme));
    }
    for mode in Mode::ALL {
        let value = mode.to_possible_value().expect("possible value");
        assert_eq!(value.get_name(), mode.to_string());
    }
    for look in Look::ALL {
        let value = look.to_possible_value().expect("possible value");
        assert_eq!(value.get_name(), look.name());
        assert_eq!(Look::from_str(look.name(), true), Ok(look));
    }
    for icons in Icons::ALL {
        let value = icons.to_possible_value().expect("possible value");
        assert_eq!(value.get_name(), icons.name());
    }
    let vscode = toml::to_string(&Config {
        theme: Theme::VsCode,
        ..Config::default()
    })
    .expect("serialized");
    assert!(vscode.contains("theme = \"vscode\""), "{vscode}");
}

#[test]
fn a_shuffle_never_gives_the_same_look_twice_in_a_row() {
    let picked: std::collections::HashSet<Look> = (0..60)
        .map(|seed| Look::shuffled_after(Look::Todo, seed))
        .collect();
    assert_eq!(picked.len(), Look::DISGUISES.len() - 1);
    assert!(!picked.contains(&Look::Todo));
    assert!(!picked.contains(&Look::Shuffle));
}
