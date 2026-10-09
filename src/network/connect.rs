use code_racer_protocol::{
    ClientMessage, MAX_MESSAGE_BYTES, PROTOCOL_VERSION, PlayerId, ServerMessage, Username,
};
use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{self, Message, protocol::WebSocketConfig},
};

use super::{
    CONNECTION_LOST, Socket, Timeouts,
    relay::{Incoming, decode},
};

pub(super) async fn connect(
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

pub(super) fn describe(error: &tungstenite::Error) -> String {
    match error {
        tungstenite::Error::Io(io) => describe_io(io),
        tungstenite::Error::Http(response) => {
            format!("not a race server (HTTP {})", response.status())
        }
        other => other.to_string(),
    }
}

/**
 * The short, platform-independent name of common network failures, and
 * otherwise the system's own message. The kind alone is useless for codes
 * that std does not map, such as an unknown host on Windows, which would
 * read "uncategorized error".
 */
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
