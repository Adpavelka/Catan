use actix::{AsyncContext, Context, WrapFuture};
use log::{error, info, warn};
use uuid::Uuid;
use shared::{GamePhase, SeatRequest, ServerMessage};
use crate::lobby::{GameInstance, Lobby};

impl Lobby { 
    pub fn handle_leave_game(&mut self, pid: Uuid, game_id: String, ctx: &mut Context<Lobby>) {
        let can_leave_result = if let Some(game) = self.games.get(&game_id) {
            if game.turn_manager.players.get(pid).is_none() {
                Err("You are not in this game")
            /* } else if !matches!(game.phase, GamePhase::WaitingForPlayers) {
                Err("Cannot leave a game in progress")*/
            } else {
                Ok(())
            }
        } else {
            Err("Game not found")
        };

        if let Err(e) = can_leave_result {
            return self.send_error(pid, e);
        }

        // Whether the table is waiting on the person walking away. Read
        // before the removal, which slides the next player into their slot.
        let was_on_turn = self
            .games
            .get(&game_id)
            .is_some_and(|game| game.turn_manager.players.get_current_player().id == pid);

        let mut should_remove_game = false;
        let mut closed_trades = Vec::new();
        {
            if let Some(game) = self.games.get_mut(&game_id) {
                match game.remove_player(pid) {
                    Ok(closed) => closed_trades = closed,
                    Err(e) => {
                        warn!("Could not remove player {} from game {}: {}", pid, game_id, e)
                    }
                }

                if game.turn_manager.players.len() == 0 {
                    should_remove_game = true;
                }
            }
        }

        for offer_id in closed_trades {
            self.broadcast_to_game(&game_id, ServerMessage::TradeCancelled { offer_id });
        }

        self.player_to_game.remove(&pid);

        info!(
            "Player {} left game {}; {} remaining, game kept: {}",
            pid,
            game_id,
            self.games
                .get(&game_id)
                .map_or(0, |game| game.turn_manager.players.len()),
            !should_remove_game,
        );

        self.send_server_msg(
            pid,
            ServerMessage::Left {
                player_id: pid,
                game_id: game_id.clone(),
            },
        );

        // Tell the players who stayed. Without this their roster still lists
        // somebody who has gone, and - if that somebody was on turn - their
        // board sits waiting on a player who will never move again. The
        // leaver is already out of the game, so a broadcast does not reach
        // them; they got their own `Left` above.
        if !should_remove_game {
            self.broadcast_to_game(
                &game_id,
                ServerMessage::Left {
                    player_id: pid,
                    game_id: game_id.clone(),
                },
            );
            self.broadcast_players_update(&game_id);

            // Their pieces have just come off the board and their cards have
            // gone back to the supply, so a roster update is not enough - the
            // buildings, the roads and the bank all moved. A full sync is the
            // one message that carries every one of them.
            let remaining: Vec<Uuid> = self
                .games
                .get(&game_id)
                .map(|game| {
                    (0..game.turn_manager.players.len())
                        .filter_map(|i| game.turn_manager.players.get_by_index(i).map(|p| p.id))
                        .collect()
                })
                .unwrap_or_default();
            for player_id in remaining {
                self.send_full_sync(player_id, &game_id);
            }

            if was_on_turn {
                if let Some(next) = self
                    .games
                    .get(&game_id)
                    .map(|game| game.turn_manager.players.get_current_player().id)
                {
                    self.broadcast_to_game(&game_id, ServerMessage::NextTurn { player_id: next });
                }
            }
        }

        if should_remove_game {
            let gid_for_repo = game_id.clone();
            let repo = self.repo.clone();
            self.games.remove(&game_id);

            ctx.spawn(async move {
                let _ = repo.delete_game_instance(&gid_for_repo).await;
            }.into_actor(self));

            info!("Game {} removed as it has no players left", game_id);
        }

        self.broadcast_lobby_status();
    }

