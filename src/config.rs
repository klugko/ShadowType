//! User settings stored in `config.toml`.
//!
//! The file is meant to be edited by hand too: missing keys take their
//! default, and saving changes only the values of the settings, so comments
//! and keys this version does not know are kept. A file that cannot be parsed
//! is moved aside rather than overwritten.

use std::{
    fmt, io,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use code_racer_engine::{CodeLanguage, Language, TextSource, WORD_COUNTS, WordOptions};
use code_racer_protocol::{RACE_WORD_COUNTS, Username};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, TableLike, Value};

pub use crate::persist::Loaded;
use crate::persist::{self, Recovered};

/// Color scheme of the interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// The default code editor look.
    #[default]
    Editor,
    /// A dark background whatever the terminal's own.
    Dark,
    /// Monochrome, for terminals without colors.
    Mono,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::Editor, Self::Dark, Self::Mono];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Dark => "dark",
            Self::Mono => "mono",
        }
    }
}

/// What a practice session asks to type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A fixed number of common words.
    #[default]
    Words,
    /// As many words as possible before the timer runs out, solo only.
    Time,
    /// A passage of literature.
    Quote,
    /// A snippet of source code.
    Code,
}

impl Mode {
    pub const ALL: [Self; 4] = [Self::Words, Self::Time, Self::Quote, Self::Code];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Words => "words",
            Self::Time => "time",
            Self::Quote => "quote",
            Self::Code => "code",
        }
    }
}

macro_rules! display_by_name {
    ($($kind:ty),+) => {$(
        impl fmt::Display for $kind {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.name())
            }
        }
    )+};
}

display_by_name!(Theme, Mode);

/// Settings of the last solo session or race, reused as the next defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Practice {
    #[serde(rename = "default_mode")]
    pub mode: Mode,
    pub language: Language,
    pub code_language: CodeLanguage,
    pub word_count: u16,
    /// Length of a time session, in seconds.
    pub duration: u16,
    pub punctuation: bool,
    pub numbers: bool,
}

impl Practice {
    /// Word counts offered by the forms, solo and in races. Other counts in
    /// [`WORD_COUNTS`], or [`RACE_WORD_COUNTS`] for races, can be set by hand.
    pub const WORD_COUNT_PRESETS: [u16; 4] = [10, 25, 50, 100];
    /// Time session lengths offered by the forms, in seconds.
    pub const DURATION_PRESETS: [u16; 4] = [15, 30, 60, 120];
    /// Accepted time session lengths, in seconds.
    pub const DURATION_LIMITS: RangeInclusive<u16> = 5..=600;

    /// Brings hand-edited counts back into the supported ranges.
    pub fn sanitized(self) -> Self {
        Self {
            word_count: clamp_to(self.word_count, &WORD_COUNTS),
            duration: clamp_to(self.duration, &Self::DURATION_LIMITS),
            ..self
        }
    }

    /// The same settings for a race: time mode becomes words mode, which
    /// races can use, and the word count is brought into [`RACE_WORD_COUNTS`].
    pub fn for_race(self) -> Self {
        Self {
            mode: match self.mode {
                Mode::Time => Mode::Words,
                mode => mode,
            },
            word_count: clamp_to(self.word_count, &RACE_WORD_COUNTS),
            ..self
        }
    }

    /// The finite text these settings describe, `None` in time mode where
    /// words keep coming until the timer runs out.
    pub fn text_source(&self) -> Option<TextSource> {
        (self.mode != Mode::Time).then(|| self.text_in(self.mode))
    }

    /// The text of a race created with these settings, which
    /// [`Practice::for_race`] makes raceable.
    pub fn race_text_source(self) -> TextSource {
        let race = self.for_race();
        race.text_in(race.mode)
    }

