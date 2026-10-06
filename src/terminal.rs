//! Raw-mode terminal that is always restored: on exit, on error, on a
//! signal and when the interface panics.

use std::{
    io::{self, Stdout},
    panic,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type Backend = CrosstermBackend<Stdout>;

/// Whether the terminal is set up for the interface and still has to be
/// restored. Restoring twice would not be harmless: leaving the alternate
/// screen again moves the cursor back over what was printed meanwhile, such
/// as a panic report.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Owns the terminal while the interface runs.
#[derive(Debug)]
pub struct TerminalGuard {
    terminal: Terminal<Backend>,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        install_panic_hook();
        ACTIVE.store(true, Ordering::SeqCst);
        let entered = enable_raw_mode()
            .and_then(|()| execute!(io::stdout(), EnterAlternateScreen))
            .and_then(|()| Terminal::new(CrosstermBackend::new(io::stdout())));
        match entered {
            Ok(terminal) => {
                enable_optional_features();
                Ok(Self { terminal })
            }
            Err(error) => {
                restore();
                Err(error)
            }
        }
    }

    pub fn terminal(&mut self) -> &mut Terminal<Backend> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/// Bracketed paste lets the interface refuse pasted text while typing, and
/// the bar cursor looks like an editor's. Consoles without them, such as
/// the legacy Windows console, still run the interface.
fn enable_optional_features() {
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let _ = execute!(io::stdout(), SetCursorStyle::SteadyBar);
}

/// Leaves raw mode and the alternate screen, once. Every step runs even when
/// another failed, and failures are ignored on purpose: this runs while
/// exiting or panicking, when nothing better can be done.
fn restore() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, DisableBracketedPaste);
    let _ = execute!(stdout, LeaveAlternateScreen);
    let _ = execute!(stdout, SetCursorStyle::DefaultUserShape);
    let _ = execute!(stdout, Show);
}

/// A panic on the interface thread ends the program: the terminal is
/// restored first so that the report is readable. A panic on another
/// thread, such as in the network task, is caught by the runtime and the
/// interface goes on, so it is logged instead of printed over the screen.
fn install_panic_hook() {
    let interface = thread::current().id();
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if thread::current().id() == interface {
            restore();
            previous(info);
        } else if ACTIVE.load(Ordering::SeqCst) {
            tracing::error!("{info}");
        } else {
            previous(info);
        }
    }));
}
