//! Application state and behaviour, independent of rendering.
//!
//! The screen is organised like a code editor: an explorer lists the
//! available buffers (practice settings, races, history, settings, help and
//! the running session) and the editor pane shows the selected one.

mod actions;
mod changes;
pub mod command;
pub mod form;
pub mod help;
pub mod input;
mod keys;
pub mod practice;
pub mod race;
pub mod settings;
pub mod text_settings;

use std::{fmt::Display, path::PathBuf, time::Instant};

use code_racer_engine::{CodeLanguage, TypingSession};
use code_racer_protocol::{RoomCode, Username};

use crate::{
    cli::Launch,
    config::{Config, Practice},
    history::History,
    network::{Connection, NetworkEvent},
};
use command::CommandError;
use form::Cursor;
use input::TextInput;
use practice::SoloRun;
use race::{Outcome, RaceClient};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Explorer,
    Editor,
}

/// What the editor pane can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buffer {
    Practice,
    Race,
    History,
    Settings,
    Help,
    /// The running solo session or room.
    Session,
}

impl Buffer {
    pub const FILES: [Self; 5] = [
        Self::Practice,
        Self::Race,
        Self::History,
        Self::Settings,
        Self::Help,
    ];

    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Practice => "practice.toml",
            Self::Race => "race.toml",
            Self::History => "history.log",
            Self::Settings => "config.toml",
            Self::Help => "help.md",
            Self::Session => "session",
        }
    }
}

/// Vim-like mode shown in the status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Error,
}

/// A line shown in the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    /// Exactly what is shown, errors with their Vim-like prefix.
    pub text: String,
}

impl Message {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            kind: MessageKind::Info,
            text: text.into(),
        }
    }

    /// An error, prefixed with `E:` like the Vim errors that have no number.
    pub fn error(text: impl Display) -> Self {
        Self {
            kind: MessageKind::Error,
            text: format!("E: {text}"),
        }
    }

    /// The error of a `:` command, which carries its Vim error number.
    pub fn command_error(error: &CommandError) -> Self {
        Self {
            kind: MessageKind::Error,
            text: error.to_string(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.kind == MessageKind::Error
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    Username,
    Server,
    RoomCode,
}

impl TextField {
    /// Longest server address that can be typed, in characters.
    const MAX_SERVER_LENGTH: usize = 120;

    /// How many characters the field takes.
    const fn max_length(self) -> usize {
        match self {
            Self::Username => Username::MAX_LENGTH,
            Self::Server => Self::MAX_SERVER_LENGTH,
            Self::RoomCode => RoomCode::LENGTH,
        }
    }
}

/// A form value being typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEdit {
    pub field: TextField,
    pub input: TextInput,
}

/// The `:` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub input: TextInput,
    completion: Option<(String, usize)>,
}

impl Prompt {
    const MAX_LENGTH: usize = 200;

    fn new() -> Self {
        Self {
            input: TextInput::new("", Self::MAX_LENGTH),
            completion: None,
        }
    }
}

#[derive(Debug)]
pub enum Activity {
    Solo(Box<SoloRun>),
    Race(Box<RaceClient>),
}

/// Everything needed to draw the text being typed.
#[derive(Debug, Clone, Copy)]
pub struct SessionView<'a> {
    pub session: &'a TypingSession,
    pub syntax: Option<CodeLanguage>,
    pub attribution: Option<&'a str>,
}

#[derive(Debug)]
pub struct App {
    pub config: Config,
    config_path: Option<PathBuf>,
    pub history: History,
    pub focus: Focus,
    pub buffer: Buffer,
    pub practice_cursor: Cursor,
    pub race_cursor: Cursor,
    pub race_settings: Practice,
    pub room_code: String,
    pub settings_cursor: Cursor,
    pub history_scroll: usize,
    pub help_scroll: usize,
    pub editing: Option<FieldEdit>,
    pub prompt: Option<Prompt>,
    pub message: Option<Message>,
    pub activity: Option<Activity>,
    pub sidebar: bool,
    pending: Option<Launch>,
    quit: bool,
}

impl App {
    pub fn new(
        config: Config,
        config_path: Option<PathBuf>,
        history: History,
        launch: Launch,
    ) -> Self {
        let race_settings = config.practice.for_race();
        let mut app = Self {
            config,
            config_path,
            history,
            focus: Focus::Explorer,
            buffer: Buffer::Practice,
            practice_cursor: Cursor::default(),
            race_cursor: Cursor::default(),
            race_settings,
            room_code: String::new(),
            settings_cursor: Cursor::default(),
            history_scroll: 0,
            help_scroll: 0,
            editing: None,
            prompt: None,
            message: None,
            activity: None,
            sidebar: true,
            pending: None,
            quit: false,
        };
        app.launch(launch);
        app
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.config_path.as_deref()
    }

    /// Buffers listed in the explorer, in order.
    pub fn entries(&self) -> Vec<Buffer> {
        let mut entries = Buffer::FILES.to_vec();
        if self.activity.is_some() {
            entries.push(Buffer::Session);
        }
        entries
    }