    /// The text of `mode`, where a timed session types words.
    fn text_in(&self, mode: Mode) -> TextSource {
        match mode {
            Mode::Words | Mode::Time => TextSource::Words {
                language: self.language,
                count: self.word_count,
                options: self.word_options(),
            },
            Mode::Quote => TextSource::Quote {
                language: self.language,
            },
            Mode::Code => TextSource::Code {
                language: self.code_language,
            },
        }
    }

    pub const fn word_options(&self) -> WordOptions {
        WordOptions {
            punctuation: self.punctuation,
            numbers: self.numbers,
        }
    }
}

impl Default for Practice {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            language: Language::default(),
            code_language: CodeLanguage::default(),
            word_count: 50,
            duration: 30,
            punctuation: false,
            numbers: false,
        }
    }
}

pub(crate) fn clamp_to(value: u16, range: &RangeInclusive<u16>) -> u16 {
    value.clamp(*range.start(), *range.end())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Multiplayer {
    /// Address of the race server, normalised by `network::server_url` when used.
    pub server: String,
}

impl Multiplayer {
    pub const DEFAULT_SERVER: &'static str = "ws://127.0.0.1:8080";
}

impl Default for Multiplayer {
    fn default() -> Self {
        Self {
            server: Self::DEFAULT_SERVER.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub username: String,
    pub theme: Theme,
    /// Settings of solo sessions, kept at the top level of the file as in
    /// earlier versions.
    #[serde(flatten)]
    pub practice: Practice,
    /// Settings of the races this player creates, in a `[race]` table. They
    /// are always raceable once loaded: see [`Practice::for_race`].
    pub race: Practice,
    pub multiplayer: Multiplayer,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: String::new(),
            theme: Theme::default(),
            practice: Practice::default(),
            race: Practice::default().for_race(),
            multiplayer: Multiplayer::default(),
        }
    }
}

impl Config {
    /// The configured name, `None` when it is missing or invalid.
    pub fn username(&self) -> Option<Username> {
        self.username.parse().ok()
    }

    /// Takes from `after` every setting that differs from `before`, and
    /// keeps the others.
    ///
    /// `after` is taken apart field by field, so that a setting added later
    /// cannot be forgotten here without a compilation error.
    pub fn adopt_changes(&mut self, before: &Self, after: &Self) {
        let Self {
            username,
            theme,
            practice,
            race,
            multiplayer: Multiplayer { server },
        } = after;
        adopt(&mut self.username, &before.username, username);
        adopt(&mut self.theme, &before.theme, theme);
        self.practice.adopt_changes(&before.practice, practice);
        self.race.adopt_changes(&before.race, race);
        adopt(
            &mut self.multiplayer.server,
            &before.multiplayer.server,
            server,
        );
    }
}

impl Practice {
    /// See [`Config::adopt_changes`].
    fn adopt_changes(&mut self, before: &Self, after: &Self) {
        let Self {
            mode,
            language,
            code_language,
            word_count,
            duration,
            punctuation,
            numbers,
        } = after;
        adopt(&mut self.mode, &before.mode, mode);
        adopt(&mut self.language, &before.language, language);
        adopt(
            &mut self.code_language,
            &before.code_language,
            code_language,
        );
        adopt(&mut self.word_count, &before.word_count, word_count);
        adopt(&mut self.duration, &before.duration, duration);
        adopt(&mut self.punctuation, &before.punctuation, punctuation);
        adopt(&mut self.numbers, &before.numbers, numbers);
    }
}

fn adopt<T: Clone + PartialEq>(kept: &mut T, before: &T, after: &T) {
    if before != after {
        kept.clone_from(after);
    }
}

/// Where code-racer keeps its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_file: PathBuf,
    pub history_file: PathBuf,
    pub log_file: PathBuf,
}

impl Paths {
    /// Platform directories for the current user, `None` when the home
    /// directory cannot be determined.
    pub fn discover() -> Option<Self> {
        let directories = ProjectDirs::from("", "", "code-racer")?;
        let data = directories.data_local_dir();
        Some(Self {
            config_file: directories.config_dir().join("config.toml"),
            history_file: data.join("history.json"),
            log_file: directories
                .state_dir()
                .unwrap_or(data)
                .join("code-racer.log"),
        })
    }
}

