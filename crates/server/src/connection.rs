/*!
 * One task per WebSocket connection.
 *
 * A connection must open with [`ClientMessage::Hello`]. Afterwards every
 * client message is forwarded to the hub, while a writer task delivers the
 * player's outbox and pings the client so that dead peers are noticed.
 */

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use code_racer_protocol::{
    ClientMessage, ErrorCode, MAX_MESSAGE_BYTES, PROTOCOL_VERSION, PlayerId, ServerError,
    ServerMessage, Username,
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc,
    task::JoinHandle,
    time::{self, timeout},
};
use tokio_tungstenite::{
    WebSocketStream, accept_async_with_config,
    tungstenite::{Bytes, Message, Utf8Bytes, protocol::WebSocketConfig},
};
use tracing::debug;

use crate::hub::Command;

type Socket = WebSocketStream<TcpStream>;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(75);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const OUTBOX_CAPACITY: usize = 256;
const RATE_LIMIT: usize = 40;
const RATE_WINDOW: Duration = Duration::from_secs(1);

pub async fn serve_player(stream: TcpStream, id: PlayerId, hub: mpsc::Sender<Command>) {
    let Some(mut socket) = accept(stream).await else {
        return;
    };
    match greet(&mut socket).await {
        Ok(name) => play(socket, id, name, hub).await,
        Err(error) => refuse(socket, error).await,
    }
}

pub async fn turn_away(stream: TcpStream, error: ServerError) {
    if let Some(socket) = accept(stream).await {
        refuse(socket, error).await;
    }
}

async fn accept(stream: TcpStream) -> Option<Socket> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    match timeout(
        HANDSHAKE_TIMEOUT,
        accept_async_with_config(stream, Some(config)),
    )
    .await
    {
        Ok(Ok(socket)) => Some(socket),
        Ok(Err(error)) => {
            debug!(%error, "websocket handshake failed");
            None
        }
        Err(_) => {
            debug!("websocket handshake timed out");
            None
        }
    }
}

async fn greet<S>(socket: &mut WebSocketStream<S>) -> Result<Username, ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let first = timeout(HANDSHAKE_TIMEOUT, next_text(socket))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ServerError::new(ErrorCode::HandshakeRequired, "no hello received"))?;
    check_hello(&first)
}

/**
 * Accepts a `Hello` in the current protocol version.
 *
 * The version is looked up leniently first, so that clients of another
 * version learn why they are refused even when the rest of their first
 * message does not parse in this version.
 */
fn check_hello(text: &str) -> Result<Username, ServerError> {
    let current = u64::from(PROTOCOL_VERSION);
    if let Some(version) = announced_version(text).filter(|&version| version != current) {
        return Err(ServerError::new(
            ErrorCode::IncompatibleVersion,
            format!("server speaks protocol v{PROTOCOL_VERSION}, client v{version}"),
        ));
    }
    match ClientMessage::from_json(text) {
        Ok(ClientMessage::Hello { username, .. }) => Ok(username),
        Ok(_) => Err(ServerError::new(
            ErrorCode::HandshakeRequired,
            "say hello before anything else",
        )),
        Err(error) => Err(ServerError::new(
            ErrorCode::HandshakeRequired,
            format!("expected a hello message: {error}"),
        )),
    }
}

fn announced_version(text: &str) -> Option<u64> {
    let message: serde_json::Value = serde_json::from_str(text).ok()?;
    message.get("data")?.get("version")?.as_u64()
}

/**
 * The next text message, skipping control frames. `None` once the client
 * closes, errs or sends something that cannot be a hello.
 */
