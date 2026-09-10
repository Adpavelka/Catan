use uuid::Uuid;
use shared::{GamePhase as ServerPhase, ServerMessage, ResourceType};
use shared::ResourceType::{Brick, Ore, Sheep, Wheat, Wood};
use std::collections::HashSet;
use crate::game::entities::pending_trade::PendingTrade;
use crate::game::entities::game_instance::GameInstance;
use crate::game::entities::statistics::{Gain, Loss};


fn is_empty(res: &shared::Resources) -> bool {
    res.brick == 0 && res.lumber == 0 && res.wool == 0 && res.grain == 0 && res.ore == 0
}


fn resources_to_set(res: &shared::Resources) -> crate::game::entities::resources::ResourceSet {
    use crate::game::entities::resources::ResourceSet;

    let mut set = ResourceSet::new();
    set.add(Brick, res.brick as u32);
    set.add(Wood, res.lumber as u32);
    set.add(Sheep, res.wool as u32);
    set.add(Wheat, res.grain as u32);
    set.add(Ore, res.ore as u32);
    set
}


/// How many offers one player may have open at the same time.
const MAX_OPEN_OFFERS_PER_PLAYER: usize = 4;

impl GameInstance {
    pub fn handle_bank_trade(&mut self, pid: Uuid,
        give: ResourceType,
        receive: ResourceType,
    ) -> Result<ServerMessage, String> {
        {
            if pid != self.turn_manager.players.get_current_player().id {
                return Err("Wait for your turn!".to_string());
            }

            if self.get_state() != ServerPhase::RegularPlay {
                return Err("You can only trade on your own turn.".to_string());
            }
        }

        // The best rate this player can get, found by trying the harbour
        // ratios in order. Kept rather than discarded: the statistics have to
        // know how many cards actually left the hand, and 2:1 and 4:1 are the
        // same trade from the outside.
        let ratio = {
            let tm = &mut self.turn_manager;
            let player = tm
                .players
                .get(pid)
                .ok_or("Player not found")?;

            let terms = [2u32, 3, 4].into_iter().find_map(|ratio| {
                let mut gives = crate::game::entities::resources::ResourceSet::new();
                gives.add(give, ratio);

                let mut takes = crate::game::entities::resources::ResourceSet::new();
                takes.add(receive, 1);

                tm.bank
                    .validate_bank_trade(player, &gives, &takes)
                    .is_ok()
                    .then_some((ratio, gives, takes))
            });

            let Some((ratio, gives, takes)) = terms else {
                return Err("Invalid bank trade".to_string());
            };

            tm.bank
                .trade_with_bank(pid, gives, takes, &mut tm.players, true)
                .map_err(|e| e.to_string())?;

            ratio
        };

        self.stats.lost(pid, Loss::Trade, ratio);
        self.stats.gained(pid, Gain::Trade, 1);
        self.stats.trade_completed(pid);
        // The card taken over the counter comes off the bank, so it is drawn.
        // The cards handed over go back into the pile and are not.
        self.stats.drew(receive, 1);

        Ok(ServerMessage::BankTradeCompleted {
            player_id: pid,
            gave: give,
            received: receive,
        })
    }


