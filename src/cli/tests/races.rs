use code_racer_engine::WordOptions;

use super::*;

#[test]
fn races_check_the_language_against_the_mode_too() {
    let saved = defaults_with_mode(Mode::Code);
    assert_eq!(
        create(&["--mode", "code", "--language", "fr"], &saved),
        Err(CliError::NaturalLanguageInCodeMode(Language::French))
    );
    assert_eq!(
        create(&["--language", "fr"], &saved),
        Ok(TextSource::Words {
            language: Language::French,
            count: 50,
            options: WordOptions::default(),
        })
    );
}

#[test]
fn create_builds_the_race_text_from_defaults_and_flags() {
    let saved = Practice {
        language: Language::French,
        word_count: 30,
        ..Practice::default()
    };
    assert_eq!(
        create(&["--punctuation"], &saved),
        Ok(TextSource::Words {
            language: Language::French,
            count: 30,
            options: WordOptions {
                punctuation: true,
                numbers: false
            },
        })
    );
    assert_eq!(
        create(&["--language", "ts"], &saved),
        Ok(TextSource::Code {
            language: CodeLanguage::TypeScript
        })
    );
    assert_eq!(
        create(&["--mode", "quote"], &saved),
        Ok(TextSource::Quote {
            language: Language::French
        })
    );
}

#[test]
fn races_refuse_time_mode() {
    let error = create(&["--mode", "time"], &Practice::default()).expect_err("time race");
    assert_eq!(error, CliError::TimeModeInRace);
    assert_eq!(error.to_string(), "time mode is only available solo");
}

#[test]
fn rooms_are_created_with_the_saved_race_settings() {
    let saved = Config {
        practice: defaults_with_mode(Mode::Code),
        race: Practice {
            mode: Mode::Quote,
            language: Language::French,
            ..Practice::default()
        },
        ..Config::default()
    };
    let cli = Cli::try_parse_from(["code-racer", "create"]).expect("valid arguments");
    assert_eq!(
        cli.launch(&saved),
        Ok(Launch::Create(TextSource::Quote {
            language: Language::French
        }))
    );
}

#[test]
fn a_saved_time_mode_races_on_words() {
    let saved = defaults_with_mode(Mode::Time);
    assert!(matches!(
        create(&[], &saved),
        Ok(TextSource::Words { count: 50, .. })
    ));
}

#[test]
fn race_word_counts_are_bounded() {
    let defaults = Practice::default();
    assert_eq!(
        create(&["--words", "4"], &defaults),
        Err(CliError::OutOfRange {
            flag: "words",
            value: 4,
            min: 5,
            max: 200
        })
    );
    assert!(create(&["--words", "201"], &defaults).is_err());
    let saved = Practice {
        word_count: 500,
        ..defaults
    };
    assert!(matches!(
        create(&[], &saved),
        Ok(TextSource::Words { count: 200, .. })
    ));
}
