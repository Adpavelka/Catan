use crate::lobby::{GameInstance, Lobby};
use actix::prelude::Context;
use shared::{ClientRequest, ServerMessage};
use uuid::Uuid;

mod lobby;

pub fn handle_lobby_action(
    lobby: &mut Lobby,
    pid: Uuid,
    req: ClientRequest,
    ctx: &mut Context<Lobby>,
) {
    match req {
        ClientRequest::CreateGame { player_count, seat } => {
            lobby.handle_create_game(pid, player_count, seat, ctx);
        }

        ClientRequest::JoinGame { game_id, seat } => {
            lobby.handle_join_game(pid, game_id, seat, ctx)
        }

        ClientRequest::LeaveGame { game_id } => {
            lobby.handle_leave_game(pid, game_id, ctx);
        }

        ClientRequest::GetLobbyList => lobby.broadcast_lobby_status(),

        ClientRequest::StartGame { game_id } => lobby.handle_start_game(pid, game_id, ctx),
        _ => {}
    }
}

pub fn handle_game_request(
    pid: Uuid,
    req: ClientRequest,
    game: &mut GameInstance,
) -> Result<ServerMessage, String> {
    match req {
        ClientRequest::RollDice => game.handle_roll_dice(pid),
        ClientRequest::EndTurn => game.handle_end_turn(pid),

        ClientRequest::BuildSettlement { x, y } => game.handle_build_settlement(pid, x, y),
        ClientRequest::BuildCity { x, y } => game.handle_build_city(pid, x, y),
        ClientRequest::BuildRoad { x1, y1 } => game.handle_build_road(pid, x1, y1),

        ClientRequest::BankTrade { give, receive } => game.handle_bank_trade(pid, give, receive),
        ClientRequest::TradeOffer {
            target_player_id,
            offer,
            request,
        } => game.handle_trade_offer(pid, target_player_id, offer, request),
        ClientRequest::TradeResponse { offer_id, accept } => {
            game.handle_trade_response(pid, offer_id, accept)
        }
        ClientRequest::ConfirmTrade { offer_id, partner_id } => {
            game.handle_confirm_trade(pid, offer_id, partner_id)
        }
        ClientRequest::CancelTrade { offer_id } => game.handle_cancel_trade(pid, offer_id),

        ClientRequest::DiscardCards { resources } => game.handle_discard_cards(pid, resources),
        ClientRequest::BuyDevelopmentCard => game.handle_buy_dev_card(pid),
        ClientRequest::PlayDevCard { card, target } => game.handle_play_dev_card(pid, card, target),
        ClientRequest::MoveRobber { q, r } => game.handle_move_robber(pid, q, r),
        ClientRequest::StealFromPlayer { victim_id } => game.handle_steal_from_player(pid, victim_id),
        ClientRequest::YearOfPlentyChoice { resource1, resource2 } => {
            game.handle_year_of_plenty_choice(pid, resource1, resource2)
        }
        ClientRequest::MonopolyChoice { resource } => game.handle_monopoly_choice(pid, resource),

        ClientRequest::Chat { message } => Ok(ServerMessage::ChatMessage {
            player_id: pid,
            text: message,
        }),

        _ => Err("Action not implemented yet".to_string()),
    }
}