    pub fn handle_trade_offer(&mut self, pid: Uuid,
        target_player_id: Option<Uuid>,
        offer: shared::Resources,
        request: shared::Resources,
        offer_any: u8,
        request_any: u8,
    ) -> Result<ServerMessage, String> {
        self.expire_stale_trades();

        {
            if pid != self.turn_manager.players.get_current_player().id {
                return Err("Wait for your turn!".to_string());
            }

            if self.get_state() != ServerPhase::RegularPlay {
                return Err("You can only trade on your own turn.".to_string());
            }
        }

        // A trade has to be a trade. Without this an empty offer creates a
        // no-op that still has to be answered by everyone.
        if (is_empty(&offer) && offer_any == 0) || (is_empty(&request) && request_any == 0) {
            return Err("A trade needs something on both sides.".to_string());
        }

        // A wildcard is a placeholder for a card nobody has named yet. Letting
        // one sit on both sides would be an offer with no content at all.
        if offer_any > 0 && request_any > 0 {
            return Err("At least one side has to name its cards.".to_string());
        }

        if let Some(target) = target_player_id {
            if target == pid {
                return Err("You cannot trade with yourself.".to_string());
            }
            if self.turn_manager.players.get(target).is_none() {
                return Err("That player is not in this game.".to_string());
            }
        }

        // Several offers at once are fine - putting two deals to the table and
        // taking whichever lands is ordinary play - but not unlimited, or a
        // client could bury everyone else under offers they each have to
        // answer. Whether you can actually pay is re-checked when an offer is
        // settled, so overlapping offers that spend the same card simply fail
        // at the point the second one is taken.
        let mine = self
            .pending_trades
            .values()
            .filter(|t| t.proposer_id == pid)
            .count();
        if mine >= MAX_OPEN_OFFERS_PER_PLAYER {
            return Err(format!(
                "You already have {MAX_OPEN_OFFERS_PER_PLAYER} offers on the table."
            ));
        }

        let tm = &mut self.turn_manager;

        let proposer = tm
            .players
            .get(pid)
            .ok_or("Player not found")?;

        let offer_set = resources_to_set(&offer);

        if !proposer.can_pay(&offer_set) {
            return Err("You don't have enough resources for this trade".to_string());
        }

        let offer_id = self.next_trade_id;
        self.next_trade_id += 1;

        let pending_trade = PendingTrade {
            offer_id,
            proposer_id: pid,
            target_player_id,
            offering: offer.clone(),
            requesting: request.clone(),
            declined_by: HashSet::new(),
            accepted_by: Vec::new(),
            offering_any: offer_any,
            requesting_any: request_any,
            created_at_secs: crate::game::entities::game_instance::now_secs(),
            counters: None,
        };

        self.pending_trades.insert(offer_id, pending_trade);
        self.stats.trade_proposed(pid);

        Ok(ServerMessage::TradeProposed {
            offer_id,
            proposer_id: pid,
            target_player_id,
            offering: offer,
            requesting: request,
            offering_any: offer_any,
            requesting_any: request_any,
            counters: None,
        })
    }


    /// Answers the active player's offer with different terms. The counter is
    /// a trade from `pid` to them, so it is still their turn's trade - two
    /// players who are both waiting can never deal with each other.
    pub fn handle_counter_offer(&mut self, pid: Uuid,
        offer_id: u64,
        offer: shared::Resources,
        request: shared::Resources,
    ) -> Result<ServerMessage, String> {
        self.expire_stale_trades();

        if self.get_state() != ServerPhase::RegularPlay {
            return Err("You can only trade during a turn.".to_string());
        }

        let Some(original) = self.pending_trades.get(&offer_id) else {
            return Ok(ServerMessage::TradeCancelled { offer_id });
        };

        if original.is_counter() {
            return Err("You cannot counter a counter-offer.".to_string());
        }

        let active = self.turn_manager.players.get_current_player().id;
        if original.proposer_id != active {
            return Err("That offer is no longer on the table.".to_string());
        }

        if pid == active {
            return Err("You cannot counter your own offer.".to_string());
        }

        if !original.is_open_to(pid) {
            return Err("This trade was not offered to you".to_string());
        }

        // A counter names its cards; that is the point of countering.
        if is_empty(&offer) || is_empty(&request) {
            return Err("A trade needs something on both sides.".to_string());
        }

        let counterer = self
            .turn_manager
            .players
            .get(pid)
            .ok_or("Player not found")?;

        if !counterer.can_pay(&resources_to_set(&offer)) {
            return Err("You don't have enough resources for this trade".to_string());
        }

        // Countering supersedes whatever you said before: your earlier terms
        // are gone, and you are no longer bidding on theirs.
        self.pending_trades
            .retain(|_, trade| !(trade.proposer_id == pid && trade.counters == Some(offer_id)));
        if let Some(original) = self.pending_trades.get_mut(&offer_id) {
            original.accepted_by.retain(|id| *id != pid);
        }

        let counter_id = self.next_trade_id;
        self.next_trade_id += 1;

        self.pending_trades.insert(counter_id, PendingTrade {
            offer_id: counter_id,
            proposer_id: pid,
            target_player_id: Some(active),
            offering: offer.clone(),
            requesting: request.clone(),
            declined_by: HashSet::new(),
            accepted_by: Vec::new(),
            offering_any: 0,
            requesting_any: 0,
            created_at_secs: crate::game::entities::game_instance::now_secs(),
            counters: Some(offer_id),
        });

        // A counter is terms put to the table, so it counts as an offer made.
        self.stats.trade_proposed(pid);

        Ok(ServerMessage::TradeProposed {
            offer_id: counter_id,
            proposer_id: pid,
            target_player_id: Some(active),
            offering: offer,
            requesting: request,
            offering_any: 0,
            requesting_any: 0,
            counters: Some(offer_id),
        })
    }


