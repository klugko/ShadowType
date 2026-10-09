/*!
 * User settings stored in `config.toml`, a file meant to be edited by hand
 * too: missing keys take their default, and saving keeps comments and keys
 * this version does not know.
 */

mod appearance;
mod file;
mod practice;
#[cfg(test)]
pub(crate) mod tests;

use std::{fmt, path::PathBuf};

use code_racer_protocol::Username;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub use appearance::{Icons, Look, Theme};
pub use file::{load_config, update_config};
pub use practice::{Mode, Practice};

pub use crate::persist::Loaded;

macro_rules! display_by_name {
    ($($kind:ty),+) => {$(
        impl fmt::Display for $kind {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.name())
            }
        }
    )+};
}

display_by_name!(Theme, Mode, Look, Icons);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Multiplayer {
    /// Address of the race server, normalised by `network::server_url` when used.
    pub server: String,
}

impl Multiplayer {
    pub const DEFAULT_SERVER: &'static str = "ws://127.0.0.1:8080";
}

impl Default for Multiplayer {
    fn default() -> Self {
        Self {
            server: Self::DEFAULT_SERVER.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub username: String,
    pub theme: Theme,
    pub icons: Icons,
    pub look: Look,
    /// Whether the mascot lives at the bottom of the explorer.
    pub mascot: bool,
    /**
     * Whether things move on screen: the ink of typed text drying, the
     * cursor breathing, the mascot, the results counting up.
     */
    pub animations: bool,
    /**
     * Whether the typing cursor leaves a trail behind it as it moves,
     * like a smooth cursor in a code editor.
     */
    pub trail: bool,
    /**
     * Whether the mouse clicks and scrolls in the interface. Off, it is
     * left to the terminal, to select and copy text.
     */
    pub mouse: bool,
    /**
     * Whether the screen shows an editor and nothing else: no speed in the
     * status line, no records, no mascot, no name of the application.
     */
    pub discreet: bool,
    /**
     * Settings of solo sessions, kept at the top level of the file as in
     * earlier versions.
     */
    #[serde(flatten)]
    pub practice: Practice,
    /**
     * Settings of the races this player creates, in a `[race]` table. They
     * are always raceable once loaded: see [`Practice::for_race`].
     */
    pub race: Practice,
    pub multiplayer: Multiplayer,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: String::new(),
            theme: Theme::default(),
            icons: Icons::default(),
            look: Look::default(),
            mascot: true,
            animations: true,
            trail: true,
            mouse: true,
            discreet: false,
            practice: Practice::default(),
            race: Practice::default().for_race(),
            multiplayer: Multiplayer::default(),
        }
    }
}

impl Config {
    /// The configured name, `None` when it is missing or invalid.
    pub fn username(&self) -> Option<Username> {
        self.username.parse().ok()
    }

    /**
     * Takes from `after` every setting that differs from `before`, and
     * keeps the others.
     *
     * `after` is taken apart field by field, so that a setting added later
     * cannot be forgotten here without a compilation error.
     */
    pub fn adopt_changes(&mut self, before: &Self, after: &Self) {
        let Self {
            username,
            theme,
            icons,
            look,
            mascot,
            animations,
            trail,
            mouse,
            discreet,
            practice,
            race,
            multiplayer: Multiplayer { server },
        } = after;
        adopt(&mut self.username, &before.username, username);
        adopt(&mut self.theme, &before.theme, theme);
        adopt(&mut self.icons, &before.icons, icons);
        adopt(&mut self.look, &before.look, look);
        adopt(&mut self.mascot, &before.mascot, mascot);
        adopt(&mut self.animations, &before.animations, animations);
        adopt(&mut self.trail, &before.trail, trail);
        adopt(&mut self.mouse, &before.mouse, mouse);
        adopt(&mut self.discreet, &before.discreet, discreet);
        self.practice.adopt_changes(&before.practice, practice);
        self.race.adopt_changes(&before.race, race);
        adopt(
            &mut self.multiplayer.server,
            &before.multiplayer.server,
            server,
        );
    }
}

fn adopt<T: Clone + PartialEq>(kept: &mut T, before: &T, after: &T) {
    if before != after {
        kept.clone_from(after);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_file: PathBuf,
    pub history_file: PathBuf,
    pub log_file: PathBuf,
}

impl Paths {
    /**
     * Platform directories for the current user, `None` when the home
     * directory cannot be determined.
     */
    pub fn discover() -> Option<Self> {
        let directories = ProjectDirs::from("", "", "code-racer")?;
        let data = directories.data_local_dir();
        Some(Self {
            config_file: directories.config_dir().join("config.toml"),
            history_file: data.join("history.json"),
            log_file: directories
                .state_dir()
                .unwrap_or(data)
                .join("code-racer.log"),
        })
    }
}
