/*!
 * The lines that describe a text to type, shared by `practice.toml` and
 * `race.toml`: the two forms differ only in the modes they offer.
 */

use code_racer_engine::{CodeLanguage, Language};

use crate::{
    app::form::{Row, Step, Value, choices, cycle, cycle_preset},
    config::{Mode, Practice},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSetting {
    Mode,
    /// The natural language, or the programming language in code mode.
    Language,
    Words,
    /// Length of a time session.
    Duration,
    Punctuation,
    Numbers,
}

/// The settings that apply in the current mode, in order.
pub fn settings(practice: &Practice) -> Vec<TextSetting> {
    let mut settings = vec![TextSetting::Mode, TextSetting::Language];
    let length = match practice.mode {
        Mode::Words => TextSetting::Words,
        Mode::Time => TextSetting::Duration,
        Mode::Quote | Mode::Code => return settings,
    };
    settings.extend([length, TextSetting::Punctuation, TextSetting::Numbers]);
    settings
}

/// The line of `setting`, offering `modes` on the mode line.
pub fn row(practice: &Practice, setting: TextSetting, modes: &[Mode]) -> Row {
    match setting {
        TextSetting::Mode => {
            Row::new("mode", Value::Text(practice.mode.to_string())).hint(choices(modes))
        }
        TextSetting::Language if practice.mode == Mode::Code => {
            Row::new("language", Value::Text(practice.code_language.to_string()))
                .hint(choices(CodeLanguage::ALL))
        }
        TextSetting::Language => Row::new("language", Value::Text(practice.language.to_string()))
            .hint(choices(Language::ALL)),
        TextSetting::Words => Row::new("words", Value::Number(practice.word_count.into()))
            .hint(choices(Practice::WORD_COUNT_PRESETS)),
        TextSetting::Duration => Row::new("seconds", Value::Number(practice.duration.into()))
            .hint(choices(Practice::DURATION_PRESETS)),
        TextSetting::Punctuation => Row::new("punctuation", Value::Bool(practice.punctuation)),
        TextSetting::Numbers => Row::new("numbers", Value::Bool(practice.numbers)),
    }
}

/**
 * Moves `setting` to its next or previous choice, cycling through `modes`
 * on the mode line.
 */
pub fn adjust(practice: &mut Practice, setting: TextSetting, step: Step, modes: &[Mode]) {
    match setting {
        TextSetting::Mode => practice.mode = cycle(modes, practice.mode, step),
        TextSetting::Language if practice.mode == Mode::Code => {
            practice.code_language = cycle(&CodeLanguage::ALL, practice.code_language, step);
        }
        TextSetting::Language => practice.language = cycle(&Language::ALL, practice.language, step),
        TextSetting::Words => {
            practice.word_count =
                cycle_preset(&Practice::WORD_COUNT_PRESETS, practice.word_count, step);
        }
        TextSetting::Duration => {
            practice.duration = cycle_preset(&Practice::DURATION_PRESETS, practice.duration, step);
        }
        TextSetting::Punctuation => practice.punctuation = !practice.punctuation,
        TextSetting::Numbers => practice.numbers = !practice.numbers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn practice(mode: Mode) -> Practice {
        Practice {
            mode,
            ..Practice::default()
        }
    }

    #[test]
    fn only_the_settings_of_the_mode_are_shown() {
        use TextSetting::{Duration, Language, Mode as ModeLine, Numbers, Punctuation, Words};
        assert_eq!(
            settings(&practice(Mode::Words)),
            [ModeLine, Language, Words, Punctuation, Numbers]
        );
        assert_eq!(
            settings(&practice(Mode::Time)),
            [ModeLine, Language, Duration, Punctuation, Numbers]
        );
        assert_eq!(settings(&practice(Mode::Code)), [ModeLine, Language]);
    }

    #[test]
    fn language_line_follows_the_mode() {
        let mut settings = practice(Mode::Code);
        adjust(&mut settings, TextSetting::Language, Step::Next, &Mode::ALL);
        assert_eq!(settings.code_language, CodeLanguage::Python);
        assert_eq!(settings.language, Language::English);
        settings.mode = Mode::Words;
        adjust(&mut settings, TextSetting::Language, Step::Next, &Mode::ALL);
        assert_eq!(settings.language, Language::French);
    }

    #[test]
    fn mode_line_offers_only_the_given_modes() {
        let modes = [Mode::Words, Mode::Quote];
        let mut settings = practice(Mode::Words);
        adjust(&mut settings, TextSetting::Mode, Step::Next, &modes);
        assert_eq!(settings.mode, Mode::Quote);
        adjust(&mut settings, TextSetting::Mode, Step::Next, &modes);
        assert_eq!(settings.mode, Mode::Words);
        assert_eq!(
            row(&settings, TextSetting::Mode, &modes).hint,
            "words · quote"
        );
    }

    #[test]
    fn lengths_cycle_through_the_presets_they_advertise() {
        let mut settings = practice(Mode::Words);
        settings.word_count = 30;
        adjust(&mut settings, TextSetting::Words, Step::Next, &Mode::ALL);
        assert_eq!(settings.word_count, 50);
        assert_eq!(
            row(&settings, TextSetting::Words, &Mode::ALL).hint,
            choices(Practice::WORD_COUNT_PRESETS)
        );
        adjust(
            &mut settings,
            TextSetting::Duration,
            Step::Previous,
            &Mode::ALL,
        );
        assert_eq!(settings.duration, 15);
    }
}
