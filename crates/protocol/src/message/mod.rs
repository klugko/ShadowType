/*!
 * Messages exchanged over the WebSocket, encoded as tagged JSON objects.
 *
 * A connection starts with [`ClientMessage::Hello`], answered by
 * [`ServerMessage::Welcome`] or an [`ErrorCode::IncompatibleVersion`] error.
 * Only peers of the same [`PROTOCOL_VERSION`] talk to each other, so a
 * message never needs to accept the shape of an older version.
 */

#[cfg(test)]
mod tests;

use code_racer_engine::{Tally, TextSource};
use serde::{Deserialize, Serialize};

use crate::{
    ids::{PlayerId, RoomCode, Username},
    room::RoomView,
};

/**
 * Incremented whenever a change breaks compatibility with older peers. The
 * server refuses a `Hello` announcing any other version. Version 3 left
 * auto-filled indentation out of keystrokes and made
 * [`Progress::indentation`] required.
 */
pub const PROTOCOL_VERSION: u16 = 3;

/**
 * Largest WebSocket message either side accepts. Every valid message fits:
 * usernames are bounded in bytes and rooms in players for that purpose.
 */
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        version: u16,
        username: Username,
    },
    CreateRoom {
        text: TextSource,
    },
    JoinRoom {
        code: RoomCode,
    },
    LeaveRoom,
    SetReady {
        ready: bool,
    },
    StartRace,
    /**
     * Sent periodically while racing. The server derives speed, accuracy and
     * finishing time from it.
     */
    Progress(Progress),
    ReturnToLobby,
}

impl ClientMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub typed: u32,
    pub correct: u32,
    /**
     * Characters of `correct` that auto-indentation filled in. Required
     * since protocol version 3, which stopped counting them as keystrokes.
     */
    pub indentation: u32,
    pub keystrokes: u32,
    pub errors: u32,
}

impl Progress {
    pub fn tally(&self) -> Tally {
        Tally {
            typed: count(self.typed),
            correct: count(self.correct),
            indentation: count(self.indentation),
            keystrokes: count(self.keystrokes),
            errors: count(self.errors),
        }
    }
}

pub(crate) fn count(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

impl From<Tally> for Progress {
    fn from(tally: Tally) -> Self {
        let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        Self {
            typed: count(tally.typed),
            correct: count(tally.correct),
            indentation: count(tally.indentation),
            keystrokes: count(tally.keystrokes),
            errors: count(tally.errors),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome { version: u16, player_id: PlayerId },
    Room(RoomView),
    Countdown { text: String, duration_ms: u32 },
    Error(ServerError),
}

impl ServerMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Error(ServerError::new(code, message))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerError {
    pub code: ErrorCode,
    pub message: String,
}

impl ServerError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ServerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    IncompatibleVersion,
    HandshakeRequired,
    InvalidMessage,
    RateLimited,
    ServerFull,
    RoomNotFound,
    RoomFull,
    RaceInProgress,
    NotInRoom,
    NotHost,
    PlayersNotReady,
    RaceNotRunning,
    InvalidProgress,
    InvalidSettings,
    #[serde(other)]
    Unknown,
}
