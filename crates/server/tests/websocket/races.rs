use code_racer_protocol::{
    ClientMessage, ErrorCode, Phase, PlayerId, PlayerProgress, ServerMessage,
};

use crate::support::{quick_races, ready_pair, start, start_race, typed, wait_until_plausible};

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
