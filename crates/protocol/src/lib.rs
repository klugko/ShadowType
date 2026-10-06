//! Wire protocol between code-racer clients and the race server.

pub mod ids;
pub mod message;
pub mod room;

pub use ids::{InvalidRoomCode, InvalidUsername, PlayerId, RoomCode, Username};
pub use message::{
    ClientMessage, ErrorCode, MAX_MESSAGE_BYTES, PROTOCOL_VERSION, Progress, ServerError,
    ServerMessage,
};
pub use room::{Phase, PlayerProgress, PlayerView, RACE_WORD_COUNTS, RoomView, is_raceable};
