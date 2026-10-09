use crossterm::event::{KeyCode, KeyEvent};

use super::{
    App, Message, command,
    input::{Edit, TextInput},
};

/// The `:` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub input: TextInput,
    completion: Option<(String, usize)>,
}

impl Prompt {
    const MAX_LENGTH: usize = 200;

    pub(super) fn new() -> Self {
        Self {
            input: TextInput::new("", Self::MAX_LENGTH),
            completion: None,
        }
    }

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

impl App {
    pub(super) fn prompt_key(&mut self, key: KeyEvent) {
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
}
