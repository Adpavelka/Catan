use std::collections::{HashMap, HashSet};
use crate::errors::GameError;
use crate::game::entities::board::{Board, Coordinates, PortType};
use crate::game::entities::development_card::DevelopmentCard::Knight;
use crate::game::entities::development_card::{DevCardState, DevelopmentCard};
use crate::game::entities::player::Player;
use crate::game::entities::resources::ResourceSet;
use crate::game::entities::robber::Robber;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uuid::Uuid;
use shared::ResourceType;
const RESOURCES: u32 = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceEndpoint {
    Player(Uuid),
    Bank,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Bank {
    #[serde_as(as = "Vec<(_, _)>")]
    pub players: HashMap<Uuid, Player>,
    pub game_resources: ResourceSet,
    pub dev_cards: Vec<DevelopmentCard>,
}

impl Bank {
    pub fn new() -> Self {
        let mut resources = ResourceSet::new();
        resources.add(ResourceType::Brick, RESOURCES);
        resources.add(ResourceType::Ore, RESOURCES);
        resources.add(ResourceType::Sheep, RESOURCES);
        resources.add(ResourceType::Wheat, RESOURCES);
        resources.add(ResourceType::Wood, RESOURCES);

        let mut dev_cards: Vec<DevelopmentCard> = Vec::new();
        for _ in 0..14 {
            dev_cards.push(Knight(DevCardState::new()));
        }
        for _ in 0..5 {
            dev_cards.push(DevelopmentCard::VictoryPoint(DevCardState::new()));
        }
        for _ in 0..2 {
            dev_cards.push(DevelopmentCard::RoadBuilder(DevCardState::new()));
        }
        for _ in 0..2 {
            dev_cards.push(DevelopmentCard::Monopoly(DevCardState::new()));
        }
        for _ in 0..2 {
            dev_cards.push(DevelopmentCard::YearOfPlenty(DevCardState::new()));
        }

        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        dev_cards.shuffle(&mut rng);

        Self {
            players: HashMap::new(),
            game_resources: resources,
            dev_cards,
        }
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.id, player);
    }

    pub fn give_resources_for_roll(
        &mut self,
        board: &Board,
        dice_number: u8,
        robber: &Robber,
    ) -> Vec<(Uuid, ResourceType, u32)> {
        let mut pending: Vec<(Uuid, ResourceType, u32)> = Vec::new();
        let mut totals: HashMap<ResourceType, u32> = HashMap::new();

        for hex in board.hexes.values() {
            if hex.number != dice_number
                || hex.resource == ResourceType::Desert
                || robber.pos == hex.coord
            {
                continue;
            }

            for &vcoord in &hex.adjacent_vertices {
                let Some(vertex) = board.vertices.get(&vcoord) else { continue };
                let Some(building) = &vertex.building else { continue };
                let Some(player_id) = vertex.owner else { continue };

                let amount = building.production();
                pending.push((player_id, hex.resource, amount));
                *totals.entry(hex.resource).or_insert(0) += amount;
            }
        }

        let can_distribute: HashSet<ResourceType> = totals
            .iter()
            .filter(|(res, needed)| {
                let available = self.game_resources.amount_of(**res);
                if available < **needed {
                    log::warn!(
                        "Bank does not have enough {:?}: needed {}, available {}",
                        res,
                        needed,
                        available
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(res, _)| *res)
            .collect();

        let mut distributed = Vec::new();

        for (player_id, res, amount) in pending {
            if !can_distribute.contains(&res) {
                continue;
            }

            let mut cost = ResourceSet::new();
            cost.add(res, amount);

            if self
                .collect_from_to(
                    ResourceEndpoint::Bank,
                    ResourceEndpoint::Player(player_id),
                    &cost,
                )
                .is_ok()
            {
                distributed.push((player_id, res, amount));
            }
        }

        distributed
    }


    pub fn give_initial_settlement_resources(
        &mut self,
        board: &Board,
        player_id: Uuid,
        settlement_pos: Coordinates,
    ) {
        use log::info;

        info!(
            "Player {} placing second settlement at {:?}, distributing initial resources",
            player_id, settlement_pos
        );

        let mut resources_given = 0;

        for hex in board.hexes.values() {
            if !hex.adjacent_vertices.contains(&settlement_pos) {
                continue;
            }

            if hex.resource == ResourceType::Desert {
                info!("  - Skipping desert hex at {:?}", hex.coord);
                continue;
            }

            let mut cost = ResourceSet::new();
            cost.add(hex.resource, 1);

            match self.collect_from_to(
                ResourceEndpoint::Bank,
                ResourceEndpoint::Player(player_id),
                &cost,
            ) {
                Ok(_) => {
                    resources_given += 1;
                    info!("  - Gave 1 {:?} from hex {:?}", hex.resource, hex.coord);
                }
                Err(_) => {
                    info!(
                        "  - FAILED: Bank out of {:?} for hex {:?}",
                        hex.resource, hex.coord
                    );
                }
            }
        }

        info!(
            "Player {} received {} resources total from second settlement",
            player_id, resources_given
        );
    }


    pub fn validate_bank_trade(
        &self,
        player: &Player,
        gives: &ResourceSet,
        takes: &ResourceSet,
    ) -> Result<(), GameError> {
        if gives.amounts.is_empty() || takes.amounts.is_empty() {
            return Err(GameError::WrongResourceRatio);
        }

        if !player.can_pay(gives) {
            return Err(GameError::NotEnoughResources);
        }

        if !self.game_resources.can_pay(takes) {
            return Err(GameError::NotEnoughResources);
        }

        let mut produced_units = 0u32;

        for (res, amount) in &gives.amounts {
            let ratio = self.best_ratio(player, *res);

            if *amount == 0 || amount % ratio != 0 {
                return Err(GameError::WrongResourceRatio);
            }

            produced_units += amount / ratio;
        }

        let takes_total: u32 = takes.amounts.values().sum();

        if produced_units != takes_total {
            return Err(GameError::WrongResourceRatio);
        }

        Ok(())
    }


    pub fn trade_with_bank(
        &mut self,
        player_id: Uuid,
        gives: ResourceSet,
        takes: ResourceSet,
    ) -> Result<(), GameError> {
        let player = self
            .players
            .get(&player_id)
            .ok_or(GameError::PlayerNotFound)?;

        self.validate_bank_trade(player, &gives, &takes)?;

        self.collect_from_to(
            ResourceEndpoint::Player(player_id),
            ResourceEndpoint::Bank,
            &gives,
        )?;

        self.collect_from_to(
            ResourceEndpoint::Bank,
            ResourceEndpoint::Player(player_id),
            &takes,
        )?;

        Ok(())
    }


    fn best_ratio(&self, player: &Player, res: ResourceType) -> u32 {
        let mut ratio = 4;

        for port in &player.ports {
            match port {
                PortType::ThreeToOne => ratio = ratio.min(3),
                PortType::TwoToOne(r) if *r == res => return 2,
                _ => {}
            }
        }

        ratio
    }

    pub fn collect_from_player(
        &mut self,
        player_id: Uuid,
        cost: ResourceSet,
    ) -> Result<(), GameError> {
        self.collect_from_to(
            ResourceEndpoint::Player(player_id),
            ResourceEndpoint::Bank,
            &cost,
        )
    }

    pub fn collect_from_player_to_player(
        &mut self,
        from_id: Uuid,
        to_id: Uuid,
        cost: &ResourceSet,
    ) -> Result<(), GameError> {
        self.collect_from_to(
            ResourceEndpoint::Player(from_id),
            ResourceEndpoint::Player(to_id),
            cost,
        )
    }

    pub fn collect_resource_from_all_to_player(&mut self, to_id: Uuid, resource: ResourceType) -> Result<u32, GameError> {
        let transfers: Vec<(Uuid, u32)> = self
            .players
            .iter()
            .filter(|(player_id, _)| *player_id != &to_id)
            .map(|(&player_id, player)| (player_id, player.resources.amount_of(resource)))
            .filter(|(_, amt)| *amt > 0)
            .collect();

        let mut total_stolen = 0u32;
        for (from_id, amt) in transfers {
            let mut cost = ResourceSet::new();
            cost.add(resource, amt);
            self.collect_from_player_to_player(from_id, to_id, &cost)?;
            total_stolen += amt;
        }

        Ok(total_stolen)
    }

    pub fn collect_from_to(
        &mut self,
        from: ResourceEndpoint,
        to: ResourceEndpoint,
        cost: &ResourceSet,
    ) -> Result<(), GameError> {
        if from == to {
            return Ok(());
        }

        match from {
            ResourceEndpoint::Player(id) => {
                let player = self
                    .players
                    .get(&id)
                    .ok_or(GameError::PlayerNotFound)?;

                if !player.can_pay(cost) {
                    return Err(GameError::NotEnoughResources);
                }
            }
            ResourceEndpoint::Bank => {
                if !self.game_resources.can_pay(cost) {
                    return Err(GameError::BankOutOfResources);
                }
            }
        }


        match from {
            ResourceEndpoint::Player(id) => {
                let player = self.players.get_mut(&id).unwrap();
                player.pay(cost);
            }
            ResourceEndpoint::Bank => {
                self.game_resources.take_set(cost);
            }
        }

        match to {
            ResourceEndpoint::Player(id) => {
                let player = self.players.get_mut(&id).unwrap();
                player.resources.add_set(cost);
            }
            ResourceEndpoint::Bank => {
                self.game_resources.add_set(cost);
            }
        }

        Ok(())
    }
}

// teeeeeeest

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entities::board::Board;
    use crate::game::entities::building::VertexBuilding;
    use crate::game::entities::player::Player;
    use crate::game::entities::robber::Robber;
    use shared::ResourceType;
    use uuid::Uuid;

    fn pid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn bank_new_initializes_resources_and_dev_cards() {
        let bank = Bank::new();

        for r in [
            ResourceType::Brick,
            ResourceType::Ore,
            ResourceType::Sheep,
            ResourceType::Wheat,
            ResourceType::Wood,
        ] {
            assert_eq!(bank.game_resources.amount_of(r), 19);
        }

        assert_eq!(bank.dev_cards.len(), 25);

        let knights = bank.dev_cards.iter().filter(|c| matches!(c, DevelopmentCard::Knight(_))).count();
        assert_eq!(knights, 14);
    }

    #[test]
    fn add_player_inserts_player() {
        let mut bank = Bank::new();
        let p = Player::new(pid(1), "Alice", 'A');

        bank.add_player(p);

        assert!(bank.players.contains_key(&pid(1)));
    }

    #[test]
    fn give_resources_for_roll_pays_settlement() {
        let mut bank = Bank::new();
        let mut board = Board::new_standard_board();

        let player_id = pid(1);
        bank.add_player(Player::new(player_id, "A", 'A'));

        let (_, dice, res, vertex) = {
            let hex = board.hexes.values()
                .find(|h| h.resource != ResourceType::Desert)
                .unwrap();
            (hex.coord, hex.number, hex.resource, hex.adjacent_vertices[0])
        };

        let v = board.vertices.get_mut(&vertex).unwrap();
        v.owner = Some(player_id);
        v.building = Some(VertexBuilding::Settlement);

        let robber = Robber { pos: (999, 999) };

        let before = bank.players.get(&player_id).unwrap().resources.amount_of(res);

        let distributed = bank.give_resources_for_roll(&board, dice, &robber);

        let after = bank.players.get(&player_id).unwrap().resources.amount_of(res);

        assert_eq!(after - before, 1);
        assert_eq!(distributed.len(), 1);
        assert_eq!(distributed[0], (player_id, res, 1));
    }

    #[test]
    fn give_resources_for_roll_blocked_by_robber() {
        let mut bank = Bank::new();
        let mut board = Board::new_standard_board();

        let player_id = pid(1);
        bank.add_player(Player::new(player_id, "A", 'A'));

        let (hex_coord, dice, res, vertex) = {
            let hex = board.hexes.values()
                .find(|h| h.resource != ResourceType::Desert)
                .unwrap();
            (hex.coord, hex.number, hex.resource, hex.adjacent_vertices[0])
        };

        let v = board.vertices.get_mut(&vertex).unwrap();
        v.owner = Some(player_id);
        v.building = Some(VertexBuilding::Settlement);

        let robber = Robber { pos: hex_coord };

        let distributed = bank.give_resources_for_roll(&board, dice, &robber);

        assert!(distributed.is_empty());
        assert_eq!(
            bank.players.get(&player_id).unwrap().resources.amount_of(res),
            0
        );
    }


    #[test]
    fn collect_from_player_to_player_transfers_resources() {
        let mut bank = Bank::new();

        let from = pid(1);
        let to = pid(2);

        let mut p1 = Player::new(from, "From", 'A');
        p1.resources.add(ResourceType::Wood, 3);

        let p2 = Player::new(to, "To", 'B');

        bank.add_player(p1);
        bank.add_player(p2);

        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Wood, 2);

        bank.collect_from_player_to_player(from, to, &cost).unwrap();

        assert_eq!(bank.players.get(&from).unwrap().resources.amount_of(ResourceType::Wood), 1);
        assert_eq!(bank.players.get(&to).unwrap().resources.amount_of(ResourceType::Wood), 2);
    }

    #[test]
    fn collect_resource_from_all_to_player_collects_everything() {
        let mut bank = Bank::new();

        let target = pid(0);
        bank.add_player(Player::new(target, "T", 'T'));

        for i in 1..=3 {
            let mut p = Player::new(pid(i), "P", 'A');
            p.resources.add(ResourceType::Ore, i as u32);
            bank.add_player(p);
        }

        let stolen = bank
            .collect_resource_from_all_to_player(target, ResourceType::Ore)
            .unwrap();

        assert_eq!(stolen, 6);
        assert_eq!(
            bank.players.get(&target).unwrap().resources.amount_of(ResourceType::Ore),
            6
        );
    }
}

