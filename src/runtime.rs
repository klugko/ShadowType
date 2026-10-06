//! The event loop: keyboard, network and clock events drive the application,
//! and the screen is redrawn only after something changed.

use std::{
    future::Future,
    pin::pin,
    time::{Duration, Instant},
};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures_util::{
    FutureExt, StreamExt,
    future::{BoxFuture, select_all},
};
use tokio::time::MissedTickBehavior;

use crate::{app::App, network::NetworkEvent, terminal::TerminalGuard, ui};

/// Refresh rate of timers and live statistics while a session runs.
const TICK: Duration = Duration::from_millis(100);

pub async fn run(mut app: App) -> anyhow::Result<()> {
    let mut shutdown = pin!(shutdown_signal());
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
            () = &mut shutdown => break,
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
            app.handle_paste(&text, Instant::now());
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

/// Resolves when the program is asked to stop from outside: SIGTERM, SIGHUP
/// or SIGINT on Unix, closing the console or Ctrl+Break on Windows. In raw
/// mode, Ctrl+C is a key rather than a signal.
///
/// The listeners are registered before this returns, so that a signal sent
/// before the first poll is not lost. Without any, it never resolves.
fn shutdown_signal() -> impl Future<Output = ()> {
    let listeners = signal_listeners();
    async move {
        if listeners.is_empty() {
            std::future::pending::<()>().await;
        }
        select_all(listeners).await;
    }
}

#[cfg(unix)]
fn signal_listeners() -> Vec<BoxFuture<'static, ()>> {
    use tokio::signal::unix::{SignalKind, signal};
    [
        SignalKind::terminate(),
        SignalKind::hangup(),
        SignalKind::interrupt(),
    ]
    .into_iter()
    .filter_map(|kind| signal(kind).ok())
    .map(|mut listener| {
        async move {
            listener.recv().await;
        }
        .boxed()
    })
    .collect()
}

#[cfg(windows)]
fn signal_listeners() -> Vec<BoxFuture<'static, ()>> {
    use tokio::signal::windows::{ctrl_break, ctrl_close};
    let mut listeners = Vec::new();
    if let Ok(mut listener) = ctrl_close() {
        listeners.push(
            async move {
                listener.recv().await;
            }
            .boxed(),
        );
    }
    if let Ok(mut listener) = ctrl_break() {
        listeners.push(
            async move {
                listener.recv().await;
            }
            .boxed(),
        );
    }
    listeners
}

#[cfg(not(any(unix, windows)))]
fn signal_listeners() -> Vec<BoxFuture<'static, ()>> {
    Vec::new()
}