    pub fn handle_trade_response(&mut self, pid: Uuid,
        offer_id: u64,
        accept: bool,
    ) -> Result<ServerMessage, String> {
        self.expire_stale_trades();

        let Some(trade) = self.pending_trades.get(&offer_id) else {
            return Ok(ServerMessage::TradeCancelled { offer_id });
        };

        let proposer_id = trade.proposer_id;

        if proposer_id == pid {
            return Err("You cannot respond to your own trade".to_string());
        }

        if !trade.is_open_to(pid) {
            return Err("This trade was not offered to you".to_string());
        }

        // A counter runs between exactly two players and is settled by the
        // active one, so there is nothing here to bid on.
        if trade.is_counter() {
            return Err("Answer a counter-offer by accepting or rejecting it.".to_string());
        }

        if !accept {
            let already_declined = trade.declined_by.contains(&pid);
            let had_accepted = self
                .pending_trades
                .get_mut(&offer_id)
                .expect("checked above")
                .decline(pid);

            if !had_accepted && already_declined {
                return Err("You already declined this trade".to_string());
            }

            // Has to run for a withdrawal too. Taking back the last acceptance
            // can be what leaves nobody willing, and returning early here left
            // the offer stranded: unanswerable, and blocking a replacement.
            if self.everyone_has_declined(offer_id) {
                self.close_offer_and_counters(offer_id);
            }

            // Taking back an acceptance is worth telling the proposer about,
            // since it changes who they can settle with.
            if had_accepted {
                return Ok(ServerMessage::TradeAcceptanceWithdrawn {
                    offer_id,
                    accepter_id: pid,
                });
            }

            return Ok(ServerMessage::TradeDeclined {
                offer_id,
                proposer_id,
                decliner_id: pid,
            });
        }

        if trade.has_wildcards() {
            return Err(
                "This offer has an unspecified card - counter with what you would give."
                    .to_string(),
            );
        }

        if trade.has_accepted(pid) {
            return Err("You have already accepted this trade".to_string());
        }

        // Check the accepter can actually cover it now. It is checked again at
        // settlement, but failing here keeps unaffordable bids off the
        // proposer's list.
        let accepter_gives = resources_to_set(&trade.requesting);
        let accepter = self
            .turn_manager
            .players
            .get(pid)
            .ok_or("Accepter not found")?;

        if !accepter.can_pay(&accepter_gives) {
            return Err("You don't have enough resources for this trade".to_string());
        }

        self.pending_trades
            .get_mut(&offer_id)
            .expect("checked above")
            .accept(pid);

        Ok(ServerMessage::TradeAccepted {
            offer_id,
            accepter_id: pid,
        })
    }


    /// Settles an offer with one of the players who accepted it. Only the
    /// proposer may call this, which is what makes acceptance a bid rather
    /// than a completed trade.
    pub fn handle_confirm_trade(&mut self, pid: Uuid, offer_id: u64, partner_id: Uuid)
        -> Result<ServerMessage, String>
    {
        self.expire_stale_trades();

        let Some(trade) = self.pending_trades.get(&offer_id).cloned() else {
            return Ok(ServerMessage::TradeCancelled { offer_id });
        };

        // Offers are cleared when a turn ends, but a 5-6 player table goes
        // into a special building phase first. Without this guard the player
        // who just finished could settle while somebody else is building.
        if self.get_state() != ServerPhase::RegularPlay
            || pid != self.turn_manager.players.get_current_player().id
        {
            return Err("You can only trade on your own turn.".to_string());
        }

        // Only the active player settles, either way round. For their own
        // offer they pick an accepter; for a counter aimed at them they pick
        // the player who countered. Both leave the trade between the active
        // player and one other, never between two players who are waiting.
        if trade.has_wildcards() {
            return Err("Wait for a counter that names the unspecified card.".to_string());
        }

        if trade.proposer_id == pid {
            if !trade.has_accepted(partner_id) {
                return Err("That player has not accepted your offer.".to_string());
            }
        } else if trade.is_counter() && trade.target_player_id == Some(pid) {
            if partner_id != trade.proposer_id {
                return Err("That player did not make this counter-offer.".to_string());
            }
        } else {
            return Err("Only the player whose turn it is can settle a trade.".to_string());
        }

        // `offering` always flows from the trade's proposer to the other side,
        // whichever of the two is the active player.
        let giver = trade.proposer_id;
        let taker = if giver == pid { partner_id } else { pid };

        let proposer_gives = resources_to_set(&trade.offering);
        let partner_gives = resources_to_set(&trade.requesting);

        let proposer = self
            .turn_manager
            .players
            .get(giver)
            .ok_or("That player is no longer in the game.")?;

        // The offer is dead either way if its proposer cannot cover it, so
        // withdraw it rather than leaving it as something that can never be
        // settled and can never be seen to have failed.
        if !proposer.can_pay(&proposer_gives) {
            self.pending_trades.remove(&offer_id);
            return Ok(ServerMessage::TradeCancelled { offer_id });
        }

        let other = self
            .turn_manager
            .players
            .get(taker)
            .ok_or("That player is no longer in the game.")?;

        if !other.can_pay(&partner_gives) {
            // A counter has only one possible partner, so it dies with them;
            // an open offer may still have other bids worth keeping.
            if trade.is_counter() {
                self.pending_trades.remove(&offer_id);
                return Ok(ServerMessage::TradeCancelled { offer_id });
            }

            self.pending_trades
                .get_mut(&offer_id)
                .expect("checked above")
                .decline(taker);

            return Ok(ServerMessage::TradeAcceptanceWithdrawn {
                offer_id,
                accepter_id: taker,
            });
        }

        let tm = &mut self.turn_manager;

        tm.bank
            .collect_from_player_to_player(giver, taker, &proposer_gives, &mut tm.players)
            .map_err(|e| e.to_string())?;

        tm.bank
            .collect_from_player_to_player(taker, giver, &partner_gives, &mut tm.players)
            .map_err(|e| e.to_string())?;

        // What each side handed over. Both directions are counted for both
        // players: a trade is a gain and a loss at once, and a page that only
        // tallied one of them would make every trader look richer than they
        // finished.
        let given = proposer_gives.get_cards_total();
        let taken = partner_gives.get_cards_total();

        self.stats.lost(giver, Loss::Trade, given);
        self.stats.gained(giver, Gain::Trade, taken);
        self.stats.lost(taker, Loss::Trade, taken);
        self.stats.gained(taker, Gain::Trade, given);
        self.stats.trade_completed(giver);
        self.stats.trade_completed(taker);

        // Settling ends the whole negotiation, including any counters that
        // were answering the same offer.
        self.close_offer_and_counters(offer_id);
        if let Some(original) = trade.counters {
            self.close_offer_and_counters(original);
        }

        Ok(ServerMessage::TradeCompleted {
            offer_id,
            proposer_id: giver,
            accepter_id: taker,
            proposer_gave: trade.offering,
            accepter_gave: trade.requesting,
        })
    }


