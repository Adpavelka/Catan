use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dice {
    value: u8,
}

impl Dice {
    pub fn new() -> Dice {
        Dice {
            value: 0,
        }
    }

    pub fn roll(&mut self) -> u8 {
        let mut rng = rand::thread_rng();
        let die1: u8 = rng.gen_range(1..=6);
        let die2: u8 = rng.gen_range(1..=6);
        self.value = die1 + die2;
        self.value
    }

}
