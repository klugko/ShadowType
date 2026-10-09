/*!
 * The hub is the single owner of every room and connected player.
 *
 * It runs as one task fed with [`Command`]s, so no state is shared between
 * connections. [`Hub`] holds the logic and is driven synchronously with
 * explicit instants; [`run`] is the thin async loop around it.
 */

mod delivery;
mod requests;
mod upkeep;

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use code_racer_protocol::{ClientMessage, PlayerId, RoomCode, ServerMessage, Username};
use rand::{SeedableRng, rngs::StdRng};
use tokio::{
    sync::mpsc,
    time::{self, MissedTickBehavior},
};

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

#[cfg(test)]
mod tests;
