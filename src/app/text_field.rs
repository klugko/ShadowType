use code_racer_protocol::{RoomCode, Username};
use crossterm::event::KeyEvent;

use super::{
    App,
    input::{Edit, TextInput},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    Username,
    Server,
    RoomCode,
}

impl TextField {
    /// Longest server address that can be typed, in characters.
    const MAX_SERVER_LENGTH: usize = 120;

    const fn max_length(self) -> usize {
        match self {
            Self::Username => Username::MAX_LENGTH,
            Self::Server => Self::MAX_SERVER_LENGTH,
            Self::RoomCode => RoomCode::LENGTH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEdit {
    pub field: TextField,
    pub input: TextInput,
}

impl App {
    /// What is being typed in `field`, while it is.
    pub fn input_of(&self, field: TextField) -> Option<&TextInput> {
        self.editing
            .as_ref()
            .filter(|edit| edit.field == field)
            .map(|edit| &edit.input)
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

    pub(super) fn field_key(&mut self, key: KeyEvent) {
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

    /**
     * Drops the field being typed, and what was to follow the name asked
     * at first launch.
     */
    pub(super) fn cancel_edit(&mut self) {
        self.editing = None;
        self.pending = None;
    }
}
