use code_racer_engine::{Language, TextSource, WordOptions};
use code_racer_protocol::{ClientMessage, Phase, PlayerId, ServerMessage};
use code_racer_server::{ServerConfig, serve};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle, time::timeout};

use super::{EVENT_TIMEOUT, closed_reason, next, username};
use crate::network::{CONNECTION_LOST, Connection, NetworkEvent};

struct RunningServer {
    url: String,
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

async fn start_server() -> RunningServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    let (stop, stopped) = oneshot::channel::<()>();
    let shutdown = async {
        let _ = stopped.await;
    };
    let task = tokio::spawn(serve(listener, ServerConfig::default(), shutdown));
    RunningServer { url, stop, task }
}

async fn connected(connection: &mut Connection) -> PlayerId {
    match next(connection).await {
        Some(NetworkEvent::Connected(id)) => id,
        other => panic!("expected Connected, got {other:?}"),
    }
}

#[tokio::test]
async fn creating_a_room_makes_the_player_its_host() {
    let server = start_server().await;
    let mut connection = Connection::open(server.url.clone(), username());
    let id = connected(&mut connection).await;
    let text = TextSource::Words {
        language: Language::English,
        count: 10,
        options: WordOptions::default(),
    };

    assert!(connection.send(ClientMessage::CreateRoom { text }));

    let room = match next(&mut connection).await {
        Some(NetworkEvent::Message(ServerMessage::Room(room))) => room,
        other => panic!("expected the room, got {other:?}"),
    };
    assert_eq!(room.host, id);
    assert_eq!(room.text, text);
    assert_eq!(room.phase, Phase::Lobby);
    let names: Vec<&str> = room
        .players
        .iter()
        .map(|player| player.name.as_str())
        .collect();
    assert_eq!(names, ["Tester"]);
}

#[tokio::test]
async fn server_shutdown_closes_the_connection() {
    let server = start_server().await;
    let mut connection = Connection::open(server.url.clone(), username());
    connected(&mut connection).await;

    server.stop.send(()).expect("server running");

    assert_eq!(closed_reason(&mut connection).await, CONNECTION_LOST);
    assert_eq!(next(&mut connection).await, None);
    let stopped = timeout(EVENT_TIMEOUT, server.task)
        .await
        .expect("server stops");
    assert!(matches!(stopped, Ok(Ok(()))), "{stopped:?}");
}
