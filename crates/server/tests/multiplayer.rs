use code_racer_protocol::*;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    time::{Duration, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
async fn send(ws: &mut Socket, msg: ClientMessage) {
    ws.send(Message::Text(
        serde_json::to_string(&msg).expect("encode").into(),
    ))
    .await
    .expect("send");
}
async fn recv(ws: &mut Socket) -> ServerMessage {
    timeout(Duration::from_secs(6), async {
        loop {
            if let Some(Ok(Message::Text(json))) = ws.next().await {
                return serde_json::from_str(&json).expect("decode");
            }
        }
    })
    .await
    .expect("response timeout")
}
async fn room(ws: &mut Socket, phase: Phase, players: usize) -> Snapshot {
    loop {
        if let ServerMessage::Room(r) = recv(ws).await
            && r.phase == phase
            && r.players.len() == players
        {
            return r;
        }
    }
}
async fn error(ws: &mut Socket) -> String {
    loop {
        if let ServerMessage::Error { message } = recv(ws).await {
            return message;
        }
    }
}
async fn connect(url: &str) -> (Socket, String) {
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");
    let ServerMessage::Welcome { player_id, .. } = recv(&mut ws).await else {
        panic!("welcome")
    };
    (ws, player_id)
}
#[tokio::test]
async fn two_clients_complete_race_reset_and_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    let server = tokio::spawn(code_racer_server::serve(listener, 2, 1800, 20));
    let (mut alice, alice_id) = connect(&url).await;
    let (mut bob, _) = connect(&url).await;
    send(
        &mut alice,
        ClientMessage::Create {
            version: VERSION,
            username: "Alice".into(),
            language: "french".into(),
        },
    )
    .await;
    let created = room(&mut alice, Phase::Waiting, 1).await;
    assert!(valid_code(&created.code));
    send(
        &mut bob,
        ClientMessage::Join {
            version: VERSION,
            code: created.code.clone(),
            username: "Bob".into(),
        },
    )
    .await;
    let joined = room(&mut bob, Phase::Waiting, 2).await;
    let host = room(&mut alice, Phase::Waiting, 2).await;
    assert_eq!(joined.text, host.text);
    send(&mut bob, ClientMessage::Start).await;
    assert!(error(&mut bob).await.contains("host"));
    send(&mut alice, ClientMessage::Start).await;
    assert!(error(&mut alice).await.contains("ready"));
    send(&mut alice, ClientMessage::Ready { ready: true }).await;
    send(&mut bob, ClientMessage::Ready { ready: true }).await;
    loop {
        let r = room(&mut alice, Phase::Waiting, 2).await;
        if r.players.iter().all(|p| p.ready) {
            break;
        }
    }
    send(&mut alice, ClientMessage::Start).await;
    let countdown = room(&mut alice, Phase::Countdown, 2).await;
    assert!(countdown.start_ms.expect("start") > countdown.server_ms);
    room(&mut alice, Phase::Racing, 2).await;
    room(&mut bob, Phase::Racing, 2).await;
    let n = grapheme_count(&joined.text);
    send(
        &mut bob,
        ClientMessage::Progress {
            position: n + 1,
            errors: 0,
            correct: n + 1,
            attempts: n + 1,
        },
    )
    .await;
    assert!(error(&mut bob).await.contains("Invalid"));
    send(
        &mut alice,
        ClientMessage::Progress {
            position: n / 2,
            errors: 1,
            correct: n / 2 - 1,
            attempts: n / 2,
        },
    )
    .await;
    loop {
        let r = room(&mut bob, Phase::Racing, 2).await;
        if r.players
            .iter()
            .any(|p| p.id == alice_id && p.position == n / 2)
        {
            break;
        }
    }
    send(
        &mut alice,
        ClientMessage::Progress {
            position: n,
            errors: 1,
            correct: n - 1,
            attempts: n,
        },
    )
    .await;
    loop {
        let r = room(&mut bob, Phase::Racing, 2).await;
        if r.players
            .iter()
            .any(|p| p.id == alice_id && p.finished_ms.is_some())
        {
            break;
        }
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    send(
        &mut bob,
        ClientMessage::Progress {
            position: n,
            errors: 0,
            correct: n,
            attempts: n,
        },
    )
    .await;
    let finished = room(&mut alice, Phase::Finished, 2).await;
    assert_eq!(finished.ranking()[0].name, "Alice");
    assert!(finished.players.iter().all(|p| p.finished_ms.is_some()));
    send(&mut alice, ClientMessage::Again).await;
    let reset = room(&mut alice, Phase::Waiting, 2).await;
    assert!(reset.players.iter().all(|p| !p.ready && p.position == 0));
    alice.close(None).await.expect("close");
    let promoted = room(&mut bob, Phase::Waiting, 1).await;
    assert_eq!(promoted.players[0].id, promoted.host);
    bob.send(Message::Text("invalid json".into()))
        .await
        .expect("invalid send");
    assert!(error(&mut bob).await.contains("Invalid protocol"));
    server.abort();
}
#[tokio::test]
async fn disconnect_during_race_and_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    let server = tokio::spawn(code_racer_server::serve(listener, 8, 1800, 1));
    let (mut a, _) = connect(&url).await;
    let (mut b, _) = connect(&url).await;
    send(
        &mut a,
        ClientMessage::Create {
            version: VERSION,
            username: "A".into(),
            language: "rust".into(),
        },
    )
    .await;
    let r = room(&mut a, Phase::Waiting, 1).await;
    send(
        &mut b,
        ClientMessage::Join {
            version: VERSION,
            username: "B".into(),
            code: r.code,
        },
    )
    .await;
    room(&mut a, Phase::Waiting, 2).await;
    room(&mut b, Phase::Waiting, 2).await;
    send(&mut a, ClientMessage::Ready { ready: true }).await;
    send(&mut b, ClientMessage::Ready { ready: true }).await;
    loop {
        if room(&mut a, Phase::Waiting, 2)
            .await
            .players
            .iter()
            .all(|p| p.ready)
        {
            break;
        }
    }
    send(&mut a, ClientMessage::Start).await;
    room(&mut b, Phase::Racing, 2).await;
    a.close(None).await.expect("close");
    let r = room(&mut b, Phase::Finished, 2).await;
    assert_eq!(r.players.iter().filter(|p| !p.connected).count(), 1);
    assert_eq!(
        r.ranking()
            .iter()
            .filter(|p| p.finished_ms.is_none())
            .count(),
        2
    );
    server.abort();
}
