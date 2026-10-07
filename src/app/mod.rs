//! Application state and behaviour, independent of rendering.
//!
//! The screen is organised like a code editor: an explorer lists the
//! available buffers (practice settings, races, history, settings, help and
//! the running session) and the editor pane shows the selected one.

mod actions;
mod changes;
pub mod command;
mod events;
pub mod form;
pub mod help;
pub mod history_log;
pub mod input;
mod keys;
mod messages;
pub mod practice;
pub mod race;
mod saved_config;
pub mod settings;
mod text_event;
pub mod text_settings;

use std::{
    fmt::Display,
    path::PathBuf,
    time::{Duration, Instant},
};

use code_racer_engine::{CodeLanguage, Stats, TypingSession};
use code_racer_protocol::{RoomCode, Username};

use crate::{
    cli::Launch,
    config::{Config, Theme},
    history::History,
    network::Connection,
};
use form::Cursor;
use input::TextInput;
use messages::Messages;
pub use messages::{Message, MessageKind};
use practice::SoloRun;
use race::RaceClient;
use saved_config::SavedConfig;
use text_event::TextEvent;

/// Settings given on the command line. They apply to this run only and are
/// never saved, unless the user changes the same setting in the app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    pub theme: Option<Theme>,
    /// A server address, already normalised by `network::server_url`.
    pub server: Option<String>,
}

impl Overrides {
    fn apply_to(&self, config: &mut Config) {
        if let Some(theme) = self.theme {
            config.theme = theme;
        }
        if let Some(server) = &self.server {
            config.multiplayer.server.clone_from(server);
        }
    }
}

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

/// Size of the terminal, as last reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u16,
    pub height: u16,
}

impl Viewport {
    /// Rows around the editor pane: the tab line, the status line and the
    /// command line.
    const CHROME_ROWS: u16 = 3;
    /// Narrowest terminal that shows the explorer by itself. Narrower ones
    /// give its columns to the buffer, whose lines would be cut otherwise.
    const EXPLORER_MIN_WIDTH: u16 = 100;

    /// Lines of a buffer the editor pane shows at once.
    pub fn editor_rows(self) -> usize {
        usize::from(self.height.saturating_sub(Self::CHROME_ROWS))
    }

    /// Whether the explorer fits next to whole buffer lines.
    fn has_room_for_explorer(self) -> bool {
        self.width >= Self::EXPLORER_MIN_WIDTH
    }
}

/// The size assumed before the terminal reports its own: a common one, with
/// room for the explorer.
impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 120,
            height: 30,
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
    /// Hands `event` to the text being typed; returns whether it took the key.
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
    /// When the race ended for a player who had not finished its text: their
    /// clock stops there. A finished text stops its clock by itself.
    pub stopped_at: Option<Instant>,
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
    messages: Messages,
    pub activity: Option<Activity>,
    /// Whether the explorer is shown. The explorer has the focus only while
    /// it is shown: keys would otherwise switch buffers out of sight.
    pub sidebar: bool,
    /// Whether the user showed or hid the explorer for good, with Ctrl+B or
    /// `:set sidebar`. Until then it is shown when the terminal has room for
    /// it, and otherwise only while it has the focus.
    sidebar_choice: Option<bool>,
    pub viewport: Viewport,
    /// What to open once the name being asked for is set.
    pending: Option<Launch>,
    /// Until when keys are ignored, after typing stopped by itself.
    quiet_until: Option<Instant>,
    /// When Esc was pressed once to leave a session in progress.
    leave_armed: Option<Instant>,
    quit: bool,
}

impl App {
    /// An application started with the `saved` settings, saved back to
    /// `config_path` when there is one, and `overrides` for this run. The
    /// `warnings` of loading the files are shown first, then `launch` opens.
    pub fn new(
        saved: Config,
        overrides: &Overrides,
        config_path: Option<PathBuf>,
        history: History,
        warnings: Vec<String>,
        launch: Launch,
    ) -> Self {
        let mut config = saved;
        overrides.apply_to(&mut config);
        let mut app = Self {
            saved: SavedConfig::new(config_path, &config),
            config,
            history,
            focus: Focus::Explorer,
            buffer: Buffer::Practice,
            practice_cursor: Cursor::default(),
            race_cursor: Cursor::default(),
            room_code: String::new(),
            settings_cursor: Cursor::default(),
            history_scroll: 0,
            help_scroll: 0,
            editing: None,
            prompt: None,
            messages: Messages::default(),
            activity: None,
            sidebar: true,
            sidebar_choice: None,
            viewport: Viewport::default(),
            pending: None,
            quiet_until: None,
            leave_armed: None,
            quit: false,
        };
        for warning in warnings {
            app.messages.next_event(false);
            app.error(warning);
        }
        app.messages.next_event(false);
        app.launch(launch);
        app
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The message shown in the command line.
    pub fn message(&self) -> Option<&Message> {
        self.messages.first()
    }

    /// Where the settings are saved, if they are.
    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.saved.path()
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

    /// What is being typed in `field`, while it is.
    pub fn input_of(&self, field: TextField) -> Option<&TextInput> {
        self.editing
            .as_ref()
            .filter(|edit| edit.field == field)
            .map(|edit| &edit.input)
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
                session: run.session(),
                syntax: run.plan.syntax(),
                attribution: run.attribution.as_deref(),
                stopped_at: None,
            }),
            Some(Activity::Race(client)) => client.session_view(),
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

