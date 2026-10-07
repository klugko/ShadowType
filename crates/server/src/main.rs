//! Command line entry point of the race server.

use std::{ffi::OsStr, io::IsTerminal, num::NonZeroUsize, time::Duration};

use anyhow::Context;
use clap::Parser;
use code_racer_protocol::MAX_ROOM_PLAYERS;
use code_racer_server::{ServerConfig, serve};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "code-racer-server", version, about)]
struct Args {
    /// Address to listen on; 0.0.0.0 accepts players from the local network.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// TCP port to listen on.
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Players allowed in one room.
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u8).range(2..=i64::from(MAX_ROOM_PLAYERS)))]
    max_players: u8,
    /// Seconds of inactivity after which a room is closed.
    #[arg(long, value_name = "SECONDS", default_value_t = 1800, value_parser = clap::value_parser!(u64).range(1..))]
    room_ttl: u64,
    /// Seconds after which a race ends, finished or not.
    #[arg(long, value_name = "SECONDS", default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..))]
    race_timeout: u64,
    /// Seconds of countdown before a race starts.
    #[arg(long, value_name = "SECONDS", default_value_t = 3, value_parser = clap::value_parser!(u64).range(1..=10))]
    countdown: u64,
    /// Connections one IP address may hold at once; 0 lifts the limit, for
    /// example behind a proxy or port forward that hides client addresses.
    #[arg(long, value_name = "COUNT", default_value_t = 16)]
    max_connections_per_address: usize,
}

impl Args {
    fn config(&self) -> ServerConfig {
        ServerConfig {
            max_players: self.max_players,
            room_ttl: Duration::from_secs(self.room_ttl),
            race_timeout: Duration::from_secs(self.race_timeout),
            countdown: Duration::from_secs(self.countdown),
            max_connections_per_address: NonZeroUsize::new(self.max_connections_per_address),
            ..ServerConfig::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(coloured_logs())
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .map_err(anyhow::Error::from_boxed)?;
    let listener = TcpListener::bind((args.host.as_str(), args.port))
        .await
        .with_context(|| format!("cannot listen on {}:{}", args.host, args.port))?;
    info!(address = %listener.local_addr()?, "race server listening");
    serve(listener, args.config(), shutdown_signal()).await?;
    info!("race server stopped");
    Ok(())
}

/// Whether log lines carry ANSI colours: only on a terminal that renders
/// them, so that files, pipes, `docker logs` and journald get plain text.
fn coloured_logs() -> bool {
    let no_color = std::env::var_os("NO_COLOR");
    colours_wanted(std::io::stderr().is_terminal(), no_color.as_deref()) && terminal_renders_ansi()
}

/// Colours suit a terminal, unless `NO_COLOR` is set to a non-empty value,
/// as <https://no-color.org> asks.
fn colours_wanted(terminal: bool, no_color: Option<&OsStr>) -> bool {
    terminal && no_color.is_none_or(OsStr::is_empty)
}

/// Windows consoles render ANSI sequences only once asked to.
#[cfg(windows)]
fn terminal_renders_ansi() -> bool {
    nu_ansi_term::enable_ansi_support().is_ok()
}

#[cfg(not(windows))]
fn terminal_renders_ansi() -> bool {
    true
}

/// Completes on Ctrl+C, or on the SIGTERM sent by `docker stop` and service managers.
async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(%error, "cannot listen for Ctrl+C");
            std::future::pending::<()>().await;
        }
    };
    tokio::select! {
        () = interrupt => {}
        () = terminate() => {}
    }
    info!("shutting down");
}

#[cfg(unix)]
async fn terminate() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            terminate.recv().await;
        }
        Err(error) => {
            warn!(%error, "cannot listen for SIGTERM");
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(not(unix))]
async fn terminate() {
    std::future::pending::<()>().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_for_terminals_that_do_not_opt_out() {
        assert!(colours_wanted(true, None));
        assert!(colours_wanted(true, Some(OsStr::new(""))));
        assert!(!colours_wanted(true, Some(OsStr::new("1"))));
        assert!(!colours_wanted(false, None));
    }
}
