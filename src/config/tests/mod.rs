mod loading;
mod practice;
mod saving;
mod settings;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::*;
use crate::persist::scratch::TempDir;

/// The settings loaded from `path`, defaults when the file was left in place.
pub(crate) fn load(path: &Path) -> Loaded<Config> {
    load_config(path).map(Option::unwrap_or_default)
}

const DOCUMENTED_EXAMPLE: &str = r#"username = "Jean"
language = "french"
theme = "dark"
default_mode = "words"
word_count = 50
[multiplayer]
server = "ws://127.0.0.1:8080"
"#;

fn save(path: &Path, config: &Config) -> io::Result<()> {
    update_config(path, |saved| saved.clone_from(config))
}

fn write_config(dir: &TempDir, contents: &str) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, contents).expect("write config");
    path
}