    /// Name of a buffer as shown in the explorer and the tab line.
    pub fn buffer_name(&self, buffer: Buffer) -> String {
        match (buffer, &self.activity) {
            (Buffer::Session, Some(Activity::Solo(run))) => run.plan.title(),
            (Buffer::Session, Some(Activity::Race(client))) => client.title(),
            _ => buffer.file_name().to_owned(),
        }
    }

    pub fn editor_mode(&self) -> EditorMode {
        if self.prompt.is_some() {
            EditorMode::Command
        } else if self.editing.is_some() || self.is_typing() {
            EditorMode::Insert
        } else {
            EditorMode::Normal
        }
    }

    pub fn solo(&self) -> Option<&SoloRun> {
        match &self.activity {
            Some(Activity::Solo(run)) => Some(run),
            _ => None,
        }
    }

    pub fn race(&self) -> Option<&RaceClient> {
        match &self.activity {
            Some(Activity::Race(client)) => Some(client),
            _ => None,
        }
    }

    pub fn session_view(&self) -> Option<SessionView<'_>> {
        match &self.activity {
            Some(Activity::Solo(run)) => Some(SessionView {
                session: &run.session,
                syntax: run.plan.syntax(),
                attribution: run.attribution.as_deref(),
            }),
            Some(Activity::Race(client)) => client.race.as_ref().map(|race| SessionView {
                session: &race.session,
                syntax: client.syntax(),
                attribution: None,
            }),
            None => None,
        }
    }

    pub fn connection_mut(&mut self) -> Option<&mut Connection> {
        match &mut self.activity {
            Some(Activity::Race(client)) => Some(client.connection_mut()),
            _ => None,
        }
    }

    /// Whether the clock has to tick: a session is running or a countdown is shown.
    pub fn needs_ticks(&self) -> bool {
        match &self.activity {
            Some(Activity::Solo(run)) => run.is_in_progress(),
            Some(Activity::Race(client)) => client.is_live(),
            None => false,
        }
    }

    /// Whether the player is in the middle of a text: leaving it takes a
    /// confirmation, and its buffer is marked as modified.
    pub fn session_in_progress(&self) -> bool {
        match &self.activity {
            Some(Activity::Solo(run)) => run.is_in_progress(),
            Some(Activity::Race(client)) => client.is_player_racing(),
            None => false,
        }
    }

    /// Whether keystrokes go to the text being typed.
    pub fn is_typing(&self) -> bool {
        if self.buffer != Buffer::Session || self.focus != Focus::Editor {
            return false;
        }
        match &self.activity {
            Some(Activity::Solo(run)) => !run.is_finished(),
            Some(Activity::Race(client)) => client.is_player_racing(),
            None => false,
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        if let Some(prompt) = &mut self.prompt {
            prompt.input.insert_str(text);
        } else if let Some(edit) = &mut self.editing {
            edit.input.insert_str(text);
        } else if self.is_typing() {
            self.error("pasting is disabled while typing");
        }
    }

    pub fn handle_network(&mut self, event: NetworkEvent, now: Instant) {
        let Some(Activity::Race(client)) = &mut self.activity else {
            return;
        };
        match client.handle(event, now) {
            Outcome::Nothing => {}
            Outcome::Entered(code) => {
                self.room_code = code.to_string();
                if self.buffer == Buffer::Race {
                    self.open(Buffer::Session);
                }
                self.info(format!(
                    "in room {code}, share the code with your teammates"
                ));
            }
            Outcome::Failure(text) => self.error(text),
            Outcome::Closed(reason) => {
                self.activity = None;
                if self.buffer == Buffer::Session {
                    self.open(Buffer::Race);
                }
                self.error(reason);
            }
            Outcome::Finished(record) => {
                if let Err(error) = self.history.add(record) {
                    self.error(format!("cannot save history: {error}"));
                }
            }
        }
    }

    pub fn tick(&mut self, now: Instant) {
        match &mut self.activity {
            Some(Activity::Solo(run)) => {
                run.session.update(now);
                self.conclude_solo(now);
            }
            Some(Activity::Race(client)) => {
                if let Some(session) = client.session_mut() {
                    session.update(now);
                }
                client.report_progress(now);
            }
            None => {}
        }
    }

    fn info(&mut self, text: impl Into<String>) {
        self.message = Some(Message::info(text));
    }

    fn error(&mut self, text: impl Display) {
        self.message = Some(Message::error(text));
    }

    /// A key press dismisses the message, except an error while typing: it
    /// would vanish at the next character, before it could be read.
    fn dismiss_message(&mut self) {
        let keep = self.is_typing() && self.message.as_ref().is_some_and(Message::is_error);
        if !keep {
            self.message = None;
        }
    }

    fn open(&mut self, buffer: Buffer) {
        self.buffer = buffer;
        self.focus = Focus::Editor;
    }
}

#[cfg(test)]
mod tests;
