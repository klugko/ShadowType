//! Solo practice: the settings form and the session being typed.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use code_racer_engine::{
    CodeLanguage, Language, Sample, SessionOptions, Stats, Status, TextSource, TypingSession,
    WordOptions, WordStream, consistency, normalize,
};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::{
        form::{Row, Step},
        text_settings::{self, TextSetting},
    },
    config::{Mode, Practice},
    history::{History, Record},
};

const TIMED_INITIAL_WORDS: usize = 80;
const TIMED_REFILL_WORDS: usize = 40;
const TIMED_REFILL_BELOW: usize = 120;
const MAX_FILE_GRAPHEMES: usize = 3_000;
/// Bytes read from a file at most: plenty for [`MAX_FILE_GRAPHEMES`], and
/// a huge file or an endless device cannot freeze the interface.
const MAX_FILE_BYTES: usize = 64 * 1024;

/// A line of `practice.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Text(TextSetting),
    Start,
}

/// Lines of the practice form for the current mode.
pub fn fields(practice: &Practice) -> Vec<Field> {
    text_settings::settings(practice)
        .into_iter()
        .map(Field::Text)
        .chain([Field::Start])
        .collect()
}

pub fn row(practice: &Practice, field: Field) -> Row {
    match field {
        Field::Text(setting) => text_settings::row(practice, setting, &Mode::ALL),
        Field::Start => Row::action("start session"),
    }
}

pub fn adjust(practice: &mut Practice, field: Field, step: Step) {
    if let Field::Text(setting) = field {
        text_settings::adjust(practice, setting, step, &Mode::ALL);
    }
}

/// What a solo session is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Text(TextSource),
    Timed {
        language: Language,
        options: WordOptions,
        seconds: u16,
    },
    File(CustomText),
}

/// The content of a file chosen with `:edit` or `--file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomText {
    pub name: String,
    pub text: String,
    pub language: Option<CodeLanguage>,
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("cannot read {path}: {source}")]
    Unreadable { path: String, source: io::Error },
    #[error("{0} is not a regular file")]
    NotAFile(String),
    #[error("{0} is not a text file")]
    Binary(String),
    #[error("{0} is empty")]
    Empty(String),
}

impl CustomText {
    /// Loads a file, keeping at most the first few thousand characters, cut at a line break.
    pub fn load(path: &Path) -> Result<Self, FileError> {
        let display = path.display().to_string();
        let bytes = read_start(path, &display)?;
        let raw = decode(&bytes).ok_or_else(|| FileError::Binary(display.clone()))?;
        let normalized = normalize(raw);
        let text = truncate_at_line(&normalized, MAX_FILE_GRAPHEMES);
        if text.is_empty() {
            return Err(FileError::Empty(display));
        }
        Ok(Self {
            name: file_name(path),
            text: text.to_owned(),
            language: path
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(CodeLanguage::from_extension),
        })
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| PathBuf::from("untitled"), PathBuf::from)
        .display()
        .to_string()
}

/// The first [`MAX_FILE_BYTES`] of a regular file, cut after a line break
/// when the file is longer.
///
/// The type is checked before opening: opening a FIFO waits for a writer,
/// and reading a terminal or `/dev/zero` never ends.
fn read_start(path: &Path, display: &str) -> Result<Vec<u8>, FileError> {
    let unreadable = |source| FileError::Unreadable {
        path: display.to_owned(),
        source,
    };
    if !fs::metadata(path).map_err(unreadable)?.is_file() {
        return Err(FileError::NotAFile(display.to_owned()));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(unreadable)?;
    if bytes.len() > MAX_FILE_BYTES {
        let cut = bytes[..MAX_FILE_BYTES]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .unwrap_or(MAX_FILE_BYTES);
        bytes.truncate(cut);
    }
    Ok(bytes)
}

/// The text in `bytes`, `None` when they are not text. A character cut in
/// two by [`read_start`] at the end is dropped rather than making a valid
/// file look binary.
fn decode(bytes: &[u8]) -> Option<&str> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    (!text.contains('\0')).then_some(text)
}

/// The first `limit` characters of `text`, cut after the last line break
/// among them when the text is longer.
fn truncate_at_line(text: &str, limit: usize) -> &str {
    let Some((end, _)) = text.grapheme_indices(true).nth(limit) else {
        return text;
    };
    let kept = &text[..end];
    kept.rfind('\n').map_or(kept, |cut| &kept[..cut])
}

