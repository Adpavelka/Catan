use super::{Lobby, actions};
use crate::network::message::{ClientActorMessage, Connect, Disconnect};
use actions::handle_game_request;
use actix::prelude::*;
use log::{error, info};
use shared::{ClientRequest, GamePhase, PendingAction, ServerMessage};
use uuid::Uuid;

impl Handler<Connect> for Lobby {
    type Result = ();
    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        let pid = msg.player_id;
        info!("Player {} connected via WebSocket", pid);

        self.sessions.insert(pid, msg.addr);
        if let Some(game_id) = self.player_to_game.get(&pid).cloned() {
            if self.games.contains_key(&game_id) {
                if self.games.get(&game_id).unwrap().turn_manager.game_over() {
                    error!(
                        "Player {} attempted to reconnect to game {} which is over",
                        pid, game_id
                    );
                    return;
                }

                info!(
                    "Player {} recognized in game {}. Restoring state...",
                    pid, game_id
                );

                self.send_server_msg(
                    pid,
                    ServerMessage::Joined {
                        player_id: pid,
                        game_id: game_id.clone(),
                    },
                );
                
                self.send_full_sync(pid, &game_id);
                return;
            }
        }
        self.broadcast_lobby_status();
    }
}

impl Handler<Disconnect> for Lobby {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        info!("Player {} disconnected", msg.player_id);
        self.sessions.remove(&msg.player_id);
        self.broadcast_lobby_status();
    }
}

impl Handler<ClientActorMessage> for Lobby {
    type Result = ();
    fn handle(&mut self, msg: ClientActorMessage, ctx: &mut Context<Self>) {
        let pid = msg.player_id;

        match msg.req {
            ClientRequest::CreateGame { .. }
            | ClientRequest::JoinGame { .. }
            | ClientRequest::GetLobbyList | ClientRequest::LeaveGame {..} => {
                actions::handle_lobby_action(self, pid, msg.req, ctx);
                return;
            }
            _ => {}
        }

        let gid = match self.player_to_game.get(&pid) {
            Some(id) => id.clone(),
            None => return self.send_error(pid, "You are not in a game"),
        };
        if self.games.get_mut(&gid).unwrap().turn_manager.game_over() {
            return self.send_error(pid, "Game is already over");
        }

        let action_result = if let Some(game) = self.games.get_mut(&gid) {
            handle_game_request(pid, msg.req, game)
        } else {
            Err("Game not found".to_string())
        };

        match action_result {
            Ok(msg) => {
                self.process_successful_action(pid, &gid, msg, ctx);
            }
            Err(e) => self.send_error(pid, &e),
        }
    }
}

impl Lobby {
    fn process_successful_action(&mut self, pid: Uuid, gid: &str, msg: ServerMessage, ctx: &mut Context<Self>) {
        self.handle_victory_if_needed(gid);
        self.persist_game(gid, ctx);
        self.broadcast_to_game(gid, msg.clone());
        self.handle_post_message_effects(pid, gid, &msg);
    }

    fn handle_victory_if_needed(&mut self, gid: &str) {
        let Some(game) = self.games.get(gid) else { return };
        let Some(winner) = game.turn_manager.winner() else { return };

        let victory_msg = ServerMessage::PlayerWon {
            player_id: winner,
            secret_victory_points: game.turn_manager.player_secret_victory_points(winner),
        };
        self.broadcast_to_game(gid, victory_msg);
    }

    fn persist_game(&self, gid: &str, ctx: &mut Context<Self>) {
        let repo = self.repo.clone();
        if let Some(instance) = self.games.get(gid).cloned() {
            ctx.spawn(
                async move {
                    let _ = repo.save_game_instance(&instance).await;
                }
                .into_actor(self),
            );
        }
    }

    fn handle_post_message_effects(&mut self, pid: Uuid, gid: &str, msg: &ServerMessage) {
        self.handle_discard_flow(gid, msg);
        self.handle_resource_updates(gid, msg);
        self.handle_initial_phase_transition(gid, msg);
        self.handle_dev_card_side_effects(pid, gid, msg);
        self.handle_robber_flow(pid, gid, msg);
    }