    /// Removes an offer along with every counter that was answering it.
    /// Returns the ids that went, for the caller to announce.
    pub(crate) fn close_offer_and_counters(&mut self, offer_id: u64) -> Vec<u64> {
        let mut closed = Vec::new();
        self.pending_trades.retain(|id, trade| {
            let doomed = *id == offer_id || trade.counters == Some(offer_id);
            if doomed {
                closed.push(*id);
            }
            !doomed
        });
        closed
    }


    /// Whether every player who could still answer this offer has declined it.
    /// Counts only players actually seated, so someone leaving mid-offer
    /// cannot keep it alive forever.
    fn everyone_has_declined(&self, offer_id: u64) -> bool {
        let Some(trade) = self.pending_trades.get(&offer_id) else {
            return false;
        };

        let eligible: Vec<Uuid> = (0..self.turn_manager.players.len())
            .filter_map(|idx| self.turn_manager.players.get_by_index(idx))
            .map(|player| player.id)
            .filter(|id| trade.is_open_to(*id))
            .collect();

        !eligible.is_empty() && eligible.iter().all(|id| trade.declined_by.contains(id))
    }


    pub fn handle_cancel_trade(&mut self, pid: Uuid, offer_id: u64) -> Result<ServerMessage, String> {
        let trade = self.pending_trades.get(&offer_id);
        match trade {
            None => Err("Trade not found".to_string()),
            Some(trade) => {
                if trade.proposer_id != pid {
                    Err("You can only cancel your own trades".to_string())
                } else {
                    self.close_offer_and_counters(offer_id);
                    Ok(ServerMessage::TradeCancelled { offer_id })
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entities::resources::ResourceType;
    use shared::Resources;

    fn res(brick: u8, lumber: u8) -> Resources {
        Resources { brick, lumber, ..Default::default() }
    }

    /// Three players in regular play. The proposer holds brick, the other two
    /// hold lumber, so both can answer an offer of brick for lumber.
    fn table() -> (GameInstance, Uuid, Uuid, Uuid) {
        let ids: Vec<Uuid> = (1..=3u128).map(Uuid::from_u128).collect();
        let mut game =
            GameInstance::new("t".into(), ids[0], 3, "p1", shared::PlayerColour::Blue);

        for (i, colour) in shared::PlayerColour::ALL.iter().skip(1).take(2).enumerate() {
            game.turn_manager
                .players
                .seat(ids[i + 1], &format!("p{}", i + 2), *colour)
                .unwrap();
        }

        game.start_game();
        game.advance_phase();
        game.advance_phase(); // RegularPlay

        game.turn_manager.players.get_mut(ids[0]).unwrap()
            .resources.add(ResourceType::Brick, 5);
        for id in &ids[1..] {
            game.turn_manager.players.get_mut(*id).unwrap()
                .resources.add(ResourceType::Wood, 5);
        }

        (game, ids[0], ids[1], ids[2])
    }

    fn offer_brick_for_lumber(game: &mut GameInstance, proposer: Uuid) -> u64 {
        match game.handle_trade_offer(proposer, None, res(1, 0), res(0, 1), 0, 0).unwrap() {
            ServerMessage::TradeProposed { offer_id, .. } => offer_id,
            other => panic!("expected TradeProposed, got {other:?}"),
        }
    }

    /// The heart of the change: accepting is a bid. Nothing moves until the
    /// proposer picks somebody.
    #[test]
    fn accepting_does_not_move_resources() {
        let (mut game, proposer, taker, _) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        let msg = game.handle_trade_response(taker, offer_id, true).unwrap();
        assert!(matches!(msg, ServerMessage::TradeAccepted { .. }));

        let brick = |g: &GameInstance, id| {
            g.turn_manager.players.get(id).unwrap().resources.amount_of(ResourceType::Brick)
        };
        assert_eq!(brick(&game, proposer), 5, "proposer still holds their brick");
        assert_eq!(brick(&game, taker), 0, "the taker has not been paid yet");
        assert!(game.pending_trades.contains_key(&offer_id), "offer stays open");
    }

    #[test]
    fn the_proposer_chooses_between_several_accepters() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        game.handle_trade_response(first, offer_id, true).unwrap();
        game.handle_trade_response(second, offer_id, true).unwrap();

        assert_eq!(
            game.pending_trades[&offer_id].accepted_by,
            vec![first, second],
            "both bids are on the table, in the order they arrived"
        );

        // Settle with the second one, not just whoever answered first.
        let msg = game.handle_confirm_trade(proposer, offer_id, second).unwrap();
        match msg {
            ServerMessage::TradeCompleted { accepter_id, .. } => assert_eq!(accepter_id, second),
            other => panic!("expected TradeCompleted, got {other:?}"),
        }

        let amount = |g: &GameInstance, id, r| {
            g.turn_manager.players.get(id).unwrap().resources.amount_of(r)
        };
        assert_eq!(amount(&game, second, ResourceType::Brick), 1);
        assert_eq!(amount(&game, proposer, ResourceType::Wood), 1);
        assert_eq!(amount(&game, first, ResourceType::Wood), 5, "the loser is untouched");
        assert!(!game.pending_trades.contains_key(&offer_id), "offer is settled");
    }

    /// Settling is the active player's alone. A bystander is turned away by
    /// the turn check before anything else is even considered.
    #[test]
    fn only_the_active_player_may_settle() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(first, offer_id, true).unwrap();

        let err = game.handle_confirm_trade(second, offer_id, first).unwrap_err();
        assert!(err.contains("own turn"), "got: {err}");
        assert!(game.pending_trades.contains_key(&offer_id));

        // Nor may the bidder settle the offer they bid on.
        let err = game.handle_confirm_trade(first, offer_id, first).unwrap_err();
        assert!(err.contains("own turn"), "got: {err}");
        assert!(game.pending_trades.contains_key(&offer_id));
    }

    #[test]
    fn cannot_settle_with_someone_who_never_accepted() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(first, offer_id, true).unwrap();

        let err = game.handle_confirm_trade(proposer, offer_id, second).unwrap_err();
        assert!(err.contains("has not accepted"), "got: {err}");
    }

    #[test]
    fn an_accepter_can_withdraw_before_being_picked() {
        let (mut game, proposer, taker, _) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        game.handle_trade_response(taker, offer_id, true).unwrap();
        let msg = game.handle_trade_response(taker, offer_id, false).unwrap();
        assert!(matches!(msg, ServerMessage::TradeAcceptanceWithdrawn { .. }));

        assert!(game.pending_trades[&offer_id].accepted_by.is_empty());
        let err = game.handle_confirm_trade(proposer, offer_id, taker).unwrap_err();
        assert!(err.contains("has not accepted"), "got: {err}");
    }

    #[test]
    fn an_offer_is_withdrawn_once_everyone_declines() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        game.handle_trade_response(first, offer_id, false).unwrap();
        assert!(game.pending_trades.contains_key(&offer_id), "one refusal is not enough");

        game.handle_trade_response(second, offer_id, false).unwrap();
        assert!(!game.pending_trades.contains_key(&offer_id), "nobody left to accept");
    }

