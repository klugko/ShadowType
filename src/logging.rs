//! Diagnostics go to a log file: the terminal belongs to the interface.

use std::{
    env,
    fs::{self, File, OpenOptions},
    path::Path,
    sync::Mutex,
};

use anyhow::Context;
use tracing_subscriber::{
    EnvFilter,
    filter::{LevelFilter, ParseError},
};

/// Environment variable holding the log filter, such as `debug` or
/// `code_racer=trace`. Defaults to `info`.
pub const FILTER_VARIABLE: &str = "CODE_RACER_LOG";

/// Sends every `tracing` event of the process to the end of the file at `path`.
///
/// Fails, without panicking, when the file cannot be opened or a logger is
/// already installed. An invalid filter is reported in the log file itself.
pub fn init(path: &Path) -> anyhow::Result<()> {
    let file = open_for_append(path)?;
    let (filter, rejected) = parse_filter(&env::var(FILTER_VARIABLE).unwrap_or_default());
    tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("cannot install the logger: {error}"))?;
    if let Some(error) = rejected {
        tracing::warn!(%error, "ignoring {FILTER_VARIABLE}, logging at the info level");
    }
    Ok(())
}

fn open_for_append(path: &Path) -> anyhow::Result<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create the log directory {}", parent.display()))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("cannot open the log file {}", path.display()))
}

/// The filter described by `directives`, or the default one with the reason
/// it was rejected. `EnvFilter`'s lossy parsing would print that reason on the
/// terminal, which belongs to the interface.
fn parse_filter(directives: &str) -> (EnvFilter, Option<ParseError>) {
    let builder = EnvFilter::builder().with_default_directive(LevelFilter::INFO.into());
    match builder.parse(directives) {
        Ok(filter) => (filter, None),
        Err(error) => (builder.parse_lossy(""), Some(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::scratch::TempDir;

    #[test]
    fn events_are_appended_to_the_file_and_a_second_logger_is_refused() {
        let dir = TempDir::new();
        let path = dir.join("state/code-racer.log");
        fs::create_dir_all(dir.join("state")).expect("create directory");
        fs::write(&path, "previous run\n").expect("seed log");

        init(&path).expect("first logger");
        tracing::error!(attempt = 3, "network task stopped");
        let second = init(&dir.join("other.log"));

        let contents = fs::read_to_string(&path).expect("read log");
        assert!(contents.starts_with("previous run\n"), "{contents}");
        assert!(contents.contains("network task stopped"), "{contents}");
        assert!(contents.contains("attempt=3"), "{contents}");
        assert!(!contents.contains('\u{1b}'), "ANSI escapes in {contents}");
        assert!(second.is_err());
    }

    #[test]
    fn filter_defaults_to_info() {
        let (filter, rejected) = parse_filter("");
        assert_eq!(filter.to_string(), "info");
        assert!(rejected.is_none());
    }

    #[test]
    fn valid_directives_are_used() {
        let (filter, rejected) = parse_filter("code_racer=debug,warn");
        assert!(rejected.is_none());
        assert_eq!(filter.max_level_hint(), Some(LevelFilter::DEBUG));
    }

    #[test]
    fn invalid_directives_fall_back_to_info() {
        let (filter, rejected) = parse_filter("code_racer=loud");
        assert!(rejected.is_some());
        assert_eq!(filter.to_string(), "info");
    }

    #[test]
    fn unopenable_file_is_an_error() {
        let dir = TempDir::new();
        assert!(init(dir.path()).is_err());
    }
}
