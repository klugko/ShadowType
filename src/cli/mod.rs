/*!
 * Command line of the `code-racer` binary.
 *
 * Flags are layered over the settings saved in `config.toml`: whatever is not
 * given on the command line keeps its saved value.
 */

mod resolve;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use code_racer_engine::{CodeLanguage, Language, TextSource};
use code_racer_protocol::RoomCode;
use thiserror::Error;

use crate::config::{Mode, Practice, Theme};

const GLOBAL_OPTIONS: &str = "Global options";

#[derive(Debug, Parser)]
#[command(
    name = "code-racer",
    version,
    about = "Typing practice that looks like your code editor, with LAN races"
)]
pub struct Cli {
    /// Race server to use, such as ws://192.168.1.20:8080
    #[arg(long, global = true, value_name = "URL", help_heading = GLOBAL_OPTIONS)]
    pub server: Option<String>,
    /// Color theme
    #[arg(long, global = true, help_heading = GLOBAL_OPTIONS)]
    pub theme: Option<Theme>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start a practice session right away
    Solo(SoloArgs),
    /// Open the multiplayer menu to create or join a race
    Multiplayer,
    /// Create a race room and wait for other players
    Create(RoomArgs),
    /// Join a race room
    Join {
        /// Code of the room, such as FK72AD
        code: RoomCode,
    },
    /// Show past results and statistics
    History,
}

/// Flags describing the text to type, shared by solo sessions and races.
#[derive(Debug, Args)]
pub struct TextArgs {
    /// What to type
    #[arg(long)]
    pub mode: Option<Mode>,
    /// english or french, or in code mode rust, python, typescript, javascript or sql
    #[arg(long, value_name = "NAME")]
    pub language: Option<String>,
    /// Number of words in words mode, 5 to 200 in a race
    #[arg(long, value_name = "N")]
    pub words: Option<u16>,
    /// Add capitals and punctuation to the words
    #[arg(long)]
    pub punctuation: bool,
    /// Mix numbers into the words
    #[arg(long)]
    pub numbers: bool,
}

#[derive(Debug, Args)]
pub struct SoloArgs {
    #[command(flatten)]
    pub text: TextArgs,
    /// Length of a time mode session, in seconds
    #[arg(long, value_name = "N")]
    pub seconds: Option<u16>,
    /// Type the contents of a file instead of generated text
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
}

/// Time mode is accepted by the parser so that `create --mode time` gets a
/// clear error, but it is not offered in the help.
#[derive(Debug, Args)]
#[command(mut_arg("mode", |mode| {
    mode.hide_possible_values(true)
        .help("What to type: words, quote or code")
}))]
pub struct RoomArgs {
    #[command(flatten)]
    pub text: TextArgs,
}

/// The screen to open, with the settings resolved from flags and defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    Home,
    Solo {
        practice: Practice,
        file: Option<PathBuf>,
    },
    Multiplayer,
    Create(TextSource),
    Join(RoomCode),
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CliError {
    #[error("unknown language `{0}`, expected one of: {names}", names = language_names())]
    UnknownLanguage(String),
    #[error("{0} is a programming language, use it with --mode code")]
    CodeLanguageOutsideCodeMode(CodeLanguage),
    #[error("{0} is not a programming language, code mode takes one of: {names}", names = code_language_names())]
    NaturalLanguageInCodeMode(Language),
    #[error("time mode is only available solo")]
    TimeModeInRace,
    #[error("--{flag} {value} is out of range, use {min} to {max}")]
    OutOfRange {
        flag: &'static str,
        value: u16,
        min: u16,
        max: u16,
    },
}

fn language_names() -> String {
    let natural = Language::ALL.map(Language::name);
    let code = CodeLanguage::ALL.map(CodeLanguage::name);
    [natural.as_slice(), code.as_slice()].concat().join(", ")
}

fn code_language_names() -> String {
    CodeLanguage::ALL.map(CodeLanguage::name).join(", ")
}
