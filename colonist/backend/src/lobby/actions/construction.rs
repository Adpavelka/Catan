use crate::{game::entities::game_instance::InitialAction, lobby::GameInstance};
use shared::ServerMessage;
use uuid::Uuid;

impl GameInstance
{
    pub fn handle_build_settlement(&mut self, pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.is_initial_phase() && self.initial_action != Some(InitialAction::Settlement) {
            return Err("You must build a road next.".into());
        }

        let is_initial = self.is_initial_phase();

        self.turn_manager
            .build_settlement((x, y), is_initial)
            .map(|_| {
                if self.is_second_phase() {
                    self.turn_manager.bank.give_initial_settlement_resources(
                        &self.turn_manager.board,
                        pid,
                        (x, y),
                        &mut self.turn_manager.players,
                    );
                }

                if is_initial {
                    self.initial_action = Some(InitialAction::Road);
                }

                ServerMessage::Built {
                    player_id: pid,
                    structure_type: "SETTLEMENT".into(),
                    coords: vec![x, y],
                }
            })
            .map_err(|e| format!("{:?}", e))
    }

    pub fn handle_build_city(&mut self,pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        self.turn_manager.build_city((x, y))
            .map(|_| {
                ServerMessage::Built {
                    player_id: pid,
                    structure_type: "CITY".into(),
                    coords: vec![x, y],
                }
            })
            .map_err(|e| format!("{:?}", e))
    }

    pub fn handle_build_road(&mut self, pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.is_initial_phase() && self.initial_action != Some(InitialAction::Road) {
            return Err("You must build a settlement first.".into());
        }

        let free = self.is_initial_phase() || self.free_roads_remaining > 0;

        self.turn_manager
            .build_road((x, y), free)
            .map(|_| {
                if self.is_initial_phase() {
                    self.initial_action = Some(InitialAction::Settlement);
                    //self.advance_initial_placement(); // <-- move turn here
                } else if self.free_roads_remaining > 0 {
                    self.free_roads_remaining -= 1;
                }

                ServerMessage::Built {
                    player_id: pid,
                    structure_type: "ROAD".into(),
                    coords: vec![x, y],
                }
            })
            .map_err(|e| format!("{:?}", e))
    }

}
