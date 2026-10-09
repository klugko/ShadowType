use code_racer_engine::{
    CodeLanguage, Language, TextSource, WordOptions,
    corpus::{quotes, snippets},
};
use code_racer_protocol::{
    ClientMessage, MAX_MESSAGE_BYTES, MAX_ROOM_PLAYERS, Phase, PlayerId, PlayerProgress,
    PlayerView, Progress, RoomView, ServerMessage, Username,
};

/**
 * The valid name with the longest JSON: as many bytes as allowed, and a
 * quote, which JSON escapes, at the start of each of its characters.
 */
fn widest_name() -> Username {
    let marks = (Username::MAX_BYTES - Username::MAX_LENGTH) / '\u{301}'.len_utf8();
    let name = format!(
        "\"{}{}",
        "\u{301}".repeat(marks),
        "\"".repeat(Username::MAX_LENGTH - 1)
    );
    assert_eq!(name.len(), Username::MAX_BYTES);
    name.parse().expect("the widest valid name")
}

fn widest_source() -> TextSource {
    TextSource::Words {
        language: Language::French,
        count: u16::MAX,
        options: WordOptions {
            punctuation: true,
            numbers: true,
        },
    }
}

fn crowded_room() -> RoomView {
    let player = |id| PlayerView {
        id: PlayerId(id),
        name: widest_name(),
        ready: false,
        connected: false,
        progress: PlayerProgress {
            typed: u32::MAX,
            correct: u32::MAX,
            errors: u32::MAX,
            wpm: f64::MIN,
            accuracy: f64::MIN,
            finish_ms: Some(u64::MAX),
        },
    };
    RoomView {
        code: "WWWWWW".parse().expect("valid code"),
        host: PlayerId(u64::MAX),
        text: widest_source(),
        text_length: u32::MAX,
        phase: Phase::Countdown,
        max_players: MAX_ROOM_PLAYERS,
        players: (0..u64::from(MAX_ROOM_PLAYERS))
            .map(|index| player(u64::MAX - index))
            .collect(),
    }
}

fn assert_fits(json: &str) {
    assert!(
        json.len() <= MAX_MESSAGE_BYTES,
        "{} bytes: {}…",
        json.len(),
        json.chars().take(80).collect::<String>()
    );
}

#[test]
fn a_full_room_of_the_widest_names_fits() {
    assert_fits(&ServerMessage::Room(crowded_room()).to_json());
}

#[test]
fn every_race_text_fits_in_its_countdown() {
    let countdown = |text: String| ServerMessage::Countdown {
        text,
        duration_ms: u32::MAX,
    };
    for language in Language::ALL {
        for quote in quotes(language) {
            assert_fits(&countdown(quote.text.clone()).to_json());
        }
        for seed in 0..50 {
            let words = TextSource::Words {
                language,
                count: *code_racer_protocol::RACE_WORD_COUNTS.end(),
                options: WordOptions {
                    punctuation: true,
                    numbers: true,
                },
            };
            assert_fits(&countdown(words.generate(seed).text).to_json());
        }
    }
    for language in CodeLanguage::ALL {
        for snippet in snippets(language) {
            assert_fits(&countdown(snippet.clone()).to_json());
        }
    }
}

#[test]
fn every_client_message_fits() {
    let messages = [
        ClientMessage::Hello {
            version: u16::MAX,
            username: widest_name(),
        },
        ClientMessage::CreateRoom {
            text: widest_source(),
        },
        ClientMessage::JoinRoom {
            code: "WWWWWW".parse().expect("valid code"),
        },
        ClientMessage::Progress(Progress {
            typed: u32::MAX,
            correct: u32::MAX,
            indentation: u32::MAX,
            keystrokes: u32::MAX,
            errors: u32::MAX,
        }),
    ];
    for message in messages {
        assert_fits(&message.to_json());
    }
}
