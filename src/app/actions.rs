//! Things the user can do, whichever key or command triggered them.

use std::time::Instant;

use code_racer_protocol::RoomCode;

use super::{
    Activity, App, Buffer, Focus, TextField,
    command::{Command, Page},
    keys::SessionEdit,
    practice::{CustomText, Plan, SoloRun},
    race::{Intent, RaceClient},
};
use crate::{
    cli::Launch,
    config::{Mode, Practice},
    network,
};

impl App {
    pub fn warn(&mut self, warning: String) {
        self.error(warning);
    }

    pub(super) fn launch(&mut self, launch: Launch) {
        if self.config.username().is_none() {
            self.pending = Some(launch);
            self.open(Buffer::Settings);
            self.begin_edit(TextField::Username);
            self.info("welcome! choose the name other racers will see, then press Enter");
            return;
        }
        match launch {
            Launch::Home => {
                self.buffer = Buffer::Practice;
                self.focus = Focus::Explorer;
            }
            Launch::Solo { practice, file } => {
                self.config.practice = practice;
                match file {
                    Some(path) => self.edit_file(&path),
                    None => self.start_practice(),
                }
            }
            Launch::Multiplayer => self.open(Buffer::Race),
            Launch::Create(text) => self.connect(Intent::Create(text)),
            Launch::Join(code) => self.join_room(code),
            Launch::History => self.open(Buffer::History),
        }
    }

    pub(super) fn start_practice(&mut self) {
        let plan = Plan::from_practice(&self.config.practice);
        self.start_solo(plan);
    }

    fn start_solo(&mut self, plan: Plan) {
        self.end_activity();
        let run = SoloRun::start(plan, rand::random());
        self.activity = Some(Activity::Solo(Box::new(run)));
        self.open(Buffer::Session);
    }

    pub(super) fn restart_solo(&mut self) {
        if let Some(run) = self.solo() {
            let plan = run.plan.clone();
            self.start_solo(plan);
        }
    }

    pub(super) fn edit_file(&mut self, path: &std::path::Path) {
        match CustomText::load(path) {
            Ok(custom) => self.start_solo(Plan::File(custom)),
            Err(error) => self.error(error),
        }
    }

    pub(super) fn create_room(&mut self) {
        self.connect(Intent::Create(self.race_settings.race_text_source()));
    }

    pub(super) fn join_typed_room(&mut self) {
        match self.room_code.parse::<RoomCode>() {
            Ok(code) => self.connect(Intent::Join(code)),
            Err(error) => {
                self.error(error);
                self.begin_edit(TextField::RoomCode);
            }
        }
    }

    fn join_room(&mut self, code: RoomCode) {
        self.room_code = code.to_string();
        self.connect(Intent::Join(code));
    }

    fn connect(&mut self, intent: Intent) {
        let Some(username) = self.config.username() else {
            self.open(Buffer::Settings);
            self.begin_edit(TextField::Username);
            self.error("choose a username before racing");
            return;
        };
        let server = match network::server_url(&self.config.multiplayer.server) {
            Ok(server) => server,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        self.end_activity();
        self.info(format!("connecting to {server}…"));
        let client = RaceClient::connect(server, username, intent);
        self.activity = Some(Activity::Race(Box::new(client)));
        self.open(Buffer::Session);
    }

    pub(super) fn leave_room(&mut self, now: Instant) {
        let Some(Activity::Race(client)) = &mut self.activity else {
            return;
        };
        if client.confirm_leave(now) {
            let room = client.room_label();
            self.close_session();
            self.info(format!("left {room}"));
        } else {
            self.info("press Esc again to leave the race");
        }
    }

    pub(super) fn stop_typing(&mut self, now: Instant) {
        match &self.activity {
            Some(Activity::Solo(_)) => {
                self.close_session();
                self.info("session abandoned");
            }
            Some(Activity::Race(_)) => self.leave_room(now),
            None => {}
        }
    }

    /// Ends the running activity and goes back to the buffer it was started from.
    pub(super) fn close_session(&mut self) {
        let origin = match self.activity {
            Some(Activity::Race(_)) => Buffer::Race,
            _ => Buffer::Practice,
        };
        self.end_activity();
        if self.buffer == Buffer::Session {
            self.open(origin);
        }
    }

    fn end_activity(&mut self) {
        if let Some(Activity::Race(client)) = &self.activity {
            client.leave();
        }
        self.activity = None;
    }

    pub(super) fn type_char(&mut self, ch: char, now: Instant) {
        match &mut self.activity {
            Some(Activity::Solo(run)) => run.type_char(ch, now),
            Some(Activity::Race(client)) if client.accepts_typing() => {
                if let Some(session) = client.session_mut() {
                    session.type_char(ch, now);
                }
            }
            _ => return,
        }
        self.after_typing(now);
    }

    pub(super) fn edit_session(&mut self, now: Instant, edit: SessionEdit) {
        let session = match &mut self.activity {
            Some(Activity::Solo(run)) => &mut run.session,
            Some(Activity::Race(client)) if client.accepts_typing() => match client.session_mut() {
                Some(session) => session,
                None => return,
            },
            _ => return,
        };
        match edit {
            SessionEdit::Backspace => session.backspace(now),
            SessionEdit::DeleteWord => session.delete_word(now),
        };
        self.after_typing(now);
    }

    fn after_typing(&mut self, now: Instant) {
        match &mut self.activity {
            Some(Activity::Solo(_)) => self.conclude_solo(now),
            Some(Activity::Race(client)) => client.report_progress(now),
            None => {}
        }
    }

    pub(super) fn conclude_solo(&mut self, now: Instant) {
        let Some(Activity::Solo(run)) = &mut self.activity else {
            return;
        };
        if let Err(error) = run.conclude(&mut self.history, now) {
            self.error(format!("cannot save history: {error}"));
        }
    }

    pub(super) fn run_command(&mut self, command: Command) {
        match command {
            Command::Quit => self.quit = true,
            Command::Solo => self.start_practice(),
            Command::Words(count) => self.practise(Mode::Words, |practice| {
                practice.word_count = count.unwrap_or(practice.word_count);
            }),
            Command::Time(seconds) => self.practise(Mode::Time, |practice| {
                practice.duration = seconds.unwrap_or(practice.duration);
            }),
            Command::Quote => self.practise(Mode::Quote, |_| {}),
            Command::Code(language) => self.practise(Mode::Code, |practice| {
                practice.code_language = language.unwrap_or(practice.code_language);
            }),
            Command::Language(language) => self.set_language(language),
            Command::CodeLanguage(language) => self.set_code_language(language),
            Command::Edit(path) => self.edit_file(&path),
            Command::Set(setting) => self.apply_setting(setting),
            Command::Create => self.create_room(),
            Command::Join(code) => self.join_room(code),
            Command::Open(page) => self.open(page_buffer(page)),
        }
    }

    /// Switches the solo settings to `mode`, adjusted by `change`, saves
    /// them and starts a session with them.
    fn practise(&mut self, mode: Mode, change: impl FnOnce(&mut Practice)) {
        self.config.practice.mode = mode;
        change(&mut self.config.practice);
        self.save_config();
        self.start_practice();
    }
}

fn page_buffer(page: Page) -> Buffer {
    match page {
        Page::Practice => Buffer::Practice,
        Page::Race => Buffer::Race,
        Page::History => Buffer::History,
        Page::Settings => Buffer::Settings,
        Page::Help => Buffer::Help,
    }
}
