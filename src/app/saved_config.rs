/*!
 * The settings saved in `config.toml`, kept apart from the settings in use.
 *
 * Command-line flags such as `--theme mono` apply to one run and never reach
 * the file unless the user chooses the same setting in the app. The file can
 * change while the app runs, edited by hand or saved by another instance, so a
 * save applies only what the user chose onto what the file holds at that moment.
 */

use std::path::{Path, PathBuf};

use crate::config::{self, Config};

#[derive(Debug)]
pub struct SavedConfig {
    /**
     * Where the settings are saved, `None` when they must not be: there is
     * no configuration directory, or the file could not be read and would
     * be lost.
     */
    path: Option<PathBuf>,
    /**
     * The settings in use when the file was last brought up to date: the
     * ones in use that differ from them are what the user changed since.
     */
    reference: Config,
}

impl SavedConfig {
    pub fn new(path: Option<PathBuf>, in_use: &Config) -> Self {
        Self {
            path,
            reference: in_use.clone(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn change_for_this_run(&mut self, in_use: &mut Config, change: impl Fn(&mut Config)) {
        change(in_use);
        change(&mut self.reference);
    }

    /**
     * Applies `choice`, made by the user in the app, to the settings in
     * use, then saves it with the other settings changed since the last
     * save, and only those.
     *
     * `choice` is applied to the file as well, so that a setting the user
     * sets to the value a flag of this run already gave it is saved too. A
     * relative choice, such as the next theme, is then overridden there by
     * its result in the settings in use, which differs from the reference.
     */
    pub fn choose(
        &mut self,
        in_use: &mut Config,
        choice: impl Fn(&mut Config),
    ) -> Result<(), String> {
        choice(in_use);
        let Some(path) = &self.path else {
            return Ok(());
        };
        config::update_config(path, |saved| {
            choice(saved);
            saved.adopt_changes(&self.reference, in_use);
        })
        .map_err(|error| format!("cannot save {}: {error}", path.display()))?;
        self.reference = in_use.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

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
        let mut saved = SavedConfig::new(Some(path.clone()), &in_use);
        saved.change_for_this_run(&mut in_use, |config| config.practice.mode = Mode::Code);

        saved
            .choose(&mut in_use, |config| config.practice.numbers = true)
            .expect("save");
        let on_disk = load(&path).value;
        assert_eq!(on_disk.theme, Theme::Editor);
        assert_eq!(on_disk.practice.mode, Mode::Words);
        assert!(on_disk.practice.numbers);

        saved
            .choose(&mut in_use, |config| config.theme = Theme::Dark)
            .expect("save");
        assert_eq!(load(&path).value.theme, Theme::Dark);
    }

    #[test]
    fn a_choice_equal_to_a_flag_of_this_run_is_saved() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut in_use = Config {
            theme: Theme::Mono,
            ..Config::default()
        };
        let mut saved = SavedConfig::new(Some(path.clone()), &in_use);

        saved
            .choose(&mut in_use, |config| config.theme = Theme::Mono)
            .expect("save");

        assert_eq!(load(&path).value.theme, Theme::Mono);
    }

    #[test]
    fn a_save_keeps_what_was_written_in_the_file_since_it_was_read() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut in_use = Config::default();
        let mut saved = SavedConfig::new(Some(path.clone()), &in_use);
        fs::write(
            &path,
            "theme = \"dark\"\n[multiplayer]\nserver = \"ws://10.0.0.5:8080\"\n",
        )
        .expect("edit by hand");

        saved
            .choose(&mut in_use, |config| config.practice.numbers = true)
            .expect("save");
        saved
            .choose(&mut in_use, |config| config.practice.word_count = 25)
            .expect("save");

        let on_disk = load(&path).value;
        assert_eq!(on_disk.theme, Theme::Dark);
        assert_eq!(on_disk.multiplayer.server, "ws://10.0.0.5:8080");
        assert!(on_disk.practice.numbers);
        assert_eq!(on_disk.practice.word_count, 25);
    }

    #[test]
    fn nothing_is_written_when_nothing_changes() {
        let dir = TempDir::new();
        let path = dir.join("config.toml");
        let mut in_use = Config::default();
        let mut saved = SavedConfig::new(Some(path.clone()), &in_use);

        assert_eq!(saved.choose(&mut in_use, |_| {}), Ok(()));
        assert!(!path.exists(), "no file made for unchanged settings");

        fs::write(&path, "theme = 'mono'  # by hand\n").expect("edit by hand");
        let modified = fs::metadata(&path).and_then(|file| file.modified()).ok();
        assert_eq!(saved.choose(&mut in_use, |_| {}), Ok(()));
        assert_eq!(
            fs::read_to_string(&path).expect("read"),
            "theme = 'mono'  # by hand\n"
        );
        assert_eq!(
            fs::metadata(&path).and_then(|file| file.modified()).ok(),
            modified
        );
    }

    #[test]
    fn nothing_is_written_without_a_path() {
        let mut in_use = Config::default();
        let mut saved = SavedConfig::new(None, &in_use);
        assert_eq!(
            saved.choose(&mut in_use, |config| config.theme = Theme::Dark),
            Ok(())
        );
        assert_eq!(in_use.theme, Theme::Dark, "the choice applies to this run");
    }
}
