use crate::{
    app::{
        TextField,
        form::{Row, Step, Value},
        settings,
        text_settings::{self, TextSetting},
    },
    config::{Config, Mode, Practice},
};

/// Modes a race can use: time mode is solo only.
const RACE_MODES: [Mode; 3] = [Mode::Words, Mode::Quote, Mode::Code];

/// A line of `race.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Room,
    Join,
    Text(TextSetting),
    Create,
    /// The race server address, the same setting as in `config.toml`.
    Server,
}

impl Field {
    /// The text typed to set this line, for the lines that take text.
    pub const fn text_field(self) -> Option<TextField> {
        match self {
            Self::Room => Some(TextField::RoomCode),
            Self::Server => Some(TextField::Server),
            Self::Join | Self::Text(_) | Self::Create => None,
        }
    }
}

/**
 * Lines of the race form. The room line comes first, where the form
 * opens, as joining a room is what most players come for.
 */
pub fn fields(settings: &Practice) -> Vec<Field> {
    [Field::Room, Field::Join]
        .into_iter()
        .chain(
            text_settings::settings(settings)
                .into_iter()
                .map(Field::Text),
        )
        .chain([Field::Create, Field::Server])
        .collect()
}

/// TOML table header printed above a field.
pub fn section(field: Field) -> Option<&'static str> {
    match field {
        Field::Room => Some("join"),
        Field::Text(TextSetting::Mode) => Some("create"),
        Field::Server => Some("multiplayer"),
        _ => None,
    }
}

pub fn row(config: &Config, room_code: &str, field: Field) -> Row {
    match field {
        Field::Room => {
            Row::new("room", Value::Text(room_code.to_owned())).hint("Enter to type the code")
        }
        Field::Join => Row::action("join room"),
        Field::Text(setting) => text_settings::row(&config.race, setting, &RACE_MODES),
        Field::Create => Row::action("create room"),
        Field::Server => settings::row(config, settings::Field::Server),
    }
}

pub fn adjust(settings: &mut Practice, field: Field, step: Step) {
    if let Field::Text(setting) = field {
        text_settings::adjust(settings, setting, step, &RACE_MODES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_form_cycles_only_race_modes() {
        let mut settings = Practice::default();
        let mut seen = Vec::new();
        for _ in 0..3 {
            adjust(&mut settings, Field::Text(TextSetting::Mode), Step::Next);
            seen.push(settings.mode);
        }
        assert!(!seen.contains(&Mode::Time));
        assert_eq!(settings.mode, Mode::Words);
        assert_eq!(fields(&settings).len(), 9);
    }

    #[test]
    fn race_words_cycle_through_raceable_presets() {
        let mut settings = Practice::default().for_race();
        let words = Field::Text(TextSetting::Words);
        for _ in 0..Practice::WORD_COUNT_PRESETS.len() {
            adjust(&mut settings, words, Step::Next);
            assert!(code_racer_protocol::is_raceable(
                &settings.race_text_source()
            ));
        }
        assert_eq!(section(Field::Text(TextSetting::Mode)), Some("create"));
    }
}
