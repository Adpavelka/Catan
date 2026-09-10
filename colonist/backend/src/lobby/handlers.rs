use super::{Lobby, actions};
use crate::network::message::{Authenticate, Authenticated, ClientActorMessage, Connect, Disconnect};
use actions::handle_game_request;
use actix::prelude::*;
use log::{error, info};
use shared::{ClientRequest, GamePhase, PendingAction, ServerMessage, StructureType};
use uuid::Uuid;

impl Handler<Authenticate> for Lobby {
    type Result = MessageResult<Authenticate>;

    fn handle(&mut self, msg: Authenticate, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(token) = msg.token {
            if let Some(player_id) = self.tokens.get(&token) {
                return MessageResult(Authenticated { player_id: *player_id, token });
            }
            info!("Rejected unknown session token; issuing a fresh identity");
        }

        let token = Uuid::new_v4();
        let player_id = Uuid::new_v4();
        self.tokens.insert(token, player_id);

        let repo = self.repo.clone();
        ctx.spawn(
            async move {
                if let Err(e) = repo.save_session(token, player_id).await {
                    error!("Failed to persist session: {}", e);
                }
            }
            .into_actor(self),
        );

        MessageResult(Authenticated { player_id, token })
    }
}

impl Handler<Connect> for Lobby {
    type Result = ();
    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        let pid = msg.player_id;
        info!("Player {} connected via WebSocket", pid);

