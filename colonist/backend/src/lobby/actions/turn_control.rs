use uuid::Uuid;
use shared::{ServerMessage, GamePhase};
use crate::lobby::GameInstance;

impl GameInstance
{
    pub fn handle_roll_dice(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        let tm = &mut self.turn_manager;

        if pid != tm.players.get_current_player().id { return Err("Wait for your turn!".to_string()); }

        if matches!(self.phase, GamePhase::InitialPlacementRound1 | GamePhase::InitialPlacementRound2) {
            return Err("Cannot roll dice during initial placement!".to_string());
        }

        tm.next_turn()
            .map(|((d1, d2), _)| {
                let roll = d1 + d2;
                let mut discards_count = 0;

                if roll == 7 {
                    self.seven_roller = Some(pid);
                    self.pending_discards.clear();

                    for idx in 0..tm.players.len() {
                        let player = tm.players.get_by_index(idx).unwrap();

                        let total = player.resources.get_cards_total();
                        if total > 7 {
                            self.pending_discards.insert(player.id);
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
        let tm = &mut self.turn_manager;

        if pid != tm.players.get_current_player().id { return Err("Wait for your turn!".to_string()); }
        if self.phase != GamePhase::RegularPlay { return Err("Cannot end turn during initial placement!".to_string()); }
        if self.seven_roller.is_some() {
            return Err("Cannot end turn until you move the robber".to_string());
        }
        tm.end_turn();
        Ok(ServerMessage::NextTurn { player_id: tm.players.get_current_player().id  })
    }
}


