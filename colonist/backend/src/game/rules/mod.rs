//! The rules of the game, expressed as operations on a `GameInstance`.
//!
//! Each entry point validates the request against the current game state and
//! either mutates the game and returns the message to broadcast, or explains
//! why the move is illegal. Nothing here knows about sockets or the lobby.

pub mod construction;
pub mod development;
pub mod trading;
pub mod turn_control;
