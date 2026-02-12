use uuid::Uuid;
use shared::{GamePhase, PendingAction, ServerMessage};
use crate::lobby::GameInstance;

impl GameInstance
{
    pub fn handle_roll_dice(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.get_state().is_initial_phase() {
            return Err("Cannot roll dice during initial placement!".to_string());
        }

        self.turn_manager.next_turn()
            .map(|((d1, d2), _)| {
                let roll = d1 + d2;
                let mut discards_count = 0;

                if roll == 7 {
                    self.add_pending_action(pid, PendingAction::MoveRobber);

                    for idx in 0..self.turn_manager.players.len() {
                        let player = self.turn_manager.players.get_by_index(idx).unwrap();
                        let total = player.resources.get_cards_total();
                        
                        if total > 7 {
                            self.add_pending_action(player.id, PendingAction::Discard);
                            discards_count += 1;
                        }
                    }
                }

                ServerMessage::DiceRolled {
                    player_id: pid,
                    dice_1: d1,
                    dice_2: d2,
                    discards_pending: discards_count,
                }
            })
            .map_err(|e| format!("{:?}", e))
    }


    pub fn handle_end_turn(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.get_state() != GamePhase::RegularPlay {
            return Err("Cannot end turn during initial placement!".to_string());
        }

        if self.pending_actions.contains_key(&pid) {
            return Err("You still have pending actions to complete.".into());
        }

        for actions in self.pending_actions.values() {
            if actions.contains(&PendingAction::Discard) {
                return Err("Cannot end turn until all players discard.".into());
            }
        }

        // TODO: also that dice was rolled...

        self.turn_manager.end_turn();

        Ok(ServerMessage::NextTurn {
            player_id: self.turn_manager.players.get_current_player().id
        })
    }
}


