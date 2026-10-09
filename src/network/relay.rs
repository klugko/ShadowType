use std::{ops::ControlFlow, time::Duration};

use code_racer_protocol::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    sync::mpsc,
    time::{self, Instant, MissedTickBehavior, timeout},
};
use tokio_tungstenite::tungstenite::{self, Bytes, Message};

use super::{CLOSED_BY_CLIENT, CONNECTION_LOST, NetworkEvent, SERVER_SILENT, Socket, Timeouts};

/**
 * Forwards messages both ways until the server goes away, falls silent or
 * the [`Connection`](super::Connection) is dropped, and returns why it
 * stopped.
 *
 * Outgoing messages come first, so that the ones queued before the drop
 * are all sent before the socket is closed. Only frames from the server
 * prove that it is alive: sending succeeds long after it vanished, as the
 * operating system buffers the data and retries for minutes.
 */
pub(super) async fn relay(
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

/**
 * Sends the next queued message, or closes the socket once the
 * [`Connection`](super::Connection) is gone and the queue is empty.
 */
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

/**
 * Hands a server message to the application. When the application stopped
 * listening, the queue of outgoing messages is closed too, which ends the
 * relay once it is flushed.
 */
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

/**
 * Sends a close frame and reads until the server answers it, for at most
 * `limit`. Dropping a socket with unread data would reset the connection,
 * which can destroy the last messages before the server reads them.
 */
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

pub(super) enum Incoming {
    Message(ServerMessage),
    Ignored,
    Closed,
}

pub(super) fn decode(frame: Option<Result<Message, tungstenite::Error>>) -> Incoming {
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
