use std::time::{Duration, Instant};

use code_racer_protocol::{ClientMessage, Phase, Progress};

use super::RaceClient;
use crate::app::text_event::TextEvent;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

impl RaceClient {
    /**
     * Hands `event` to the race text, which takes keys only while the race
     * is on and the player's text unfinished, then reports the progress.
     * Returns whether the text took the key.
     */
    pub fn text_event(&mut self, event: TextEvent, now: Instant) -> bool {
        let open = event == TextEvent::Tick || self.accepts_typing();
        let taken = match &mut self.race {
            Some(race) if open => race.ink.apply(event, &mut race.session, now),
            _ => false,
        };
        self.report_progress(now);
        taken
    }

    /// Sends the player's progress when [`progress_due`] says so.
    fn report_progress(&mut self, now: Instant) {
        let Some(race) = &self.race else {
            return;
        };
        if self.phase() != Some(Phase::Racing) {
            return;
        }
        let progress = Progress::from(race.session.tally());
        let due = progress_due(self.reported, progress, race.session.is_finished(), now);
        if due && self.connection.send(ClientMessage::Progress(progress)) {
            self.reported = Some((now, progress));
        }
    }
}

/**
 * Whether `progress` should be sent: the first report at once, then only
 * a changed one, at most every [`PROGRESS_INTERVAL`] but at once when the
 * text is complete, so that the server times the finish exactly.
 */
fn progress_due(
    last: Option<(Instant, Progress)>,
    progress: Progress,
    complete: bool,
    now: Instant,
) -> bool {
    last.is_none_or(|(at, sent)| {
        sent != progress && (complete || now.duration_since(at) >= PROGRESS_INTERVAL)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(count: u32) -> Progress {
        Progress {
            typed: count,
            correct: count,
            keystrokes: count,
            ..Progress::default()
        }
    }

    #[test]
    fn progress_is_sent_at_once_then_throttled() {
        let start = Instant::now();
        let soon = start + PROGRESS_INTERVAL / 2;
        let later = start + PROGRESS_INTERVAL;
        assert!(progress_due(None, typed(0), false, start), "first report");
        let last = Some((start, typed(1)));
        assert!(!progress_due(last, typed(1), false, later), "unchanged");
        assert!(!progress_due(last, typed(2), false, soon), "too soon");
        assert!(progress_due(last, typed(2), false, later));
        assert!(progress_due(last, typed(2), true, soon), "completion");
        assert!(!progress_due(last, typed(1), true, soon), "completion sent");
    }
}
