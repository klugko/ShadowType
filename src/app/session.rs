use std::time::{Duration, Instant};

use code_racer_engine::{CodeLanguage, Stats, TypingSession};

use super::{Activity, App, Buffer, Focus, ink::Ink, practice::SoloRun, race::RaceClient};
use crate::{config::Look, network::Connection};

/// Everything needed to draw the text being typed.
#[derive(Debug, Clone, Copy)]
pub struct SessionView<'a> {
    pub session: &'a TypingSession,
    pub syntax: Option<CodeLanguage>,
    pub attribution: Option<&'a str>,
    /**
     * When the race ended for a player who had not finished its text: their
     * clock stops there. A finished text stops its clock by itself.
     */
    pub stopped_at: Option<Instant>,
    pub ink: Option<&'a Ink>,
    /// What the text looks like when it is prose; code looks like code.
    pub disguise: Option<Disguise<'a>>,
}

/**
 * What a prose text looks like on screen, and what its look takes from
 * the settings.
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disguise<'a> {
    pub look: Look,
    /// The language of the code a doc comment documents.
    pub language: CodeLanguage,
    /// Who signs an email draft.
    pub author: &'a str,
}

impl Default for Disguise<'_> {
    fn default() -> Self {
        Self {
            look: Look::Notes,
            language: CodeLanguage::default(),
            author: "",
        }
    }
}

impl SessionView<'_> {
    /// The statistics of the text at `now`, or when its clock stopped.
    pub fn stats(&self, now: Instant) -> Stats {
        self.session.stats(self.clock(now))
    }

    pub fn time_left(&self, now: Instant) -> Option<Duration> {
        self.session.time_left(self.clock(now))
    }

    fn clock(&self, now: Instant) -> Instant {
        self.stopped_at.map_or(now, |stop| stop.min(now))
    }
}

impl App {
    pub fn solo(&self) -> Option<&SoloRun> {
        match &self.activity {
            Some(Activity::Solo(run)) => Some(run),
            _ => None,
        }
    }

    pub fn race(&self) -> Option<&RaceClient> {
        match &self.activity {
            Some(Activity::Race(client)) => Some(client),
            _ => None,
        }
    }

    pub fn session_view(&self) -> Option<SessionView<'_>> {
        let (view, prose) = match &self.activity {
            Some(Activity::Solo(run)) => {
                let view = SessionView {
                    session: run.session(),
                    syntax: run.plan.syntax(),
                    attribution: run.attribution.as_deref(),
                    stopped_at: None,
                    ink: Some(run.ink()),
                    disguise: None,
                };
                (view, run.plan.is_prose())
            }
            Some(Activity::Race(client)) => (client.session_view()?, client.syntax().is_none()),
            None => return None,
        };
        Some(SessionView {
            disguise: prose.then(|| self.disguise()),
            ..view
        })
    }

    /**
     * What prose looks like in the session: the look of the settings, or
     * for a shuffle the one drawn for the session.
     */
    pub fn disguise(&self) -> Disguise<'_> {
        let look = match self.config.look {
            Look::Shuffle => self.shuffled,
            look => look,
        };
        Disguise {
            look,
            language: self.config.practice.code_language,
            author: &self.config.username,
        }
    }

    pub fn connection_mut(&mut self) -> Option<&mut Connection> {
        match &mut self.activity {
            Some(Activity::Race(client)) => Some(client.connection_mut()),
            _ => None,
        }
    }

    /// Whether the clock has to tick: a session is running or a countdown is shown.
    pub fn needs_ticks(&self) -> bool {
        match &self.activity {
            Some(Activity::Solo(run)) => run.is_in_progress(),
            Some(Activity::Race(client)) => client.is_live(),
            None => false,
        }
    }

    /**
     * Whether the player is in the middle of a text: leaving it takes a
     * confirmation, and its buffer is marked as modified.
     */
    pub fn session_in_progress(&self) -> bool {
        match &self.activity {
            Some(Activity::Solo(run)) => run.is_in_progress(),
            Some(Activity::Race(client)) => client.is_player_racing(),
            None => false,
        }
    }

    /// Whether keystrokes go to the text being typed.
    pub fn is_typing(&self) -> bool {
        if self.buffer != Buffer::Session || self.focus != Focus::Editor {
            return false;
        }
        match &self.activity {
            Some(Activity::Solo(run)) => !run.is_finished(),
            Some(Activity::Race(client)) => client.is_player_racing(),
            None => false,
        }
    }

    /// Whether the text refuses input until its first mistake is fixed.
    pub fn is_typing_blocked(&self) -> bool {
        self.is_typing()
            && self
                .session_view()
                .is_some_and(|view| view.session.is_blocked())
    }
}
