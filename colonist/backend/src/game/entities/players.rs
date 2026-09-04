use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::game::entities::player::Player;
use serde_with::serde_as;
use log::info;
use shared::PlayerColour;

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

    /// Seats a player with a free colour. Returns false when every colour is
    /// taken, in which case nothing is added.
    /// Colours already claimed in this game.
    pub fn taken_colours(&self) -> Vec<PlayerColour> {
        self.order
            .iter()
            .filter_map(|id| self.players.get(id))
            .map(|player| player.colour)
            .collect()
    }

    /// Colours a joining player may still pick from.
    pub fn available_colours(&self) -> Vec<PlayerColour> {
        let taken = self.taken_colours();
        PlayerColour::ALL
            .into_iter()
            .filter(|colour| !taken.contains(colour))
            .collect()
    }

    /// Seats a player under the name and colour they asked for. Names are
    /// trimmed and capped, and a blank one falls back to the colour's default.
    /// Fails if the colour has already been claimed.
    pub fn seat(
        &mut self,
        player_id: Uuid,
        name: &str,
        colour: PlayerColour,
    ) -> Result<(), String> {
        if self.players.contains_key(&player_id) {
            return Ok(());
        }

        if self.taken_colours().contains(&colour) {
            return Err(format!("{} is already taken", colour.label()));
        }

        let trimmed = name.trim();
        let name = if trimmed.is_empty() {
            colour.default_name().to_string()
        } else {
            trimmed.chars().take(PlayerColour::MAX_NAME_LEN).collect()
        };

        let player = Player::new(player_id, &name, colour);
        self.order.push(player.id);
        self.players.insert(player_id, player);
        Ok(())
    }

    pub fn remove_player(&mut self, player_id: Uuid) -> Result<(), String> {
        let Some(position) = self.order.iter().position(|id| *id == player_id) else {
            return Err("Player not found".to_string());
        };

        self.players.remove(&player_id);
        self.order.remove(position);

        if self.order.is_empty() {
            self.current_index = 0;
            return Ok(());
        }

        // Keep whoever was on turn on turn: removing someone ahead of them in
        // the order shifts every later slot down by one. If the player on turn
        // is the one leaving, the slot now holds the next player already,
        // except when they were last and the index has to wrap.
        if position < self.current_index {
            self.current_index -= 1;
        } else if self.current_index >= self.order.len() {
            self.current_index = 0;
        }

        Ok(())
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


#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn four_players() -> Players {
        Players::new(
            (1..=4)
                .map(|n| Player::new(pid(n), &format!("P{n}"), PlayerColour::Blue))
                .collect(),
        )
    }

    #[test]
    fn seat_honours_the_chosen_name_and_colour() {
        let mut players = Players::new(Vec::new());

        players.seat(pid(1), "  Ada  ", PlayerColour::Green).unwrap();

        let seated = players.get(pid(1)).unwrap();
        assert_eq!(seated.name, "Ada", "surrounding whitespace is trimmed");
        assert_eq!(seated.colour, PlayerColour::Green);
    }

    #[test]
    fn seat_rejects_a_colour_someone_already_took() {
        let mut players = Players::new(Vec::new());
        players.seat(pid(1), "first", PlayerColour::Red).unwrap();

        let err = players
            .seat(pid(2), "second", PlayerColour::Red)
            .expect_err("the same colour twice must be refused");

        assert!(err.contains("Red"), "unexpected error: {err}");
        assert_eq!(players.len(), 1, "the refused player must not be seated");
    }

    #[test]
    fn available_colours_shrink_as_players_are_seated() {
        let mut players = Players::new(Vec::new());
        assert_eq!(players.available_colours().len(), 4);

        players.seat(pid(1), "a", PlayerColour::Blue).unwrap();
        assert_eq!(players.available_colours(), vec![
            PlayerColour::Red,
            PlayerColour::Green,
            PlayerColour::Yellow,
        ]);

        players.seat(pid(2), "b", PlayerColour::Green).unwrap();
        assert_eq!(players.available_colours(), vec![
            PlayerColour::Red,
            PlayerColour::Yellow,
        ]);
    }

    #[test]
    fn a_blank_name_falls_back_to_the_colours_default() {
        let mut players = Players::new(Vec::new());
        players.seat(pid(1), "   ", PlayerColour::Yellow).unwrap();

        assert_eq!(
            players.get(pid(1)).unwrap().name,
            PlayerColour::Yellow.default_name()
        );
    }

    #[test]
    fn an_overlong_name_is_truncated_rather_than_rejected() {
        let mut players = Players::new(Vec::new());
        let long = "x".repeat(200);
        players.seat(pid(1), &long, PlayerColour::Blue).unwrap();

        assert_eq!(
            players.get(pid(1)).unwrap().name.chars().count(),
            PlayerColour::MAX_NAME_LEN
        );
    }

    /// Re-seating an existing player is a reconnect, not a colour clash.
    #[test]
    fn seating_an_already_seated_player_is_a_no_op() {
        let mut players = Players::new(Vec::new());
        players.seat(pid(1), "original", PlayerColour::Blue).unwrap();

        players.seat(pid(1), "renamed", PlayerColour::Red).unwrap();

        assert_eq!(players.len(), 1);
        assert_eq!(players.get(pid(1)).unwrap().name, "original");
        assert_eq!(players.get(pid(1)).unwrap().colour, PlayerColour::Blue);
    }

    #[test]
    fn remove_player_shrinks_the_turn_order() {
        let mut players = four_players();

        players.remove_player(pid(2)).unwrap();

        assert_eq!(players.len(), 3, "len() must reflect the removal");
        assert!(players.get(pid(2)).is_none());
        assert_eq!(
            (0..players.len())
                .map(|i| players.get_by_index(i).unwrap().id)
                .collect::<Vec<_>>(),
            vec![pid(1), pid(3), pid(4)]
        );
    }

    #[test]
    fn remove_player_reports_unknown_player() {
        let mut players = four_players();
        assert!(players.remove_player(pid(99)).is_err());
        assert_eq!(players.len(), 4);
    }

    /// Regression test: `order` used to keep the departed id, so `get_current_player`
    /// unwrapped a missing entry and panicked.
    #[test]
    fn current_player_survives_every_removal_position() {
        for victim in 1..=4u128 {
            for turn in 0..4 {
                let mut players = four_players();
                for _ in 0..turn {
                    players.next_turn();
                }

                players.remove_player(pid(victim)).unwrap();

                // Must not panic, and must never hand back the player who left.
                assert_ne!(players.get_current_player().id, pid(victim));
                players.next_turn();
                assert_ne!(players.get_current_player().id, pid(victim));
            }
        }
    }

    #[test]
    fn removing_someone_earlier_keeps_the_same_player_on_turn() {
        let mut players = four_players();
        players.next_turn();
        players.next_turn(); // on P3
        assert_eq!(players.get_current_player().id, pid(3));

        players.remove_player(pid(1)).unwrap();

        assert_eq!(
            players.get_current_player().id,
            pid(3),
            "removing an earlier player must not change whose turn it is"
        );
    }

    #[test]
    fn removing_the_last_player_on_turn_wraps_to_the_start() {
        let mut players = four_players();
        for _ in 0..3 {
            players.next_turn();
        }
        assert_eq!(players.get_current_player().id, pid(4));

        players.remove_player(pid(4)).unwrap();

        assert_eq!(players.get_current_player().id, pid(1));
    }

    #[test]
    fn removing_the_last_remaining_player_leaves_an_empty_roster() {
        let mut players = Players::new(vec![Player::new(pid(1), "solo", PlayerColour::Red)]);

        players.remove_player(pid(1)).unwrap();

        assert_eq!(players.len(), 0);
    }
}
