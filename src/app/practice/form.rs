use crate::{
    app::{
        form::{Row, Step, Value, choices, cycle},
        text_settings::{self, TextSetting},
    },
    config::{Config, Look, Mode, Practice},
};

/// A line of `practice.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Text(TextSetting),
    /// What prose looks like on screen; code always looks like code.
    Look,
    Start,
}

/// Lines of the practice form for the current mode.
pub fn fields(practice: &Practice) -> Vec<Field> {
    let look = (practice.mode != Mode::Code).then_some(Field::Look);
    text_settings::settings(practice)
        .into_iter()
        .map(Field::Text)
        .chain(look)
        .chain([Field::Start])
        .collect()
}

pub fn row(config: &Config, field: Field) -> Row {
    match field {
        Field::Text(setting) => text_settings::row(&config.practice, setting, &Mode::ALL),
        Field::Look => {
            Row::new("look", Value::Text(config.look.to_string())).hint(choices(Look::ALL))
        }
        Field::Start => Row::action("start session"),
    }
}

pub fn adjust(config: &mut Config, field: Field, step: Step) {
    match field {
        Field::Text(setting) => {
            text_settings::adjust(&mut config.practice, setting, step, &Mode::ALL);
        }
        Field::Look => config.look = cycle(&Look::ALL, config.look, step),
        Field::Start => {}
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
    fn form_ends_with_the_start_line_and_offers_time_mode() {
        let fields = fields(&practice(Mode::Code));
        assert_eq!(
            fields,
            [
                Field::Text(TextSetting::Mode),
                Field::Text(TextSetting::Language),
                Field::Start
            ]
        );
        let mut config = Config::default();
        adjust(&mut config, Field::Text(TextSetting::Mode), Step::Next);
        assert_eq!(config.practice.mode, Mode::Time);
        adjust(&mut config, Field::Start, Step::Next);
        assert_eq!(
            config.practice.mode,
            Mode::Time,
            "the start line has no value"
        );
    }

    #[test]
    fn prose_offers_a_look_and_code_does_not() {
        let words = fields(&practice(Mode::Words));
        assert_eq!(words[words.len() - 2], Field::Look);
        assert!(!fields(&practice(Mode::Code)).contains(&Field::Look));
        let mut config = Config::default();
        adjust(&mut config, Field::Look, Step::Next);
        assert_eq!(config.look, Look::Todo);
        adjust(&mut config, Field::Look, Step::Previous);
        adjust(&mut config, Field::Look, Step::Previous);
        assert_eq!(config.look, Look::Shuffle);
        assert_eq!(row(&config, Field::Look).hint, choices(Look::ALL));
    }
}
