use crate::game::entities::game_instance::GameInstance;

use shared::ServerMessage;
use uuid::Uuid;

impl GameInstance
{
    pub fn handle_build_settlement(&mut self, pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        let give_resources = self.validate_and_advance_after_settlement(x, y)?;

        let is_initial = self.get_state().is_initial_phase();

        self.turn_manager
            .build_settlement((x, y), is_initial)
            .map(|_| {
                if give_resources {
                    self.turn_manager.bank.give_initial_settlement_resources(
                        &self.turn_manager.board,
                        pid,
                        (x, y),
                        &mut self.turn_manager.players,
                    );
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

        let free = self.validate_and_advance_after_road((x, y))?;

        self.turn_manager
            .build_road((x, y), free)
            .map_err(|e| format!("{:?}", e))?;

        if free {
            self.decrement_road_building(pid);
        }

        Ok(ServerMessage::Built {
            player_id: pid,
            structure_type: "ROAD".into(),
            coords: vec![x, y],
        })
    }
}
