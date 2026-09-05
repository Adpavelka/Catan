use std::collections::HashSet;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use shared::Resources;

/// A pending trade offer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTrade {
    #[allow(dead_code)]
    pub offer_id: u64,
    pub proposer_id: Uuid,
    pub target_player_id: Option<Uuid>,  // If Some, only this player can accept
    pub offering: Resources,
    pub requesting: Resources,
    /// Unspecified cards on each side. "Any card" is a request to negotiate:
    /// the trade cannot be settled while one is outstanding, because nobody
    /// has said what it stands for. The answer is a counter that names it.
    #[serde(default)]
    pub offering_any: u8,
    #[serde(default)]
    pub requesting_any: u8,
    pub declined_by: HashSet<Uuid>,
    /// Players who said they would take this trade, oldest first. Accepting
    /// only puts you in this queue - the proposer chooses who to settle with,
    /// so the order they are shown in is the order they answered.
    #[serde(default)]
    pub accepted_by: Vec<Uuid>,
    /// When the offer was made, in seconds since the epoch.
    pub created_at_secs: u64,
    /// Set when this trade answers another player's offer with different
    /// terms. A counter always runs between the countering player and the
    /// active player, which is what stops two idle players trading.
    #[serde(default)]
    pub counters: Option<u64>,
}

impl PendingTrade {
    /// Records `pid` as willing to trade. Accepting twice is harmless, and
    /// accepting after declining replaces the decline.
    pub fn accept(&mut self, pid: Uuid) {
        self.declined_by.remove(&pid);
        if !self.accepted_by.contains(&pid) {
            self.accepted_by.push(pid);
        }
    }

    /// Records `pid` as unwilling, taking back any earlier acceptance.
    /// Returns whether they had previously accepted.
    pub fn decline(&mut self, pid: Uuid) -> bool {
        let had_accepted = self.accepted_by.contains(&pid);
        self.accepted_by.retain(|id| *id != pid);
        self.declined_by.insert(pid);
        had_accepted
    }

    /// Whether either side still has an unspecified card in it.
    pub fn has_wildcards(&self) -> bool {
        self.offering_any > 0 || self.requesting_any > 0
    }

    pub fn is_counter(&self) -> bool {
        self.counters.is_some()
    }

    pub fn has_accepted(&self, pid: Uuid) -> bool {
        self.accepted_by.contains(&pid)
    }

    /// Whether `pid` is allowed to answer this offer at all.
    pub fn is_open_to(&self, pid: Uuid) -> bool {
        pid != self.proposer_id
            && self.target_player_id.map_or(true, |target| target == pid)
    }
}
