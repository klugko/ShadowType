//! The `config.toml` buffer.

use crate::{
    app::{
        TextField,
        form::{Row, Step, Value, choices, cycle},
    },
    config::{Config, Icons, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Username,
    Theme,
    Icons,
    Mascot,
    Animations,
    Trail,
    Mouse,
    Discreet,
    Server,
}

impl Field {
    /// The text typed to set this line, for the lines that take text.
    pub const fn text_field(self) -> Option<TextField> {
        match self {
            Self::Username => Some(TextField::Username),
            Self::Server => Some(TextField::Server),
            Self::Theme
            | Self::Icons
            | Self::Mascot
            | Self::Animations
            | Self::Trail
            | Self::Mouse
            | Self::Discreet => None,
        }
    }
}

pub const FIELDS: [Field; 9] = [
    Field::Username,
    Field::Theme,
    Field::Icons,
    Field::Mascot,
    Field::Animations,
    Field::Trail,
    Field::Mouse,
    Field::Discreet,
    Field::Server,
];

pub fn row(config: &Config, field: Field) -> Row {
    match field {
        Field::Username => Row::new("username", Value::Text(config.username.clone())),
        Field::Theme => {
            Row::new("theme", Value::Text(config.theme.to_string())).hint(choices(Theme::ALL))
        }
        Field::Icons => {
            Row::new("icons", Value::Text(config.icons.to_string())).hint(choices(Icons::ALL))
        }
        Field::Mascot => Row::new("mascot", Value::Bool(config.mascot)),
        Field::Animations => Row::new("animations", Value::Bool(config.animations)),
        Field::Trail => Row::new("trail", Value::Bool(config.trail)),
        Field::Mouse => Row::new("mouse", Value::Bool(config.mouse)),
        Field::Discreet => Row::new("discreet", Value::Bool(config.discreet)).hint("F12"),
        Field::Server => Row::new("server", Value::Text(config.multiplayer.server.clone()))
            .hint("e.g. ws://192.168.1.42:8080"),
    }
}

/// Moves the value of `field` to its next or previous choice: the theme
/// cycles, the switches turn over.
pub fn adjust(config: &mut Config, field: Field, step: Step) {
    match field {
        Field::Theme => config.theme = cycle(&Theme::ALL, config.theme, step),
        Field::Icons => config.icons = cycle(&Icons::ALL, config.icons, step),
        Field::Trail => config.trail = !config.trail,
        Field::Mascot => config.mascot = !config.mascot,
        Field::Animations => config.animations = !config.animations,
        Field::Mouse => config.mouse = !config.mouse,
        Field::Discreet => config.discreet = !config.discreet,
        Field::Username | Field::Server => {}
    }
}
