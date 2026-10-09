use std::time::{Duration, Instant};

use code_racer_protocol::{
    ClientMessage, ErrorCode, PROTOCOL_VERSION, PlayerId, ServerMessage, Username,
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{self, timeout},
};
use tokio_tungstenite::tungstenite::{Bytes, Message};
use tracing::debug;

use super::{CLOSE_TIMEOUT, Socket, encode, rate_limit::RateLimiter};
use crate::hub::Command;

const PING_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(75);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const OUTBOX_CAPACITY: usize = 256;

pub(super) async fn play(
    mut socket: Socket,
    id: PlayerId,
    name: Username,
    hub: mpsc::Sender<Command>,
) {
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
