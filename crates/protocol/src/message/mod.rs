/*!
 * Messages exchanged over the WebSocket, encoded as tagged JSON objects.
 *
 * A connection starts with [`ClientMessage::Hello`], answered by
 * [`ServerMessage::Welcome`] or an [`ErrorCode::IncompatibleVersion`] error.
 * Only peers of the same [`PROTOCOL_VERSION`] talk to each other, so a
 * message never needs to accept the shape of an older version.
 */

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerError {
    pub code: ErrorCode,
    pub message: String,
}

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

impl ClientMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
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

#[cfg(test)]
mod tests {
    use code_racer_engine::{CodeLanguage, Language, WordOptions};

    use super::*;
    use crate::room::{Phase, PlayerProgress, PlayerView};

    fn round_trip_client(message: ClientMessage) {
        assert_eq!(
            ClientMessage::from_json(&message.to_json()).ok(),
            Some(message)
        );
    }

    fn round_trip_server(message: ServerMessage) {
        assert_eq!(
            ServerMessage::from_json(&message.to_json()).ok(),
            Some(message)
        );
    }

    #[test]
    fn every_client_message_round_trips() {
        let messages = [
            ClientMessage::Hello {
                version: PROTOCOL_VERSION,
                username: "Jean".parse().expect("name"),
            },
            ClientMessage::CreateRoom {
                text: TextSource::Words {
                    language: Language::French,
                    count: 30,
                    options: WordOptions {
                        punctuation: true,
                        numbers: false,
                    },
                },
            },
            ClientMessage::CreateRoom {
                text: TextSource::Code {
                    language: CodeLanguage::Python,
                },
            },
            ClientMessage::JoinRoom {
                code: "FK72AD".parse().expect("code"),
            },
            ClientMessage::LeaveRoom,
            ClientMessage::SetReady { ready: true },
            ClientMessage::StartRace,
            ClientMessage::Progress(Progress {
                typed: 12,
                correct: 11,
                indentation: 4,
                keystrokes: 10,
                errors: 2,
            }),
            ClientMessage::ReturnToLobby,
        ];
        for message in messages {
            round_trip_client(message);
        }
    }

    #[test]
    fn every_server_message_round_trips() {
        let room = RoomView {
            code: "FK72AD".parse().expect("code"),
            host: PlayerId(7),
            text: TextSource::Quote {
                language: Language::English,
            },
            text_length: 120,
            phase: Phase::Racing,
            max_players: 8,
            players: vec![PlayerView {
                id: PlayerId(7),
                name: "Alice".parse().expect("name"),
                ready: true,
                connected: true,
                progress: PlayerProgress {
                    typed: 60,
                    correct: 58,
                    errors: 3,
                    wpm: 81.5,
                    accuracy: 96.2,
                    finish_ms: None,
                },
            }],
        };
        round_trip_server(ServerMessage::Welcome {
            version: PROTOCOL_VERSION,
            player_id: PlayerId(7),
        });
        round_trip_server(ServerMessage::Room(room));
        round_trip_server(ServerMessage::Countdown {
            text: "fn main() {}".to_owned(),
            duration_ms: 3_000,
        });
        round_trip_server(ServerMessage::error(
            ErrorCode::RoomNotFound,
            "room FK72AD not found",
        ));
    }

    #[test]
    fn wire_format_is_tagged_snake_case() {
        let json = ClientMessage::SetReady { ready: true }.to_json();
        assert_eq!(json, r#"{"type":"set_ready","data":{"ready":true}}"#);
        assert_eq!(
            ClientMessage::StartRace.to_json(),
            r#"{"type":"start_race"}"#
        );
    }

    #[test]
    fn invalid_payloads_are_rejected() {
        for json in [
            r#"{"type":"unknown"}"#,
            r#"{"type":"join_room","data":{"code":"0000OO"}}"#,
            r#"{"type":"hello","data":{"version":3,"username":""}}"#,
            r#"{"type":"progress","data":{"typed":-1,"correct":0,"indentation":0,"keystrokes":0,"errors":0}}"#,
            r#"{"type":"progress","data":{"typed":9,"correct":8,"keystrokes":9,"errors":1}}"#,
            "not json",
        ] {
            assert!(ClientMessage::from_json(json).is_err(), "{json}");
        }
    }

    #[test]
    fn progress_is_scored_by_the_engine_rules() {
        let progress = Progress {
            typed: 30,
            correct: 28,
            indentation: 8,
            keystrokes: 25,
            errors: 5,
        };
        let tally = progress.tally();
        assert_eq!(tally.correctly_typed(), 20);
        assert_eq!(tally.accuracy(), 80.0);
    }

    #[test]
    fn progress_reports_the_session_tally() {
        let tally = Tally {
            typed: 12,
            correct: 11,
            indentation: 4,
            keystrokes: 10,
            errors: 2,
        };
        let progress = Progress::from(tally);
        assert_eq!(
            progress,
            Progress {
                typed: 12,
                correct: 11,
                indentation: 4,
                keystrokes: 10,
                errors: 2,
            }
        );
        assert_eq!(progress.tally(), tally);
    }

    #[test]
    fn counters_beyond_the_wire_range_are_clamped() {
        let tally = Tally {
            keystrokes: usize::MAX,
            ..Tally::default()
        };
        assert_eq!(Progress::from(tally).keystrokes, u32::MAX);
    }

    #[test]
    fn unknown_error_codes_are_tolerated() {
        let json = r#"{"type":"error","data":{"code":"from_the_future","message":"hi"}}"#;
        let message = ServerMessage::from_json(json).expect("parse");
        assert_eq!(message, ServerMessage::error(ErrorCode::Unknown, "hi"));
    }
}