    /// Takes the new size of the terminal, keeping the scrolled buffers
    /// within their content, and the explorer as it rests at that size.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = Viewport { width, height };
        self.show_sidebar(self.resting_sidebar());
        self.history_scroll = self.history_scroll.min(self.last_scroll(Buffer::History));
        self.help_scroll = self.help_scroll.min(self.last_scroll(Buffer::Help));
    }

    /// The scroll of `buffer` that shows its last line at the bottom of the
    /// editor pane, zero for buffers that do not scroll.
    fn last_scroll(&self, buffer: Buffer) -> usize {
        let lines = match buffer {
            Buffer::History => history_log::line_count(self.history.records().len()),
            Buffer::Help => help::LINES.len(),
            _ => 0,
        };
        lines.saturating_sub(self.viewport.editor_rows())
    }

    /// Shows or hides the explorer as the user asks, whatever the width of
    /// the terminal from then on.
    fn set_sidebar(&mut self, visible: bool) {
        self.sidebar_choice = Some(visible);
        self.show_sidebar(visible);
    }

    fn show_sidebar(&mut self, visible: bool) {
        self.sidebar = visible;
        if !visible {
            self.focus = Focus::Editor;
        }
    }

    /// Whether the explorer is shown while the editor has the focus: as the
    /// user chose, or when the terminal has room for it.
    fn resting_sidebar(&self) -> bool {
        self.sidebar_choice
            .unwrap_or_else(|| self.viewport.has_room_for_explorer())
    }

    /// Focuses the explorer, showing it while it has the focus.
    fn focus_explorer(&mut self) {
        self.sidebar = true;
        self.focus = Focus::Explorer;
    }

    /// Focuses the editor, the explorer going back to how it rests.
    fn focus_editor(&mut self) {
        self.focus = Focus::Editor;
        self.sidebar = self.resting_sidebar();
    }

    /// Focuses the explorer if it is shown, the editor otherwise.
    fn focus_explorer_if_shown(&mut self) {
        self.focus = if self.sidebar {
            Focus::Explorer
        } else {
            Focus::Editor
        };
    }

    /// Whether the text refuses input until its first mistake is fixed.
    pub fn is_typing_blocked(&self) -> bool {
        self.is_typing()
            && self
                .session_view()
                .is_some_and(|view| view.session.is_blocked())
    }

    /// Starts handling an event. The screen was drawn since the previous
    /// one, showing the first message unless the command line hid it.
    fn begin_event(&mut self) {
        self.messages.next_event(self.prompt.is_none());
    }

    fn info(&mut self, text: impl Into<String>) {
        self.messages.push(Message::info(text));
    }

    fn error(&mut self, text: impl Display) {
        self.messages.push(Message::error(text));
    }

    /// A key press dismisses the message on screen. The command line hides
    /// the messages, so none is dismissed while it is open. An error while
    /// typing stays unless another message waits: it would vanish at the
    /// next character, before it could be read.
    fn dismiss_message(&mut self) {
        let hidden = self.prompt.is_some();
        let typing_error = self.is_typing()
            && self.messages.waiting_behind() == 0
            && self.message().is_some_and(Message::is_error);
        if !hidden && !typing_error {
            self.messages.dismiss();
        }
    }

    /// Shows `buffer` in the editor. A field being typed in another buffer
    /// is cancelled: hidden, it would still take every key.
    fn open(&mut self, buffer: Buffer) {
        if buffer != self.buffer {
            self.cancel_edit();
        }
        self.buffer = buffer;
        self.focus_editor();
    }

    /// Drops the field being typed, and what was to follow the name asked
    /// at first launch.
    fn cancel_edit(&mut self) {
        self.editing = None;
        self.pending = None;
    }
}

#[cfg(test)]
mod tests;
