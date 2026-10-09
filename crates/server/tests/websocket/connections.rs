use std::{num::NonZeroUsize, time::Duration};

use code_racer_protocol::{ErrorCode, PROTOCOL_VERSION, ServerMessage};
use code_racer_server::ServerConfig;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;

use crate::support::{Client, PATIENCE, assert_closed, hello, open, receive, send, start};

#[tokio::test]
async fn connections_beyond_the_limit_are_refused() {
    let config = ServerConfig {
        max_connections: 1,
        ..ServerConfig::default()
    };
    let server = start(config).await;
    let _alice = Client::connect(&server.url, "Alice").await;

    let mut socket = open(&server.url).await;
    send(&mut socket, &hello(PROTOCOL_VERSION, "Bob")).await;
    let ServerMessage::Error(error) = receive(&mut socket).await else {
        panic!("expected an error");
    };
    assert_eq!(error.code, ErrorCode::ServerFull);
    assert_closed(&mut socket).await;
}

#[tokio::test]
async fn an_address_holds_no_more_than_its_share_of_connections() {
    let config = ServerConfig {
        max_connections_per_address: NonZeroUsize::new(1),
        ..ServerConfig::default()
    };
    let server = start(config).await;
    let alice = Client::connect(&server.url, "Alice").await;
    assert!(
        connect_async(&server.url).await.is_err(),
        "a second connection from the same address is dropped"
    );

    drop(alice);
    let reconnected = timeout(PATIENCE, async {
        loop {
            if connect_async(&server.url).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(
        reconnected.is_ok(),
        "the slot is given back once Alice left"
    );
}

#[tokio::test]
async fn stopping_the_server_closes_every_connection() {
    let server = start(ServerConfig::default()).await;
    let mut alice = Client::connect(&server.url, "Alice").await;
    server.stop.send(()).expect("server running");
    assert_closed(&mut alice.socket).await;
    let stopped = timeout(PATIENCE, server.task)
        .await
        .expect("the server stops in time");
    assert!(matches!(stopped, Ok(Ok(()))));
}
