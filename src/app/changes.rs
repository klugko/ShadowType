//! Changes to the settings, whichever form, field or command made them, and
//! saving them.
//!
//! A failure to save is an error: it replaces a report of the change made
//! before it, and a report made after it waits behind it.

use code_racer_engine::{CodeLanguage, Language};
use code_racer_protocol::{RoomCode, Username};

use super::{App, TextField, command::Setting};
use crate::{
    cli::Launch,
    config::{Config, Practice},
    network,
};

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
            Setting::Sidebar(visible) => self.set_sidebar(visible),
            Setting::Mascot(shown) => self.choose(|config| config.mascot = shown),
            Setting::Animations(on) => self.choose(|config| config.animations = on),
            Setting::Trail(on) => self.choose(|config| config.trail = on),
            Setting::Icons(icons) => self.choose(|config| config.icons = icons),
            Setting::Mouse(on) => self.choose(|config| config.mouse = on),
            Setting::Discreet(on) => self.set_discreet(on),
            Setting::Theme(theme) => self.choose(|config| config.theme = theme),
            Setting::Look(look) => {
                self.choose(|config| config.look = look);
                self.info(format!("prose now looks like {}", look.describe()));
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

    /// Shows an editor and nothing else, or the application again.
    pub(super) fn set_discreet(&mut self, on: bool) {
        self.choose(|config| config.discreet = on);
        if !on {
            self.info("discreet mode off, F12 turns it back on");
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

    fn change_text(&mut self, change: impl Fn(&mut Practice)) {
        self.choose(|config| {
            change(&mut config.practice);
            change(&mut config.race);
        });
    }

    /// Sets the name other racers see, then opens what waited for it. A
    /// failure to save it is an error, which the greeting does not replace.
    fn set_username(&mut self, value: &str) -> Result<(), String> {
        let name: Username = value.parse().map_err(|error| format!("{error}"))?;
        self.choose(|config| config.username = name.to_string());
        self.info(format!("hello {name}"));
        if let Some(launch) = self.pending.take() {
            self.editing = None;
            self.run_launch(launch);
        }
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
        self.choose(|config| config.multiplayer.server.clone_from(&url));
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

    pub(super) fn choose(&mut self, choice: impl Fn(&mut Config)) {
        if let Err(error) = self.saved.choose(&mut self.config, choice) {
            self.error(error);
        }
    }
}