impl Plan {
    pub fn from_practice(practice: &Practice) -> Self {
        practice.text_source().map_or(
            Self::Timed {
                language: practice.language,
                options: practice.word_options(),
                seconds: practice.duration,
            },
            Self::Text,
        )
    }

    /// File name shown in the editor for this session.
    pub fn title(&self) -> String {
        match self {
            Self::Text(TextSource::Words { .. }) => "notes.md".to_owned(),
            Self::Timed { .. } => "scratch.txt".to_owned(),
            Self::Text(TextSource::Quote { .. }) => "README.md".to_owned(),
            Self::Text(TextSource::Code { language }) => code_file_name(*language),
            Self::File(custom) => custom.name.clone(),
        }
    }

    /// Short description such as `words 50 · english · punctuation`.
    pub fn label(&self) -> String {
        let mut parts = vec![self.mode_label(), self.language_label()];
        if let Self::Text(TextSource::Words { options, .. }) | Self::Timed { options, .. } = self {
            if options.punctuation {
                parts.push("punctuation".to_owned());
            }
            if options.numbers {
                parts.push("numbers".to_owned());
            }
        }
        parts.join(" · ")
    }

    pub fn syntax(&self) -> Option<CodeLanguage> {
        match self {
            Self::Text(TextSource::Code { language }) => Some(*language),
            Self::File(custom) => custom.language,
            _ => None,
        }
    }

    pub fn mode_label(&self) -> String {
        match self {
            Self::Text(TextSource::Words { count, .. }) => format!("words {count}"),
            Self::Timed { seconds, .. } => format!("time {seconds}"),
            Self::Text(TextSource::Quote { .. }) => "quote".to_owned(),
            Self::Text(TextSource::Code { .. }) => "code".to_owned(),
            Self::File(_) => "file".to_owned(),
        }
    }

    pub fn language_label(&self) -> String {
        match self {
            Self::Text(TextSource::Words { language, .. } | TextSource::Quote { language })
            | Self::Timed { language, .. } => language.to_string(),
            Self::Text(TextSource::Code { language }) => language.to_string(),
            Self::File(custom) => custom
                .language
                .map_or_else(|| "text".to_owned(), |language| language.to_string()),
        }
    }
}

pub fn code_file_name(language: CodeLanguage) -> String {
    let stem = match language {
        CodeLanguage::Rust => "main",
        CodeLanguage::Python => "app",
        CodeLanguage::TypeScript | CodeLanguage::JavaScript => "index",
        CodeLanguage::Sql => "query",
    };
    format!("{stem}.{}", language.extension())
}

/// A solo session in progress or just finished.
#[derive(Debug)]
pub struct SoloRun {
    pub plan: Plan,
    pub session: TypingSession,
    pub attribution: Option<String>,
    pub result: Option<SoloResult>,
    feed: Option<WordStream>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoloResult {
    pub stats: Stats,
    pub samples: Vec<Sample>,
    pub consistency: f64,
    pub previous_best: Option<f64>,
}

impl SoloResult {
    pub fn is_personal_best(&self) -> bool {
        self.previous_best.is_none_or(|best| self.stats.wpm > best)
    }
}

impl SoloRun {
    pub fn start(plan: Plan, seed: u64) -> Self {
        let code_like = plan.syntax().is_some() || matches!(plan, Plan::File(_));
        let auto_indent = SessionOptions {
            auto_indent: code_like,
            ..SessionOptions::default()
        };
        let (session, attribution, feed) = match &plan {
            Plan::Text(source) => {
                let generated = source.generate(seed);
                let session = TypingSession::new(&generated.text, auto_indent);
                (session, generated.attribution, None)
            }
            Plan::Timed {
                language,
                options,
                seconds,
            } => {
                let mut feed = WordStream::new(*language, *options, seed);
                let limit = SessionOptions {
                    time_limit: Some(Duration::from_secs(u64::from(*seconds))),
                    auto_indent: false,
                };
                let session = TypingSession::new(&feed.phrase(TIMED_INITIAL_WORDS), limit);
                (session, None, Some(feed))
            }
            Plan::File(custom) => (TypingSession::new(&custom.text, auto_indent), None, None),
        };
        Self {
            plan,
            session,
            attribution,
            result: None,
            feed,
        }
    }

