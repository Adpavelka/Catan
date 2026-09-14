use crate::game::entities::development_card::DevelopmentCard;
use crate::game::entities::resources::ResourceSet;
use crate::game::entities::game_instance::GameInstance;
use crate::game::entities::statistics::{Gain, Loss};
use shared::ResourceType::{Brick, Ore, Sheep, Wheat, Wood};
use shared::{PendingAction, ServerMessage};
use uuid::Uuid;

impl GameInstance {
    pub fn handle_discard_cards(&mut self, pid: Uuid, resources: shared::Resources) -> Result<ServerMessage, String> {
        if !self.has_pending_action(pid, PendingAction::Discard) {
            return Err("You are not required to discard right now".to_string());
        }

        let tm = &mut self.turn_manager;
        let mut discard_set = crate::game::entities::resources::ResourceSet::new();
        discard_set.add(Brick, resources.brick as u32);
        discard_set.add(Wood, resources.lumber as u32);
        discard_set.add(Sheep, resources.wool as u32);
        discard_set.add(Wheat, resources.grain as u32);
        discard_set.add(Ore, resources.ore as u32);

        match tm.players.get_mut(pid) {
            Some(player) => {
                // Half the hand, rounded down - the same figure the client was
                // given in `MustDiscardCards`. Checking only that they can pay
                // what they offered let a player holding twelve cards hand
                // back one and keep the rest.
                let owed = player.resources.get_cards_total() / 2;
                let offered = discard_set.get_cards_total();
                if offered != owed {
                    return Err(format!(
                        "You must discard exactly {owed} cards, not {offered}."
                    ));
                }

                if player.resources.can_pay(&discard_set) {
                    tm.bank
                        .collect_from_player(pid, discard_set, &mut tm.players)
                        .map_err(|e| e.to_string())?;

                    self.remove_pending_action(pid, PendingAction::Discard);

                    let count = resources.brick + resources.lumber + resources.wool + resources.grain + resources.ore;
                    self.stats.lost(pid, Loss::Seven, count as u32);

                    Ok(ServerMessage::CardsDiscarded {
                        player_id: pid,
                        count: count as usize,
                    })
                } else {
                    Err("Not enough resources to discard".to_string())
                }
            }

            None => Err("Player not found".to_string())
        }
    }

    pub fn handle_buy_dev_card(&mut self, pid: Uuid) -> Result<ServerMessage, String> {
        // Buying is allowed in a special building phase; playing is not.
        if pid != self.active_player() {
            return Err("Wait for your turn!".to_string());
        }

        if !self.get_state().allows_building() {
            return Err("You cannot buy a development card right now.".to_string());
        }

        // The card itself is private; only the buyer is told which one it was.
        let card = self.turn_manager.buy_dev_card(pid).map_err(|e| e.to_string())?;

        // The kind is recorded here, where it is already in hand. It is not
        // sent anywhere until the game is over, by which point every card is
        // face up anyway.
        self.stats.dev_card_bought(pid, &card);
        self.stats.lost(pid, Loss::Spending, DevelopmentCard::cost().get_cards_total());

        Ok(ServerMessage::DevCardBought { player_id: pid })
    }

    pub fn handle_play_dev_card(&mut self, pid: Uuid, card: shared::DevCardType, target: Option<shared::DevCardTarget>) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        if self.get_state() != shared::GamePhase::RegularPlay {
            return Err("You can only play development cards on your own turn.".to_string());
        }

