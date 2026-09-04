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
    pub declined_by: HashSet<Uuid>,
    /// When the offer was made, in seconds since the epoch.
    pub created_at_secs: u64,
}
