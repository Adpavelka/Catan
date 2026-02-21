use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::game::entities::player::Player;
use serde_with::serde_as;
use log::info;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Players {
    #[serde_as(as = "Vec<(_, _)>")]
    players: HashMap<Uuid, Player>,

    order: Vec<Uuid>,
    current_index: usize,
}

impl Players {
    pub fn new(pplayers: Vec<Player>) -> Self {
        let mut players = HashMap::new();
        let mut order = Vec::new();

        for p in pplayers {
            let id = p.id;
            players.insert(id, p);
            order.push(id);
        }

        Self { 
            players,
            order,
            current_index: 0,
        }
    }

    pub fn add_player(&mut self, player: Player) {
        let id = player.id;
        self.players.insert(id, player);
        self.order.push(id);
    }

    pub fn add_player_with_colour(&mut self, player_id: Uuid) {
        if self.players.contains_key(&player_id) {
            return;
        }

        let all_colors = vec![
            ('b', "Steve"),
            ('r', "Bob"),
            ('g', "Kevin"),
            ('w', "George")
        ];

        let taken_colors: Vec<char> = self.players.iter()
            .map(|(_, p)| p.colour)
            .collect();

        let mut available: Vec<(char, &str)> = all_colors.into_iter()
            .filter(|(c, _)| !taken_colors.contains(c))
            .collect();

        if let Some(pos) = self.get_random_index(available.len()) {
            let (color, default_name) = available.remove(pos);

            let player = Player::new(player_id, default_name, color);
            self.order.push(player.id);
            self.players.insert(player_id, player);
        }
    }

    fn get_random_index(&self, len: usize) -> Option<usize> {
        if len == 0 { return None; }
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Some(rng.gen_range(0..len))
    }

    pub fn remove_player(&mut self, player_id: Uuid) -> Result<(), String> {
        if self.players.remove(&player_id).is_some() {
            self.players.retain(|_, p| p.id != player_id);
            Ok(())
        } else {
            Err("Player not found".to_string())
        }
    }

    pub fn get_current_player(&self) -> &Player {
        let id = *self.order.get(self.current_index).unwrap();
        self.players.get(&id).unwrap()
    }

    pub fn get_current_index(&self) -> usize {
        self.current_index
    }

    pub fn next_turn(&mut self) {
        self.current_index = (self.current_index + 1) % self.len();
        self.players.iter().for_each(|(pid, _p)| info!("{}", pid));
    }

    pub fn prev_turn(&mut self) {
        self.current_index = (self.current_index + self.len() - 1) % self.len();
        self.players.iter().for_each(|(pid, _p)| info!("{}", pid));
    }

    pub fn reset_order(&mut self) {
       self.current_index = 0;
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn get(&self, id: Uuid) -> Option<&Player> {
        self.players.get(&id)
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Player> {
        self.players.get_mut(&id)
    }

    pub fn get_by_index(&self, idx: usize) -> Option<&Player> {
        self.order.get(idx).and_then(|id| self.players.get(id))
    }
}
