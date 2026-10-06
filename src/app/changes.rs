//! Changes to the settings, whichever form, field or command made them, and
//! saving them.
//!
//! A change is reported before it is saved, so that a failure to save is the
//! message left on screen.

use code_racer_engine::{CodeLanguage, Language};
use code_racer_protocol::{RoomCode, Username};

use super::{App, TextField, command::Setting};
use crate::{cli::Launch, config::Practice, network};

impl App {
    pub(super) fn commit_field(&mut self, field: TextField, value: &str) {
        let result = match field {
            TextField::Username => self.set_username(value),
            TextField::Server => self.set_server(value),
            TextField::RoomCode => self.set_room_code(value),
        };
        match result {
            Ok(()) => {
                self.editing = None;
                if field == TextField::RoomCode && !self.room_code.is_empty() {
                    self.join_typed_room();
                }
            }
            Err(error) => self.error(error),
        }
    }

    pub(super) fn apply_setting(&mut self, setting: Setting) {
        match setting {
            Setting::Punctuation(enabled) => self.change_text(|text| text.punctuation = enabled),
            Setting::Numbers(enabled) => self.change_text(|text| text.numbers = enabled),
            Setting::Sidebar(visible) => self.sidebar = visible,
            Setting::Theme(theme) => {
                self.config.theme = theme;
                self.save_config();
            }
            Setting::Server(server) => {
                if let Err(error) = self.set_server(&server) {
                    self.error(error);
                }
            }
            Setting::Username(name) => {
                if let Err(error) = self.set_username(name.as_str()) {
                    self.error(error);
                }
            }
        }
    }

    pub(super) fn set_language(&mut self, language: Language) {
        self.info(format!("language set to {language}"));
        self.change_text(|text| text.language = language);
    }

    /// Sets the language of code sessions and code races, without starting
    /// one: `:code` does.
    pub(super) fn set_code_language(&mut self, language: CodeLanguage) {
        self.info(format!("code language set to {language}"));
        self.change_text(|text| text.code_language = language);
    }

    /// Changes how texts are written, solo and in races alike, and saves it.
    fn change_text(&mut self, change: impl Fn(&mut Practice)) {
        change(&mut self.config.practice);
        change(&mut self.config.race);
        self.save_config();
    }

    fn set_username(&mut self, value: &str) -> Result<(), String> {
        let name: Username = value.parse().map_err(|error| format!("{error}"))?;
        self.config.username = name.to_string();
        self.info(format!("hello {name}"));
        if let Some(launch) = self.pending.take() {
            self.editing = None;
            self.run_launch(launch);
        }
        self.save_config();
        Ok(())
    }

    /// Esc on the name asked before a launch: the launch goes on without
    /// it, unless it is a race, which needs a name.
    pub(super) fn skip_username(&mut self) {
        match self.pending.take() {
            Some(Launch::Create(_) | Launch::Join(_)) => {
                self.error("a race needs a username: press Enter on it, or :set username=NAME");
            }
            Some(launch) => self.run_launch(launch),
            None => {}
        }
    }

    fn set_server(&mut self, value: &str) -> Result<(), String> {
        let url = network::server_url(value).map_err(|error| error.to_string())?;
        self.config.multiplayer.server = url;
        self.save_config();
        Ok(())
    }

    fn set_room_code(&mut self, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            self.room_code.clear();
            return Ok(());
        }
        let code: RoomCode = value.parse().map_err(|error| format!("{error}"))?;
        self.room_code = code.to_string();
        Ok(())
    }

    /// Saves the settings the user changed. Call it after any message about
    /// the change: a failure replaces that message.
    pub(super) fn save_config(&mut self) {
        if let Err(error) = self.saved.save(&self.config) {
            self.error(error);
        }
    }
}
