//! Multiplayer: the race form and the client side of a room.

mod form;
mod incoming;
mod invite;
mod requests;
mod typing;

use std::time::{Duration, Instant};

use code_racer_engine::{CodeLanguage, TextSource, TypingSession};
use code_racer_protocol::{
    ClientMessage, Phase, PlayerId, PlayerView, Progress, RoomCode, RoomView, Username,
};

use crate::{
    app::{
        Disguise, SessionView,
        ink::Ink,
        practice::{Plan, code_file_name, disguised_name},
    },
    cli::Launch,
    config::Look,
    history::Record,
    network::{self, Connection},
};
pub use form::{Field, adjust, fields, row, section};
use invite::SharedServer;
pub use requests::RoomRequest;

/// Why the player connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Create(TextSource),
    Join(RoomCode),
}

impl From<Intent> for Launch {
    fn from(intent: Intent) -> Self {
        match intent {
            Intent::Create(text) => Self::Create(text),
            Intent::Join(code) => Self::Join(code),
        }
    }
}

/// Where the player is in the life of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Connecting,
    Joining,
    Lobby,
    Countdown(Duration),
    Racing,
    Finished,
}

/// What the application should do after a network event.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Nothing,
    Entered(RoomCode),
    /// The countdown of a race began: the player should see its text.
    Starting,
    Failure(String),
    Closed(String),
    /// The player's own race is over, with this record to keep.
    Finished(Record),
}

/// How teammates join the room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /**
     * What they run, such as
     * `code-racer join FK72AD --server ws://192.168.1.42:8080`.
     */
    pub command: String,
    /**
     * What the host must check for the command to work, when the server
     * runs on their computer.
     */
    pub note: Option<&'static str>,
}

/// The race currently typed by the player.
#[derive(Debug)]
struct LiveRace {
    session: TypingSession,
    ink: Ink,
    countdown_ends: Instant,
    /**
     * When the room finished the race, which stops the clock of a text
     * the player had not finished.
     */
    ended_at: Option<Instant>,
    recorded: bool,
}

/// Connection to the race server and local view of the room.
#[derive(Debug)]
pub struct RaceClient {
    connection: Connection,
    pub server: String,
    shared_server: SharedServer,
    pub player: Option<PlayerId>,
    pub room: Option<RoomView>,
    race: Option<LiveRace>,
    intent: Option<Intent>,
    reported: Option<(Instant, Progress)>,
    /**
     * Whether a room request is still unanswered. The server answers each
     * with the new room view or an error, and only then can the next one
     * be decided: a held key would otherwise flood the server, and
     * toggling twice would ask for the same state twice.
     */
    awaiting_answer: bool,
    last_press: Option<(RoomRequest, Instant)>,
}

impl RaceClient {
    pub fn connect(server: String, username: Username, intent: Intent) -> Self {
        let connection = Connection::open(server.clone(), username);
        Self::over(connection, server, intent)
    }

    pub(super) fn over(connection: Connection, server: String, intent: Intent) -> Self {
        Self {
            connection,
            shared_server: SharedServer::of(&server, network::lan_address),
            server,
            player: None,
            room: None,
            race: None,
            intent: Some(intent),
            reported: None,
            awaiting_answer: false,
            last_press: None,
        }
    }

    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    /**
     * Leaves the room and hands back the connection, which still has the
     * goodbye to send.
     */
    pub fn leave(self) -> Connection {
        self.connection.send(ClientMessage::LeaveRoom);
        self.connection
    }

    pub fn stage(&self, now: Instant) -> Stage {
        let Some(room) = &self.room else {
            return if self.player.is_some() {
                Stage::Joining
            } else {
                Stage::Connecting
            };
        };
        match room.phase {
            Phase::Lobby => Stage::Lobby,
            Phase::Countdown => Stage::Countdown(self.countdown_left(now)),
            Phase::Racing => Stage::Racing,
            Phase::Finished => Stage::Finished,
        }
    }

    fn countdown_left(&self, now: Instant) -> Duration {
        self.race.as_ref().map_or(Duration::ZERO, |race| {
            race.countdown_ends.saturating_duration_since(now)
        })
    }

    pub fn me(&self) -> Option<&PlayerView> {
        self.room.as_ref()?.player(self.player?)
    }

    pub fn is_host(&self) -> bool {
        self.player
            .zip(self.room.as_ref())
            .is_some_and(|(player, room)| room.is_host(player))
    }

    pub fn phase(&self) -> Option<Phase> {
        self.room.as_ref().map(|room| room.phase)
    }

    /// Whether the room counts down or races, whoever is still typing.
    pub fn is_live(&self) -> bool {
        matches!(self.phase(), Some(Phase::Countdown | Phase::Racing))
    }

    /// Whether the room shows a race: its countdown, the race or its results.
    pub fn shows_a_race(&self) -> bool {
        self.is_live() || self.phase() == Some(Phase::Finished)
    }

    pub fn accepts_typing(&self) -> bool {
        self.phase() == Some(Phase::Racing)
            && self
                .race
                .as_ref()
                .is_some_and(|race| !race.session.is_finished())
    }

    /**
     * Whether the player is in a race they have not finished: the keyboard
     * belongs to the race text, already during the countdown so that early
     * keys are not taken as commands.
     */
    pub fn is_player_racing(&self) -> bool {
        self.phase() == Some(Phase::Countdown) || self.accepts_typing()
    }

    /// The race text, from its countdown to the results.
    pub fn session_view(&self) -> Option<SessionView<'_>> {
        self.race.as_ref().map(|race| SessionView {
            session: &race.session,
            syntax: self.syntax(),
            attribution: None,
            stopped_at: race.ended_at,
            ink: Some(&race.ink),
            disguise: None,
        })
    }

    /**
     * File name shown in the editor for the race text: prose is in a file
     * named after the room, unless `disguise` makes it another kind of file.
     */
    pub fn title(&self, disguise: Disguise<'_>) -> String {
        match self.room.as_ref().map(|room| room.text) {
            Some(TextSource::Code { language }) => code_file_name(language),
            Some(_) if disguise.look != Look::Notes => disguised_name(disguise),
            Some(_) => format!("{}.md", self.room_label()),
            None => "race.md".to_owned(),
        }
    }

    pub fn room_label(&self) -> String {
        self.room
            .as_ref()
            .map_or_else(|| "race".to_owned(), |room| room.code.to_string())
    }

    pub fn syntax(&self) -> Option<CodeLanguage> {
        match self.room.as_ref()?.text {
            TextSource::Code { language } => Some(language),
            _ => None,
        }
    }

    /// What the room races on, once in a room.
    pub fn plan(&self) -> Option<Plan> {
        self.room.as_ref().map(|room| Plan::Text(room.text))
    }

    pub fn text_label(&self) -> String {
        self.plan().map(|plan| plan.label()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
