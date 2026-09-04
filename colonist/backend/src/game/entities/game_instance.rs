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

    pub max_players: usize,
    pub turn_manager: TurnManager,

    phase: GamePhase,

    pub pending_actions: HashMap<Uuid, Vec<PendingAction>>,

    #[serde(default)]
    pub pending_trades: HashMap<u64, PendingTrade>,  // Active trade offers
    #[serde(default = "first_trade_id")]
    pub next_trade_id: u64,  // Counter for trade IDs

    /// Wall-clock seconds since the epoch when this game last saw activity.
    /// Used to evict abandoned games; `0` means "never touched".
    #[serde(default)]
    pub last_activity_secs: u64,
}

/// Catan needs at least three players; below that the trading and robber
/// rules stop making sense.
pub const MIN_PLAYERS: usize = 3;
/// There are only four player colours, so four is a hard ceiling.
pub const MAX_PLAYERS: usize = 4;

/// Trade offers older than this are treated as withdrawn.
pub const TRADE_LIFETIME_SECS: u64 = 120;

fn first_trade_id() -> u64 {
    1
}

/// Seconds since the Unix epoch. Saturates rather than panicking if the clock
/// is somehow before the epoch.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl GameInstance {
    pub fn new(
        gid: String,
        creator_pid: Uuid,
        player_count: usize,
        creator_name: &str,
        creator_colour: shared::PlayerColour,
    ) -> Self {
        // The requested size comes straight from a client, so pin it to what
        // the game can actually seat.
        let max_players = player_count.clamp(MIN_PLAYERS, MAX_PLAYERS);

        Self {
            id: gid,
            max_players,

            turn_manager: TurnManager::new(player_count, creator_pid, creator_name, creator_colour),

            phase: GamePhase::WaitingForPlayers,
            pending_actions: HashMap::new(),


            pending_trades: HashMap::new(),
            next_trade_id: 1,
            last_activity_secs: now_secs(),
        }
    }


    /// The public roster, with the bonus-card flags filled in. This is the one
    /// place `PlayerInfo` is built; `From<&Player>` cannot know who holds the
    /// bonus cards, so it is not used directly.
    pub fn get_all_players_info(&self) -> Vec<shared::PlayerInfo> {
        (0..self.turn_manager.players.len())
            .filter_map(|i| self.turn_manager.players.get_by_index(i))
            .map(|player| {
                let mut info = shared::PlayerInfo::from(player);
                info.has_longest_road = self.turn_manager.road_bonus.holder() == Some(player.id);
                info.has_largest_army = self.turn_manager.army_bonus.holder() == Some(player.id);
                info
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

    /// Forgets offers nobody answered in time, so they cannot be accepted
    /// hours later against a hand that has completely changed.
    pub fn expire_stale_trades(&mut self) {
        let now = now_secs();
        self.pending_trades
            .retain(|_, trade| now.saturating_sub(trade.created_at_secs) <= TRADE_LIFETIME_SECS);
    }

    /// Colours a joining player may still pick.
    pub fn available_colours(&self) -> Vec<shared::PlayerColour> {
        self.turn_manager.players.available_colours()
    }

    /// Whether there are enough players seated to begin.
    pub fn can_start(&self) -> bool {
        self.get_state() == GamePhase::WaitingForPlayers
            && self.turn_manager.players.len() >= MIN_PLAYERS
    }

    pub fn touch(&mut self) {
        self.last_activity_secs = now_secs();
    }

    pub fn idle_for_secs(&self) -> u64 {
        now_secs().saturating_sub(self.last_activity_secs)
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
    use crate::errors::GameError;

    fn started_game() -> (GameInstance, Uuid) {
        let pid = Uuid::from_u128(1);
        let mut game = GameInstance::new("test".into(), pid, 1, "Tester", shared::PlayerColour::Blue);
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

    /// A client picks the table size, so it has to be pinned to what the game
    /// can actually seat - there are only four colours.
    #[test]
    fn requested_table_size_is_clamped_to_a_playable_range() {
        let creator = Uuid::from_u128(1);

        assert_eq!(GameInstance::new("a".into(), creator, 99, "Tester", shared::PlayerColour::Blue).max_players, MAX_PLAYERS);
        assert_eq!(GameInstance::new("b".into(), creator, 0, "Tester", shared::PlayerColour::Blue).max_players, MIN_PLAYERS);
        assert_eq!(GameInstance::new("c".into(), creator, 2, "Tester", shared::PlayerColour::Blue).max_players, MIN_PLAYERS);
        assert_eq!(GameInstance::new("d".into(), creator, 4, "Tester", shared::PlayerColour::Blue).max_players, 4);
    }

    /// Seating must report failure rather than silently dropping the player.
    #[test]
    fn seating_fails_once_every_colour_is_taken() {
        let mut game = GameInstance::new("test".into(), Uuid::from_u128(1), 4, "Tester", shared::PlayerColour::Blue);

        // The creator already took Blue.
        for (n, colour) in [
            (2u128, shared::PlayerColour::Red),
            (3, shared::PlayerColour::Green),
            (4, shared::PlayerColour::Yellow),
        ] {
            game.turn_manager
                .players
                .seat(Uuid::from_u128(n), "x", colour)
                .expect("a free colour must be seatable");
        }
        assert_eq!(game.turn_manager.players.len(), 4);
        assert!(game.available_colours().is_empty());

        let err = game
            .turn_manager
            .players
            .seat(Uuid::from_u128(5), "late", shared::PlayerColour::Red)
            .expect_err("a taken colour must be refused");
        assert!(err.contains("Red"), "unexpected error: {err}");
        assert_eq!(game.turn_manager.players.len(), 4, "the roster must not grow");
    }

    #[test]
    fn a_game_cannot_start_below_the_minimum_player_count() {
        let creator = Uuid::from_u128(1);
        let mut game = GameInstance::new("test".into(), creator, 4, "Tester", shared::PlayerColour::Blue);
        assert!(!game.can_start(), "one player is not enough");

        let _ = game.turn_manager.players.seat(Uuid::from_u128(2), "b", shared::PlayerColour::Red);
        assert!(!game.can_start(), "two players is still not enough");

        let _ = game.turn_manager.players.seat(Uuid::from_u128(3), "c", shared::PlayerColour::Green);
        assert!(game.can_start(), "three players may start without filling the table");

        game.start_game();
        assert!(!game.can_start(), "an already started game cannot start again");
    }

    #[test]
    fn stale_trade_offers_expire() {
        use crate::game::entities::pending_trade::PendingTrade;
        use std::collections::HashSet;

        let mut game = GameInstance::new("test".into(), Uuid::from_u128(1), 3, "Tester", shared::PlayerColour::Blue);

        let mut offer = |id: u64, age: u64| {
            game.pending_trades.insert(id, PendingTrade {
                offer_id: id,
                proposer_id: Uuid::from_u128(1),
                target_player_id: None,
                offering: Default::default(),
                requesting: Default::default(),
                declined_by: HashSet::new(),
                created_at_secs: now_secs().saturating_sub(age),
            });
        };
        offer(1, 0);
        offer(2, TRADE_LIFETIME_SECS + 60);

        game.expire_stale_trades();

        assert!(game.pending_trades.contains_key(&1), "a fresh offer survives");
        assert!(!game.pending_trades.contains_key(&2), "an old offer is withdrawn");
    }

    /// Trades used to be dropped entirely on save/load, and the id counter
    /// restarted at 0.
    #[test]
    fn trades_survive_a_save_and_reload() {
        use crate::game::entities::pending_trade::PendingTrade;
        use std::collections::HashSet;

        let mut game = GameInstance::new("test".into(), Uuid::from_u128(1), 3, "Tester", shared::PlayerColour::Blue);
        game.next_trade_id = 7;
        game.pending_trades.insert(6, PendingTrade {
            offer_id: 6,
            proposer_id: Uuid::from_u128(1),
            target_player_id: None,
            offering: Default::default(),
            requesting: Default::default(),
            declined_by: HashSet::new(),
            created_at_secs: now_secs(),
        });

        let json = serde_json::to_value(&game).unwrap();
        let restored: GameInstance = serde_json::from_value(json).unwrap();

        assert!(restored.pending_trades.contains_key(&6));
        assert_eq!(restored.next_trade_id, 7);
    }

    #[test]
    fn touch_resets_the_idle_clock() {
        let mut game = GameInstance::new("test".into(), Uuid::from_u128(1), 2, "Tester", shared::PlayerColour::Blue);

        // Pretend the game has been sitting untouched for two hours.
        game.last_activity_secs = now_secs().saturating_sub(7200);
        assert!(game.idle_for_secs() >= 7200);

        game.touch();
        assert!(game.idle_for_secs() < 5, "touch should reset the idle clock");
    }

    /// Games persisted before activity tracking existed must still load.
    #[test]
    fn missing_activity_timestamp_defaults_instead_of_failing() {
        let game = GameInstance::new("test".into(), Uuid::from_u128(1), 2, "Tester", shared::PlayerColour::Blue);
        let mut json = serde_json::to_value(&game).unwrap();
        json.as_object_mut().unwrap().remove("last_activity_secs");

        let restored: GameInstance =
            serde_json::from_value(json).expect("older saves must still deserialize");
        assert_eq!(restored.last_activity_secs, 0);
    }

    /// Stealing used to be completely unvalidated: anyone could rob anyone,
    /// at any time, as often as they liked.
    #[test]
    fn stealing_requires_having_moved_the_robber() {
        use crate::game::entities::building::VertexBuilding;

        let thief = Uuid::from_u128(1);
        let victim = Uuid::from_u128(2);
        let outsider = Uuid::from_u128(3);

        let mut game = GameInstance::new("test".into(), thief, 3, "Tester", shared::PlayerColour::Blue);
        let _ = game.turn_manager.players.seat(victim, "v", shared::PlayerColour::Red);
        let _ = game.turn_manager.players.seat(outsider, "o", shared::PlayerColour::Green);
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
            game.handle_steal_from_player(thief, victim).is_err(),
            "stealing without moving the robber must be refused"
        );

        // A player who is not on turn may never steal, even once it is unlocked.
        game.add_pending_action(thief, PendingAction::Steal);
        assert!(
            game.handle_steal_from_player(outsider, victim).is_err(),
            "a player who is not on turn must not be able to steal"
        );

        // A victim with nothing next to the robber is not a legal target.
        assert!(
            game.handle_steal_from_player(thief, outsider).is_err(),
            "a player with no building by the robber must not be robbable"
        );

        // The legal steal works exactly once.
        assert!(game.handle_steal_from_player(thief, victim).is_ok());
        assert!(
            game.handle_steal_from_player(thief, victim).is_err(),
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
        assert_eq!(
            err,
            GameError::InvalidPosition.to_string(),
            "players should see the readable message, not the Debug form"
        );

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
