use crate::game::entities::resources::ResourceSet;
use crate::lobby::GameInstance;
use shared::ResourceType::{Brick, Ore, Sheep, Wheat, Wood};
use shared::{PendingAction, ServerMessage};
use uuid::Uuid;

pub fn handle_discard_cards(pid: Uuid, resources: shared::Resources, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if !game.has_pending_action(pid, PendingAction::Discard) {
        return Err("You are not required to discard right now".to_string());
    }

    let tm = &mut game.turn_manager;
    let mut discard_set = crate::game::entities::resources::ResourceSet::new();
    discard_set.add(Brick, resources.brick as u32);
    discard_set.add(Wood, resources.lumber as u32);
    discard_set.add(Sheep, resources.wool as u32);
    discard_set.add(Wheat, resources.grain as u32);
    discard_set.add(Ore, resources.ore as u32);

    match tm.players.get_mut(pid) {
        Some(player) => {
            if player.resources.can_pay(&discard_set) {
                tm.bank.collect_from_player(pid, discard_set, &mut tm.players).unwrap();

                game.remove_pending_action(pid, PendingAction::Discard);

                let count = resources.brick + resources.lumber + resources.wool + resources.grain + resources.ore;

                Ok(ServerMessage::CardsDiscarded {
                    player_id: pid,
                    count: count as usize,
                })
            } else {
                Err("Not enough resources to discard".to_string())
            }
        }

        None => Err("Player not found".to_string())
    }
}

pub fn handle_buy_dev_card(pid: Uuid, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if pid != game.turn_manager.players.get_current_player().id {
        return Err("Wait for your turn!".to_string());
    }

    game.turn_manager.buy_dev_card()
        .map(|card_type| {
            ServerMessage::DevCardBought {
                player_id: pid,
                card_type,
            }
        })
        .map_err(|e| format!("{:?}", e))
}

pub fn handle_play_dev_card(pid: Uuid, card: shared::DevCardType, target: Option<shared::DevCardTarget>, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if pid != game.turn_manager.players.get_current_player().id {
        return Err("Wait for your turn!".to_string());
    }

    game.turn_manager.play_development_card(card.clone(), target)
        .map(|_| {
            match card {
                shared::DevCardType::Knight => {
                    game.add_pending_action(pid, PendingAction::MoveRobber);
                }

                shared::DevCardType::RoadBuilding => {
                    game.add_pending_action(pid, PendingAction::RoadBuilding { remaining: 2 });
                }

                shared::DevCardType::YearOfPlenty => {
                    game.add_pending_action(pid, PendingAction::YearOfPlenty);
                }

                shared::DevCardType::Monopoly => {
                    game.add_pending_action(pid, PendingAction::Monopoly);
                }

                _ => {} // Vitory_point cannot be played
            }

            ServerMessage::DevCardPlayed {
                player_id: pid,
                card_type: card,
            }
        })
        .map_err(|e| format!("{:?}", e))
}

pub fn handle_move_robber(pid: Uuid, q: i32, r: i32, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if pid != game.turn_manager.players.get_current_player().id {
        return Err("Wait for your turn!".to_string());
    }

    let can_move =
        game.has_pending_action(pid, PendingAction::MoveRobber)
        || game.has_pending_action(pid, PendingAction::PlayKnight);

    if !can_move {
        return Err("You are not allowed to move the robber right now.".into());
    }

    match game.turn_manager.move_robber((q, r)) {
        Ok(_) => {
            game.remove_pending_action(pid, PendingAction::MoveRobber);
            game.remove_pending_action(pid, PendingAction::PlayKnight);

            // Moving the robber is what earns the single steal, and only if
            // somebody is actually sitting on the new hex.
            if !game.turn_manager.robbable_players(pid).is_empty() {
                game.add_pending_action(pid, PendingAction::Steal);
            }

            Ok(ServerMessage::RobberMoved {
                player_id: pid,
                new_q: q,
                new_r: r,
            })
        }

        Err(_) => {
            Ok(ServerMessage::MustMoveRobber { player_id: pid })
        }
    }
}

pub fn handle_steal_from_player(pid: Uuid, victim_id: Uuid, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if pid != game.turn_manager.players.get_current_player().id {
        return Err("Wait for your turn!".to_string());
    }

    // Stealing is only unlocked by moving the robber, and only once.
    if !game.has_pending_action(pid, PendingAction::Steal) {
        return Err("You are not allowed to steal right now.".to_string());
    }

    if victim_id == pid {
        return Err("You cannot steal from yourself".to_string());
    }

    if !game.turn_manager.robbable_players(pid).contains(&victim_id) {
        return Err("That player has no building next to the robber.".to_string());
    }

    let resource = game
        .turn_manager
        .steal_card(pid, victim_id)
        .map_err(|e| e.to_string())?;

    game.remove_pending_action(pid, PendingAction::Steal);

    Ok(ServerMessage::PlayerRobbed {
        thief_id: pid,
        victim_id,
        resource,
    })
}

pub fn handle_year_of_plenty_choice(pid: Uuid, resource1: shared::ResourceType, resource2: shared::ResourceType, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if pid != game.turn_manager.players.get_current_player().id {
        return Err("Wait for your turn!".to_string());
    }
    
    if !game.has_pending_action(pid, PendingAction::YearOfPlenty) {
        return Err("You don't have a pending Year of Plenty.".into());
    }

    let tm = &mut game.turn_manager;

    let mut gain  = ResourceSet::new();
    gain .add(resource1, 1);
    gain .add(resource2, 1);

    tm.bank
        .trade_with_bank(pid, ResourceSet::new(), gain, &mut tm.players, false)
        .map_err(|e| format!("{:?}", e))?;

    game.remove_pending_action(pid, PendingAction::YearOfPlenty);

    Ok(ServerMessage::YearOfPlentyResourcesReceived {
        player_id: pid,
        resource1,
        resource2,
    })
}

pub fn handle_monopoly_choice(pid: Uuid, resource: shared::ResourceType, game: &mut GameInstance) -> Result<ServerMessage, String> {
    if pid != game.turn_manager.players.get_current_player().id {
        return Err("Wait for your turn!".to_string());
    }
    
    if !game.has_pending_action(pid, PendingAction::Monopoly) {
        return Err("You don't have a pending Monopoly".to_string());
    }

    let tm = &mut game.turn_manager;

    let total_stolen = tm.bank
        .collect_resource_from_all_to_player(pid, resource, &mut tm.players)
        .map_err(|e| format!("{:?}", e))?;

    game.remove_pending_action(pid, PendingAction::Monopoly);

    Ok(ServerMessage::MonopolyResourcesStolen {
        player_id: pid,
        resource,
        total_stolen,
    })
}

