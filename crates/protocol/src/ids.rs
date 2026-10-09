use std::{fmt, str::FromStr};

use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub u64);

impl fmt::Display for PlayerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{}", self.0)
    }
}

/**
 * Short code used to invite people into a room, such as `FK72AD`.
 *
 * Codes use uppercase letters and digits, leaving out the look-alikes
 * `0`, `O`, `1` and `I` so they can be read aloud or copied by hand.
 */
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoomCode(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidRoomCode {
    #[error("room codes have {} characters", RoomCode::LENGTH)]
    Length,
    #[error("`{0}` cannot appear in a room code")]
    Character(char),
}

impl RoomCode {
    pub const LENGTH: usize = 6;
    pub const ALPHABET: &'static str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        let alphabet = Self::ALPHABET.as_bytes();
        let code = (0..Self::LENGTH)
            .map(|_| char::from(alphabet[rng.random_range(0..alphabet.len())]))
            .collect();
        Self(code)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RoomCode {
    type Err = InvalidRoomCode;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let code = input.trim().to_uppercase();
        if let Some(invalid) = code.chars().find(|c| !Self::ALPHABET.contains(*c)) {
            return Err(InvalidRoomCode::Character(invalid));
        }
        if code.chars().count() != Self::LENGTH {
            return Err(InvalidRoomCode::Length);
        }
        Ok(Self(code))
    }
}

impl TryFrom<String> for RoomCode {
    type Error = InvalidRoomCode;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<RoomCode> for String {
    fn from(code: RoomCode) -> Self {
        code.0
    }
}

impl fmt::Display for RoomCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Username(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidUsername {
    #[error("username cannot be empty")]
    Empty,
    #[error("username is limited to {} characters", Username::MAX_LENGTH)]
    TooLong,
    #[error("username is limited to {} bytes", Username::MAX_BYTES)]
    TooManyBytes,
    #[error("username cannot contain control characters or line breaks")]
    ControlCharacter,
    #[error("username cannot contain invisible characters")]
    InvisibleCharacter,
}

impl Username {
    pub const MAX_LENGTH: usize = 24;
    /**
     * Longest name in bytes of UTF-8. One character can stack any number of
     * combining marks, so the character limit alone does not bound a name,
     * while a room view listing [`MAX_ROOM_PLAYERS`] names must fit in
     * [`MAX_MESSAGE_BYTES`].
     *
     * [`MAX_ROOM_PLAYERS`]: crate::MAX_ROOM_PLAYERS
     * [`MAX_MESSAGE_BYTES`]: crate::MAX_MESSAGE_BYTES
     */
    pub const MAX_BYTES: usize = 128;

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Username {
    type Err = InvalidUsername;

    /**
     * Surrounding spaces are ignored.
     *
     * Characters that take no room on screen are refused: zero-width spaces,
     * bidirectional controls and lone combining marks would make a name blank
     * or scramble the line it is shown on. Joiners inside a visible
     * character, as in emoji, are fine.
     */
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let name = input.trim();
        if name.chars().any(is_control_or_line_break) {
            return Err(InvalidUsername::ControlCharacter);
        }
        if name.graphemes(true).any(|grapheme| grapheme.width() == 0) {
            return Err(InvalidUsername::InvisibleCharacter);
        }
        match name.graphemes(true).count() {
            0 => Err(InvalidUsername::Empty),
            length if length > Self::MAX_LENGTH => Err(InvalidUsername::TooLong),
            _ if name.len() > Self::MAX_BYTES => Err(InvalidUsername::TooManyBytes),
            _ => Ok(Self(name.to_owned())),
        }
    }
}

fn is_control_or_line_break(ch: char) -> bool {
    ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}')
}

impl TryFrom<String> for Username {
    type Error = InvalidUsername;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Username> for String {
    fn from(name: Username) -> Self {
        name.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    #[test]
    fn random_codes_are_valid() {
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..1_000 {
            let code = RoomCode::random(&mut rng);
            assert_eq!(code.as_str().parse::<RoomCode>(), Ok(code));
        }
    }

    #[test]
    fn codes_are_normalised_to_uppercase() {
        assert_eq!(
            " fk72ad ".parse::<RoomCode>().map(String::from),
            Ok("FK72AD".to_owned())
        );
    }

    #[test]
    fn ambiguous_characters_are_rejected() {
        for ambiguous in ["FK72A0", "FK72AO", "FK72A1", "FK72AI"] {
            assert!(matches!(
                ambiguous.parse::<RoomCode>(),
                Err(InvalidRoomCode::Character(_))
            ));
        }
        assert_eq!("FK72A".parse::<RoomCode>(), Err(InvalidRoomCode::Length));
        assert_eq!("FK72ADX".parse::<RoomCode>(), Err(InvalidRoomCode::Length));
    }

    #[test]
    fn usernames_are_bounded_in_bytes_too() {
        let coder = "👩‍💻";
        assert!(coder.repeat(11).parse::<Username>().is_ok());
        assert_eq!(
            coder.repeat(12).parse::<Username>(),
            Err(InvalidUsername::TooManyBytes)
        );
        let stacked = format!("a{}", "\u{301}".repeat(8_150));
        assert_eq!(
            stacked.parse::<Username>(),
            Err(InvalidUsername::TooManyBytes)
        );
    }

    #[test]
    fn usernames_must_be_visible() {
        for name in [
            "\u{200b}",
            "a\u{202e}b",
            "\u{301}",
            "Bob\u{2066}",
            "\u{feff}Ann",
        ] {
            assert_eq!(
                name.parse::<Username>(),
                Err(InvalidUsername::InvisibleCharacter),
                "{name:?}"
            );
        }
        assert_eq!(
            "a\u{2028}b".parse::<Username>(),
            Err(InvalidUsername::ControlCharacter)
        );
        for name in ["Zoe\u{301}", "👩‍💻 dev", "می‌خواهم"] {
            assert!(name.parse::<Username>().is_ok(), "{name:?}");
        }
    }

    #[test]
    fn usernames_count_graphemes() {
        assert!("Élodie".parse::<Username>().is_ok());
        assert!("e\u{301}".repeat(24).parse::<Username>().is_ok());
        assert_eq!(
            "a".repeat(25).parse::<Username>(),
            Err(InvalidUsername::TooLong)
        );
        assert_eq!("   ".parse::<Username>(), Err(InvalidUsername::Empty));
        assert_eq!(
            "Bob\nBob".parse::<Username>(),
            Err(InvalidUsername::ControlCharacter)
        );
        assert_eq!(
            "  Jean ".parse::<Username>().map(String::from),
            Ok("Jean".to_owned())
        );
    }

    #[test]
    fn deserialisation_validates() {
        assert!(serde_json::from_str::<RoomCode>("\"ABC234\"").is_ok());
        assert!(serde_json::from_str::<RoomCode>("\"ABC10O\"").is_err());
        assert!(serde_json::from_str::<Username>("\"\"").is_err());
    }
}
