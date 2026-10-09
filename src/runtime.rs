//! The event loop: keyboard, network and clock events drive the application,
//! and the screen is redrawn only after something changed, or while
//! something moves on it.

use std::{
    future::Future,
    io,
    pin::pin,
    time::{Duration, Instant},
};

use crossterm::event::{Event, EventStream};
use futures_util::{
    FutureExt, StreamExt,
    future::{BoxFuture, select_all},
};

use crate::{
    app::{App, Viewport, mouse::Hits},
    network::NetworkEvent,
    terminal::TerminalGuard,
    ui,
};

/// Refresh rate of timers and live statistics while a session runs.
const TICK: Duration = Duration::from_millis(100);
/// Longest time an idle loop sleeps for, which only bounds a sleep that
/// nothing waits on.
const IDLE: Duration = Duration::from_secs(3_600);
/// Longest wait, once the terminal is restored, for the room the player
/// was in to hear that they left.
const GOODBYE_TIME: Duration = Duration::from_secs(1);

/// Runs the interface until the user quits or the program is asked to
/// stop, then leaves the room the player is in.
pub async fn run(mut app: App) -> anyhow::Result<()> {
    let result = interact(&mut app).await;
    if let Some(connection) = app.finish() {
        connection.close(GOODBYE_TIME).await;
    }
    result
}

/// Drives `app` in the terminal, which is restored when this returns.
async fn interact(app: &mut App) -> anyhow::Result<()> {
    let mut shutdown = pin!(shutdown_signal());
    let mut guard = TerminalGuard::enter()?;
    let mut events = EventStream::new();
    let mut clock = Clock::default();
    let mut hits = Hits::default();
    let mut dirty = true;
    while !app.should_quit() {
        if dirty {
            fit_to_terminal(app, &mut guard)?;
            guard.set_mouse(app.config.mouse);
            guard.terminal().draw(|frame| {
                hits = ui::draw(frame, app, Instant::now());
            })?;
        }
        let ticking = app.needs_ticks();
        let period = [
            ticking.then_some(TICK),
            ui::frame_period(app, Instant::now()),
        ]
        .into_iter()
        .flatten()
        .min();
        let alarm = clock.next(period);
        tokio::select! {
            () = &mut shutdown => break,
            event = events.next() => match event {
                Some(Ok(event)) => dirty = handle_terminal_event(app, event, &hits),
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
            event = next_network_event(app) => {
                let event = event.unwrap_or_else(|| NetworkEvent::Closed {
                    reason: "connection lost".to_owned(),
                });
                app.handle_network(event, Instant::now());
                dirty = true;
            }
            () = tokio::time::sleep_until(alarm), if period.is_some() => {
                clock.rang();
                if ticking {
                    app.tick(Instant::now());
                }
                dirty = true;
            }
        }
    }
    Ok(())
}

/// When the loop wakes up by itself next: every period while there is one,
/// without catching up on wake-ups missed while busy.
#[derive(Debug, Default)]
struct Clock {
    next: Option<tokio::time::Instant>,
}

impl Clock {
    /// The next wake-up for `period`, far off without one, when nothing
    /// waits on it. A wake-up already set stays, so that events coming
    /// faster than the period never put it off.
    fn next(&mut self, period: Option<Duration>) -> tokio::time::Instant {
        let now = tokio::time::Instant::now();
        let Some(period) = period else {
            self.next = None;
            return now + IDLE;
        };
        let next = match self.next {
            Some(next) if next <= now + period => next,
            _ => now + period,
        };
        self.next = Some(next);
        next
    }

    /// The wake-up came: the next one is to be set from now.
    fn rang(&mut self) {
        self.next = None;
    }
}

fn handle_terminal_event(app: &mut App, event: Event, hits: &Hits) -> bool {
    match event {
        Event::Key(key) => {
            app.handle_key(key, Instant::now());
            true
        }
        Event::Paste(text) => {
            app.handle_paste(&text, Instant::now());
            true
        }
        Event::Mouse(mouse) => app.handle_mouse(mouse, hits, Instant::now()),
        Event::Resize(..) => true,
        _ => false,
    }
}

/// Gives `app` the size of the terminal, read before each draw rather than
/// taken from resize events: Windows reports in them the size of the
/// screen buffer, one more than the window each way, and consoles that do
/// not report resizes at all are caught up at the next redraw.
fn fit_to_terminal(app: &mut App, guard: &mut TerminalGuard) -> io::Result<()> {
    let size = guard.terminal().size()?;
    let viewport = Viewport {
        width: size.width,
        height: size.height,
    };
    if app.viewport != viewport {
        app.resize(viewport.width, viewport.height);
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::Overrides, cli::Launch, config::Config, history::History};

    #[test]
    fn a_resize_event_asks_for_a_redraw_without_trusting_its_size() {
        let mut app = App::new(
            Config::default(),
            &Overrides::default(),
            None,
            History::in_memory(),
            Vec::new(),
            Launch::Home,
        );
        let before = app.viewport;
        assert!(handle_terminal_event(
            &mut app,
            Event::Resize(121, 31),
            &Hits::default()
        ));
        assert_eq!(app.viewport, before, "the size is read before drawing");
        assert_ne!(
            app.viewport,
            Viewport {
                width: 121,
                height: 31
            }
        );
    }
}
