//! Events other than key presses: pasted text, the network and the clock,
//! and the quiet period that follows the end of typing.

use std::time::{Duration, Instant};

use super::{Activity, App, Buffer, race::Outcome};
use crate::{history::Record, network::NetworkEvent};

/// How long keys are ignored once typing stops by itself: a fast typist
/// still has keystrokes on their way, which would otherwise be taken as
/// commands, such as `q` to quit or Enter to start another text.
const QUIET_PERIOD: Duration = Duration::from_millis(600);

impl App {
    pub fn handle_paste(&mut self, text: &str, now: Instant) {
        if self.is_quiet(now) {
            return;
        }
        if let Some(prompt) = &mut self.prompt {
            prompt.input.insert_str(text);
        } else if let Some(edit) = &mut self.editing {
            edit.input.insert_str(text);
        } else if self.is_typing() {
            self.error("pasting is disabled while typing");
        }
    }

    pub fn handle_network(&mut self, event: NetworkEvent, now: Instant) {
        let was_typing = self.is_typing();
        let Some(Activity::Race(client)) = &mut self.activity else {
            return;
        };
        match client.handle(event, now) {
            Outcome::Nothing => {}
            Outcome::Entered(code) => {
                self.room_code = code.to_string();
                if self.buffer == Buffer::Race {
                    self.open(Buffer::Session);
                }
                self.info(format!(
                    "in room {code}, share the code with your teammates"
                ));
            }
            Outcome::Starting => self.show_race(),
            Outcome::Failure(text) => self.error(text),
            Outcome::Closed(reason) => {
                self.activity = None;
                if self.buffer == Buffer::Session {
                    self.open(Buffer::Race);
                }
                self.error(reason);
            }
            Outcome::Finished(record) => self.save_record(record),
        }
        self.quiet_if_typing_stopped(was_typing, now);
    }

    pub fn tick(&mut self, now: Instant) {
        let was_typing = self.is_typing();
        match &mut self.activity {
            Some(Activity::Solo(run)) => {
                run.session.update(now);
                self.conclude_solo(now);
            }
            Some(Activity::Race(client)) => {
                if let Some(session) = client.session_mut() {
                    session.update(now);
                }
                client.report_progress(now);
            }
            None => {}
        }
        self.quiet_if_typing_stopped(was_typing, now);
    }

    /// Brings the player to the race text when the countdown begins,
    /// wherever they were: a command line or a field being typed would
    /// otherwise take the first keystrokes of the race.
    fn show_race(&mut self) {
        self.prompt = None;
        self.editing = None;
        self.open(Buffer::Session);
        self.info("the race is starting");
    }

    pub(super) fn save_record(&mut self, record: Record) {
        if let Err(error) = self.history.add(record) {
            self.error(format!("cannot save history: {error}"));
        }
    }

    /// Whether keys are still ignored after typing stopped by itself.
    pub(super) fn is_quiet(&self, now: Instant) -> bool {
        self.quiet_until.is_some_and(|until| now < until)
    }

    /// Starts the quiet period when typing has just stopped, unless the
    /// player stopped it: the text was completed, the time ran out, the race
    /// ended or the connection was lost. The message on screen stays, as
    /// the ignored keys do not dismiss it.
    pub(super) fn quiet_if_typing_stopped(&mut self, was_typing: bool, now: Instant) {
        if was_typing && !self.is_typing() {
            self.quiet_until = Some(now + QUIET_PERIOD);
        }
    }
}
