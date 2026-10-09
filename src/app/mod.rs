/*!
 * Application state and behaviour, independent of rendering. The screen is
 * organised like a code editor: an explorer lists the buffers and the editor
 * pane shows the selected one.
 */

mod actions;
mod buffer;
mod changes;
pub mod command;
mod events;
pub mod form;
mod forms;
pub mod help;
pub mod history_log;
pub mod ink;
pub mod input;
mod keys;
mod layout;
pub mod mascot;
mod messages;
pub mod mouse;
pub mod palette;
pub mod practice;
pub mod race;
mod saved_config;
pub mod settings;
mod startup;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod text_event;
pub mod text_settings;

use std::{
    fmt::Display,
    time::{Duration, Instant},
};

use code_racer_engine::{CodeLanguage, Stats, TypingSession};
use code_racer_protocol::{RoomCode, Username};

use crate::{
    cli::Launch,
    config::{Config, Look},
    history::History,
    network::Connection,
};
pub use buffer::Buffer;
use form::Cursor;
use ink::Ink;
use input::TextInput;
pub use layout::{Focus, Viewport};
use messages::Messages;
pub use messages::{Message, MessageKind};
use palette::CommandPalette;
use practice::SoloRun;
use race::RaceClient;
use saved_config::SavedConfig;
pub use startup::Overrides;
use text_event::TextEvent;

/// Vim-like mode shown in the status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Command,
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

impl Activity {
    /// Returns whether the text being typed took the key.
    fn text_event(&mut self, event: TextEvent, now: Instant) -> bool {
        match self {
            Self::Solo(run) => run.text_event(event, now),
            Self::Race(client) => client.text_event(event, now),
        }
    }
}

/// Everything needed to draw the text being typed.
#[derive(Debug, Clone, Copy)]
pub struct SessionView<'a> {
    pub session: &'a TypingSession,
    pub syntax: Option<CodeLanguage>,
    pub attribution: Option<&'a str>,
    /**
     * When the race ended for a player who had not finished its text: their
     * clock stops there. A finished text stops its clock by itself.
     */
    pub stopped_at: Option<Instant>,
    /// When each character was typed.
    pub ink: Option<&'a Ink>,
    /// What the text looks like when it is prose; code looks like code.
    pub disguise: Option<Disguise<'a>>,
}

/**
 * What a prose text looks like on screen, and what its look takes from
 * the settings.
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disguise<'a> {
    pub look: Look,
    /// The language of the code a doc comment documents.
    pub language: CodeLanguage,
    /// Who signs an email draft.
    pub author: &'a str,
}

impl Default for Disguise<'_> {
    fn default() -> Self {
        Self {
            look: Look::Notes,
            language: CodeLanguage::default(),
            author: "",
        }
    }
}

impl SessionView<'_> {
    /// The statistics of the text at `now`, or when its clock stopped.
    pub fn stats(&self, now: Instant) -> Stats {
        self.session.stats(self.clock(now))
    }

    /// Time left at `now` in a timed session.
    pub fn time_left(&self, now: Instant) -> Option<Duration> {
        self.session.time_left(self.clock(now))
    }

    fn clock(&self, now: Instant) -> Instant {
        self.stopped_at.map_or(now, |stop| stop.min(now))
    }
}

#[derive(Debug)]
pub struct App {
    /// The settings in use, command-line flags included.
    pub config: Config,
    saved: SavedConfig,
    pub history: History,
    pub focus: Focus,
    pub buffer: Buffer,
    pub practice_cursor: Cursor,
    pub race_cursor: Cursor,
    pub room_code: String,
    pub settings_cursor: Cursor,
    pub history_scroll: usize,
    pub help_scroll: usize,
    pub editing: Option<FieldEdit>,
    pub prompt: Option<Prompt>,
    pub palette: Option<CommandPalette>,
    messages: Messages,
    pub activity: Option<Activity>,
    /**
     * Whether the explorer is shown. The explorer has the focus only while
     * it is shown: keys would otherwise switch buffers out of sight.
     */
    pub sidebar: bool,
    /**
     * Whether the user showed or hid the explorer for good, with Ctrl+B or
     * `:set sidebar`. Until then it is shown when the terminal has room for
     * it, and otherwise only while it has the focus.
     */
    sidebar_choice: Option<bool>,
    pub viewport: Viewport,
    /// What to open once the name being asked for is set.
    pending: Option<Launch>,
    /// Until when keys are ignored, after typing stopped by itself.
    quiet_until: Option<Instant>,
    /// When Esc was pressed once to leave a session in progress.
    leave_armed: Option<Instant>,
    /// The look drawn for the session when the settings shuffle looks.
    shuffled: Look,
    born: Instant,
    /// When the player last pressed a key, pasted or clicked.
    last_input: Option<Instant>,
    /**
     * The name of the directory the app runs in, which the explorer shows
     * in discreet mode, as an editor shows the project it opened.
     */
    pub workspace: String,
    quit: bool,
}

