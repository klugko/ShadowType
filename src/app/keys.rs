//! Key handling, from the most specific context to the global bindings.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    Activity, App, Buffer, FieldEdit, Focus, Prompt, TextField, command,
    form::{Cursor, Step},
    help,
    input::{Edit, TextInput},
    practice, race, settings,
};
use code_racer_protocol::Phase;

const PAGE: usize = 10;

impl App {
    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        self.message = None;
        if is_control(key, 'c') {
            self.quit = true;
        } else if is_control(key, 'b') {
            self.sidebar = !self.sidebar;
        } else if self.prompt.is_some() {
            self.prompt_key(key);
        } else if self.editing.is_some() {
            self.field_key(key);
        } else if self.is_typing() {
            self.typing_key(key, now);
        } else if !self.context_key(key, now) {
            self.global_key(key);
        }
    }

    fn context_key(&mut self, key: KeyEvent, now: Instant) -> bool {
        let shortcut = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.focus == Focus::Explorer {
            return !shortcut && self.explorer_key(key);
        }
        if shortcut && !matches!(self.buffer, Buffer::History | Buffer::Help) {
            return false;
        }
        match self.buffer {
            Buffer::Practice => self.form_key(key, FormKind::Practice),
            Buffer::Race => self.form_key(key, FormKind::Race),
            Buffer::Settings => self.form_key(key, FormKind::Settings),
            Buffer::History => {
                scroll_key(key, &mut self.history_scroll, self.history.records().len())
            }
            Buffer::Help => scroll_key(key, &mut self.help_scroll, help::LINES.len()),
            Buffer::Session => self.session_key(key, now),
        }
    }

    fn global_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if is_control(key, 'r') {
                self.restart_solo();
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(':') => self.prompt = Some(Prompt::new()),
            KeyCode::Char('?') => self.open(Buffer::Help),
            KeyCode::Char('s') => self.start_practice(),
            KeyCode::Char('m') => self.open(Buffer::Race),
            KeyCode::Char('c') => self.create_room(),
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => self.focus = Focus::Explorer,
            _ => {}
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Explorer => Focus::Editor,
            Focus::Editor => Focus::Explorer,
        };
    }

    fn explorer_key(&mut self, key: KeyEvent) -> bool {
        let entries = self.entries();
        let index = entries
            .iter()
            .position(|entry| *entry == self.buffer)
            .unwrap_or(0);
        let target = match key.code {
            KeyCode::Char('j') | KeyCode::Down => (index + 1).min(entries.len() - 1),
            KeyCode::Char('k') | KeyCode::Up => index.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => entries.len() - 1,
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                self.focus = Focus::Editor;
                return true;
            }
            _ => return false,
        };
        self.buffer = entries[target];
        true
    }

    fn form_key(&mut self, key: KeyEvent, kind: FormKind) -> bool {
        let len = self.form_len(kind);
        let cursor = self.form_cursor(kind);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => cursor.down(len),
            KeyCode::Char('k') | KeyCode::Up => cursor.up(len),
            KeyCode::Char('g') | KeyCode::Home => cursor.first(),
            KeyCode::Char('G') | KeyCode::End => cursor.last(len),
            _ => {
                let Some(intent) = value_intent(key) else {
                    return false;
                };
                match kind {
                    FormKind::Practice => self.practice_intent(intent),
                    FormKind::Race => self.race_intent(intent),
                    FormKind::Settings => self.settings_intent(intent),
                }
            }
        }
        true
    }

    fn form_len(&self, kind: FormKind) -> usize {
        match kind {
            FormKind::Practice => practice::fields(&self.config.practice).len(),
            FormKind::Race => race::fields(&self.race_settings).len(),
            FormKind::Settings => settings::FIELDS.len(),
        }
    }

    fn form_cursor(&mut self, kind: FormKind) -> &mut Cursor {
        match kind {
            FormKind::Practice => &mut self.practice_cursor,
            FormKind::Race => &mut self.race_cursor,
            FormKind::Settings => &mut self.settings_cursor,
        }
    }

    fn practice_intent(&mut self, intent: ValueIntent) {
        let fields = practice::fields(&self.config.practice);
        let field = fields[self.practice_cursor.index(fields.len())];
        match (intent, field) {
            (ValueIntent::Activate, practice::Field::Start) => self.start_practice(),
            (ValueIntent::Change(step), _) => {
                practice::adjust(&mut self.config.practice, field, step);
                self.save_config();
            }
            (ValueIntent::Activate, _) => {
                practice::adjust(&mut self.config.practice, field, Step::Next);
                self.save_config();
            }
            (ValueIntent::Edit, _) => {}
        }
    }

    fn race_intent(&mut self, intent: ValueIntent) {
        let fields = race::fields(&self.race_settings);
        let field = fields[self.race_cursor.index(fields.len())];
        match (intent, field) {
            (ValueIntent::Activate | ValueIntent::Edit, race::Field::Room) => {
                self.begin_edit(TextField::RoomCode);
            }
            (ValueIntent::Activate, race::Field::Join) => self.join_typed_room(),
            (ValueIntent::Activate, race::Field::Create) => self.create_room(),
            (ValueIntent::Change(step), _) => race::adjust(&mut self.race_settings, field, step),
            (ValueIntent::Activate, _) => race::adjust(&mut self.race_settings, field, Step::Next),
            (ValueIntent::Edit, _) => {}
        }
    }

    fn settings_intent(&mut self, intent: ValueIntent) {
        let field = settings::FIELDS[self.settings_cursor.index(settings::FIELDS.len())];
        match (intent, field) {
            (ValueIntent::Activate | ValueIntent::Edit, settings::Field::Username) => {
                self.begin_edit(TextField::Username);
            }
            (ValueIntent::Activate | ValueIntent::Edit, settings::Field::Server) => {
                self.begin_edit(TextField::Server);
            }
            (ValueIntent::Change(step), settings::Field::Theme) => {
                settings::cycle_theme(&mut self.config, step);
                self.save_config();
            }
            (ValueIntent::Activate, settings::Field::Theme) => {
                settings::cycle_theme(&mut self.config, Step::Next);
                self.save_config();
            }
            _ => {}
        }
    }

    fn session_key(&mut self, key: KeyEvent, now: Instant) -> bool {
        match &self.activity {
            Some(Activity::Solo(_)) => self.solo_result_key(key),
            Some(Activity::Race(client)) => {
                let phase = client.room.as_ref().map(|room| room.phase);
                self.room_key(key, phase, now)
            }
            None => false,
        }
    }

    fn solo_result_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('r') | KeyCode::Enter => self.restart_solo(),
            KeyCode::Char('e') => self.open(Buffer::Practice),
            KeyCode::Esc => self.close_session(),
            _ => return false,
        }
        true
    }

    fn room_key(&mut self, key: KeyEvent, phase: Option<Phase>, now: Instant) -> bool {
        match (phase, key.code) {
            (Some(Phase::Lobby), KeyCode::Char('r' | ' ') | KeyCode::Enter) => {
                if let Some(client) = self.race() {
                    client.toggle_ready();
                }
            }
            (Some(Phase::Lobby), KeyCode::Char('s')) => {
                if let Some(Err(reason)) = self.race().map(race::RaceClient::start_race) {
                    self.error(reason);
                }
            }
            (Some(Phase::Finished), KeyCode::Char('r' | 'l') | KeyCode::Enter) => {
                if let Some(Err(reason)) = self.race().map(race::RaceClient::return_to_lobby) {
                    self.info(reason);
                }
            }
            (_, KeyCode::Esc) => self.leave_room(now),
            _ => return false,
        }
        true
    }

    fn typing_key(&mut self, key: KeyEvent, now: Instant) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => self.stop_typing(now),
            KeyCode::Char('r') if control && !alt => self.restart_solo(),
            KeyCode::Char('w') if control && !alt => {
                self.edit_session(now, SessionEdit::DeleteWord)
            }
            KeyCode::Char('h') if control && !alt => self.edit_session(now, SessionEdit::Backspace),
            KeyCode::Backspace if control || alt => self.edit_session(now, SessionEdit::DeleteWord),
            KeyCode::Backspace => self.edit_session(now, SessionEdit::Backspace),
            KeyCode::Enter => self.type_char('\n', now),
            KeyCode::Char(ch) if control == alt => self.type_char(ch, now),
            _ => {}
        }
    }

    fn prompt_key(&mut self, key: KeyEvent) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        if key.code == KeyCode::Tab {
            prompt.complete();
            return;
        }
        if key.code == KeyCode::Backspace && prompt.input.value().is_empty() {
            self.prompt = None;
            return;
        }
        match prompt.input.handle_key(key) {
            Edit::Submitted => {
                let line = prompt.input.value().to_owned();
                self.prompt = None;
                match command::parse(&line) {
                    Ok(command) => self.run_command(command),
                    Err(error) => self.error(error.to_string()),
                }
            }
            Edit::Cancelled => self.prompt = None,
            Edit::Changed => prompt.completion = None,
            Edit::Ignored => {}
        }
    }

    fn field_key(&mut self, key: KeyEvent) {
        let Some(edit) = &mut self.editing else {
            return;
        };
        match edit.input.handle_key(key) {
            Edit::Submitted => {
                let field = edit.field;
                let value = edit.input.value().to_owned();
                self.commit_field(field, &value);
            }
            Edit::Cancelled => self.editing = None,
            Edit::Changed | Edit::Ignored => {}
        }
    }

    pub(super) fn begin_edit(&mut self, field: TextField) {
        let (value, limit) = match field {
            TextField::Username => (self.config.username.as_str(), 24),
            TextField::Server => (self.config.multiplayer.server.as_str(), 120),
            TextField::RoomCode => (self.room_code.as_str(), 6),
        };
        self.editing = Some(FieldEdit {
            field,
            input: TextInput::new(value, limit),
        });
    }
}

