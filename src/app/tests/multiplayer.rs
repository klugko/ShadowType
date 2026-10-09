use std::future::Future;

use code_racer_protocol::Phase;
use code_racer_server::ServerConfig;
use tokio::{net::TcpListener, sync::oneshot, time::timeout};

use super::*;

/**
 * Longer than the client's connection and handshake timeouts, five seconds
 * each, so that a test fails on its assertions rather than on this limit.
 */
const WAIT: Duration = Duration::from_secs(15);

async fn server() -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    let config = ServerConfig {
        countdown: Duration::from_millis(300),
        ..ServerConfig::default()
    };
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(code_racer_server::serve(listener, config, async {
        let _ = stopped.await;
    }));
    (url, stop)
}

fn player(name: &str, url: &str) -> App {
    let mut config = configured(name);
    config.multiplayer.server = url.to_owned();
    let mut app = app_with(config, Launch::Home);
    app.config.race.word_count = 5;
    app
}

async fn pump_until(app: &mut App, done: impl Fn(&App) -> bool) {
    let waited = timeout(WAIT, async {
        while !done(app) {
            let Some(connection) = app.connection_mut() else {
                panic!("connection closed: {:?}", app.message());
            };
            let event = connection.next_event().await.expect("event");
            app.handle_network(event, Instant::now());
            app.tick(Instant::now());
        }
    })
    .await;
    assert!(waited.is_ok(), "timed out, message: {:?}", app.message());
}

async fn pump_until_closed(app: &mut App) {
    let waited = timeout(WAIT, async {
        while let Some(connection) = app.connection_mut() {
            let event = connection
                .next_event()
                .await
                .expect("an explicit close event");
            app.handle_network(event, Instant::now());
        }
    })
    .await;
    assert!(waited.is_ok());
}

fn phase(app: &App) -> Option<Phase> {
    app.race()?.room.as_ref().map(|room| room.phase)
}

fn players(app: &App) -> usize {
    app.race()
        .and_then(|client| client.room.as_ref())
        .map_or(0, |room| room.players.len())
}

/**
 * The server refuses progress faster than 30 characters per second plus
 * a burst of 5: how long typing `length` characters takes at least.
 */
fn believable_typing_time(length: usize) -> Duration {
    Duration::from_secs_f64(length.saturating_sub(5) as f64 / 30.0 + 0.05)
}

async fn both<F: Future<Output = ()>>(first: F, second: impl Future<Output = ()>) {
    tokio::join!(first, second);
}

#[tokio::test]
async fn two_players_race_through_a_real_server() {
    let (url, stop) = server().await;
    let mut alice = player("alice", &url);
    let mut bob = player("bob", &url);

    command(&mut alice, "create");
    pump_until(&mut alice, |app| phase(app) == Some(Phase::Lobby)).await;
    assert_eq!(alice.buffer, Buffer::Session);
    let code = alice.room_code.clone();

    command(&mut bob, &format!("join {code}"));
    pump_until(&mut bob, |app| players(app) == 2).await;
    pump_until(&mut alice, |app| players(app) == 2).await;

    press(&mut alice, KeyCode::Char('s'));
    assert!(
        alice.message().is_some_and(Message::is_error),
        "nobody is ready yet"
    );
    press(&mut alice, KeyCode::Char('r'));
    press(&mut bob, KeyCode::Char('r'));
    let everyone_ready = |app: &App| {
        app.race()
            .and_then(|client| client.room.as_ref())
            .is_some_and(|room| room.players.len() == 2 && room.everyone_ready())
    };
    pump_until(&mut alice, everyone_ready).await;
    press(&mut alice, KeyCode::Char('s'));

    both(
        pump_until(&mut alice, |app| phase(app) == Some(Phase::Racing)),
        pump_until(&mut bob, |app| phase(app) == Some(Phase::Racing)),
    )
    .await;
    assert!(alice.is_typing() && bob.is_typing());
    assert_eq!(
        session(&alice).target().concat(),
        session(&bob).target().concat(),
        "everyone types the same text"
    );

    let length = session(&alice).target().len();
    tokio::time::sleep(believable_typing_time(length)).await;
    type_remaining(&mut bob);
    pump_until(&mut bob, |app| {
        app.race()
            .and_then(RaceClient::me)
            .is_some_and(|me| me.progress.is_finished())
    })
    .await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    type_remaining(&mut alice);
    both(
        pump_until(&mut alice, |app| phase(app) == Some(Phase::Finished)),
        pump_until(&mut bob, |app| phase(app) == Some(Phase::Finished)),
    )
    .await;

    let room = alice
        .race()
        .and_then(|client| client.room.clone())
        .expect("room");
    assert!(
        room.players
            .iter()
            .all(|player| player.progress.is_finished())
    );
    assert_eq!(
        room.standings()[0].name.as_str(),
        "bob",
        "bob finished first, although alice joined first"
    );
    assert_eq!(alice.history.records().len(), 1);
    assert_eq!(bob.history.records()[0].mode, "race");

    let later = Instant::now() + AFTER_QUIET;
    press_at(&mut bob, KeyCode::Char('r'), later);
    assert_eq!(
        bob.message(),
        Some(&Message::info("waiting for the host to start another race")),
        "only the host restarts"
    );
    press_at(&mut alice, KeyCode::Char('r'), later);
    both(
        pump_until(&mut alice, |app| phase(app) == Some(Phase::Lobby)),
        pump_until(&mut bob, |app| phase(app) == Some(Phase::Lobby)),
    )
    .await;

    press_at(&mut bob, KeyCode::Esc, later);
    assert!(bob.activity.is_none());
    assert_eq!(bob.buffer, Buffer::Race);
    pump_until(&mut alice, |app| players(app) == 1).await;
    let _ = stop.send(());
}

#[tokio::test]
async fn joining_an_unknown_room_reports_the_error() {
    let (url, stop) = server().await;
    let mut app = player("carol", &url);
    command(&mut app, "join ABCDEF");
    pump_until_closed(&mut app).await;
    assert!(
        app.message()
            .is_some_and(|message| message.is_error() && message.text.contains("ABCDEF"))
    );
    assert_eq!(app.buffer, Buffer::Race);
    let _ = stop.send(());
}

#[tokio::test]
async fn a_secure_address_without_tls_is_reported_without_crashing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    tokio::spawn(async move {
        let (plain_socket, _) = listener.accept().await.expect("accept");
        drop(plain_socket);
    });
    let mut app = player("erin", &format!("wss://{address}"));
    command(&mut app, "create");
    pump_until_closed(&mut app).await;
    assert!(app.message().is_some_and(Message::is_error));
}

#[tokio::test]
async fn an_unreachable_server_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    drop(listener);
    let mut app = player("dave", &format!("ws://{address}"));
    command(&mut app, "create");
    pump_until_closed(&mut app).await;
    assert!(app.activity.is_none());
    assert!(app.message().is_some_and(Message::is_error));
}
