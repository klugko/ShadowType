use std::time::{Duration, Instant};

use crate::{
    app::{Activity, App, Buffer},
    network::Connection,
};

/// Longest time between the two presses of Esc that leave a session.
const LEAVE_CONFIRMATION: Duration = Duration::from_secs(2);

impl App {
    /**
     * Abandons the solo session or leaves the room. While there is
     * something to lose, it takes a second Esc: in an editor Esc is a
     * reflex, and one press would throw it away.
     */
    pub(crate) fn leave_session(&mut self, now: Instant) {
        let (asked, done) = match &self.activity {
            Some(Activity::Solo(_)) => (
                "press Esc again to abandon the session".to_owned(),
                "session abandoned".to_owned(),
            ),
            Some(Activity::Race(client)) => (
                format!("press Esc again to leave {}", client.room_label()),
                format!("left {}", client.room_label()),
            ),
            None => return,
        };
        if self.confirm_leave(now) {
            self.close_session();
            self.info(done);
        } else {
            self.info(asked);
        }
    }

    /**
     * Whether leaving is confirmed: at once when there is nothing to lose,
     * otherwise by a second Esc within [`LEAVE_CONFIRMATION`] of the first.
     */
    fn confirm_leave(&mut self, now: Instant) -> bool {
        let confirmed = !self.leaving_loses_something()
            || self
                .leave_armed
                .is_some_and(|armed| now.duration_since(armed) <= LEAVE_CONFIRMATION);
        self.leave_armed = (!confirmed).then_some(now);
        confirmed
    }

    /**
     * Whether leaving would lose a solo text in progress, or a race from
     * its countdown to its results, where the host may start another one.
     */
    fn leaving_loses_something(&self) -> bool {
        match &self.activity {
            Some(Activity::Solo(run)) => run.is_in_progress(),
            Some(Activity::Race(client)) => client.shows_a_race(),
            None => false,
        }
    }

    /// Ends the running activity and goes back to the buffer it was started from.
    pub(crate) fn close_session(&mut self) {
        let origin = match self.activity {
            Some(Activity::Race(_)) => Buffer::Race,
            _ => Buffer::Practice,
        };
        self.end_activity();
        if self.buffer == Buffer::Session {
            self.open(origin);
        }
    }

    /**
     * Ends the running activity, leaving the room the player is in. Returns
     * the connection to that room, which sends the goodbye, then closes,
     * once dropped.
     */
    pub(super) fn end_activity(&mut self) -> Option<Connection> {
        match self.activity.take() {
            Some(Activity::Race(client)) => Some(client.leave()),
            _ => None,
        }
    }

    /**
     * Ends the run, leaving the room the player is in. Returns the
     * connection to that room, which the program should give time to send
     * the goodbye before it exits.
     */
    pub fn finish(mut self) -> Option<Connection> {
        self.end_activity()
    }
}
