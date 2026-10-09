mod completion;
mod erasing;
mod indentation;
mod keyboard;
mod samples;
mod unicode;

use super::*;

fn at(start: Instant, millis: u64) -> Instant {
    start + Duration::from_millis(millis)
}

fn type_text(session: &mut TypingSession, text: &str, now: Instant) {
    for ch in text.chars() {
        session.type_char(ch, now);
    }
}

/**
 * Types `keys` one every `interval_ms` from `start`, `\u{8}` standing for
 * Backspace, and returns the instant of the last key.
 */
fn play(session: &mut TypingSession, keys: &str, start: Instant, interval_ms: u64) -> Instant {
    let mut now = start;
    for (index, key) in (0..).zip(keys.chars()) {
        now = at(start, index * interval_ms);
        if key == '\u{8}' {
            session.backspace(now);
        } else {
            session.type_char(key, now);
        }
    }
    now
}

fn auto_indented(text: &str) -> TypingSession {
    let options = SessionOptions {
        auto_indent: true,
        ..SessionOptions::default()
    };
    TypingSession::new(text, options)
}
