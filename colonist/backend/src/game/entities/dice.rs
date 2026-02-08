use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dice {
    value1: u8,
    value2: u8,
}

impl Dice {
    pub fn new() -> Dice {
        Dice {
            value1: 0,
            value2: 0,
        }
    }

    pub fn roll(&mut self) -> u8 {
        let mut rng = rand::thread_rng();
        self.value1 = rng.gen_range(1..=6);
        self.value2 = rng.gen_range(1..=6);
        self.value1 + self.value2
    }


    pub fn values(&self) -> (u8, u8) {
        (self.value1, self.value2)
    }
}
