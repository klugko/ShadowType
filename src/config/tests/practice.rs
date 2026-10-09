use code_racer_engine::{CodeLanguage, Language, TextSource, WORD_COUNTS, WordOptions};
use code_racer_protocol::RACE_WORD_COUNTS;

use super::*;

#[test]
fn sanitizing_keeps_values_already_in_range() {
    let practice = Practice {
        word_count: 0,
        duration: 10_000,
        ..Practice::default()
    }
    .sanitized();
    assert_eq!(practice.word_count, *WORD_COUNTS.start());
    assert_eq!(practice.duration, *Practice::DURATION_LIMITS.end());

    let valid = Practice {
        word_count: 100,
        duration: 15,
        ..Practice::default()
    };
    assert_eq!(valid.sanitized(), valid);
}

#[test]
fn race_settings_never_use_time_mode_and_bound_the_count() {
    let solo = Practice {
        mode: Mode::Time,
        word_count: 500,
        language: Language::French,
        ..Practice::default()
    };
    let race = solo.for_race();
    assert_eq!(race.mode, Mode::Words);
    assert_eq!(race.word_count, *RACE_WORD_COUNTS.end());
    assert_eq!(race.language, Language::French);
    assert!(code_racer_protocol::is_raceable(&solo.race_text_source()));

    let quote = Practice {
        mode: Mode::Quote,
        word_count: 1,
        ..Practice::default()
    };
    assert_eq!(quote.for_race().mode, Mode::Quote);
    assert_eq!(quote.for_race().word_count, *RACE_WORD_COUNTS.start());
}

#[test]
fn every_race_text_is_raceable() {
    for mode in Mode::ALL {
        for word_count in [0, 1, 30, 1_000] {
            let settings = Practice {
                mode,
                word_count,
                ..Practice::default()
            };
            assert!(code_racer_protocol::is_raceable(
                &settings.race_text_source()
            ));
        }
    }
}

#[test]
fn presets_are_within_the_solo_and_race_limits() {
    for preset in Practice::WORD_COUNT_PRESETS {
        assert!(WORD_COUNTS.contains(&preset), "{preset}");
        assert!(RACE_WORD_COUNTS.contains(&preset), "{preset}");
    }
    for preset in Practice::DURATION_PRESETS {
        assert!(Practice::DURATION_LIMITS.contains(&preset), "{preset}");
    }
}

#[test]
fn text_source_follows_the_mode() {
    let practice = Practice {
        language: Language::French,
        code_language: CodeLanguage::Python,
        word_count: 25,
        punctuation: true,
        ..Practice::default()
    };
    let with_mode = |mode| Practice { mode, ..practice }.text_source();

    assert_eq!(
        with_mode(Mode::Words),
        Some(TextSource::Words {
            language: Language::French,
            count: 25,
            options: WordOptions {
                punctuation: true,
                numbers: false,
            },
        })
    );
    assert_eq!(with_mode(Mode::Time), None);
    assert_eq!(
        with_mode(Mode::Quote),
        Some(TextSource::Quote {
            language: Language::French
        })
    );
    assert_eq!(
        with_mode(Mode::Code),
        Some(TextSource::Code {
            language: CodeLanguage::Python
        })
    );
}
