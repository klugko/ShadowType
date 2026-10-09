use crossterm::event::{KeyCode, KeyEvent};

use super::{
    App, TextField,
    form::{Cursor, Step},
    practice, race, settings,
};
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormKind {
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

impl App {
    pub(super) fn form_key(&mut self, key: KeyEvent, kind: FormKind) -> bool {
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
                practice::adjust(config, field, step);
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
        if field.text_field().is_none() {
            self.change_value(intent, |config, step| settings::adjust(config, field, step));
        }
    }

    /**
     * Begins typing the value of a line that takes `text`, when `intent`
     * edits or activates it. Returns whether it did.
     */
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

    /**
     * Moves the selected value as `intent` says, Enter moving it forward,
     * and saves it.
     */
    fn change_value(&mut self, intent: ValueIntent, change: impl Fn(&mut Config, Step)) {
        let step = match intent {
            ValueIntent::Change(step) => step,
            ValueIntent::Activate => Step::Next,
            ValueIntent::Edit => return,
        };
        self.choose(|config| change(config, step));
    }
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
