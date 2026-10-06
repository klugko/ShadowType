//! Vim-style `:` commands.

use std::path::PathBuf;

use clap::ValueEnum;
use code_racer_engine::{CodeLanguage, Language};
use code_racer_protocol::{RoomCode, Username};
use thiserror::Error;

use crate::config::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    /// Start a solo session with the current settings.
    Solo,
    Words(Option<u16>),
    Time(Option<u16>),
    Quote,
    Code(Option<CodeLanguage>),
    Language(Language),
    /// Practise on the content of a file.
    Edit(PathBuf),
    Set(Setting),
    Create,
    Join(RoomCode),
    Open(Page),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    Punctuation(bool),
    Numbers(bool),
    Sidebar(bool),
    Theme(Theme),
    Server(String),
    Username(Username),
}

/// Buffers that a command can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Practice,
    Race,
    History,
    Settings,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandError {
    #[error("E492: Not an editor command: {0}")]
    Unknown(String),
    #[error("E471: Argument required: {0}")]
    MissingArgument(&'static str),
    #[error("E474: Invalid argument: {0}")]
    InvalidArgument(String),
}

/// Names that complete a `:` command, in the order they are suggested.
pub const COMMAND_NAMES: [&str; 15] = [
    "solo", "words", "time", "quote", "code", "lang", "edit", "set", "create", "join", "race",
    "history", "config", "help", "quit",
];

pub fn parse(input: &str) -> Result<Command, CommandError> {
    let input = input.trim().trim_start_matches(':').trim();
    let (name, argument) = match input.split_once(char::is_whitespace) {
        Some((name, argument)) => (name, Some(argument.trim())),
        None => (input, None),
    };
    match name {
        "q" | "q!" | "qa" | "qa!" | "quit" | "wq" | "x" => Ok(Command::Quit),
        "solo" | "start" | "s" => Ok(Command::Solo),
        "words" | "w" => argument.map(number).transpose().map(Command::Words),
        "time" | "t" => argument.map(number).transpose().map(Command::Time),
        "quote" => Ok(Command::Quote),
        "code" => argument.map(code_language).transpose().map(Command::Code),
        "lang" | "language" => language_command(required(argument, "lang")?),
        "e" | "edit" | "open" => Ok(Command::Edit(PathBuf::from(required(argument, "edit")?))),
        "set" | "se" => setting(required(argument, "set")?).map(Command::Set),
        "create" | "new" => Ok(Command::Create),
        "join" | "j" => required(argument, "join")?
            .parse()
            .map(Command::Join)
            .map_err(|error| invalid(format!("{error}"))),
        "race" | "multiplayer" | "mp" => Ok(Command::Open(Page::Race)),
        "practice" => Ok(Command::Open(Page::Practice)),
        "history" | "hist" => Ok(Command::Open(Page::History)),
        "config" | "settings" => Ok(Command::Open(Page::Settings)),
        "help" | "h" => Ok(Command::Open(Page::Help)),
        "" => Err(CommandError::Unknown(String::new())),
        other => Err(CommandError::Unknown(other.to_owned())),
    }
}

/// Completes the command name being typed, cycling with `index`.
pub fn complete(input: &str, index: usize) -> Option<&'static str> {
    if input.contains(char::is_whitespace) {
        return None;
    }
    let candidates: Vec<&'static str> = COMMAND_NAMES
        .into_iter()
        .filter(|name| name.starts_with(input))
        .collect();
    (!candidates.is_empty()).then(|| candidates[index % candidates.len()])
}

fn required<'a>(argument: Option<&'a str>, command: &'static str) -> Result<&'a str, CommandError> {
    argument
        .filter(|argument| !argument.is_empty())
        .ok_or(CommandError::MissingArgument(command))
}

fn number(argument: &str) -> Result<u16, CommandError> {
    argument
        .parse::<u16>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid(argument))
}

fn code_language(argument: &str) -> Result<CodeLanguage, CommandError> {
    argument.parse().map_err(|_| invalid(argument))
}

fn language_command(argument: &str) -> Result<Command, CommandError> {
    if let Ok(language) = argument.parse::<Language>() {
        return Ok(Command::Language(language));
    }
    code_language(argument).map(|language| Command::Code(Some(language)))
}