    /// Types one character; returns whether the session accepted it.
    pub fn type_char(&mut self, ch: char, now: Instant) -> bool {
        let accepted = self.session.type_char(ch, now);
        if accepted {
            self.refill();
        }
        accepted
    }

    pub fn is_finished(&self) -> bool {
        self.session.is_finished()
    }

    /// Whether typing has started and is not over.
    pub fn is_in_progress(&self) -> bool {
        self.session.status() == Status::Running
    }

    /// Computes the result once the session is over, compared with the
    /// best of `history`, and returns the record to keep, only once.
    pub fn conclude(&mut self, history: &History, now: Instant) -> Option<Record> {
        if self.result.is_some() || !self.session.is_finished() {
            return None;
        }
        let stats = self.session.stats(now);
        let samples = self.session.samples(now);
        let mode = self.plan.mode_label();
        let language = self.plan.language_label();
        let result = SoloResult {
            consistency: consistency(&samples),
            previous_best: history.personal_best(&mode, &language),
            stats,
            samples,
        };
        self.result = Some(result);
        Some(Record::from_stats(mode, language, &stats))
    }

    fn refill(&mut self) {
        let Some(feed) = &mut self.feed else {
            return;
        };
        if self.session.remaining() < TIMED_REFILL_BELOW {
            self.session
                .extend(&format!(" {}", feed.phrase(TIMED_REFILL_WORDS)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::scratch::TempDir;

    fn practice(mode: Mode) -> Practice {
        Practice {
            mode,
            ..Practice::default()
        }
    }

    fn type_all(run: &mut SoloRun, now: Instant) {
        let text = run.session.target().concat();
        for ch in text.chars() {
            run.type_char(ch, now);
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
        let mut settings = practice(Mode::Words);
        adjust(&mut settings, Field::Text(TextSetting::Mode), Step::Next);
        assert_eq!(settings.mode, Mode::Time);
        adjust(&mut settings, Field::Start, Step::Next);
        assert_eq!(settings.mode, Mode::Time, "the start line has no value");
    }

    #[test]
    fn plan_reflects_the_settings() {
        let mut settings = practice(Mode::Words);
        settings.word_count = 25;
        settings.punctuation = true;
        let plan = Plan::from_practice(&settings);
        assert_eq!(plan.label(), "words 25 · english · punctuation");
        assert_eq!(plan.title(), "notes.md");
        settings.mode = Mode::Code;
        settings.code_language = CodeLanguage::Sql;
        assert_eq!(Plan::from_practice(&settings).title(), "query.sql");
        assert_eq!(
            Plan::from_practice(&settings).syntax(),
            Some(CodeLanguage::Sql)
        );
    }

    #[test]
    fn words_session_has_the_requested_length() {
        let mut settings = practice(Mode::Words);
        settings.word_count = 10;
        let run = SoloRun::start(Plan::from_practice(&settings), 3);
        let text = run.session.target().concat();
        assert_eq!(text.split(' ').count(), 10);
    }

    #[test]
    fn timed_session_never_runs_out_of_text() {
        let mut settings = practice(Mode::Time);
        settings.duration = 15;
        let mut run = SoloRun::start(Plan::from_practice(&settings), 5);
        let now = Instant::now();
        for _ in 0..3_000 {
            let next = run.session.target()[run.session.cursor()].clone();
            for ch in next.chars() {
                run.type_char(ch, now);
            }
        }
        assert!(run.session.remaining() >= TIMED_REFILL_BELOW / 2);
        assert_eq!(run.session.status(), Status::Running);
    }

    #[test]
    fn a_code_session_is_compared_with_the_code_records_of_version_one() {
        let directory = TempDir::new();
        let path = directory.join("history.json");
        let legacy = r#"[{"date":"2025-05-01T10:00:00+00:00","mode":"code/50","language":"rust",
            "stats":{"wpm":62.5,"raw_wpm":70.0,"accuracy":96.0,"errors":4,"length":250,"elapsed":48.0}}]"#;
        fs::write(&path, legacy).expect("write");
        let mut history = History::load(&path).value;
        let rust = TextSource::Code {
            language: CodeLanguage::Rust,
        };
        let mut run = SoloRun::start(Plan::Text(rust), 1);
        let now = Instant::now();
        run.session.start(now);
        while !run.is_finished() {
            let next = run.session.target()[run.session.cursor()].clone();
            for ch in next.chars() {
                run.type_char(ch, now + Duration::from_secs(60));
            }
        }
        let record = run
            .conclude(&history, now + Duration::from_secs(60))
            .expect("a record");
        history.add(record).expect("save");
        let result = run.result.as_ref().expect("result");
        assert_eq!(result.previous_best, Some(62.5));
        let records = history.records();
        assert_eq!(records[1].mode, records[0].mode);
    }

    #[test]
    fn code_sessions_auto_indent() {
        let run = SoloRun::start(
            Plan::Text(TextSource::Code {
                language: CodeLanguage::Rust,
            }),
            1,
        );
        assert!(run.session.options().auto_indent);
    }

    #[test]
    fn conclusion_is_recorded_once_with_personal_best() {
        let mut history = History::in_memory();
        let now = Instant::now();
        let mut run = SoloRun::start(
            Plan::Text(TextSource::Quote {
                language: Language::English,
            }),
            2,
        );
        run.session.start(now);
        type_all(&mut run, now + Duration::from_secs(30));
        assert!(run.is_finished());
        let record = run
            .conclude(&history, now + Duration::from_secs(30))
            .expect("a record");
        history.add(record).expect("in-memory history");
        assert_eq!(
            run.conclude(&history, now + Duration::from_secs(31)),
            None,
            "only once"
        );
        assert_eq!(history.records().len(), 1);
        let result = run.result.as_ref().expect("result");
        assert!(result.is_personal_best());
        assert_eq!(history.records()[0].mode, "quote");
        assert_eq!(history.records()[0].language, "english");
    }

    #[test]
    fn custom_files_are_normalised_and_typed_with_syntax() {
        let directory = TempDir::new();
        let path = directory.join("lib.rs");
        fs::write(&path, "fn main() {\r\n\tlet x = 1;   \r\n}\r\n").expect("write");
        let custom = CustomText::load(&path).expect("load");
        assert_eq!(custom.text, "fn main() {\n    let x = 1;\n}");
        assert_eq!(custom.language, Some(CodeLanguage::Rust));
        assert_eq!(Plan::File(custom).title(), "lib.rs");
        fs::write(&path, [0u8, 159, 146, 150]).expect("write binary");
        assert!(matches!(CustomText::load(&path), Err(FileError::Binary(_))));
    }

    #[test]
    fn long_files_are_cut_at_a_line_break() {
        let text = "abc\n".repeat(1_000);
        assert_eq!(truncate_at_line(&text, 10), "abc\nabc");
        assert_eq!(truncate_at_line("short", 10), "short");
    }

    #[test]
    fn huge_files_are_read_only_up_to_the_limit() {
        let directory = TempDir::new();
        let path = directory.join("huge.log");
        fs::write(&path, "abc\n".repeat(MAX_FILE_BYTES)).expect("write");
        let custom = CustomText::load(&path).expect("load");
        assert!(custom.text.ends_with("abc"), "whole lines are kept");
        assert!(custom.text.graphemes(true).count() <= MAX_FILE_GRAPHEMES);
    }

    #[test]
    fn a_character_cut_by_the_byte_limit_does_not_make_the_file_binary() {
        let directory = TempDir::new();
        let path = directory.join("accents.txt");
        let text = format!("a{}", "é".repeat(MAX_FILE_BYTES));
        assert!(!text.is_char_boundary(MAX_FILE_BYTES));
        fs::write(&path, text).expect("write");
        let custom = CustomText::load(&path).expect("load");
        assert!(custom.text.starts_with("aé"));
    }

    #[test]
    fn only_regular_files_are_read() {
        let directory = TempDir::new();
        assert!(matches!(
            CustomText::load(directory.path()),
            Err(FileError::NotAFile(_))
        ));
        assert!(matches!(
            CustomText::load(&directory.join("missing.rs")),
            Err(FileError::Unreadable { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn devices_and_pipes_are_refused_without_blocking() {
        let directory = TempDir::new();
        let fifo = directory.join("pipe");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|status| status.success());
        let mut refused = vec![Path::new("/dev/zero").to_owned()];
        refused.extend(made.then_some(fifo));
        for path in refused {
            assert!(
                matches!(CustomText::load(&path), Err(FileError::NotAFile(_))),
                "{}",
                path.display()
            );
        }
    }
}
