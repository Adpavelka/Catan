use crate::game::entities::game_instance::GameInstance;
use crate::network::message::ServerMessage;
use crate::repository::game_repository::GameRepository;
use actix::prelude::*;
use std::time::Duration;
use std::collections::HashMap;
use uuid::Uuid;

mod handlers;
pub mod actions;
mod networking;
mod game_logic;



pub struct Lobby {
    pub sessions: HashMap<Uuid, Recipient<ServerMessage>>,
    pub games: HashMap<String, GameInstance>,
    pub player_to_game: HashMap<Uuid, String>,
    /// Session token -> player identity. A connection is only that player if
    /// it presents the matching token.
    pub tokens: HashMap<Uuid, Uuid>,
    pub repo: GameRepository, // Repository for persisting game instances
}

/// How often to look for games nobody is playing any more.
const SWEEP_INTERVAL: Duration = Duration::from_secs(300);
/// How long a game may sit with no connected players before it is dropped.
const ABANDONED_AFTER: u64 = 60 * 60;

impl Actor for Lobby {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(SWEEP_INTERVAL, |lobby, ctx| lobby.sweep_stale_games(ctx));
    }
}

impl Lobby {

    pub fn new(
        repo: GameRepository,
        recovered_games: Vec<GameInstance>,
        recovered_tokens: Vec<(Uuid, Uuid)>,
    ) -> Self {
        let mut games = HashMap::new();
        let mut player_to_game = HashMap::new();

        for inst in recovered_games {
            let gid = inst.id.clone();

            for idx in 0..inst.turn_manager.players.len() {
                if let Some(player) = inst.turn_manager.players.get_by_index(idx) {
                    player_to_game.insert(player.id, gid.clone());
                }
            }

            games.insert(gid, inst);
        }

        Lobby {
            sessions: HashMap::new(),
            games,
            player_to_game,
            tokens: recovered_tokens.into_iter().collect(),
            repo, // Store the repository for future persistence calls
        }
    }
}

impl Lobby {
    /// Drops games that are finished, or that have had no connected player for
    /// a while. Without this the in-memory map and the lobby list grow forever.
    fn sweep_stale_games(&mut self, ctx: &mut Context<Self>) {
        let stale: Vec<String> = self
            .games
            .iter()
            .filter(|(_, game)| {
                let anyone_online = (0..game.turn_manager.players.len())
                    .filter_map(|idx| game.turn_manager.players.get_by_index(idx))
                    .any(|player| self.sessions.contains_key(&player.id));

                if anyone_online {
                    return false;
                }

                game.turn_manager.game_over() || game.idle_for_secs() > ABANDONED_AFTER
            })
            .map(|(gid, _)| gid.clone())
            .collect();

        if stale.is_empty() {
            return;
        }

        for gid in stale {
            log::info!("Evicting stale game {}", gid);

            if let Some(game) = self.games.remove(&gid) {
                for idx in 0..game.turn_manager.players.len() {
                    if let Some(player) = game.turn_manager.players.get_by_index(idx) {
                        self.player_to_game.remove(&player.id);
                    }
                }
            }

            let repo = self.repo.clone();
            ctx.spawn(
                async move {
                    let _ = repo.delete_game_instance(&gid).await;
                }
                .into_actor(self),
            );
        }

        self.broadcast_lobby_status();
    }
}
