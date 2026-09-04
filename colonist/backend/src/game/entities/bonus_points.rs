use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::game::entities::{player::Player, players::Players};

pub trait BonusCard {
    fn points(&self) -> u8;

    fn holder(&self) -> Option<Uuid>;
    fn set_holder(&mut self, holder: Option<Uuid>);

    fn minimum_number(&self) -> usize;
    fn player_number(&self, player: &Player) -> usize;

    /// Awards the card to whoever leads outright. The holder keeps it while
    /// they are still level with the best score: a challenger has to *beat*
    /// them, not merely match them. If the holder falls behind and several
    /// challengers tie for the lead, the card is set aside until one of them
    /// pulls ahead.
    fn recalculate(&mut self, players: &mut Players) {
        let leaders = self.leaders(players);

        let Some((best_value, leader_ids)) = leaders else {
            self.remove_holder(players);
            return;
        };

        if best_value < self.minimum_number() {
            self.remove_holder(players);
            return;
        }

        if let Some(holder) = self.holder() {
            if leader_ids.contains(&holder) {
                return;
            }
        }

        match leader_ids.as_slice() {
            [only] => self.transfer_holder(players, *only),
            _ => self.remove_holder(players),
        }
    }

    /// The highest score and everyone holding it.
    fn leaders(&self, players: &Players) -> Option<(usize, Vec<Uuid>)> {
        let scores: Vec<(Uuid, usize)> = (0..players.len())
            .filter_map(|idx| players.get_by_index(idx))
            .map(|player| (player.id, self.player_number(player)))
            .collect();

        let best_value = scores.iter().map(|(_, value)| *value).max()?;

        let leader_ids = scores
            .into_iter()
            .filter(|(_, value)| *value == best_value)
            .map(|(id, _)| id)
            .collect();

        Some((best_value, leader_ids))
    }

    fn transfer_holder(
        &mut self,
        players: &mut Players,
        new_holder: Uuid,
    ) {
        if let Some(old_holder) = self.holder() {
            if let Some(player) = players.get_mut(old_holder) {
                self.remove_points(player);
            }
        }

        if let Some(player) = players.get_mut(new_holder) {
            self.add_points(player);
        }

        self.set_holder(Some(new_holder));
    }