impl Prompt {
    fn complete(&mut self) {
        let (typed, index) = match &self.completion {
            Some((typed, index)) => (typed.clone(), index + 1),
            None => (self.input.value().to_owned(), 0),
        };
        if let Some(name) = command::complete(&typed, index) {
            self.input = TextInput::new(name, Self::MAX_LENGTH);
            self.completion = Some((typed, index));
        }
    }
}

/// How a key press edits the session text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionEdit {
    Backspace,
    DeleteWord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormKind {
    Practice,
    Race,
    Settings,
}

/// What a key does to the selected line of a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueIntent {
    Change(Step),
    Activate,
    Edit,
}

fn value_intent(key: KeyEvent) -> Option<ValueIntent> {
    match key.code {
        KeyCode::Char('l' | ' ') | KeyCode::Right => Some(ValueIntent::Change(Step::Next)),
        KeyCode::Char('h') | KeyCode::Left => Some(ValueIntent::Change(Step::Previous)),
        KeyCode::Enter => Some(ValueIntent::Activate),
        KeyCode::Char('i' | 'a') => Some(ValueIntent::Edit),
        _ => None,
    }
}

fn scroll_key(key: KeyEvent, scroll: &mut usize, len: usize) -> bool {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => *scroll += 1,
        KeyCode::Char('k') | KeyCode::Up => *scroll = scroll.saturating_sub(1),
        KeyCode::Char('d') if control => *scroll += PAGE,
        KeyCode::Char('u') if control => *scroll = scroll.saturating_sub(PAGE),
        KeyCode::PageDown => *scroll += PAGE,
        KeyCode::PageUp => *scroll = scroll.saturating_sub(PAGE),
        KeyCode::Char('g') | KeyCode::Home => *scroll = 0,
        KeyCode::Char('G') | KeyCode::End => *scroll = len,
        _ => return false,
    }
    *scroll = (*scroll).min(len.saturating_sub(1));
    true
}

fn is_control(key: KeyEvent, letter: char) -> bool {
    key.code == KeyCode::Char(letter) && key.modifiers.contains(KeyModifiers::CONTROL)
}
