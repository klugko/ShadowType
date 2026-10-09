//! Things the user can do, whichever key or command triggered them.

mod leaving;

use std::time::Instant;

use code_racer_protocol::RoomCode;

use super::{
    Activity, App, Buffer, TextField,
    command::{Command, Page},
    practice::{CustomText, Plan, SoloRun},
    race::{Intent, RaceClient, RoomRequest},
    text_event::TextEvent,
};
use crate::{
    cli::Launch,
    config::{Look, Mode, Practice},
    network,
};

impl App {
    /**
     * Opens what the command line asked for, after asking for the name
     * other racers will see on first launch.
     */
    pub(super) fn launch(&mut self, launch: Launch) {
        if self.config.username().is_none() {
            self.ask_username(launch);
            self.info("welcome! choose the name other racers will see, then Enter (Esc skips)");
            return;
        }
        self.run_launch(launch);
    }

    /// Opens the username field, `then` to follow once it is set.
    fn ask_username(&mut self, then: Launch) {
        self.open(Buffer::Settings);
        self.settings_cursor.first();
        self.begin_edit(TextField::Username);
        self.pending = Some(then);
    }

    pub(super) fn run_launch(&mut self, launch: Launch) {
        match launch {
            Launch::Home => {
                self.buffer = Buffer::Practice;
                self.focus_explorer_if_shown();
            }
            Launch::Solo { practice, file } => {
                self.saved
                    .change_for_this_run(&mut self.config, |config| config.practice = practice);
                match file {
                    Some(path) => self.edit_file(&path),
                    None => self.start_practice(),
                }
            }
            Launch::Multiplayer => self.open(Buffer::Race),
            Launch::Create(text) => {
                self.connect(Intent::Create(text));
            }
            Launch::Join(code) => self.join_room(code),
            Launch::History => self.open(Buffer::History),
        }
    }

    pub(super) fn start_practice(&mut self) {
        let plan = Plan::from_practice(&self.config.practice);
        self.start_solo(plan);
    }

    fn start_solo(&mut self, plan: Plan) {
        if self.refuse_while_in_room() {
            return;
        }
        self.end_activity();
        let seed = rand::random();
        self.shuffle_look(seed);
        let run = SoloRun::start(plan, seed);
        self.activity = Some(Activity::Solo(Box::new(run)));
        self.open(Buffer::Session);
    }

    /// Draws the look of the next text, for when the settings shuffle looks.
    pub(super) fn shuffle_look(&mut self, seed: u64) {
        self.shuffled = Look::shuffled_after(self.shuffled, seed.rotate_right(11));
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
        self.connect(Intent::Create(self.config.race.race_text_source()));
    }

    pub(super) fn join_typed_room(&mut self) {
        match self.room_code.parse::<RoomCode>() {
            Ok(code) => {
                self.connect(Intent::Join(code));
            }
            Err(error) => {
                self.error(error);
                self.begin_edit(TextField::RoomCode);
            }
        }
    }

    /**
     * Joins room `code`, which the room line of `race.toml` then shows once
     * the connection is under way.
     */
    fn join_room(&mut self, code: RoomCode) {
        let line = code.to_string();
        if self.connect(Intent::Join(code)) {
            self.room_code = line;
        }
    }

    /**
     * Connects to the race server for `intent`. Returns whether the
     * connection is under way, rather than refused or waiting for a name.
     */
    fn connect(&mut self, intent: Intent) -> bool {
        if self.refuse_while_in_room() {
            return false;
        }
        let Some(username) = self.config.username() else {
            self.ask_username(intent.into());
            self.error("choose a username before racing, then Enter");
            return false;
        };
        let server = match network::server_url(&self.config.multiplayer.server) {
            Ok(server) => server,
            Err(error) => {
                self.error(error);
                return false;
            }
        };
        self.end_activity();
        self.info(format!("connecting to {server}…"));
        let client = RaceClient::connect(server, username, intent);
        self.activity = Some(Activity::Race(Box::new(client)));
        self.open(Buffer::Session);
        true
    }

    /**
     * Whether the player is in a room, which starting anything else would
     * leave: that is refused, as leaving takes Esc, twice during a race.
     */
    fn refuse_while_in_room(&mut self) -> bool {
        let Some(client) = self.race() else {
            return false;
        };
        let refusal = match &client.room {
            Some(room) => format!("leave room {} first with Esc", room.code),
            None => "cancel the connection first with Esc".to_owned(),
        };
        self.error(refusal);
        true
    }

    /**
     * Asks the room for `request`, telling the player why when it cannot be
     * made.
     */
    pub(super) fn request_room(&mut self, request: RoomRequest, now: Instant) {
        let Some(Activity::Race(client)) = &mut self.activity else {
            return;
        };
        match (request, client.request(request, now)) {
            (_, Ok(())) => {}
            (RoomRequest::Again, Err(reason)) => self.info(reason),
            (_, Err(reason)) => self.error(reason),
        }
    }

    pub(super) fn type_char(&mut self, ch: char, now: Instant) {
        let taken = self.text_event(TextEvent::Typed(ch), now);
        if !taken && self.is_typing_blocked() {
            self.info("fix the mistake first: Backspace or Ctrl+W");
        }
    }

    /**
     * Hands `event` to the text being typed, solo or in a race, and keeps
     * the record of a solo session it ended. Returns whether the text took
     * the key.
     */
    pub(super) fn text_event(&mut self, event: TextEvent, now: Instant) -> bool {
        let taken = self
            .activity
            .as_mut()
            .is_some_and(|activity| activity.text_event(event, now));
        self.conclude_solo(now);
        taken
    }

    fn conclude_solo(&mut self, now: Instant) {
        let Some(Activity::Solo(run)) = &mut self.activity else {
            return;
        };
        if let Some(record) = run.conclude(&self.history, now) {
            self.save_record(record);
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

    /**
     * Switches the solo settings to `mode`, adjusted by `change`, saves
     * them and starts a session with them.
     */
    fn practise(&mut self, mode: Mode, change: impl Fn(&mut Practice)) {
        if self.refuse_while_in_room() {
            return;
        }
        self.choose(|config| {
            config.practice.mode = mode;
            change(&mut config.practice);
        });
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
