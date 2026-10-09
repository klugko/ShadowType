/*!
 * The command palette: every action of the application, listed and found
 * by typing a few letters of it, as in a code editor. It opens with Ctrl+P
 * or F1 from anywhere.
 */

mod entries;
mod matching;

use crossterm::event::{KeyCode, KeyEvent};

use super::{
    App, Buffer, TextField,
    command::Command,
    input::{Edit, TextInput, control_letter},
};
pub use entries::entries;
pub use matching::{Match, matches};

/// Longest query, in characters.
const MAX_QUERY: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPalette {
    pub query: TextInput,
    /// Index of the selected entry among those that match the query.
    selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Run(Command),
    Restart,
    /// Opens the `:` line with this already typed.
    Prompt(&'static str),
    /// Opens `race.toml` on the room line, ready to type a code.
    JoinRoom,
    /// Opens `config.toml` on the name line, ready to type it.
    Rename,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    /// The key or command that does the same, shown on the right.
    pub shortcut: String,
    pub action: Action,
}

impl CommandPalette {
    fn new() -> Self {
        Self {
            query: TextInput::new("", MAX_QUERY),
            selected: 0,
        }
    }

    pub fn selected(&self, count: usize) -> usize {
        self.selected.min(count.saturating_sub(1))
    }

    pub fn select(&mut self, index: usize, count: usize) {
        self.selected = index.min(count.saturating_sub(1));
    }

    /**
     * Moves the selection one entry, without wrapping around, as the
     * wheel does.
     */
    pub fn scroll(&mut self, down: bool, count: usize) {
        let selected = self.selected(count);
        let selected = if down {
            selected + 1
        } else {
            selected.saturating_sub(1)
        };
        self.select(selected, count);
    }

    fn step(&mut self, down: bool, count: usize) {
        if count == 0 {
            return;
        }
        let current = self.selected(count);
        self.selected = if down {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
    }
}

impl App {
    pub(super) fn open_palette(&mut self) {
        self.prompt = None;
        self.palette = Some(CommandPalette::new());
    }

    /// The entries of the open palette that match its query, best first.
    pub fn palette_matches(&self) -> Vec<Match> {
        self.palette
            .as_ref()
            .map(|palette| matches(entries(self), palette.query.value()))
            .unwrap_or_default()
    }

    pub(super) fn palette_key(&mut self, key: KeyEvent) {
        let count = self.palette_matches().len();
        let Some(palette) = &mut self.palette else {
            return;
        };
        match (control_letter(key), key.code) {
            (Some('n' | 'j'), _) | (_, KeyCode::Down | KeyCode::Tab) => palette.step(true, count),
            (Some('p' | 'k'), _) | (_, KeyCode::Up | KeyCode::BackTab) => {
                palette.step(false, count);
            }
            (_, KeyCode::PageDown) => palette.select(palette.selected(count) + 5, count),
            (_, KeyCode::PageUp) => {
                let selected = palette.selected(count).saturating_sub(5);
                palette.select(selected, count);
            }
            _ => match palette.query.handle_key(key) {
                Edit::Submitted => self.run_palette_entry(None),
                Edit::Cancelled => self.palette = None,
                Edit::Changed => palette.selected = 0,
                Edit::Ignored => {}
            },
        }
    }

    /**
     * Runs the entry `index` of the matches, or the selected one, and
     * closes the palette.
     */
    pub(super) fn run_palette_entry(&mut self, index: Option<usize>) {
        let matches = self.palette_matches();
        let Some(palette) = self.palette.take() else {
            return;
        };
        let index = index.unwrap_or_else(|| palette.selected(matches.len()));
        let Some(found) = matches.into_iter().nth(index) else {
            return;
        };
        match found.entry.action {
            Action::Run(command) => self.run_command(command),
            Action::Restart => self.restart_solo(),
            Action::Prompt(text) => {
                let mut prompt = super::Prompt::new();
                prompt.input.insert_str(text);
                self.prompt = Some(prompt);
            }
            Action::JoinRoom => {
                self.open(Buffer::Race);
                self.race_cursor.first();
                self.begin_edit(TextField::RoomCode);
            }
            Action::Rename => {
                self.open(Buffer::Settings);
                self.settings_cursor.first();
                self.begin_edit(TextField::Username);
            }
        }
    }
}

#[cfg(test)]
mod tests;
