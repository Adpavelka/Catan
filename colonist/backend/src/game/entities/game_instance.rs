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

    // TODO: min player count, option to start before max lobby
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
        Self {
            id: gid,
            max_players: player_count,

            turn_manager: TurnManager::new(player_count, creator_pid),

            phase: GamePhase::WaitingForPlayers,
            pending_actions: HashMap::new(),


            pending_trades: HashMap::new(),
            next_trade_id: 1,
        }
    }


    pub fn get_all_players_info(&self) -> Vec<shared::PlayerInfo> {
        let mut infos = Vec::new();

        for i in 0..self.turn_manager.players.len() {
            let player = self.turn_manager.players.get_by_index(i).unwrap();
            let pid = player.id;

            let has_longest_road = self.turn_manager.road_bonus.holder() == Some(pid);
            let has_largest_army = self.turn_manager.army_bonus.holder() == Some(pid);

            infos.push(shared::PlayerInfo {
                player_id: pid,
                name: player.name.clone(),
                color: player.colour.to_string(),
                victory_points: player.get_victory_points(),
                dev_cards: player.dev_cards.iter().map(|c| c.get_type()).collect(),
                ports: player.ports.iter().map(|port| port.into()).collect(),
                resources: (&player.resources).into(),
                knights_played: player.knight_played,
                roads_count: player.longest_road,
                has_longest_road,
                has_largest_army,
            });
        }

        infos
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


    /// Checks whether a road may be placed on `edge` right now. Purely a read:
    /// the phase is only advanced afterwards, by `advance_after_road`, once the
    /// placement has actually succeeded.
    ///
    /// Returns whether the road is free (initial placement, or a Road Building card).
    pub (crate) fn validate_road_placement(&self, edge: (i32, i32)) -> Result<bool, String> {
        match self.phase {
            GamePhase::InitialPlacement { step, .. } => {
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


    /// Advances the initial-placement step after a road was successfully built.
    /// No-op outside initial placement.
    pub (crate) fn advance_after_road(&mut self) {
        if let GamePhase::InitialPlacement {
            round,
            step: PlacementStep::BuildRoad { .. },
        } = self.phase
        {
            self.phase = GamePhase::InitialPlacement {
                round,
                step: PlacementStep::BuildSettlement,
            };
        }
    }


    /// Checks whether a settlement may be placed right now. Purely a read: the
    /// phase is only advanced afterwards, by `advance_after_settlement`, once
    /// the placement has actually succeeded.
    ///
    /// Returns whether the settlement should yield its starting resources
    /// (second round of initial placement).
    pub (crate) fn validate_settlement_placement(&self) -> Result<bool, String> {
        match self.phase {
            GamePhase::InitialPlacement { round, step } => {
                if step != PlacementStep::BuildSettlement {
                    return Err("You must build a road next.".into());
                }

                Ok(round == InitialRound::Second)
            }

            GamePhase::RegularPlay => Ok(false),

            GamePhase::WaitingForPlayers => {
                Err("Game has not started.".into())
            }
        }
    }


    /// Records the settlement just built as the anchor the next road must touch.
    /// No-op outside initial placement.
    pub (crate) fn advance_after_settlement(&mut self, x: i32, y: i32) {
        if let GamePhase::InitialPlacement {
            round,
            step: PlacementStep::BuildSettlement,
        } = self.phase
        {
            self.phase = GamePhase::InitialPlacement {
                round,
                step: PlacementStep::BuildRoad {
                    settlement: (x, y),
                },
            };
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


#[cfg(test)]
mod tests {
    use super::*;

    fn started_game() -> (GameInstance, Uuid) {
        let pid = Uuid::from_u128(1);
        let mut game = GameInstance::new("test".into(), pid, 1);
        game.start_game();
        (game, pid)
    }

    /// A buildable vertex that is far enough from `avoid` to satisfy the
    /// distance-two rule.
    fn free_vertex(game: &GameInstance, avoid: Option<(i32, i32)>) -> (i32, i32) {
        let board = &game.turn_manager.board;
        let mut coords: Vec<_> = board
            .vertices
            .keys()
            .copied()
            .filter(|c| board.is_buildable_vertex(*c))
            .filter(|c| match avoid {
                Some(a) => {
                    let dist = (c.0 - a.0).abs() + (c.1 - a.1).abs();
                    dist > 6
                }
                None => true,
            })
            .collect();
        coords.sort();
        coords[0]
    }

    /// Stealing used to be completely unvalidated: anyone could rob anyone,
    /// at any time, as often as they liked.
    #[test]
    fn stealing_requires_having_moved_the_robber() {
        use crate::game::entities::building::VertexBuilding;
        use crate::lobby::actions::development::handle_steal_from_player;

        let thief = Uuid::from_u128(1);
        let victim = Uuid::from_u128(2);
        let outsider = Uuid::from_u128(3);

        let mut game = GameInstance::new("test".into(), thief, 3);
        game.turn_manager.players.add_player_with_colour(victim);
        game.turn_manager.players.add_player_with_colour(outsider);
        game.start_game();
        game.advance_phase();
        game.advance_phase(); // RegularPlay

        // Give the victim something worth taking, and a settlement next to the robber.
        game.turn_manager
            .players
            .get_mut(victim)
            .unwrap()
            .resources
            .add(crate::game::entities::resources::ResourceType::Wood, 3);

        let robber_hex = game.turn_manager.get_robber_pos();
        let corner = crate::game::entities::board::Board::get_adjacent_hexes(robber_hex)[0];
        game.turn_manager
            .board
            .build_vertex(victim, corner, VertexBuilding::Settlement);

        // No robber move has happened, so there is nothing to cash in.
        assert!(
            handle_steal_from_player(thief, victim, &mut game).is_err(),
            "stealing without moving the robber must be refused"
        );

        // A player who is not on turn may never steal, even once it is unlocked.
        game.add_pending_action(thief, PendingAction::Steal);
        assert!(
            handle_steal_from_player(outsider, victim, &mut game).is_err(),
            "a player who is not on turn must not be able to steal"
        );

        // A victim with nothing next to the robber is not a legal target.
        assert!(
            handle_steal_from_player(thief, outsider, &mut game).is_err(),
            "a player with no building by the robber must not be robbable"
        );

        // The legal steal works exactly once.
        assert!(handle_steal_from_player(thief, victim, &mut game).is_ok());
        assert!(
            handle_steal_from_player(thief, victim, &mut game).is_err(),
            "the steal must be consumed, not repeatable"
        );
    }

    /// A rejected settlement must leave the phase untouched, so the player can
    /// simply click somewhere else. Regression test: the phase used to advance
    /// before the placement was validated, which deadlocked initial placement.
    #[test]
    fn rejected_settlement_leaves_phase_unchanged() {
        let (mut game, pid) = started_game();
        let phase_before = game.get_state();

        let err = game
            .handle_build_settlement(pid, 9999, 9999)
            .expect_err("a nonexistent vertex must be rejected");
        assert!(err.contains("InvalidPosition"), "unexpected error: {err}");

        assert_eq!(
            game.get_state(),
            phase_before,
            "a rejected settlement must not advance the placement step"
        );

        let good = free_vertex(&game, None);
        game.handle_build_settlement(pid, good.0, good.1)
            .expect("a legal settlement must still be accepted afterwards");
    }

    /// A rejected road must not advance the step back to BuildSettlement,
    /// which would let the player place a second settlement in one round.
    #[test]
    fn rejected_road_leaves_phase_unchanged() {
        let (mut game, pid) = started_game();

        let settlement = free_vertex(&game, None);
        game.handle_build_settlement(pid, settlement.0, settlement.1)
            .unwrap();

        let phase_before = game.get_state();
        assert!(matches!(
            phase_before,
            GamePhase::InitialPlacement {
                step: PlacementStep::BuildRoad { .. },
                ..
            }
        ));

        game.handle_build_road(pid, 9999, 9999)
            .expect_err("a nonexistent edge must be rejected");

        assert_eq!(
            game.get_state(),
            phase_before,
            "a rejected road must not advance the placement step"
        );
    }

    /// The second settlement is bound only by the distance-two rule; it does
    /// not have to touch the road placed in the first round.
    #[test]
    fn second_settlement_need_not_touch_existing_road() {
        let (mut game, pid) = started_game();

        let first = free_vertex(&game, None);
        game.handle_build_settlement(pid, first.0, first.1).unwrap();

        let road = *game
            .turn_manager
            .board
            .vertices
            .get(&first)
            .unwrap()
            .adjacent_edges
            .iter()
            .next()
            .unwrap();
        game.handle_build_road(pid, road.0, road.1).unwrap();

        // Single-player game, so we are now in round two on the same player.
        game.advance_phase();

        let far = free_vertex(&game, Some(first));
        assert!(
            !game
                .turn_manager
                .board
                .is_vertex_connected_to_player(far, pid),
            "test vertex should not be connected to the player's road"
        );

        game.handle_build_settlement(pid, far.0, far.1)
            .expect("an unconnected settlement is legal during initial placement");
    }
}
