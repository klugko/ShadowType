use std::ops::RangeInclusive;

use code_racer_engine::{CodeLanguage, Language, TextSource, WORD_COUNTS};
use code_racer_protocol::RACE_WORD_COUNTS;

use super::{Cli, CliError, Command, Launch, RoomArgs, SoloArgs, TextArgs};
use crate::config::{Config, Mode, Practice};

impl Cli {
    /**
     * Resolves the screen to open, layering the flags over the saved solo
     * or race settings.
     */
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
    /**
     * Time mode cannot be raced: it is refused when asked for explicitly and
     * replaced by words mode when it is only the saved default.
     */
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

    /**
     * Without `--mode`, the other flags tell which mode is meant: a
     * programming language means code, `--seconds` time and `--words` words.
     */
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