        self.sessions.insert(pid, msg.addr);
        if let Some(game_id) = self.player_to_game.get(&pid).cloned() {
            if let Some(game) = self.games.get(&game_id) {
                if game.turn_manager.game_over() {
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
            | ClientRequest::GetLobbyList
            | ClientRequest::StartGame { .. }
            | ClientRequest::LeaveGame {..} => {
                actions::handle_lobby_action(self, pid, msg.req, ctx);
                return;
            }
            _ => {}
        }

        let gid = match self.player_to_game.get(&pid) {
            Some(id) => id.clone(),
            None => return self.send_error(pid, "You are not in a game"),
        };
        match self.games.get(&gid) {
            Some(game) if game.turn_manager.game_over() => {
                return self.send_error(pid, "Game is already over");
            }
            None => return self.send_error(pid, "Game not found"),
            _ => {}
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
    pub(super) fn process_successful_action(&mut self, pid: Uuid, gid: &str, msg: ServerMessage, ctx: &mut Context<Self>) {
        if let Some(game) = self.games.get_mut(gid) {
            game.touch();
        }
        self.handle_victory_if_needed(gid);
        self.save_game_async(gid, ctx);
        self.deliver_action_result(gid, msg.clone());
        self.handle_post_message_effects(pid, gid, &msg);
    }

    /// Most results go to the whole table. A decline is between the two
    /// players involved - broadcasting it would tell everyone who refused
    /// what, which is information they should have to ask for.
    fn deliver_action_result(&mut self, gid: &str, msg: ServerMessage) {
        match msg {
            ServerMessage::TradeDeclined { proposer_id, decliner_id, .. } => {
                self.send_server_msg(proposer_id, msg.clone());
                self.send_server_msg(decliner_id, msg);
            }

            // Only the two players involved learn which card was taken.
            // Broadcasting it would hand everyone the information that
            // hiding hands in `PlayerInfo` was meant to withhold.
            ServerMessage::PlayerRobbed { thief_id, victim_id, resource, stole_a_card } => {
                let private = ServerMessage::PlayerRobbed {
                    thief_id,
                    victim_id,
                    resource,
                    stole_a_card,
                };
                self.send_server_msg(thief_id, private.clone());
                self.send_server_msg(victim_id, private);

                self.broadcast_except(
                    gid,
                    &[thief_id, victim_id],
                    ServerMessage::PlayerRobbed {
                        thief_id,
                        victim_id,
                        resource: None,
                        stole_a_card,
                    },
                );
            }

            other => self.broadcast_to_game(gid, other),
        }
    }

    fn handle_victory_if_needed(&mut self, gid: &str) {
        let victory_msg = {
            let Some(game) = self.games.get_mut(gid) else { return };
            let Some(winner) = game.turn_manager.winner() else { return };

            // Stop the clock before reading the tally, so the duration is how
            // long the game took rather than how long it took to be told.
            game.stats.finish(crate::game::entities::game_instance::now_secs());

            ServerMessage::PlayerWon {
                player_id: winner,
                secret_victory_points: game.turn_manager.player_secret_victory_points(winner),
                stats: game.stats_snapshot(),
            }
        };

        self.broadcast_to_game(gid, victory_msg);
    }

    fn handle_post_message_effects(&mut self, pid: Uuid, gid: &str, msg: &ServerMessage) {
        self.handle_discard_flow(gid, msg);
        self.handle_resource_updates(gid, msg);
        self.handle_initial_phase_transition(gid, msg);
        self.handle_dev_card_side_effects(pid, gid, msg);
        self.handle_robber_flow(pid, gid, msg);
        self.close_trades_on_turn_end(gid, msg);
        self.announce_dead_offer(gid, msg);
    }

    /// An offer belongs to the turn it was made in. Letting one outlive the
    /// turn would move resources while somebody else is playing.
    ///
    /// At 5-6 players a turn ends with `PhaseChanged` into the special
    /// building phase rather than `NextTurn`, so both have to close offers.
    fn close_trades_on_turn_end(&mut self, gid: &str, msg: &ServerMessage) {
        // Only a turn actually ending closes offers. Setup broadcasts
        // PhaseChanged for every placement step, and those are not turn ends.
        let ends_turn = matches!(
            msg,
            ServerMessage::NextTurn { .. }
                | ServerMessage::PhaseChanged { new_phase: GamePhase::SpecialBuilding { .. } }
        );
        if !ends_turn {
            return;
        }

        let Some(game) = self.games.get_mut(gid) else { return };
        let closed = game.clear_trades();

        for offer_id in closed {
            self.broadcast_to_game(gid, ServerMessage::TradeCancelled { offer_id });
        }
    }

    /// A decline is private, but the offer disappearing is not: once the last
    /// eligible player refuses, everyone still showing the offer needs to be
    /// told it is gone, or it sits on their screen until the expiry sweep.
    fn announce_dead_offer(&mut self, gid: &str, msg: &ServerMessage) {
        let offer_id = match msg {
            ServerMessage::TradeDeclined { offer_id, .. }
            | ServerMessage::TradeAcceptanceWithdrawn { offer_id, .. } => *offer_id,
            _ => return,
        };

        let still_open = self
            .games
            .get(gid)
            .is_some_and(|game| game.pending_trades.contains_key(&offer_id));

        if !still_open {
            self.broadcast_to_game(gid, ServerMessage::TradeCancelled { offer_id });
        }
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

            if let Some(bank) = self.games.get(gid).map(|g| g.turn_manager.bank.to_info()) {
                self.broadcast_to_game(gid, ServerMessage::BankUpdate { bank });
            }
        }
    }

    fn handle_initial_phase_transition(&mut self, gid: &str, msg: &ServerMessage) {
        // Placing a settlement during setup moves the step to BuildRoad and
        // records which settlement the road has to touch. Without announcing
        // it, clients keep the stale step and cannot tell which edges are
        // legal, so they offer every edge the player is connected to.
        if let ServerMessage::Built { structure_type: StructureType::Settlement, .. } = msg {
            if let Some(phase) = self
                .games
                .get(gid)
                .map(|g| g.get_state())
                .filter(|p| p.is_initial_phase())
            {
                self.broadcast_to_game(gid, ServerMessage::PhaseChanged { new_phase: phase });
            }
        }

        if let ServerMessage::Built { structure_type: StructureType::Road, .. } = msg {
            {
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

                    if let Some(phase) = self
                        .games
                        .get(gid)
                        .map(|g| g.get_state())
                        .filter(|p| p.is_initial_phase())
                    {
                        self.broadcast_to_game(gid, ServerMessage::PhaseChanged { new_phase: phase });
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


        // The card a player drew is theirs alone to see.
        if let ServerMessage::DevCardBought { player_id } = msg {
            let drawn = self
                .games
                .get(gid)
                .and_then(|game| game.turn_manager.players.get(*player_id))
                .and_then(|player| player.dev_cards.last())
                .map(|card| card.get_type());

            if let Some(card_type) = drawn {
                let is_victory_point = card_type == shared::DevCardType::VictoryPoint;
                self.send_server_msg(*player_id, ServerMessage::DevCardDrawn { card_type });

                if is_victory_point {
                    self.send_secret_victory_points_to_player(*player_id, gid);
                }
            }
        }
    }

    fn handle_discard_flow(&mut self, gid: &str, msg: &ServerMessage) {
        // A 7 opens the discard round; each completed discard may still leave
        // others outstanding. Both cases just re-prompt whoever still owes cards.
        let prompt_needed = match msg {
            ServerMessage::DiceRolled { dice_1, dice_2, .. } => dice_1 + dice_2 == 7,
            ServerMessage::CardsDiscarded { .. } => true,
            _ => false,
        };

        if !prompt_needed {
            return;
        }

        let Some(game) = self.games.get(gid) else { return };

        let outstanding: Vec<(Uuid, usize)> = game
            .pending_actions
            .iter()
            .filter(|(_, actions)| actions.contains(&PendingAction::Discard))
            .filter_map(|(player_id, _)| {
                let player = game.turn_manager.players.get(*player_id)?;
                Some((*player_id, (player.resources.get_cards_total() / 2) as usize))
            })
            .collect();

        for (player_id, count) in outstanding {
            self.send_server_msg(
                player_id,
                ServerMessage::MustDiscardCards { player_id, count },
            );
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