fn setting(argument: &str) -> Result<Setting, CommandError> {
    if let Some((key, value)) = argument.split_once('=') {
        return assignment(key.trim(), value.trim().trim_matches('"'));
    }
    let (name, enabled) = match argument.strip_prefix("no") {
        Some(name) => (name, false),
        None => (argument, true),
    };
    match name {
        "punctuation" | "punct" => Ok(Setting::Punctuation(enabled)),
        "numbers" | "num" => Ok(Setting::Numbers(enabled)),
        "sidebar" | "explorer" => Ok(Setting::Sidebar(enabled)),
        _ => Err(invalid(argument)),
    }
}

fn assignment(key: &str, value: &str) -> Result<Setting, CommandError> {
    match key {
        "theme" => <Theme as ValueEnum>::from_str(value, true)
            .map(Setting::Theme)
            .map_err(|_| invalid(value)),
        "server" if !value.is_empty() => Ok(Setting::Server(value.to_owned())),
        "username" | "name" => value
            .parse()
            .map(Setting::Username)
            .map_err(|error| invalid(format!("{error}"))),
        _ => Err(invalid(format!("{key}={value}"))),
    }
}

fn invalid(argument: impl Into<String>) -> CommandError {
    CommandError::InvalidArgument(argument.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quits_like_vim() {
        for input in ["q", ":q", "q!", "quit", "wq", "  x  "] {
            assert_eq!(parse(input), Ok(Command::Quit), "{input}");
        }
    }

    #[test]
    fn session_commands_take_optional_values() {
        assert_eq!(parse("words"), Ok(Command::Words(None)));
        assert_eq!(parse("words 25"), Ok(Command::Words(Some(25))));
        assert_eq!(parse("time 60"), Ok(Command::Time(Some(60))));
        assert_eq!(
            parse("code ts"),
            Ok(Command::Code(Some(CodeLanguage::TypeScript)))
        );
        assert!(matches!(
            parse("words lots"),
            Err(CommandError::InvalidArgument(_))
        ));
        assert!(matches!(
            parse("time 0"),
            Err(CommandError::InvalidArgument(_))
        ));
    }

    #[test]
    fn language_accepts_natural_and_code_languages() {
        assert_eq!(
            parse("lang french"),
            Ok(Command::Language(Language::French))
        );
        assert_eq!(
            parse("lang rust"),
            Ok(Command::Code(Some(CodeLanguage::Rust)))
        );
        assert_eq!(parse("lang"), Err(CommandError::MissingArgument("lang")));
    }

    #[test]
    fn set_supports_toggles_and_assignments() {
        assert_eq!(
            parse("set punctuation"),
            Ok(Command::Set(Setting::Punctuation(true)))
        );
        assert_eq!(
            parse("set nonumbers"),
            Ok(Command::Set(Setting::Numbers(false)))
        );
        assert_eq!(
            parse("set theme=mono"),
            Ok(Command::Set(Setting::Theme(Theme::Mono)))
        );
        assert_eq!(
            parse("set server = ws://10.0.0.2:8080"),
            Ok(Command::Set(Setting::Server(
                "ws://10.0.0.2:8080".to_owned()
            )))
        );
        assert!(parse("set username=").is_err());
        assert!(parse("set colour=red").is_err());
    }

    #[test]
    fn join_validates_the_room_code() {
        assert_eq!(
            parse("join fk72ad"),
            Ok(Command::Join("FK72AD".parse().expect("valid")))
        );
        assert!(matches!(
            parse("join FK72A0"),
            Err(CommandError::InvalidArgument(_))
        ));
    }

    #[test]
    fn edit_keeps_the_path() {
        assert_eq!(
            parse("e src/main.rs"),
            Ok(Command::Edit(PathBuf::from("src/main.rs")))
        );
    }

    #[test]
    fn unknown_commands_use_vim_error_codes() {
        assert_eq!(
            parse("frobnicate").map_err(|error| error.to_string()),
            Err("E492: Not an editor command: frobnicate".to_owned())
        );
    }

    #[test]
    fn completion_cycles_through_matches() {
        assert_eq!(complete("h", 0), Some("history"));
        assert_eq!(complete("h", 1), Some("help"));
        assert_eq!(complete("h", 2), Some("history"));
        assert_eq!(complete("zz", 0), None);
        assert_eq!(complete("join ab", 0), None);
    }
}
