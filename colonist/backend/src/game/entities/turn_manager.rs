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
    dice: Dice,
    pub bank: Bank,
    pub board: Board,

    pub last_roll: Option<u8>, // atribut kostky, nebo networking by si to měl úkládat

    game_over: bool,
    pub robber: Robber,
    
    pub army_bonus: BiggestArmy, // tohle by taky mělo private ne?
    pub road_bonus: LongestRoad, // tohle by taky mělo private ne?


    pub new_players: Players,
}

impl TurnManager {
    pub fn new(player_count: usize, first_player_id:Uuid) -> TurnManager {
        let first_player = Player::new(first_player_id, "Player 1", 'b');

        let board = Board::new();

        info!("New game initialized for {} players", player_count);

        Self {
            dice: Dice::new(),
            bank: Bank::new(),
            board: Board::new(),
            game_over: false,
            robber: Robber::new(&board),
            army_bonus: BiggestArmy::new(),
            road_bonus: LongestRoad::new(),
            last_roll: None,
            new_players: Players::new(vec![first_player.clone()]),
        }
    }


    pub fn next_turn(&mut self) -> Result<(u8, Vec<(Uuid, shared::ResourceType, u32)>), GameError> {
        if self.game_over {
            return Err(GameError::InvalidAction);
        }

        if self.last_roll.is_some() {
            return Ok((self.last_roll.unwrap(), Vec::new()));
        }

        let roll_value = self.dice.roll();
        info!(
            "Player {} rolled: {}",
            self.new_players.get_current_index(), roll_value
        );

        let distributed = if roll_value == 7 {
            info!("Robber activated (7 rolled)");
            // Logic handled by Lobby
            Vec::new()
        } else {
            self.bank
                .give_resources_for_roll(&self.board, roll_value, &self.robber, &mut self.new_players)
        };
        self.last_roll = Some(roll_value);
        Ok((roll_value, distributed))
    }

    pub fn end_turn(&mut self) {
        let prev_player = self.new_players.get_current_index();
        {
            let pid = self.new_players.get_current_player().id;
            let player = self.new_players.get_mut(pid).unwrap();

            player
                .dev_cards
                .iter_mut()
                .for_each(|card| card.next_turn());
            player.dev_card_played_this_turn = false;
        }

        self.new_players.next_turn();

        self.last_roll = None;
        info!(
            "Player {}'s turn started.",
            self.new_players.get_current_player().id
        );
        info!(
            "Turn ended for Player {}. Now on turn: Player {}",
            prev_player, self.new_players.get_current_index()
        );
    }

    fn pay_resources(&mut self, pid: Uuid, cost: ResourceSet) -> Result<(), GameError> {
        {
            let player = self
                .new_players
                .get(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if !player.can_pay(&cost) {
                return Err(GameError::NotEnoughResources);
            }
        }
        self.bank.collect_from_player(pid, cost, &mut self.new_players)?;
        Ok(())
    }

    pub fn build_settlement(
        &mut self,
        pos: Coordinates,
        is_initial: bool,
    ) -> Result<(), GameError> {
        let pid: Uuid = self.new_players.get_current_player().id;

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
            .new_players
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

        info!("Player {} built a SETTLEMENT at {:?}", pid, pos);
        Ok(())
    }

    pub fn build_city(&mut self, pos: Coordinates) -> Result<(), GameError> {
        let pid = self.new_players.get_current_player().id;

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
            .new_players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;
        player.use_city()?;

        self.board.build_vertex(pid, pos, City);

        info!("Player {} built a CITY at {:?}", pid, pos);
        Ok(())
    }

    pub fn build_road(&mut self, pos: Coordinates, is_initial: bool) -> Result<(), GameError> {
        let pid = self.new_players.get_current_player().id;

        let edge = self.board.edges.get(&pos).ok_or(GameError::InvalidAction)?;
        if edge.building.is_some() || !self.board.is_edge_connected_to_player(pos, pid) {
            return Err(GameError::InvalidAction);
        }

        let cost = Road.cost();

        if !is_initial {
            let player = self
                .new_players
                .get(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if !player.can_pay(&cost) {
                return Err(GameError::NotEnoughResources);
            }
            self.bank.collect_from_player(pid, cost, &mut self.new_players)?;
        }

        let longest_road_len = {
            let player = self
                .new_players
                .get_mut(pid)
                .ok_or(GameError::PlayerNotFound)?;
            player.use_road()?;
            self.board.build_edge(pid, pos, Road);

            self.board.calculate_longest_road(pid)
        };

        let player = self
            .new_players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;
        player.longest_road = longest_road_len;

        self.road_bonus.recalculate(&mut self.new_players);

        info!("Player {} built a ROAD at {:?}", pid, pos);
        Ok(())
    }

    pub fn buy_dev_card(&mut self) -> Result<shared::DevCardType, GameError> {
        let pid = self.new_players.get_current_player().id;
        let cost = DevelopmentCard::cost();

        {
            let player = self
                .new_players
                .get_mut(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if !player.can_pay(&cost) {
                return Err(GameError::NotEnoughResources);
            }
        }

        self.bank.collect_from_player(pid, cost, &mut self.new_players)?;

        let card = self.bank.draw_dev_card()?;
        let card_type = card.get_type();

        let player = self
            .new_players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;

        // Victory Point cards immediately grant a secret victory point
        if card_type == shared::DevCardType::VictoryPoint {
            player.add_secret_victory_point();
        }

        player.dev_cards.push(card);

        Ok(card_type)
    }

    pub fn play_development_card(
        &mut self,
        card_type: shared::DevCardType,
        target: Option<shared::DevCardTarget>,
    ) -> Result<(), GameError> {
        let pid = self.new_players.get_current_player().id;

        {
            let player = self
                .new_players
                .get(pid)
                .ok_or(GameError::PlayerNotFound)?;
            if player.dev_card_played_this_turn {
                return Err(GameError::InvalidAction);
            }
        }

        let mut card = {
            let player = self
                .new_players
                .get_mut(pid)
                .ok_or(GameError::PlayerNotFound)?;
            let idx = player
                .dev_cards
                .iter()
                .position(|c| c.get_type() == card_type && c.can_play())
                .ok_or(GameError::InvalidAction)?;
            player.dev_cards.remove(idx)
        };

        card.play(self, &target);

        let player = self
            .new_players
            .get_mut(pid)
            .ok_or(GameError::PlayerNotFound)?;
        player.dev_cards.push(card);
        player.dev_card_played_this_turn = true;

        Ok(())
    }

    pub fn move_robber(&mut self, hex_coords: Coordinates) -> Result<(), GameError> {
        info!("Robber moved to {:?}", hex_coords);
        self.robber.pos = hex_coords;
        Ok(())
    }

    pub fn steal_card(
        &mut self,
        thief_id: Uuid,
        victim_id: Uuid,
    ) -> Result<Option<shared::ResourceType>, GameError> {
        let victim = self
            .new_players
            .get_mut(victim_id)
            .ok_or(GameError::PlayerNotFound)?;
        if let Some(res_type) = victim.resources.take_random_card() {
            let thief = self
                .new_players
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

    pub fn has_player_won(&mut self, player_id: Uuid) -> bool {
        let answer = self.new_players
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
        if let Some(player) = self.new_players.get(player_id) {
            player.get_secret_victory_points()
        } else {
            0
        }
    }
}
