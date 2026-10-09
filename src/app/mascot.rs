/*!
 * The mood of the mascot that lives under the explorer, from what the
 * player is doing. Drawing it is left to the interface.
 */

use std::time::{Duration, Instant};

use super::{Activity, App};

/**
 * How long without a key before the mascot falls asleep. Asleep it stops
 * moving, so that an idle editor uses no processor time.
 */
pub const NAP_AFTER: Duration = Duration::from_secs(45);
/// How long the mascot looks startled after a mistake.
const STARTLE: Duration = Duration::from_millis(900);
/// How long the mascot keeps typing along after the last key.
const TYPING_ALONG: Duration = Duration::from_secs(2);
/// How long the mascot rejoices once a text is done.
const REJOICING: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    /// Floating about, blinking now and then.
    Idle,
    /// Watching the text go by as the player types.
    Typing,
    /// Startled by a mistake.
    Oops,
    /// Glad the text is done.
    Happy,
    /// Thrilled by a personal best.
    Proud,
    /// Asleep, still, until the next key.
    Asleep,
}

impl App {
    pub fn mascot_mood(&self, now: Instant) -> Mood {
        let typing = self.session_view().and_then(|view| view.ink);
        if self.is_typing_blocked()
            || typing
                .and_then(|ink| ink.last_mistake())
                .is_some_and(|at| now.saturating_duration_since(at) < STARTLE)
        {
            return Mood::Oops;
        }
        if let Some(Activity::Solo(run)) = &self.activity
            && let Some(result) = &run.result
            && now.saturating_duration_since(result.at) < REJOICING
        {
            return if result.is_personal_best() {
                Mood::Proud
            } else {
                Mood::Happy
            };
        }
        let live_race = self.race().is_some_and(|client| client.is_live());
        let idle = now.saturating_duration_since(self.last_input.unwrap_or(self.born));
        if idle >= NAP_AFTER && !live_race {
            return Mood::Asleep;
        }
        let typing_along = self.is_typing()
            && typing
                .and_then(|ink| ink.last_key())
                .is_some_and(|at| now.saturating_duration_since(at) < TYPING_ALONG);
        if typing_along || live_race {
            Mood::Typing
        } else {
            Mood::Idle
        }
    }

    /// How long the mascot has lived at `now`, which its moves follow.
    pub fn mascot_age(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.born)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent};

    use super::*;
    use crate::{
        app::{
            Overrides,
            test_support::{type_next, type_remaining_at},
        },
        cli::Launch,
        config::Config,
        history::History,
    };

    fn app() -> App {
        let config = Config {
            username: "ada".to_owned(),
            ..Config::default()
        };
        App::new(
            config,
            &Overrides::default(),
            None,
            History::in_memory(),
            Vec::new(),
            Launch::Home,
        )
    }

    fn press(app: &mut App, code: KeyCode, at: Instant) {
        app.handle_key(KeyEvent::from(code), at);
    }

    #[test]
    fn the_mascot_naps_when_nothing_happens_and_wakes_at_a_key() {
        let mut app = app();
        let start = app.born;
        assert_eq!(app.mascot_mood(start), Mood::Idle);
        let later = start + NAP_AFTER;
        assert_eq!(app.mascot_mood(later), Mood::Asleep);
        press(&mut app, KeyCode::Char('j'), later);
        assert_eq!(app.mascot_mood(later), Mood::Idle);
    }

    #[test]
    fn the_mascot_types_along_and_startles_at_mistakes() {
        let mut app = app();
        let now = Instant::now();
        app.start_practice();
        type_next(&mut app, now);
        assert_eq!(app.mascot_mood(now), Mood::Typing);
        assert_eq!(app.mascot_mood(now + TYPING_ALONG), Mood::Idle);
        let expected = app.session_view().map(|view| {
            let session = view.session;
            session.target()[session.cursor()].clone()
        });
        let wrong = if expected.as_deref() == Some("x") {
            'y'
        } else {
            'x'
        };
        press(&mut app, KeyCode::Char(wrong), now);
        assert_eq!(app.mascot_mood(now), Mood::Oops);
    }

    #[test]
    fn the_mascot_rejoices_at_the_end_of_a_text() {
        let mut app = app();
        let now = Instant::now();
        app.start_practice();
        type_remaining_at(&mut app, now);
        assert_eq!(
            app.mascot_mood(now),
            Mood::Proud,
            "a first result is a best"
        );
        assert_ne!(app.mascot_mood(now + REJOICING), Mood::Proud);
    }
}
