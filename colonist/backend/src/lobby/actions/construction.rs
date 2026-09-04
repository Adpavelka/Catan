use crate::game::entities::game_instance::GameInstance;

use shared::{GamePhase, ServerMessage, StructureType};
use uuid::Uuid;

impl GameInstance
{
    pub fn handle_build_settlement(&mut self, pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        let give_resources = self.validate_settlement_placement()?;

        let is_initial = self.get_state().is_initial_phase();

        // Only advance the phase once the placement has actually succeeded,
        // otherwise a rejected click would leave the game waiting for a road
        // next to a settlement that was never built.
        self.turn_manager
            .build_settlement((x, y), is_initial)
            .map_err(|e| e.to_string())?;

        self.advance_after_settlement(x, y);

        if give_resources {
            self.turn_manager.bank.give_initial_settlement_resources(
                &self.turn_manager.board,
                pid,
                (x, y),
                &mut self.turn_manager.players,
            );
        }

        Ok(ServerMessage::Built {
            player_id: pid,
            structure_type: StructureType::Settlement,
            coords: (x, y),
        })
    }


    pub fn handle_build_city(&mut self,pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.get_state() != GamePhase::RegularPlay {
            return Err("You can only build cities during regular play.".to_string());
        }

        self.turn_manager.build_city((x, y))
            .map(|_| {
                ServerMessage::Built {
                    player_id: pid,
                    structure_type: StructureType::City,
                    coords: (x, y),
                }
            })
            .map_err(|e| e.to_string())
    }


    pub fn handle_build_road(&mut self, pid: Uuid, x: i32, y: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        let free = self.validate_road_placement((x, y))?;

        self.turn_manager
            .build_road((x, y), free)
            .map_err(|e| e.to_string())?;

        self.advance_after_road();

        if free {
            self.decrement_road_building(pid);
        }

        Ok(ServerMessage::Built {
            player_id: pid,
            structure_type: StructureType::Road,
            coords: (x, y),
        })
    }
}
