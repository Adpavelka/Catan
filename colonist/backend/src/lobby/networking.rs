use actix::{AsyncContext, Context, WrapFuture};
use log::{error, info, warn};
use shared::PendingAction;
use uuid::Uuid;
use crate::lobby::Lobby;
use crate::network::message::ServerMessage;
use crate::game::entities::game_instance::GameInstance;

impl Lobby
{
    pub(crate) fn send_server_msg(&self, pid: Uuid, msg: shared::ServerMessage) {
        match serde_json::to_string(&msg) {
            Ok(json_string) => {
                let actix_msg = ServerMessage(json_string);
                if let Some(addr) = self.sessions.get(&pid) {
                    let _ = addr.do_send(actix_msg);
                } else {
                    warn!("Failed to send: Player {} session not found", pid);
                }
            },
            Err(e) => error!("Serialization error: {}", e),
        }
    }

    pub fn broadcast_to_game(&self, game_id: &str, msg: shared::ServerMessage) {
        if let Some(game) = self.games.get(game_id) {
            if let Ok(json_string) = serde_json::to_string(&msg) {
                let actix_msg = ServerMessage(json_string);
                for idx in 0..game.turn_manager.players.len() {
                    let Some(player) = game.turn_manager.players.get_by_index(idx) else { continue };
                    if let Some(addr) = self.sessions.get(&player.id) {
                        let _ = addr.do_send(actix_msg.clone());
                    }
                }
            }
        }
    }

    /// Broadcast to everyone in the game except `skip`. Used where the same
    /// event carries more detail for the players involved than for onlookers.
    pub fn broadcast_except(&self, game_id: &str, skip: &[Uuid], msg: shared::ServerMessage) {
        let Some(game) = self.games.get(game_id) else { return };
        let Ok(json_string) = serde_json::to_string(&msg) else { return };

        let actix_msg = ServerMessage(json_string);
        for idx in 0..game.turn_manager.players.len() {
            let Some(player) = game.turn_manager.players.get_by_index(idx) else { continue };
            if skip.contains(&player.id) {
                continue;
            }
            if let Some(addr) = self.sessions.get(&player.id) {
                let _ = addr.do_send(actix_msg.clone());
            }
        }
    }

    pub fn broadcast_lobby_status(&self) {
        info!("Broadcasting lobby status to all players");
        let mut games_data: Vec<shared::LobbyGameInfo> = self.games.iter()
            .map(|(gid, game)| shared::LobbyGameInfo {
                game_id: gid.clone(),
                players: game.turn_manager.players.len(),
                max_players: game.max_players,
                available_colours: game.available_colours(),
                victory_points_to_win: game.rules().victory_points_to_win,
            })
            .collect();

        // `games` is a HashMap, so its iteration order changes whenever a game
        // is created or evicted. Sending it unsorted makes the lobby list jump
        // around under the cursor, and somebody aiming at their friend's game
        // clicks JOIN on whatever slid into that row instead. Sorting by id
        // gives every client the same stable list.
        games_data.sort_by(|a, b| a.game_id.cmp(&b.game_id));

        let lobby_update = shared::ServerMessage::LobbyUpdate { games: games_data };
        for pid in self.sessions.keys() {
            self.send_server_msg(*pid, lobby_update.clone());
        }
    }
    fn send_game_started(&self, pid: Uuid, game_id: &str) {
        let Some(game) = self.games.get(game_id) else { return };

        let players = game.get_all_players_info();

        let info = game.turn_manager.board.to_info(game.turn_manager.get_robber_pos());
        let board = shared::BoardState {
            hexes: info.hexes,
            settlements: info.settlements,
            cities: info.cities,
            roads: info.roads,
            robber_pos: info.robber_pos,
            ports: info.ports,
        };

        let (your_resources, your_dev_cards) = Self::private_hand(game, pid);

        let msg = shared::ServerMessage::GameStarted {
            your_player_id: pid,
            players,
            board,
            game_phase: game.get_state().clone(),
            your_resources,
            your_dev_cards,
        };

        self.send_server_msg(pid, msg);
    }
    