/// Reads the configuration. A missing file gives the defaults, and an
/// invalid one is moved aside, explained in the warning, and gives the
/// defaults too. `None` means the file could be neither used nor moved
/// aside: it is still there, and must not be written over.
pub fn load_config(path: &Path) -> Loaded<Option<Config>> {
    persist::read_or_recover(path, "defaults loaded", parse_config).map(|found| match found {
        Recovered::Parsed(config) => Some(config),
        Recovered::Absent => Some(Config::default()),
        Recovered::LeftInPlace => None,
    })
}

/// Applies `change` to the settings saved at `path`, as the file holds them
/// now, and writes them back when they changed. The values are updated in
/// place, so that the comments, layout and unknown keys of the file survive.
///
/// A file that cannot be read, or that does not hold valid settings, is
/// refused rather than overwritten: it is what failed to load, and the
/// settings in memory are only defaults. Other instances of code-racer wait
/// while the file is read and replaced, so that none undoes another's save.
pub fn update_config(path: &Path, change: impl FnOnce(&mut Config)) -> io::Result<()> {
    let _lock = persist::lock(path)?;
    let existing = persist::read_existing(path)?.unwrap_or_default();
    let saved = parse_config(&existing).map_err(|problem| persist::invalid_contents(&problem))?;
    let mut updated = saved.clone();
    change(&mut updated);
    if updated == saved {
        return Ok(());
    }
    let contents = updated_document(&existing, &updated)?;
    persist::write_atomically(path, contents.as_bytes())
}

fn updated_document(existing: &str, config: &Config) -> io::Result<String> {
    let mut document: DocumentMut = existing.parse().map_err(io::Error::other)?;
    let settings: DocumentMut = toml::to_string(config)
        .map_err(io::Error::other)?
        .parse()
        .map_err(io::Error::other)?;
    update_table(document.as_table_mut(), settings.as_table());
    Ok(document.to_string())
}

/// Copies every setting into `table`, inserting the missing ones and
/// leaving keys that are not settings alone.
fn update_table(table: &mut dyn TableLike, settings: &dyn TableLike) {
    for (key, setting) in settings.iter() {
        match table.get_mut(key) {
            Some(item) => update_item(item, setting),
            None => {
                table.insert(key, setting.clone());
            }
        }
    }
}

fn update_item(item: &mut Item, setting: &Item) {
    if let (Some(table), Some(settings)) = (item.as_table_like_mut(), setting.as_table_like()) {
        update_table(table, settings);
    } else if let (Some(value), Some(new_value)) = (item.as_value_mut(), setting.as_value()) {
        update_value(value, new_value);
    } else {
        *item = setting.clone();
    }
}

/// Replaces a changed value, keeping the whitespace and comment around it.
/// An unchanged one keeps its spelling too, such as single quotes.
fn update_value(value: &mut Value, new_value: &Value) {
    if !same_setting(value, new_value) {
        let decor = value.decor().clone();
        *value = new_value.clone();
        *value.decor_mut() = decor;
    }
}

/// Settings are only ever strings, integers or booleans.
fn same_setting(value: &Value, other: &Value) -> bool {
    match (value, other) {
        (Value::String(value), Value::String(other)) => value.value() == other.value(),
        (Value::Integer(value), Value::Integer(other)) => value.value() == other.value(),
        (Value::Boolean(value), Value::Boolean(other)) => value.value() == other.value(),
        _ => false,
    }
}

fn parse_config(contents: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(contents).map_err(|error| describe(&error, contents))?;
    Ok(Config {
        practice: config.practice.sanitized(),
        race: config.race.sanitized().for_race(),
        ..config
    })
}