async fn next_text<S>(socket: &mut WebSocketStream<S>) -> Option<Utf8Bytes>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(frame) = socket.next().await {
        match frame.ok()? {
            Message::Text(text) => return Some(text),
            Message::Binary(_) | Message::Close(_) => return None,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    None
}

/**
 * Sends a last error and closes, waiting briefly for the client to
 * acknowledge so that the error is not lost to a connection reset.
 */
async fn refuse(mut socket: Socket, error: ServerError) {
    let farewell = async {
        socket.send(encode(&ServerMessage::Error(error))).await?;
        socket.close(None).await?;
        while let Some(Ok(_)) = socket.next().await {}
        Ok::<(), tokio_tungstenite::tungstenite::Error>(())
    };
    if let Ok(Err(error)) = timeout(CLOSE_TIMEOUT, farewell).await {
        debug!(%error, "could not refuse the client cleanly");
    }
}

async fn play(mut socket: Socket, id: PlayerId, name: Username, hub: mpsc::Sender<Command>) {
    let welcome = ServerMessage::Welcome {
        version: PROTOCOL_VERSION,
        player_id: id,
    };
    if socket.send(encode(&welcome)).await.is_err() {
        return;
    }
    let (outbox, inbox) = mpsc::channel(OUTBOX_CAPACITY);
    let replies = outbox.downgrade();
    if hub
        .send(Command::Connected { id, name, outbox })
        .await
        .is_err()
    {
        return;
    }
    let (sink, mut stream) = socket.split();
    let mut writer = tokio::spawn(write_messages(sink, inbox));
    tokio::select! {
        () = read_messages(&mut stream, id, &hub, &replies) => {}
        _ = &mut writer => {}
    }
    if hub.send(Command::Disconnected { id }).await.is_err() {
        debug!(player = %id, "the hub stopped before the player left");
    }
    say_goodbye(stream, writer).await;
}

async fn read_messages(
    stream: &mut SplitStream<Socket>,
    id: PlayerId,
    hub: &mpsc::Sender<Command>,
    replies: &mpsc::WeakSender<ServerMessage>,
) {
    let mut limiter = RateLimiter::default();
    while let Ok(Some(Ok(frame))) = timeout(IDLE_TIMEOUT, stream.next()).await {
        if !limiter.allow(Instant::now()) {
            reply(
                replies,
                ErrorCode::RateLimited,
                "too many messages, slow down",
            );
            return;
        }
        let message = match frame {
            Message::Text(text) => {
                ClientMessage::from_json(&text).map_err(|error| format!("invalid message: {error}"))
            }
            Message::Binary(_) => Err("binary messages are not supported".to_owned()),
            Message::Close(_) => return,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
        };
        match message {
            Ok(message) => {
                if hub.send(Command::Message { id, message }).await.is_err() {
                    return;
                }
            }
            Err(reason) => reply(replies, ErrorCode::InvalidMessage, reason),
        }
    }
}

/**
 * Queues an error for this client only. Holds no strong outbox sender, so
 * the connection still closes as soon as the hub lets go of the player.
 */
fn reply(replies: &mpsc::WeakSender<ServerMessage>, code: ErrorCode, message: impl Into<String>) {
    let error = ServerMessage::error(code, message);
    let queued = replies
        .upgrade()
        .is_some_and(|outbox| outbox.try_send(error).is_ok());
    if !queued {
        debug!("reply dropped, the outbox is closed or full");
    }
}

/**
 * Delivers the outbox and pings the client. Closes the connection once the
 * hub drops the outbox, or gives up when the client stops reading.
 */
async fn write_messages(
    mut sink: SplitSink<Socket, Message>,
    mut inbox: mpsc::Receiver<ServerMessage>,
) {
    let mut ping = time::interval_at(time::Instant::now() + PING_INTERVAL, PING_INTERVAL);
    loop {
        let frame = tokio::select! {
            message = inbox.recv() => match message {
                Some(message) => encode(&message),
                None => break,
            },
            _ = ping.tick() => Message::Ping(Bytes::new()),
        };
        if !matches!(timeout(WRITE_TIMEOUT, sink.send(frame)).await, Ok(Ok(()))) {
            return;
        }
    }
    if !matches!(timeout(WRITE_TIMEOUT, sink.close()).await, Ok(Ok(()))) {
        debug!("could not close the connection cleanly");
    }
}

/**
 * Keeps reading, and discarding, until the client acknowledges the close.
 * Dropping a socket with unread data would reset the connection and could
 * destroy the last messages before the client reads them.
 */
async fn say_goodbye(mut stream: SplitStream<Socket>, writer: JoinHandle<()>) {
    let drain = async { while let Some(Ok(_)) = stream.next().await {} };
    if timeout(CLOSE_TIMEOUT, drain).await.is_err() {
        debug!("the client did not acknowledge the close");
    }
    writer.abort();
}

fn encode(message: &ServerMessage) -> Message {
    Message::text(message.to_json())
}

#[derive(Debug, Default)]
struct RateLimiter {
    arrivals: VecDeque<Instant>,
}

impl RateLimiter {
    fn allow(&mut self, now: Instant) -> bool {
        while self
            .arrivals
            .front()
            .is_some_and(|&arrival| now.saturating_duration_since(arrival) >= RATE_WINDOW)
        {
            self.arrivals.pop_front();
        }
        self.arrivals.push_back(now);
        self.arrivals.len() <= RATE_LIMIT
    }
}

#[cfg(test)]
mod tests {
    use code_racer_engine::{CodeLanguage, TextSource};
    use tokio::io::{DuplexStream, duplex};
    use tokio_tungstenite::tungstenite::protocol::Role;

    use super::*;

    async fn socket_pair() -> (WebSocketStream<DuplexStream>, WebSocketStream<DuplexStream>) {
        let (server, client) = duplex(MAX_MESSAGE_BYTES);
        (
            WebSocketStream::from_raw_socket(server, Role::Server, None).await,
            WebSocketStream::from_raw_socket(client, Role::Client, None).await,
        )
    }

    fn hello(version: u16) -> String {
        ClientMessage::Hello {
            version,
            username: "Ada".parse().expect("valid name"),
        }
        .to_json()
    }

    fn refusal(text: &str) -> Option<(ErrorCode, String)> {
        check_hello(text)
            .err()
            .map(|error| (error.code, error.message))
    }

    #[test]
    fn a_hello_in_the_current_version_gives_the_username() {
        assert_eq!(
            check_hello(&hello(PROTOCOL_VERSION)).map(String::from),
            Ok("Ada".to_owned())
        );
    }

    #[test]
    fn other_versions_are_named_even_when_their_hello_does_not_parse() {
        let newer = PROTOCOL_VERSION + 1;
        let reshaped = format!(r#"{{"type":"hello","data":{{"version":{newer},"user":{{}}}}}}"#);
        let expected = format!("server speaks protocol v{PROTOCOL_VERSION}, client v{newer}");
        assert_eq!(
            refusal(&reshaped),
            Some((ErrorCode::IncompatibleVersion, expected))
        );
        let first_release = r#"{"type":"Create","data":{"version":1,"username":"Ada"}}"#;
        assert_eq!(
            refusal(first_release).map(|(code, _)| code),
            Some(ErrorCode::IncompatibleVersion)
        );
    }

    #[test]
    fn anything_but_a_valid_hello_requires_the_handshake() {
        let create = ClientMessage::CreateRoom {
            text: TextSource::Code {
                language: CodeLanguage::Rust,
            },
        }
        .to_json();
        let nameless = hello(PROTOCOL_VERSION).replace("Ada", "");
        for text in [
            create.as_str(),
            &nameless,
            "not json",
            r#"{"data":{"version":"2"}}"#,
        ] {
            assert_eq!(
                refusal(text).map(|(code, _)| code),
                Some(ErrorCode::HandshakeRequired),
                "{text}"
            );
        }
    }

    #[tokio::test]
    async fn the_hello_may_follow_control_frames() {
        let (mut server, mut client) = socket_pair().await;
        client
            .send(Message::Ping(Bytes::new()))
            .await
            .expect("ping");
        client
            .send(Message::text(hello(PROTOCOL_VERSION)))
            .await
            .expect("hello");
        assert_eq!(
            greet(&mut server).await.map(String::from),
            Ok("Ada".to_owned())
        );
    }

    #[tokio::test]
    async fn a_binary_first_message_is_not_a_hello() {
        let (mut server, mut client) = socket_pair().await;
        client
            .send(Message::binary(vec![1, 2, 3]))
            .await
            .expect("binary");
        assert_eq!(
            greet(&mut server).await.map_err(|error| error.code),
            Err(ErrorCode::HandshakeRequired)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_silent_client_is_refused_once_the_handshake_times_out() {
        let (mut server, _client) = socket_pair().await;
        let started = time::Instant::now();
        let refused = greet(&mut server).await;
        assert_eq!(
            refused.map_err(|error| error.code),
            Err(ErrorCode::HandshakeRequired)
        );
        assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
    }

    fn burst(limiter: &mut RateLimiter, count: usize, at: Instant) -> Vec<bool> {
        (0..count).map(|_| limiter.allow(at)).collect()
    }

    #[test]
    fn allows_forty_messages_within_a_second_but_not_forty_one() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        assert!(
            burst(&mut limiter, RATE_LIMIT, start)
                .iter()
                .all(|&allowed| allowed)
        );
        assert!(!limiter.allow(start + Duration::from_millis(999)));
    }

    #[test]
    fn the_window_slides_with_time() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        for index in 0..200u64 {
            assert!(
                limiter.allow(start + Duration::from_millis(index * 25)),
                "message {index} arrives at 40 per second"
            );
        }
    }

    #[test]
    fn old_messages_stop_counting_after_a_second() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        burst(&mut limiter, RATE_LIMIT, start);
        let later = start + RATE_WINDOW;
        assert!(
            burst(&mut limiter, RATE_LIMIT, later)
                .iter()
                .all(|&allowed| allowed)
        );
        assert!(!limiter.allow(later));
    }
}
