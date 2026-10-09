use super::*;

#[test]
fn solo_without_flags_keeps_the_saved_settings() {
    let saved = Practice {
        mode: Mode::Quote,
        language: Language::French,
        word_count: 25,
        punctuation: true,
        ..Practice::default()
    };
    assert_eq!(solo(&[], &saved), saved);
}

#[test]
fn solo_flags_override_the_saved_settings() {
    let practice = solo(
        &[
            "--mode",
            "time",
            "--seconds",
            "60",
            "--language",
            "fr",
            "--numbers",
        ],
        &Practice::default(),
    );
    assert_eq!(
        practice,
        Practice {
            mode: Mode::Time,
            language: Language::French,
            duration: 60,
            numbers: true,
            ..Practice::default()
        }
    );
}

#[test]
fn programming_language_implies_code_mode() {
    let saved = Practice {
        language: Language::French,
        ..Practice::default()
    };
    let practice = solo(&["--language", "python"], &saved);
    assert_eq!(practice.mode, Mode::Code);
    assert_eq!(practice.code_language, CodeLanguage::Python);
    assert_eq!(practice.language, Language::French);
}

#[test]
fn natural_language_leaves_a_saved_code_mode_for_words() {
    let practice = solo(&["--language", "english"], &defaults_with_mode(Mode::Code));
    assert_eq!(practice.mode, Mode::Words);
    let quote = solo(&["--language", "french"], &defaults_with_mode(Mode::Quote));
    assert_eq!(quote.mode, Mode::Quote);
}

#[test]
fn count_flags_imply_their_mode() {
    let saved = defaults_with_mode(Mode::Quote);
    assert_eq!(solo(&["--seconds", "15"], &saved).mode, Mode::Time);
    assert_eq!(solo(&["--words", "10"], &saved).mode, Mode::Words);
    assert_eq!(
        solo(&["--words", "10", "--mode", "quote"], &saved).mode,
        Mode::Quote
    );
}

#[test]
fn language_must_match_the_mode() {
    let defaults = Practice::default();
    assert_eq!(
        launch(
            &["solo", "--mode", "words", "--language", "rust"],
            &defaults
        ),
        Err(CliError::CodeLanguageOutsideCodeMode(CodeLanguage::Rust))
    );
    assert_eq!(
        launch(
            &["solo", "--mode", "code", "--language", "french"],
            &defaults
        ),
        Err(CliError::NaturalLanguageInCodeMode(Language::French))
    );
}

#[test]
fn unknown_language_lists_the_choices() {
    let error = launch(&["solo", "--language", "klingon"], &Practice::default())
        .expect_err("unknown language");
    assert_eq!(error, CliError::UnknownLanguage("klingon".to_owned()));
    assert_eq!(
        error.to_string(),
        "unknown language `klingon`, expected one of: english, french, rust, python, typescript, javascript, sql"
    );
}

#[test]
fn solo_counts_must_be_in_range() {
    let defaults = Practice::default();
    let error = |arguments: &[&str]| {
        launch(arguments, &defaults)
            .expect_err("out of range")
            .to_string()
    };
    assert_eq!(
        error(&["solo", "--words", "0"]),
        "--words 0 is out of range, use 1 to 500"
    );
    assert_eq!(
        error(&["solo", "--seconds", "601"]),
        "--seconds 601 is out of range, use 5 to 600"
    );
    assert_eq!(
        error(&["solo", "--seconds", "4"]),
        "--seconds 4 is out of range, use 5 to 600"
    );
    assert_eq!(solo(&["--words", "500"], &defaults).word_count, 500);
    assert_eq!(solo(&["--seconds", "5"], &defaults).duration, 5);
}

#[test]
fn seconds_take_precedence_over_words_without_a_mode() {
    let practice = solo(&["--words", "25", "--seconds", "60"], &Practice::default());
    assert_eq!(practice.mode, Mode::Time);
    assert_eq!((practice.word_count, practice.duration), (25, 60));
}

#[test]
fn flags_cannot_switch_off_saved_options() {
    let saved = Practice {
        punctuation: true,
        ..Practice::default()
    };
    let practice = solo(&["--numbers"], &saved);
    assert!(practice.punctuation && practice.numbers);
}

#[test]
fn explicit_code_mode_takes_language_aliases() {
    let practice = solo(
        &["--mode", "code", "--language", "PY"],
        &Practice::default(),
    );
    assert_eq!(
        (practice.mode, practice.code_language),
        (Mode::Code, CodeLanguage::Python)
    );
}

#[test]
fn solo_file_is_passed_through() {
    let launched = launch(&["solo", "--file", "src/main.rs"], &Practice::default());
    assert_eq!(
        launched,
        Ok(Launch::Solo {
            practice: Practice::default(),
            file: Some(PathBuf::from("src/main.rs")),
        })
    );
}
