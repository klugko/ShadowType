use std::time::Instant;

use code_racer_protocol::RoomCode;

use crate::{
    app::{
        Activity, App, Buffer, TextField,
        race::{Intent, RaceClient, RoomRequest},
    },
    network,
};

impl App {
    pub(crate) fn create_room(&mut self) {
        self.connect(Intent::Create(self.config.race.race_text_source()));
    }

    pub(crate) fn join_typed_room(&mut self) {
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
    pub(super) fn join_room(&mut self, code: RoomCode) {
        let line = code.to_string();
        if self.connect(Intent::Join(code)) {
            self.room_code = line;
        }
    }

    /**
     * Connects to the race server for `intent`. Returns whether the
     * connection is under way, rather than refused or waiting for a name.
     */
    pub(super) fn connect(&mut self, intent: Intent) -> bool {
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
    pub(super) fn refuse_while_in_room(&mut self) -> bool {
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
    pub(crate) fn request_room(&mut self, request: RoomRequest, now: Instant) {
        let Some(Activity::Race(client)) = &mut self.activity else {
            return;
        };
        match (request, client.request(request, now)) {
            (_, Ok(())) => {}
            (RoomRequest::Again, Err(reason)) => self.info(reason),
            (_, Err(reason)) => self.error(reason),
        }
    }
}
