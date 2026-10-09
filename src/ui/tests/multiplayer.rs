use code_racer_protocol::MAX_ROOM_PLAYERS;

use super::*;

#[tokio::test]
async fn the_lobby_shows_the_whole_server_address_at_the_smallest_size() {
    let mut app = in_room_on("ws://10.0.0.9:8080", room(Phase::Lobby), Instant::now());
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    for expected in [
        "# invite: code-racer join FK72AD --server ws://10.0.0.9:8080",
        "server  = \"ws://10.0.0.9:8080\"",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    assert!(!text.contains("--host 0.0.0.0"), "{text}");
}

#[tokio::test]
async fn the_lobby_never_invites_teammates_to_their_own_computer() {
    let mut app = in_room(Phase::Lobby);
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let invite = text
        .lines()
        .find(|line| line.contains("# invite: code-racer join FK72AD --server ws://"))
        .unwrap_or_else(|| panic!("no invite:\n{text}"));
    assert!(!invite.contains("127.0.0.1"), "{invite}");
    assert!(invite.trim_end().ends_with(":9"), "{invite}");
    assert!(
        text.contains("# start the server with --host 0.0.0.0 for teammates to reach it"),
        "{text}"
    );
    assert!(text.contains("server  = \"ws://127.0.0.1:9\""), "{text}");
}

#[tokio::test]
async fn lobby_lists_players_and_invite_command() {
    let app = in_room(Phase::Lobby);
    let text = screen(&app, 120, 30);
    for expected in [
        "room FK72AD",
        "code-racer join FK72AD",
        "alice",
        "host",
        "ready",
        "## players",
        "▾ session",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
    let status = status_line(&text);
    assert!(status.trim_end().ends_with(" markdown"), "{status}");
}

#[tokio::test]
async fn race_shows_the_text_and_live_standings() {
    let app = in_room(Phase::Racing);
    let text = screen(&app, 120, 30);
    for expected in [
        "PLAYERS",
        "Simplicity",
        "jean",
        "alice",
        "offline",
        "━",
        "wpm",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[tokio::test]
async fn the_standings_always_show_the_player_even_last_of_a_full_room() {
    let mut app = in_room_at(race_of_eight(), Instant::now());
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let standings: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.contains("PLAYERS"))
        .skip(1)
        .take_while(|line| !line.contains("INSERT"))
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert!(
        standings.len() < 8,
        "the panel cannot show everyone:\n{text}"
    );
    let last = standings.last().expect("standings");
    assert!(
        last.trim_start().starts_with("8 racer1"),
        "own row with its real place:\n{text}"
    );
    assert!(standings[0].trim_start().starts_with("1 racer8"), "{text}");
}

#[tokio::test]
async fn a_full_lobby_keeps_the_players_row_and_what_to_do_next_in_view() {
    for size in [8, MAX_ROOM_PLAYERS] {
        let lobby = full_room(Phase::Lobby, size, |id| PlayerView {
            ready: id != 1,
            ..player(id, &racer(id), 0, None)
        });
        let mut app = in_room_at(lobby, Instant::now());
        app.resize(MIN_WIDTH, MIN_HEIGHT);
        let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        for expected in [
            "· jean            host, you   not ready",
            "more",
            "# waiting for 1 player to get ready",
            "r  toggle ready",
            "s  start the race",
        ] {
            assert!(
                text.contains(expected),
                "{size}: missing {expected}:\n{text}"
            );
        }
    }
}

#[tokio::test]
async fn full_results_keep_the_players_place_and_what_to_do_next_in_view() {
    let results = full_room(Phase::Finished, MAX_ROOM_PLAYERS, |id| {
        let time = if id == 1 { 90_000 } else { 40_000 + id * 100 };
        player(id, &racer(id), 120, Some(time))
    });
    let mut app = in_room_at(results, Instant::now());
    app.resize(MIN_WIDTH, MIN_HEIGHT);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    for expected in [
        " 32  jean ",
        "more",
        "you finished 32nd of 32",
        "r  back to the lobby",
        "Esc  leave",
    ] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

#[tokio::test]
async fn race_results_rank_players() {
    let app = in_room(Phase::Finished);
    let text = screen(&app, 120, 30);
    for expected in ["results", "0:41.2", "DNF", "you finished 1st"] {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}