    #[test]
    fn several_offers_may_be_open_at_once_up_to_the_cap() {
        let (mut game, proposer, _, _) = table();

        // Putting more than one deal to the table is ordinary play.
        for _ in 0..MAX_OPEN_OFFERS_PER_PLAYER {
            game.handle_trade_offer(proposer, None, res(1, 0), res(0, 1), 0, 0)
                .expect("within the cap");
        }
        assert_eq!(
            game.pending_trades
                .values()
                .filter(|t| t.proposer_id == proposer)
                .count(),
            MAX_OPEN_OFFERS_PER_PLAYER
        );

        // Past it, so one client cannot bury the table.
        let err = game
            .handle_trade_offer(proposer, None, res(1, 0), res(0, 1), 0, 0)
            .unwrap_err();
        assert!(err.contains("offers on the table"), "got: {err}");
    }

    #[test]
    fn overlapping_offers_that_spend_the_same_card_fail_at_settle_time() {
        let (mut game, proposer, first, second) = table();

        // Two offers that together want more brick than the proposer holds
        // (they have five). Both are legal to make; only one can be settled.
        let a = game.handle_trade_offer(proposer, None, res(3, 0), res(0, 1), 0, 0).unwrap();
        let b = game.handle_trade_offer(proposer, None, res(3, 0), res(0, 1), 0, 0).unwrap();
        let (a, b) = match (a, b) {
            (
                ServerMessage::TradeProposed { offer_id: a, .. },
                ServerMessage::TradeProposed { offer_id: b, .. },
            ) => (a, b),
            other => panic!("expected two proposals, got {other:?}"),
        };

        game.handle_trade_response(first, a, true).unwrap();
        game.handle_trade_response(second, b, true).unwrap();
        game.handle_confirm_trade(proposer, a, first).unwrap();

        // The brick is gone, so the second offer withdraws itself rather than
        // sitting there as something that can never be settled.
        let out = game.handle_confirm_trade(proposer, b, second).unwrap();
        assert!(
            matches!(out, ServerMessage::TradeCancelled { offer_id } if offer_id == b),
            "got: {out:?}"
        );
        assert!(!game.pending_trades.contains_key(&b));
    }

