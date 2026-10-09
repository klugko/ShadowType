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
mod prompt;
pub mod race;
mod saved_config;
mod session;
pub mod settings;
mod startup;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod text_event;
mod text_field;
pub mod text_settings;

use std::{fmt::Display, time::Instant};

use crate::{
    cli::Launch,
    config::{Config, Look},
    history::History,
};
pub use buffer::Buffer;
use form::Cursor;
pub use layout::{Focus, Viewport};
use messages::Messages;
pub use messages::{Message, MessageKind};
use palette::CommandPalette;
use practice::SoloRun;
pub use prompt::Prompt;
use race::RaceClient;
use saved_config::SavedConfig;
pub use session::{Disguise, SessionView};
pub use startup::Overrides;
use text_event::TextEvent;
pub use text_field::{FieldEdit, TextField};

/// Vim-like mode shown in the status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Command,
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

    pub fn editor_mode(&self) -> EditorMode {
        if self.prompt.is_some() || self.palette.is_some() {
            EditorMode::Command
        } else if self.editing.is_some() || self.is_typing() {
            EditorMode::Insert
        } else {
            EditorMode::Normal
        }
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
}

#[cfg(test)]
mod tests;
