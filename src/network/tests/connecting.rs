use code_racer_protocol::ClientMessage;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite;

use super::{SHORT_TIMEOUT, closed_reason, next, username};
use crate::network::{Connection, NetworkEvent, OUTGOING_CAPACITY, Timeouts, connect::describe};

#[test]
fn socket_errors_are_described_without_os_codes() {
    let describe_io = |error: std::io::Error| describe(&tungstenite::Error::Io(error));
    let unknown_host = describe_io(std::io::Error::from_raw_os_error(11001));

    assert!(!unknown_host.contains("uncategorized"), "{unknown_host}");
    assert!(!unknown_host.contains("os error"), "{unknown_host}");
    assert!(!unknown_host.is_empty());
    assert_eq!(
        describe_io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "No connection could be made (os error 10061)"
        )),
        "connection refused"
    );
    assert_eq!(
        describe_io(std::io::Error::other(
            "failed to lookup address information"
        )),
        "failed to lookup address information"
    );
}

#[tokio::test]
async fn unreachable_server_reports_a_precise_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    drop(listener);
    let url = format!("ws://{address}");

    let mut connection = Connection::open(url.clone(), username());

    assert_eq!(
        closed_reason(&mut connection).await,
        format!("cannot reach {url}: connection refused")
    );
    assert_eq!(next(&mut connection).await, None);
    assert!(!connection.send(ClientMessage::LeaveRoom));
}

#[tokio::test]
async fn server_that_never_upgrades_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));

    let timeouts = Timeouts {
        connect: SHORT_TIMEOUT,
        ..Timeouts::default()
    };
    let mut connection = Connection::open_with(url.clone(), username(), timeouts);

    assert_eq!(
        closed_reason(&mut connection).await,
        format!("cannot reach {url}: timed out")
    );
    drop(listener);
}

#[tokio::test]
async fn sending_while_connecting_refuses_rather_than_blocks() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    let connection = Connection::open(url, username());

    let accepted = (0..OUTGOING_CAPACITY + 10)
        .filter(|_| connection.send(ClientMessage::LeaveRoom))
        .count();

    assert_eq!(accepted, OUTGOING_CAPACITY);
    drop(listener);
}

#[test]
fn opening_outside_a_runtime_is_closed_immediately() {
    let mut connection = Connection::open("ws://127.0.0.1:1".to_owned(), username());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let event = runtime.block_on(connection.next_event());
    assert!(
        matches!(event, Some(NetworkEvent::Closed { .. })),
        "{event:?}"
    );
    assert!(!connection.send(ClientMessage::LeaveRoom));
}
