/*!
 * WebSocket connection to a race server.
 *
 * [`Connection::open`] returns immediately: connecting, the handshake and the
 * traffic run in a background task that reports through [`NetworkEvent`]s.
 */

mod address;
mod connect;
mod relay;
#[cfg(test)]
mod tests;

use std::time::Duration;

use code_racer_protocol::{ClientMessage, PlayerId, ServerMessage, Username};
use tokio::{net::TcpStream, runtime::Handle, sync::mpsc, task::JoinHandle, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub use address::{is_local_only, lan_address, server_url, with_host};
use connect::connect;
use relay::relay;

const OUTGOING_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const CONNECTION_LOST: &str = "connection lost";
const CLOSED_BY_CLIENT: &str = "connection closed by the client";
const SERVER_SILENT: &str = "the server stopped responding";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone, Copy)]
struct Timeouts {
    /// Reaching the server and upgrading to WebSocket.
    connect: Duration,
    /// Getting the welcome once the hello is sent.
    handshake: Duration,
    /**
     * Sending one frame, or closing: a server that takes longer stopped
     * reading.
     */
    write: Duration,
    /**
     * Longest time without any frame from the server, which notices a
     * server that vanished without closing the connection, such as a machine
     * that lost the network or went to sleep.
     */
    silence: Duration,
}

impl Timeouts {
    /**
     * The client pings three times per silence period, so a live server
     * always has pongs to send in time even when nothing else happens.
     */
    fn ping_interval(self) -> Duration {
        self.silence / 3
    }
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(5),
            handshake: Duration::from_secs(5),
            write: Duration::from_secs(5),
            silence: Duration::from_secs(45),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkEvent {
    Connected(PlayerId),
    Message(ServerMessage),
    /// Always the last event.
    Closed {
        reason: String,
    },
}

/**
 * Dropping a connection never blocks: the background task still sends the
 * messages already queued, such as a last [`ClientMessage::LeaveRoom`], then
 * closes the WebSocket and stops, as long as the runtime runs. A program
 * about to exit waits for that with [`Connection::close`].
 */
#[derive(Debug)]
pub struct Connection {
    outgoing: mpsc::Sender<ClientMessage>,
    events: mpsc::Receiver<NetworkEvent>,
    /// `None` when the connection was born closed.
    task: Option<JoinHandle<()>>,
}

impl Connection {
    /**
     * Starts connecting to `url` on the current tokio runtime.
     *
     * Outside of a runtime the connection is born closed.
     */
    pub fn open(url: String, username: Username) -> Self {
        Self::open_with(url, username, Timeouts::default())
    }

    fn open_with(url: String, username: Username, timeouts: Timeouts) -> Self {
        let (outgoing, outgoing_receiver) = mpsc::channel(OUTGOING_CAPACITY);
        let (event_sender, events) = mpsc::channel(EVENT_CAPACITY);
        let task = match Handle::try_current() {
            Ok(runtime) => Some(runtime.spawn(run(
                url,
                username,
                outgoing_receiver,
                event_sender,
                timeouts,
            ))),
            Err(error) => {
                let reason = error.to_string();
                let _ = event_sender.try_send(NetworkEvent::Closed { reason });
                None
            }
        };
        Self {
            outgoing,
            events,
            task,
        }
    }

    /**
     * Closes the connection, waiting at most `limit` for the messages
     * already queued to be sent and the WebSocket to be closed.
     */
    pub async fn close(self, limit: Duration) {
        let Self {
            outgoing,
            events,
            task,
        } = self;
        drop((outgoing, events));
        if let Some(task) = task {
            let _ = timeout(limit, task).await;
        }
    }

    /**
     * Queues a message for the server. Returns `false` when the connection
     * is closed or too far behind to accept more.
     */
    pub fn send(&self, message: ClientMessage) -> bool {
        self.outgoing.try_send(message).is_ok()
    }

    /**
     * The next event, or `None` once [`NetworkEvent::Closed`] was delivered.
     * Cancel safe.
     */
    pub async fn next_event(&mut self) -> Option<NetworkEvent> {
        self.events.recv().await
    }
}

#[cfg(test)]
impl Connection {
    /// A connection to nowhere that hands the messages sent to the test.
    pub(crate) fn loopback() -> (Self, mpsc::Receiver<ClientMessage>) {
        let (outgoing, sent) = mpsc::channel(OUTGOING_CAPACITY);
        let (_, events) = mpsc::channel(EVENT_CAPACITY);
        let connection = Self {
            outgoing,
            events,
            task: None,
        };
        (connection, sent)
    }
}

/**
 * The background task of a [`Connection`]. Connecting is abandoned as soon
 * as the connection is dropped: nobody would hear of the result.
 */
async fn run(
    url: String,
    username: Username,
    mut outgoing: mpsc::Receiver<ClientMessage>,
    events: mpsc::Sender<NetworkEvent>,
    timeouts: Timeouts,
) {
    let connected = tokio::select! {
        connected = connect(&url, username, timeouts) => connected,
        () = events.closed() => return,
    };
    let reason = match connected {
        Ok((socket, player_id)) => {
            let _ = events.send(NetworkEvent::Connected(player_id)).await;
            relay(socket, &mut outgoing, &events, timeouts)
                .await
                .to_owned()
        }
        Err(reason) => reason,
    };
    let _ = events.send(NetworkEvent::Closed { reason }).await;
}