    pub(crate) fn send_full_sync(&mut self, pid: Uuid, game_id: &str) {
        let Some(game) = self.games.get(game_id) else { return };

        let last_dice_roll = game.turn_manager.dice.last_roll();
        let (your_resources, your_dev_cards) = Self::private_hand(game, pid);

        let sync_msg = shared::ServerMessage::FullStateSync {
            player_id: pid,
            players: game.get_all_players_info(),
            board: game.turn_manager.board.to_info(game.turn_manager.get_robber_pos()),
            game_phase: game.get_state().clone(),
            current_turn_player_id: game.turn_manager.players.get_current_player().id,
            robber_pos: game.turn_manager.get_robber_pos(),
            last_dice_roll,
            your_resources,
            your_dev_cards,
            pending_trades: game.trade_snapshots_for(pid),
            bank: game.turn_manager.bank.to_info(),
        };

        self.send_server_msg(pid, sync_msg);
        if game.turn_manager.player_secret_victory_points(pid) != 0 {
            self.send_secret_victory_points_to_player(pid, &game_id.to_string());
        }

        let must_move_robber = game
            .pending_actions
            .get(&pid)
            .map_or(false, |actions| {
                actions.contains(&PendingAction::MoveRobber)
                    || actions.contains(&PendingAction::PlayKnight)
            });

        if must_move_robber {
            self.send_server_msg(
                pid,
                shared::ServerMessage::MustMoveRobber { player_id: pid },
            );
        }
    }
    
    pub(crate) fn send_error(&self, pid: Uuid, error_msg: &str) {
        self.send_server_msg(pid, shared::ServerMessage::Error {
            message: error_msg.to_string(),
        });
    }
    pub(crate) fn broadcast_resource_updates(&mut self, gid: &str) {
    let Some(game) = self.games.get(gid) else { return };

    for idx in 0..game.turn_manager.players.len() {
        if let Some(player) = game.turn_manager.players.get_by_index(idx) {
            let res: shared::Resources = (&player.resources).into();
            info!("Sending resource update to pid {}: {:?}", player.id, res);
            let msg = shared::ServerMessage::ResourceUpdate {
                player_id: player.id,
                resources: res,
            };
            self.send_server_msg(player.id, msg);
        }
        }
    }

    pub(crate) fn broadcast_players_update(&self, gid: &str) {
        if let Some(game) = self.games.get(gid) {
            let msg = shared::ServerMessage::PlayersUpdate {
                players: game.get_all_players_info(),
            };
            self.broadcast_to_game(gid, msg);
        }
    }

    pub fn send_secret_victory_points_to_player(&self, pid: Uuid, gid: &str) {
        if let Some(game) = self.games.get(gid) {
            let points = game.turn_manager.player_secret_victory_points(pid);
            let msg = shared::ServerMessage::PlayerSecretVictoryPointsUpdated {
                secret_victory_points: points as i32,
            };
            self.send_server_msg(pid, msg);
        } else {
            error!("Game not found for UUID: {}", gid);
        }
    }
    pub fn save_game_async(&self, gid: &str, ctx: &mut Context<Self>) {
        if let Some(instance) = self.games.get(gid).cloned() {
            let repo = self.repo.clone();
            ctx.spawn(async move {
                let _ = repo.save_game_instance(&instance).await;
            }.into_actor(self));
        }
    }
    pub fn refresh_game_for_all(&mut self, gid: &str) {
        let player_ids: Vec<Uuid> = self.games
            .get(gid)
            .map(|g| (0..g.turn_manager.players.len())
                .filter_map(|i| g.turn_manager.players.get_by_index(i).map(|p| p.id))
                .collect()
            )
            .unwrap_or_default();

        for pid in player_ids {
            self.send_game_started(pid, gid);
        }
    }
    /// A player's own hand. Only ever sent to that player.
    fn private_hand(game: &GameInstance, pid: Uuid) -> (shared::Resources, Vec<shared::DevCardType>) {
        game.turn_manager
            .players
            .get(pid)
            .map(|player| {
                (
                    (&player.resources).into(),
                    player.dev_cards.iter().map(|card| card.get_type()).collect(),
                )
            })
            .unwrap_or_default()
    }

}