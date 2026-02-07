use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::game::entities::player::Player;

pub trait BonusCard {
    fn points(&self) -> u8;

    fn holder(&self) -> Option<Uuid>;
    fn set_holder(&mut self, holder: Option<Uuid>);

    fn minimum_number(&self) -> usize;
    fn player_number(&self, player: &Player) -> usize;

    fn recalculate(&mut self, players: &mut HashMap<Uuid, Player>) {
        let best = self.find_best_player(players);

        let Some((best_pid, best_value)) = best else {
            self.remove_holder(players);
            return;
        };

        if best_value < self.minimum_number() {
            self.remove_holder(players);
            return;
        }

        if self.holder() == Some(best_pid) {
            return;
        }

        self.transfer_holder(players, best_pid);
    }

    fn find_best_player(
        &self,
        players: &HashMap<Uuid, Player>,
    ) -> Option<(Uuid, usize)> {
        let mut best: Option<(Uuid, usize)> = None;
        let mut tie = false;

        for (pid, player) in players {
            let value = self.player_number(player);

            match best {
                None => {
                    best = Some((*pid, value));
                }
                Some((_, best_value)) if value > best_value => {
                    best = Some((*pid, value));
                    tie = false;
                }
                Some((_, best_value)) if value == best_value && value != 0 => {
                    tie = true;
                }
                _ => {}
            }
        }

        if tie {
            None
        } else {
            best
        }
    }

    fn transfer_holder(
        &mut self,
        players: &mut HashMap<Uuid, Player>,
        new_holder: Uuid,
    ) {
        if let Some(old_holder) = self.holder() {
            if let Some(player) = players.get_mut(&old_holder) {
                self.remove_points(player);
            }
        }

        if let Some(player) = players.get_mut(&new_holder) {
            self.add_points(player);
        }

        self.set_holder(Some(new_holder));
    }

    fn remove_holder(&mut self, players: &mut HashMap<Uuid, Player>) {
        if let Some(old_holder) = self.holder() {
            if let Some(player) = players.get_mut(&old_holder) {
                self.remove_points(player);
            }
        }
        self.set_holder(None);
    }

    fn add_points(&self, player: &mut Player) {
        for _ in 0..self.points() {
            player.add_victory_point();
        }
    }

    fn remove_points(&self, player: &mut Player) {
        for _ in 0..self.points() {
            player.remove_victory_point();
        }
    }
}



#[derive(Serialize, Deserialize, Clone)]
pub struct LongestRoad {
    holder: Option<Uuid>,
}

impl LongestRoad {
    pub fn new() -> Self {
        Self { holder: None }
    }
}

impl BonusCard for LongestRoad {
    fn points(&self) -> u8 {
        2
    }

    fn holder(&self) -> Option<Uuid> {
        self.holder
    }

    fn set_holder(&mut self, holder: Option<Uuid>) {
        self.holder = holder;
    }

    fn minimum_number(&self) -> usize {
        5
    }

    fn player_number(&self, player: &Player) -> usize {
        player.longest_road
    }
}


#[derive(Serialize, Deserialize, Clone)]
pub struct BiggestArmy {
    holder: Option<Uuid>,
}

impl BiggestArmy {
    pub fn new() -> Self {
        Self { holder: None }
    }
}

impl BonusCard for BiggestArmy {
    fn points(&self) -> u8 {
        2
    }

    fn holder(&self) -> Option<Uuid> {
        self.holder
    }

    fn set_holder(&mut self, holder: Option<Uuid>) {
        self.holder = holder;
    }

    fn minimum_number(&self) -> usize {
        3
    }

    fn player_number(&self, player: &Player) -> usize {
        player.knight_played
    }
}









