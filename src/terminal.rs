//! Raw-mode terminal that is always restored: on exit, on error and on panic.

use std::{
    io::{self, Stdout},
    panic,
};

use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type Backend = CrosstermBackend<Stdout>;

/// Owns the terminal while the interface runs.
#[derive(Debug)]
pub struct TerminalGuard {
    terminal: Terminal<Backend>,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        install_panic_hook();
        enable_raw_mode()?;
        let entered = execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            SetCursorStyle::SteadyBar
        )
        .and_then(|()| Terminal::new(CrosstermBackend::new(io::stdout())));
        match entered {
            Ok(terminal) => Ok(Self { terminal }),
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

/// Leaves raw mode and the alternate screen. Failures are ignored on purpose:
/// this runs while exiting or panicking, when nothing better can be done.
pub fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        SetCursorStyle::DefaultUserShape,
        Show
    );
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
