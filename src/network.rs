//! WebSocket connection to a race server.
//!
//! [`Connection::open`] returns immediately: connecting, the handshake and the
//! traffic run in a background task that reports through [`NetworkEvent`]s.

use std::{ops::ControlFlow, time::Duration};

use code_racer_protocol::{
    ClientMessage, MAX_MESSAGE_BYTES, PROTOCOL_VERSION, PlayerId, ServerMessage, Username,
};
use futures_util::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::{
    net::TcpStream,
    runtime::Handle,
    sync::mpsc,
    time::{self, Instant, MissedTickBehavior, timeout},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{self, Bytes, Message, protocol::WebSocketConfig},
};

const OUTGOING_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const CONNECTION_LOST: &str = "connection lost";
const CLOSED_BY_CLIENT: &str = "connection closed by the client";
const SERVER_SILENT: &str = "the server stopped responding";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// How long the connection waits on the server.
#[derive(Debug, Clone, Copy)]
struct Timeouts {
    /// Reaching the server and upgrading to WebSocket.
    connect: Duration,
    /// Getting the welcome once the hello is sent.
    handshake: Duration,
    /// Sending one frame, or closing: a server that takes longer stopped
    /// reading.
    write: Duration,
    /// Longest time without any frame from the server, which notices a
    /// server that vanished without closing the connection, such as a machine
    /// that lost the network or went to sleep.
    silence: Duration,
}

impl Timeouts {
    /// The client pings three times per silence period, so a live server
    /// always has pongs to send in time even when nothing else happens.
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidServerUrl {
    #[error("the server address is empty")]
    Empty,
    #[error("`{0}://` is not supported, use ws:// or wss://")]
    Scheme(String),
    #[error("`{0}` does not name a host")]
    Host(String),
    #[error("`{0}` has an invalid port, use a number up to 65535")]
    Port(String),
}

/// Turns what a user typed into a WebSocket URL.
///
/// An address without scheme gets `ws://`, `http` and `https` become `ws` and
/// `wss`, and any other scheme is refused. Normalising twice changes nothing.
pub fn server_url(input: &str) -> Result<String, InvalidServerUrl> {
    let input = input.trim();
    if input.is_empty() {
        return Err(InvalidServerUrl::Empty);
    }
    let (scheme, address) = match input.split_once("://") {
        Some((scheme, address)) => (websocket_scheme(scheme)?, address),
        None => ("ws", input),
    };
    let invalid_host = || InvalidServerUrl::Host(input.to_owned());
    let (host, port) = host_and_port(address).ok_or_else(invalid_host)?;
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err(invalid_host());
    }
    if port.is_some_and(|port| !is_port(port)) {
        return Err(InvalidServerUrl::Port(input.to_owned()));
    }
    Ok(format!("{scheme}://{address}"))
}

fn websocket_scheme(scheme: &str) -> Result<&'static str, InvalidServerUrl> {
    match scheme.to_ascii_lowercase().as_str() {
        "ws" | "http" => Ok("ws"),
        "wss" | "https" => Ok("wss"),
        _ => Err(InvalidServerUrl::Scheme(scheme.to_owned())),
    }
}

/// Host and port of the authority in `address`, `None` when an IPv6 address
/// is not properly bracketed.
fn host_and_port(address: &str) -> Option<(&str, Option<&str>)> {
    let authority = address.split(['/', '?', '#']).next().unwrap_or_default();
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_and_port)| host_and_port);
    let Some(bracketed) = host_and_port.strip_prefix('[') else {
        return Some(match host_and_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_and_port, None),
        });
    };
    let (host, rest) = bracketed.split_once(']')?;
    match rest.strip_prefix(':') {
        Some(port) => Some((host, Some(port))),
        None => rest.is_empty().then_some((host, None)),
    }
}

