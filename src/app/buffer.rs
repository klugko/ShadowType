use super::{Activity, App};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buffer {
    Practice,
    Race,
    History,
    Settings,
    Help,
    /// The running solo session or room.
    Session,
}

impl Buffer {
    pub const FILES: [Self; 5] = [
        Self::Practice,
        Self::Race,
        Self::History,
        Self::Settings,
        Self::Help,
    ];

    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Practice => "practice.toml",
            Self::Race => "race.toml",
            Self::History => "history.log",
            Self::Settings => "config.toml",
            Self::Help => "help.md",
            Self::Session => "session",
        }
    }
}

impl App {
    /// Buffers listed in the explorer, in order.
    pub fn entries(&self) -> Vec<Buffer> {
        let mut entries = Buffer::FILES.to_vec();
        if self.activity.is_some() {
            entries.push(Buffer::Session);
        }
        entries
    }

    pub fn buffer_name(&self, buffer: Buffer) -> String {
        match (buffer, &self.activity) {
            (Buffer::Session, Some(Activity::Solo(run))) => run.plan.title(self.disguise()),
            (Buffer::Session, Some(Activity::Race(client))) => client.title(self.disguise()),
            _ => buffer.file_name().to_owned(),
        }
    }

    /**
     * Shows `buffer` in the editor. A field being typed in another buffer
     * is cancelled: hidden, it would still take every key.
     */
    pub(super) fn open(&mut self, buffer: Buffer) {
        if buffer != self.buffer {
            self.cancel_edit();
        }
        self.buffer = buffer;
        self.focus_editor();
    }
}
