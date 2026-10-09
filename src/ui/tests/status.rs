use super::*;
use crate::config::Theme;

#[test]
fn the_status_line_keeps_its_statistics_at_the_smallest_size() {
    let mut app = app();
    command(&mut app, "set punctuation");
    command(&mut app, "set numbers");
    command(&mut app, "words 100");
    mistake(&mut app);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let status = status_line(&text);
    for expected in ["INSERT", "notes.md", "words 100", " 1 error ", " 00:00 "] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
    assert!(status.trim_end().ends_with(" 0%"), "{status}");
}

#[tokio::test]
async fn the_race_status_line_keeps_its_statistics_at_the_smallest_size() {
    let app = in_room(Phase::Racing);
    let text = screen(&app, MIN_WIDTH, MIN_HEIGHT);
    let status = status_line(&text);
    for expected in ["room FK72AD", " 0 errors ", " 00:00 "] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
    assert!(status.trim_end().ends_with(" 0%"), "{status}");
}

#[test]
fn blocked_typing_is_shown_in_the_status_line_in_the_error_colour() {
    let mut app = app();
    command(&mut app, "words 10");
    for _ in 0..code_racer_engine::ERROR_RUN_LIMIT {
        mistake(&mut app);
    }
    assert!(app.is_typing_blocked());
    let terminal = drawn(&app, MIN_WIDTH, MIN_HEIGHT, Instant::now());
    let status = MIN_HEIGHT - 2;
    let at = find(&terminal, "fix the mistake").expect("the blocked state");
    assert_eq!(at.1, status, "{}", text_of(&terminal));
    let palette = Palette::of(Theme::Editor);
    assert_eq!(style_at(&terminal, at).bg, Some(palette.error));
    assert!(status_line(&text_of(&terminal)).contains(" 10 errors "));
}

#[tokio::test]
async fn the_status_line_stops_when_the_race_ends_before_the_player_finishes() {
    let start = Instant::now();
    let mut app = in_room_at(room(Phase::Racing), start);
    for ch in "Simplicity".chars() {
        app.handle_key(
            KeyEvent::from(KeyCode::Char(ch)),
            start + Duration::from_secs(4),
        );
    }
    let end = start + Duration::from_secs(12);
    let mut over = room(Phase::Finished);
    over.players[0].progress.finish_ms = None;
    app.handle_network(NetworkEvent::Message(ServerMessage::Room(over)), end);

    let at_the_end = screen_at(&app, 120, 30, end);
    let later = screen_at(&app, 120, 30, end + Duration::from_secs(45));
    assert!(later.contains("DNF"), "{later}");
    assert!(status_line(&later).contains(" 00:12 "), "{later}");
    assert_eq!(status_line(&later), status_line(&at_the_end));
}
