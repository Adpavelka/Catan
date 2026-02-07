use log::info;
use shared::GamePhase;
use crate::lobby::Lobby;

impl Lobby
{
    pub(crate) fn advance_initial_placement(&mut self, gid: &str) -> bool {
        let game = match self.games.get_mut(gid) {
            Some(g) => g,
            None => return false,
        };

        let max_settlements = game.max_players * 2;

        if game.initial_settlements_placed >= max_settlements {
            info!("Initial placement complete, transitioning to regular play");
            game.phase = GamePhase::RegularPlay;
            game.turn_manager.new_players.reset_order();
            return true;  // Signal that we transitioned to regular play
        }

        let settlements_in_round1 = game.max_players;
        if game.initial_settlements_placed <= settlements_in_round1 {
            if game.initial_settlements_placed == settlements_in_round1 {
                info!("Transitioning to initial placement round 2 - player {} goes again", game.turn_manager.new_players.get_current_index());
                game.phase = GamePhase::InitialPlacementRound2;
            } else {
                game.turn_manager.new_players.next_turn();
            }
        } else {
            game.turn_manager.new_players.prev_turn();
        }

        info!("Initial placement: Now player {} (slot {})'s turn", game.turn_manager.new_players.get_current_player().id, game.turn_manager.new_players.get_current_index());
        false  // Did not transition to regular play
    }
}