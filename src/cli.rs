//! Command line of the `code-racer` binary.
//!
//! Flags are layered over the settings saved in `config.toml`: whatever is not
//! given on the command line keeps its saved value.

use std::{ops::RangeInclusive, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use code_racer_engine::{CodeLanguage, Language, TextSource, WORD_COUNTS};
use code_racer_protocol::{RACE_WORD_COUNTS, RoomCode};
use thiserror::Error;

use crate::config::{Config, Mode, Practice, Theme};

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

impl Cli {
    /// Resolves the screen to open, layering the flags over the saved solo
    /// or race settings.
    pub fn launch(&self, saved: &Config) -> Result<Launch, CliError> {
        Ok(match &self.command {
            None => Launch::Home,
            Some(Command::Solo(args)) => Launch::Solo {
                practice: args.practice(&saved.practice)?,
                file: args.file.clone(),
            },
            Some(Command::Multiplayer) => Launch::Multiplayer,
            Some(Command::Create(args)) => Launch::Create(args.text_source(&saved.race)?),
            Some(Command::Join { code }) => Launch::Join(code.clone()),
            Some(Command::History) => Launch::History,
        })
    }
}

impl SoloArgs {
    fn practice(&self, defaults: &Practice) -> Result<Practice, CliError> {
        check_range("seconds", self.seconds, &Practice::DURATION_LIMITS)?;
        let practice = self.text.practice(defaults, self.seconds, &WORD_COUNTS)?;
        Ok(practice.sanitized())
    }
}

impl RoomArgs {
    /// Time mode cannot be raced: it is refused when asked for explicitly and
    /// replaced by words mode when it is only the saved default.
    fn text_source(&self, defaults: &Practice) -> Result<TextSource, CliError> {
        if self.text.mode == Some(Mode::Time) {
            return Err(CliError::TimeModeInRace);
        }
        let practice = self
            .text
            .practice(&defaults.for_race(), None, &RACE_WORD_COUNTS)?;
        Ok(practice.race_text_source())
    }
}

impl TextArgs {
    fn practice(
        &self,
        defaults: &Practice,
        seconds: Option<u16>,
        word_counts: &RangeInclusive<u16>,
    ) -> Result<Practice, CliError> {
        check_range("words", self.words, word_counts)?;
        let language = self
            .language
            .as_deref()
            .map(LanguageName::parse)
            .transpose()?;
        let practice = Practice {
            mode: self.mode(language, seconds.is_some(), defaults.mode),
            word_count: self.words.unwrap_or(defaults.word_count),
            duration: seconds.unwrap_or(defaults.duration),
            punctuation: self.punctuation || defaults.punctuation,
            numbers: self.numbers || defaults.numbers,
            ..*defaults
        };
        match language {
            Some(language) => language.apply_to(practice),
            None => Ok(practice),
        }
    }

    /// Without `--mode`, the other flags tell which mode is meant: a
    /// programming language means code, `--seconds` time and `--words` words.
    fn mode(&self, language: Option<LanguageName>, seconds_given: bool, default: Mode) -> Mode {
        if let Some(mode) = self.mode {
            return mode;
        }
        match language {
            Some(LanguageName::Code(_)) => Mode::Code,
            _ if seconds_given => Mode::Time,
            _ if self.words.is_some() => Mode::Words,
            Some(LanguageName::Natural(_)) if default == Mode::Code => Mode::Words,
            _ => default,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum LanguageName {
    Natural(Language),
    Code(CodeLanguage),
}

impl LanguageName {
    fn parse(name: &str) -> Result<Self, CliError> {
        name.parse()
            .map(Self::Natural)
            .or_else(|_| name.parse().map(Self::Code))
            .map_err(|_| CliError::UnknownLanguage(name.to_owned()))
    }

    /// Sets the language of `practice`, which must suit its mode.
    fn apply_to(self, practice: Practice) -> Result<Practice, CliError> {
        match (practice.mode, self) {
            (Mode::Code, Self::Code(code_language)) => Ok(Practice {
                code_language,
                ..practice
            }),
            (Mode::Code, Self::Natural(language)) => {
                Err(CliError::NaturalLanguageInCodeMode(language))
            }
            (_, Self::Code(language)) => Err(CliError::CodeLanguageOutsideCodeMode(language)),
            (_, Self::Natural(language)) => Ok(Practice {
                language,
                ..practice
            }),
        }
    }
}

fn check_range(
    flag: &'static str,
    value: Option<u16>,
    range: &RangeInclusive<u16>,
) -> Result<(), CliError> {
    match value {
        Some(value) if !range.contains(&value) => Err(CliError::OutOfRange {
            flag,
            value,
            min: *range.start(),
            max: *range.end(),
        }),
        _ => Ok(()),
    }
}

fn language_names() -> String {
    let natural = Language::ALL.map(Language::name);
    let code = CodeLanguage::ALL.map(CodeLanguage::name);
    [natural.as_slice(), code.as_slice()].concat().join(", ")
}

fn code_language_names() -> String {
    CodeLanguage::ALL.map(CodeLanguage::name).join(", ")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use code_racer_engine::WordOptions;

    use super::*;

    /// Launches with `defaults` saved for solo sessions and, made raceable,
    /// for races.
    fn launch(arguments: &[&str], defaults: &Practice) -> Result<Launch, CliError> {
        let saved = Config {
            practice: *defaults,
            race: defaults.for_race(),
            ..Config::default()
        };
        let cli =
            Cli::try_parse_from(["code-racer"].iter().chain(arguments)).expect("valid arguments");
        cli.launch(&saved)
    }

    fn solo(arguments: &[&str], defaults: &Practice) -> Practice {
        let mut command = vec!["solo"];
        command.extend_from_slice(arguments);
        match launch(&command, defaults) {
            Ok(Launch::Solo { practice, .. }) => practice,
            other => panic!("expected a solo launch, got {other:?}"),
        }
    }

    fn create(arguments: &[&str], defaults: &Practice) -> Result<TextSource, CliError> {
        let mut command = vec!["create"];
        command.extend_from_slice(arguments);
        launch(&command, defaults).map(|launch| match launch {
            Launch::Create(text) => text,
            other => panic!("expected a create launch, got {other:?}"),
        })
    }

    fn defaults_with_mode(mode: Mode) -> Practice {
        Practice {
            mode,
            ..Practice::default()
        }
    }

    #[test]
    fn no_subcommand_opens_home() {
        assert_eq!(launch(&[], &Practice::default()), Ok(Launch::Home));
    }

    #[test]
    fn global_flags_are_accepted_after_a_subcommand() {
        let cli = Cli::try_parse_from([
            "code-racer",
            "history",
            "--theme",
            "mono",
            "--server",
            "lan:9000",
        ])
        .expect("valid arguments");
        assert_eq!(cli.theme, Some(Theme::Mono));
        assert_eq!(cli.server.as_deref(), Some("lan:9000"));
        assert_eq!(cli.launch(&Config::default()), Ok(Launch::History));
    }

    #[test]
    fn solo_without_flags_keeps_the_saved_settings() {
        let saved = Practice {
            mode: Mode::Quote,
            language: Language::French,
            word_count: 25,
            punctuation: true,
            ..Practice::default()
        };
        assert_eq!(solo(&[], &saved), saved);
    }

    #[test]
    fn solo_flags_override_the_saved_settings() {
        let practice = solo(
            &[
                "--mode",
                "time",
                "--seconds",
                "60",
                "--language",
                "fr",
                "--numbers",
            ],
            &Practice::default(),
        );
        assert_eq!(
            practice,
            Practice {
                mode: Mode::Time,
                language: Language::French,
                duration: 60,
                numbers: true,
                ..Practice::default()
            }
        );
    }

    #[test]
    fn programming_language_implies_code_mode() {
        let saved = Practice {
            language: Language::French,
            ..Practice::default()
        };
        let practice = solo(&["--language", "python"], &saved);
        assert_eq!(practice.mode, Mode::Code);
        assert_eq!(practice.code_language, CodeLanguage::Python);
        assert_eq!(practice.language, Language::French);
    }

    #[test]
    fn natural_language_leaves_a_saved_code_mode_for_words() {
        let practice = solo(&["--language", "english"], &defaults_with_mode(Mode::Code));
        assert_eq!(practice.mode, Mode::Words);
        let quote = solo(&["--language", "french"], &defaults_with_mode(Mode::Quote));
        assert_eq!(quote.mode, Mode::Quote);
    }

    #[test]
    fn count_flags_imply_their_mode() {
        let saved = defaults_with_mode(Mode::Quote);
        assert_eq!(solo(&["--seconds", "15"], &saved).mode, Mode::Time);
        assert_eq!(solo(&["--words", "10"], &saved).mode, Mode::Words);
        assert_eq!(
            solo(&["--words", "10", "--mode", "quote"], &saved).mode,
            Mode::Quote
        );
    }

    #[test]
    fn language_must_match_the_mode() {
        let defaults = Practice::default();
        assert_eq!(
            launch(
                &["solo", "--mode", "words", "--language", "rust"],
                &defaults
            ),
            Err(CliError::CodeLanguageOutsideCodeMode(CodeLanguage::Rust))
        );
        assert_eq!(
            launch(
                &["solo", "--mode", "code", "--language", "french"],
                &defaults
            ),
            Err(CliError::NaturalLanguageInCodeMode(Language::French))
        );
    }

    #[test]
    fn unknown_language_lists_the_choices() {
        let error = launch(&["solo", "--language", "klingon"], &Practice::default())
            .expect_err("unknown language");
        assert_eq!(error, CliError::UnknownLanguage("klingon".to_owned()));
        assert_eq!(
            error.to_string(),
            "unknown language `klingon`, expected one of: english, french, rust, python, typescript, javascript, sql"
        );
    }

    #[test]
    fn solo_counts_must_be_in_range() {
        let defaults = Practice::default();
        let error = |arguments: &[&str]| {
            launch(arguments, &defaults)
                .expect_err("out of range")
                .to_string()
        };
        assert_eq!(
            error(&["solo", "--words", "0"]),
            "--words 0 is out of range, use 1 to 500"
        );
        assert_eq!(
            error(&["solo", "--seconds", "601"]),
            "--seconds 601 is out of range, use 5 to 600"
        );
        assert_eq!(
            error(&["solo", "--seconds", "4"]),
            "--seconds 4 is out of range, use 5 to 600"
        );
        assert_eq!(solo(&["--words", "500"], &defaults).word_count, 500);
        assert_eq!(solo(&["--seconds", "5"], &defaults).duration, 5);
    }

    #[test]
    fn seconds_take_precedence_over_words_without_a_mode() {
        let practice = solo(&["--words", "25", "--seconds", "60"], &Practice::default());
        assert_eq!(practice.mode, Mode::Time);
        assert_eq!((practice.word_count, practice.duration), (25, 60));
    }

    #[test]
    fn flags_cannot_switch_off_saved_options() {
        let saved = Practice {
            punctuation: true,
            ..Practice::default()
        };
        let practice = solo(&["--numbers"], &saved);
        assert!(practice.punctuation && practice.numbers);
    }

    #[test]
    fn explicit_code_mode_takes_language_aliases() {
        let practice = solo(
            &["--mode", "code", "--language", "PY"],
            &Practice::default(),
        );
        assert_eq!(
            (practice.mode, practice.code_language),
            (Mode::Code, CodeLanguage::Python)
        );
    }

    #[test]
    fn races_check_the_language_against_the_mode_too() {
        let saved = defaults_with_mode(Mode::Code);
        assert_eq!(
            create(&["--mode", "code", "--language", "fr"], &saved),
            Err(CliError::NaturalLanguageInCodeMode(Language::French))
        );
        assert_eq!(
            create(&["--language", "fr"], &saved),
            Ok(TextSource::Words {
                language: Language::French,
                count: 50,
                options: WordOptions::default(),
            })
        );
    }

    #[test]
    fn solo_file_is_passed_through() {
        let launched = launch(&["solo", "--file", "src/main.rs"], &Practice::default());
        assert_eq!(
            launched,
            Ok(Launch::Solo {
                practice: Practice::default(),
                file: Some(PathBuf::from("src/main.rs")),
            })
        );
    }

    #[test]
    fn create_builds_the_race_text_from_defaults_and_flags() {
        let saved = Practice {
            language: Language::French,
            word_count: 30,
            ..Practice::default()
        };
        assert_eq!(
            create(&["--punctuation"], &saved),
            Ok(TextSource::Words {
                language: Language::French,
                count: 30,
                options: WordOptions {
                    punctuation: true,
                    numbers: false
                },
            })
        );
        assert_eq!(
            create(&["--language", "ts"], &saved),
            Ok(TextSource::Code {
                language: CodeLanguage::TypeScript
            })
        );
        assert_eq!(
            create(&["--mode", "quote"], &saved),
            Ok(TextSource::Quote {
                language: Language::French
            })
        );
    }

    #[test]
    fn races_refuse_time_mode() {
        let error = create(&["--mode", "time"], &Practice::default()).expect_err("time race");
        assert_eq!(error, CliError::TimeModeInRace);
        assert_eq!(error.to_string(), "time mode is only available solo");
    }

    #[test]
    fn rooms_are_created_with_the_saved_race_settings() {
        let saved = Config {
            practice: defaults_with_mode(Mode::Code),
            race: Practice {
                mode: Mode::Quote,
                language: Language::French,
                ..Practice::default()
            },
            ..Config::default()
        };
        let cli = Cli::try_parse_from(["code-racer", "create"]).expect("valid arguments");
        assert_eq!(
            cli.launch(&saved),
            Ok(Launch::Create(TextSource::Quote {
                language: Language::French
            }))
        );
    }

    #[test]
    fn a_saved_time_mode_races_on_words() {
        let saved = defaults_with_mode(Mode::Time);
        assert!(matches!(
            create(&[], &saved),
            Ok(TextSource::Words { count: 50, .. })
        ));
    }

    #[test]
    fn race_word_counts_are_bounded() {
        let defaults = Practice::default();
        assert_eq!(
            create(&["--words", "4"], &defaults),
            Err(CliError::OutOfRange {
                flag: "words",
                value: 4,
                min: 5,
                max: 200
            })
        );
        assert!(create(&["--words", "201"], &defaults).is_err());
        let saved = Practice {
            word_count: 500,
            ..defaults
        };
        assert!(matches!(
            create(&[], &saved),
            Ok(TextSource::Words { count: 200, .. })
        ));
    }

    #[test]
    fn join_accepts_lowercase_codes_and_rejects_invalid_ones() {
        assert_eq!(
            launch(&["join", "fk72ad"], &Practice::default()),
            Ok(Launch::Join("FK72AD".parse().expect("code")))
        );
        assert!(Cli::try_parse_from(["code-racer", "join", "FK72A0"]).is_err());
        assert!(Cli::try_parse_from(["code-racer", "join"]).is_err());
    }

    #[test]
    fn multiplayer_opens_the_menu() {
        assert_eq!(
            launch(&["multiplayer"], &Practice::default()),
            Ok(Launch::Multiplayer)
        );
    }

    #[test]
    fn create_help_only_offers_raceable_modes() {
        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("create")
            .expect("create subcommand")
            .render_help()
            .to_string();
        assert!(help.contains("words, quote or code"), "{help}");
        assert!(!help.contains("time"), "{help}");
    }

    #[test]
    fn every_subcommand_and_flag_is_documented() {
        let mut command = Cli::command();
        command.build();
        for command in std::iter::once(&command).chain(command.get_subcommands()) {
            let name = command.get_name();
            assert!(command.get_about().is_some(), "{name} has no description");
            for argument in command.get_arguments() {
                let id = argument.get_id();
                assert!(
                    argument.get_help().is_some(),
                    "{name} {id} has no description"
                );
            }
        }
    }
}
