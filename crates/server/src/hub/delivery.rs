use std::{collections::HashMap, time::Instant};

use code_racer_protocol::{PlayerId, RoomCode, ServerMessage};
use tokio::sync::mpsc::error::TrySendError;
use tracing::warn;

use super::{Hub, Player};
use crate::room::Room;

impl Hub {
    pub(super) fn broadcast_view(&mut self, code: &RoomCode) {
        if let Some(room) = self.rooms.get(code) {
            let view = ServerMessage::Room(room.view());
            self.broadcast(code, &view);
            self.dirty.remove(code);
        }
    }

    pub(super) fn broadcast(&mut self, code: &RoomCode, message: &ServerMessage) {
        let Some(room) = self.rooms.get(code) else {
            return;
        };
        for id in room.connected_members() {
            if !deliver(&self.players, id, message.clone()) {
                self.stuck.push(id);
            }
        }
    }

    pub(super) fn send_view(&mut self, id: PlayerId, code: &RoomCode) {
        if let Some(view) = self.rooms.get(code).map(Room::view) {
            self.send(id, ServerMessage::Room(view));
        }
    }

    pub(super) fn send(&mut self, id: PlayerId, message: ServerMessage) {
        if !deliver(&self.players, id, message) {
            self.stuck.push(id);
        }
    }

    pub(super) fn drop_stuck_players(&mut self, now: Instant) {
        while let Some(id) = self.stuck.pop() {
            self.disconnect(id, now);
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
