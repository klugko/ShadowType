/*!
 * The hub is the single owner of every room and connected player.
 *
 * It runs as one task fed with [`Command`]s, so no state is shared between
 * connections. [`Hub`] holds the logic and is driven synchronously with
 * explicit instants; [`run`] is the thin async loop around it.
 */

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use code_racer_engine::TextSource;
use code_racer_protocol::{
    ClientMessage, ErrorCode, PlayerId, Progress, RACE_WORD_COUNTS, RoomCode, ServerError,
    ServerMessage, Username, is_raceable,
};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use tokio::{
    sync::mpsc::{self, error::TrySendError},
    time::{self, MissedTickBehavior},
};
use tracing::warn;

use crate::{config::ServerConfig, room::Room};

const TICK_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum Command {
    Connected {
        id: PlayerId,
        name: Username,
        outbox: mpsc::Sender<ServerMessage>,
    },
    Message {
        id: PlayerId,
        message: ClientMessage,
    },
    Disconnected {
        id: PlayerId,
    },
}

#[derive(Debug)]
struct Player {
    name: Username,
    outbox: mpsc::Sender<ServerMessage>,
    room: Option<RoomCode>,
}

#[derive(Debug)]
pub struct Hub {
    config: ServerConfig,
    players: HashMap<PlayerId, Player>,
    rooms: HashMap<RoomCode, Room>,
    dirty: HashSet<RoomCode>,
    stuck: Vec<PlayerId>,
    rng: StdRng,
}

impl Hub {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            players: HashMap::new(),
            rooms: HashMap::new(),
            dirty: HashSet::new(),
            stuck: Vec::new(),
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    pub fn handle(&mut self, command: Command, now: Instant) {
        match command {
            Command::Connected { id, name, outbox } => {
                self.players.insert(
                    id,
                    Player {
                        name,
                        outbox,
                        room: None,
                    },
                );
            }
            Command::Message { id, message } => self.handle_message(id, message, now),
            Command::Disconnected { id } => self.disconnect(id, now),
        }
        self.drop_stuck_players(now);
    }

    pub fn tick(&mut self, now: Instant) {
        let race_timeout = self.config.race_timeout;
        for (code, room) in &mut self.rooms {
            if room.advance(now, race_timeout) {
                self.dirty.insert(code.clone());
            }
        }
        for code in std::mem::take(&mut self.dirty) {
            self.broadcast_view(&code);
        }
        self.close_idle_rooms(now);
        self.drop_stuck_players(now);
    }

    fn handle_message(&mut self, id: PlayerId, message: ClientMessage, now: Instant) {
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

    fn create_room(
        &mut self,
        id: PlayerId,
        text: TextSource,
        now: Instant,
    ) -> Result<(), ServerError> {
        if !is_raceable(&text) {
            return Err(ServerError::new(
                ErrorCode::InvalidSettings,
                format!(
                    "races need {} to {} words",
                    RACE_WORD_COUNTS.start(),
                    RACE_WORD_COUNTS.end()
                ),
            ));
        }
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

    fn disconnect(&mut self, id: PlayerId, now: Instant) {
        if let Some(Player {
            room: Some(code), ..
        }) = self.players.remove(&id)
        {
            self.remove_member(id, &code, now);
        }
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

    fn close_idle_rooms(&mut self, now: Instant) {
        let ttl = self.config.room_ttl;
        let idle: Vec<RoomCode> = self
            .rooms
            .iter()
            .filter(|(_, room)| room.idle_for(now) >= ttl)
            .map(|(code, _)| code.clone())
            .collect();
        for code in idle {
            self.close_room(&code);
        }
    }

    fn close_room(&mut self, code: &RoomCode) {
        let notice = ServerMessage::error(
            ErrorCode::RoomNotFound,
            format!(
                "room {code} closed after {} of inactivity",
                describe(self.config.room_ttl)
            ),
        );
        self.broadcast(code, &notice);
        if let Some(room) = self.rooms.remove(code) {
            for id in room.connected_members() {
                if let Some(player) = self.players.get_mut(&id) {
                    player.room = None;
                }
            }
        }
        self.dirty.remove(code);
    }

    fn broadcast_view(&mut self, code: &RoomCode) {
        if let Some(room) = self.rooms.get(code) {
            let view = ServerMessage::Room(room.view());
            self.broadcast(code, &view);
            self.dirty.remove(code);
        }
    }

    fn broadcast(&mut self, code: &RoomCode, message: &ServerMessage) {
        let Some(room) = self.rooms.get(code) else {
            return;
        };
        for id in room.connected_members() {
            if !deliver(&self.players, id, message.clone()) {
                self.stuck.push(id);
            }
        }
    }

    fn send_view(&mut self, id: PlayerId, code: &RoomCode) {
        if let Some(view) = self.rooms.get(code).map(Room::view) {
            self.send(id, ServerMessage::Room(view));
        }
    }

    fn send(&mut self, id: PlayerId, message: ServerMessage) {
        if !deliver(&self.players, id, message) {
            self.stuck.push(id);
        }
    }

    fn drop_stuck_players(&mut self, now: Instant) {
        while let Some(id) = self.stuck.pop() {
            self.disconnect(id, now);
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

    fn advance_room(&mut self, code: &RoomCode, now: Instant) {
        let race_timeout = self.config.race_timeout;
        let changed = self
            .rooms
            .get_mut(code)
            .is_some_and(|room| room.advance(now, race_timeout));
        if changed {
            self.broadcast_view(code);
        }
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

/// Runs the hub until every connection and the server dropped their command senders.
pub async fn run(mut hub: Hub, mut commands: mpsc::Receiver<Command>) {
    let mut ticker = time::interval(TICK_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(command) => hub.handle(command, Instant::now()),
                None => return,
            },
            _ = ticker.tick() => hub.tick(Instant::now()),
        }
    }
}

/**
 * Queues a message for a player. Returns false when their connection is gone
 * or so far behind that its outbox is full.
 */
fn deliver(players: &HashMap<PlayerId, Player>, id: PlayerId, message: ServerMessage) -> bool {
    let Some(player) = players.get(&id) else {
        return true;
    };
    match player.outbox.try_send(message) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            warn!(player = %id, "dropping a client that stopped reading its messages");
            false
        }
        Err(TrySendError::Closed(_)) => false,
    }
}

fn describe(duration: Duration) -> String {
    let (count, unit) = match duration.as_secs() {
        seconds @ 0..60 => (seconds, "second"),
        seconds => (seconds / 60, "minute"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural}")
}

#[cfg(test)]
mod tests;
