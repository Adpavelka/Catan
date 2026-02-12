use std::collections::{HashMap, HashSet};
use crate::errors::GameError;
use crate::game::entities::board::{Board, Coordinates, PortType};
use crate::game::entities::development_card::DevelopmentCard::Knight;
use crate::game::entities::development_card::{DevCardState, DevelopmentCard};
use crate::game::entities::player::Player;
use crate::game::entities::players::Players;
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
    game_resources: ResourceSet,
    dev_cards: Vec<DevelopmentCard>,
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
        for _ in 0..2000 {
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
            game_resources: resources,
            dev_cards,
        }
    }

    pub fn give_resources_for_roll(&mut self, board: &Board, dice_number: u8, robber: &Robber, players: &mut Players) -> Vec<(Uuid, ResourceType, u32)> {
        let mut pending: Vec<(Uuid, ResourceType, u32)> = Vec::new();
        let mut totals: HashMap<ResourceType, u32> = HashMap::new();

        for hex in board.hexes.values() {
            if hex.number != dice_number || hex.resource == ResourceType::Desert || robber.get_pos() == hex.coord {
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
                    players,
                )
                .is_ok()
            {
                distributed.push((player_id, res, amount));
            }
        }

        distributed
    }


    pub fn give_initial_settlement_resources(&mut self, board: &Board, player_id: Uuid, settlement_pos: Coordinates, players: &mut Players,) {
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
                players,
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


    pub fn validate_bank_trade(&self, player: &Player, gives: &ResourceSet, takes: &ResourceSet) -> Result<(), GameError> {
        if gives.get_cards_total() == 0 || takes.get_cards_total() == 0 {
            return Err(GameError::WrongResourceRatio);
        }

        if !player.can_pay(gives) {
            return Err(GameError::NotEnoughResources);
        }

        if !self.game_resources.can_pay(takes) {
            return Err(GameError::NotEnoughResources);
        }

        let mut produced_units = 0u32;

        for res in [
            ResourceType::Brick,
            ResourceType::Ore,
            ResourceType::Sheep,
            ResourceType::Wheat,
            ResourceType::Wood,
        ] {
            let amount = gives.amount_of(res);

            if amount == 0 {
                continue;
            }

            let ratio = self.best_ratio(player, res);

            if amount % ratio != 0 {
                return Err(GameError::WrongResourceRatio);
            }

            produced_units += amount / ratio;
        }

        let takes_total: u32 = takes.get_cards_total();

        if produced_units != takes_total {
            return Err(GameError::WrongResourceRatio);
        }

        Ok(())
    }


    pub fn trade_with_bank(&mut self, pid: Uuid, gives: ResourceSet, takes: ResourceSet, players: &mut Players, validate: bool) -> Result<(), GameError> {
        if validate {
            self.validate_bank_trade(players.get(pid).unwrap(), &gives, &takes)?;
        }

        self.collect_from_to(
            ResourceEndpoint::Player(pid),
            ResourceEndpoint::Bank,
            &gives,
            players,
        )?;

        self.collect_from_to(
            ResourceEndpoint::Bank,
            ResourceEndpoint::Player(pid),
            &takes,
            players,
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

    pub fn collect_from_player(&mut self, player_id: Uuid, cost: ResourceSet, players: &mut Players) -> Result<(), GameError> {
        self.collect_from_to(
            ResourceEndpoint::Player(player_id),
            ResourceEndpoint::Bank,
            &cost,
            players,
        )
    }

    pub fn collect_from_player_to_player(&mut self, from_id: Uuid, to_id: Uuid, cost: &ResourceSet, players: &mut Players) -> Result<(), GameError> {
        self.collect_from_to(
            ResourceEndpoint::Player(from_id),
            ResourceEndpoint::Player(to_id),
            cost,
            players,
        )
    }

    pub fn collect_resource_from_all_to_player(&mut self, to_id: Uuid, resource: ResourceType, players: &mut Players) -> Result<u32, GameError> {
        let mut transfers: Vec<(Uuid, u32)> = Vec::new();
        for idx in 0..players.len() {
            let player = players.get_by_index(idx).unwrap();

            if player.id == to_id {
                continue;
            }

            let amount = player.resources.amount_of(resource);
            if amount > 0 {
                transfers.push((player.id, amount));
            }
        }

        let mut total_stolen = 0u32;
        for (from_id, amt) in transfers {
            let mut cost = ResourceSet::new();
            cost.add(resource, amt);
            self.collect_from_player_to_player(from_id, to_id, &cost, players)?;
            total_stolen += amt;
        }

        Ok(total_stolen)
    }

    fn collect_from_to(&mut self, from: ResourceEndpoint, to: ResourceEndpoint, cost: &ResourceSet, players: &mut Players) -> Result<(), GameError> {
        if from == to {
            return Ok(());
        }

        match from {
            ResourceEndpoint::Player(id) => {
                let player = players
                                        .get(id)
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

        match to {
            ResourceEndpoint::Player(id) => {
                let _ = players
                    .get(id)
                    .ok_or(GameError::PlayerNotFound)?;
            }
            _ => {}
        }


        match from {
            ResourceEndpoint::Player(id) => {
                let player = players.get_mut(id).unwrap();
                player.pay(cost);
            }
            ResourceEndpoint::Bank => {
                self.game_resources.take_set(cost);
            }
        }

        match to {
            ResourceEndpoint::Player(id) => {
                let player = players.get_mut(id).unwrap();
                player.resources.add_set(cost);
            }
            ResourceEndpoint::Bank => {
                self.game_resources.add_set(cost);
            }
        }

        Ok(())
    }

    pub fn draw_dev_card(&mut self) -> Result<DevelopmentCard, GameError> {
        self.dev_cards
            .pop()
            .ok_or(GameError::InvalidAction)
    }
}





















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

    fn players(ps: Vec<Player>) -> Players {
        Players::new(ps)
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
    fn give_resources_for_roll_pays_settlement() {
        let mut bank = Bank::new();
        let mut board = Board::new();

        let player_id = pid(1);
        let mut players = players(vec![
            Player::new(player_id, "A", 'A')
        ]);

        let (_, dice, res, vertex) = {
            let hex = board.hexes.values()
                .find(|h| h.resource != ResourceType::Desert)
                .unwrap();
            (hex.coord, hex.number, hex.resource, hex.adjacent_vertices[0])
        };

        let v = board.vertices.get_mut(&vertex).unwrap();
        v.owner = Some(player_id);
        v.building = Some(VertexBuilding::Settlement);

        let robber = Robber::new(&board);

        let before = players
            .get(player_id)
            .unwrap()
            .resources
            .amount_of(res);

        let distributed = bank.give_resources_for_roll(
            &board,
            dice,
            &robber,
            &mut players,
        );

        let after = players
            .get(player_id)
            .unwrap()
            .resources
            .amount_of(res);

        assert_eq!(after - before, 1);
        assert_eq!(distributed, vec![(player_id, res, 1)]);
    }


    #[test]
    fn give_resources_for_roll_blocked_by_robber() {
        let mut bank = Bank::new();
        let mut board = Board::new();

        let player_id = pid(1);
        let mut players = players(vec![
            Player::new(player_id, "A", 'A')
        ]);

        let (hex_coord, dice, res, vertex) = {
            let hex = board.hexes.values()
                .find(|h| h.resource != ResourceType::Desert)
                .unwrap();
            (hex.coord, hex.number, hex.resource, hex.adjacent_vertices[0])
        };

        let v = board.vertices.get_mut(&vertex).unwrap();
        v.owner = Some(player_id);
        v.building = Some(VertexBuilding::Settlement);

        let mut robber = Robber::new(&board);
        robber.move_to(hex_coord).unwrap();

        let distributed = bank.give_resources_for_roll(
            &board,
            dice,
            &robber,
            &mut players,
        );

        assert!(distributed.is_empty());
        assert_eq!(
            players.get(player_id).unwrap().resources.amount_of(res),
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

        let mut players = players(vec![p1, p2]);

        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Wood, 2);

        bank.collect_from_player_to_player(
            from,
            to,
            &cost,
            &mut players,
        ).unwrap();

        assert_eq!(
            players.get(from).unwrap().resources.amount_of(ResourceType::Wood),
            1
        );
        assert_eq!(
            players.get(to).unwrap().resources.amount_of(ResourceType::Wood),
            2
        );
    }


    #[test]
    fn collect_resource_from_all_to_player_collects_everything() {
        let mut bank = Bank::new();

        let target = pid(0);

        let mut ps = vec![Player::new(target, "T", 'T')];

        for i in 1..=3 {
            let mut p = Player::new(pid(i), "P", 'A');
            p.resources.add(ResourceType::Ore, i as u32);
            ps.push(p);
        }

        let mut players = players(ps);

        let stolen = bank
            .collect_resource_from_all_to_player(
                target,
                ResourceType::Ore,
                &mut players,
            )
            .unwrap();

        assert_eq!(stolen, 6);
        assert_eq!(
            players.get(target).unwrap().resources.amount_of(ResourceType::Ore),
            6
        );
    }


    #[test]
    fn collect_from_player_to_player_returns_err_when_target_missing_and_source_is_restored() {
        let mut bank = Bank::new();

        let pid1 = Uuid::from_u128(1);
        let pid2 = Uuid::from_u128(2); // nebude existovat

        let mut from = Player::new(pid1, "From", 'A');
        from.resources.add(ResourceType::Brick, 2);

        let mut players = Players::new(vec![from]);

        let mut cost = ResourceSet::new();
        cost.add(ResourceType::Brick, 1);

        let from_brick_before = players
            .get(pid1)
            .unwrap()
            .resources
            .amount_of(ResourceType::Brick);

        let err = bank
            .collect_from_player_to_player(pid1, pid2, &cost, &mut players)
            .unwrap_err();

        assert_eq!(err, GameError::PlayerNotFound);

        let from_after = players.get(pid1).unwrap();
        assert_eq!(
            from_after.resources.amount_of(ResourceType::Brick),
            from_brick_before,
            "source player must not lose resources when transfer fails"
        );
    }

}

