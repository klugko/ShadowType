mod address;
mod closing;
mod connecting;
mod messages;
mod race_server;
mod scripted_server;

use std::time::Duration;

use code_racer_protocol::Username;
use tokio::time::timeout;

use super::{Connection, NetworkEvent};

const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
/// Replaces a production timeout that a test waits for on purpose.
const SHORT_TIMEOUT: Duration = Duration::from_millis(100);
/**
 * Replaces the silence timeout where a test needs the client to ping:
 * long enough that a busy machine still answers in time.
 */
const SHORT_SILENCE: Duration = Duration::from_millis(400);

fn username() -> Username {
    "Tester".parse().expect("valid name")
}

async fn next(connection: &mut Connection) -> Option<NetworkEvent> {
    timeout(EVENT_TIMEOUT, connection.next_event())
        .await
        .expect("an event before the timeout")
}

async fn closed_reason(connection: &mut Connection) -> String {
    match next(connection).await {
        Some(NetworkEvent::Closed { reason }) => reason,
        other => panic!("expected Closed, got {other:?}"),
    }
}
