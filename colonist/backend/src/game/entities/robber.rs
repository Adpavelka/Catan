use serde::{Deserialize, Serialize};
use shared::ResourceType;
use crate::game::entities::board::{Coordinates, Board};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Robber {
    pub pos: Coordinates,
}

impl Robber {
    pub fn new(board: &Board) -> Self {
        for (coord, hex) in &board.hexes {
            if hex.resource == ResourceType::Desert {
                return Self {
                    pos: *coord,
                };
            }
        }

        panic!("No desert hex found on the board");
    }
}

impl Default for Robber {
    fn default() -> Self {
        Self { pos: (0, 0) } // some placeholder
    }
}




#[cfg(test)]
mod tests {
    use crate::game::entities::board::Board;
    use crate::game::entities::robber::Robber;
    use crate::game::entities::resources::ResourceType;
    #[test]
    fn robber_new_starts_on_desert_hex() {
        let board = Board::new_standard_board();
        let robber = Robber::new(&board);

        let hex = board
            .hexes
            .get(&robber.pos)
            .expect("robber position must exist in board hexes");
        assert_eq!(hex.resource, ResourceType::Desert);
    }

    #[test]
    fn robber_default_position_is_placeholder() {
        let r = Robber::default();
        assert_eq!(r.pos, (0, 0));
    }
}
