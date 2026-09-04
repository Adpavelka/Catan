use serde::{Deserialize, Serialize};
use uuid::Uuid;
use shared::{PlayerInfo, Resources};
use crate::errors::GameError;
use crate::game::entities::board::PortType;
use crate::game::entities::development_card::DevelopmentCard;
use crate::game::entities::resources::{ResourceSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: Uuid,
    pub name: String,
    pub resources: ResourceSet, // TODO Private
    
    pub colour: char,

    settlements_left: u8,
    cities_left: u8,
    roads_left: u8,

    pub dev_cards: Vec<DevelopmentCard>, // TODO Encapsulate
    pub dev_card_played_this_turn: bool,
    pub knight_played: usize,
    pub longest_road: usize,


    victory_points: u8,
    secret_victory_points: u8,

    pub ports: Vec<PortType>, // TODO: why public?
}

impl Player {
    pub fn new(id: Uuid, name: &str, colour: char) -> Self {
        Self {
            id,
            name: name.to_string(),
            resources: ResourceSet::new(),
            colour,
            settlements_left: 5, // TODO: global constants file
            cities_left: 4,
            roads_left: 15,
            dev_cards: Vec::new(),
            knight_played: 0,
            longest_road: 0,
            victory_points: 0,
            ports: Vec::new(),
            dev_card_played_this_turn: false,
            secret_victory_points: 0,
        }
    }

    pub fn add_victory_point(&mut self) {
        self.victory_points += 1;
    }
    
    pub fn add_secret_victory_point(&mut self) {
        self.secret_victory_points += 1;
    }

    /// Road Building may be played with a single road left; the player simply
    /// places fewer than two.
    pub fn can_play_road_builder(&self) -> bool {
        self.roads_left >= 1
    }

    pub fn roads_left(&self) -> u8 {
        self.roads_left
    }

    // useless????
    pub fn get_total_victory_points(&self) -> u8 {
        self.victory_points + self.secret_victory_points
    }

    pub fn get_secret_victory_points(&self) -> u8 {
        self.secret_victory_points
    }

    pub fn remove_victory_point(&mut self) {
        self.victory_points -= 1;
    }

    pub fn get_victory_points(&self) -> u8 {
        self.victory_points
    }

    pub fn can_pay(&self, cost: &ResourceSet) -> bool {
        self.resources.can_pay(cost)
    }

    pub fn pay(&mut self, cost: &ResourceSet) -> bool {
        if !self.can_pay(cost) {
            return false;
        }
        self.resources.take_set(cost);
        true
    }

    fn has_settlement(&self) -> bool {
        self.settlements_left > 0
    }

    fn has_city(&self) -> bool {
        self.cities_left > 0
    }

    fn has_road(&self) -> bool {
        self.roads_left > 0
    }


    pub fn use_settlement(&mut self) -> Result<(), GameError> {
        if !self.has_settlement() {
            return Err(GameError::NotEnoughBuildingsOfThisType);
        }
        self.settlements_left -= 1;
        self.add_victory_point();
        Ok(())
    }

    pub fn use_city(&mut self) -> Result<(), GameError> {
        if !self.has_city() {
            return Err(GameError::NotEnoughBuildingsOfThisType);
        }
        self.cities_left -= 1;
        self.settlements_left += 1;
        self.add_victory_point();
        Ok(())
    }

    pub fn use_road(&mut self) -> Result<(), GameError> {
        if !self.has_road() {
            return Err(GameError::NotEnoughBuildingsOfThisType);
        }
        self.roads_left -= 1;
        Ok(())
    }

}
impl From<&Player> for PlayerInfo {
    fn from(p: &Player) -> Self {
        PlayerInfo {
            player_id: p.id,
            name: p.name.clone(),
            color: p.colour.to_string(),
            victory_points: p.get_victory_points(),
            dev_cards: p.dev_cards.iter().map(|c| c.get_type()).collect(),
            ports: p.ports.iter().map(|port| port.into()).collect(),
            resources: Resources::from(&p.resources),
            knights_played: p.knight_played,
            roads_count: p.longest_road,
            has_longest_road: false,  // Cannot determine without TurnManager context
            has_largest_army: false,  // Cannot determine without TurnManager context
        }
    }
}