    #[test]
    fn an_offer_must_have_something_on_both_sides() {
        let (mut game, proposer, _, _) = table();

        let err = game
            .handle_trade_offer(proposer, None, res(0, 0), res(0, 1), 0, 0)
            .unwrap_err();
        assert!(err.contains("something on both sides"), "got: {err}");
    }

    #[test]
    fn an_offer_cannot_target_a_stranger() {
        let (mut game, proposer, _, _) = table();

        let err = game
            .handle_trade_offer(proposer, Some(Uuid::from_u128(99)), res(1, 0), res(0, 1), 0, 0)
            .unwrap_err();
        assert!(err.contains("not in this game"), "got: {err}");
    }

    /// A bid that the player can no longer cover drops out on its own rather
    /// than killing the whole offer.
    #[test]
    fn settling_with_a_broke_partner_drops_only_that_bid() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        game.handle_trade_response(first, offer_id, true).unwrap();
        game.handle_trade_response(second, offer_id, true).unwrap();

        game.turn_manager.players.get_mut(first).unwrap()
            .resources.take(ResourceType::Wood, 5);

        let msg = game.handle_confirm_trade(proposer, offer_id, first).unwrap();
        assert!(matches!(msg, ServerMessage::TradeAcceptanceWithdrawn { .. }));
        assert!(game.pending_trades.contains_key(&offer_id), "the offer survives");

