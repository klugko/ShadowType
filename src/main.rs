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

use std::{path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;

use crate::{
    app::{App, Overrides},
    cli::Cli,
    config::{Config, Loaded, Paths},
    history::History,
};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("code-racer: {error:#}");
            ExitCode::FAILURE
        }
    }
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
