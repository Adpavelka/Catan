use uuid::Uuid;
use shared::{GamePhase as ServerPhase, ServerMessage, ResourceType};
use shared::ResourceType::{Brick, Ore, Sheep, Wheat, Wood};
use std::collections::HashSet;
use crate::game::entities::pending_trade::PendingTrade;
use crate::game::entities::game_instance::GameInstance;


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

        let tm = &mut self.turn_manager;
        let player = tm
            .players
            .get(pid)
            .ok_or("Player not found")?;

        let possible_ratios = [2, 3, 4];

        for ratio in possible_ratios {
            let mut gives = crate::game::entities::resources::ResourceSet::new();
            gives.add(give, ratio);

            let mut takes = crate::game::entities::resources::ResourceSet::new();
            takes.add(receive, 1);

            if tm.bank.validate_bank_trade(player, &gives, &takes).is_ok() {
                tm.bank
                    .trade_with_bank(pid, gives, takes, &mut tm.players, true)
                    .map_err(|e| e.to_string())?;

                return Ok(ServerMessage::BankTradeCompleted {
                    player_id: pid,
                    gave: give,
                    received: receive,
                });
            }
        }

        Err("Invalid bank trade".to_string())
    }


    pub fn handle_trade_offer(&mut self, pid: Uuid,
        target_player_id: Option<Uuid>,
        offer: shared::Resources,
        request: shared::Resources,
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
        if is_empty(&offer) || is_empty(&request) {
            return Err("A trade needs something on both sides.".to_string());
        }

        if let Some(target) = target_player_id {
            if target == pid {
                return Err("You cannot trade with yourself.".to_string());
            }
            if self.turn_manager.players.get(target).is_none() {
                return Err("That player is not in this game.".to_string());
            }
        }

        // One open offer each, so a client cannot flood the table.
        if self.pending_trades.values().any(|t| t.proposer_id == pid) {
            return Err("You already have an offer on the table.".to_string());
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
            created_at_secs: crate::game::entities::game_instance::now_secs(),
        };

        self.pending_trades.insert(offer_id, pending_trade);

        Ok(ServerMessage::TradeProposed {
            offer_id,
            proposer_id: pid,
            target_player_id,
            offering: offer,
            requesting: request,
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
                self.pending_trades.remove(&offer_id);
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

        if trade.proposer_id != pid {
            return Err("Only the player who offered the trade can settle it.".to_string());
        }

        // Offers are cleared when a turn ends, but a 5-6 player table goes
        // into a special building phase first. Without this guard the player
        // who just finished could settle while somebody else is building.
        if self.get_state() != ServerPhase::RegularPlay
            || pid != self.turn_manager.players.get_current_player().id
        {
            return Err("You can only trade on your own turn.".to_string());
        }

        if !trade.has_accepted(partner_id) {
            return Err("That player has not accepted your offer.".to_string());
        }

        let proposer_gives = resources_to_set(&trade.offering);
        let partner_gives = resources_to_set(&trade.requesting);

        let proposer = self
            .turn_manager
            .players
            .get(pid)
            .ok_or("Proposer not found")?;

        // The offer is dead either way if the proposer cannot cover it, so
        // withdraw it rather than leaving them stuck with an offer they can
        // neither settle nor see the state of.
        if !proposer.can_pay(&proposer_gives) {
            self.pending_trades.remove(&offer_id);
            return Ok(ServerMessage::TradeCancelled { offer_id });
        }

        let partner = self
            .turn_manager
            .players
            .get(partner_id)
            .ok_or("That player is no longer in the game.")?;

        // Only this bid is dead; other players may still be able to settle.
        if !partner.can_pay(&partner_gives) {
            self.pending_trades
                .get_mut(&offer_id)
                .expect("checked above")
                .decline(partner_id);

            return Ok(ServerMessage::TradeAcceptanceWithdrawn {
                offer_id,
                accepter_id: partner_id,
            });
        }

        let tm = &mut self.turn_manager;

        tm.bank
            .collect_from_player_to_player(pid, partner_id, &proposer_gives, &mut tm.players)
            .map_err(|e| e.to_string())?;

        tm.bank
            .collect_from_player_to_player(partner_id, pid, &partner_gives, &mut tm.players)
            .map_err(|e| e.to_string())?;

        self.pending_trades.remove(&offer_id);

        Ok(ServerMessage::TradeCompleted {
            offer_id,
            proposer_id: pid,
            accepter_id: partner_id,
            proposer_gave: trade.offering,
            accepter_gave: trade.requesting,
        })
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
                    self.pending_trades.remove(&offer_id);
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
        match game.handle_trade_offer(proposer, None, res(1, 0), res(0, 1)).unwrap() {
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

    #[test]
    fn only_the_proposer_may_settle() {
        let (mut game, proposer, first, second) = table();
        let offer_id = offer_brick_for_lumber(&mut game, proposer);
        game.handle_trade_response(first, offer_id, true).unwrap();

        let err = game.handle_confirm_trade(second, offer_id, first).unwrap_err();
        assert!(err.contains("Only the player who offered"), "got: {err}");
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
    fn a_player_may_only_have_one_offer_open() {
        let (mut game, proposer, _, _) = table();
        offer_brick_for_lumber(&mut game, proposer);

        let err = game.handle_trade_offer(proposer, None, res(1, 0), res(0, 1)).unwrap_err();
        assert!(err.contains("already have an offer"), "got: {err}");
    }

    #[test]
    fn an_offer_must_have_something_on_both_sides() {
        let (mut game, proposer, _, _) = table();

        let err = game
            .handle_trade_offer(proposer, None, res(0, 0), res(0, 1))
            .unwrap_err();
        assert!(err.contains("something on both sides"), "got: {err}");
    }

    #[test]
    fn an_offer_cannot_target_a_stranger() {
        let (mut game, proposer, _, _) = table();

        let err = game
            .handle_trade_offer(proposer, Some(Uuid::from_u128(99)), res(1, 0), res(0, 1))
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

        game.handle_trade_offer(proposer, None, res(1, 0), res(0, 1))
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
}
