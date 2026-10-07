//! The `config.toml` buffer.

use crate::{
    app::{
        TextField,
        form::{Row, Step, Value, choices, cycle},
    },
    config::{Config, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Username,
    Theme,
    Server,
}

impl Field {
    /// The text typed to set this line, for the lines that take text.
    pub const fn text_field(self) -> Option<TextField> {
        match self {
            Self::Username => Some(TextField::Username),
            Self::Server => Some(TextField::Server),
            Self::Theme => None,
        }
    }
}

pub const FIELDS: [Field; 3] = [Field::Username, Field::Theme, Field::Server];

pub fn row(config: &Config, field: Field) -> Row {
    match field {
        Field::Username => {
            Row::new("username", Value::Text(config.username.clone())).hint("shown to other racers")
        }
        Field::Theme => {
            Row::new("theme", Value::Text(config.theme.to_string())).hint(choices(Theme::ALL))
        }
        Field::Server => Row::new("server", Value::Text(config.multiplayer.server.clone()))
            .hint("e.g. ws://192.168.1.42:8080"),
    }
}

pub fn cycle_theme(config: &mut Config, step: Step) {
    config.theme = cycle(&Theme::ALL, config.theme, step);
}
