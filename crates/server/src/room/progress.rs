use std::time::Instant;

use code_racer_protocol::{ErrorCode, PlayerId, Progress, ServerError};

use super::{
    Room,
    race::{Race, Stage},
};

const MAX_CHARS_PER_SECOND: f64 = 30.0;
const SPEED_BURST_CHARS: f64 = 5.0;

impl Room {
    /**
     * Records a player's typing counters and derives their speed, accuracy
     * and finishing time from the server clock.
     *
     * Reports of players who already finished are ignored, and so are those
     * arriving after the race ended, which were on their way when it did.
     * Implausible reports are rejected and leave the room unchanged.
     */
    pub fn report_progress(
        &mut self,
        by: PlayerId,
        counters: Progress,
        now: Instant,
    ) -> Result<(), ServerError> {
        let index = self.member_index(by)?;
        self.start_race_if_due(now);
        let race = match &self.stage {
            Stage::Racing(race) => race,
            Stage::Finished(_) => return Ok(()),
            Stage::Lobby | Stage::Countdown(_) => {
                return Err(ServerError::new(
                    ErrorCode::RaceNotRunning,
                    format!("room {} is not racing", self.code),
                ));
            }
        };
        let member = &mut self.members[index];
        if member.progress.is_finished() {
            return Ok(());
        }
        check_progress(member.counters, counters, race, now).map_err(|reason| {
            ServerError::new(
                ErrorCode::InvalidProgress,
                format!("progress rejected: {reason}"),
            )
        })?;
        member.record(counters, race, now);
        self.last_activity = now;
        Ok(())
    }
}

/**
 * Checks a report against the previous one and the race text. Auto-filled
 * indentation needs no keystroke, so it is exempt from the keystroke and
 * speed checks but must match indentation the text actually has.
 */
fn check_progress(
    previous: Progress,
    next: Progress,
    race: &Race,
    now: Instant,
) -> Result<(), &'static str> {
    let elapsed = race.elapsed(now);
    let speed_limit = elapsed.as_secs_f64() * MAX_CHARS_PER_SECOND + SPEED_BURST_CHARS;
    let counts = next.tally();
    let rules = [
        (next.typed <= race.length, "typed past the end of the text"),
        (
            next.correct <= next.typed,
            "more correct characters than typed ones",
        ),
        (
            next.indentation <= next.correct,
            "more indentation than correct characters",
        ),
        (
            counts.indentation <= race.indentation.within(counts.typed),
            "more indentation than the text has",
        ),
        (
            next.errors <= next.keystrokes,
            "more errors than keystrokes",
        ),
        (
            next.typed <= next.keystrokes.saturating_add(next.indentation),
            "more characters than keystrokes",
        ),
        (
            next.keystrokes >= previous.keystrokes,
            "keystrokes cannot decrease",
        ),
        (next.errors >= previous.errors, "errors cannot decrease"),
        (
            counts.correctly_typed() as f64 <= speed_limit,
            "faster than humanly possible",
        ),
    ];
    match rules.into_iter().find(|(holds, _)| !holds) {
        Some((_, broken)) => Err(broken),
        None => Ok(()),
    }
}
