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
use shared::{GameRules, ResourceType};

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
    /// The bank is stocked according to the table size: the 5-6 player game
    /// uses a deeper resource supply and a larger development deck.
    pub fn new(rules: &GameRules) -> Self {
        let mut resources = ResourceSet::new();
        for res in [
            ResourceType::Brick,
            ResourceType::Ore,
            ResourceType::Sheep,
            ResourceType::Wheat,
            ResourceType::Wood,
        ] {
            resources.add(res, rules.bank_per_resource);
        }

        let mut dev_cards: Vec<DevelopmentCard> = Vec::new();
        for _ in 0..rules.knight_cards {
            dev_cards.push(Knight(DevCardState::new()));
        }
        for _ in 0..rules.victory_point_cards {
            dev_cards.push(DevelopmentCard::VictoryPoint(DevCardState::new()));
        }
        for _ in 0..rules.road_building_cards {
            dev_cards.push(DevelopmentCard::RoadBuilder(DevCardState::new()));
        }
        for _ in 0..rules.monopoly_cards {
            dev_cards.push(DevelopmentCard::Monopoly(DevCardState::new()));
        }
        for _ in 0..rules.year_of_plenty_cards {
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

        let mut claimants: HashMap<ResourceType, HashSet<Uuid>> = HashMap::new();
        for (player_id, res, _) in &pending {
            claimants.entry(*res).or_default().insert(*player_id);
        }

        // When the bank cannot cover a resource, the rules distinguish two
        // cases: a single claimant receives whatever is left, but if several
        // players are owed it then nobody receives any of it.
        let mut budget: HashMap<ResourceType, u32> = HashMap::new();
        for (res, needed) in &totals {
            let available = self.game_resources.amount_of(*res);

            let payable = if available >= *needed {
                *needed
            } else if claimants.get(res).map_or(0, |ids| ids.len()) == 1 {
                log::warn!(
                    "Bank low on {:?}: paying sole claimant {} of {}",
                    res,
                    available,
                    needed
                );
                available
            } else {
                log::warn!(
                    "Bank does not have enough {:?} for {} claimants: needed {}, available {}",
                    res,
                    claimants.get(res).map_or(0, |ids| ids.len()),
                    needed,
                    available
                );
                0
            };

            budget.insert(*res, payable);
        }

        let mut distributed = Vec::new();

        for (player_id, res, amount) in pending {
            let remaining = budget.entry(res).or_insert(0);
            let payout = amount.min(*remaining);
            if payout == 0 {
                continue;
            }

            let mut cost = ResourceSet::new();
            cost.add(res, payout);

            if self
                .collect_from_to(
                    ResourceEndpoint::Bank,
                    ResourceEndpoint::Player(player_id),
                    &cost,
                    players,
                )
                .is_ok()
            {
                *budget.get_mut(&res).expect("budget entry exists") -= payout;
                distributed.push((player_id, res, payout));
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

    /// What is left, for the clients. Card identities stay hidden; only the
    /// size of the deck travels.
    pub fn to_info(&self) -> shared::BankInfo {
        shared::BankInfo {
            resources: (&self.game_resources).into(),
            dev_cards: self.dev_cards.len(),
        }
    }

    pub fn draw_dev_card(&mut self) -> Result<DevelopmentCard, GameError> {
        self.dev_cards
            .pop()
            .ok_or(GameError::InvalidAction)
    }
}





















#[cfg(test)]
mod tests {
    use shared::GameRules;

    /// The base-game setup, which most of these tests assume.
    fn base_rules() -> GameRules {
        GameRules::for_player_count(4)
    }

    use shared::PlayerColour;
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

    /// Builds a board where exactly `settlers` players own a settlement on a
    /// corner of one chosen hex, and returns that hex's number and resource.
    fn board_with_settlements_on_one_hex(
        settlers: &[Uuid],
    ) -> (Board, Robber, Coordinates, u8, ResourceType) {
        let mut board = Board::new();

        let (coord, number, resource, corners) = board
            .hexes
            .values()
            .find(|h| h.resource != ResourceType::Desert)
            .map(|h| (h.coord, h.number, h.resource, h.adjacent_vertices))
            .unwrap();

        // Any other hex sharing this number would pay out too, which would
        // muddy the accounting, so silence them.
        let others: Vec<_> = board
            .hexes
            .values()
            .filter(|h| h.coord != coord && h.number == number)
            .map(|h| h.coord)
            .collect();
        for other in others {
            board.hexes.get_mut(&other).unwrap().number = 0;
        }

        for (i, owner) in settlers.iter().enumerate() {
            let v = board.vertices.get_mut(&corners[i * 2]).unwrap();
            v.owner = Some(*owner);
            v.building = Some(VertexBuilding::Settlement);
        }

        let robber = Robber::new(&board);
        (board, robber, coord, number, resource)
    }

    /// A lone claimant takes whatever the bank has left, even if it is short.
    #[test]
    fn bank_shortfall_pays_a_sole_claimant_the_remainder() {
        let only = pid(1);
        let (mut board, robber, _, number, resource) =
            board_with_settlements_on_one_hex(&[only]);

        // Upgrade to a city so the player is owed 2 but the bank holds only 1.
        let corner = *board
            .vertices
            .iter()
            .find(|(_, v)| v.owner == Some(only))
            .map(|(c, _)| c)
            .unwrap();
        board.vertices.get_mut(&corner).unwrap().building = Some(VertexBuilding::City);

        let mut bank = Bank::new(&base_rules());
        let mut players = players(vec![Player::new(only, "A", PlayerColour::Blue)]);

        bank.game_resources = ResourceSet::new();
        bank.game_resources.add(resource, 1);

        let distributed = bank.give_resources_for_roll(&board, number, &robber, &mut players);

        assert_eq!(
            distributed,
            vec![(only, resource, 1)],
            "the sole claimant should receive the bank's last card"
        );
        assert_eq!(players.get(only).unwrap().resources.amount_of(resource), 1);
        assert_eq!(bank.game_resources.amount_of(resource), 0);
    }

    /// If two players are owed a resource the bank cannot cover, neither gets any.
    #[test]
    fn bank_shortfall_pays_nobody_when_several_players_claim() {
        let a = pid(1);
        let b = pid(2);
        let (board, robber, _, number, resource) = board_with_settlements_on_one_hex(&[a, b]);

        let mut bank = Bank::new(&base_rules());
        let mut players = players(vec![Player::new(a, "A", PlayerColour::Red), Player::new(b, "B", PlayerColour::Green)]);

        // One card left, two players each owed one.
        bank.game_resources = ResourceSet::new();
        bank.game_resources.add(resource, 1);

        let distributed = bank.give_resources_for_roll(&board, number, &robber, &mut players);

        assert!(
            distributed.is_empty(),
            "with two claimants and one card, nobody is paid"
        );
        assert_eq!(players.get(a).unwrap().resources.amount_of(resource), 0);
        assert_eq!(players.get(b).unwrap().resources.amount_of(resource), 0);
        assert_eq!(bank.game_resources.amount_of(resource), 1, "card stays in the bank");
    }

    #[test]
    fn bank_new_initializes_resources_and_dev_cards() {
        let bank = Bank::new(&base_rules());

        for r in [
            ResourceType::Brick,
            ResourceType::Ore,
            ResourceType::Sheep,
            ResourceType::Wheat,
            ResourceType::Wood,
        ] {
            assert_eq!(bank.game_resources.amount_of(r), base_rules().bank_per_resource);
        }

        assert_eq!(bank.dev_cards.len(), base_rules().dev_card_total());

        let knights = bank.dev_cards.iter().filter(|c| matches!(c, DevelopmentCard::Knight(_))).count();
        assert_eq!(knights, base_rules().knight_cards);
    }

    #[test]
    fn give_resources_for_roll_pays_settlement() {
        let player_id = pid(1);
        let (board, robber, _, number, resource) =
            board_with_settlements_on_one_hex(&[player_id]);

        let mut bank = Bank::new(&base_rules());
        let mut players = players(vec![Player::new(player_id, "A", PlayerColour::Yellow)]);

        let distributed = bank.give_resources_for_roll(&board, number, &robber, &mut players);

        assert_eq!(distributed, vec![(player_id, resource, 1)]);
        assert_eq!(
            players.get(player_id).unwrap().resources.amount_of(resource),
            1
        );
    }

    #[test]
    fn give_resources_for_roll_blocked_by_robber() {
        let player_id = pid(1);
        let (board, mut robber, hex_coord, number, resource) =
            board_with_settlements_on_one_hex(&[player_id]);

        let mut bank = Bank::new(&base_rules());
        let mut players = players(vec![Player::new(player_id, "A", PlayerColour::Blue)]);

        robber.move_to(hex_coord).unwrap();

        let distributed = bank.give_resources_for_roll(&board, number, &robber, &mut players);

        assert!(distributed.is_empty(), "the robber blocks this hex");
        assert_eq!(
            players.get(player_id).unwrap().resources.amount_of(resource),
            0
        );
    }



    #[test]
    fn collect_from_player_to_player_transfers_resources() {
        let mut bank = Bank::new(&base_rules());

        let from = pid(1);
        let to = pid(2);

        let mut p1 = Player::new(from, "From", PlayerColour::Red);
        p1.resources.add(ResourceType::Wood, 3);

        let p2 = Player::new(to, "To", PlayerColour::Green);

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
        let mut bank = Bank::new(&base_rules());

        let target = pid(0);

        let mut ps = vec![Player::new(target, "T", PlayerColour::Yellow)];

        for i in 1..=3 {
            let mut p = Player::new(pid(i), "P", PlayerColour::Blue);
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
        let mut bank = Bank::new(&base_rules());

        let pid1 = Uuid::from_u128(1);
        let pid2 = Uuid::from_u128(2); // nebude existovat

        let mut from = Player::new(pid1, "From", PlayerColour::Blue);
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

