/*!
 * Raw-mode terminal that is always restored: on exit, on error, on a
 * signal and when the interface panics.
 */

use std::{
    io::{self, Stdout},
    panic,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type Backend = CrosstermBackend<Stdout>;

/**
 * Whether the terminal is set up for the interface and still has to be
 * restored. Restoring twice would not be harmless: leaving the alternate
 * screen again moves the cursor back over what was printed meanwhile, such
 * as a panic report.
 */
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Whether the terminal reports the mouse, which restoring turns off.
static MOUSE: AtomicBool = AtomicBool::new(false);

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
                window_input::enable();
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

    /**
     * Asks the terminal to report the mouse, or to keep it for selecting
     * text. Terminals without mouse reports run the interface all the same.
     */
    pub fn set_mouse(&mut self, captured: bool) {
        if MOUSE.load(Ordering::SeqCst) == captured {
            return;
        }
        let changed = if captured {
            execute!(io::stdout(), EnableMouseCapture)
        } else {
            execute!(io::stdout(), DisableMouseCapture)
        };
        if changed.is_ok() {
            MOUSE.store(captured, Ordering::SeqCst);
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/**
 * Bracketed paste lets the interface refuse pasted text while typing, and
 * the bar cursor looks like an editor's. Consoles without them, such as
 * the legacy Windows console, still run the interface.
 */
fn enable_optional_features() {
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let _ = execute!(io::stdout(), SetCursorStyle::SteadyBar);
}

/**
 * Leaves raw mode and the alternate screen, once. Every step runs even when
 * another failed, and failures are ignored on purpose: this runs while
 * exiting or panicking, when nothing better can be done.
 */
fn restore() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut stdout = io::stdout();
    /*
     * Before raw mode goes: on Windows, it gives the console back the mode
     * it had when the capture began, which raw mode would undo.
     */
    if MOUSE.swap(false, Ordering::SeqCst) {
        let _ = execute!(stdout, DisableMouseCapture);
    }
    let _ = disable_raw_mode();
    window_input::restore();
    let _ = execute!(stdout, DisableBracketedPaste);
    let _ = execute!(stdout, LeaveAlternateScreen);
    let _ = execute!(stdout, SetCursorStyle::DefaultUserShape);
    let _ = execute!(stdout, Show);
}

/**
 * Resizes of a Windows console reach the program only when it asks for
 * window input, which crossterm does for mouse capture alone. Asking is
 * best effort: without it, a resize is noticed at the next redraw.
 */
#[cfg(windows)]
mod window_input {
    use std::{io, sync::OnceLock};

    use crossterm_winapi::{ConsoleMode, Handle};

    /// `ENABLE_WINDOW_INPUT` of the console API.
    const WINDOW_INPUT: u32 = 0x0008;

    /// Whether the console had window input before [`enable`] asked for it.
    static HAD_WINDOW_INPUT: OnceLock<bool> = OnceLock::new();

    pub fn enable() {
        if let Ok(had) = update(|mode| mode | WINDOW_INPUT) {
            let _ = HAD_WINDOW_INPUT.set(had);
        }
    }

    /**
     * Gives the console back the window input setting it had, leaving the
     * rest of its mode to the code that changed it.
     */
    pub fn restore() {
        if HAD_WINDOW_INPUT.get() == Some(&false) {
            let _ = update(|mode| mode & !WINDOW_INPUT);
        }
    }

    /**
     * Changes the input mode of the console with `change`; returns whether
     * it had window input before.
     */
    fn update(change: impl FnOnce(u32) -> u32) -> io::Result<bool> {
        let console = ConsoleMode::from(Handle::current_in_handle()?);
        let mode = console.mode()?;
        console.set_mode(change(mode))?;
        Ok(mode & WINDOW_INPUT != 0)
    }
}

/// Other terminals report their resizes by themselves.
#[cfg(not(windows))]
mod window_input {
    pub fn enable() {}

    pub fn restore() {}
}

/**
 * A panic on the interface thread ends the program: the terminal is
 * restored first so that the report is readable. A panic on another
 * thread, such as in the network task, is caught by the runtime and the
 * interface goes on, so it is logged instead of printed over the screen.
 */
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
