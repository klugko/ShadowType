//! The messages of the command line, shown one at a time.

use std::{collections::VecDeque, fmt::Display};

use super::command::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    /// Exactly what is shown, errors with their Vim-like prefix.
    pub text: String,
}

impl Message {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            kind: MessageKind::Info,
            text: text.into(),
        }
    }

    /// An error, prefixed with `E:` like the Vim errors that have no number.
    pub fn error(text: impl Display) -> Self {
        Self {
            kind: MessageKind::Error,
            text: format!("E: {text}"),
        }
    }

    /// The error of a `:` command, which carries its Vim error number.
    pub fn command_error(error: &CommandError) -> Self {
        Self {
            kind: MessageKind::Error,
            text: error.to_string(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.kind == MessageKind::Error
    }
}

/// The messages waiting for the command line, the first one on screen.
///
/// An information is news that a later one makes stale: a new message
/// replaces it once it has been on screen, or when the same event left it,
/// so that a failure to save replaces the report of the change. An error
/// waits until a key dismisses it. Otherwise messages wait their turn, so
/// that none is lost before it could be read: the warnings of the start-up,
/// or one that arrives while the `:` line hides the messages.
#[derive(Debug, Default)]
pub struct Messages {
    waiting: VecDeque<Message>,
    first_seen: bool,
    /// Whether the last message was left by the event being handled.
    last_is_new: bool,
}

impl Messages {
    /// Most messages kept waiting: the oldest go first past it.
    const CAPACITY: usize = 8;

    pub fn first(&self) -> Option<&Message> {
        self.waiting.front()
    }

    pub fn waiting_behind(&self) -> usize {
        self.waiting.len().saturating_sub(1)
    }

    /// Starts handling an event, such as a key or a network message.
    /// `first_shown` tells whether the screen drawn since the previous event
    /// showed the first message.
    pub fn next_event(&mut self, first_shown: bool) {
        self.first_seen |= first_shown && !self.waiting.is_empty();
        self.last_is_new = false;
    }

    pub fn push(&mut self, message: Message) {
        if self.last_is_stale() {
            self.waiting.pop_back();
            if self.waiting.is_empty() {
                self.first_seen = false;
            }
        }
        if self.waiting.len() == Self::CAPACITY {
            self.dismiss();
        }
        self.waiting.push_back(message);
        self.last_is_new = true;
    }

    pub fn dismiss(&mut self) {
        self.waiting.pop_front();
        self.first_seen = false;
        if self.waiting.is_empty() {
            self.last_is_new = false;
        }
    }

    /// Whether the last message is an information a new message replaces:
    /// one the same event left, one already seen, or one waiting behind
    /// another, which a newer one supersedes before it is ever shown.
    fn last_is_stale(&self) -> bool {
        let Some(last) = self.waiting.back() else {
            return false;
        };
        let behind_another = self.waiting.len() > 1;
        !last.is_error() && (self.last_is_new || self.first_seen || behind_another)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(messages: &Messages) -> Vec<&str> {
        messages
            .waiting
            .iter()
            .map(|message| message.text.as_str())
            .collect()
    }

    #[test]
    fn a_failure_replaces_the_report_of_the_same_event() {
        let mut messages = Messages::default();
        messages.push(Message::info("language set to french"));
        messages.push(Message::error("cannot save"));
        assert_eq!(texts(&messages), ["E: cannot save"]);
    }

    #[test]
    fn an_error_is_never_replaced() {
        let mut messages = Messages::default();
        messages.push(Message::error("cannot save"));
        messages.push(Message::info("hello Ada"));
        messages.next_event(true);
        messages.push(Message::error("room closed"));
        assert_eq!(texts(&messages), ["E: cannot save", "E: room closed"]);
    }

    #[test]
    fn news_seen_on_screen_is_replaced() {
        let mut messages = Messages::default();
        messages.push(Message::info("connecting"));
        messages.next_event(true);
        messages.push(Message::info("in room FK72AD"));
        assert_eq!(texts(&messages), ["in room FK72AD"]);
    }

    #[test]
    fn news_never_shown_waits_its_turn() {
        let mut messages = Messages::default();
        messages.push(Message::info("in room FK72AD"));
        messages.next_event(false);
        messages.push(Message::info("language set to french"));
        assert_eq!(
            texts(&messages),
            ["in room FK72AD", "language set to french"]
        );
        messages.next_event(false);
        messages.push(Message::info("code language set to rust"));
        assert_eq!(
            texts(&messages),
            ["in room FK72AD", "code language set to rust"],
            "news waiting behind is superseded"
        );
    }

    #[test]
    fn dismissing_shows_the_next_message() {
        let mut messages = Messages::default();
        for warning in ["config.toml was invalid", "history.json was invalid"] {
            messages.next_event(false);
            messages.push(Message::error(warning));
        }
        messages.next_event(false);
        messages.push(Message::info("welcome"));
        assert_eq!(messages.waiting_behind(), 2);
        messages.next_event(true);
        messages.dismiss();
        assert_eq!(
            messages.first(),
            Some(&Message::error("history.json was invalid"))
        );
        messages.next_event(true);
        messages.dismiss();
        messages.next_event(false);
        messages.push(Message::info("hello Ada"));
        assert_eq!(texts(&messages), ["welcome", "hello Ada"]);
    }

    #[test]
    fn the_oldest_message_goes_past_the_capacity() {
        let mut messages = Messages::default();
        for number in 0..=Messages::CAPACITY {
            messages.next_event(false);
            messages.push(Message::error(number));
        }
        assert_eq!(messages.waiting.len(), Messages::CAPACITY);
        assert_eq!(messages.first(), Some(&Message::error(1)));
    }
}
