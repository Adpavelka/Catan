use crate::game::entities::bonus_points::BonusCard;
use crate::game::entities::pending_trade::PendingTrade;
use crate::game::entities::turn_manager::TurnManager;
use serde::{Deserialize, Serialize};
use shared::GamePhase;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct GameInstance {
    pub id: String,

    pub player_ids: Vec<Uuid>,  // player IDs in join order
    pub player_id_to_slot: HashMap<Uuid, usize>,  // Maps connection ID to game slot (0-3)
    pub max_players: usize,

    pub turn_manager: TurnManager,
    pub phase: GamePhase,
    
    pub pending_discards: HashSet<Uuid>,  // Players who still need to discard
    pub seven_roller: Option<Uuid>,  // Player who rolled 7 and needs to move robber
    pub knight_mover: Option<Uuid>,  // Player who played a knight and needs to move robber
    pub free_roads_remaining: u8,  // Free roads from Road Builder card
    pub year_of_plenty_pending: Option<Uuid>,  // Player who played Year of Plenty and needs to choose resources
    pub monopoly_pending: Option<Uuid>,
    #[serde(skip)]
    pub pending_trades: HashMap<u64, PendingTrade>,  // Active trade offers
    #[serde(skip)]
    pub next_trade_id: u64,  // Counter for trade IDs

    pub initial_settlements_placed: usize,  // Track how many initial settlements have been placed
    pub initial_settlements: HashMap<Uuid, usize>,
    pub initial_roads: HashMap<Uuid, usize>,
}
impl GameInstance {
    pub fn get_all_players_info(&self) -> Vec<shared::PlayerInfo> {
        self.player_ids.iter()
            .filter_map(|&pid| self.turn_manager.players.get(pid).map(|p| (pid, p)))
            .map(|(pid, p)| {
                let has_longest_road = self.turn_manager.road_bonus.holder() == Some(pid);
                let has_largest_army = self.turn_manager.army_bonus.holder() == Some(pid);

                shared::PlayerInfo {
                    player_id: pid,
                    name: p.name.clone(),
                    color: p.colour.to_string(),
                    victory_points: p.get_victory_points(),
                    dev_cards: p.dev_cards.iter().map(|c| c.get_type()).collect(),
                    ports: p.ports.iter().map(|port| port.into()).collect(), // why
                    resources: (&p.resources).into(),
                    knights_played: p.knight_played,
                    roads_count: p.longest_road,
                    has_longest_road,
                    has_largest_army,
                }
            })
            .collect()
    }


    // jsou tyhle metody vůbec třeba?
    // co by se stalo bez nich?
    // nebude mít prostě suroviny ne?
    // nebo mu to povolí postavit více měst ze začátku?
    // asi bych radši dal nějaký free counter a dal to do construction jen
    pub fn can_build_settlement_in_phase(&mut self, pid: Uuid) -> Result<(), String> {
        if self.is_initial_phase() {
            let count = *self.initial_settlements.get(&pid).unwrap_or(&0);

            if self.is_second_phase() {
                if count >= 2 {
                    return Err("You have already placed your second initial settlement.".into());
                } else if count < 1 {
                    return Err("Invalid state: Missing first settlement.".into());
                }
            } else {
                if count >= 1 {
                    return Err("You must place exactly one settlement in the first round.".into());
                }
            }

            //*self.initial_settlements.entry(pid).or_insert(0) += 1;
        }

        Ok(())
    }


    pub fn can_build_road_in_phase(&mut self, pid: Uuid) -> Result<(), String> {
        if self.is_initial_phase() {
            let road_count = *self.initial_roads.get(&pid).unwrap_or(&0);
            let settlement_count = *self.initial_settlements.get(&pid).unwrap_or(&0);

            if road_count >= settlement_count {
                //return Err("You must place a settlement before placing a road.".into());
            }
            //*self.initial_settlements.entry(pid).or_insert(0) += 1;

            //*self.initial_settlements.entry(pid).or_insert(0) += 1;
            //*self.initial_roads.entry(pid).or_insert(0) += 1;
        }

        Ok(())
    }
}