fn is_port(port: &str) -> bool {
    port.bytes().all(|byte| byte.is_ascii_digit()) && port.parse::<u16>().is_ok()
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkEvent {
    /// The server accepted the handshake and assigned this id.
    Connected(PlayerId),
    Message(ServerMessage),
    /// The connection is over. It is the last event.
    Closed {
        reason: String,
    },
}

/// A connection to a race server.
///
/// Dropping it never blocks: the background task still sends the messages
/// already queued, such as a last [`ClientMessage::LeaveRoom`], then closes
/// the WebSocket and stops.
#[derive(Debug)]
pub struct Connection {
    outgoing: mpsc::Sender<ClientMessage>,
    events: mpsc::Receiver<NetworkEvent>,
}

impl Connection {
    /// Starts connecting to `url` on the current tokio runtime.
    ///
    /// Outside of a runtime the connection is born closed.
    pub fn open(url: String, username: Username) -> Self {
        Self::open_with(url, username, Timeouts::default())
    }

    fn open_with(url: String, username: Username, timeouts: Timeouts) -> Self {
        let (outgoing, outgoing_receiver) = mpsc::channel(OUTGOING_CAPACITY);
        let (event_sender, events) = mpsc::channel(EVENT_CAPACITY);
        match Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(run(
                    url,
                    username,
                    outgoing_receiver,
                    event_sender,
                    timeouts,
                ));
            }
            Err(error) => {
                let reason = error.to_string();
                let _ = event_sender.try_send(NetworkEvent::Closed { reason });
            }
        }
        Self { outgoing, events }
    }

    /// Queues a message for the server. Returns `false` when the connection
    /// is closed or too far behind to accept more.
    pub fn send(&self, message: ClientMessage) -> bool {
        self.outgoing.try_send(message).is_ok()
    }

    /// The next event, or `None` once [`NetworkEvent::Closed`] was delivered.
    /// Cancel safe.
    pub async fn next_event(&mut self) -> Option<NetworkEvent> {
        self.events.recv().await
    }
}

/// The background task of a [`Connection`]. Connecting is abandoned as soon
/// as the connection is dropped: nobody would hear of the result.
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

async fn connect(
    url: &str,
    username: Username,
    timeouts: Timeouts,
) -> Result<(Socket, PlayerId), String> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let unreachable = |detail: String| format!("cannot reach {url}: {detail}");
    let (mut socket, _) = timeout(
        timeouts.connect,
        connect_async_with_config(url, Some(config), true),
    )
    .await
    .map_err(|_| unreachable("timed out".to_owned()))?
    .map_err(|error| unreachable(describe(&error)))?;
    let player_id = timeout(timeouts.handshake, handshake(&mut socket, username))
        .await
        .map_err(|_| format!("{url} did not answer the handshake"))??;
    Ok((socket, player_id))
}

async fn handshake(socket: &mut Socket, username: Username) -> Result<PlayerId, String> {
    let hello = ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        username,
    };
    socket
        .send(Message::text(hello.to_json()))
        .await
        .map_err(|_| CONNECTION_LOST.to_owned())?;
    loop {
        match decode(socket.next().await) {
            Incoming::Message(ServerMessage::Welcome { player_id, .. }) => return Ok(player_id),
            Incoming::Message(ServerMessage::Error(error)) => return Err(error.message),
            Incoming::Message(unexpected) => {
                tracing::warn!(
                    ?unexpected,
                    "skipping a message received before the welcome"
                );
            }
            Incoming::Ignored => {}
            Incoming::Closed => return Err(CONNECTION_LOST.to_owned()),
        }
    }
}

/// Forwards messages both ways until the server goes away, falls silent or
/// the [`Connection`] is dropped, and returns why it stopped.
///
/// Outgoing messages come first, so that the ones queued before the drop
/// are all sent before the socket is closed. Only frames from the server
/// prove that it is alive: sending succeeds long after it vanished, as the
/// operating system buffers the data and retries for minutes.
async fn relay(
    mut socket: Socket,
    outgoing: &mut mpsc::Receiver<ClientMessage>,
    events: &mpsc::Sender<NetworkEvent>,
    timeouts: Timeouts,
) -> &'static str {
    let silence = time::sleep(timeouts.silence);
    tokio::pin!(silence);
    let mut pings = time::interval_at(
        Instant::now() + timeouts.ping_interval(),
        timeouts.ping_interval(),
    );
    pings.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let step = tokio::select! {
            biased;
            message = outgoing.recv() => send_next(&mut socket, message, timeouts.write).await,
            frame = socket.next() => {
                silence.as_mut().reset(Instant::now() + timeouts.silence);
                deliver(decode(frame), outgoing, events).await
            }
            _ = pings.tick() => {
                send_frame(&mut socket, Message::Ping(Bytes::new()), timeouts.write).await
            }
            () = &mut silence => ControlFlow::Break(SERVER_SILENT),
        };
        if let ControlFlow::Break(reason) = step {
            return reason;
        }
    }
}