        game.handle_confirm_trade(proposer, offer_id, second)
            .expect("the other bid is still good");
    }

    /// The proposer used to be stranded here: the offer vanished with only an
    /// error to the accepter, leaving their client showing it forever.
    #[test]
    fn a_proposer_who_cannot_pay_gets_their_offer_withdrawn() {
        let (mut game, proposer, taker, _) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(taker, offer_id, true).unwrap();

        game.turn_manager.players.get_mut(proposer).unwrap()
            .resources.take(ResourceType::Brick, 5);

        let msg = game.handle_confirm_trade(proposer, offer_id, taker).unwrap();
        assert!(
            matches!(msg, ServerMessage::TradeCancelled { .. }),
            "the table must be told, not just the accepter, got {msg:?}"
        );
        assert!(!game.pending_trades.contains_key(&offer_id));
    }

    #[test]
    fn offers_do_not_outlive_the_turn() {
        let (mut game, proposer, _, _) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        assert_eq!(game.clear_trades(), vec![offer_id]);
        assert!(game.pending_trades.is_empty());
    }

    #[test]
    fn expiry_reports_what_it_dropped() {
        let (mut game, proposer, _, _) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        game.pending_trades.get_mut(&offer_id).unwrap().created_at_secs = 0;

        assert_eq!(game.expire_stale_trades(), vec![offer_id]);
        assert!(game.pending_trades.is_empty());
    }

    /// Regression: an acceptance taken back used to return early, skipping the
    /// "everyone has declined" cleanup. The offer then stranded - nobody could
    /// answer it again, and the proposer could not replace it.
    #[test]
    fn withdrawing_the_last_acceptance_closes_the_offer() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);

        game.handle_trade_response(first, offer_id, false).unwrap();
        game.handle_trade_response(second, offer_id, true).unwrap();
        game.handle_trade_response(second, offer_id, false).unwrap();

        assert!(
            !game.pending_trades.contains_key(&offer_id),
            "every eligible player has now declined, so the offer must close"
        );

        game.handle_trade_offer(proposer, None, res(1, 0), res(0, 1), 0, 0)
            .expect("the proposer is free to offer again");
    }

    /// Regression: at 5-6 players ending a turn opens a special building phase
    /// and returns PhaseChanged, not NextTurn, so an offer could survive and
    /// be settled while somebody else was building.
    #[test]
    fn an_offer_cannot_be_settled_once_the_turn_is_over() {
        let (mut game, proposer, taker, _) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(taker, offer_id, true).unwrap();

        // Stand in for any phase that is not this player's regular turn.
        game.force_phase_for_test(ServerPhase::SpecialBuilding { builder: taker });

        let err = game.handle_confirm_trade(proposer, offer_id, taker).unwrap_err();
        assert!(err.contains("own turn"), "got: {err}");
    }

    /// A reconnect has to restore the negotiation, or the offer stays live on
    /// the server while both sides have lost sight of it.
    #[test]
    fn a_sync_carries_the_offers_a_player_can_see() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(first, offer_id, true).unwrap();

        let for_proposer = game.trade_snapshots_for(proposer);
        assert_eq!(for_proposer.len(), 1);
        assert_eq!(for_proposer[0].accepted_by, vec![first], "their bidder is restored");
        assert!(for_proposer[0].seconds_remaining > 0);

        let for_bidder = game.trade_snapshots_for(first);
        assert_eq!(for_bidder.len(), 1, "the bidder still sees the offer");
        assert!(for_bidder[0].accepted_by.contains(&first), "and that they bid on it");

        let for_other = game.trade_snapshots_for(second);
        assert_eq!(for_other.len(), 1, "an open offer is visible to everyone eligible");
        assert!(!for_other[0].you_declined);
    }

    /// `declined_by` is private: a sync tells you whether *you* refused, not
    /// who else did.
    #[test]
    fn a_sync_does_not_leak_who_else_declined() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(first, offer_id, false).unwrap();

        let for_decliner = game.trade_snapshots_for(first);
        assert_eq!(for_decliner.len(), 1);
        assert!(for_decliner[0].you_declined, "the decliner is told they refused");

        let for_other = game.trade_snapshots_for(second);
        assert!(!for_other[0].you_declined, "somebody else's refusal is not theirs");

        // The only per-player flag is `you_declined`; the set itself never
        // travels, so there is nothing else to leak.
        let for_proposer = game.trade_snapshots_for(proposer);
        assert!(!for_proposer[0].you_declined);
    }

    #[test]
    fn a_targeted_offer_is_only_visible_to_the_two_players_involved() {
        let (mut game, proposer, first, second) = table();
        game.handle_trade_offer(proposer, Some(first), res(1, 0), res(0, 1), 0, 0).unwrap();

        assert_eq!(game.trade_snapshots_for(proposer).len(), 1);
        assert_eq!(game.trade_snapshots_for(first).len(), 1);
        assert_eq!(
            game.trade_snapshots_for(second).len(),
            0,
            "a private offer must not show up for a bystander"
        );
    }

    fn counter(game: &mut GameInstance, pid: Uuid, original: u64,
               give: Resources, want: Resources) -> Result<u64, String> {
        match game.handle_counter_offer(pid, original, give, want)? {
            ServerMessage::TradeProposed { offer_id, counters, .. } => {
                assert_eq!(counters, Some(original), "a counter must point at its original");
                Ok(offer_id)
            }
            other => panic!("expected TradeProposed, got {other:?}"),
        }
    }

    /// The active player offers, somebody counters, the active player takes
    /// the counter. Resources move on the counter's terms, not the original's.
    #[test]
    fn a_counter_can_be_settled_by_the_active_player() {
        let (mut game, proposer, taker, _) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);

        // They want two brick for their one lumber, not one.
        let counter_id = counter(&mut game, taker, original, res(0, 1), res(2, 0)).unwrap();

        let msg = game.handle_confirm_trade(proposer, counter_id, taker).unwrap();
        match msg {
            ServerMessage::TradeCompleted { proposer_id, accepter_id, proposer_gave, accepter_gave, .. } => {
                assert_eq!(proposer_id, taker, "the counter's proposer is the counterer");
                assert_eq!(accepter_id, proposer);
                assert_eq!(proposer_gave, res(0, 1));
                assert_eq!(accepter_gave, res(2, 0));
            }
            other => panic!("expected TradeCompleted, got {other:?}"),
        }

        let amount = |g: &GameInstance, id, r| {
            g.turn_manager.players.get(id).unwrap().resources.amount_of(r)
        };
        assert_eq!(amount(&game, proposer, ResourceType::Brick), 3, "paid two brick");
        assert_eq!(amount(&game, proposer, ResourceType::Wood), 1);
        assert_eq!(amount(&game, taker, ResourceType::Brick), 2);

        assert!(game.pending_trades.is_empty(), "settling ends the negotiation");
    }

    /// The rule that matters: a counter is aimed at the active player, so two
    /// players who are both waiting can never trade with each other.
    #[test]
    fn a_waiting_player_cannot_settle_a_counter() {
        let (mut game, proposer, first, second) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);
        let counter_id = counter(&mut game, first, original, res(0, 1), res(1, 0)).unwrap();

        let err = game.handle_confirm_trade(second, counter_id, first).unwrap_err();
        assert!(err.contains("own turn"), "got: {err}");

        // ...and they cannot bid on it to sneak in either.
        let err = game.handle_trade_response(second, counter_id, true).unwrap_err();
        assert!(err.contains("not offered to you"), "got: {err}");
    }

    #[test]
    fn a_counter_is_not_something_to_bid_on() {
        let (mut game, proposer, taker, _) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);
        let counter_id = counter(&mut game, taker, original, res(0, 1), res(1, 0)).unwrap();

        let err = game.handle_trade_response(proposer, counter_id, true).unwrap_err();
        assert!(err.contains("accepting or rejecting"), "got: {err}");
    }

    #[test]
    fn countering_replaces_your_earlier_answer() {
        let (mut game, proposer, taker, _) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);

        game.handle_trade_response(taker, original, true).unwrap();
        assert_eq!(game.pending_trades[&original].accepted_by, vec![taker]);

        let first_counter = counter(&mut game, taker, original, res(0, 1), res(2, 0)).unwrap();
        assert!(
            game.pending_trades[&original].accepted_by.is_empty(),
            "countering withdraws the bid on their original terms"
        );

        // Changing your mind again replaces the counter rather than stacking.
        let second_counter = counter(&mut game, taker, original, res(0, 1), res(3, 0)).unwrap();
        assert!(!game.pending_trades.contains_key(&first_counter));
        assert!(game.pending_trades.contains_key(&second_counter));
    }

    #[test]
    fn counters_die_with_the_offer_they_answer() {
        let (mut game, proposer, first, second) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);
        counter(&mut game, first, original, res(0, 1), res(2, 0)).unwrap();
        counter(&mut game, second, original, res(0, 1), res(3, 0)).unwrap();
        assert_eq!(game.pending_trades.len(), 3);

        game.handle_cancel_trade(proposer, original).unwrap();
        assert!(game.pending_trades.is_empty(), "withdrawing the offer clears its counters");
    }

    #[test]
    fn you_cannot_counter_your_own_offer_or_a_counter() {
        let (mut game, proposer, taker, _) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);

        let err = game
            .handle_counter_offer(proposer, original, res(1, 0), res(0, 1))
            .unwrap_err();
        assert!(err.contains("your own offer"), "got: {err}");

        let counter_id = counter(&mut game, taker, original, res(0, 1), res(1, 0)).unwrap();
        let err = game
            .handle_counter_offer(proposer, counter_id, res(1, 0), res(0, 1))
            .unwrap_err();
        assert!(err.contains("counter a counter"), "got: {err}");
    }

    #[test]
    fn a_counter_you_cannot_afford_is_refused() {
        let (mut game, proposer, taker, _) = table();
        let original = offer_brick_for_lumber(&mut game, proposer);

        let err = game
            .handle_counter_offer(taker, original, res(0, 99), res(1, 0))
            .unwrap_err();
        assert!(err.contains("enough resources"), "got: {err}");
    }
}