        self.turn_manager.play_development_card(card.clone(), target)
            .map(|_| {
                self.stats.dev_card_played(pid, &card);

                match card {
                    shared::DevCardType::Knight => {
                        self.add_pending_action(pid, PendingAction::MoveRobber);
                    }

                    shared::DevCardType::RoadBuilding => {
                        // Two free roads, or fewer if the player is nearly out.
                        let remaining = self
                            .turn_manager
                            .players
                            .get(pid)
                            .map_or(0, |player| player.roads_left())
                            .min(2);
                        self.add_pending_action(pid, PendingAction::RoadBuilding { remaining });
                    }

                    shared::DevCardType::YearOfPlenty => {
                        self.add_pending_action(pid, PendingAction::YearOfPlenty);
                    }

                    shared::DevCardType::Monopoly => {
                        self.add_pending_action(pid, PendingAction::Monopoly);
                    }

                    _ => {} // Vitory_point cannot be played
                }

                ServerMessage::DevCardPlayed {
                    player_id: pid,
                    card_type: card,
                }
            })
            .map_err(|e| e.to_string())
    }

    pub fn handle_move_robber(&mut self, pid: Uuid, q: i32, r: i32) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        let can_move =
            self.has_pending_action(pid, PendingAction::MoveRobber)
            || self.has_pending_action(pid, PendingAction::PlayKnight);

        if !can_move {
            return Err("You are not allowed to move the robber right now.".into());
        }

        match self.turn_manager.move_robber((q, r)) {
            Ok(_) => {
                self.remove_pending_action(pid, PendingAction::MoveRobber);
                self.remove_pending_action(pid, PendingAction::PlayKnight);

                // Moving the robber is what earns the single steal, and only if
                // somebody is actually sitting on the new hex.
                if !self.turn_manager.robbable_players(pid).is_empty() {
                    self.add_pending_action(pid, PendingAction::Steal);
                }

                Ok(ServerMessage::RobberMoved {
                    player_id: pid,
                    new_q: q,
                    new_r: r,
                })
            }

            Err(_) => {
                Ok(ServerMessage::MustMoveRobber { player_id: pid })
            }
        }
    }

    pub fn handle_steal_from_player(&mut self, pid: Uuid, victim_id: Uuid) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }

        // Stealing is only unlocked by moving the robber, and only once.
        if !self.has_pending_action(pid, PendingAction::Steal) {
            return Err("You are not allowed to steal right now.".to_string());
        }

        if victim_id == pid {
            return Err("You cannot steal from yourself".to_string());
        }

        if !self.turn_manager.robbable_players(pid).contains(&victim_id) {
            return Err("That player has no building next to the robber.".to_string());
        }

        let resource = self
            .turn_manager
            .steal_card(pid, victim_id)
            .map_err(|e| e.to_string())?;

        self.remove_pending_action(pid, PendingAction::Steal);

        // A victim with an empty hand loses nothing, so nothing is counted.
        if resource.is_some() {
            self.stats.gained(pid, Gain::Robbery, 1);
            self.stats.lost(victim_id, Loss::Robber, 1);
        }

        Ok(ServerMessage::PlayerRobbed {
            thief_id: pid,
            victim_id,
            stole_a_card: resource.is_some(),
            resource,
        })
    }

    pub fn handle_year_of_plenty_choice(&mut self, pid: Uuid, resource1: shared::ResourceType, resource2: shared::ResourceType) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }
    
        if !self.has_pending_action(pid, PendingAction::YearOfPlenty) {
            return Err("You don't have a pending Year of Plenty.".into());
        }

        let tm = &mut self.turn_manager;

        let mut gain  = ResourceSet::new();
        gain .add(resource1, 1);
        gain .add(resource2, 1);

        tm.bank
            .trade_with_bank(pid, ResourceSet::new(), gain, &mut tm.players, false)
            .map_err(|e| e.to_string())?;

        self.remove_pending_action(pid, PendingAction::YearOfPlenty);
        self.stats.gained(pid, Gain::DevCard, 2);
        // Both cards come off the bank, so both are draws.
        self.stats.drew(resource1, 1);
        self.stats.drew(resource2, 1);

        Ok(ServerMessage::YearOfPlentyResourcesReceived {
            player_id: pid,
            resource1,
            resource2,
        })
    }

    pub fn handle_monopoly_choice(&mut self, pid: Uuid, resource: shared::ResourceType) -> Result<ServerMessage, String> {
        if pid != self.turn_manager.players.get_current_player().id {
            return Err("Wait for your turn!".to_string());
        }
    
        if !self.has_pending_action(pid, PendingAction::Monopoly) {
            return Err("You don't have a pending Monopoly".to_string());
        }

        // Who is about to lose what. The bank reports only the grand total,
        // and afterwards the victims' hands no longer say what was taken.
        let losses: Vec<(Uuid, u32)> = (0..self.turn_manager.players.len())
            .filter_map(|index| self.turn_manager.players.get_by_index(index))
            .filter(|player| player.id != pid)
            .map(|player| (player.id, player.resources.amount_of(resource)))
            .collect();

        let tm = &mut self.turn_manager;

        let victims: Vec<Uuid> = losses.iter().map(|(victim, _)| *victim).collect();
        let total_stolen = tm.bank
            .collect_resource_from_all_to_player(pid, resource, &mut tm.players)
            .map_err(|e| e.to_string())?;

        self.remove_pending_action(pid, PendingAction::Monopoly);

        self.stats.gained(pid, Gain::DevCard, total_stolen);
        for (victim, amount) in losses {
            self.stats.lost(victim, Loss::DevCard, amount);
        }

        Ok(ServerMessage::MonopolyResourcesStolen {
            player_id: pid,
            resource,
            total_stolen,
            victims,
        })
    }
}
