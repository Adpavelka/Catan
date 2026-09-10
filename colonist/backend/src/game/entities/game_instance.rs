use crate::game::entities::bonus_points::BonusCard;
use crate::game::entities::pending_trade::PendingTrade;
use crate::game::entities::statistics::Statistics;
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

    /// Whose turn the clock is currently running for, and when it started.
    /// Comparing the player rather than hooking every transition means the
    /// clock re-arms itself whenever the turn changes hands, however it
    /// changed - end of turn, a special build finishing, a player leaving.
    #[serde(default)]
    clock_player: Option<Uuid>,
    #[serde(default)]
    clock_started_secs: u64,
    /// The turn the clock was armed for. Keyed off the turn counter rather
    /// than the player: with one player left the player never changes, so the
    /// clock never re-armed and fired its deadline again every single tick.
    #[serde(default)]
    clock_turn_seq: Option<u64>,

    /// The running tally behind the end-of-game screen. Counters only - see
    /// `statistics`. Defaulted so games saved before it existed still load,
    /// and simply report an empty table.
    #[serde(default)]
    pub stats: Statistics,
}

/// What the turn clock wants done, once a deadline has passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnClockAction {
    Roll,
    EndTurn,
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
            clock_player: None,
            clock_started_secs: 0,
            clock_turn_seq: None,
            stats: Statistics::default(),
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
        // The clock measures play, not how long people took to sit down.
        self.stats.start(now_secs());
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

    /// What the turn clock says should happen now, if anything.
    ///
    /// Re-arms itself when the turn changes hands, and refuses to force
    /// anything while the player still owes the game an answer - a discard, a
    /// robber move, a resource pick - because those are the player's to make
    /// and skipping them would corrupt the position.
    pub fn turn_clock_due(&mut self) -> Option<TurnClockAction> {
        if self.phase != GamePhase::RegularPlay {
            self.clock_player = None;
            self.clock_turn_seq = None;
            return None;
        }

        let current = self.turn_manager.players.get_current_player().id;
        let seq = self.turn_manager.turn_seq();
        let now = now_secs();

        if self.clock_player != Some(current) || self.clock_turn_seq != Some(seq) {
            self.clock_player = Some(current);
            self.clock_turn_seq = Some(seq);
            self.clock_started_secs = now;
            return None;
        }

        if self.pending_actions.contains_key(&current)
            || self
                .pending_actions
                .values()
                .any(|actions| actions.contains(&PendingAction::Discard))
        {
            return None;
        }

        let elapsed = now.saturating_sub(self.clock_started_secs);

        if !self.turn_manager.dice.was_dice_rolled() {
            (elapsed >= shared::TURN_AUTO_ROLL_SECS).then_some(TurnClockAction::Roll)
        } else {
            (elapsed >= shared::TURN_LIMIT_SECS).then_some(TurnClockAction::EndTurn)
        }
    }

    /// The open offers `pid` is entitled to see, for restoring their trade
    /// panel after a reconnect: their own, plus any they may answer.
    ///
    /// Acceptances are public, so they travel; `declined_by` does not, beyond
    /// telling this player whether they themselves refused.
    pub fn trade_snapshots_for(&self, pid: Uuid) -> Vec<shared::TradeSnapshot> {
        let now = now_secs();

        let mut snapshots: Vec<shared::TradeSnapshot> = self
            .pending_trades
            .values()
            .filter(|trade| trade.proposer_id == pid || trade.is_open_to(pid))
            .map(|trade| {
                let age = now.saturating_sub(trade.created_at_secs);

                shared::TradeSnapshot {
                    offer_id: trade.offer_id,
                    proposer_id: trade.proposer_id,
                    target_player_id: trade.target_player_id,
                    offering: trade.offering.clone(),
                    requesting: trade.requesting.clone(),
                    accepted_by: trade.accepted_by.clone(),
                    offering_any: trade.offering_any,
                    requesting_any: trade.requesting_any,
                    seconds_remaining: TRADE_LIFETIME_SECS.saturating_sub(age),
                    you_declined: trade.declined_by.contains(&pid),
                    counters: trade.counters,
                }
            })
            .collect();

        // `pending_trades` is a map, so fix an order the client can rely on.
        snapshots.sort_by_key(|snapshot| snapshot.offer_id);
        snapshots
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

    /// What a roll of `total` would have paid out, had the robber not been
    /// standing on the tile. One entry per player who lost cards to it.
    ///
    /// Worked out here rather than inside the bank: the bank's payout loop
    /// skips the robbed hex before it knows who was standing next to it, and
    /// threading a second return value through it to serve a counter would put
    /// bookkeeping in the middle of the rules.
    pub(crate) fn robber_blocked_payout(&self, total: u8) -> Vec<(Uuid, u32)> {
        // A seven pays nobody, so it blocks nothing either.
        if total == 7 {
            return Vec::new();
        }

        let board = &self.turn_manager.board;
        let Some(hex) = board.hexes.get(&self.turn_manager.get_robber_pos()) else {
            return Vec::new();
        };
        if hex.number != total || hex.resource == shared::ResourceType::Desert {
            return Vec::new();
        }

        let mut blocked: Vec<(Uuid, u32)> = Vec::new();
        for coord in &hex.adjacent_vertices {
            let Some(vertex) = board.vertices.get(coord) else { continue };
            let (Some(building), Some(owner)) = (&vertex.building, vertex.owner) else { continue };

            let amount = building.production();
            match blocked.iter_mut().find(|(id, _)| *id == owner) {
                Some((_, running)) => *running += amount,
                None => blocked.push((owner, amount)),
            }
        }

        blocked
    }

    /// The finished tally, ready to send. Combines the counters kept as the
    /// game ran with what can simply be read off the final position - the
    /// points breakdown, the pieces on the board - so nothing that the board
    /// already knows is tracked twice and risks drifting from it.
    pub fn stats_snapshot(&self) -> shared::GameStats {
        use crate::game::entities::building::VertexBuilding;

        let turn_manager = &self.turn_manager;
        let road_holder = turn_manager.road_bonus.holder();
        let army_holder = turn_manager.army_bonus.holder();
        let road_points = turn_manager.road_bonus.points();
        let army_points = turn_manager.army_bonus.points();

        let players = (0..turn_manager.players.len())
            .filter_map(|index| turn_manager.players.get_by_index(index))
            .map(|player| {
                let mut settlements = 0u8;
                let mut cities = 0u8;
                for vertex in turn_manager.board.vertices.values() {
                    if vertex.owner != Some(player.id) {
                        continue;
                    }
                    match vertex.building {
                        Some(VertexBuilding::Settlement) => settlements += 1,
                        // A city is worth two: the point the settlement it
                        // replaced already carried, and the one the upgrade
                        // added.
                        Some(VertexBuilding::City) => cities += 2,
                        None => {}
                    }
                }

                let points = shared::PointBreakdown {
                    settlements,
                    cities,
                    dev_cards: player.get_secret_victory_points(),
                    longest_road: if road_holder == Some(player.id) { road_points } else { 0 },
                    largest_army: if army_holder == Some(player.id) { army_points } else { 0 },
                };

                let tally = self.stats.player(player.id);

                shared::PlayerStats {
                    player_id: player.id,
                    name: player.name.clone(),
                    colour: player.colour,
                    victory_points: player.get_total_victory_points(),
                    points,
                    dice_rolls: tally.rolls,
                    resources: tally.resources,
                    dev_cards_bought: tally.dev_cards_bought,
                    dev_cards_played: tally.dev_cards_played,
                    activity: tally.activity,
                    settlements_built: tally.settlements_built,
                    cities_built: tally.cities_built,
                    roads_built: tally.roads_built,
                    knights_played: player.knight_played as u32,
                    longest_road_length: player.longest_road as u32,
                }
            })
            .collect();

        shared::GameStats {
            duration_secs: self.stats.duration_secs(now_secs()),
            turns: turn_manager.turn_seq(),
            winner: turn_manager.winner(),
            victory_points_to_win: turn_manager.rules().victory_points_to_win,
            dice_rolls: self.stats.table_rolls(),
            resource_draws: self.stats.resource_draws(),
            players,
        }
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
                offering_any: 0,
                requesting_any: 0,
                counters: None,
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
                offering_any: 0,
                requesting_any: 0,
                counters: None,
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
        let mine = match game.handle_trade_offer(ids[0], None, give, want, 0, 0).unwrap() {
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

    // ------------------------------------------------------- statistics

    /// Puts cards in a hand the way the game itself would: the resources and
    /// the counter that records them move together. That is what lets the
    /// following tests start from a balanced position and check that every
    /// handler keeps it balanced.
    fn grant(
        game: &mut GameInstance,
        pid: Uuid,
        res: crate::game::entities::resources::ResourceType,
        n: u32,
    ) {
        game.turn_manager
            .players
            .get_mut(pid)
            .unwrap()
            .resources
            .add(res, n);
        game.stats
            .gained(pid, crate::game::entities::statistics::Gain::Setup, n);
    }

    /// The invariant the whole resource page rests on: what came in, less what
    /// went out, is what is still in the hand. A hook that forgets to count a
    /// movement - or counts it twice - breaks this and nothing else.
    fn assert_books_balance(game: &GameInstance, ids: &[Uuid]) {
        for id in ids {
            let Some(player) = game.turn_manager.players.get(*id) else { continue };
            let held = player.resources.get_cards_total() as i64;
            let scored = game.stats.player(*id).resources.score();
            assert_eq!(
                scored, held,
                "player {id}: the tally says {scored} cards, the hand holds {held}"
            );
        }
    }

    #[test]
    fn the_statistics_follow_every_card_that_moves() {
        use crate::game::entities::resources::ResourceType;

        let (mut game, ids) = seated_game(3);
        let (a, b) = (ids[0], ids[1]);

        grant(&mut game, a, ResourceType::Wood, 4);
        grant(&mut game, a, ResourceType::Sheep, 1);
        grant(&mut game, a, ResourceType::Wheat, 1);
        grant(&mut game, a, ResourceType::Ore, 1);
        grant(&mut game, a, ResourceType::Brick, 1);
        grant(&mut game, b, ResourceType::Ore, 3);
        assert_books_balance(&game, &ids);

        // Four wood over the counter for one brick.
        game.handle_bank_trade(a, shared::ResourceType::Wood, shared::ResourceType::Brick)
            .expect("a 4:1 trade the player can cover");
        let flow = game.stats.player(a).resources;
        assert_eq!(flow.lost_by_trading, 4, "the whole stack left the hand, not one card");
        assert_eq!(flow.gained_by_trading, 1);
        assert_eq!(game.stats.player(a).activity.trades_completed, 1);
        assert_books_balance(&game, &ids);

        // A development card: three cards to the bank, one card back.
        game.handle_buy_dev_card(a).expect("sheep, wheat and ore are in hand");
        let tally = game.stats.player(a);
        assert_eq!(tally.dev_cards_bought.total(), 1);
        assert_eq!(tally.activity.dev_cards_bought, 1);
        assert_eq!(tally.resources.spent, 3, "a development card is paid for, not traded");
        assert_books_balance(&game, &ids);

        // A seven, and a card handed back.
        game.add_pending_action(a, PendingAction::Discard);
        game.handle_discard_cards(a, shared::Resources { brick: 1, ..Default::default() })
            .expect("the brick is in hand");
        assert_eq!(game.stats.player(a).resources.lost_by_seven, 1);
        assert_books_balance(&game, &ids);

        // Monopoly: every ore at the table changes hands at once.
        game.add_pending_action(a, PendingAction::Monopoly);
        game.handle_monopoly_choice(a, shared::ResourceType::Ore)
            .expect("the call is pending");
        assert_eq!(game.stats.player(a).resources.gained_by_dev_cards, 3);
        assert_eq!(
            game.stats.player(b).resources.lost_by_dev_cards,
            3,
            "the victim's loss is counted, not just the caller's gain"
        );
        assert_books_balance(&game, &ids);
    }

    /// Initial placement, played out on a real board through the same
    /// handlers the clients call. The starting hand is the only source of
    /// cards at that point, so what the bank dealt and what the players hold
    /// have to be the same figure - which is exactly what would break if the
    /// setup payout were counted in one place and not the other.
    #[test]
    fn setup_on_a_real_board_agrees_with_what_the_bank_dealt() {
        let pid = Uuid::from_u128(1);
        let mut game =
            GameInstance::new("t".into(), pid, 2, "Solo", shared::PlayerColour::Blue);
        game.start_game();

        // Round one: a settlement and a road touching it. Nothing is paid out.
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

        assert_eq!(
            game.stats.resource_draws(),
            [0; 5],
            "the first round pays nothing"
        );

        // Round two: the settlement pays a starting hand off its tiles.
        game.advance_phase();
        let second = free_vertex(&game, Some(first));
        game.handle_build_settlement(pid, second.0, second.1).unwrap();
        let road = *game
            .turn_manager
            .board
            .vertices
            .get(&second)
            .unwrap()
            .adjacent_edges
            .iter()
            .next()
            .unwrap();
        game.handle_build_road(pid, road.0, road.1).unwrap();

        let dealt: u32 = game.stats.resource_draws().iter().sum();
        let held = game
            .turn_manager
            .players
            .get(pid)
            .unwrap()
            .resources
            .get_cards_total();

        assert!(dealt > 0, "a second settlement touches at least one producing tile");
        assert_eq!(dealt, held, "every card dealt should be in the hand");
        assert_books_balance(&game, &[pid]);

        let tally = game.stats.player(pid);
        assert_eq!(tally.settlements_built, 2);
        assert_eq!(tally.roads_built, 2);
        assert_eq!(
            tally.resources.spent, 0,
            "setup placements are free, so nothing was spent"
        );

        // And the snapshot reports it all without inventing anything.
        let snapshot = game.stats_snapshot();
        assert_eq!(snapshot.resource_draws.iter().sum::<u32>(), dealt);
        assert_eq!(snapshot.players.len(), 1);
        assert_eq!(snapshot.players[0].settlements_built, 2);
    }

    /// Only the bank deals cards. A trade between two players moves cards
    /// that were drawn once already, and counting them again would make the
    /// island look twice as productive as it was.
    #[test]
    fn only_cards_off_the_bank_count_as_drawn() {
        use crate::game::entities::resources::ResourceType;

        let (mut game, ids) = seated_game(3);
        let (a, b) = (ids[0], ids[1]);

        grant(&mut game, a, ResourceType::Wood, 4);
        grant(&mut game, b, ResourceType::Brick, 1);
        assert_eq!(
            game.stats.resource_draws(),
            [0; 5],
            "seeding a hand directly is not a draw"
        );

        // Over the counter: four wood back to the bank, one brick out of it.
        game.handle_bank_trade(a, shared::ResourceType::Wood, shared::ResourceType::Brick)
            .expect("a 4:1 trade");
        assert_eq!(game.stats.draws_of_for_test(shared::ResourceType::Brick), 1);
        assert_eq!(
            game.stats.draws_of_for_test(shared::ResourceType::Wood),
            0,
            "cards handed back are not drawn"
        );

        // Year of Plenty takes two more straight off the bank.
        game.add_pending_action(a, PendingAction::YearOfPlenty);
        game.handle_year_of_plenty_choice(a, shared::ResourceType::Ore, shared::ResourceType::Ore)
            .expect("the call is pending");
        assert_eq!(game.stats.draws_of_for_test(shared::ResourceType::Ore), 2);

        // A player-to-player trade moves cards that already exist.
        let before = game.stats.resource_draws();
        let offer = shared::Resources { brick: 1, ..Default::default() };
        let want = shared::Resources { lumber: 1, ..Default::default() };
        let offer_id = match game.handle_trade_offer(a, None, offer, want, 0, 0).unwrap() {
            shared::ServerMessage::TradeProposed { offer_id, .. } => offer_id,
            other => panic!("{other:?}"),
        };
        grant(&mut game, b, ResourceType::Wood, 1);
        game.handle_trade_response(b, offer_id, true).unwrap();
        game.handle_confirm_trade(a, offer_id, b).unwrap();

        assert_eq!(
            game.stats.resource_draws(),
            before,
            "a trade between players draws nothing"
        );
    }

    /// A steal is one card off one player and onto another. Both halves have
    /// to be counted, or the table's books stop adding up.
    #[test]
    fn a_steal_is_counted_on_both_sides() {
        use crate::game::entities::building::VertexBuilding;
        use crate::game::entities::resources::ResourceType;

        let (mut game, ids) = seated_game(3);
        let (thief, victim) = (ids[0], ids[1]);

        grant(&mut game, victim, ResourceType::Wood, 3);

        let robber_hex = game.turn_manager.get_robber_pos();
        let corner = crate::game::entities::board::Board::get_adjacent_hexes(robber_hex)[0];
        game.turn_manager
            .board
            .build_vertex(victim, corner, VertexBuilding::Settlement);

        game.add_pending_action(thief, PendingAction::Steal);
        game.handle_steal_from_player(thief, victim).expect("a legal steal");

        assert_eq!(game.stats.player(thief).resources.gained_by_robbing, 1);
        assert_eq!(game.stats.player(victim).resources.lost_by_robber, 1);
        assert_books_balance(&game, &ids);
    }

    /// Robbing somebody with nothing takes nothing, so it must count nothing.
    #[test]
    fn robbing_an_empty_hand_counts_nothing() {
        use crate::game::entities::building::VertexBuilding;

        let (mut game, ids) = seated_game(3);
        let (thief, victim) = (ids[0], ids[1]);

        let robber_hex = game.turn_manager.get_robber_pos();
        let corner = crate::game::entities::board::Board::get_adjacent_hexes(robber_hex)[0];
        game.turn_manager
            .board
            .build_vertex(victim, corner, VertexBuilding::Settlement);

        game.add_pending_action(thief, PendingAction::Steal);
        game.handle_steal_from_player(thief, victim).expect("a legal attempt");

        assert_eq!(game.stats.player(thief).resources.gained_by_robbing, 0);
        assert_eq!(game.stats.player(victim).resources.lost_by_robber, 0);
    }

    /// A trade is a gain and a loss at once, for both players.
    #[test]
    fn a_settled_trade_is_counted_in_both_directions() {
        use crate::game::entities::resources::ResourceType;

        let (mut game, ids) = seated_game(3);
        let (giver, taker) = (ids[0], ids[1]);

        grant(&mut game, giver, ResourceType::Brick, 2);
        grant(&mut game, taker, ResourceType::Wood, 1);

        let offer = shared::Resources { brick: 2, ..Default::default() };
        let want = shared::Resources { lumber: 1, ..Default::default() };
        let offer_id = match game.handle_trade_offer(giver, None, offer, want, 0, 0).unwrap() {
            shared::ServerMessage::TradeProposed { offer_id, .. } => offer_id,
            other => panic!("{other:?}"),
        };
        assert_eq!(game.stats.player(giver).activity.trades_proposed, 1);

        game.handle_trade_response(taker, offer_id, true).unwrap();
        game.handle_confirm_trade(giver, offer_id, taker).unwrap();

        let giver_flow = game.stats.player(giver).resources;
        let taker_flow = game.stats.player(taker).resources;
        assert_eq!((giver_flow.lost_by_trading, giver_flow.gained_by_trading), (2, 1));
        assert_eq!((taker_flow.lost_by_trading, taker_flow.gained_by_trading), (1, 2));
        assert_books_balance(&game, &ids);
    }

    /// The points breakdown is read off the final position, so it has to add
    /// up to the score the game itself is using to decide who won.
    #[test]
    fn the_points_breakdown_adds_up_to_the_score() {
        use crate::game::entities::building::VertexBuilding;

        let (mut game, ids) = seated_game(3);
        let player = ids[0];

        // Two settlements, one of them upgraded, and a hidden point.
        let mut spots: Vec<(i32, i32)> = game
            .turn_manager
            .board
            .vertices
            .keys()
            .copied()
            .collect();
        spots.sort();
        game.turn_manager.board.build_vertex(player, spots[0], VertexBuilding::Settlement);
        game.turn_manager.players.get_mut(player).unwrap().use_settlement().unwrap();
        game.turn_manager.board.build_vertex(player, spots[9], VertexBuilding::City);
        game.turn_manager.players.get_mut(player).unwrap().use_settlement().unwrap();
        game.turn_manager.players.get_mut(player).unwrap().use_city().unwrap();
        game.turn_manager.players.get_mut(player).unwrap().add_secret_victory_point();

        let stats = game.stats_snapshot();
        let them = stats.player(player).expect("in the table");

        assert_eq!(them.points.settlements, 1);
        assert_eq!(them.points.cities, 2, "a city carries the settlement's point as well");
        assert_eq!(them.points.dev_cards, 1);
        assert_eq!(
            them.points.total(),
            them.victory_points,
            "the breakdown must account for every point the game is counting"
        );
    }

    /// Everybody at the table appears, in seat order, whether or not they ever
    /// did anything worth counting.
    #[test]
    fn the_snapshot_lists_every_seat_in_order() {
        for count in 2..=6usize {
            let (game, ids) = seated_game(count);
            let stats = game.stats_snapshot();

            assert_eq!(stats.players.len(), count, "a table of {count}");
            let listed: Vec<Uuid> = stats.players.iter().map(|p| p.player_id).collect();
            assert_eq!(listed, ids, "seat order must be the same on every page");

            for player in &stats.players {
                assert!(!player.name.is_empty(), "a player without a name cannot be labelled");
                assert_eq!(player.resources.gained_total, 0);
            }
            assert_eq!(stats.victory_points_to_win, game.rules().victory_points_to_win);
        }
    }

    /// The robber is worth counting for what it stops, not just what it takes.
    #[test]
    fn the_robber_blocking_a_tile_is_counted() {
        use crate::game::entities::building::VertexBuilding;

        let (mut game, ids) = seated_game(3);
        let owner = ids[1];

        // Find a producing tile, park the robber on it, and put a city on one
        // of its corners.
        let (coord, number) = game
            .turn_manager
            .board
            .hexes
            .values()
            .find(|hex| hex.resource != shared::ResourceType::Desert)
            .map(|hex| (hex.coord, hex.number))
            .expect("every board has producing tiles");

        let corner = game.turn_manager.board.hexes.get(&coord).unwrap().adjacent_vertices[0];
        game.turn_manager.board.build_vertex(owner, corner, VertexBuilding::City);
        game.turn_manager.move_robber(coord).unwrap();

        let blocked = game.robber_blocked_payout(number);
        assert_eq!(blocked, vec![(owner, 2)], "a city loses two cards, not one");

        assert!(
            game.robber_blocked_payout(7).is_empty(),
            "a seven pays nobody, so it blocks nobody"
        );
    }

    /// Regression: the clock re-armed by comparing whose turn it was. With one
    /// player left that never changes, so the deadline stayed passed and the
    /// sweep ended their turn again on every tick.
    #[test]
    fn the_clock_re_arms_even_when_the_same_player_goes_again() {
        let pid = Uuid::from_u128(1);
        let mut game =
            GameInstance::new("t".into(), pid, 2, "solo", shared::PlayerColour::Blue);
        game.start_game();
        game.advance_phase();
        game.advance_phase(); // RegularPlay

        // Arm the clock, then wind it past the roll deadline.
        assert_eq!(game.turn_clock_due(), None, "first tick only arms it");
        game.clock_started_secs = now_secs() - shared::TURN_AUTO_ROLL_SECS - 1;
        assert_eq!(game.turn_clock_due(), Some(TurnClockAction::Roll));

        game.turn_manager.roll_dice().unwrap();
        game.clock_started_secs = now_secs() - shared::TURN_LIMIT_SECS - 1;
        assert_eq!(game.turn_clock_due(), Some(TurnClockAction::EndTurn));

        // The turn ends and comes straight back to the same player.
        game.handle_end_turn(pid).unwrap();
        assert_eq!(
            game.turn_manager.players.get_current_player().id,
            pid,
            "a one-player table hands the turn back to the same player"
        );

        assert_eq!(
            game.turn_clock_due(),
            None,
            "the new turn must get a fresh clock, not inherit the expired one"
        );
        assert_eq!(game.turn_clock_due(), None, "and stay fresh on the next tick");
    }
}
