//! The running tally behind the end-of-game statistics screen.
//!
//! Counters only. Nothing here decides anything about the game: the rules
//! call in after a move has already succeeded, so a rejected action leaves no
//! trace and a counter can never be the reason something is allowed.
//!
//! It lives on `GameInstance` rather than on the players because a game that
//! is saved and reloaded has to keep its history, and because a player who
//! walks out still belongs in the final table - `Players` drops them, this
//! does not.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use shared::{ActivityTally, DevCardTally, DevCardType, ResourceFlow, ResourceType, DICE_TOTALS};
use uuid::Uuid;

/// Why a player gained cards. Each variant is a column on the resource page,
/// except `Setup`, which has no column: the starting hand is part of the
/// total but is not something a player *did*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gain {
    Roll,
    Trade,
    DevCard,
    Robbery,
    Setup,
}

/// Why a player lost cards. `Spending` is paying the bank for a building or a
/// development card; it counts towards the total lost but is reported on the
/// activity page rather than beside the losses to other players.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loss {
    Trade,
    DevCard,
    Robber,
    Seven,
    Spending,
}

/// One player's counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerTally {
    pub rolls: [u32; DICE_TOTALS],
    pub resources: ResourceFlow,
    pub dev_cards_bought: DevCardTally,
    pub dev_cards_played: DevCardTally,
    pub activity: ActivityTally,
    pub settlements_built: u32,
    pub cities_built: u32,
    pub roads_built: u32,
}

/// Everything counted over the life of one game.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Statistics {
    /// Unix seconds when play began, or `0` before it does.
    started_at_secs: u64,
    /// Unix seconds when the game was won. `None` while it is still running,
    /// so a snapshot taken mid-game reports the time so far.
    ended_at_secs: Option<u64>,

    /// Every roll at the table, indexed by `shared::dice_index`.
    rolls: [u32; DICE_TOTALS],

    /// Cards drawn from the bank, by resource, indexed by
    /// `ResourceType::card_index`. Global rather than per player: the question
    /// this answers is what the island produced, not who got it.
    #[serde(default)]
    resource_draws: [u32; 5],

    /// Keyed by player, so a departed player keeps their history.
    #[serde(default)]
    players: HashMap<Uuid, PlayerTally>,
}

impl Statistics {
    /// Starts the clock. Called when the game leaves the lobby, so the
    /// duration measures play rather than how long people took to sit down.
    pub fn start(&mut self, now_secs: u64) {
        self.started_at_secs = now_secs;
        self.ended_at_secs = None;
    }

    /// Stops the clock, if it has not been stopped already. Idempotent: the
    /// victory broadcast can be reached more than once, and the second one
    /// must not stretch the recorded duration.
    pub fn finish(&mut self, now_secs: u64) {
        if self.ended_at_secs.is_none() {
            self.ended_at_secs = Some(now_secs);
        }
    }

    /// How long the game has run. Zero before it starts, and frozen once it
    /// has been won.
    pub fn duration_secs(&self, now_secs: u64) -> u64 {
        if self.started_at_secs == 0 {
            return 0;
        }
        self.ended_at_secs
            .unwrap_or(now_secs)
            .saturating_sub(self.started_at_secs)
    }

    pub fn table_rolls(&self) -> [u32; DICE_TOTALS] {
        self.rolls
    }

    pub fn resource_draws(&self) -> [u32; 5] {
        self.resource_draws
    }

    #[cfg(test)]
    pub fn draws_of_for_test(&self, resource: ResourceType) -> u32 {
        resource.card_index().map_or(0, |i| self.resource_draws[i])
    }

    /// Records cards coming out of the bank.
    ///
    /// Only the bank deals cards, so this is what "drawn" means: production,
    /// the starting hands, Year of Plenty, and the bank's side of a harbour
    /// trade. A card moving between two players was drawn once already and
    /// must not be counted again.
    pub fn drew(&mut self, resource: ResourceType, amount: u32) {
        let Some(index) = resource.card_index() else {
            // The desert produces nothing, so nothing can be drawn from it.
            return;
        };
        self.resource_draws[index] += amount;
    }

