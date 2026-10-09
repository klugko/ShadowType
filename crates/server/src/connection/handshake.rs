use code_racer_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerError, Username};
use futures_util::StreamExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, Utf8Bytes},
};

use super::HANDSHAKE_TIMEOUT;

pub(super) async fn greet<S>(socket: &mut WebSocketStream<S>) -> Result<Username, ServerError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let first = timeout(HANDSHAKE_TIMEOUT, next_text(socket))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ServerError::new(ErrorCode::HandshakeRequired, "no hello received"))?;
    check_hello(&first)
}

/**
 * Accepts a `Hello` in the current protocol version.
 *
 * The version is looked up leniently first, so that clients of another
 * version learn why they are refused even when the rest of their first
 * message does not parse in this version.
 */
fn check_hello(text: &str) -> Result<Username, ServerError> {
    let current = u64::from(PROTOCOL_VERSION);
    if let Some(version) = announced_version(text).filter(|&version| version != current) {
        return Err(ServerError::new(
            ErrorCode::IncompatibleVersion,
            format!("server speaks protocol v{PROTOCOL_VERSION}, client v{version}"),
        ));
    }
    match ClientMessage::from_json(text) {
        Ok(ClientMessage::Hello { username, .. }) => Ok(username),
        Ok(_) => Err(ServerError::new(
            ErrorCode::HandshakeRequired,
            "say hello before anything else",
        )),
        Err(error) => Err(ServerError::new(
            ErrorCode::HandshakeRequired,
            format!("expected a hello message: {error}"),
        )),
    }
}

fn announced_version(text: &str) -> Option<u64> {
    let message: serde_json::Value = serde_json::from_str(text).ok()?;
    message.get("data")?.get("version")?.as_u64()
}

/**
 * The next text message, skipping control frames. `None` once the client
 * closes, errs or sends something that cannot be a hello.
 */
async fn next_text<S>(socket: &mut WebSocketStream<S>) -> Option<Utf8Bytes>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(frame) = socket.next().await {
        match frame.ok()? {
            Message::Text(text) => return Some(text),
            Message::Binary(_) | Message::Close(_) => return None,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use code_racer_engine::{CodeLanguage, TextSource};
    use code_racer_protocol::MAX_MESSAGE_BYTES;
    use futures_util::SinkExt;
    use tokio::{
        io::{DuplexStream, duplex},
        time,
    };
    use tokio_tungstenite::tungstenite::{Bytes, protocol::Role};

    use super::*;

    async fn socket_pair() -> (WebSocketStream<DuplexStream>, WebSocketStream<DuplexStream>) {
        let (server, client) = duplex(MAX_MESSAGE_BYTES);
        (
            WebSocketStream::from_raw_socket(server, Role::Server, None).await,
            WebSocketStream::from_raw_socket(client, Role::Client, None).await,
        )
    }

    fn hello(version: u16) -> String {
        ClientMessage::Hello {
            version,
            username: "Ada".parse().expect("valid name"),
        }
        .to_json()
    }

    fn refusal(text: &str) -> Option<(ErrorCode, String)> {
        check_hello(text)
            .err()
            .map(|error| (error.code, error.message))
    }

    #[test]
    fn a_hello_in_the_current_version_gives_the_username() {
        assert_eq!(
            check_hello(&hello(PROTOCOL_VERSION)).map(String::from),
            Ok("Ada".to_owned())
        );
    }

    #[test]
    fn other_versions_are_named_even_when_their_hello_does_not_parse() {
        let newer = PROTOCOL_VERSION + 1;
        let reshaped = format!(r#"{{"type":"hello","data":{{"version":{newer},"user":{{}}}}}}"#);
        let expected = format!("server speaks protocol v{PROTOCOL_VERSION}, client v{newer}");
        assert_eq!(
            refusal(&reshaped),
            Some((ErrorCode::IncompatibleVersion, expected))
        );
        let first_release = r#"{"type":"Create","data":{"version":1,"username":"Ada"}}"#;
        assert_eq!(
            refusal(first_release).map(|(code, _)| code),
            Some(ErrorCode::IncompatibleVersion)
        );
    }

    #[test]
    fn anything_but_a_valid_hello_requires_the_handshake() {
        let create = ClientMessage::CreateRoom {
            text: TextSource::Code {
                language: CodeLanguage::Rust,
            },
        }
        .to_json();
        let nameless = hello(PROTOCOL_VERSION).replace("Ada", "");
        for text in [
            create.as_str(),
            &nameless,
            "not json",
            r#"{"data":{"version":"2"}}"#,
        ] {
            assert_eq!(
                refusal(text).map(|(code, _)| code),
                Some(ErrorCode::HandshakeRequired),
                "{text}"
            );
        }
    }

    #[tokio::test]
    async fn the_hello_may_follow_control_frames() {
        let (mut server, mut client) = socket_pair().await;
        client
            .send(Message::Ping(Bytes::new()))
            .await
            .expect("ping");
        client
            .send(Message::text(hello(PROTOCOL_VERSION)))
            .await
            .expect("hello");
        assert_eq!(
            greet(&mut server).await.map(String::from),
            Ok("Ada".to_owned())
        );
    }

    #[tokio::test]
    async fn a_binary_first_message_is_not_a_hello() {
        let (mut server, mut client) = socket_pair().await;
        client
            .send(Message::binary(vec![1, 2, 3]))
            .await
            .expect("binary");
        assert_eq!(
            greet(&mut server).await.map_err(|error| error.code),
            Err(ErrorCode::HandshakeRequired)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_silent_client_is_refused_once_the_handshake_times_out() {
        let (mut server, _client) = socket_pair().await;
        let started = time::Instant::now();
        let refused = greet(&mut server).await;
        assert_eq!(
            refused.map_err(|error| error.code),
            Err(ErrorCode::HandshakeRequired)
        );
        assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
    }
}