/// Sends the next queued message, or closes the socket once the
/// [`Connection`] is gone and the queue is empty.
async fn send_next(
    socket: &mut Socket,
    message: Option<ClientMessage>,
    limit: Duration,
) -> ControlFlow<&'static str> {
    match message {
        Some(message) => send_frame(socket, Message::text(message.to_json()), limit).await,
        None => {
            hang_up(socket, limit).await;
            ControlFlow::Break(CLOSED_BY_CLIENT)
        }
    }
}

/// Sends `frame`, giving up after `limit`.
async fn send_frame(
    socket: &mut Socket,
    frame: Message,
    limit: Duration,
) -> ControlFlow<&'static str> {
    match timeout(limit, socket.send(frame)).await {
        Ok(Ok(())) => ControlFlow::Continue(()),
        _ => ControlFlow::Break(CONNECTION_LOST),
    }
}

/// Hands a server message to the application. When the application stopped
/// listening, the queue of outgoing messages is closed too, which ends the
/// relay once it is flushed.
async fn deliver(
    incoming: Incoming,
    outgoing: &mut mpsc::Receiver<ClientMessage>,
    events: &mpsc::Sender<NetworkEvent>,
) -> ControlFlow<&'static str> {
    match incoming {
        Incoming::Message(message) => {
            if events.send(NetworkEvent::Message(message)).await.is_err() {
                outgoing.close();
            }
            ControlFlow::Continue(())
        }
        Incoming::Ignored => ControlFlow::Continue(()),
        Incoming::Closed => ControlFlow::Break(CONNECTION_LOST),
    }
}

/// Sends a close frame and reads until the server answers it, for at most
/// `limit`. Dropping a socket with unread data would reset the connection,
/// which can destroy the last messages before the server reads them.
async fn hang_up(socket: &mut Socket, limit: Duration) {
    let closing = async {
        socket.close(None).await?;
        while let Some(Ok(_)) = socket.next().await {}
        Ok::<(), tungstenite::Error>(())
    };
    if let Ok(Err(error)) = timeout(limit, closing).await {
        tracing::debug!(%error, "could not close the connection cleanly");
    }
}

enum Incoming {
    Message(ServerMessage),
    Ignored,
    Closed,
}

fn decode(frame: Option<Result<Message, tungstenite::Error>>) -> Incoming {
    match frame {
        Some(Ok(Message::Text(text))) => match ServerMessage::from_json(&text) {
            Ok(message) => Incoming::Message(message),
            Err(error) => {
                tracing::warn!(%error, "skipping an unreadable server message");
                Incoming::Ignored
            }
        },
        Some(Ok(Message::Binary(_))) => {
            tracing::warn!("skipping a binary frame from the server");
            Incoming::Ignored
        }
        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => Incoming::Ignored,
        Some(Ok(Message::Close(frame))) => {
            tracing::info!(?frame, "server closed the connection");
            Incoming::Closed
        }
        Some(Err(error)) => {
            tracing::warn!(%error, "connection failed");
            Incoming::Closed
        }
        None => Incoming::Closed,
    }
}

fn describe(error: &tungstenite::Error) -> String {
    match error {
        tungstenite::Error::Io(io) => describe_io(io),
        tungstenite::Error::Http(response) => {
            format!("not a race server (HTTP {})", response.status())
        }
        other => other.to_string(),
    }
}

