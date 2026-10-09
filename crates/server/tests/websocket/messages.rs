use code_racer_protocol::{ClientMessage, ErrorCode, ServerMessage};
use code_racer_server::ServerConfig;
use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;

use crate::support::{Client, assert_closed, five_words, hello, open, receive, send, start};

#[tokio::test]
async fn an_incompatible_client_is_told_the_versions_and_disconnected() {
    let server = start(ServerConfig::default()).await;
    let mut socket = open(&server.url).await;
    send(&mut socket, &hello(1, "Old")).await;
    let ServerMessage::Error(error) = receive(&mut socket).await else {
        panic!("expected an error");
    };
    assert_eq!(error.code, ErrorCode::IncompatibleVersion);
    assert_eq!(error.message, "server speaks protocol v3, client v1");
    assert_closed(&mut socket).await;
}

#[tokio::test]
async fn the_first_message_must_be_hello() {
    let server = start(ServerConfig::default()).await;
    let mut socket = open(&server.url).await;
    send(
        &mut socket,
        &ClientMessage::CreateRoom { text: five_words() },
    )
    .await;
    let ServerMessage::Error(error) = receive(&mut socket).await else {
        panic!("expected an error");
    };
    assert_eq!(error.code, ErrorCode::HandshakeRequired);
    assert_closed(&mut socket).await;
}

#[tokio::test]
async fn malformed_messages_are_reported_and_the_connection_stays_open() {
    let server = start(ServerConfig::default()).await;
    let mut alice = Client::connect(&server.url, "Alice").await;
    alice
        .socket
        .send(Message::text("not json"))
        .await
        .expect("send");
    assert_eq!(alice.error().await.code, ErrorCode::InvalidMessage);
    alice
        .socket
        .send(Message::binary(vec![1, 2, 3]))
        .await
        .expect("send");
    assert_eq!(alice.error().await.code, ErrorCode::InvalidMessage);

    alice
        .send(ClientMessage::CreateRoom { text: five_words() })
        .await;
    let view = alice.room_where(|_| true).await;
    assert_eq!(view.host, alice.id);
}

#[tokio::test]
async fn flooding_clients_are_rate_limited_and_disconnected() {
    let server = start(ServerConfig::default()).await;
    let mut client = Client::connect(&server.url, "Flood").await;
    let flood = Message::text(ClientMessage::SetReady { ready: true }.to_json());
    for _ in 0..60 {
        client.socket.feed(flood.clone()).await.expect("queue");
    }
    client.socket.flush().await.expect("flush");

    let error = loop {
        let error = client.error().await;
        if error.code != ErrorCode::NotInRoom {
            break error;
        }
    };
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_closed(&mut client.socket).await;
}
