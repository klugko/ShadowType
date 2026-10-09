//! End-to-end tests against a real server listening on a loopback port.

use std::{io, num::NonZeroUsize, time::Duration};

use code_racer_engine::{Language, TextSource, WordOptions, grapheme_count};
use code_racer_protocol::{
    ClientMessage, ErrorCode, PROTOCOL_VERSION, Phase, PlayerId, PlayerProgress, Progress,
    RoomView, ServerError, ServerMessage,
};
use code_racer_server::{ServerConfig, serve};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

const PATIENCE: Duration = Duration::from_secs(5);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Server {
    url: String,
    stop: oneshot::Sender<()>,
    task: JoinHandle<io::Result<()>>,
}

async fn start(config: ServerConfig) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("local address"));
    let (stop, stopped) = oneshot::channel::<()>();
    let shutdown = async {
        stopped.await.ok();
    };
    let task = tokio::spawn(serve(listener, config, shutdown));
    Server { url, stop, task }
}

fn quick_races() -> ServerConfig {
    ServerConfig {
        countdown: Duration::from_millis(200),
        ..ServerConfig::default()
    }
}

struct Client {
    socket: Socket,
    id: PlayerId,
}

impl Client {
    async fn connect(url: &str, name: &str) -> Self {
        let mut socket = open(url).await;
        send(&mut socket, &hello(PROTOCOL_VERSION, name)).await;
        match receive(&mut socket).await {
            ServerMessage::Welcome { version, player_id } => {
                assert_eq!(version, PROTOCOL_VERSION);
                Self {
                    socket,
                    id: player_id,
                }
            }
            other => panic!("expected a welcome, got {other:?}"),
        }
    }

    async fn send(&mut self, message: ClientMessage) {
        send(&mut self.socket, &message).await;
    }

    async fn receive(&mut self) -> ServerMessage {
        receive(&mut self.socket).await
    }

    async fn room_where(&mut self, condition: impl Fn(&RoomView) -> bool) -> RoomView {
        loop {
            if let ServerMessage::Room(view) = self.receive().await
                && condition(&view)
            {
                return view;
            }
        }
    }

    async fn error(&mut self) -> ServerError {
        loop {
            if let ServerMessage::Error(error) = self.receive().await {
                return error;
            }
        }
    }
}

async fn open(url: &str) -> Socket {
    connect_async(url).await.expect("connect").0
}

async fn send(socket: &mut Socket, message: &ClientMessage) {
    socket
        .send(Message::text(message.to_json()))
        .await
        .expect("send");
}

async fn receive(socket: &mut Socket) -> ServerMessage {
    let next_text = async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    return ServerMessage::from_json(&text).expect("server message");
                }
                Some(Ok(Message::Close(_))) | None => panic!("the server closed the connection"),
                Some(Err(error)) => panic!("connection failed: {error}"),
                Some(Ok(_)) => {}
            }
        }
    };
    timeout(PATIENCE, next_text)
        .await
        .expect("a message in time")
}

/// Reads until the server ends the connection, acknowledging its close like a real client.
async fn assert_closed(socket: &mut Socket) {
    let closed = async { while let Some(Ok(_)) = socket.next().await {} };
    timeout(PATIENCE, closed)
        .await
        .expect("the server closes the connection");
}

fn hello(version: u16, name: &str) -> ClientMessage {
    ClientMessage::Hello {
        version,
        username: name.parse().expect("valid name"),
    }
}

fn five_words() -> TextSource {
    TextSource::Words {
        language: Language::English,
        count: 5,
        options: WordOptions::default(),
    }
}

fn typed(length: u32, keystrokes: u32, errors: u32) -> ClientMessage {
    ClientMessage::Progress(Progress {
        typed: length,
        correct: length,
        indentation: 0,
        keystrokes,
        errors,
    })
}

/**
 * The server refuses progress faster than 30 characters per second plus a
 * burst of 5; waits until typing `length` characters is believable.
 */
async fn wait_until_plausible(length: u32) {
    let seconds = (f64::from(length) - 5.0) / 30.0;
    if seconds > 0.0 {
        tokio::time::sleep(Duration::from_secs_f64(seconds + 0.05)).await;
    }
}

/// Alice hosts a five word room that Bob joins; both are ready.
async fn ready_pair(url: &str) -> (Client, Client) {
    let mut alice = Client::connect(url, "Alice").await;
    let mut bob = Client::connect(url, "Bob").await;
    alice
        .send(ClientMessage::CreateRoom { text: five_words() })
        .await;
    let code = alice.room_where(|_| true).await.code;
    bob.send(ClientMessage::JoinRoom { code }).await;
    for client in [&mut alice, &mut bob] {
        client.send(ClientMessage::SetReady { ready: true }).await;
    }
    for client in [&mut alice, &mut bob] {
        client
            .room_where(|view| view.players.len() == 2 && view.everyone_ready())
            .await;
    }
    (alice, bob)
}

