mod members;
mod progress;
mod race;

use std::time::{Duration, Instant};

use code_racer_engine::{Language, TextSource};
use code_racer_protocol::{ErrorCode, PlayerId, PlayerView, Progress, ServerError, Username};

use super::Room;

const ALICE: PlayerId = PlayerId(1);
const BOB: PlayerId = PlayerId(2);
const CAROL: PlayerId = PlayerId(3);
const STRANGER: PlayerId = PlayerId(99);
const COUNTDOWN: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(300);
const TEXT_LENGTH: u32 = 100;

struct Clock(Instant);

impl Clock {
    fn at(&self, millis: u64) -> Instant {
        self.0 + Duration::from_millis(millis)
    }

    fn racing(&self, millis: u64) -> Instant {
        self.at(millis) + COUNTDOWN
    }
}

fn name(id: PlayerId) -> Username {
    match id {
        ALICE => "Alice",
        BOB => "Bob",
        CAROL => "Carol",
        _ => "Stranger",
    }
    .parse()
    .expect("valid name")
}

fn lobby(clock: &Clock, players: &[PlayerId], max_players: u8) -> Room {
    let code = "ABC234".parse().expect("valid code");
    let text = TextSource::Quote {
        language: Language::English,
    };
    let mut room = Room::new(code, ALICE, name(ALICE), text, max_players, clock.at(0));
    for &id in players {
        room.join(id, name(id), clock.at(0)).expect("join");
    }
    room
}

fn counting_down(clock: &Clock, players: &[PlayerId]) -> Room {
    counting_down_on(clock, players, &"a".repeat(TEXT_LENGTH as usize))
}

fn counting_down_on(clock: &Clock, players: &[PlayerId], text: &str) -> Room {
    let mut room = lobby(clock, players, 8);
    for id in std::iter::once(ALICE).chain(players.iter().copied()) {
        room.set_ready(id, true, clock.at(0)).expect("ready");
    }
    room.start_countdown(ALICE, text, clock.at(0), COUNTDOWN)
        .expect("start");
    room
}

fn racing(clock: &Clock, players: &[PlayerId]) -> Room {
    let mut room = counting_down(clock, players);
    assert!(room.advance(clock.racing(0), TIMEOUT));
    room
}

fn racing_on(clock: &Clock, text: &str) -> Room {
    let mut room = counting_down_on(clock, &[], text);
    assert!(room.advance(clock.racing(0), TIMEOUT));
    room
}

fn progress(typed: u32, correct: u32, keystrokes: u32, errors: u32) -> Progress {
    Progress {
        typed,
        correct,
        indentation: 0,
        keystrokes,
        errors,
    }
}

fn indented(typed: u32, correct: u32, indentation: u32, keystrokes: u32) -> Progress {
    Progress {
        indentation,
        ..progress(typed, correct, keystrokes, 0)
    }
}

fn clean(characters: u32) -> Progress {
    progress(characters, characters, characters, 0)
}

fn player(room: &Room, id: PlayerId) -> PlayerView {
    room.view().player(id).cloned().expect("member")
}

fn error_code<T>(result: Result<T, ServerError>) -> Option<ErrorCode> {
    result.err().map(|error| error.code)
}