/// The short, platform-independent name of common network failures, and
/// otherwise the system's own message. The kind alone is useless for codes
/// that std does not map, such as an unknown host on Windows, which would
/// read "uncategorized error".
fn describe_io(error: &std::io::Error) -> String {
    use std::io::ErrorKind::{
        AddrNotAvailable, ConnectionAborted, ConnectionRefused, ConnectionReset, HostUnreachable,
        NetworkDown, NetworkUnreachable, PermissionDenied, TimedOut,
    };
    match error.kind() {
        ConnectionRefused | ConnectionReset | ConnectionAborted | TimedOut | HostUnreachable
        | NetworkUnreachable | NetworkDown | AddrNotAvailable | PermissionDenied => {
            error.kind().to_string()
        }
        _ => {
            let mut message = error.to_string();
            if let Some(code) = message.rfind(" (os error ") {
                message.truncate(code);
            }
            message
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
    /// Replaces a production timeout that a test waits for on purpose.
    const SHORT_TIMEOUT: Duration = Duration::from_millis(100);
    /// Replaces the silence timeout where a test needs the client to ping:
    /// long enough that a busy machine still answers in time.
    const SHORT_SILENCE: Duration = Duration::from_millis(400);

    fn username() -> Username {
        "Tester".parse().expect("valid name")
    }

    async fn next(connection: &mut Connection) -> Option<NetworkEvent> {
        timeout(EVENT_TIMEOUT, connection.next_event())
            .await
            .expect("an event before the timeout")
    }

    async fn closed_reason(connection: &mut Connection) -> String {
        match next(connection).await {
            Some(NetworkEvent::Closed { reason }) => reason,
            other => panic!("expected Closed, got {other:?}"),
        }
    }

    #[test]
    fn server_urls_are_normalised() {
        let cases = [
            ("192.168.1.20:8080", "ws://192.168.1.20:8080"),
            ("  localhost:9000 ", "ws://localhost:9000"),
            ("ws://race.lan:8080", "ws://race.lan:8080"),
            ("wss://race.example.com", "wss://race.example.com"),
            ("http://race.lan:8080/", "ws://race.lan:8080/"),
            ("HTTPS://race.example.com", "wss://race.example.com"),
            ("ws://[::1]:8080", "ws://[::1]:8080"),
            ("[::1]", "ws://[::1]"),
            ("ws://player@race.lan", "ws://player@race.lan"),
            ("race.lan:65535/rooms?x=1", "ws://race.lan:65535/rooms?x=1"),
        ];
        for (input, expected) in cases {
            assert_eq!(server_url(input).as_deref(), Ok(expected), "{input}");
            assert_eq!(server_url(expected).as_deref(), Ok(expected), "{expected}");
        }
    }

    #[test]
    fn invalid_server_urls_are_rejected() {
        let cases = [
            ("", InvalidServerUrl::Empty),
            ("   ", InvalidServerUrl::Empty),
            ("ftp://race.lan", InvalidServerUrl::Scheme("ftp".to_owned())),
            ("ws://", InvalidServerUrl::Host("ws://".to_owned())),
            (
                "ws://:8080",
                InvalidServerUrl::Host("ws://:8080".to_owned()),
            ),
            (
                "wss:///path",
                InvalidServerUrl::Host("wss:///path".to_owned()),
            ),
            (
                "ws://[]:80",
                InvalidServerUrl::Host("ws://[]:80".to_owned()),
            ),
            (
                "my server:80",
                InvalidServerUrl::Host("my server:80".to_owned()),
            ),
            ("ws://[::1", InvalidServerUrl::Host("ws://[::1".to_owned())),
            (
                "ws://[::1]8080",
                InvalidServerUrl::Host("ws://[::1]8080".to_owned()),
            ),
            ("::1", InvalidServerUrl::Host("::1".to_owned())),
            ("race.lan:", InvalidServerUrl::Port("race.lan:".to_owned())),
            (
                "race.lan:http",
                InvalidServerUrl::Port("race.lan:http".to_owned()),
            ),
            (
                "race.lan:65536",
                InvalidServerUrl::Port("race.lan:65536".to_owned()),
            ),
            (
                "race.lan:+80",
                InvalidServerUrl::Port("race.lan:+80".to_owned()),
            ),
            (
                "ws://[::1]:x",
                InvalidServerUrl::Port("ws://[::1]:x".to_owned()),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(server_url(input), Err(expected), "{input:?}");
        }
    }

    #[test]
    fn socket_errors_are_described_without_os_codes() {
        let describe_io = |error: std::io::Error| describe(&tungstenite::Error::Io(error));
        let unknown_host = describe_io(std::io::Error::from_raw_os_error(11001));

        assert!(!unknown_host.contains("uncategorized"), "{unknown_host}");
        assert!(!unknown_host.contains("os error"), "{unknown_host}");
        assert!(!unknown_host.is_empty());
        assert_eq!(
            describe_io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "No connection could be made (os error 10061)"
            )),
            "connection refused"
        );
        assert_eq!(
            describe_io(std::io::Error::other(
                "failed to lookup address information"
            )),
            "failed to lookup address information"
        );
    }

    #[tokio::test]
    async fn unreachable_server_reports_a_precise_reason() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        drop(listener);
        let url = format!("ws://{address}");

        let mut connection = Connection::open(url.clone(), username());

        assert_eq!(
            closed_reason(&mut connection).await,
            format!("cannot reach {url}: connection refused")
        );
        assert_eq!(next(&mut connection).await, None);
        assert!(!connection.send(ClientMessage::LeaveRoom));
    }

    #[tokio::test]
    async fn server_that_never_upgrades_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("ws://{}", listener.local_addr().expect("address"));

        let timeouts = Timeouts {
            connect: SHORT_TIMEOUT,
            ..Timeouts::default()
        };
        let mut connection = Connection::open_with(url.clone(), username(), timeouts);

        assert_eq!(
            closed_reason(&mut connection).await,
            format!("cannot reach {url}: timed out")
        );
        drop(listener);
    }

    #[tokio::test]
    async fn sending_while_connecting_refuses_rather_than_blocks() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("ws://{}", listener.local_addr().expect("address"));
        let connection = Connection::open(url, username());

        let accepted = (0..OUTGOING_CAPACITY + 10)
            .filter(|_| connection.send(ClientMessage::LeaveRoom))
            .count();

        assert_eq!(accepted, OUTGOING_CAPACITY);
        drop(listener);
    }

    #[test]
    fn opening_outside_a_runtime_is_closed_immediately() {
        let mut connection = Connection::open("ws://127.0.0.1:1".to_owned(), username());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let event = runtime.block_on(connection.next_event());
        assert!(
            matches!(event, Some(NetworkEvent::Closed { .. })),
            "{event:?}"
        );
        assert!(!connection.send(ClientMessage::LeaveRoom));
    }

    mod against_a_scripted_server {
        use code_racer_protocol::ErrorCode;
        use tokio::sync::oneshot;

        use super::*;

        type ServerSocket = WebSocketStream<TcpStream>;

        /// Accepts one WebSocket client and hands it to `script`.
        async fn scripted_server<F, Fut>(script: F) -> String
        where
            F: FnOnce(ServerSocket) -> Fut + Send + 'static,
            Fut: Future<Output = ()> + Send,
        {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let address = listener.local_addr().expect("address");
            tokio::spawn(async move {
                let (stream, _) = listener.accept().await.expect("accept");
                let socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("websocket handshake");
                script(socket).await;
            });
            format!("ws://{address}")
        }

        async fn receive(socket: &mut ServerSocket) -> ClientMessage {
            loop {
                match socket.next().await {
                    Some(Ok(Message::Text(text))) => {
                        return ClientMessage::from_json(&text).expect("client message");
                    }
                    Some(Ok(_)) => continue,
                    other => panic!("client went away: {other:?}"),
                }
            }
        }

        async fn send(socket: &mut ServerSocket, message: ServerMessage) {
            socket
                .send(Message::text(message.to_json()))
                .await
                .expect("send to client");
        }

        async fn accept_hello(socket: &mut ServerSocket, id: u64) {
            match receive(socket).await {
                ClientMessage::Hello { version, username } => {
                    assert_eq!(version, PROTOCOL_VERSION);
                    assert_eq!(username.as_str(), "Tester");
                }
                other => panic!("expected Hello, got {other:?}"),
            }
            send(
                socket,
                ServerMessage::Welcome {
                    version: PROTOCOL_VERSION,
                    player_id: PlayerId(id),
                },
            )
            .await;
        }

        #[tokio::test]
        async fn messages_flow_both_ways_after_the_handshake() {
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 7).await;
                let echoed = match receive(&mut socket).await {
                    ClientMessage::SetReady { ready } => ServerMessage::error(
                        code_racer_protocol::ErrorCode::NotInRoom,
                        format!("ready={ready}"),
                    ),
                    other => panic!("unexpected {other:?}"),
                };
                send(&mut socket, echoed).await;
                let _ = socket.next().await;
            })
            .await;

            let mut connection = Connection::open(url, username());

            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(7)))
            );
            assert!(connection.send(ClientMessage::SetReady { ready: true }));
            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Message(ServerMessage::error(
                    code_racer_protocol::ErrorCode::NotInRoom,
                    "ready=true"
                )))
            );
        }

        #[tokio::test]
        async fn handshake_error_becomes_the_close_reason() {
            let url = scripted_server(|mut socket| async move {
                let _ = receive(&mut socket).await;
                send(
                    &mut socket,
                    ServerMessage::error(
                        code_racer_protocol::ErrorCode::IncompatibleVersion,
                        "server speaks protocol v3, client v2",
                    ),
                )
                .await;
                let _ = socket.close(None).await;
            })
            .await;

            let mut connection = Connection::open(url, username());

            assert_eq!(
                closed_reason(&mut connection).await,
                "server speaks protocol v3, client v2"
            );
            assert_eq!(next(&mut connection).await, None);
        }

        #[tokio::test]
        async fn unreadable_frames_are_skipped() {
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 1).await;
                for garbage in [Message::text("not json"), Message::binary(vec![1, 2, 3])] {
                    socket.send(garbage).await.expect("send garbage");
                }
                let countdown = ServerMessage::Countdown {
                    text: "fn main() {}".to_owned(),
                    duration_ms: 3_000,
                };
                send(&mut socket, countdown).await;
                let _ = socket.next().await;
            })
            .await;

            let mut connection = Connection::open(url, username());

            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(1)))
            );
            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Message(ServerMessage::Countdown {
                    text: "fn main() {}".to_owned(),
                    duration_ms: 3_000,
                }))
            );
        }

        #[tokio::test]
        async fn server_going_away_closes_exactly_once() {
            let (hang_up, hung_up) = oneshot::channel::<()>();
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 3).await;
                let _ = hung_up.await;
                drop(socket);
            })
            .await;

            let mut connection = Connection::open(url, username());
            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(3)))
            );
            hang_up.send(()).expect("server still running");

            assert_eq!(closed_reason(&mut connection).await, CONNECTION_LOST);
            assert_eq!(next(&mut connection).await, None);
            assert!(!connection.send(ClientMessage::LeaveRoom));
        }

        #[tokio::test]
        async fn messages_queued_while_connecting_follow_the_hello() {
            let (report, received) = oneshot::channel();
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 4).await;
                let _ = report.send(receive(&mut socket).await);
                let _ = socket.next().await;
            })
            .await;

            let mut connection = Connection::open(url, username());
            assert!(connection.send(ClientMessage::StartRace));

            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(4)))
            );
            let first = timeout(EVENT_TIMEOUT, received).await.expect("in time");
            assert_eq!(first.ok(), Some(ClientMessage::StartRace));
        }

        #[tokio::test]
        async fn dropping_the_connection_sends_what_was_queued_then_closes() {
            let (report, received) = oneshot::channel();
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 5).await;
                let queued = receive(&mut socket).await;
                let after = loop {
                    match socket.next().await {
                        Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                        other => break other,
                    }
                };
                let _ = report.send((queued, after));
            })
            .await;
            let mut connection = Connection::open(url, username());
            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(5)))
            );

            assert!(connection.send(ClientMessage::LeaveRoom));
            drop(connection);

            let (queued, after) = timeout(EVENT_TIMEOUT, received)
                .await
                .expect("in time")
                .expect("the server saw the client leave");
            assert_eq!(queued, ClientMessage::LeaveRoom);
            assert!(matches!(after, Some(Ok(Message::Close(_)))), "{after:?}");
        }

        #[tokio::test]
        async fn dropping_during_the_handshake_hangs_up_at_once() {
            let (greeted, hello) = oneshot::channel();
            let (report, gone) = oneshot::channel();
            let url = scripted_server(|mut socket| async move {
                let _ = greeted.send(receive(&mut socket).await);
                let _ = report.send(!matches!(socket.next().await, Some(Ok(_))));
            })
            .await;
            let timeouts = Timeouts {
                handshake: Duration::from_secs(60),
                ..Timeouts::default()
            };
            let connection = Connection::open_with(url, username(), timeouts);
            timeout(EVENT_TIMEOUT, hello)
                .await
                .expect("in time")
                .expect("hello");

            drop(connection);

            let hung_up = timeout(EVENT_TIMEOUT, gone).await.expect("in time");
            assert_eq!(hung_up.ok(), Some(true));
        }

        #[tokio::test]
        async fn a_server_that_falls_silent_is_given_up_on() {
            let (keep_alive, kept) = oneshot::channel::<()>();
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 9).await;
                let _ = kept.await;
            })
            .await;
            let timeouts = Timeouts {
                silence: SHORT_SILENCE,
                ..Timeouts::default()
            };
            let started = std::time::Instant::now();
            let mut connection = Connection::open_with(url, username(), timeouts);
            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(9)))
            );

            let event = timeout(EVENT_TIMEOUT, async {
                loop {
                    tokio::select! {
                        event = connection.next_event() => break event,
                        () = time::sleep(SHORT_SILENCE / 8) => {
                            connection.send(ClientMessage::SetReady { ready: true });
                        }
                    }
                }
            })
            .await
            .expect("in time");

            assert_eq!(
                event,
                Some(NetworkEvent::Closed {
                    reason: SERVER_SILENT.to_owned()
                })
            );
            assert!(started.elapsed() >= SHORT_SILENCE);
            drop(keep_alive);
        }

        #[tokio::test]
        async fn a_server_that_answers_pings_is_not_silent() {
            let url = scripted_server(|mut socket| async move {
                accept_hello(&mut socket, 10).await;
                let answer_pings = async { while let Some(Ok(_)) = socket.next().await {} };
                let _ = timeout(SHORT_SILENCE * 5 / 2, answer_pings).await;
                send(
                    &mut socket,
                    ServerMessage::error(ErrorCode::NotInRoom, "still here"),
                )
                .await;
                let _ = socket.next().await;
            })
            .await;
            let timeouts = Timeouts {
                silence: SHORT_SILENCE,
                ..Timeouts::default()
            };
            let mut connection = Connection::open_with(url, username(), timeouts);
            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Connected(PlayerId(10)))
            );

            assert_eq!(
                next(&mut connection).await,
                Some(NetworkEvent::Message(ServerMessage::error(
                    ErrorCode::NotInRoom,
                    "still here"
                )))
            );
        }

        #[tokio::test]
        async fn silent_server_times_out_the_handshake() {
            let url = scripted_server(|mut socket| async move {
                let _ = receive(&mut socket).await;
                let _ = socket.next().await;
            })
            .await;

            let timeouts = Timeouts {
                handshake: SHORT_TIMEOUT,
                ..Timeouts::default()
            };
            let mut connection = Connection::open_with(url.clone(), username(), timeouts);

            assert_eq!(
                closed_reason(&mut connection).await,
                format!("{url} did not answer the handshake")
            );
        }
    }

    mod against_the_race_server {
        use code_racer_engine::{Language, TextSource, WordOptions};
        use code_racer_protocol::Phase;
        use code_racer_server::{ServerConfig, serve};
        use tokio::{sync::oneshot, task::JoinHandle};

        use super::*;

        struct RunningServer {
            url: String,
            stop: oneshot::Sender<()>,
            task: JoinHandle<std::io::Result<()>>,
        }

        async fn start_server() -> RunningServer {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let url = format!("ws://{}", listener.local_addr().expect("address"));
            let (stop, stopped) = oneshot::channel::<()>();
            let shutdown = async {
                let _ = stopped.await;
            };
            let task = tokio::spawn(serve(listener, ServerConfig::default(), shutdown));
            RunningServer { url, stop, task }
        }

        async fn connected(connection: &mut Connection) -> PlayerId {
            match next(connection).await {
                Some(NetworkEvent::Connected(id)) => id,
                other => panic!("expected Connected, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn creating_a_room_makes_the_player_its_host() {
            let server = start_server().await;
            let mut connection = Connection::open(server.url.clone(), username());
            let id = connected(&mut connection).await;
            let text = TextSource::Words {
                language: Language::English,
                count: 10,
                options: WordOptions::default(),
            };

            assert!(connection.send(ClientMessage::CreateRoom { text }));

            let room = match next(&mut connection).await {
                Some(NetworkEvent::Message(ServerMessage::Room(room))) => room,
                other => panic!("expected the room, got {other:?}"),
            };
            assert_eq!(room.host, id);
            assert_eq!(room.text, text);
            assert_eq!(room.phase, Phase::Lobby);
            let names: Vec<&str> = room
                .players
                .iter()
                .map(|player| player.name.as_str())
                .collect();
            assert_eq!(names, ["Tester"]);
        }

        #[tokio::test]
        async fn server_shutdown_closes_the_connection() {
            let server = start_server().await;
            let mut connection = Connection::open(server.url.clone(), username());
            connected(&mut connection).await;

            server.stop.send(()).expect("server running");

            assert_eq!(closed_reason(&mut connection).await, CONNECTION_LOST);
            assert_eq!(next(&mut connection).await, None);
            let stopped = timeout(EVENT_TIMEOUT, server.task)
                .await
                .expect("server stops");
            assert!(matches!(stopped, Ok(Ok(()))), "{stopped:?}");
        }
    }
}
