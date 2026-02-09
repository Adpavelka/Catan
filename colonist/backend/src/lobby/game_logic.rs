use log::info;
use crate::{game::entities::game_instance::InitialAction, lobby::Lobby};

impl Lobby
{
    pub(crate) fn advance_initial_placement(&mut self, gid: &str) -> bool {
        let game = match self.games.get_mut(gid) {
            Some(g) => g,
            None => return false,
        };

        if !game.is_initial_phase() {
            return false;
        }

        if game.is_second_phase() {
            if game.turn_manager.players.get_current_index() == 0 {
                info!("Initial placement complete, transitioning to regular play");

                game.advance_phase();
                game.initial_action = None;
                game.turn_manager.players.reset_order();
                return true;
            } else {
                game.turn_manager.players.prev_turn();
            }
        } else {
            if game.turn_manager.players.get_current_index() + 1 == game.turn_manager.players.len() {
                info!("Transitioning to initial placement round 2");

                game.advance_phase();
            } else {
                game.turn_manager.players.next_turn();
            }
        }

        game.initial_action = Some(InitialAction::Settlement);

        info!(
            "Initial placement: Now player {} (slot {})'s turn",
            game.turn_manager.players.get_current_player().id,
            game.turn_manager.players.get_current_index()
        );

        false
    }
}