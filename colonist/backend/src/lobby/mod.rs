use crate::game::entities::game_instance::{GameInstance, TurnClockAction};
use crate::network::message::ServerMessage;
use crate::repository::game_repository::GameRepository;
use shared::ServerMessage as ServerMsg;
use actix::prelude::*;
use std::time::Duration;
use std::collections::HashMap;
use uuid::Uuid;

mod handlers;
mod actions;
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
/// How often to retire trade offers that have run out of time. Much shorter
/// than `SWEEP_INTERVAL`, so an offer dies close to when the clients say it
/// will rather than up to five minutes later.
const TRADE_SWEEP_INTERVAL: Duration = Duration::from_secs(5);
/// How often to check turn deadlines. A second is fine granularity for a
/// ten-second roll window and cheap: it is a comparison per running game.
const TURN_SWEEP_INTERVAL: Duration = Duration::from_secs(1);

impl Actor for Lobby {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(SWEEP_INTERVAL, |lobby, ctx| lobby.sweep_stale_games(ctx));
        ctx.run_interval(TRADE_SWEEP_INTERVAL, |lobby, _| lobby.sweep_expired_trades());
        ctx.run_interval(TURN_SWEEP_INTERVAL, |lobby, ctx| lobby.sweep_turn_clocks(ctx));
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
    /// Retires trade offers that have run out of time, telling the table about
    /// each one. The server owns trade expiry: clients only display a
    /// countdown, so without this an offer would sit on screen forever.
    fn sweep_expired_trades(&mut self) {
        let expired: Vec<(String, Vec<u64>)> = self
            .games
            .iter_mut()
            .map(|(gid, game)| (gid.clone(), game.expire_stale_trades()))
            .filter(|(_, ids)| !ids.is_empty())
            .collect();

        for (gid, offer_ids) in expired {
            for offer_id in offer_ids {
                log::info!("Trade offer {} in game {} expired", offer_id, gid);
                self.broadcast_to_game(&gid, ServerMsg::TradeCancelled { offer_id });
            }
        }
    }

    /// Rolls for, or ends the turn of, any player who has run out of time.
    ///
    /// The server owns the clock. A client drawing a countdown cannot enforce
    /// one: the player it is counting down is exactly the player who might
    /// have closed their tab, and the rest of the table would wait forever.
    fn sweep_turn_clocks(&mut self, ctx: &mut Context<Self>) {
        let due: Vec<(String, Uuid, TurnClockAction)> = self
            .games
            .iter_mut()
            .filter_map(|(gid, game)| {
                let action = game.turn_clock_due()?;
                Some((gid.clone(), game.active_player(), action))
            })
            .collect();

        for (gid, pid, action) in due {
            let Some(game) = self.games.get_mut(&gid) else { continue };

            let result = match action {
                TurnClockAction::Roll => game.handle_roll_dice(pid),
                TurnClockAction::EndTurn => game.handle_end_turn(pid),
            };

            match result {
                Ok(msg) => {
                    log::info!("Turn clock: {:?} for player {} in game {}", action, pid, gid);
                    self.process_successful_action(pid, &gid, msg, ctx);
                }
                // Losing a race with the player's own click is normal.
                Err(e) => log::debug!("Turn clock no-op in game {}: {}", gid, e),
            }
        }
    }

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

                // Retire their session tokens with the game. A token cannot be
                // dropped on disconnect - reclaiming a seat after a dropped
                // connection is the whole point of it - but once the game it
                // belonged to is gone there is nothing left to reclaim, and
                // keeping it means the map only ever grows.
                let gone: std::collections::HashSet<Uuid> = (0..game
                    .turn_manager
                    .players
                    .len())
                    .filter_map(|idx| game.turn_manager.players.get_by_index(idx))
                    .map(|player| player.id)
                    .collect();
                self.tokens.retain(|_, pid| !gone.contains(pid));
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