    pub fn handle_join_game(&mut self, pid: Uuid, game_id: String, seat: SeatRequest, ctx: &mut Context<Lobby>) {
        let (is_full, missing) = {
            let Some(game) = self.games.get_mut(&game_id) else {
                return self.send_error(pid, "Game not found");
            };

            // Already seated? This is a reconnect, so their existing name and
            // colour stand and the request is ignored.
            let missing = game.turn_manager.players.get(pid).is_none();

            if missing {
                // A game used to only start when it was full, so "full" also
                // meant "started". It can now start early, and seating a
                // newcomer mid-game would splice them into the turn order
                // with no settlements and shift everybody else's slot.
                if game.get_state() != GamePhase::WaitingForPlayers {
                    return self.send_error(pid, "This game has already started");
                }

                if game.turn_manager.players.len() >= game.max_players {
                    return self.send_error(pid, "Game is full");
                }

                if let Err(e) = game.turn_manager.players.seat(pid, &seat.name, seat.colour) {
                    return self.send_error(pid, &e);
                }
            }

            (game.turn_manager.players.len() >= game.max_players, missing)
        };

        if missing {
            self.player_to_game.insert(pid, game_id.clone());
        }

        if is_full {
            self.begin_game(&game_id);
        }

        self.save_game_async(&game_id, ctx);
        self.send_server_msg(pid, ServerMessage::Joined { player_id: pid, game_id: game_id.clone() });
        self.refresh_game_for_all(&game_id);
        self.broadcast_lobby_status();
    }
    
    /// Start a game early, once the minimum player count is met. Any player in
    /// the lobby may trigger it - waiting for a full table can mean waiting
    /// forever.
    pub fn handle_start_game(&mut self, pid: Uuid, game_id: String, ctx: &mut Context<Lobby>) {
        let Some(game) = self.games.get(&game_id) else {
            return self.send_error(pid, "Game not found");
        };

        if game.turn_manager.players.get(pid).is_none() {
            return self.send_error(pid, "You are not in this game");
        }

        if game.get_state() != GamePhase::WaitingForPlayers {
            return self.send_error(pid, "This game has already started");
        }

        if !game.can_start() {
            return self.send_error(
                pid,
                &format!("Need at least {} players to start", crate::game::entities::game_instance::MIN_PLAYERS),
            );
        }

        self.begin_game(&game_id);
        self.save_game_async(&game_id, ctx);
        self.refresh_game_for_all(&game_id);
        self.broadcast_lobby_status();
    }

    fn begin_game(&mut self, game_id: &str) {
        let first_player = {
            let Some(game) = self.games.get_mut(game_id) else { return };
            if !game.can_start() {
                return;
            }

            game.start_game();
            info!("Game {} starting!", game_id);
            game.turn_manager.players.get_by_index(0).map(|p| p.id)
        };

        if let Some(player_id) = first_player {
            self.broadcast_to_game(game_id, ServerMessage::NextTurn { player_id });
        }
    }

    pub fn handle_create_game(&mut self, pid: Uuid, player_count: usize, seat: SeatRequest, ctx: &mut Context<Lobby>) {
        // Six hex digits is 24 bits, which is short enough to read out to a
        // friend and short enough to collide. A collision used to replace a
        // live game outright - its players still pointed at the id by
        // `player_to_game`, so they woke up in a stranger's game - so keep
        // drawing until the id is free.
        let gid = std::iter::repeat_with(|| Uuid::new_v4().to_string()[..6].to_string())
            .take(16)
            .find(|candidate| !self.games.contains_key(candidate));

        let Some(gid) = gid else {
            error!("Could not find a free game id after 16 tries");
            return self.send_error(pid, "Could not start a game just now. Please try again.");
        };
        info!("Creating game {} for player {}", gid, pid);

        // GameInstance::new seats the creator with the name and colour they chose.
        let game = GameInstance::new(gid.clone(), pid, player_count, &seat.name, seat.colour);

        self.games.insert(gid.clone(), game);
        self.player_to_game.insert(pid, gid.clone());

        self.save_game_async(&gid, ctx);
        self.send_server_msg(pid, ServerMessage::Joined { player_id: pid, game_id: gid.clone() });
        self.refresh_game_for_all(&gid);
        self.broadcast_lobby_status();
    }
}