/// Starts the race of a ready pair and returns the text length once it runs.
async fn start_race(alice: &mut Client, bob: &mut Client) -> u32 {
    alice.send(ClientMessage::StartRace).await;
    let mut lengths = Vec::new();
    for client in [alice, bob] {
        let ServerMessage::Countdown { text, duration_ms } = client.receive().await else {
            panic!("the race text comes first");
        };
        assert_eq!(duration_ms, 200);
        let ServerMessage::Room(view) = client.receive().await else {
            panic!("the countdown view follows the text");
        };
        assert_eq!(view.phase, Phase::Countdown);
        assert_eq!(view.text_length as usize, grapheme_count(&text));
        let racing = client.room_where(|view| view.phase == Phase::Racing).await;
        lengths.push(racing.text_length);
    }
    assert_eq!(lengths[0], lengths[1]);
    lengths[0]
}

#[tokio::test]
async fn two_players_race_get_ranked_and_return_to_the_lobby() {
    let server = start(quick_races()).await;
    let (mut alice, mut bob) = ready_pair(&server.url).await;
    let length = start_race(&mut alice, &mut bob).await;
    wait_until_plausible(length).await;

    bob.send(typed(length, length, 0)).await;
    let bob_id = bob.id;
    alice
        .room_where(|view| {
            view.player(bob_id)
                .is_some_and(|bob| bob.progress.is_finished())
        })
        .await;
    alice.send(typed(length, length + 2, 2)).await;

    for client in [&mut alice, &mut bob] {
        let results = client
            .room_where(|view| view.phase == Phase::Finished)
            .await;
        assert_eq!(results.place_of(bob_id), Some(1));
        assert_eq!(results.place_of(results.host), Some(2));
        let winner = results.player(bob_id).expect("bob").progress;
        let runner_up = results.player(results.host).expect("alice").progress;
        assert_eq!(winner.accuracy, 100.0);
        assert!(runner_up.accuracy < 100.0);
        assert!(winner.finish_ms <= runner_up.finish_ms);
    }

    bob.send(ClientMessage::ReturnToLobby).await;
    assert_eq!(bob.error().await.code, ErrorCode::NotHost);
    alice.send(ClientMessage::ReturnToLobby).await;
    for client in [&mut alice, &mut bob] {
        let lobby = client.room_where(|view| view.phase == Phase::Lobby).await;
        assert_eq!(lobby.players.len(), 2);
        assert!(lobby.players.iter().all(|player| !player.ready));
        assert!(
            lobby
                .players
                .iter()
                .all(|player| player.progress == PlayerProgress::default())
        );
    }
}

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
async fn only_the_host_can_start_a_race() {
    let server = start(quick_races()).await;
    let (mut alice, mut bob) = ready_pair(&server.url).await;
    bob.send(ClientMessage::StartRace).await;
    let error = bob.error().await;
    assert_eq!(error.code, ErrorCode::NotHost);

    alice.send(ClientMessage::SetReady { ready: false }).await;
    match alice.receive().await {
        ServerMessage::Room(view) => assert_eq!(view.phase, Phase::Lobby),
        other => panic!("expected the lobby to be unchanged, got {other:?}"),
    }
}

#[tokio::test]
async fn a_host_leaving_mid_race_is_shown_offline_and_the_race_still_finishes() {
    let server = start(quick_races()).await;
    let (mut alice, mut bob) = ready_pair(&server.url).await;
    let length = start_race(&mut alice, &mut bob).await;
    let alice_id = alice.id;
    drop(alice);

    let view = bob
        .room_where(|view| view.player(alice_id).is_some_and(|alice| !alice.connected))
        .await;
    assert_eq!(view.host, bob.id);
    assert_eq!(view.phase, Phase::Racing);

    wait_until_plausible(length).await;
    bob.send(typed(length, length, 0)).await;
    let results = bob.room_where(|view| view.phase == Phase::Finished).await;
    assert_eq!(results.place_of(bob.id), Some(1));

    bob.send(ClientMessage::ReturnToLobby).await;
    let lobby = bob.room_where(|view| view.phase == Phase::Lobby).await;
    let ids: Vec<PlayerId> = lobby.players.iter().map(|player| player.id).collect();
    assert_eq!(ids, [bob.id]);
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
