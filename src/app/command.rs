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
    fn lengths_out_of_range_are_refused_with_the_range() {
        assert_eq!(
            parse("words 9999").map_err(|error| error.to_string()),
            Err("E474: Invalid argument: 9999, use 1 to 500".to_owned())
        );
        assert!(parse("time 4").is_err());
        assert_eq!(parse("time 600"), Ok(Command::Time(Some(600))));
    }

    #[test]
    fn language_accepts_natural_and_code_languages() {
        assert_eq!(
            parse("lang french"),
            Ok(Command::Language(Language::French))
        );
        assert_eq!(
            parse("lang rust"),
            Ok(Command::CodeLanguage(CodeLanguage::Rust))
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
    fn set_toggles_the_mascot_animations_mouse_and_chooses_the_look() {
        assert_eq!(
            parse("set nomascot"),
            Ok(Command::Set(Setting::Mascot(false)))
        );
        assert_eq!(
            parse("set animations"),
            Ok(Command::Set(Setting::Animations(true)))
        );
        assert_eq!(
            parse("set nomouse"),
            Ok(Command::Set(Setting::Mouse(false)))
        );
        assert_eq!(
            parse("set discreet"),
            Ok(Command::Set(Setting::Discreet(true)))
        );
        assert_eq!(
            parse("set look=commit"),
            Ok(Command::Set(Setting::Look(Look::Commit)))
        );
        assert!(parse("set look=poem").is_err());
        assert_eq!(
            parse("set theme=vscode"),
            Ok(Command::Set(Setting::Theme(Theme::VsCode)))
        );
        assert_eq!(
            parse("set icons=nerd"),
            Ok(Command::Set(Setting::Icons(Icons::Nerd)))
        );
        assert_eq!(
            parse("set notrail"),
            Ok(Command::Set(Setting::Trail(false)))
        );
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
        assert_eq!(
            parse("e \"notes/my code.rs\""),
            Ok(Command::Edit(PathBuf::from("notes/my code.rs")))
        );
    }

    #[test]
    fn file_arguments_start_from_the_home_directory_with_a_tilde() {
        let home = Path::new("/home/ada");
        let cases = [
            ("~", "/home/ada"),
            ("~/projects/lib.rs", "/home/ada/projects/lib.rs"),
            ("~ada/lib.rs", "~ada/lib.rs"),
            ("~notes.txt", "~notes.txt"),
            ("a/~/b.rs", "a/~/b.rs"),
            ("src/main.rs", "src/main.rs"),
        ];
        for (argument, path) in cases {
            assert_eq!(
                file_argument(argument, Some(home)),
                PathBuf::from(path),
                "{argument}"
            );
        }
        assert_eq!(file_argument("~/lib.rs", None), PathBuf::from("~/lib.rs"));
    }

    #[cfg(windows)]
    #[test]
    fn file_arguments_take_a_backslash_after_the_tilde_on_windows() {
        let home = Path::new(r"C:\Users\ada");
        assert_eq!(
            file_argument(r"~\code\main.rs", Some(home)),
            home.join(r"code\main.rs")
        );
    }

    #[test]
    fn file_arguments_lose_the_quotes_around_them() {
        let cases = [
            (r#""C:\a b\c.rs""#, r"C:\a b\c.rs"),
            ("'x y'", "x y"),
            (r#"""#, r#"""#),
            (r#""x'"#, r#""x'"#),
            ("my notes.txt", "my notes.txt"),
            (r#""~/my notes.txt""#, "/home/ada/my notes.txt"),
        ];
        for (argument, path) in cases {
            assert_eq!(
                file_argument(argument, Some(Path::new("/home/ada"))),
                PathBuf::from(path),
                "{argument}"
            );
        }
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
