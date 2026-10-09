use code_racer_protocol::MAX_ROOM_PLAYERS;
use crossterm::event::KeyModifiers;

use super::*;
use crate::config::Theme;

const PREVIEW_SIZES: [(u16, u16); 2] = [(MIN_WIDTH, MIN_HEIGHT), (200, 60)];

/// `app` drawn at every preview size, resized first as by the terminal.
fn shots(name: &str, app: &mut App) -> Vec<(String, String)> {
    PREVIEW_SIZES
        .iter()
        .map(|&(width, height)| {
            app.resize(width, height);
            (
                format!("{name} {width}x{height}"),
                screen(app, width, height),
            )
        })
        .collect()
}

/**
 * Prints every screen; run with `cargo test preview_screens -- --ignored --nocapture`
 * to review the interface without a terminal.
 */
#[tokio::test]
#[ignore = "visual preview, not an assertion"]
async fn preview_screens() {
    let mut app = app();
    for wpm in [58.0, 61.0, 66.0, 64.0, 70.0, 73.0, 71.0, 78.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    let mut screens = shots("home", &mut app);
    for page in ["race", "config", "history", "help"] {
        command(&mut app, page);
        screens.extend(shots(page, &mut app));
    }
    command(&mut app, "code rust");
    type_prefix(&mut app, 140);
    screens.extend(shots("code", &mut app));
    press(&mut app, KeyCode::Esc);
    screens.extend(shots("leaving", &mut app));
    press(&mut app, KeyCode::Esc);
    command(&mut app, "words 50");
    type_prefix(&mut app, 60);
    press(&mut app, KeyCode::Char('x'));
    screens.extend(shots("words", &mut app));
    for _ in 0..code_racer_engine::ERROR_RUN_LIMIT {
        press(&mut app, KeyCode::Char('x'));
    }
    screens.extend(shots("blocked", &mut app));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    command(&mut app, "quote");
    type_whole_text(&mut app, Duration::from_secs(20));
    screens.extend(shots("results", &mut app));
    for phase in [Phase::Lobby, Phase::Racing, Phase::Finished] {
        screens.extend(shots(&format!("{phase:?}"), &mut in_room(phase)));
    }
    let mut full = room(Phase::Racing);
    full.players = (1..=8_u32)
        .map(|n| player(n.into(), &format!("racer{n}"), 10 * n, None))
        .collect();
    screens.extend(shots("full room", &mut in_room_at(full, Instant::now())));
    let lobby = full_room(Phase::Lobby, MAX_ROOM_PLAYERS, |id| {
        player(id, &racer(id), 0, None)
    });
    screens.extend(shots("full lobby", &mut in_room_at(lobby, Instant::now())));
    let results = full_room(Phase::Finished, MAX_ROOM_PLAYERS, |id| {
        player(id, &racer(id), 120, Some(40_000 + id * 100))
    });
    screens.extend(shots(
        "full results",
        &mut in_room_at(results, Instant::now()),
    ));
    screens.extend(long_input_shots());
    screens.extend(look_shots());
    screens.extend(vscode_shots());
    for (name, text) in screens {
        println!("──── {name}\n{text}");
    }
}

fn vscode_shots() -> Vec<(String, String)> {
    let mut vscode = app();
    vscode.config.theme = Theme::VsCode;
    command(&mut vscode, "code typescript");
    type_prefix(&mut vscode, 40);
    shots("vscode", &mut vscode)
}

/// The command palette, then a words session in every look.
fn look_shots() -> Vec<(String, String)> {
    let mut app = app();
    for wpm in [58.0, 61.0] {
        app.history.add(record(wpm)).expect("in memory");
    }
    app.handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let mut screens = shots("palette", &mut app);
    type_keys(&mut app, "cod");
    screens.extend(shots("palette code", &mut app));
    press(&mut app, KeyCode::Esc);
    for look in crate::config::Look::DISGUISES {
        command(&mut app, &format!("set look={look}"));
        command(&mut app, "words 25");
        type_prefix(&mut app, 70);
        screens.extend(shots(&format!("look {look}"), &mut app));
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Esc);
    }
    screens
}

/// A server address and then a command too long for their line, being typed.
fn long_input_shots() -> Vec<(String, String)> {
    let mut app = app();
    command(&mut app, "config");
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    type_keys(
        &mut app,
        "/a-very-long-path/to/a/race/server/behind/a/proxy",
    );
    let mut screens = shots("long value", &mut app);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char(':'));
    type_keys(&mut app, &format!("e {}", "/a-very-long-path".repeat(6)));
    screens.extend(shots("long command", &mut app));
    screens
}
