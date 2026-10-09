//! Race server for code-racer: rooms and races behind a WebSocket endpoint.
//!
//! Each connection runs in its own task and talks to a single hub task that
//! owns every room, so room state is never shared or locked.

mod config;
mod connection;
mod hub;
mod peers;
mod room;

use std::{future::Future, io, net::IpAddr, sync::Arc, time::Duration};

use code_racer_protocol::{ErrorCode, PlayerId, ServerError};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc},
    task::{JoinError, JoinSet},
    time,
};
use tracing::{debug, warn};

pub use config::ServerConfig;

use crate::{
    hub::{Command, Hub},
    peers::PeerSlots,
};

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
    let admission = Admission::new(&config);
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
                    let id = PlayerId(last_id);
                    admission.admit(&mut connections, stream, address.ip(), id, &commands);
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

/// Bounds how many players are served, how many are being refused, and how
/// many connections one address holds, at once.
#[derive(Debug)]
struct Admission {
    players: Arc<Semaphore>,
    refusals: Arc<Semaphore>,
    peers: Arc<PeerSlots>,
}

impl Admission {
    fn new(config: &ServerConfig) -> Self {
        let players = config.max_connections.min(Semaphore::MAX_PERMITS);
        Self {
            players: Arc::new(Semaphore::new(players)),
            refusals: Arc::new(Semaphore::new(MAX_REFUSALS)),
            peers: Arc::new(PeerSlots::new(config.max_connections_per_address)),
        }
    }

    /// Serves or refuses a new connection. A peer over its share is dropped
    /// without a word, so that it cannot fill the refusal slots either.
    fn admit(
        &self,
        connections: &mut JoinSet<()>,
        stream: TcpStream,
        peer: IpAddr,
        id: PlayerId,
        commands: &mpsc::Sender<Command>,
    ) {
        let Some(slot) = self.peers.claim(peer) else {
            debug!(%peer, "connection dropped, this address holds too many already");
            return;
        };
        if let Ok(permit) = Arc::clone(&self.players).try_acquire_owned() {
            let hub = commands.clone();
            connections.spawn(async move {
                let _held = (permit, slot);
                connection::serve_player(stream, id, hub).await;
            });
        } else if let Ok(permit) = Arc::clone(&self.refusals).try_acquire_owned() {
            let full =
                ServerError::new(ErrorCode::ServerFull, "the server is full, try again later");
            connections.spawn(async move {
                let _held = (permit, slot);
                connection::turn_away(stream, full).await;
            });
        } else {
            debug!("connection dropped, too many are already being refused");
        }
    }
}

async fn close_all(mut connections: JoinSet<()>) {
    let drained = time::timeout(SHUTDOWN_GRACE, async {
        while connections.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        connections.shutdown().await;
    }
}
