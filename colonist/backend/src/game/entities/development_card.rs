use crate::game::entities::{bonus_points::BonusCard, resources::ResourceSet};
use crate::game::entities::turn_manager::TurnManager;
use shared::{DevCardTarget, DevCardType, ResourceType};
use std::fmt::Debug;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevCardState {
    pub bought_this_turn: bool,
    pub played: bool,
}

impl DevCardState {
    pub fn new() -> Self {
        Self {
            bought_this_turn: true,
            played: false,
        }
    }

    pub fn next_turn(&mut self) {
        self.bought_this_turn = false;
    }

    pub fn can_play(&self) -> bool {
        !self.bought_this_turn && !self.played
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevelopmentCard {
    Knight(DevCardState),
    VictoryPoint(DevCardState),
    RoadBuilder(DevCardState),
    YearOfPlenty(DevCardState),
    Monopoly(DevCardState),
}

impl DevelopmentCard {
    pub fn new(card_type: DevCardType) -> Self {
        let state = DevCardState::new();
        match card_type {
            DevCardType::Knight => Self::Knight(state),
            DevCardType::VictoryPoint => Self::VictoryPoint(state),
            DevCardType::RoadBuilding => Self::RoadBuilder(state),
            DevCardType::YearOfPlenty => Self::YearOfPlenty(state),
            DevCardType::Monopoly => Self::Monopoly(state),
        }
    }

    pub fn cost() -> ResourceSet {
        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Sheep, 1);
        cost.add(ResourceType::Wheat, 1);
        cost.add(ResourceType::Ore, 1);
        cost
    }

    pub fn can_play(&self) -> bool {
        match self {
            Self::VictoryPoint(_) => false, // Victory Point cards are never playable - they automatically count as VP
            Self::Knight(s) | Self::RoadBuilder(s) | Self::YearOfPlenty(s) | Self::Monopoly(s) => s.can_play(),
        }
    }

    pub fn next_turn(&mut self) {
        match self {
            Self::Knight(s) | Self::RoadBuilder(s) | Self::YearOfPlenty(s) | Self::Monopoly(s) => s.next_turn(),
            Self::VictoryPoint(_) => {},
        }
    }

    pub fn play(&mut self, game: &mut TurnManager, _target: &Option<DevCardTarget>) {
        if !self.can_play() {
            return;
        }

        match self {
            Self::Knight(s) => {
                let pid = game.players.get_current_player().id;
                if let Some(player) = game.players.get_mut(pid) {
                    player.knight_played += 1;
                }
                game.army_bonus.recalculate(&mut game.players);
                s.played = true;
            }
            Self::VictoryPoint(_) => {
                unreachable!("Victory Point cards should never be played");
            }
            Self::RoadBuilder(s) => {
                s.played = true;
            }
            Self::YearOfPlenty(s) => {
                s.played = true;
            }
            Self::Monopoly(s) => {
                s.played = true;
            }
        }
    }

    pub fn get_type(&self) -> DevCardType {
        match self {
            Self::Knight(_) => DevCardType::Knight,
            Self::VictoryPoint(_) => DevCardType::VictoryPoint,
            Self::RoadBuilder(_) => DevCardType::RoadBuilding,
            Self::YearOfPlenty(_) => DevCardType::YearOfPlenty,
            Self::Monopoly(_) => DevCardType::Monopoly,
        }
    }
}





#[cfg(test)]
mod tests {
    use shared::DevCardType;

    use crate::game::entities::development_card::{
        DevelopmentCard, DevCardState,
    };
    use crate::game::entities::resources::ResourceType;

    #[test]
    fn dev_card_trait_cost_is_standard_cost() {
        let cost = DevelopmentCard::cost();

        assert_eq!(cost.amount_of(ResourceType::Sheep), 1);
        assert_eq!(cost.amount_of(ResourceType::Wheat), 1);
        assert_eq!(cost.amount_of(ResourceType::Ore), 1);
        assert_eq!(cost.amount_of(ResourceType::Wood), 0);
        assert_eq!(cost.amount_of(ResourceType::Brick), 0);
    }

    #[test]
    fn dev_card_state_new_is_not_playable_same_turn() {
        let s = DevCardState::new();
        assert!(s.bought_this_turn);
        assert!(!s.played);
        assert!(!s.can_play());
    }

    #[test]
    fn dev_card_state_next_turn_makes_playable_if_not_played() {
        let mut s = DevCardState::new();
        s.next_turn();
        assert!(!s.bought_this_turn);
        assert!(!s.played);
        assert!(s.can_play());
    }

    #[test]
    fn knight_glyph_and_can_play_follows_state() {
        let mut k = DevelopmentCard::new(DevCardType::Knight);
        assert!(!k.can_play(), "bought_this_turn should block play");

        k.next_turn();
        assert!(k.can_play());
    }

    #[test]
    fn victory_point_glyph_and_can_play_follows_state() {
        let mut v = DevelopmentCard::new(DevCardType::VictoryPoint);
        assert!(!v.can_play());
        v.next_turn();
        assert!(!v.can_play(), "victory points cannot be played");
    }

    #[test]
    fn road_builder_glyph_and_can_play_follows_state() {
        let mut r = DevelopmentCard::new(DevCardType::RoadBuilding);
        assert!(!r.can_play());
        r.next_turn();
        assert!(r.can_play());
    }

    #[test]
    fn year_of_plenty_glyph_and_can_play_follows_state() {
        let mut y = DevelopmentCard::new(DevCardType::YearOfPlenty);
        assert!(!y.can_play());
        y.next_turn();
        assert!(y.can_play());
    }

    #[test]
    fn monopoly_glyph_and_can_play_follows_state() {
        let mut m = DevelopmentCard::new(DevCardType::Monopoly);
        assert!(!m.can_play());
        m.next_turn();
        assert!(m.can_play());
    }
}