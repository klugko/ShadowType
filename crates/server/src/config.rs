use std::{num::NonZeroUsize, time::Duration};

use code_racer_protocol::MAX_ROOM_PLAYERS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub max_players: u8,
    pub room_ttl: Duration,
    pub race_timeout: Duration,
    pub countdown: Duration,
    pub max_rooms: usize,
    pub max_connections: usize,
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
    pub(crate) fn room_capacity(&self) -> u8 {
        self.max_players.min(MAX_ROOM_PLAYERS)
    }
}