#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::game::entities::player::Player;
    use std::collections::HashMap;
    use crate::game::entities::bonus_points::{BiggestArmy, LongestRoad, BonusCard};

    fn players(ids: &[Uuid]) -> HashMap<Uuid, Player> {
        ids.iter().copied().map(|id| (id, p(id))).collect()
    }

    fn p(id: Uuid) -> Player {
        Player::new(id, &format!("Player {}", id), 'A')
    }

    #[test]
    fn biggest_army_awarded_when_player_reaches_minimum_and_is_best() {
        let mut players = players(&[Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)]);

        players.get_mut(&Uuid::from_u128(2)).unwrap().knight_played = 3; // min is 3
        players.get_mut(&Uuid::from_u128(1)).unwrap().knight_played = 2;

        let vp_before = players[&Uuid::from_u128(2)].get_victory_points();

        let mut ba = BiggestArmy::new();
        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), Some(Uuid::from_u128(2)));
        assert_eq!(players[&Uuid::from_u128(2)].get_victory_points(), vp_before + ba.points());
    }

    #[test]
    fn biggest_army_not_awarded() {
        let mut players = players(&[Uuid::from_u128(1), Uuid::from_u128(2)]);

        players.get_mut(&Uuid::from_u128(1)).unwrap().knight_played = 2; // min is 3
        players.get_mut(&Uuid::from_u128(2)).unwrap().knight_played = 2;

        let mut ba = BiggestArmy::new();
        let p1_vp_before = players[&Uuid::from_u128(1)].get_victory_points();
        let p2_vp_before = players[&Uuid::from_u128(2)].get_victory_points();
        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), None, "threshold not reached, no holder");
        assert_eq!(players[&Uuid::from_u128(1)].get_victory_points(), p1_vp_before);
        assert_eq!(players[&Uuid::from_u128(2)].get_victory_points(), p2_vp_before);
    }

    #[test]
    fn biggest_army_transfers_points_when_holder_changes() {
        let mut players = players(&[Uuid::from_u128(1), Uuid::from_u128(2)]);

        players.get_mut(&Uuid::from_u128(1)).unwrap().knight_played = 3;

        let mut ba = BiggestArmy::new();
        ba.recalculate(&mut players);
        assert_eq!(ba.holder(), Some(Uuid::from_u128(1)));

        let p1_vp_after_award = players[&Uuid::from_u128(1)].get_victory_points();

        // Player 2 overtakes
        players.get_mut(&Uuid::from_u128(2)).unwrap().knight_played = 4;

        let p2_before = players[&Uuid::from_u128(2)].get_victory_points();
        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), Some(Uuid::from_u128(2)));
        assert_eq!(
            players[&Uuid::from_u128(1)].get_victory_points(),
            p1_vp_after_award - ba.points(),
            "old holder should lose the bonus points"
        );
        assert_eq!(
            players[&Uuid::from_u128(2)].get_victory_points(),
            p2_before + ba.points(),
            "new holder should gain the bonus points"
        );
    }

    #[test]
    fn longest_road_not_awarded_when_best_below_minimum() {
        let mut players = players(&[Uuid::from_u128(1), Uuid::from_u128(2)]);

        players.get_mut(&Uuid::from_u128(1)).unwrap().longest_road = 4; // min is 5
        players.get_mut(&Uuid::from_u128(2)).unwrap().longest_road = 4;

        let mut lr = LongestRoad::new();
        let p1_vp_before = players[&Uuid::from_u128(1)].get_victory_points();
        let p2_vp_before = players[&Uuid::from_u128(2)].get_victory_points();
        lr.recalculate(&mut players);

        assert_eq!(lr.holder(), None);
        assert_eq!(players[&Uuid::from_u128(1)].get_victory_points(), p1_vp_before);
        assert_eq!(players[&Uuid::from_u128(2)].get_victory_points(), p2_vp_before);
    }

    #[test]
    fn longest_road_awarded_when_player_reaches_minimum_and_is_best() {
        let mut players = players(&[Uuid::from_u128(1), Uuid::from_u128(2)]);

        players.get_mut(&Uuid::from_u128(1)).unwrap().longest_road = 5; // min is 5
        players.get_mut(&Uuid::from_u128(2)).unwrap().longest_road = 4;

        let p1_vp_before = players[&Uuid::from_u128(1)].get_victory_points();
        let p2_vp_before = players[&Uuid::from_u128(2)].get_victory_points();

        let mut lr = LongestRoad::new();
        lr.recalculate(&mut players);

        assert_eq!(lr.holder(), Some(Uuid::from_u128(1)));
        assert_eq!(players[&Uuid::from_u128(1)].get_victory_points(), p1_vp_before + lr.points());
        assert_eq!(players[&Uuid::from_u128(2)].get_victory_points(), p2_vp_before);
    }
}