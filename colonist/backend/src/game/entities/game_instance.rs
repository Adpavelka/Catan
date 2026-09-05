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

    /// Players still owed a special building phase before the next turn
    /// begins, in the order they get it. Only ever non-empty at 5-6 players.
    #[serde(default)]
    special_build_queue: Vec<Uuid>,

    /// Wall-clock seconds since the epoch when this game last saw activity.
    /// Used to evict abandoned games; `0` means "never touched".
    #[serde(default)]
    pub last_activity_secs: u64,
}

/// Table sizes we support. The size chosen when the lobby is created fixes the
/// board, the bank and the victory target for the whole game - see `GameRules`.
pub const MIN_PLAYERS: usize = shared::GameRules::MIN_PLAYERS;
pub const MAX_PLAYERS: usize = shared::GameRules::MAX_PLAYERS;

const _: () = assert!(
    shared::PlayerColour::ALL.len() >= MAX_PLAYERS,
    "every seat must be able to claim a distinct colour",
);

/// Trade offers older than this are treated as withdrawn. Shared with the
/// client so its countdown and the server's expiry cannot drift apart.
pub const TRADE_LIFETIME_SECS: u64 = shared::TRADE_LIFETIME_SECS;

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
            special_build_queue: Vec::new(),
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


    /// Drops the game straight into a phase. Tests only - real transitions go
    /// through `start_game`, `advance_phase` and the special-building queue.
    #[cfg(test)]
    pub (crate) fn force_phase_for_test(&mut self, phase: GamePhase) {
        self.phase = phase;
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

            // A special build is paid for normally: free roads come from a
            // Road Building card, which cannot be played outside your turn.
            GamePhase::SpecialBuilding { .. } => Ok(false),

            GamePhase::RegularPlay => {
                let pid = self.active_player();

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

            GamePhase::RegularPlay | GamePhase::SpecialBuilding { .. } => Ok(false),

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
    ///
    /// Returns the offers that were dropped. Callers must tell the table about
    /// them: an offer that vanishes silently leaves the proposer's client
    /// believing it is still open, and they can never propose another.
    pub fn expire_stale_trades(&mut self) -> Vec<u64> {
        let now = now_secs();

        let expired: Vec<u64> = self
            .pending_trades
            .iter()
            .filter(|(_, trade)| now.saturating_sub(trade.created_at_secs) > TRADE_LIFETIME_SECS)
            .map(|(id, _)| *id)
            .collect();

        for id in &expired {
            self.pending_trades.remove(id);
        }

        expired
    }

    /// Drops every open offer. Called when the turn ends: an offer belongs to
    /// the turn it was made in, and must not be settled during someone else's.
    pub fn clear_trades(&mut self) -> Vec<u64> {
        self.pending_trades.drain().map(|(id, _)| id).collect()
    }

    /// Takes a player out of the game and clears everything that referred to
    /// them. The seat is the easy part: what strands a table is the debris -
    /// a discard nobody can now make, a special build nobody can now take, a
    /// trade offer nobody can now answer.
    ///
    /// Returns the trade offers that died with them, for the caller to
    /// announce. The player's buildings stay on the board; they simply stop
    /// producing, since the owner is no longer anybody's resources to gain.
    pub fn remove_player(&mut self, pid: Uuid) -> Result<Vec<u64>, String> {
        let was_on_turn = self.turn_manager.players.get_current_player().id == pid;

        self.turn_manager.players.remove_player(pid)?;

        // A pending action belonging to somebody who has gone can never be
        // discharged, and `end_turn` refuses to advance while a discard is
        // outstanding - so this would freeze the table.
        self.pending_actions.remove(&pid);

        let mut closed: Vec<u64> = Vec::new();
        self.pending_trades.retain(|offer_id, trade| {
            if trade.proposer_id == pid || trade.target_player_id == Some(pid) {
                closed.push(*offer_id);
                return false;
            }
            true
        });
        for trade in self.pending_trades.values_mut() {
            trade.decline(pid);
        }

        self.special_build_queue.retain(|id| *id != pid);

        // If they were the one building, hand the slot on rather than leaving
        // a phase that only a departed player can end.
        if self.phase.special_builder() == Some(pid) && !self.advance_special_building() {
            self.finish_special_building();
            self.turn_manager.end_turn();
        }

        // Removing the player on turn slides the next one into their slot, but
        // the dice still read as rolled for the turn that just vanished.
        if was_on_turn && self.turn_manager.players.len() > 0 {
            self.turn_manager.dice.next_turn();
        }

        Ok(closed)
    }

    /// Who may act right now. During a special building phase that is the
    /// player being offered the build, not the player whose turn it is.
    pub fn active_player(&self) -> Uuid {
        self.phase
            .special_builder()
            .unwrap_or_else(|| self.turn_manager.players.get_current_player().id)
    }

    /// Begins the special building phase after `finished` ends their turn.
    /// Everyone else gets one chance to build, in turn order. Players who
    /// could not afford anything are skipped so the round does not drag.
    ///
    /// Returns false when nobody is owed a phase, and the next turn should
    /// simply begin.
    pub(crate) fn open_special_building(&mut self, finished: Uuid) -> bool {
        if !self.turn_manager.rules().uses_special_building() {
            return false;
        }

        let players = &self.turn_manager.players;
        let count = players.len();
        let start = players.get_current_index();

        self.special_build_queue = (0..count)
            .filter_map(|offset| players.get_by_index((start + offset + 1) % count))
            .filter(|player| player.id != finished)
            .filter(|player| player.can_afford_anything())
            .map(|player| player.id)
            .collect();

        self.advance_special_building()
    }

    /// Moves to the next player owed a special build. Returns false once the
    /// queue is empty and the phase is over.
    pub(crate) fn advance_special_building(&mut self) -> bool {
        if self.special_build_queue.is_empty() {
            return false;
        }

        let builder = self.special_build_queue.remove(0);
        self.phase = GamePhase::SpecialBuilding { builder };
        true
    }

    /// Closes the special building phase and hands play back to the table.
    pub(crate) fn finish_special_building(&mut self) {
        self.special_build_queue.clear();
        self.phase = GamePhase::RegularPlay;
    }

    /// The setup this game is playing under, fixed by the lobby size.
    pub fn rules(&self) -> shared::GameRules {
        self.turn_manager.rules()
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
    /// The lobby size chosen at creation fixes the whole setup.
    #[test]
    fn table_size_drives_the_victory_target_board_and_bank() {
        use shared::{BoardLayout, GameRules};

        // A duel needs a longer race; everything else wins at ten.
        assert_eq!(GameRules::for_player_count(2).victory_points_to_win, 15);
        for n in 3..=6 {
            assert_eq!(GameRules::for_player_count(n).victory_points_to_win, 10);
        }

        // Two to four play the base set; five and six need the extension.
        for n in 2..=4 {
            let rules = GameRules::for_player_count(n);
            assert_eq!(rules.board, BoardLayout::Standard);
            assert_eq!(rules.bank_per_resource, 19);
            assert_eq!(rules.dev_card_total(), 25);
            assert_eq!(rules.port_count, 9);
        }
        for n in 5..=6 {
            let rules = GameRules::for_player_count(n);
            assert_eq!(rules.board, BoardLayout::Extended);
            assert_eq!(rules.bank_per_resource, 24);
            assert_eq!(rules.dev_card_total(), 34);
            assert_eq!(rules.port_count, 11);
        }
    }

    /// The rules reach the actual game, not just the table.
    #[test]
    fn a_two_player_game_is_actually_built_to_fifteen_points() {
        let creator = Uuid::from_u128(1);

        let duel = GameInstance::new("a".into(), creator, 2, "T", shared::PlayerColour::Blue);
        assert_eq!(duel.rules().victory_points_to_win, 15);
        assert_eq!(duel.turn_manager.board.hexes.len(), 19);

        let six = GameInstance::new("b".into(), creator, 6, "T", shared::PlayerColour::Blue);
        assert_eq!(six.rules().victory_points_to_win, 10);
        assert_eq!(six.turn_manager.board.hexes.len(), 30, "six players need the big board");
    }

    /// A two-player game must not end at ten points.
    #[test]
    fn ten_points_does_not_win_a_two_player_game() {
        let creator = Uuid::from_u128(1);
        let mut game = GameInstance::new("a".into(), creator, 2, "T", shared::PlayerColour::Blue);

        for _ in 0..10 {
            game.turn_manager
                .players
                .get_mut(creator)
                .unwrap()
                .add_secret_victory_point();
        }
        assert!(!game.turn_manager.game_over(), "ten is not enough for a duel");

        for _ in 0..5 {
            game.turn_manager
                .players
                .get_mut(creator)
                .unwrap()
                .add_secret_victory_point();
        }
        game.turn_manager.check_for_winner_for_test();
        assert!(game.turn_manager.game_over(), "fifteen should win");
    }

    #[test]
    fn requested_table_size_is_clamped_to_a_playable_range() {
        let creator = Uuid::from_u128(1);

        assert_eq!(GameInstance::new("a".into(), creator, 99, "Tester", shared::PlayerColour::Blue).max_players, MAX_PLAYERS);
        assert_eq!(GameInstance::new("b".into(), creator, 0, "Tester", shared::PlayerColour::Blue).max_players, MIN_PLAYERS);
        for n in MIN_PLAYERS..=MAX_PLAYERS {
            let game = GameInstance::new("c".into(), creator, n, "Tester", shared::PlayerColour::Blue);
            assert_eq!(game.max_players, n, "a table of {n} should be honoured");
        }
    }

    /// Seating must report failure rather than silently dropping the player.
    #[test]
    fn seating_rejects_a_colour_another_player_holds() {
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
        assert!(game.can_start(), "two players may start without filling the table");

        let _ = game.turn_manager.players.seat(Uuid::from_u128(3), "c", shared::PlayerColour::Green);
        assert!(game.can_start());

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
                accepted_by: Vec::new(),
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
                accepted_by: Vec::new(),
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

    /// A five-player table with everybody holding enough to build.
    fn five_player_game() -> (GameInstance, Vec<Uuid>) {
        use crate::game::entities::resources::ResourceType;

        let ids: Vec<Uuid> = (1..=5u128).map(Uuid::from_u128).collect();
        let mut game =
            GameInstance::new("t".into(), ids[0], 5, "p1", shared::PlayerColour::Blue);

        for (i, colour) in shared::PlayerColour::ALL.iter().skip(1).take(4).enumerate() {
            game.turn_manager
                .players
                .seat(ids[i + 1], &format!("p{}", i + 2), *colour)
                .unwrap();
        }

        game.start_game();
        game.advance_phase();
        game.advance_phase(); // RegularPlay

        for id in &ids {
            let player = game.turn_manager.players.get_mut(*id).unwrap();
            for res in [ResourceType::Wood, ResourceType::Brick] {
                player.resources.add(res, 5);
            }
        }

        (game, ids)
    }

    #[test]
    fn ending_a_turn_opens_a_special_build_for_everyone_else() {
        let (mut game, ids) = five_player_game();
        let first = game.turn_manager.players.get_current_player().id;

        game.turn_manager.roll_dice().unwrap();
        game.handle_end_turn(first).unwrap();

        // The turn has not advanced yet; the next player is building instead.
        let mut offered = Vec::new();
        while let Some(builder) = game.get_state().special_builder() {
            offered.push(builder);
            game.handle_end_turn(builder).unwrap();
        }

        assert_eq!(offered.len(), 4, "everyone except the player who just went");
        assert!(!offered.contains(&first), "the finished player does not build again");
        for id in &ids {
            if *id != first {
                assert!(offered.contains(id), "{id} was skipped");
            }
        }

        assert_eq!(game.get_state(), GamePhase::RegularPlay);
        assert_ne!(
            game.turn_manager.players.get_current_player().id,
            first,
            "the turn advances once the phase is over"
        );
    }

    #[test]
    fn a_four_player_game_has_no_special_building() {
        let creator = Uuid::from_u128(1);
        let mut game = GameInstance::new("t".into(), creator, 4, "p1", shared::PlayerColour::Blue);
        for (i, colour) in shared::PlayerColour::ALL.iter().skip(1).take(3).enumerate() {
            game.turn_manager
                .players
                .seat(Uuid::from_u128(i as u128 + 2), "p", *colour)
                .unwrap();
        }
        game.start_game();
        game.advance_phase();
        game.advance_phase();

        let first = game.turn_manager.players.get_current_player().id;
        game.turn_manager.roll_dice().unwrap();
        game.handle_end_turn(first).unwrap();

        assert_eq!(game.get_state(), GamePhase::RegularPlay, "no special build below five");
        assert_ne!(game.turn_manager.players.get_current_player().id, first);
    }

    /// Only the player being offered the build may act, and only to build.
    #[test]
    fn a_special_build_permits_building_but_not_playing_the_turn() {
        let (mut game, _) = five_player_game();
        let first = game.turn_manager.players.get_current_player().id;

        game.turn_manager.roll_dice().unwrap();
        game.handle_end_turn(first).unwrap();

        let builder = game.get_state().special_builder().expect("a build is offered");
        assert_ne!(builder, first);

        // Somebody else's special build is not an invitation to act.
        assert!(game.handle_end_turn(first).is_err(), "only the builder may pass");

        // No rolling, no trading, no development cards.
        assert!(game.handle_roll_dice(builder).is_err(), "rolling is not allowed");
        assert!(
            game.handle_bank_trade(builder, shared::ResourceType::Wood, shared::ResourceType::Ore).is_err(),
            "trading is not allowed"
        );
        assert!(
            game.handle_play_dev_card(builder, shared::DevCardType::Knight, None).is_err(),
            "playing a development card is not allowed"
        );

        // Building is.
        assert!(game.validate_settlement_placement().is_ok());
        assert_eq!(game.active_player(), builder, "the builder is the one who may act");
    }

    /// A player who cannot afford anything has nothing to do, so is skipped
    /// rather than stalling the table.
    #[test]
    fn players_who_can_afford_nothing_are_skipped() {
        let (mut game, ids) = five_player_game();
        let first = game.turn_manager.players.get_current_player().id;

        // Strip everyone but one player bare.
        let keep = *ids.iter().find(|id| **id != first).unwrap();
        for id in &ids {
            if *id != keep {
                game.turn_manager.players.get_mut(*id).unwrap().resources =
                    crate::game::entities::resources::ResourceSet::new();
            }
        }

        game.turn_manager.roll_dice().unwrap();
        game.handle_end_turn(first).unwrap();

        let mut offered = Vec::new();
        while let Some(builder) = game.get_state().special_builder() {
            offered.push(builder);
            game.handle_end_turn(builder).unwrap();
        }

        assert_eq!(offered, vec![keep], "only the player who can buy is offered");
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

    /// A three-player game where everyone is seated and play has begun.
    fn seated_game(n: usize) -> (GameInstance, Vec<Uuid>) {
        let ids: Vec<Uuid> = (1..=n as u128).map(Uuid::from_u128).collect();
        let mut game =
            GameInstance::new("t".into(), ids[0], n, "p1", shared::PlayerColour::Blue);
        for (i, colour) in shared::PlayerColour::ALL.iter().skip(1).take(n - 1).enumerate() {
            game.turn_manager.players.seat(ids[i + 1], "p", *colour).unwrap();
        }
        game.start_game();
        game.advance_phase();
        game.advance_phase(); // RegularPlay
        (game, ids)
    }

    /// A discard owed by somebody who has left can never be made, and
    /// `end_turn` refuses to advance while one is outstanding.
    #[test]
    fn leaving_clears_the_pending_actions_that_would_freeze_the_table() {
        let (mut game, ids) = seated_game(3);
        game.add_pending_action(ids[2], PendingAction::Discard);

        game.remove_player(ids[2]).unwrap();

        assert!(!game.pending_actions.contains_key(&ids[2]));
        game.turn_manager.roll_dice().unwrap();
        game.handle_end_turn(ids[0]).expect("the turn must still be endable");
    }

    /// The departed player's offers die with them, and their acceptance of
    /// somebody else's offer is taken back.
    #[test]
    fn leaving_closes_the_trades_that_referred_to_them() {
        use crate::game::entities::resources::ResourceType;
        let (mut game, ids) = seated_game(3);

        for id in &ids {
            game.turn_manager.players.get_mut(*id).unwrap()
                .resources.add(ResourceType::Brick, 5);
        }

        let give = shared::Resources { brick: 1, ..Default::default() };
        let want = shared::Resources { lumber: 1, ..Default::default() };
        let mine = match game.handle_trade_offer(ids[0], None, give, want).unwrap() {
            shared::ServerMessage::TradeProposed { offer_id, .. } => offer_id,
            other => panic!("{other:?}"),
        };

        let closed = game.remove_player(ids[0]).unwrap();
        assert_eq!(closed, vec![mine], "their own offer is withdrawn");
        assert!(game.pending_trades.is_empty());
    }

    /// If the player currently taking a special build leaves, the slot has to
    /// pass on. Otherwise `active_player` names a ghost and nobody can act.
    #[test]
    fn leaving_mid_special_build_hands_the_slot_on() {
        let (mut game, ids) = seated_game(5);
        let builder = ids[2];
        game.force_phase_for_test(GamePhase::SpecialBuilding { builder });

        game.remove_player(builder).unwrap();

        assert_ne!(
            game.get_state().special_builder(),
            Some(builder),
            "a departed player must not still hold the build slot"
        );
        assert!(
            game.turn_manager.players.get(game.active_player()).is_some(),
            "whoever may act now must actually be at the table"
        );
    }

    /// Removing the player on turn slides the next one into their slot. The
    /// dice must be reset or that player inherits an already-rolled turn.
    #[test]
    fn the_next_player_gets_a_fresh_turn_when_the_current_one_leaves() {
        let (mut game, ids) = seated_game(3);
        let first = game.turn_manager.players.get_current_player().id;
        game.turn_manager.roll_dice().unwrap();
        assert!(game.turn_manager.dice.was_dice_rolled());

        game.remove_player(first).unwrap();

        assert!(
            !game.turn_manager.dice.was_dice_rolled(),
            "the incoming player must be able to roll"
        );
        assert!(game.turn_manager.players.get(ids[0]).is_none());
    }
}