    fn handle_resource_updates(&mut self, gid: &str, msg: &ServerMessage) {
        let resource_update_needed = matches!(msg,
            ServerMessage::DiceRolled{..} |
            ServerMessage::Built{..} |
            ServerMessage::CardsDiscarded{..} |
            ServerMessage::BankTradeCompleted{..} |
            ServerMessage::TradeCompleted{..} |
            ServerMessage::PlayerRobbed{..} |
            ServerMessage::DevCardBought{..} |
            ServerMessage::DevCardPlayed{..} |
            ServerMessage::YearOfPlentyResourcesReceived{..} |
            ServerMessage::MonopolyResourcesStolen{..}
        );

        if resource_update_needed {
            self.broadcast_resource_updates(gid);
            self.broadcast_players_update(gid);
        }
    }

    fn handle_initial_phase_transition(&mut self, gid: &str, msg: &ServerMessage) {
        if let ServerMessage::Built { structure_type, .. } = msg {
            if structure_type == "ROAD" {
                let phase_check = self
                    .games
                    .get(gid)
                    .map(|g| g.get_state().is_initial_phase())
                    .unwrap_or(false);

                if phase_check {
                    let transitioned = self.advance_initial_placement(gid);

                    if transitioned {
                        self.broadcast_to_game(
                            gid,
                            ServerMessage::PhaseChanged {
                                new_phase: GamePhase::RegularPlay,
                            },
                        );
                    }

                    if let Some(game) = self.games.get(gid) {
                        self.broadcast_to_game(
                            gid,
                            ServerMessage::NextTurn {
                                player_id: game.turn_manager.players.get_current_player().id,
                            },
                        );
                    }
                }
            }
        }
    }

    fn handle_dev_card_side_effects(&mut self, pid: Uuid, gid: &str, msg: &ServerMessage) {
        if let Some(game) = self.games.get(gid) {
            if let Some(actions) = game.pending_actions.get(&pid) {
                for action in actions {
                    match action {
                        PendingAction::MoveRobber | PendingAction::PlayKnight => {
                            self.send_server_msg(
                                pid,
                                ServerMessage::MustMoveRobber { player_id: pid },
                            );
                        }

                        PendingAction::RoadBuilding { remaining } => {
                            self.send_server_msg(
                                pid,
                                ServerMessage::MustPlaceRoads {
                                    player_id: pid,
                                    roads_remaining: *remaining,
                                },
                            );
                        }

                        PendingAction::YearOfPlenty => {
                            self.send_server_msg(
                                pid,
                                ServerMessage::MustChooseYearOfPlentyResources { player_id: pid },
                            );
                        }

                        PendingAction::Monopoly => {
                            self.send_server_msg(
                                pid,
                                ServerMessage::MustChooseMonopolyResource { player_id: pid },
                            );
                        }

                        _ => {} // Discard handled elsewhere
                    }
                }
            }
        }


        if let ServerMessage::DevCardBought {card_type: shared::DevCardType::VictoryPoint, ..} = msg {
            self.send_secret_victory_points_to_player(pid, gid);
        }
    }

    fn handle_discard_flow(&mut self, gid: &str, msg: &ServerMessage) {
        match msg {
            ServerMessage::DiceRolled { dice_1, dice_2, .. } if dice_1 + dice_2 == 7 => {
                if let Some(game) = self.games.get(gid) {
                    for (player_id, actions) in &game.pending_actions {
                        if actions.contains(&PendingAction::Discard) {
                            if let Some(player) = game.turn_manager.players.get(*player_id) {
                                let total = player.resources.get_cards_total();
                                let count = (total / 2) as usize;

                                self.send_server_msg(
                                    *player_id,
                                    ServerMessage::MustDiscardCards {
                                        player_id: *player_id,
                                        count,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            ServerMessage::CardsDiscarded { .. } => {
                // If someone just discarded, check if other players still need to discard
                if let Some(game) = self.games.get(gid) {
                    for (player_id, actions) in &game.pending_actions {
                        if actions.contains(&PendingAction::Discard) {
                            if let Some(player) = game.turn_manager.players.get(*player_id) {
                                let total = player.resources.get_cards_total();
                                let count = (total / 2) as usize;

                                self.send_server_msg(
                                    *player_id,
                                    ServerMessage::MustDiscardCards {
                                        player_id: *player_id,
                                        count,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            _ => {
                // Other messages are ignored
            }
        }
    }


    fn handle_robber_flow(&mut self, pid: Uuid, gid: &str, msg: &ServerMessage) {
        if let ServerMessage::RobberMoved { .. } = msg {
            if let Some(game) = self.games.get(gid) {
                let robbable_players = game.turn_manager.robbable_players(pid);

                self.send_server_msg(
                    pid,
                    ServerMessage::CanRobPlayers {
                        player_ids: robbable_players.into_iter().collect(),
                    },
                );
            }
        }
    }
}