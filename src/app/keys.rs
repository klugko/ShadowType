//! Key handling, from the most specific context to the global bindings.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{
    Activity, App, Buffer, FieldEdit, Focus, Message, Prompt, TextField, command,
    form::{Cursor, Step},
    input::{Edit, TextInput, control_letter, erases_word, normalized, typed_char},
    practice,
    race::{self, RoomRequest},
    settings,
    text_event::TextEvent,
};
use crate::config::Config;
use code_racer_protocol::Phase;

const PAGE: usize = 10;

impl App {
    /// Handles a key press or repeat. Releases are ignored: Windows reports
    /// one after every press, which would act twice. Ctrl+C always quits;
    /// any other key is ignored during the quiet period that follows the
    /// end of typing.
    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.begin_event();
        let key = normalized(key);
        if is_control(key, 'c') {
            self.quit = true;
            return;
        }
        if self.is_quiet(now) {
            return;
        }
        if key.code != KeyCode::Esc {
            self.leave_armed = None;
        }
        let was_typing = self.is_typing();
        self.dismiss_message();
        self.dispatch_key(key, now);
        if key.code != KeyCode::Esc {
            self.quiet_if_typing_stopped(was_typing, now);
        }
    }

    fn dispatch_key(&mut self, key: KeyEvent, now: Instant) {
        if is_control(key, 'b') {
            self.set_sidebar(!self.sidebar);
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
                let last = self.last_scroll(Buffer::History);
                scroll_key(key, &mut self.history_scroll, last)
            }
            Buffer::Help => {
                let last = self.last_scroll(Buffer::Help);
                scroll_key(key, &mut self.help_scroll, last)
            }
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
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => self.focus_explorer(),
            _ => {}
        }
    }

    fn toggle_focus(&mut self) {
        match self.focus {
            Focus::Explorer => self.focus_editor(),
            Focus::Editor => self.focus_explorer(),
        }
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
                self.focus_editor();
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
            FormKind::Race => race::fields(&self.config.race).len(),
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
            _ => self.change_value(intent, |config, step| {
                practice::adjust(&mut config.practice, field, step);
            }),
        }
    }

    fn race_intent(&mut self, intent: ValueIntent) {
        let fields = race::fields(&self.config.race);
        let field = fields[self.race_cursor.index(fields.len())];
        if self.edit_text_line(intent, field.text_field()) {
            return;
        }
        match (intent, field) {
            (ValueIntent::Activate, race::Field::Join) => self.join_typed_room(),
            (ValueIntent::Activate, race::Field::Create) => self.create_room(),
            _ => self.change_value(intent, |config, step| {
                race::adjust(&mut config.race, field, step);
            }),
        }
    }

    fn settings_intent(&mut self, intent: ValueIntent) {
        let field = settings::FIELDS[self.settings_cursor.index(settings::FIELDS.len())];
        if self.edit_text_line(intent, field.text_field()) {
            return;
        }
        if field == settings::Field::Theme {
            self.change_value(intent, settings::cycle_theme);
        }
    }

    /// Begins typing the value of a line that takes `text`, when `intent`
    /// edits or activates it. Returns whether it did.
    fn edit_text_line(&mut self, intent: ValueIntent, text: Option<TextField>) -> bool {
        let editing = matches!(intent, ValueIntent::Activate | ValueIntent::Edit);
        match text.filter(|_| editing) {
            Some(field) => {
                self.begin_edit(field);
                true
            }
            None => false,
        }
    }

    /// Moves the selected value as `intent` says, Enter moving it forward,
    /// and saves it.
    fn change_value(&mut self, intent: ValueIntent, change: impl Fn(&mut Config, Step)) {
        let step = match intent {
            ValueIntent::Change(step) => step,
            ValueIntent::Activate => Step::Next,
            ValueIntent::Edit => return,
        };
        self.choose(|config| change(config, step));
    }

    fn session_key(&mut self, key: KeyEvent, now: Instant) -> bool {
        match &self.activity {
            Some(Activity::Solo(_)) => self.solo_result_key(key),
            Some(Activity::Race(client)) => {
                let phase = client.phase();
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

    /// Room keys. A held key makes its request only once: the server limits
    /// how many messages a player sends.
    fn room_key(&mut self, key: KeyEvent, phase: Option<Phase>, now: Instant) -> bool {
        let request = match (phase, key.code) {
            (_, KeyCode::Esc) => {
                self.leave_session(now);
                return true;
            }
            (Some(Phase::Lobby), KeyCode::Char('r' | ' ') | KeyCode::Enter) => {
                RoomRequest::ToggleReady
            }
            (Some(Phase::Lobby), KeyCode::Char('s')) => RoomRequest::Start,
            (Some(Phase::Finished), KeyCode::Char('r' | 'l') | KeyCode::Enter) => {
                RoomRequest::Again
            }
            _ => return false,
        };
        if key.kind != KeyEventKind::Repeat {
            self.request_room(request, now);
        }
        true
    }

    fn typing_key(&mut self, key: KeyEvent, now: Instant) {
        if let Some(ch) = typed_char(key) {
            self.type_char(ch, now);
            return;
        }
        match (control_letter(key), key.code) {
            (Some('r'), _) => self.restart_solo(),
            (Some('w'), _) => {
                self.text_event(TextEvent::DeleteWord, now);
            }
            (_, KeyCode::Backspace) if erases_word(key) => {
                self.text_event(TextEvent::DeleteWord, now);
            }
            (_, KeyCode::Backspace) => {
                self.text_event(TextEvent::Backspace, now);
            }
            (_, KeyCode::Enter) => self.type_char('\n', now),
            (_, KeyCode::Esc) => self.leave_session(now),
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
                    Err(error) => self.messages.push(Message::command_error(&error)),
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
            Edit::Cancelled => {
                let field = edit.field;
                self.editing = None;
                if field == TextField::Username {
                    self.skip_username();
                }
            }
            Edit::Changed | Edit::Ignored => {}
        }
    }

    pub(super) fn begin_edit(&mut self, field: TextField) {
        let value = match field {
            TextField::Username => self.config.username.as_str(),
            TextField::Server => self.config.multiplayer.server.as_str(),
            TextField::RoomCode => self.room_code.as_str(),
        };
        self.editing = Some(FieldEdit {
            field,
            input: TextInput::new(value, field.max_length()),
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

/// Scrolls between the first line and `last`, the scroll that shows the
/// last line of the buffer at the bottom of the editor.
fn scroll_key(key: KeyEvent, scroll: &mut usize, last: usize) -> bool {
    let target = match (control_letter(key), key.code) {
        (Some('d'), _) | (_, KeyCode::PageDown) => scroll.saturating_add(PAGE),
        (Some('u'), _) | (_, KeyCode::PageUp) => scroll.saturating_sub(PAGE),
        (Some(_), _) => return false,
        (_, KeyCode::Char('j') | KeyCode::Down) => scroll.saturating_add(1),
        (_, KeyCode::Char('k') | KeyCode::Up) => scroll.saturating_sub(1),
        (_, KeyCode::Char('g') | KeyCode::Home) => 0,
        (_, KeyCode::Char('G') | KeyCode::End) => last,
        _ => return false,
    };
    *scroll = target.min(last);
    true
}

fn is_control(key: KeyEvent, letter: char) -> bool {
    control_letter(key) == Some(letter)
}
