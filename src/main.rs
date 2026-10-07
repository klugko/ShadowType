//! code-racer: typing practice that looks like your code editor, with LAN races.

mod app;
mod cli;
mod config;
mod history;
mod logging;
mod network;
mod persist;
mod runtime;
mod terminal;
mod ui;

use std::{future::Future, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;

use crate::{
    app::{App, Overrides},
    cli::Cli,
    config::{Config, Loaded, Paths},
    history::History,
};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run_to_completion(run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("code-racer: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Runs `work` on a runtime of its own, then stops it without waiting for
/// the blocking jobs still running: resolving a server's host name is one,
/// which only the system can time out, and quitting must not hang on it.
fn run_to_completion(work: impl Future<Output = anyhow::Result<()>>) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("cannot start the async runtime")?;
    let result = runtime.block_on(work);
    runtime.shutdown_background();
    result
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let paths = Paths::discover();
    if let Some(paths) = &paths
        && let Err(error) = logging::init(&paths.log_file)
    {
        eprintln!("code-racer: logging disabled: {error:#}");
    }
    let overrides = overrides(&cli)?;
    let Loaded {
        value: (config, config_path),
        warning: config_warning,
    } = load_settings(paths.as_ref());
    let Loaded {
        value: history,
        warning: history_warning,
    } = paths.as_ref().map_or_else(
        || Loaded {
            value: History::in_memory(),
            warning: None,
        },
        |paths| History::load(&paths.history_file),
    );
    let launch = cli.launch(&config)?;
    let warnings: Vec<String> = [config_warning, history_warning]
        .into_iter()
        .flatten()
        .inspect(|warning| tracing::warn!("{warning}"))
        .collect();
    let app = App::new(config, &overrides, config_path, history, warnings, launch);
    runtime::run(app).await
}

/// The saved settings, and where to save them. A file that could not be
/// read is left alone for the whole run: the settings in memory are then
/// only defaults, and saving them would lose the user's.
fn load_settings(paths: Option<&Paths>) -> Loaded<(Config, Option<PathBuf>)> {
    let Some(paths) = paths else {
        return Loaded::clean((Config::default(), None));
    };
    config::load_config(&paths.config_file).map(|found| match found {
        Some(config) => (config, Some(paths.config_file.clone())),
        None => (Config::default(), None),
    })
}

/// The flags that change settings for this run, the server address
/// checked now rather than when connecting.
fn overrides(cli: &Cli) -> anyhow::Result<Overrides> {
    let server = cli
        .server
        .as_deref()
        .map(network::server_url)
        .transpose()
        .context("--server")?;
    Ok(Overrides {
        theme: cli.theme,
        server,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn exiting_does_not_wait_for_a_host_name_lookup() {
        let started = Instant::now();
        let result = run_to_completion(async {
            tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_secs(30)));
            Ok(())
        });
        assert!(result.is_ok());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
