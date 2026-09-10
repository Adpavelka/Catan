use uuid::Uuid;
use shared::{GamePhase, PendingAction, ServerMessage};
use crate::game::entities::game_instance::GameInstance;
use crate::game::entities::statistics::Gain;

impl GameInstance
{
    pub fn handle_roll_dice(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.get_state() != GamePhase::RegularPlay {
            return Err("You cannot roll right now.".to_string());
        }

        let ((d1, d2), distributed) = self
            .turn_manager
            .roll_dice()
            .map_err(|e| e.to_string())?;

        let roll = d1 + d2;

        // Counters, once the roll has actually happened. The payout is the
        // bank's own record of what it handed out, so the tally cannot claim
        // cards a depleted bank never paid.
        self.stats.record_roll(pid, roll);
        for (player_id, resource, amount) in &distributed {
            self.stats.gained(*player_id, Gain::Roll, *amount);
            self.stats.drew(*resource, *amount);
        }
        for (player_id, amount) in self.robber_blocked_payout(roll) {
            self.stats.blocked_by_robber(player_id, amount);
        }

        let mut discards_count = 0;

        if roll == 7 {
            self.add_pending_action(pid, PendingAction::MoveRobber);

            for idx in 0..self.turn_manager.players.len() {
                let Some(player) = self.turn_manager.players.get_by_index(idx) else { continue };
                let total = player.resources.get_cards_total();

                if total > 7 {
                    self.add_pending_action(player.id, PendingAction::Discard);
                    discards_count += 1;
                }
            }
        }

        Ok(ServerMessage::DiceRolled {
            player_id: pid,
            dice_1: d1,
            dice_2: d2,
            discards_pending: discards_count,
        })
    }


    pub fn handle_end_turn(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        // Finishing a special build hands the slot to the next player rather
        // than ending anybody's turn.
        if self.get_state().special_builder().is_some() {
            return self.handle_end_special_build(pid);
        }

        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.get_state() != GamePhase::RegularPlay {
            return Err("Cannot end turn during initial placement!".to_string());
        }

        if self.pending_actions.contains_key(&pid) {
            return Err("You still have pending actions to complete.".into());
        }

        // Only players still at the table can hold this up. Departures clear
        // their own actions, but a stray entry must never freeze the game.
        for (owner, actions) in &self.pending_actions {
            if actions.contains(&PendingAction::Discard)
                && self.turn_manager.players.get(*owner).is_some()
            {
                return Err("Cannot end turn until all players discard.".into());
            }
        }

        if !self.turn_manager.dice.was_dice_rolled() {
            return Err("Cannot end turn without rolling dices.".into());
        }

        // At 5-6 players everyone else gets a chance to build before the next
        // turn starts, so the turn does not advance yet.
        if self.open_special_building(pid) {
            return Ok(ServerMessage::PhaseChanged { new_phase: self.get_state() });
        }

        self.turn_manager.end_turn();

        Ok(ServerMessage::NextTurn {
            player_id: self.turn_manager.players.get_current_player().id
        })
    }

    fn handle_end_special_build(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        if Some(pid) != self.get_state().special_builder() {
            return Err("It is not your turn to build.".to_string());
        }

        if self.advance_special_building() {
            return Ok(ServerMessage::PhaseChanged { new_phase: self.get_state() });
        }

        self.finish_special_building();
        self.turn_manager.end_turn();

        Ok(ServerMessage::NextTurn {
            player_id: self.turn_manager.players.get_current_player().id,
        })
    }
}