fn describe(error: &toml::de::Error, contents: &str) -> String {
    let message = error
        .message()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    match error.span() {
        Some(span) => {
            let line = contents
                .bytes()
                .take(span.start)
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            format!("line {line}: {message}")
        }
        None => message,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;

    use super::*;
    use crate::persist::scratch::{TempDir, occupy_every_backup};

    /// The settings loaded from `path`, defaults when the file was left in place.
    pub(crate) fn load(path: &Path) -> Loaded<Config> {
        load_config(path).map(Option::unwrap_or_default)
    }

    const DOCUMENTED_EXAMPLE: &str = r#"username = "Jean"
language = "french"
theme = "dark"
default_mode = "words"
word_count = 50
[multiplayer]
server = "ws://127.0.0.1:8080"
"#;

    /// Saves every setting of `config` over those of the file.
    fn save(path: &Path, config: &Config) -> io::Result<()> {
        update_config(path, |saved| saved.clone_from(config))
    }

    fn write_config(dir: &TempDir, contents: &str) -> PathBuf {
        let path = dir.join("config.toml");
        fs::write(&path, contents).expect("write config");
        path
    }

    #[test]
    fn documented_example_is_understood() {
        let dir = TempDir::new();
        let loaded = load(&write_config(&dir, DOCUMENTED_EXAMPLE));

        assert_eq!(
            loaded,
            Loaded::clean(Config {
                username: "Jean".to_owned(),
                theme: Theme::Dark,
                practice: Practice {
                    mode: Mode::Words,
                    language: Language::French,
                    word_count: 50,
                    ..Practice::default()
                },
                race: Practice::default().for_race(),
                multiplayer: Multiplayer {
                    server: "ws://127.0.0.1:8080".to_owned(),
                },
            })
        );
        assert_eq!(
            loaded.value.username().map(String::from),
            Some("Jean".to_owned())
        );
    }

    #[test]
    fn documented_example_survives_a_save() {
        let dir = TempDir::new();
        let path = write_config(&dir, DOCUMENTED_EXAMPLE);
        let changed = Config {
            theme: Theme::Mono,
            ..load(&path).value
        };

        save(&path, &changed).expect("save");

        assert_eq!(load(&path), Loaded::clean(changed));
    }

    #[test]
    fn saving_keeps_comments_spelling_and_unknown_keys() {
        let dir = TempDir::new();
        let path = write_config(
            &dir,
            "# code-racer settings\n\
             username = 'Jean'  # shown to other racers\n\
             theme = \"dark\"     # editor, dark or mono\n\
             default_mode = \"words\"\n\
             future_key = true\n\
             \n\
             [multiplayer]\n\
             # the LAN server\n\
             server = \"ws://127.0.0.1:8080\"\n\
             retries = 3\n",
        );
        let config = Config {
            theme: Theme::Mono,
            ..load(&path).value
        };

        save(&path, &config).expect("save");

        let saved = fs::read_to_string(&path).expect("read");
        for line in [
            "# code-racer settings",
            "username = 'Jean'  # shown to other racers",
            "theme = \"mono\"     # editor, dark or mono",
            "future_key = true",
            "# the LAN server",
            "retries = 3",
        ] {
            assert!(
                saved.lines().any(|saved| saved == line),
                "{line} in {saved}"
            );
        }
        assert_eq!(load(&path), Loaded::clean(config));
    }

    #[test]
    fn discovered_paths_use_the_documented_file_names() {
        let Some(paths) = Paths::discover() else {
            return;
        };
        let name = |path: &Path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        };
        assert_eq!(name(&paths.config_file).as_deref(), Some("config.toml"));
        assert_eq!(name(&paths.history_file).as_deref(), Some("history.json"));
        assert_eq!(name(&paths.log_file).as_deref(), Some("code-racer.log"));
    }

    #[test]
    fn missing_keys_default_and_unknown_keys_are_ignored() {
        let dir = TempDir::new();
        let contents = "editor = \"vim\"\ndefault_mode = \"code\"\n[multiplayer]\nretries = 3\n";
        let loaded = load(&write_config(&dir, contents));

        assert_eq!(loaded.warning, None);
        assert_eq!(
            loaded.value,
            Config {
                practice: Practice {
                    mode: Mode::Code,
                    ..Practice::default()
                },
                ..Config::default()
            }
        );
    }

    #[test]
    fn missing_file_gives_defaults_without_creating_it() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        assert_eq!(load(&path), Loaded::clean(Config::default()));
        assert!(!path.exists());
    }

    #[test]
    fn saved_settings_load_back_identically() {
        let dir = TempDir::new();
        let path = dir.join("settings/config.toml");
        let config = Config {
            username: "Élodie".to_owned(),
            theme: Theme::Mono,
            practice: Practice {
                mode: Mode::Time,
                language: Language::French,
                code_language: CodeLanguage::Sql,
                word_count: 25,
                duration: 120,
                punctuation: true,
                numbers: true,
            },
            race: Practice {
                mode: Mode::Code,
                language: Language::English,
                code_language: CodeLanguage::Rust,
                word_count: 200,
                duration: 30,
                punctuation: false,
                numbers: true,
            },
            multiplayer: Multiplayer {
                server: "ws://192.168.1.20:9000".to_owned(),
            },
        };

        save(&path, &config).expect("save");

        assert_eq!(load(&path), Loaded::clean(config));
    }

    #[test]
    fn saved_file_keeps_the_documented_key_names() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let config = Config {
            username: "Jean".to_owned(),
            ..Config::default()
        };
        save(&path, &config).expect("save");
        let saved = fs::read_to_string(&path).expect("read");

        for line in [
            "default_mode = \"words\"",
            "word_count = 50",
            "theme = \"editor\"",
            "[race]",
            "[multiplayer]",
        ] {
            assert!(
                saved.lines().any(|saved_line| saved_line == line),
                "{line} in {saved}"
            );
        }
    }

    #[test]
    fn invalid_toml_is_backed_up_and_replaced_by_defaults() {
        let dir = TempDir::new();
        let path = write_config(&dir, "username = \"Jean\"\ntheme = \n");

        let loaded = load(&path);

        assert_eq!(loaded.value, Config::default());
        let warning = loaded.warning.expect("warning");
        let backup = dir.join("config.toml.bak");
        assert!(
            warning.starts_with("config.toml was invalid (line 2: "),
            "{warning}"
        );
        assert!(
            warning.ends_with(&format!(
                "); defaults loaded, backup at {}",
                backup.display()
            )),
            "{warning}"
        );
        assert_eq!(
            fs::read_to_string(backup).expect("backup"),
            "username = \"Jean\"\ntheme = \n"
        );
        assert!(!path.exists());
    }

    #[test]
    fn invalid_file_that_cannot_be_backed_up_is_never_overwritten() {
        let dir = TempDir::new();
        let path = write_config(&dir, "username = \"Jean\"\ntheme = \"solarized\"\n");
        occupy_every_backup(&path);

        let loaded = load(&path);
        let saved = save(&path, &loaded.value);

        assert_eq!(loaded.value, Config::default());
        assert!(loaded.warning.is_some());
        let error = saved.expect_err("the invalid file is kept");
        assert!(
            error
                .to_string()
                .starts_with("the file is invalid (line 2: "),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("read"),
            "username = \"Jean\"\ntheme = \"solarized\"\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_that_cannot_be_read_is_never_overwritten() {
        let dir = TempDir::new();
        let path = write_config(&dir, DOCUMENTED_EXAMPLE);
        if !crate::persist::scratch::make_unreadable(&path) {
            return;
        }

        let loaded = load(&path);
        let saved = save(&path, &loaded.value);

        assert_eq!(loaded.value, Config::default());
        let warning = loaded.warning.expect("warning");
        assert!(
            warning.starts_with("cannot read config.toml ("),
            "{warning}"
        );
        assert!(saved.is_err());
        assert_eq!(
            crate::persist::scratch::read_unreadable(&path),
            DOCUMENTED_EXAMPLE
        );
    }

    #[test]
    fn unknown_theme_names_the_offending_line() {
        let dir = TempDir::new();
        let path = write_config(&dir, "username = \"Jean\"\n\ntheme = \"solarized\"\n");

        let warning = load(&path).warning.expect("warning");

        assert!(warning.contains("(line 3: "), "{warning}");
        assert!(warning.contains("solarized"), "{warning}");
        assert!(!warning.contains('\n'), "{warning}");
    }

    #[test]
    fn out_of_range_counts_are_clamped_on_load() {
        let dir = TempDir::new();
        let path = write_config(&dir, "word_count = 9000\nduration = 1\n");

        let practice = load(&path).value.practice;

        assert_eq!(practice.word_count, *WORD_COUNTS.end());
        assert_eq!(practice.duration, *Practice::DURATION_LIMITS.start());
    }

    #[test]
    fn race_settings_are_kept_apart_from_solo_settings() {
        let dir = TempDir::new();
        let contents = "default_mode = \"time\"\nword_count = 10\n\
                        [race]\ndefault_mode = \"quote\"\nlanguage = \"french\"\n";

        let config = load(&write_config(&dir, contents)).value;

        assert_eq!(
            (config.practice.mode, config.practice.word_count),
            (Mode::Time, 10)
        );
        assert_eq!(
            config.race,
            Practice {
                mode: Mode::Quote,
                language: Language::French,
                ..Practice::default().for_race()
            }
        );
    }

    #[test]
    fn hand_edited_race_settings_are_made_raceable() {
        let dir = TempDir::new();
        let contents = "[race]\ndefault_mode = \"time\"\nword_count = 1000\nduration = 1\n";

        let race = load(&write_config(&dir, contents)).value.race;

        assert_eq!(race.mode, Mode::Words);
        assert_eq!(race.word_count, *RACE_WORD_COUNTS.end());
        assert_eq!(race.duration, *Practice::DURATION_LIMITS.start());
        assert!(
            race.text_source()
                .is_some_and(|text| code_racer_protocol::is_raceable(&text))
        );
    }

    #[test]
    fn missing_race_settings_default_to_the_solo_defaults_made_raceable() {
        let dir = TempDir::new();
        let path = write_config(&dir, DOCUMENTED_EXAMPLE);

        assert_eq!(load(&path).value.race, Practice::default().for_race());
    }

    #[test]
    fn sanitizing_keeps_values_already_in_range() {
        let practice = Practice {
            word_count: 0,
            duration: 10_000,
            ..Practice::default()
        }
        .sanitized();
        assert_eq!(practice.word_count, *WORD_COUNTS.start());
        assert_eq!(practice.duration, *Practice::DURATION_LIMITS.end());

        let valid = Practice {
            word_count: 100,
            duration: 15,
            ..Practice::default()
        };
        assert_eq!(valid.sanitized(), valid);
    }

    #[test]
    fn race_settings_never_use_time_mode_and_bound_the_count() {
        let solo = Practice {
            mode: Mode::Time,
            word_count: 500,
            language: Language::French,
            ..Practice::default()
        };
        let race = solo.for_race();
        assert_eq!(race.mode, Mode::Words);
        assert_eq!(race.word_count, *RACE_WORD_COUNTS.end());
        assert_eq!(race.language, Language::French);
        assert!(code_racer_protocol::is_raceable(&solo.race_text_source()));

        let quote = Practice {
            mode: Mode::Quote,
            word_count: 1,
            ..Practice::default()
        };
        assert_eq!(quote.for_race().mode, Mode::Quote);
        assert_eq!(quote.for_race().word_count, *RACE_WORD_COUNTS.start());
    }

    #[test]
    fn every_race_text_is_raceable() {
        for mode in Mode::ALL {
            for word_count in [0, 1, 30, 1_000] {
                let settings = Practice {
                    mode,
                    word_count,
                    ..Practice::default()
                };
                assert!(code_racer_protocol::is_raceable(
                    &settings.race_text_source()
                ));
            }
        }
    }

    #[test]
    fn presets_are_within_the_solo_and_race_limits() {
        for preset in Practice::WORD_COUNT_PRESETS {
            assert!(WORD_COUNTS.contains(&preset), "{preset}");
            assert!(RACE_WORD_COUNTS.contains(&preset), "{preset}");
        }
        for preset in Practice::DURATION_PRESETS {
            assert!(Practice::DURATION_LIMITS.contains(&preset), "{preset}");
        }
    }

    #[test]
    fn only_changed_settings_are_adopted() {
        let kept = Config {
            theme: Theme::Dark,
            ..Config::default()
        };
        let before = Config {
            theme: Theme::Mono,
            practice: Practice {
                mode: Mode::Code,
                ..Practice::default()
            },
            ..Config::default()
        };
        let after = Config {
            username: "Ada".to_owned(),
            practice: Practice {
                word_count: 25,
                ..before.practice
            },
            ..before.clone()
        };
        let mut adopted = kept.clone();
        adopted.adopt_changes(&before, &after);
        assert_eq!(adopted.username, "Ada");
        assert_eq!(adopted.theme, Theme::Dark, "unchanged since before");
        assert_eq!(adopted.practice.mode, Mode::Words, "unchanged since before");
        assert_eq!(adopted.practice.word_count, 25);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_left_in_place_is_not_writable() {
        let dir = TempDir::new();
        let path = write_config(&dir, DOCUMENTED_EXAMPLE);
        if !crate::persist::scratch::make_unreadable(&path) {
            return;
        }
        let loaded = load_config(&path);
        assert_eq!(loaded.value, None);
        assert!(loaded.warning.is_some());
        crate::persist::scratch::read_unreadable(&path);
    }

    #[test]
    fn username_is_only_given_when_valid() {
        let named = |username: &str| Config {
            username: username.to_owned(),
            ..Config::default()
        };
        assert_eq!(named("").username(), None);
        assert_eq!(named("   ").username(), None);
        assert_eq!(named("Bob\u{7}").username(), None);
        assert_eq!(named(&"x".repeat(25)).username(), None);
        assert_eq!(
            named("  Jean ").username().map(String::from),
            Some("Jean".to_owned())
        );
    }

    #[test]
    fn text_source_follows_the_mode() {
        let practice = Practice {
            language: Language::French,
            code_language: CodeLanguage::Python,
            word_count: 25,
            punctuation: true,
            ..Practice::default()
        };
        let with_mode = |mode| Practice { mode, ..practice }.text_source();

        assert_eq!(
            with_mode(Mode::Words),
            Some(TextSource::Words {
                language: Language::French,
                count: 25,
                options: WordOptions {
                    punctuation: true,
                    numbers: false,
                },
            })
        );
        assert_eq!(with_mode(Mode::Time), None);
        assert_eq!(
            with_mode(Mode::Quote),
            Some(TextSource::Quote {
                language: Language::French
            })
        );
        assert_eq!(
            with_mode(Mode::Code),
            Some(TextSource::Code {
                language: CodeLanguage::Python
            })
        );
    }

    #[test]
    fn names_match_the_serialized_and_command_line_forms() {
        for theme in Theme::ALL {
            let value = theme.to_possible_value().expect("possible value");
            assert_eq!(value.get_name(), theme.name());
            assert_eq!(Theme::from_str(theme.name(), true), Ok(theme));
        }
        for mode in Mode::ALL {
            let value = mode.to_possible_value().expect("possible value");
            assert_eq!(value.get_name(), mode.to_string());
        }
    }
}
