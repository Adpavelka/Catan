use crate::errors::GameError;
use crate::game::entities::bank::Bank;
use crate::game::entities::board::{Board, Coordinates};
use crate::game::entities::bonus_points::{BiggestArmy, BonusCard, LongestRoad};
use crate::game::entities::development_card::DevelopmentCard;
use crate::game::entities::dice::Dice;
use crate::game::entities::player::Player;
use crate::game::entities::players::Players;
use crate::game::entities::resources::ResourceSet;
use crate::game::entities::robber::Robber;
use log::info;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::game::entities::building::EdgeBuilding::Road;
use crate::game::entities::building::VertexBuilding::{City, Settlement};

#[derive(Serialize, Deserialize, Clone)]
pub struct TurnManager {
    pub dice: Dice,
    pub bank: Bank,
    pub board: Board,
    pub players: Players,

    robber: Robber,
    game_over: bool,
    
    pub army_bonus: BiggestArmy, // tohle by taky mělo private ne?
    pub road_bonus: LongestRoad, // tohle by taky mělo private ne?
}

impl TurnManager {
    pub fn new(player_count: usize, first_player_id:Uuid) -> TurnManager {
        let first_player = Player::new(first_player_id, "Player 1", 'b');

        let board = Board::new();
        let robber = Robber::new(&board);

        info!("New game initialized for {} players", player_count);

        Self {
            dice: Dice::new(),
            bank: Bank::new(),
            board,
            game_over: false,
            robber,
            army_bonus: BiggestArmy::new(),
            road_bonus: LongestRoad::new(),
            players: Players::new(vec![first_player.clone()]),
        }
    }


    pub fn next_turn(&mut self) -> Result<((u8, u8), Vec<(Uuid, shared::ResourceType, u32)>), GameError> {
        if self.game_over {
            return Err(GameError::InvalidAction);
        }

        let roll_value = self.dice.roll()?;
        info!(
            "Player {} rolled: {}",
            self.players.get_current_index(), roll_value
        );

        let distributed = if roll_value == 7 {
            info!("Robber activated (7 rolled)");
            // Logic handled by Lobby
            Vec::new()
        } else {
            self.bank.give_resources_for_roll(&self.board, roll_value, &self.robber, &mut self.players)
        };

        Ok((self.dice.values(), distributed))
    }


    pub fn end_turn(&mut self) {
        let prev_player = self.players.get_current_index();
        {
            let pid = self.players.get_current_player().id;
            let player = self.players.get_mut(pid).unwrap();

            player
                .dev_cards
                .iter_mut()
                .for_each(|card| card.next_turn());
            player.dev_card_played_this_turn = false;
        }

        self.players.next_turn();

        self.dice.next_turn();

        info!(
            "Player {}'s turn started.",
            self.players.get_current_player().id
        );
        info!(
            "Turn ended for Player {}. Now on turn: Player {}",
            prev_player, self.players.get_current_index()
        );
    }