impl App {
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The message shown in the command line.
    pub fn message(&self) -> Option<&Message> {
        self.messages.first()
    }

    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.saved.path()
    }

    /// What is being typed in `field`, while it is.
    pub fn input_of(&self, field: TextField) -> Option<&TextInput> {
        self.editing
            .as_ref()
            .filter(|edit| edit.field == field)
            .map(|edit| &edit.input)
    }

    pub fn editor_mode(&self) -> EditorMode {
        if self.prompt.is_some() || self.palette.is_some() {
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
        let (view, prose) = match &self.activity {
            Some(Activity::Solo(run)) => {
                let view = SessionView {
                    session: run.session(),
                    syntax: run.plan.syntax(),
                    attribution: run.attribution.as_deref(),
                    stopped_at: None,
                    ink: Some(run.ink()),
                    disguise: None,
                };
                (view, run.plan.is_prose())
            }
            Some(Activity::Race(client)) => (client.session_view()?, client.syntax().is_none()),
            None => return None,
        };
        Some(SessionView {
            disguise: prose.then(|| self.disguise()),
            ..view
        })
    }

    /**
     * What prose looks like in the session: the look of the settings, or
     * for a shuffle the one drawn for the session.
     */
    pub fn disguise(&self) -> Disguise<'_> {
        let look = match self.config.look {
            Look::Shuffle => self.shuffled,
            look => look,
        };
        Disguise {
            look,
            language: self.config.practice.code_language,
            author: &self.config.username,
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

    /**
     * Whether the player is in the middle of a text: leaving it takes a
     * confirmation, and its buffer is marked as modified.
     */
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

    /// Whether the text refuses input until its first mistake is fixed.
    pub fn is_typing_blocked(&self) -> bool {
        self.is_typing()
            && self
                .session_view()
                .is_some_and(|view| view.session.is_blocked())
    }

    /**
     * Starts handling an event. The screen was drawn since the previous
     * one, showing the first message unless the command line hid it.
     */
    fn begin_event(&mut self) {
        self.messages.next_event(self.prompt.is_none());
    }

    fn info(&mut self, text: impl Into<String>) {
        self.messages.push(Message::info(text));
    }

    fn error(&mut self, text: impl Display) {
        self.messages.push(Message::error(text));
    }

    /**
     * A key press dismisses the message on screen. The command line hides
     * the messages, so none is dismissed while it is open. An error while
     * typing stays unless another message waits: it would vanish at the
     * next character, before it could be read.
     */
    fn dismiss_message(&mut self) {
        let hidden = self.prompt.is_some();
        let typing_error = self.is_typing()
            && self.messages.waiting_behind() == 0
            && self.message().is_some_and(Message::is_error);
        if !hidden && !typing_error {
            self.messages.dismiss();
        }
    }

    /**
     * Drops the field being typed, and what was to follow the name asked
     * at first launch.
     */
    fn cancel_edit(&mut self) {
        self.editing = None;
        self.pending = None;
    }
}

#[cfg(test)]
mod tests;
