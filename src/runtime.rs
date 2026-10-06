//! The event loop: keyboard, network and clock events drive the application,
//! and the screen is redrawn only after something changed.

use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures_util::StreamExt;
use tokio::time::MissedTickBehavior;

use crate::{app::App, network::NetworkEvent, terminal::TerminalGuard, ui};

/// Refresh rate of timers and live statistics while a session runs.
const TICK: Duration = Duration::from_millis(100);

pub async fn run(mut app: App) -> anyhow::Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut dirty = true;
    while !app.should_quit() {
        if dirty {
            guard
                .terminal()
                .draw(|frame| ui::draw(frame, &app, Instant::now()))?;
        }
        let ticking = app.needs_ticks();
        tokio::select! {
            event = events.next() => match event {
                Some(Ok(event)) => dirty = handle_terminal_event(&mut app, event),
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
            event = next_network_event(&mut app) => {
                let event = event.unwrap_or_else(|| NetworkEvent::Closed {
                    reason: "connection lost".to_owned(),
                });
                app.handle_network(event, Instant::now());
                dirty = true;
            }
            _ = ticker.tick(), if ticking => {
                app.tick(Instant::now());
                dirty = true;
            }
        }
    }
    Ok(())
}

fn handle_terminal_event(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            app.handle_key(key, Instant::now());
            true
        }
        Event::Paste(text) => {
            app.handle_paste(&text);
            true
        }
        Event::Resize(..) => true,
        _ => false,
    }
}

async fn next_network_event(app: &mut App) -> Option<NetworkEvent> {
    match app.connection_mut() {
        Some(connection) => connection.next_event().await,
        None => std::future::pending().await,
    }
}
