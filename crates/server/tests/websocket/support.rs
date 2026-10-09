use std::{io, time::Duration};

use code_racer_engine::{Language, TextSource, WordOptions, grapheme_count};
use code_racer_protocol::{
    ClientMessage, PROTOCOL_VERSION, Phase, PlayerId, Progress, RoomView, ServerError,
    ServerMessage,
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

pub(crate) const PATIENCE: Duration = Duration::from_secs(5);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(crate) struct Server {
    pub(crate) url: String,
    pub(crate) stop: oneshot::Sender<()>,
    pub(crate) task: JoinHandle<io::Result<()>>,
}

pub(crate) async fn start(config: ServerConfig) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("local address"));
    let (stop, stopped) = oneshot::channel::<()>();
    let shutdown = async {
        stopped.await.ok();
    };
    let task = tokio::spawn(serve(listener, config, shutdown));
    Server { url, stop, task }
}

pub(crate) fn quick_races() -> ServerConfig {
    ServerConfig {
        countdown: Duration::from_millis(200),
        ..ServerConfig::default()
    }
}

pub(crate) struct Client {
    pub(crate) socket: Socket,
    pub(crate) id: PlayerId,
}

impl Client {
    pub(crate) async fn connect(url: &str, name: &str) -> Self {
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

    pub(crate) async fn send(&mut self, message: ClientMessage) {
        send(&mut self.socket, &message).await;
    }

    pub(crate) async fn receive(&mut self) -> ServerMessage {
        receive(&mut self.socket).await
    }

    pub(crate) async fn room_where(&mut self, condition: impl Fn(&RoomView) -> bool) -> RoomView {
        loop {
            if let ServerMessage::Room(view) = self.receive().await
                && condition(&view)
            {
                return view;
            }
        }
    }

    pub(crate) async fn error(&mut self) -> ServerError {
        loop {
            if let ServerMessage::Error(error) = self.receive().await {
                return error;
            }
        }
    }
}

pub(crate) async fn open(url: &str) -> Socket {
    connect_async(url).await.expect("connect").0
}

pub(crate) async fn send(socket: &mut Socket, message: &ClientMessage) {
    socket
        .send(Message::text(message.to_json()))
        .await
        .expect("send");
}

pub(crate) async fn receive(socket: &mut Socket) -> ServerMessage {
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
pub(crate) async fn assert_closed(socket: &mut Socket) {
    let closed = async { while let Some(Ok(_)) = socket.next().await {} };
    timeout(PATIENCE, closed)
        .await
        .expect("the server closes the connection");
}

pub(crate) fn hello(version: u16, name: &str) -> ClientMessage {
    ClientMessage::Hello {
        version,
        username: name.parse().expect("valid name"),
    }
}

pub(crate) fn five_words() -> TextSource {
    TextSource::Words {
        language: Language::English,
        count: 5,
        options: WordOptions::default(),
    }
}

pub(crate) fn typed(length: u32, keystrokes: u32, errors: u32) -> ClientMessage {
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
pub(crate) async fn wait_until_plausible(length: u32) {
    let seconds = (f64::from(length) - 5.0) / 30.0;
    if seconds > 0.0 {
        tokio::time::sleep(Duration::from_secs_f64(seconds + 0.05)).await;
    }
}

/// Alice hosts a five word room that Bob joins; both are ready.
pub(crate) async fn ready_pair(url: &str) -> (Client, Client) {
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
pub(crate) async fn start_race(alice: &mut Client, bob: &mut Client) -> u32 {
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
