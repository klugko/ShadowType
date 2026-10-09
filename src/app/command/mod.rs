//! Vim-style `:` commands.

use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use code_racer_engine::{CodeLanguage, Language, WORD_COUNTS};
use code_racer_protocol::{RoomCode, Username};
use directories::BaseDirs;
use thiserror::Error;

use crate::config::{Icons, Look, Practice, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    /// Start a solo session with the current settings.
    Solo,
    Words(Option<u16>),
    Time(Option<u16>),
    Quote,
    /// Start a code session, in this language if given.
    Code(Option<CodeLanguage>),
    /// Set the natural language of words and quotes.
    Language(Language),
    /// Set the programming language of code sessions, without starting one.
    CodeLanguage(CodeLanguage),
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
    Mascot(bool),
    Animations(bool),
    Trail(bool),
    Mouse(bool),
    Discreet(bool),
    Theme(Theme),
    Icons(Icons),
    Look(Look),
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
        "words" | "w" => argument
            .map(|count| number_in(count, &WORD_COUNTS))
            .transpose()
            .map(Command::Words),
        "time" | "t" => argument
            .map(|seconds| number_in(seconds, &Practice::DURATION_LIMITS))
            .transpose()
            .map(Command::Time),
        "quote" => Ok(Command::Quote),
        "code" => argument.map(code_language).transpose().map(Command::Code),
        "lang" | "language" => language_command(required(argument, "lang")?),
        "e" | "edit" | "open" => {
            let home = BaseDirs::new();
            let path = file_argument(
                required(argument, "edit")?,
                home.as_ref().map(BaseDirs::home_dir),
            );
            Ok(Command::Edit(path))
        }
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

/**
 * The file named by `argument` as a shell would read it: without the
 * quotes around it, such as those of a path copied from a file manager,
 * and from the `home` directory when it starts with `~` alone, as in
 * `~/projects/lib.rs`. `~user` forms are left alone.
 */
fn file_argument(argument: &str, home: Option<&Path>) -> PathBuf {
    let path = unquoted(argument);
    let Some(rest) = path.strip_prefix('~') else {
        return PathBuf::from(path);
    };
    let mut after_tilde = rest.chars();
    let from_home = after_tilde
        .next()
        .is_none_or(std::path::is_separator)
        .then_some(after_tilde.as_str());
    match (home, from_home) {
        (Some(home), Some(relative)) => home.join(relative),
        _ => PathBuf::from(path),
    }
}

/// `text` without one pair of matching quotes around it.
fn unquoted(text: &str) -> &str {
    ['"', '\'']
        .into_iter()
        .find_map(|quote| text.strip_prefix(quote)?.strip_suffix(quote))
        .unwrap_or(text)
}

fn required<'a>(argument: Option<&'a str>, command: &'static str) -> Result<&'a str, CommandError> {
    argument
        .filter(|argument| !argument.is_empty())
        .ok_or(CommandError::MissingArgument(command))
}

/// A number within `range`, refused rather than clamped as on the command line.
fn number_in(argument: &str, range: &RangeInclusive<u16>) -> Result<u16, CommandError> {
    argument
        .parse::<u16>()
        .ok()
        .filter(|value| range.contains(value))
        .ok_or_else(|| {
            invalid(format!(
                "{argument}, use {} to {}",
                range.start(),
                range.end()
            ))
        })
}

fn code_language(argument: &str) -> Result<CodeLanguage, CommandError> {
    argument.parse().map_err(|_| invalid(argument))
}

fn language_command(argument: &str) -> Result<Command, CommandError> {
    if let Ok(language) = argument.parse::<Language>() {
        return Ok(Command::Language(language));
    }
    code_language(argument).map(Command::CodeLanguage)
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
        "mascot" | "pet" => Ok(Setting::Mascot(enabled)),
        "animations" | "anim" => Ok(Setting::Animations(enabled)),
        "trail" => Ok(Setting::Trail(enabled)),
        "mouse" => Ok(Setting::Mouse(enabled)),
        "discreet" | "stealth" => Ok(Setting::Discreet(enabled)),
        _ => Err(invalid(argument)),
    }
}

fn assignment(key: &str, value: &str) -> Result<Setting, CommandError> {
    match key {
        "theme" => <Theme as ValueEnum>::from_str(value, true)
            .map(Setting::Theme)
            .map_err(|_| invalid(value)),
        "icons" => <Icons as ValueEnum>::from_str(value, true)
            .map(Setting::Icons)
            .map_err(|_| invalid(value)),
        "look" => <Look as ValueEnum>::from_str(value, true)
            .map(Setting::Look)
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
mod tests;
