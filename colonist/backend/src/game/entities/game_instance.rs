use crate::game::entities::bonus_points::BonusCard;
use crate::game::entities::pending_trade::PendingTrade;
use crate::game::entities::turn_manager::TurnManager;
use serde::{Deserialize, Serialize};
use shared::{GamePhase, InitialRound, PendingAction, PlacementStep};
use std::{collections::HashMap, mem};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct GameInstance {
    pub id: String,

    // TODO: group
    pub player_ids: Vec<Uuid>,  // player IDs in join order
    pub player_id_to_slot: HashMap<Uuid, usize>,  // Maps connection ID to game slot (0-3)
    pub max_players: usize,
    pub turn_manager: TurnManager,

    phase: GamePhase,

    pub pending_actions: HashMap<Uuid, Vec<PendingAction>>,

    // TODO class, trader?
    #[serde(skip)]
    pub pending_trades: HashMap<u64, PendingTrade>,  // Active trade offers
    #[serde(skip)]
    pub next_trade_id: u64,  // Counter for trade IDs

}

impl GameInstance {
    pub fn new(gid: String, creator_pid: Uuid, player_count: usize) -> Self {
        let mut player_id_to_slot = HashMap::new();
        player_id_to_slot.insert(creator_pid, 0);

        Self {
            id: gid,
            player_ids: vec![creator_pid],
            player_id_to_slot,
            max_players: player_count,

            turn_manager: TurnManager::new(player_count, creator_pid),

            phase: GamePhase::WaitingForPlayers,
            pending_actions: HashMap::new(),


            pending_trades: HashMap::new(),
            next_trade_id: 1,
        }
    }


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

    pub (crate) fn advance_phase(&mut self) {
        self.phase = match self.phase.clone() {
            GamePhase::InitialPlacement { round, step: _ } => {
                match round {
                    InitialRound::First => GamePhase::InitialPlacement {
                        round: InitialRound::Second,
                        step: PlacementStep::BuildSettlement,
                    },
                    InitialRound::Second => GamePhase::RegularPlay,
                }
            }
            other => other,
        };
    }


    pub (crate) fn get_state(&self) -> GamePhase {
        self.phase
    }


    pub (crate) fn start_game(&mut self) {
        self.phase = GamePhase::InitialPlacement {
            round: InitialRound::First,
            step: PlacementStep::BuildSettlement,
        };
    }


    pub (crate) fn validate_and_advance_after_road(&mut self, edge: (i32, i32)) -> Result<bool, String> {
        match self.phase {
            GamePhase::InitialPlacement { round, step } => {
                match step {
                    PlacementStep::BuildRoad { settlement } => {
                        if !self
                            .turn_manager
                            .board
                            .is_edge_touching_vertex(edge, settlement)
                        {
                            return Err(
                                "You must build road touching your last settlement built."
                                    .into(),
                            );
                        }

                        self.phase = GamePhase::InitialPlacement {
                            round,
                            step: PlacementStep::BuildSettlement
                        };

                        Ok(true)
                    }

                    PlacementStep::BuildSettlement => {
                        Err("You must build a settlement first.".into())
                    }
                }
            }

            GamePhase::RegularPlay => {
                let pid = self.turn_manager.players.get_current_player().id;

                let free = self
                    .pending_actions
                    .get(&pid)
                    .map(|actions| {
                        actions.iter().any(|a| matches!(a, PendingAction::RoadBuilding { .. }))
                    })
                    .unwrap_or(false);

                Ok(free)
            }

            GamePhase::WaitingForPlayers => {
                Err("Game has not started.".into())
            }
        }
    }


    pub (crate) fn validate_and_advance_after_settlement(&mut self, x: i32, y: i32) -> Result<bool, String> {
        match self.phase {
            GamePhase::InitialPlacement { round, step } => {
                if step != PlacementStep::BuildSettlement {
                    return Err("You must build a road next.".into());
                }

                self.phase = GamePhase::InitialPlacement {
                    round,
                    step: PlacementStep::BuildRoad {
                        settlement: (x, y),
                    },
                };

                Ok(round == InitialRound::Second)
            }

            GamePhase::RegularPlay => Ok(false),

            GamePhase::WaitingForPlayers => {
                Err("Game has not started.".into())
            }
        }
    }

    pub (crate) fn decrement_road_building(&mut self, pid: Uuid) {
        if let Some(actions) = self.pending_actions.get_mut(&pid) {
            if let Some(action) = actions.iter_mut()
                .find(|a| matches!(a, PendingAction::RoadBuilding { .. })) 
            {
                if let PendingAction::RoadBuilding { remaining } = action {
                    *remaining -= 1;

                    if *remaining == 0 {
                        self.remove_pending_action(
                            pid,
                            PendingAction::RoadBuilding { remaining: 0 }
                        );
                    }
                }
            }
        }
    }


    pub fn add_pending_action(&mut self, pid: Uuid, action: PendingAction) {
        self.pending_actions.entry(pid).or_default().push(action);
    }

    pub fn remove_pending_action(&mut self, pid: Uuid, action: PendingAction) {
        if let Some(actions) = self.pending_actions.get_mut(&pid) {
            let target = mem::discriminant(&action);

            actions.retain(|a| mem::discriminant(a) != target);
            
            if actions.is_empty() {
                self.pending_actions.remove(&pid);
            }
        }
    }

    pub fn has_pending_action(&self, pid: Uuid, action: PendingAction) -> bool {
        self.pending_actions
            .get(&pid)
            .map_or(false, |actions| actions.contains(&action))
    }
}
