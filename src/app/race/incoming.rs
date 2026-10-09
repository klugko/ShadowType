use std::time::{Duration, Instant};

use code_racer_engine::{SessionOptions, TypingSession};
use code_racer_protocol::{ClientMessage, ErrorCode, Phase, RoomView, ServerMessage};

use super::{Intent, LiveRace, Outcome, RaceClient};
use crate::{app::ink::Ink, history::Record, network::NetworkEvent};

impl RaceClient {
    pub fn handle(&mut self, event: NetworkEvent, now: Instant) -> Outcome {
        match event {
            NetworkEvent::Connected(player) => {
                self.player = Some(player);
                self.send_intent();
                Outcome::Nothing
            }
            NetworkEvent::Message(message) => self.handle_message(message, now),
            NetworkEvent::Closed { reason } => Outcome::Closed(reason),
        }
    }

    fn send_intent(&mut self) {
        let message = match self.intent.take() {
            Some(Intent::Create(text)) => ClientMessage::CreateRoom { text },
            Some(Intent::Join(code)) => ClientMessage::JoinRoom { code },
            None => return,
        };
        self.connection.send(message);
    }

    fn handle_message(&mut self, message: ServerMessage, now: Instant) -> Outcome {
        if matches!(message, ServerMessage::Room(_) | ServerMessage::Error(_)) {
            self.awaiting_answer = false;
        }
        match message {
            ServerMessage::Room(room) => self.apply_room(room, now),
            ServerMessage::Countdown { text, duration_ms } => {
                self.begin_countdown(&text, duration_ms, now);
                Outcome::Starting
            }
            ServerMessage::Error(error) if self.room.is_none() => Outcome::Closed(error.message),
            ServerMessage::Error(error) if error.code == ErrorCode::RoomNotFound => {
                Outcome::Closed(error.message)
            }
            ServerMessage::Error(error) => Outcome::Failure(error.message),
            ServerMessage::Welcome { .. } => Outcome::Nothing,
        }
    }

    fn begin_countdown(&mut self, text: &str, duration_ms: u32, now: Instant) {
        let options = SessionOptions {
            auto_indent: self.syntax().is_some(),
            ..SessionOptions::default()
        };
        self.race = Some(LiveRace {
            session: TypingSession::new(text, options),
            ink: Ink::default(),
            countdown_ends: now + Duration::from_millis(duration_ms.into()),
            ended_at: None,
            recorded: false,
        });
        self.reported = None;
    }

    fn apply_room(&mut self, room: RoomView, now: Instant) -> Outcome {
        let entered = self.room.is_none().then(|| room.code.clone());
        let phase = room.phase;
        self.room = Some(room);
        match (phase, &mut self.race) {
            (Phase::Lobby, _) => self.race = None,
            (Phase::Racing, Some(race)) => race.session.start(now),
            (Phase::Finished, Some(race)) => {
                race.ended_at.get_or_insert(now);
            }
            _ => {}
        }
        if let Some(record) = self.conclude(now) {
            return Outcome::Finished(record);
        }
        entered.map_or(Outcome::Nothing, Outcome::Entered)
    }

    /**
     * Whether the player's race is over: the server timed their finish,
     * or the race ended, finished or not.
     */
    fn is_over_for_me(&self) -> bool {
        self.phase() == Some(Phase::Finished)
            || self.me().is_some_and(|me| me.progress.is_finished())
    }

    /**
     * The record of the race once it is over for the player, and only
     * once: as soon as the server timed their finish, so that leaving
     * before the slowest player finishes keeps it. It takes the server's
     * figures where it has them, as in the standings, and the local ones
     * otherwise. A race where nothing was typed is not recorded.
     */
    fn conclude(&mut self, now: Instant) -> Option<Record> {
        if !self.is_over_for_me() {
            return None;
        }
        let progress = self.me()?.progress;
        let language = self.plan()?.language_label();
        let stats = self.session_view()?.stats(now);
        let race = self.race.as_mut()?;
        if race.recorded || stats.keystrokes == 0 {
            return None;
        }
        race.recorded = true;
        let duration = progress
            .finish_ms
            .map_or(stats.elapsed.as_secs_f64(), |ms| ms as f64 / 1000.0);
        Some(Record {
            duration,
            wpm: progress.wpm,
            accuracy: progress.accuracy,
            ..Record::from_stats(Record::RACE_MODE.to_owned(), language, &stats)
        })
    }
}
