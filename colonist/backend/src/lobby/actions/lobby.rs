use actix::{AsyncContext, Context, WrapFuture};
use log::{info, warn};
use uuid::Uuid;
use shared::{GamePhase, ServerMessage};
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

        let mut should_remove_game = false;
        {
            if let Some(game) = self.games.get_mut(&game_id) {
                if let Err(e) = game.turn_manager.players.remove_player(pid) {
                    warn!("Could not remove player {} from game {}: {}", pid, game_id, e);
                }

                if game.turn_manager.players.len() == 0 {
                    should_remove_game = true;
                }
            }
        }

            self.player_to_game.remove(&pid);

            self.send_server_msg(
            pid,
            ServerMessage::Left {
                player_id: pid,
                game_id: game_id.clone(),
            },
        );

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

    pub fn handle_join_game(&mut self, pid: Uuid, game_id: String, ctx: &mut Context<Lobby>) {
        let (is_full, missing) = {
            let Some(game) = self.games.get_mut(&game_id) else {
                return self.send_error(pid, "Game not found");
            };

            if game.turn_manager.players.len() >= game.max_players && game.turn_manager.players.get(pid).is_none() {
                return self.send_error(pid, "Game is full");
            }

            let missing = game.turn_manager.players.get(pid).is_none();
            if missing {
                game.turn_manager.players.add_player_with_colour(pid);
            }

            (game.turn_manager.players.len() >= game.max_players, missing)
        };

        if missing {
            self.player_to_game.insert(pid, game_id.clone());
        }

        if is_full {
            if let Some(game) = self.games.get_mut(&game_id) {
                if game.get_state() == GamePhase::WaitingForPlayers {
                    game.start_game();
                    info!("Game {} starting!", game_id);

                    let first_player = game.turn_manager.players.get_by_index(0).unwrap().id;
                    self.send_server_msg(pid, ServerMessage::NextTurn { player_id: first_player });
                }
            }
        }

        self.save_game_async(&game_id, ctx);
        self.send_server_msg(pid, ServerMessage::Joined { player_id: pid, game_id: game_id.clone() });
        self.refresh_game_for_all(&game_id);
        self.broadcast_lobby_status();
    }
    
    pub fn handle_create_game(&mut self, pid: Uuid, player_count: usize, ctx: &mut Context<Lobby>) {
        let gid = Uuid::new_v4().to_string()[..6].to_string();
        info!("Creating game {} for player {}", gid, pid);

        // GameInstance::new already seats the creator.
        let game = GameInstance::new(gid.clone(), pid, player_count);

        self.games.insert(gid.clone(), game);
        self.player_to_game.insert(pid, gid.clone());

        self.save_game_async(&gid, ctx);
        self.send_server_msg(pid, ServerMessage::Joined { player_id: pid, game_id: gid.clone() });
        self.refresh_game_for_all(&gid);
        self.broadcast_lobby_status();
    }
}