    fn pay_resources(&mut self, pid: Uuid, cost: ResourceSet) -> Result<(), GameError> {
        {
            let player = self
                .players
                .get(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if !player.can_pay(&cost) {
                return Err(GameError::NotEnoughResources);
            }
        }
        self.bank.collect_from_player(pid, cost, &mut self.players)?;
        Ok(())
    }


    pub fn build_settlement(&mut self, pos: Coordinates, is_initial: bool) -> Result<(), GameError> {
        let pid: Uuid = self.players.get_current_player().id;

        let vertex = self
            .board
            .vertices
            .get(&pos)
            .ok_or(GameError::InvalidPosition)?;
        if vertex.building.is_some() || !self.board.is_buildable_vertex(pos) {
            return Err(GameError::InvalidPosition);
        }
        if !is_initial && !self.board.is_vertex_connected_to_player(pos, pid) {
            return Err(GameError::InvalidPosition);
        }

        if !is_initial {
            self.pay_resources(pid, Settlement.cost())?;
        }

        let port_type = self.board.ports.get(&pos).map(|p| p.port_type);

        let player = self
            .players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;
        player.use_settlement()?;

        // Add port to player if building on a port vertex
        if let Some(pt) = port_type {
            if !player.ports.contains(&pt) {
                player.ports.push(pt);
                info!("Player {} gained access to port {:?}", pid, pt);
            }
        }

        self.board.build_vertex(pid, pos, Settlement);
        self.has_player_won(pid);

        info!("Player {} built a SETTLEMENT at {:?}", pid, pos);
        Ok(())
    }


    pub fn build_city(&mut self, pos: Coordinates) -> Result<(), GameError> {
        let pid = self.players.get_current_player().id;

        let vertex = self
            .board
            .vertices
            .get(&pos)
            .ok_or(GameError::InvalidPosition)?;
        let is_owner = vertex.owner == Some(pid);
        if !is_owner {
            return Err(GameError::InvalidAction);
        }
        if vertex.building != Some(Settlement) {
            return Err(GameError::InvalidAction);
        }
        self.pay_resources(pid, City.cost())?;

        let player = self
            .players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;
        player.use_city()?;

        self.board.build_vertex(pid, pos, City);
        self.has_player_won(pid);

        info!("Player {} built a CITY at {:?}", pid, pos);
        Ok(())
    }


    pub fn build_road(&mut self, pos: Coordinates, is_initial: bool) -> Result<(), GameError> {
        let pid = self.players.get_current_player().id;

        let edge = self.board.edges.get(&pos).ok_or(GameError::InvalidAction)?;
        if edge.building.is_some() || !self.board.is_edge_connected_to_player(pos, pid) {
            return Err(GameError::InvalidAction);
        }

        let cost = Road.cost();

        if !is_initial {
            let player = self
                .players
                .get(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if !player.can_pay(&cost) {
                return Err(GameError::NotEnoughResources);
            }
            self.bank.collect_from_player(pid, cost, &mut self.players)?;
        }

        let longest_road_len = {
            let player = self
                .players
                .get_mut(pid)
                .ok_or(GameError::PlayerNotFound)?;
            player.use_road()?;
            self.board.build_edge(pid, pos, Road);

            self.board.calculate_longest_road(pid)
        };

        let player = self
            .players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;
        player.longest_road = longest_road_len;

        self.road_bonus.recalculate(&mut self.players);
        self.has_player_won(pid);

        info!("Player {} built a ROAD at {:?}", pid, pos);
        Ok(())
    }


    pub fn buy_dev_card(&mut self) -> Result<shared::DevCardType, GameError> {
        let pid = self.players.get_current_player().id;
        let cost = DevelopmentCard::cost();

        {
            let player = self
                .players
                .get_mut(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if !player.can_pay(&cost) {
                return Err(GameError::NotEnoughResources);
            }
        }

        self.bank.collect_from_player(pid, cost, &mut self.players)?;

        let card = self.bank.draw_dev_card()?;
        let card_type = card.get_type();

        let player = self
            .players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;

        // Victory Point cards immediately grant a secret victory point
        if card_type == shared::DevCardType::VictoryPoint {
            player.add_secret_victory_point();
        }

        player.dev_cards.push(card);
        self.has_player_won(pid);

        Ok(card_type)
    }


    pub fn play_development_card(&mut self, card_type: shared::DevCardType, target: Option<shared::DevCardTarget>) -> Result<(), GameError> {
        let pid = self.players.get_current_player().id;

        {
            let player = self
                .players
                .get(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if player.dev_card_played_this_turn {
                return Err(GameError::InvalidAction);
            }
        }

        let current_player = self.players.get_current_player().clone(); 

        let mut card = {
            let player = self
                .players
                .get_mut(pid)
                .ok_or(GameError::PlayerNotFound)?;

            let idx = player
                .dev_cards
                .iter()
                .position(|c| {
                    c.get_type() == card_type &&
                    c.can_play(&current_player)
                })
                .ok_or(GameError::InvalidAction)?;

            player.dev_cards.remove(idx)
        };

        card.play(self, &target);

        let player = self
            .players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;

        player.dev_card_played_this_turn = true;
        self.has_player_won(pid);

        Ok(())
    }


    pub fn steal_card(&mut self, thief_id: Uuid, victim_id: Uuid) -> Result<Option<shared::ResourceType>, GameError> {
        let victim = self
            .players
            .get_mut(victim_id)
            .ok_or(GameError::PlayerNotFound)?;
        if let Some(res_type) = victim.resources.take_random_card() {
            let thief = self
                .players
                .get_mut(thief_id)
                .ok_or(GameError::PlayerNotFound)?;
            thief.resources.add(res_type, 1);
            info!(
                "Player {} stole resource from Player {}",
                thief_id, victim_id
            );
            return Ok(Some(res_type));
        }
        Ok(None)
    }    

    
    fn has_player_won(&mut self, player_id: Uuid) -> bool {
        let answer = self.players
                                .get(player_id)
                                .map(|p| p.get_total_victory_points() >= 10)
                                .unwrap_or(false);

        if answer {
            self.game_over = true;
        }
        answer
    }


    pub fn game_over(&self) -> bool {
        self.game_over
    }

    pub fn player_secret_victory_points(&self, player_id: Uuid) -> u8 {
        if let Some(player) = self.players.get(player_id) {
            player.get_secret_victory_points()
        } else {
            0
        }
    }

    pub fn move_robber(&mut self, coords: Coordinates) -> Result<(), GameError> {
        if !self.board.hexes.contains_key(&coords) {
            return Err(GameError::InvalidPosition);
        }

        self.robber.move_to(coords)
    }

    pub fn get_robber_pos(&self) -> Coordinates {
        self.robber.get_pos()
    }
}



#[cfg(test)]
mod tests {
    use crate::errors::GameError;
    use crate::game::entities::building::EdgeBuilding::Road;
    use crate::game::entities::building::VertexBuilding::Settlement;
    use crate::game::entities::resources::{ResourceSet, ResourceType};
    use crate::game::entities::turn_manager::TurnManager;
    use crate::game::entities::player::Player;
    use uuid::Uuid;

    fn pid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn make_tm(player_count: usize) -> TurnManager {
        TurnManager::new(player_count, pid(1))
    }

    fn add_players(tm: &mut TurnManager, count: usize) {
        for i in 2..=count as u128 {
            tm.players
                .add_player(Player::new(pid(i), &format!("Player {}", i), 'x'));
        }
    }

    fn find_any_vertex_coords(tm: &TurnManager) -> (i32, i32) {
        *tm.board.vertices.keys().next().unwrap()
    }

    fn find_any_edge_coords(tm: &TurnManager) -> (i32, i32) {
        *tm.board.edges.keys().next().unwrap()
    }

    #[test]
    fn new_initializes_state() {
        let tm = make_tm(3);

        assert!(!tm.game_over());
        assert_eq!(tm.players.len(), 1);
    }

    /// The robber must start on the desert of the board the game is actually
    /// played on. Regression test: the robber used to be placed on the desert
    /// of a second, discarded board, so it started on a producing hex.
    #[test]
    fn robber_starts_on_the_desert_of_the_game_board() {
        for _ in 0..20 {
            let tm = make_tm(3);
            let pos = tm.get_robber_pos();

            let hex = tm
                .board
                .hexes
                .get(&pos)
                .expect("robber must stand on a hex of the game board");

            assert_eq!(
                hex.resource,
                ResourceType::Desert,
                "robber started on {:?} at {:?}",
                hex.resource,
                pos
            );
        }
    }

    #[test]
    fn add_players_increases_count() {
        let mut tm = make_tm(3);
        add_players(&mut tm, 3);

        assert_eq!(tm.players.len(), 3);
    }

    #[test]
    fn current_player_is_first_player() {
        let tm = make_tm(3);
        let pid = tm.players.get_current_player().id;

        assert_eq!(pid, Uuid::from_u128(1));
    }

    #[test]
    fn end_turn_rotates_current_player() {
        let mut tm = make_tm(3);
        add_players(&mut tm, 3);

        let first = tm.players.get_current_player().id;
        tm.end_turn();
        let second = tm.players.get_current_player().id;

        assert_ne!(first, second);
    }

    #[test]
    fn next_turn_fails_when_game_over() {
        let mut tm = make_tm(3);
        let pid = tm.players.get_current_player().id;

        for _ in 0..10 {
            tm.players
                .get_mut(pid)
                .unwrap()
                .add_secret_victory_point();
        }

        tm.has_player_won(pid);
        let res = tm.next_turn();
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), GameError::InvalidAction);
    }

    #[test]
    fn build_settlement_fails_if_vertex_missing() {
        let mut tm = make_tm(3);

        let res = tm.build_settlement((9999, 9999), true);
        assert_eq!(res.unwrap_err(), GameError::InvalidPosition);
    }

    #[test]
    fn build_settlement_fails_if_vertex_occupied() {
        let mut tm = make_tm(3);
        let pid = tm.players.get_current_player().id;
        let v = find_any_vertex_coords(&tm);

        tm.board.build_vertex(pid, v, Settlement);

        let res = tm.build_settlement(v, true);
        assert_eq!(res.unwrap_err(), GameError::InvalidPosition);
    }

    #[test]
    fn initial_settlement_does_not_charge_resources() {
        let mut tm = make_tm(3);
        let pid = tm.players.get_current_player().id;

        let v = tm.board.vertices.iter()
            .find(|(c, v)| v.building.is_none() && tm.board.is_buildable_vertex(**c))
            .map(|(c, _)| *c)
            .unwrap();

        let player = tm.players.get_mut(pid).unwrap();
        player.resources = ResourceSet::new();
        player.resources.add(ResourceType::Wood, 2);
        player.resources.add(ResourceType::Brick, 2);

        let before = player.resources.clone();

        tm.build_settlement(v, true).unwrap();

        let after = tm.players.get(pid).unwrap().resources.clone();
        assert_eq!(before, after);
    }

    #[test]
    fn build_city_fails_without_settlement() {
        let mut tm = make_tm(3);
        let v = find_any_vertex_coords(&tm);

        let res = tm.build_city(v);
        assert_eq!(res.unwrap_err(), GameError::InvalidAction);
    }

    #[test]
    fn build_city_fails_if_settlement_not_owned() {
        let mut tm = make_tm(3);
        add_players(&mut tm, 2);

        let v = find_any_vertex_coords(&tm);
        tm.board.build_vertex(pid(2), v, Settlement);

        let res = tm.build_city(v);
        assert_eq!(res.unwrap_err(), GameError::InvalidAction);
    }

    #[test]
    fn build_road_fails_if_edge_missing() {
        let mut tm = make_tm(3);

        let res = tm.build_road((9999, 9999), true);
        assert_eq!(res.unwrap_err(), GameError::InvalidAction);
    }

    #[test]
    fn build_road_fails_if_edge_occupied() {
        let mut tm = make_tm(3);
        let pid = tm.players.get_current_player().id;
        let e = find_any_edge_coords(&tm);

        tm.board.build_edge(pid, e, Road);

        let res = tm.build_road(e, true);
        assert_eq!(res.unwrap_err(), GameError::InvalidAction);
    }

    #[test]
    fn buy_dev_card_fails_if_player_cannot_pay() {
        let mut tm = make_tm(3);
        let pid = tm.players.get_current_player().id;

        tm.players.get_mut(pid).unwrap().resources = ResourceSet::new();

        let res = tm.buy_dev_card();
        assert_eq!(res.unwrap_err(), GameError::NotEnoughResources);
    }

    #[test]
    fn has_player_won_sets_game_over() {
        let mut tm = make_tm(3);
        let pid = tm.players.get_current_player().id;

        for _ in 0..10 {
            tm.players
                .get_mut(pid)
                .unwrap()
                .add_secret_victory_point();
        }

        let won = tm.has_player_won(pid);
        assert!(won);
        assert!(tm.game_over());
    }
}
