/*!
 * One task per WebSocket connection.
 *
 * A connection must open with [`ClientMessage::Hello`]. Afterwards every
 * client message is forwarded to the hub, while a writer task delivers the
 * player's outbox and pings the client so that dead peers are noticed.
 *
 * [`ClientMessage::Hello`]: code_racer_protocol::ClientMessage::Hello
 */

mod handshake;
mod rate_limit;
mod relay;

use std::time::Duration;

use code_racer_protocol::{MAX_MESSAGE_BYTES, PlayerId, ServerError, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc, time::timeout};
use tokio_tungstenite::{
    WebSocketStream, accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use tracing::debug;

use crate::hub::Command;

type Socket = WebSocketStream<TcpStream>;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn serve_player(stream: TcpStream, id: PlayerId, hub: mpsc::Sender<Command>) {
    let Some(mut socket) = accept(stream).await else {
        return;
    };
    match handshake::greet(&mut socket).await {
        Ok(name) => relay::play(socket, id, name, hub).await,
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

fn encode(message: &ServerMessage) -> Message {
    Message::text(message.to_json())
}
