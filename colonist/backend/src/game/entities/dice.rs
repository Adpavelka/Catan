use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::errors::GameError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dice {
    value1: u8,
    value2: u8,
    thrown: bool,
}

impl Dice {
    pub fn new() -> Dice {
        Dice {
            value1: 0,
            value2: 0,
            thrown: false,
        }
    }

    pub fn roll(&mut self) -> Result<u8, GameError> {
        if self.thrown {
            return Err(GameError::UnauthorizedDiceThrow);
        }

        let mut rng = rand::thread_rng();

        self.value1 = rng.gen_range(1..=6);
        self.value2 = rng.gen_range(1..=6);
        self.thrown = true;

        Ok(self.value1 + self.value2)
    }

    pub fn next_turn(&mut self) {
        self.thrown = false;
    }

    pub fn values(&self) -> (u8, u8) {
        (self.value1, self.value2)
    }

    pub fn was_dice_rolled(&self) -> bool {
        self.thrown
    }
}