    /// This player's counters, creating an empty set on first use.
    fn tally(&mut self, pid: Uuid) -> &mut PlayerTally {
        self.players.entry(pid).or_default()
    }

    /// This player's counters as they stand, or an empty set for a player who
    /// has done nothing worth counting.
    pub fn player(&self, pid: Uuid) -> PlayerTally {
        self.players.get(&pid).cloned().unwrap_or_default()
    }

    pub fn record_roll(&mut self, pid: Uuid, total: u8) {
        let Some(index) = shared::dice_index(total) else {
            log::warn!("Ignoring impossible dice total {total} for statistics");
            return;
        };
        self.rolls[index] += 1;
        self.tally(pid).rolls[index] += 1;
    }

    pub fn gained(&mut self, pid: Uuid, source: Gain, amount: u32) {
        if amount == 0 {
            return;
        }
        let flow = &mut self.tally(pid).resources;
        flow.gained_total += amount;
        match source {
            Gain::Roll => flow.gained_by_rolling += amount,
            Gain::Trade => flow.gained_by_trading += amount,
            Gain::DevCard => flow.gained_by_dev_cards += amount,
            Gain::Robbery => flow.gained_by_robbing += amount,
            // The starting hand belongs in the total but has no column of its
            // own; it is not a move anybody made.
            Gain::Setup => {}
        }
    }

    pub fn lost(&mut self, pid: Uuid, source: Loss, amount: u32) {
        if amount == 0 {
            return;
        }
        let tally = self.tally(pid);
        let flow = &mut tally.resources;
        flow.lost_total += amount;
        match source {
            Loss::Trade => flow.lost_by_trading += amount,
            Loss::DevCard => flow.lost_by_dev_cards += amount,
            Loss::Robber => flow.lost_by_robber += amount,
            Loss::Seven => flow.lost_by_seven += amount,
            Loss::Spending => {
                flow.spent += amount;
                tally.activity.resources_spent += amount;
            }
        }
    }

    pub fn blocked_by_robber(&mut self, pid: Uuid, amount: u32) {
        if amount == 0 {
            return;
        }
        self.tally(pid).activity.resources_blocked_by_robber += amount;
    }

    pub fn dev_card_bought(&mut self, pid: Uuid, card: &DevCardType) {
        let tally = self.tally(pid);
        tally.dev_cards_bought.add(card, 1);
        tally.activity.dev_cards_bought += 1;
    }

    pub fn dev_card_played(&mut self, pid: Uuid, card: &DevCardType) {
        let tally = self.tally(pid);
        tally.dev_cards_played.add(card, 1);
        tally.activity.dev_cards_played += 1;
    }

    pub fn trade_proposed(&mut self, pid: Uuid) {
        self.tally(pid).activity.trades_proposed += 1;
    }

    pub fn trade_completed(&mut self, pid: Uuid) {
        self.tally(pid).activity.trades_completed += 1;
    }

    pub fn settlement_built(&mut self, pid: Uuid) {
        self.tally(pid).settlements_built += 1;
    }

    pub fn city_built(&mut self, pid: Uuid) {
        self.tally(pid).cities_built += 1;
    }

