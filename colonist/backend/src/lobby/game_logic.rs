use log::info;
use shared::{GamePhase, InitialRound, PlacementStep};
use crate::lobby::Lobby;

impl Lobby
{
    pub(crate) fn advance_initial_placement(&mut self, gid: &str) -> bool {
        let game = match self.games.get_mut(gid) {
            Some(g) => g,
            None => return false,
        };

        let (round, step) = match game.get_state() {
            GamePhase::InitialPlacement { round, step } => (round, step),
            _ => return false,
        };

        if !matches!(step, PlacementStep::BuildSettlement) {
            return false;
        }


        match round {
            InitialRound::First => {
                if game.turn_manager.players.get_current_index() + 1 == game.turn_manager.players.len() {
                    info!("Transitioning to initial placement round 2");

                    game.advance_phase();
                } else {
                    game.turn_manager.players.next_turn();
                }
            }

            InitialRound::Second => {
                if game.turn_manager.players.get_current_index() == 0 {
                    info!("Initial placement complete, transitioning to regular play");

                    game.advance_phase();
                    game.turn_manager.players.reset_order();
                    return true;
                } else {
                    game.turn_manager.players.prev_turn();
                }
            }
        }

        info!(
            "Initial placement: Now player {} (slot {})'s turn",
            game.turn_manager.players.get_current_player().id,
            game.turn_manager.players.get_current_index()
        );

        false
    }
}