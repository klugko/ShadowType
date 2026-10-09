use std::time::Instant;

use code_racer_engine::TextSource;
use code_racer_protocol::{
    ClientMessage, ErrorCode, PlayerId, Progress, RACE_WORD_COUNTS, RoomCode, ServerError,
    ServerMessage, Username, is_raceable,
};
use rand::RngExt;

use super::{Hub, Player};
use crate::room::Room;

impl Hub {
    pub(super) fn handle_message(&mut self, id: PlayerId, message: ClientMessage, now: Instant) {
        if !self.players.contains_key(&id) {
            return;
        }
        let result = match message {
            ClientMessage::Hello { .. } => Err(ServerError::new(
                ErrorCode::InvalidMessage,
                "the handshake is already done",
            )),
            ClientMessage::CreateRoom { text } => self.create_room(id, text, now),
            ClientMessage::JoinRoom { code } => self.join_room(id, code, now),
            ClientMessage::LeaveRoom => self.leave_room(id, now),
            ClientMessage::SetReady { ready } => self.set_ready(id, ready, now),
            ClientMessage::StartRace => self.start_race(id, now),
            ClientMessage::Progress(progress) => self.report_progress(id, progress, now),
            ClientMessage::ReturnToLobby => self.return_to_lobby(id, now),
        };
        if let Err(error) = result {
            self.send(id, ServerMessage::Error(error));
        }
    }

    pub(super) fn disconnect(&mut self, id: PlayerId, now: Instant) {
        if let Some(Player {
            room: Some(code), ..
        }) = self.players.remove(&id)
        {
            self.remove_member(id, &code, now);
        }
    }

    fn create_room(
        &mut self,
        id: PlayerId,
        text: TextSource,
        now: Instant,
    ) -> Result<(), ServerError> {
        require_raceable(&text)?;
        if self.rooms.len() >= self.config.max_rooms {
            return Err(ServerError::new(
                ErrorCode::ServerFull,
                "the server cannot host more rooms, try again later",
            ));
        }
        let name = self.name_of(id)?;
        let code = self.unused_code();
        let room = Room::new(
            code.clone(),
            id,
            name,
            text,
            self.config.room_capacity(),
            now,
        );
        self.rooms.insert(code.clone(), room);
        self.move_player(id, code, now);
        Ok(())
    }

    fn join_room(&mut self, id: PlayerId, code: RoomCode, now: Instant) -> Result<(), ServerError> {
        if self.room_of(id) == Some(&code) {
            self.send_view(id, &code);
            return Ok(());
        }
        let name = self.name_of(id)?;
        let room = self.rooms.get_mut(&code).ok_or_else(|| {
            ServerError::new(ErrorCode::RoomNotFound, format!("room {code} not found"))
        })?;
        room.join(id, name, now)?;
        self.move_player(id, code, now);
        Ok(())
    }

    fn leave_room(&mut self, id: PlayerId, now: Instant) -> Result<(), ServerError> {
        self.current_room(id, now)?;
        self.leave_current_room(id, now);
        Ok(())
    }

    fn set_ready(&mut self, id: PlayerId, ready: bool, now: Instant) -> Result<(), ServerError> {
        let (code, room) = self.current_room(id, now)?;
        room.set_ready(id, ready, now)?;
        self.broadcast_view(&code);
        Ok(())
    }

    fn start_race(&mut self, id: PlayerId, now: Instant) -> Result<(), ServerError> {
        let seed = self.rng.random();
        let countdown = self.config.countdown;
        let (code, room) = self.current_room(id, now)?;
        let text = room.text_source().generate(seed).text;
        room.start_countdown(id, &text, now, countdown)?;
        let duration_ms = u32::try_from(countdown.as_millis()).unwrap_or(u32::MAX);
        self.broadcast(&code, &ServerMessage::Countdown { text, duration_ms });
        self.broadcast_view(&code);
        Ok(())
    }

    fn report_progress(
        &mut self,
        id: PlayerId,
        progress: Progress,
        now: Instant,
    ) -> Result<(), ServerError> {
        let (code, room) = self.current_room(id, now)?;
        room.report_progress(id, progress, now)?;
        self.dirty.insert(code);
        Ok(())
    }

    fn return_to_lobby(&mut self, id: PlayerId, now: Instant) -> Result<(), ServerError> {
        let (code, room) = self.current_room(id, now)?;
        room.return_to_lobby(id, now)?;
        self.broadcast_view(&code);
        Ok(())
    }

    /// Puts a player who was just added to the room `code` there, leaving their previous room.
    fn move_player(&mut self, id: PlayerId, code: RoomCode, now: Instant) {
        self.leave_current_room(id, now);
        if let Some(player) = self.players.get_mut(&id) {
            player.room = Some(code.clone());
        }
        self.broadcast_view(&code);
    }

    fn leave_current_room(&mut self, id: PlayerId, now: Instant) {
        if let Some(code) = self
            .players
            .get_mut(&id)
            .and_then(|player| player.room.take())
        {
            self.remove_member(id, &code, now);
        }
    }

    fn remove_member(&mut self, id: PlayerId, code: &RoomCode, now: Instant) {
        let Some(room) = self.rooms.get_mut(code) else {
            return;
        };
        room.leave(id, now);
        room.advance(now, self.config.race_timeout);
        if room.has_connected_members() {
            self.broadcast_view(code);
        } else {
            self.rooms.remove(code);
            self.dirty.remove(code);
        }
    }

    /**
     * The room of a player, first brought up to date with the clock so that
     * no request acts on a countdown or a race that is already over.
     */
    fn current_room(
        &mut self,
        id: PlayerId,
        now: Instant,
    ) -> Result<(RoomCode, &mut Room), ServerError> {
        let not_in_room = || ServerError::new(ErrorCode::NotInRoom, "you are not in a room");
        let code = self.room_of(id).cloned().ok_or_else(not_in_room)?;
        self.advance_room(&code, now);
        let room = self.rooms.get_mut(&code).ok_or_else(not_in_room)?;
        Ok((code, room))
    }

    fn room_of(&self, id: PlayerId) -> Option<&RoomCode> {
        self.players.get(&id)?.room.as_ref()
    }

    fn name_of(&self, id: PlayerId) -> Result<Username, ServerError> {
        self.players
            .get(&id)
            .map(|player| player.name.clone())
            .ok_or_else(|| ServerError::new(ErrorCode::HandshakeRequired, "say hello first"))
    }

    fn unused_code(&mut self) -> RoomCode {
        loop {
            let code = RoomCode::random(&mut self.rng);
            if !self.rooms.contains_key(&code) {
                return code;
            }
        }
    }
}

fn require_raceable(text: &TextSource) -> Result<(), ServerError> {
    if is_raceable(text) {
        Ok(())
    } else {
        Err(ServerError::new(
            ErrorCode::InvalidSettings,
            format!(
                "races need {} to {} words",
                RACE_WORD_COUNTS.start(),
                RACE_WORD_COUNTS.end()
            ),
        ))
    }
}