    pub fn road_built(&mut self, pid: Uuid) {
        self.tally(pid).roads_built += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn every_gain_lands_in_the_total_and_its_own_bucket() {
        let mut stats = Statistics::default();
        let me = pid(1);

        stats.gained(me, Gain::Roll, 3);
        stats.gained(me, Gain::Trade, 2);
        stats.gained(me, Gain::DevCard, 2);
        stats.gained(me, Gain::Robbery, 1);
        stats.gained(me, Gain::Setup, 4);

        let flow = stats.player(me).resources;
        assert_eq!(flow.gained_total, 12);
        assert_eq!(flow.gained_by_rolling, 3);
        assert_eq!(flow.gained_by_trading, 2);
        assert_eq!(flow.gained_by_dev_cards, 2);
        assert_eq!(flow.gained_by_robbing, 1);
    }

    /// The whole point of the score column: what came in, less what went out,
    /// is what is still in the hand.
    #[test]
    fn the_score_is_gains_less_losses() {
        let mut stats = Statistics::default();
        let me = pid(1);

        stats.gained(me, Gain::Roll, 10);
        stats.lost(me, Loss::Spending, 4);
        stats.lost(me, Loss::Seven, 3);

        let flow = stats.player(me).resources;
        assert_eq!(flow.lost_total, 7);
        assert_eq!(flow.spent, 4, "spending is a loss with no column of its own");
        assert_eq!(flow.lost_by_seven, 3);
        assert_eq!(flow.score(), 3);
    }

    /// Spending is the one loss that also shows up on the activity page.
    #[test]
    fn spending_is_counted_once_as_activity() {
        let mut stats = Statistics::default();
        let me = pid(1);

        stats.lost(me, Loss::Spending, 5);
        stats.lost(me, Loss::Robber, 1);

        let tally = stats.player(me);
        assert_eq!(tally.activity.resources_spent, 5);
        assert_eq!(tally.resources.lost_total, 6);
    }

    #[test]
    fn rolls_are_tallied_at_the_table_and_per_player() {
        let mut stats = Statistics::default();
        let (a, b) = (pid(1), pid(2));

        stats.record_roll(a, 7);
        stats.record_roll(a, 7);
        stats.record_roll(b, 2);
        stats.record_roll(b, 12);

        let table = stats.table_rolls();
        assert_eq!(table[shared::dice_index(7).unwrap()], 2);
        assert_eq!(table[shared::dice_index(2).unwrap()], 1);
        assert_eq!(table[shared::dice_index(12).unwrap()], 1);
        assert_eq!(table.iter().sum::<u32>(), 4);

        assert_eq!(stats.player(a).rolls.iter().sum::<u32>(), 2);
        assert_eq!(stats.player(b).rolls.iter().sum::<u32>(), 2);
    }

    /// A total two dice cannot show must not index off the end of the tally.
    #[test]
    fn an_impossible_total_is_ignored_rather_than_panicking() {
        let mut stats = Statistics::default();
        stats.record_roll(pid(1), 13);
        stats.record_roll(pid(1), 0);
        assert_eq!(stats.table_rolls().iter().sum::<u32>(), 0);
    }

    #[test]
    fn the_clock_freezes_when_the_game_is_won() {
        let mut stats = Statistics::default();
        stats.start(1_000);
        assert_eq!(stats.duration_secs(1_060), 60);

        stats.finish(1_100);
        assert_eq!(stats.duration_secs(9_999), 100, "the clock stops at the win");

        stats.finish(1_500);
        assert_eq!(stats.duration_secs(9_999), 100, "and does not restart");
    }

    /// A game restored from a save predates the clock starting; it must not
    /// report a duration measured from the Unix epoch.
    #[test]
    fn a_game_that_never_started_has_no_duration() {
        let stats = Statistics::default();
        assert_eq!(stats.duration_secs(1_700_000_000), 0);
    }

    #[test]
    fn a_player_who_did_nothing_reads_as_empty_rather_than_missing() {
        let stats = Statistics::default();
        let tally = stats.player(pid(9));
        assert_eq!(tally.resources.gained_total, 0);
        assert_eq!(tally.dev_cards_bought.total(), 0);
    }

    #[test]
    fn development_cards_are_split_by_kind() {
        let mut stats = Statistics::default();
        let me = pid(1);

        stats.dev_card_bought(me, &DevCardType::Knight);
        stats.dev_card_bought(me, &DevCardType::Knight);
        stats.dev_card_bought(me, &DevCardType::VictoryPoint);
        stats.dev_card_played(me, &DevCardType::Knight);

        let tally = stats.player(me);
        assert_eq!(tally.dev_cards_bought.knight, 2);
        assert_eq!(tally.dev_cards_bought.victory_point, 1);
        assert_eq!(tally.dev_cards_bought.total(), 3);
        assert_eq!(tally.dev_cards_played.knight, 1);
        assert_eq!(tally.activity.dev_cards_bought, 3);
        assert_eq!(tally.activity.dev_cards_played, 1);
    }
}
