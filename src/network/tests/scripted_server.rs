use code_racer_protocol::{ClientMessage, PROTOCOL_VERSION, PlayerId, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

type ServerSocket = WebSocketStream<TcpStream>;

/// Accepts one WebSocket client and hands it to `script`.
pub(super) async fn scripted_server<F, Fut>(script: F) -> String
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

pub(super) async fn receive(socket: &mut ServerSocket) -> ClientMessage {
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

pub(super) async fn send(socket: &mut ServerSocket, message: ServerMessage) {
    socket
        .send(Message::text(message.to_json()))
        .await
        .expect("send to client");
}

pub(super) async fn accept_hello(socket: &mut ServerSocket, id: u64) {
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
