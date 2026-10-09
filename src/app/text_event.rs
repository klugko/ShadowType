//! What happens to the text being typed, solo or in a race.

use std::time::Instant;

use code_racer_engine::TypingSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEvent {
    Typed(char),
    Backspace,
    /// Ctrl+W or Alt+Backspace.
    DeleteWord,
    /// The clock moved on, which ends a timed text.
    Tick,
}

impl TextEvent {
    /// Returns whether the session took the key, never for a tick.
    pub fn apply_to(self, session: &mut TypingSession, now: Instant) -> bool {
        match self {
            Self::Typed(ch) => session.type_char(ch, now),
            Self::Backspace => session.backspace(now),
            Self::DeleteWord => session.delete_word(now),
            Self::Tick => {
                session.update(now);
                false
            }
        }
    }
}
