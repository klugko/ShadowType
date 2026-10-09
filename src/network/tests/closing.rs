use std::time::Duration;

use code_racer_protocol::{ClientMessage, ErrorCode, PlayerId, ServerMessage};
use futures_util::StreamExt;
use tokio::{
    sync::oneshot,
    time::{self, timeout},
};
use tokio_tungstenite::tungstenite::Message;

use super::{
    EVENT_TIMEOUT, SHORT_SILENCE, SHORT_TIMEOUT, closed_reason, next,
    scripted_server::{accept_hello, receive, scripted_server, send},
    username,
};
use crate::network::{CONNECTION_LOST, Connection, NetworkEvent, SERVER_SILENT, Timeouts};

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
async fn closing_sends_what_was_queued_and_waits_no_longer_than_asked() {
    let (report, received) = oneshot::channel();
    let (_keep_alive, kept) = oneshot::channel::<()>();
    let url = scripted_server(|mut socket| async move {
        accept_hello(&mut socket, 6).await;
        let _ = report.send(receive(&mut socket).await);
        let _ = kept.await;
    })
    .await;
    let mut connection = Connection::open(url, username());
    assert_eq!(
        next(&mut connection).await,
        Some(NetworkEvent::Connected(PlayerId(6)))
    );
    assert!(connection.send(ClientMessage::LeaveRoom));

    let started = std::time::Instant::now();
    connection.close(SHORT_TIMEOUT).await;

    assert!(
        started.elapsed() < EVENT_TIMEOUT / 2,
        "{:?}",
        started.elapsed()
    );
    let queued = timeout(EVENT_TIMEOUT, received).await.expect("in time");
    assert_eq!(queued.ok(), Some(ClientMessage::LeaveRoom));
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