    fn remove_holder(&mut self, players: &mut Players) {
        if let Some(old_holder) = self.holder() {
            if let Some(player) = players.get_mut(old_holder) {
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
    use shared::PlayerColour;
    use uuid::Uuid;

    use crate::game::entities::player::Player;
    use crate::game::entities::players::Players;
    use crate::game::entities::bonus_points::{BiggestArmy, LongestRoad, BonusCard};

    fn players(ids: &[Uuid]) -> Players {
        let ps = ids
            .iter()
            .copied()
            .map(|id| p(id))
            .collect::<Vec<_>>();

        Players::new(ps)
    }

    fn p(id: Uuid) -> Player {
        Player::new(id, &format!("Player {}", id), PlayerColour::Blue)
    }

    /// Regression test: tying the holder used to strip the card from everyone.
    #[test]
    fn tying_the_holder_does_not_take_the_card_away() {
        let p1 = Uuid::from_u128(1);
        let p2 = Uuid::from_u128(2);
        let mut players = players(&[p1, p2]);

        players.get_mut(p1).unwrap().longest_road = 5;
        let mut road = LongestRoad::new();
        road.recalculate(&mut players);

        assert_eq!(road.holder(), Some(p1));
        assert_eq!(players.get(p1).unwrap().get_victory_points(), road.points());

        // p2 only matches p1, so p1 keeps both the card and the points.
        players.get_mut(p2).unwrap().longest_road = 5;
        road.recalculate(&mut players);

        assert_eq!(road.holder(), Some(p1), "a tie must not unseat the holder");
        assert_eq!(players.get(p1).unwrap().get_victory_points(), road.points());
        assert_eq!(players.get(p2).unwrap().get_victory_points(), 0);
    }

    #[test]
    fn beating_the_holder_transfers_the_card_and_the_points() {
        let p1 = Uuid::from_u128(1);
        let p2 = Uuid::from_u128(2);
        let mut players = players(&[p1, p2]);

        players.get_mut(p1).unwrap().longest_road = 5;
        let mut road = LongestRoad::new();
        road.recalculate(&mut players);

        players.get_mut(p2).unwrap().longest_road = 6;
        road.recalculate(&mut players);

        assert_eq!(road.holder(), Some(p2));
        assert_eq!(players.get(p1).unwrap().get_victory_points(), 0);
        assert_eq!(players.get(p2).unwrap().get_victory_points(), road.points());
    }

    /// If the holder is overtaken by two players at once, nobody holds it.
    #[test]
    fn holder_overtaken_by_tied_challengers_loses_the_card() {
        let p1 = Uuid::from_u128(1);
        let p2 = Uuid::from_u128(2);
        let p3 = Uuid::from_u128(3);
        let mut players = players(&[p1, p2, p3]);

        players.get_mut(p1).unwrap().longest_road = 5;
        let mut road = LongestRoad::new();
        road.recalculate(&mut players);
        assert_eq!(road.holder(), Some(p1));

        players.get_mut(p2).unwrap().longest_road = 7;
        players.get_mut(p3).unwrap().longest_road = 7;
        road.recalculate(&mut players);

        assert_eq!(road.holder(), None);
        assert_eq!(players.get(p1).unwrap().get_victory_points(), 0);
    }

    #[test]
    fn biggest_army_awarded_when_player_reaches_minimum_and_is_best() {
        let mut players = players(&[
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
        ]);

        players.get_mut(Uuid::from_u128(2)).unwrap().knight_played = 3;
        players.get_mut(Uuid::from_u128(1)).unwrap().knight_played = 2;

        let vp_before = players
            .get(Uuid::from_u128(2))
            .unwrap()
            .get_victory_points();

        let mut ba = BiggestArmy::new();
        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), Some(Uuid::from_u128(2)));
        assert_eq!(
            players.get(Uuid::from_u128(2)).unwrap().get_victory_points(),
            vp_before + ba.points()
        );
    }

    #[test]
    fn biggest_army_not_awarded() {
        let mut players = players(&[
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        ]);

        players.get_mut(Uuid::from_u128(1)).unwrap().knight_played = 2;
        players.get_mut(Uuid::from_u128(2)).unwrap().knight_played = 2;

        let mut ba = BiggestArmy::new();

        let p1_vp_before = players
            .get(Uuid::from_u128(1))
            .unwrap()
            .get_victory_points();
        let p2_vp_before = players
            .get(Uuid::from_u128(2))
            .unwrap()
            .get_victory_points();

        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), None);
        assert_eq!(
            players.get(Uuid::from_u128(1)).unwrap().get_victory_points(),
            p1_vp_before
        );
        assert_eq!(
            players.get(Uuid::from_u128(2)).unwrap().get_victory_points(),
            p2_vp_before
        );
    }

    #[test]
    fn biggest_army_transfers_points_when_holder_changes() {
        let mut players = players(&[
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        ]);

        players.get_mut(Uuid::from_u128(1)).unwrap().knight_played = 3;

        let mut ba = BiggestArmy::new();
        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), Some(Uuid::from_u128(1)));

        let p1_vp_after_award = players
            .get(Uuid::from_u128(1))
            .unwrap()
            .get_victory_points();

        players.get_mut(Uuid::from_u128(2)).unwrap().knight_played = 4;

        let p2_before = players
            .get(Uuid::from_u128(2))
            .unwrap()
            .get_victory_points();

        ba.recalculate(&mut players);

        assert_eq!(ba.holder(), Some(Uuid::from_u128(2)));
        assert_eq!(
            players.get(Uuid::from_u128(1)).unwrap().get_victory_points(),
            p1_vp_after_award - ba.points()
        );
        assert_eq!(
            players.get(Uuid::from_u128(2)).unwrap().get_victory_points(),
            p2_before + ba.points()
        );
    }

    #[test]
    fn longest_road_not_awarded_when_best_below_minimum() {
        let mut players = players(&[
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        ]);

        players.get_mut(Uuid::from_u128(1)).unwrap().longest_road = 4;
        players.get_mut(Uuid::from_u128(2)).unwrap().longest_road = 4;

        let mut lr = LongestRoad::new();

        let p1_vp_before = players
            .get(Uuid::from_u128(1))
            .unwrap()
            .get_victory_points();
        let p2_vp_before = players
            .get(Uuid::from_u128(2))
            .unwrap()
            .get_victory_points();

        lr.recalculate(&mut players);

        assert_eq!(lr.holder(), None);
        assert_eq!(
            players.get(Uuid::from_u128(1)).unwrap().get_victory_points(),
            p1_vp_before
        );
        assert_eq!(
            players.get(Uuid::from_u128(2)).unwrap().get_victory_points(),
            p2_vp_before
        );
    }

    #[test]
    fn longest_road_awarded_when_player_reaches_minimum_and_is_best() {
        let mut players = players(&[
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        ]);

        players.get_mut(Uuid::from_u128(1)).unwrap().longest_road = 5;
        players.get_mut(Uuid::from_u128(2)).unwrap().longest_road = 4;

        let p1_vp_before = players
            .get(Uuid::from_u128(1))
            .unwrap()
            .get_victory_points();
        let p2_vp_before = players
            .get(Uuid::from_u128(2))
            .unwrap()
            .get_victory_points();

        let mut lr = LongestRoad::new();
        lr.recalculate(&mut players);

        assert_eq!(lr.holder(), Some(Uuid::from_u128(1)));
        assert_eq!(
            players.get(Uuid::from_u128(1)).unwrap().get_victory_points(),
            p1_vp_before + lr.points()
        );
        assert_eq!(
            players.get(Uuid::from_u128(2)).unwrap().get_victory_points(),
            p2_vp_before
        );
    }
}
