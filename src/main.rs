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

use std::process::ExitCode;

use clap::Parser;

use crate::{
    app::App,
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
    let Loaded {
        value: mut config,
        warning: config_warning,
    } = paths.as_ref().map_or_else(Loaded::default, |paths| {
        config::load_config(&paths.config_file)
    });
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
    apply_overrides(&cli, &mut config);
    let launch = cli.launch(&config.practice)?;
    let config_path = paths.map(|paths| paths.config_file);
    let mut app = App::new(config, config_path, history, launch);
    for warning in [config_warning, history_warning].into_iter().flatten() {
        app.warn(warning);
    }
    runtime::run(app).await
}

fn apply_overrides(cli: &Cli, config: &mut Config) {
    if let Some(server) = &cli.server {
        config.multiplayer.server.clone_from(server);
    }
    if let Some(theme) = cli.theme {
        config.theme = theme;
    }
}
