//! Limits and timings of a race server.

use std::{num::NonZeroUsize, time::Duration};

use code_racer_protocol::MAX_ROOM_PLAYERS;

/// Everything an operator can tune about a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Players allowed in one room, capped at [`MAX_ROOM_PLAYERS`].
    pub max_players: u8,
    /// Inactivity after which a room is closed and its members are told so.
    pub room_ttl: Duration,
    /// Longest a race may last; unfinished players are ranked by progress.
    pub race_timeout: Duration,
    /// Delay between the host starting a race and the first keystroke.
    pub countdown: Duration,
    pub max_rooms: usize,
    /// Simultaneous WebSocket connections; extra ones are refused with `ServerFull`.
    pub max_connections: usize,
    /**
     * Simultaneous connections from one IP address, handshakes included, so
     * that one machine cannot hold every slot; extra ones are dropped at
     * once. Peers behind one shared address, such as a NAT or a port forward
     * that hides client addresses, count together. `None` lifts the limit.
     */
    pub max_connections_per_address: Option<NonZeroUsize>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_players: 8,
            room_ttl: Duration::from_secs(30 * 60),
            race_timeout: Duration::from_secs(5 * 60),
            countdown: Duration::from_secs(3),
            max_rooms: 256,
            max_connections: 512,
            max_connections_per_address: NonZeroUsize::new(16),
        }
    }
}

impl ServerConfig {
    /**
     * Players a room holds: beyond [`MAX_ROOM_PLAYERS`], room views could
     * outgrow the messages clients accept.
     */
    pub(crate) fn room_capacity(&self) -> u8 {
        self.max_players.min(MAX_ROOM_PLAYERS)
    }
}
