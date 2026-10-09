use code_racer_protocol::{ClientMessage, PlayerId, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::{sync::oneshot, time::timeout};
use tokio_tungstenite::tungstenite::Message;

use super::{
    EVENT_TIMEOUT, SHORT_TIMEOUT, closed_reason, next,
    scripted_server::{accept_hello, receive, scripted_server, send},
    username,
};
use crate::network::{Connection, NetworkEvent, Timeouts};

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
