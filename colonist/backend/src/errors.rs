use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum GameError {
    NotEnoughBuildingsOfThisType,
    WrongResourceRatio,
    PlayerNotFound,
    NotPlayersTurn,
    NotEnoughResources,
    BankOutOfResources,
    InvalidPosition,
    InvalidRobberPosition,
    InvalidAction,
    TradeNotFound,
    UnauthorizedDiceThrow,
    UnauthorizedTrade,
    TradeWithSelf,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            GameError::NotEnoughBuildingsOfThisType => "Not enough buildings of this type",
            GameError::WrongResourceRatio => "Wrong resource ratio",
            GameError::PlayerNotFound => "Player not found",
            GameError::NotPlayersTurn => "Not player's turn",
            GameError::NotEnoughResources => "Not enough resources",
            GameError::InvalidPosition => "Invalid position",
            GameError::InvalidRobberPosition => "You cannot place robber on the same spot",
            GameError::InvalidAction => "Invalid action",
            GameError::TradeNotFound => "Trade not found",
            GameError::UnauthorizedDiceThrow => "Cannot roll multiple times in one turn",
            GameError::UnauthorizedTrade => "Unauthorized trade",
            GameError::TradeWithSelf => "Cannot trade with self",
            GameError::BankOutOfResources => "Bank not enough resources",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for GameError {}