#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::game::entities::player::Player;
    use crate::game::entities::resources::{ResourceType, ResourceSet};
    use crate::errors::GameError;

    #[test]
    fn player_new_initializes_defaults() {
        let p = Player::new(Uuid::from_u128(42), "Alice", 'A');

        assert_eq!(p.id, Uuid::from_u128(42));
        assert_eq!(p.name, "Alice");
        assert_eq!(p.colour, 'A');

        assert_eq!(p.get_victory_points(), 0);
        assert_eq!(p.knight_played, 0);
        assert_eq!(p.longest_road, 0);

        assert!(p.has_settlement());
        assert!(p.has_city());
        assert!(p.has_road());

        assert_eq!(p.resources.amount_of(ResourceType::Wood), 0);
        assert_eq!(p.resources.amount_of(ResourceType::Brick), 0);
        assert_eq!(p.resources.amount_of(ResourceType::Sheep), 0);
        assert_eq!(p.resources.amount_of(ResourceType::Wheat), 0);
        assert_eq!(p.resources.amount_of(ResourceType::Ore), 0);

        assert!(p.dev_cards.is_empty());
    }

    /// Road Building is playable with a single road left; the rules do not
    /// require being able to place both.
    #[test]
    fn road_builder_playable_with_one_road_left() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');

        for _ in 0..14 {
            p.use_road().unwrap();
        }
        assert_eq!(p.roads_left(), 1);
        assert!(p.can_play_road_builder());

        p.use_road().unwrap();
        assert_eq!(p.roads_left(), 0);
        assert!(!p.can_play_road_builder(), "no roads left means nothing to place");
    }

    #[test]
    fn add_victory_point_increments() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');
        assert_eq!(p.get_victory_points(), 0);
        p.add_victory_point();
        assert_eq!(p.get_victory_points(), 1);
        p.add_victory_point();
        assert_eq!(p.get_victory_points(), 2);
    }

    #[test]
    fn can_pay_and_pay_succeeds_and_deducts_resources() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');
        p.resources.add(ResourceType::Wood, 2);
        p.resources.add(ResourceType::Brick, 1);

        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Wood, 2);
        cost.add(ResourceType::Brick, 1);
        assert!(p.can_pay(&cost));

        let ok = p.pay(&cost);
        assert!(ok);

        assert_eq!(p.resources.amount_of(ResourceType::Wood), 0);
        assert_eq!(p.resources.amount_of(ResourceType::Brick), 0);
    }

    #[test]
    fn pay_fails_when_cannot_pay_and_changes_nothing() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');
        p.resources.add(ResourceType::Wood, 1);

        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Wood, 2);
        assert!(!p.can_pay(&cost));

        let before = p.resources.clone();
        let ok = p.pay(&cost);
        assert!(!ok);

        assert_eq!(p.resources, before);
    }

    #[test]
    fn use_settlement_decrements_available_and_adds_victory_point() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');

        let vp_before = p.get_victory_points();
        p.use_settlement().expect("should have settlements");

        assert_eq!(p.get_victory_points(), vp_before + 1);

        // Use all remaining settlements (total 5)
        for _ in 0..4 {
            p.use_settlement().expect("should still have settlements");
        }
        assert!(!p.has_settlement());

        let err = p.use_settlement().unwrap_err();
        assert_eq!(err, GameError::NotEnoughBuildingsOfThisType);
    }

    #[test]
    fn use_city_decrements_cities_increments_settlements_and_adds_victory_point() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');

        let vp_before = p.get_victory_points();
        // Use all settlements so we can observe that using a city increases settlements_left by 1.
        for _ in 0..5 {
            p.use_settlement().unwrap();
        }
        assert!(!p.has_settlement());

        p.use_city().expect("should have cities");
        assert_eq!(p.get_victory_points(), vp_before + 6, "5 settlements + 1 city = +6 VP");
        assert!(p.has_settlement(), "using a city should add one settlement back");

        // Use remaining cities (started 4, used 1 already)
        for _ in 0..3 {
            p.use_city().expect("should still have cities");
        }
        assert!(!p.has_city());

        let err = p.use_city().unwrap_err();
        assert_eq!(err, GameError::NotEnoughBuildingsOfThisType);
    }

    #[test]
    fn use_road_decrements_roads_and_errors_when_none_left() {
        let mut p = Player::new(Uuid::from_u128(1), "P", 'A');

        // Start: 15 roads => has_road and
        assert!(p.has_road());

        // Use 14 roads => 1 left
        for _ in 0..14 {
            p.use_road().expect("should have roads");
        }
        assert!(p.has_road());

        // Use last road => 0 left
        p.use_road().expect("should have last road");
        assert!(!p.has_road());

        let err = p.use_road().unwrap_err();
        assert_eq!(err, GameError::NotEnoughBuildingsOfThisType);
    }
}