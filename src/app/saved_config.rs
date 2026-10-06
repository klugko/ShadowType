//! The settings saved in `config.toml`, kept apart from the settings in use.
//!
//! Command-line flags such as `--theme mono` or `solo --mode code` apply to
//! one run: they change the settings in use and never reach the file,
//! unless the user changes the same setting in the app.

use std::path::{Path, PathBuf};

use crate::config::{self, Config};

#[derive(Debug)]
pub struct SavedConfig {
    /// Where the settings are saved, `None` when they must not be: there is
    /// no configuration directory, or the file could not be read and would
    /// be lost.
    path: Option<PathBuf>,
    /// What the file holds.
    saved: Config,
    /// The settings in use when `saved` was last brought up to date: the
    /// ones that differ from it since are what the user changed.
    reference: Config,
}

impl SavedConfig {
    pub fn new(path: Option<PathBuf>, saved: Config, in_use: &Config) -> Self {
        Self {
            path,
            saved,
            reference: in_use.clone(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Applies `change` to the settings in use for this run only.
    pub fn change_for_this_run(&mut self, in_use: &mut Config, change: impl Fn(&mut Config)) {
        change(in_use);
        change(&mut self.reference);
    }

    /// Saves the settings changed since the last save, and only those.
    pub fn save(&mut self, in_use: &Config) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut saved = self.saved.clone();
        saved.adopt_changes(&self.reference, in_use);
        config::save_config(path, &saved)
            .map_err(|error| format!("cannot save {}: {error}", path.display()))?;
        self.saved = saved;
        self.reference = in_use.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Mode, Theme, tests::load},
        persist::scratch::TempDir,
    };

    #[test]
    fn settings_of_this_run_are_saved_only_once_changed() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut in_use = Config {
            theme: Theme::Mono,
            ..Config::default()
        };
        let mut saved = SavedConfig::new(Some(path.clone()), Config::default(), &in_use);
        saved.change_for_this_run(&mut in_use, |config| config.practice.mode = Mode::Code);

        in_use.practice.numbers = true;
        saved.save(&in_use).expect("save");
        let on_disk = load(&path).value;
        assert_eq!(on_disk.theme, Theme::Editor);
        assert_eq!(on_disk.practice.mode, Mode::Words);
        assert!(on_disk.practice.numbers);

        in_use.theme = Theme::Dark;
        saved.save(&in_use).expect("save");
        assert_eq!(load(&path).value.theme, Theme::Dark);
    }

    #[test]
    fn nothing_is_written_without_a_path() {
        let mut saved = SavedConfig::new(None, Config::default(), &Config::default());
        assert_eq!(saved.save(&Config::default()), Ok(()));
    }
}
