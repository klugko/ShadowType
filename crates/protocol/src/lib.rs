use rand::Rng;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

pub const VERSION: u16 = 1;
pub const MAX_MESSAGE: usize = 8192;
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn room_code() -> String {
    let mut rng = rand::rng();
    (0..6).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect()
}
pub fn valid_code(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|b| ALPHABET.contains(&b))
}
pub fn valid_username(name: &str) -> bool {
    let n = name.graphemes(true).count();
    (1..=24).contains(&n) && name.trim() == name && !name.chars().any(char::is_control)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Create { version: u16, username: String, language: String },
    Join { version: u16, code: String, username: String },
    Ready { ready: bool },
    Start,
    Progress { position: usize, errors: usize, correct: usize, attempts: usize },
    Leave,
    Again,
    Ping { client_ms: u64 },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub ready: bool,
    pub connected: bool,
    pub position: usize,
    pub errors: usize,
    pub wpm: f64,
    pub accuracy: f64,
    pub finished_ms: Option<u64>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase { Waiting, Countdown, Racing, Finished }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub code: String,
    pub host: String,
    pub players: Vec<Player>,
    pub phase: Phase,
    pub text: String,
    pub language: String,
    pub start_ms: Option<u64>,
    pub server_ms: u64,
}
impl Snapshot {
    pub fn ranking(&self) -> Vec<&Player> {
        let mut players: Vec<_> = self.players.iter().collect();
        players.sort_by(|a,b| match (a.finished_ms,b.finished_ms) {
            (Some(a),Some(b)) => a.cmp(&b),
            (Some(_),None) => std::cmp::Ordering::Less,
            (None,Some(_)) => std::cmp::Ordering::Greater,
            (None,None) => b.position.cmp(&a.position).then_with(|| a.name.cmp(&b.name)),
        });
        players
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome { version: u16, player_id: String },
    Room(Snapshot),
    Error { message: String },
    Pong { client_ms: u64, server_ms: u64 },
}
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
pub fn text(language: &str) -> Option<&'static str> {
    Some(match language {
        "english" => include_str!("../../../texts/english.txt").trim_end(),
        "french" => include_str!("../../../texts/french.txt").trim_end(),
        "rust" => include_str!("../../../texts/code/rust.txt").trim_end(),
        "python" => include_str!("../../../texts/code/python.txt").trim_end(),
        "javascript" | "typescript" => include_str!("../../../texts/code/javascript.txt").trim_end(),
        "sql" => include_str!("../../../texts/code/sql.txt").trim_end(),
        _ => return None,
    })
}
pub fn grapheme_count(text: &str) -> usize { text.graphemes(true).count() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn codes_and_names() {
        for _ in 0..1000 { assert!(valid_code(&room_code())); }
        assert!(!valid_code("ABC01O"));
        assert!(valid_username("Élodie"));
        assert!(!valid_username("\nBob"));
        assert!(!valid_username(&"a".repeat(25)));
    }
    #[test] fn protocol_roundtrip() {
        let message = ClientMessage::Join { version: VERSION, code: "ABC234".into(), username: "Jean".into() };
        assert_eq!(serde_json::from_str::<ClientMessage>(&serde_json::to_string(&message).expect("encode")).expect("decode"),message);
        assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"Unknown"}"#).is_err());
    }
}
