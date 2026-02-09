use uuid::Uuid;
use shared::{ServerMessage, ResourceType};
use shared::ResourceType::{Brick, Ore, Sheep, Wheat, Wood};
use std::collections::HashSet;
use crate::game::entities::pending_trade::PendingTrade;
use crate::lobby::GameInstance;


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


pub fn handle_bank_trade(
    pid: Uuid,
    give: ResourceType,
    receive: ResourceType,
    game: &mut GameInstance,
) -> Result<ServerMessage, String> {
    {
        if pid != game.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if game.is_initial_phase() {
            return Err("Cannot trade during initial placement!".to_string());
        }
    }

    let tm = &mut game.turn_manager;
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
                .map_err(|e| format!("{:?}", e))?;

            return Ok(ServerMessage::BankTradeCompleted {
                player_id: pid,
                gave: give,
                received: receive,
            });
        }
    }

    Err("Invalid bank trade".to_string())
}


pub fn handle_trade_offer(
    pid: Uuid,
    target_player_id: Option<Uuid>,
    offer: shared::Resources,
    request: shared::Resources,
    game: &mut GameInstance,
) -> Result<ServerMessage, String> {
    {
        if pid != game.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if game.is_initial_phase() {
            return Err("Cannot trade during initial placement!".to_string());
        }
    }

    let tm = &mut game.turn_manager;

    let proposer = tm
        .players
        .get(pid)
        .ok_or("Player not found")?;

    let offer_set = resources_to_set(&offer);

    if !proposer.can_pay(&offer_set) {
        return Err("You don't have enough resources for this trade".to_string());
    }

    let offer_id = game.next_trade_id;
    game.next_trade_id += 1;

    let pending_trade = PendingTrade {
        offer_id,
        proposer_id: pid,
        target_player_id,
        offering: offer.clone(),
        requesting: request.clone(),
        declined_by: HashSet::new(),
    };

    game.pending_trades.insert(offer_id, pending_trade);

    Ok(ServerMessage::TradeProposed {
        offer_id,
        proposer_id: pid,
        target_player_id,
        offering: offer,
        requesting: request,
    })
}


pub fn handle_trade_response(
    pid: Uuid,
    offer_id: u64,
    accept: bool,
    game: &mut GameInstance,
) -> Result<ServerMessage, String> {
    let trade = game.pending_trades.get(&offer_id).cloned();
    let tm = &mut game.turn_manager;

    let trade = match trade {
        None => {
            return Ok(ServerMessage::TradeCancelled { offer_id });
        }
        Some(t) => t,
    };

    if trade.proposer_id == pid {
        return Err("You cannot respond to your own trade".to_string());
    }

    if trade.target_player_id.is_some() && trade.target_player_id != Some(pid) {
        return Err("This trade was not offered to you".to_string());
    }

    if trade.declined_by.contains(&pid) {
        return Err("You already declined this trade".to_string());
    }

    if !accept {
        if let Some(t) = game.pending_trades.get_mut(&offer_id) {
            t.declined_by.insert(pid);
        }

        let eligible_count = if trade.target_player_id.is_some() {
            1
        } else {
            game.player_ids.len() - 1
        };

        let declined_count = game
            .pending_trades
            .get(&offer_id)
            .map(|t| t.declined_by.len())
            .unwrap_or(0);

        if declined_count >= eligible_count {
            game.pending_trades.remove(&offer_id);
        }

        return Ok(ServerMessage::TradeDeclined {
            offer_id,
            decliner_id: pid,
        });
    }

    let proposer_gives = resources_to_set(&trade.offering);
    let accepter_gives = resources_to_set(&trade.requesting);

    let proposer = tm
        .players
        .get(trade.proposer_id)
        .ok_or("Proposer not found")?;

    let accepter = tm
        .players
        .get(pid)
        .ok_or("Accepter not found")?;

    if !proposer.can_pay(&proposer_gives) {
        game.pending_trades.remove(&offer_id);
        return Err("Proposer no longer has the resources".to_string());
    }

    if !accepter.can_pay(&accepter_gives) {
        return Err("You don't have enough resources for this trade".to_string());
    }

    tm.bank
        .collect_from_player_to_player(
            trade.proposer_id,
            pid,
            &proposer_gives,
            &mut tm.players
        )
        .map_err(|e| format!("{:?}", e))?;

    tm.bank
        .collect_from_player_to_player(
            pid,
            trade.proposer_id,
            &accepter_gives,
            &mut tm.players
        )
        .map_err(|e| format!("{:?}", e))?;

    game.pending_trades.remove(&offer_id);

    Ok(ServerMessage::TradeCompleted {
        offer_id,
        proposer_id: trade.proposer_id,
        accepter_id: pid,
        proposer_gave: trade.offering.clone(),
        accepter_gave: trade.requesting.clone(),
    })
}


pub fn handle_cancel_trade(pid: Uuid, offer_id: u64, game: &mut GameInstance) -> Result<ServerMessage, String> {
    let trade = game.pending_trades.get(&offer_id);
    match trade {
        None => Err("Trade not found".to_string()),
        Some(trade) => {
            if trade.proposer_id != pid {
                Err("You can only cancel your own trades".to_string())
            } else {
                game.pending_trades.remove(&offer_id);
                Ok(ServerMessage::TradeCancelled { offer_id })
            }
        }
    }
}