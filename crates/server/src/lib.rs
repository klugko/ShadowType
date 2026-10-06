//! Race server for code-racer: rooms and races behind a WebSocket endpoint.
//!
//! Each connection runs in its own task and talks to a single hub task that
//! owns every room, so room state is never shared or locked.

mod config;
mod connection;
mod hub;
mod room;

use std::{future::Future, io, sync::Arc, time::Duration};

use code_racer_protocol::{ErrorCode, PlayerId, ServerError};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc},
    task::{JoinError, JoinSet},
    time,
};
use tracing::{debug, warn};

pub use config::ServerConfig;

use crate::hub::{Command, Hub};

const COMMAND_QUEUE: usize = 1024;
/// Connections being turned away at once; further ones are dropped without a word.
const MAX_REFUSALS: usize = 32;
/// Pause after a failed accept, typically when the process ran out of file descriptors.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
/// Time connections get to close cleanly once the server stops.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Accepts players on `listener` until `shutdown` completes, then closes every connection.
///
/// Fails only if the task owning the rooms stops unexpectedly.
pub async fn serve(
    listener: TcpListener,
    config: ServerConfig,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let (commands, receiver) = mpsc::channel(COMMAND_QUEUE);
    let admission = Admission::new(config.max_connections);
    let mut hub = tokio::spawn(hub::run(Hub::new(config), receiver));
    let mut connections = JoinSet::new();
    let mut last_id = 0;
    let mut shutdown = std::pin::pin!(shutdown);
    let outcome = loop {
        tokio::select! {
            () = &mut shutdown => break Ok(()),
            stopped = &mut hub => break Err(hub_failure(stopped)),
            Some(finished) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = finished {
                    warn!(%error, "a connection task failed");
                }
            }
            accepted = listener.accept() => match accepted {
                Ok((stream, address)) => {
                    debug!(%address, "connection accepted");
                    last_id += 1;
                    admission.admit(&mut connections, stream, PlayerId(last_id), &commands);
                }
                Err(error) => {
                    warn!(%error, "cannot accept a connection");
                    time::sleep(ACCEPT_BACKOFF).await;
                }
            },
        }
    };
    hub.abort();
    close_all(connections).await;
    outcome
}

fn hub_failure(stopped: Result<(), JoinError>) -> io::Error {
    io::Error::other(format!("the room hub stopped unexpectedly: {stopped:?}"))
}

/// Bounds how many players are served, and how many are being refused, at once.
#[derive(Debug)]
struct Admission {
    players: Arc<Semaphore>,
    refusals: Arc<Semaphore>,
}

impl Admission {
    fn new(max_connections: usize) -> Self {
        Self {
            players: Arc::new(Semaphore::new(max_connections.min(Semaphore::MAX_PERMITS))),
            refusals: Arc::new(Semaphore::new(MAX_REFUSALS)),
        }
    }

    fn admit(
        &self,
        connections: &mut JoinSet<()>,
        stream: TcpStream,
        id: PlayerId,
        commands: &mpsc::Sender<Command>,
    ) {
        if let Ok(permit) = Arc::clone(&self.players).try_acquire_owned() {
            let hub = commands.clone();
            connections.spawn(async move {
                let _permit = permit;
                connection::serve_player(stream, id, hub).await;
            });
        } else if let Ok(permit) = Arc::clone(&self.refusals).try_acquire_owned() {
            let full =
                ServerError::new(ErrorCode::ServerFull, "the server is full, try again later");
            connections.spawn(async move {
                let _permit = permit;
                connection::turn_away(stream, full).await;
            });
        } else {
            debug!("connection dropped, too many are already being refused");
        }
    }
}

/// Lets connections finish their goodbyes, then cuts the ones still open.
async fn close_all(mut connections: JoinSet<()>) {
    let drained = time::timeout(SHUTDOWN_GRACE, async {
        while connections.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        connections.shutdown().await;
    }
}
