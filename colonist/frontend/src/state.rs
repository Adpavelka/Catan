use leptos::*;
use shared::{BuildingInfo, ClientRequest, GamePhase, HexInfo, InitialRound, LobbyGameInfo, PlayerColour, PlayerInfo, PortInfo, Resources, ServerMessage};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use futures::{SinkExt, StreamExt};
use futures::channel::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildMode {
    None,
    Settlement,
    City,
    Road,
}

/// A pending trade offer from another player
#[derive(Clone, Debug, PartialEq)]
pub struct PendingTradeOffer {
    pub offer_id: u64,
    pub proposer_id: Uuid,
    pub target_player_id: Option<Uuid>,
    pub offering: Resources,
    pub requesting: Resources,
    /// Unspecified cards on each side; answered by countering, not accepting.
    pub offering_any: u8,
    pub requesting_any: u8,
    pub received_at: f64,  // Timestamp when we received this trade (for timer)
    /// Set when this is a counter to an earlier offer.
    pub counters: Option<u64>,
}

/// An offer of mine that is still on the table, and how people have answered.
#[derive(Clone, Debug, PartialEq)]
pub struct MyOffer {
    pub offer_id: u64,
    pub offering: Resources,
    pub requesting: Resources,
    pub offering_any: u8,
    pub requesting_any: u8,
    /// Bids, oldest first. Accepting is a bid, not a settlement - I pick one.
    pub accepters: Vec<Uuid>,
    /// Refusals, so the panel can show a cross rather than leaving a player
    /// blank: "said no" and "has not answered yet" are different waits.
    pub decliners: Vec<Uuid>,
}

/// A card taken by the robber, for the animation to follow.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct StealEvent {
    pub thief: Uuid,
    pub victim: Uuid,
    /// `None` for onlookers, who are not told which card it was.
    pub resource: Option<shared::ResourceType>,
    /// Monotonic, so two identical steals still count as two events.
    pub seq: u64,
}

/// A Monopoly transfer, which can involve several players at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonopolyEvent {
    pub thief: Uuid,
    pub resource: shared::ResourceType,
    pub victims: Vec<Uuid>,
    pub total_stolen: u32,
    pub seq: u64,
}

/// One line of player chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatLine {
    pub player_id: Uuid,
    pub text: String,
    /// Monotonic, so two identical messages still key apart in a `For`.
    pub seq: u64,
}

#[derive(Clone, Debug, Copy)]
pub struct GameState {
    // Lobby state
    pub messages: RwSignal<Vec<String>>,
    /// Player talk, kept apart from the event log: one is a transcript of the
    /// game, the other is people talking, and mixing them buries both.
    pub chat: RwSignal<Vec<ChatLine>>,
    /// What the bank has left.
    pub bank: RwSignal<shared::BankInfo>,
    pub is_in_game: RwSignal<bool>,
    pub lobby_games: RwSignal<Vec<LobbyGameInfo>>,
    /// The name this player last typed, remembered between sessions.
    pub my_name: RwSignal<String>,
    pub ws_sender: RwSignal<Option<UnboundedSender<Message>>>,
    pub connection_error: RwSignal<Option<String>>,
    pub is_connecting: RwSignal<bool>,

    // Game state
    pub game_id: RwSignal<Option<String>>,
    pub player_id: RwSignal<Option<Uuid>>,
    pub players: RwSignal<Vec<PlayerInfo>>,
    pub current_turn_player: RwSignal<Uuid>,
    pub my_resources: RwSignal<Resources>,
    pub game_phase: RwSignal<GamePhase>,

    // Board state
    pub hexes: RwSignal<Vec<HexInfo>>,
    pub settlements: RwSignal<Vec<BuildingInfo>>,
    pub cities: RwSignal<Vec<BuildingInfo>>,
    pub roads: RwSignal<Vec<BuildingInfo>>,
    pub robber_pos: RwSignal<Option<(i32, i32)>>,
    pub robber_asset: RwSignal<String>,
    pub board_ports: RwSignal<Vec<PortInfo>>,

    // UI state
    /// The roll for *this* turn, cleared when the turn changes. Drives "have
    /// I rolled yet", so it must not outlive the turn.
    pub last_dice_roll: RwSignal<Option<(u8, u8)>>,
    /// The last roll anybody made, never cleared. What the dice tray shows, so
    /// it keeps displaying the previous player's roll instead of blanking.
    pub table_last_roll: RwSignal<Option<(u8, u8)>>,
    /// The total just rolled, and a counter that ticks on every roll.
    ///
    /// The counter is what the board and the resource animation watch: two
    /// eights in a row are the same number but two different events, and a
    /// signal that only carried the total would not fire for the second.
    pub last_roll_event: RwSignal<Option<(u8, u64)>>,
    /// A card that has just changed hands to the robber: who took it, who lost
    /// it, which card it was, and the same ticking counter as the roll.
    ///
    /// The card is only named for the two players in it. Everyone else is told
    /// that *a* card moved, which is what they are entitled to know, and the
    /// animation shows them a face-down one.
    pub last_steal_event: RwSignal<Option<StealEvent>>,
    /// A Monopoly draw that can take several cards at once from several
    /// different players. The animation needs the whole victim list, not just a
    /// single card, because the server is telling us exactly what was taken.
    pub last_monopoly_event: RwSignal<Option<MonopolyEvent>>,
    /// Ticks once per turn handed out by the server.
    ///
    /// The turn clock resets off this rather than off `current_turn_player`.
    /// A signal only notifies when its value actually changes, so keying the
    /// reset on the player's id silently skipped every turn where the same
    /// person went twice - one player left in the game, say - and the
    /// countdown stayed wherever the previous turn had left it. The server
    /// keys its own clock on a turn sequence for exactly this reason.
    pub turn_epoch: RwSignal<u64>,
    pub build_mode: RwSignal<BuildMode>,
    pub my_dev_cards: RwSignal<Vec<shared::DevCardType>>,
    /// Cards drawn this turn. They cannot be played until the next one, so
    /// tracking them is what lets the UI grey them out rather than letting you
    /// click into a server error.
    pub fresh_dev_cards: RwSignal<Vec<shared::DevCardType>>,
    /// One development card per turn: once you play one, the rest grey out.
    pub dev_card_played_this_turn: RwSignal<bool>,

    // Robber state
    pub must_discard_count: RwSignal<Option<usize>>,
    pub must_move_robber: RwSignal<bool>,
    pub must_steal_from_players: RwSignal<Vec<Uuid>>,
    pub waiting_for_discards: RwSignal<bool>,  // True when I rolled 7 and waiting for others to discard

    // Road Builder state
    pub free_roads_remaining: RwSignal<u8>,  // Free roads from Road Builder card

    // Year of Plenty state
    pub year_of_plenty_pending: RwSignal<bool>,  // True when player must choose 2 resources

    // Monopoly state
    pub monopoly_pending: RwSignal<bool>,  // True when player must choose a resource type

    // Port trading state
    pub my_ports: RwSignal<Vec<shared::PortType>>,

    // Player-to-player trading state
    pub incoming_trades: RwSignal<Vec<PendingTradeOffer>>,
    /// Every offer of mine still on the table, oldest first. Several at once
    /// is ordinary play - put two deals up and take whichever lands - and the
    /// server caps how many, so this is a list rather than an Option.
    pub my_offers: RwSignal<Vec<MyOffer>>,
    /// Offer ids I have accepted and am waiting on the proposer to settle.
    pub my_accepted_offers: RwSignal<Vec<u64>>,
    /// Counters I have outstanding, as (offer I answered, my counter's id).
    pub my_counter_offers: RwSignal<Vec<(u64, u64)>>,

    // Winner
    pub secret_victory_points:RwSignal<i32>,
    pub winner_player_id: RwSignal<Option<Uuid>>,
    /// The end-of-game tally, as sent with the victory. `None` until the game
    /// is over; the statistics screen reads nothing else, so it cannot show a
    /// figure the server did not count.
    pub game_stats: RwSignal<Option<shared::GameStats>>,
}

pub fn card_label(card: &shared::DevCardType) -> &'static str {
    match card {
        shared::DevCardType::Knight => "Knight",
        shared::DevCardType::VictoryPoint => "Victory Point",
        shared::DevCardType::RoadBuilding => "Road Building",
        shared::DevCardType::Monopoly => "Monopoly",
        shared::DevCardType::YearOfPlenty => "Year of Plenty",
    }
}

/// A resource's name in running prose, for the log.
pub fn resource_label(res: shared::ResourceType) -> &'static str {
    match res {
        shared::ResourceType::Wood => "wood",
        shared::ResourceType::Brick => "brick",
        shared::ResourceType::Sheep => "sheep",
        shared::ResourceType::Wheat => "wheat",
        shared::ResourceType::Ore => "ore",
        shared::ResourceType::Desert => "nothing",
    }
}

fn bank_trade_resource_key(name: &str) -> Option<shared::ResourceType> {
    match name.trim().to_ascii_lowercase().as_str() {
        "wood" | "lumber" => Some(shared::ResourceType::Wood),
        "brick" => Some(shared::ResourceType::Brick),
        "sheep" | "wool" => Some(shared::ResourceType::Sheep),
        "wheat" | "grain" => Some(shared::ResourceType::Wheat),
        "ore" => Some(shared::ResourceType::Ore),
        _ => None,
    }
}

fn bank_trade_resource_totals(side: &str) -> Vec<(shared::ResourceType, u32)> {
    let mut totals: std::collections::HashMap<shared::ResourceType, u32> = std::collections::HashMap::new();

    for chunk in side.split(" and ").flat_map(|s| s.split(", ")) {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut words = trimmed.split_whitespace();
        let count = words.next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        if count == 0 {
            continue;
        }

        let resource_name = words.collect::<Vec<_>>().join(" ");
        if let Some(resource) = bank_trade_resource_key(&resource_name) {
            *totals.entry(resource).or_insert(0) += count;
        }
    }

    let order = [
        shared::ResourceType::Wood,
        shared::ResourceType::Brick,
        shared::ResourceType::Sheep,
        shared::ResourceType::Wheat,
        shared::ResourceType::Ore,
    ];

    order.into_iter().filter_map(|resource| totals.get(&resource).copied().map(|count| (resource, count))).collect()
}

fn bank_trade_summary(player_name: &str, give: Vec<(shared::ResourceType, u32)>, receive: Vec<(shared::ResourceType, u32)>) -> String {
    let format_pair = |items: &[(shared::ResourceType, u32)]| {
        items.iter()
            .filter(|(_, count)| *count > 0)
            .map(|(resource, count)| format!("{} {}", count, resource.label().to_ascii_lowercase()))
            .collect::<Vec<_>>()
            .join(" and ")
    };

    format!(
        "{player_name} traded {} for {}",
        format_pair(&give),
        format_pair(&receive)
    )
}

fn merge_bank_trade_lines(existing: &str, incoming: &str, player_name: &str) -> String {
    let mut give_totals: std::collections::HashMap<shared::ResourceType, u32> = std::collections::HashMap::new();
    let mut receive_totals: std::collections::HashMap<shared::ResourceType, u32> = std::collections::HashMap::new();

    let parse_line = |line: &str| -> Option<(Vec<(shared::ResourceType, u32)>, Vec<(shared::ResourceType, u32)>)> {
        let line = line.strip_prefix(&format!("{player_name} traded "))?;
        let (give_part, receive_part) = line.split_once(" for ")?;
        Some((bank_trade_resource_totals(give_part), bank_trade_resource_totals(receive_part)))
    };

    if let Some((existing_give, existing_receive)) = parse_line(existing) {
        for (resource, count) in existing_give {
            *give_totals.entry(resource).or_insert(0) += count;
        }
        for (resource, count) in existing_receive {
            *receive_totals.entry(resource).or_insert(0) += count;
        }
    }

    if let Some((incoming_give, incoming_receive)) = parse_line(incoming) {
        for (resource, count) in incoming_give {
            *give_totals.entry(resource).or_insert(0) += count;
        }
        for (resource, count) in incoming_receive {
            *receive_totals.entry(resource).or_insert(0) += count;
        }
    }

    let resource_order = [
        shared::ResourceType::Wood,
        shared::ResourceType::Brick,
        shared::ResourceType::Sheep,
        shared::ResourceType::Wheat,
        shared::ResourceType::Ore,
    ];

    let give = resource_order
        .iter()
        .filter_map(|resource| give_totals.get(resource).copied().map(|count| (*resource, count)))
        .collect::<Vec<_>>();

    let receive = resource_order
        .iter()
        .filter_map(|resource| receive_totals.get(resource).copied().map(|count| (*resource, count)))
        .collect::<Vec<_>>();

    bank_trade_summary(player_name, give, receive)
}

/// The six corners of a hex, in the axial-times-three coordinates the server
/// uses for buildings. Mirrors the server's own formula.
fn hex_vertices(q: i32, r: i32) -> [(i32, i32); 6] {
    let base = (q * 3, r * 3);
    const NB: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];
    std::array::from_fn(|i| {
        let a = NB[i];
        let b = NB[(i + 1) % 6];
        (base.0 + a.0 + b.0, base.1 + a.1 + b.1)
    })
}

impl GameState {
    /// How many offers one player may have open at once. Mirrors the
    /// server's cap so the control greys out instead of inviting a click that
    /// comes back as an error.
    pub const MAX_OPEN_OFFERS: usize = 4;

    /// Whether I already have as many offers on the table as I am allowed.
    pub fn offers_are_full(&self) -> bool {
        self.my_offers.get().len() >= Self::MAX_OPEN_OFFERS
    }

    /// Whether the 5-6 special building phase is currently offered to us.
    pub fn is_my_special_build(&self) -> bool {
        match (self.game_phase.get().special_builder(), self.player_id.get()) {
            (Some(builder), Some(me)) => builder == me,
            _ => false,
        }
    }

    /// Whether we may build or buy right now: on our own turn after rolling,
    /// or when a special building phase has been handed to us.
    pub fn can_build_now(&self) -> bool {
        if self.game_phase.get().special_builder().is_some() {
            return self.is_my_special_build();
        }

        let my_turn = self
            .player_id
            .get()
            .map(|id| id == self.current_turn_player.get())
            .unwrap_or(false);

        my_turn
            && self.last_dice_roll.get().is_some()
            && self.game_phase.get() == GamePhase::RegularPlay
    }

    /// The seat this player is asking for, remembering the name for next time.
    pub fn seat_request(&self, colour: PlayerColour) -> shared::SeatRequest {
        let name = self.my_name.get_untracked().trim().to_string();

        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item("catan_player_name", &name);
        }

        shared::SeatRequest { name, colour }
    }

    /// Rebuilds the trade panel from a full sync. A reconnect used to drop
    /// every open negotiation on the floor: the offer stayed live on the
    /// server, but the proposer could no longer see their bidders and an
    /// accepter could no longer see the offer at all.
    fn restore_trades(&self, me: Uuid, snapshots: Vec<shared::TradeSnapshot>) {
        let now = js_sys::Date::now();
        let lifetime_ms = shared::TRADE_LIFETIME_SECS as f64 * 1000.0;

        let mut incoming = Vec::new();
        let mut accepted = Vec::new();
        let mut mine: Vec<MyOffer> = Vec::new();
        let mut my_counters = Vec::new();

        for snapshot in snapshots {
            if snapshot.accepted_by.contains(&me) {
                accepted.push(snapshot.offer_id);
            }

            if snapshot.proposer_id == me {
                // A counter of mine is an answer, not an offer of my own.
                if let Some(original) = snapshot.counters {
                    my_counters.push((original, snapshot.offer_id));
                } else {
                    // Terms and bids both, so a reconnect redraws the deal
                    // rather than showing an offer with nothing in it. The
                    // server reports who accepted but not who refused, so the
                    // decline tally starts over rather than being invented.
                    mine.push(MyOffer {
                        offer_id: snapshot.offer_id,
                        offering: snapshot.offering,
                        requesting: snapshot.requesting,
                        offering_any: snapshot.offering_any,
                        requesting_any: snapshot.requesting_any,
                        accepters: snapshot.accepted_by,
                        decliners: Vec::new(),
                    });
                }
                continue;
            }

            if snapshot.you_declined {
                continue;
            }

            // Wind the countdown back to where the server has it, so the bar
            // does not restart at full on every reconnect.
            let elapsed_ms = lifetime_ms - (snapshot.seconds_remaining as f64 * 1000.0);

            incoming.push(PendingTradeOffer {
                offer_id: snapshot.offer_id,
                proposer_id: snapshot.proposer_id,
                target_player_id: snapshot.target_player_id,
                offering: snapshot.offering,
                requesting: snapshot.requesting,
                offering_any: snapshot.offering_any,
                requesting_any: snapshot.requesting_any,
                received_at: now - elapsed_ms,
                counters: snapshot.counters,
            });
        }

        self.incoming_trades.set(incoming);
        self.my_accepted_offers.set(accepted);
        mine.sort_by_key(|o| o.offer_id);
        self.my_offers.set(mine);
        self.my_counter_offers.set(my_counters);
    }

    /// Drops any record of a counter once `offer_id` is gone, whether that id
    /// is the counter itself or the offer it was answering.
    fn forget_counters(&self, offer_id: u64) {
        self.my_counter_offers
            .update(|ids| ids.retain(|(original, counter)| *original != offer_id && *counter != offer_id));
        self.incoming_trades
            .update(|trades| trades.retain(|t| t.counters != Some(offer_id)));
    }

    /// Display name for a player, falling back to their id if they are not in
    /// our roster yet.
    pub fn player_name(&self, player_id: Uuid) -> String {
        self.players
            .get_untracked()
            .iter()
            .find(|p| p.player_id == player_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Player {}", player_id))
    }

    /// Everything the table might be waiting on you for.
    ///
    /// Each of these is set by a server prompt and cleared by answering it, so
    /// none of them survives the game it belongs to. Cleared together when a
    /// new game's state arrives, or a prompt from the last one would still be
    /// on screen with nothing behind it.
    pub fn clear_pending_prompts(&self) {
        self.must_discard_count.set(None);
        self.must_move_robber.set(false);
        self.must_steal_from_players.set(Vec::new());
        self.waiting_for_discards.set(false);
        self.year_of_plenty_pending.set(false);
        self.monopoly_pending.set(false);
        self.free_roads_remaining.set(0);
        self.build_mode.set(BuildMode::None);

        // Trades belong to the turn they were made in, and certainly to the
        // game. A win produces no `NextTurn`, so nothing closes the offers
        // that were open on the winning turn - they were still on screen at
        // the next game's first frame. Safe on a reconnect too: `restore_trades`
        // replaces all four wholesale from the sync.
        self.incoming_trades.set(Vec::new());
        self.my_offers.set(Vec::new());
        self.my_accepted_offers.set(Vec::new());
        self.my_counter_offers.set(Vec::new());
    }

    /// What a roll of `total` pays out, as one entry per card: the tile it
    /// comes off, who gets it, and what it is.
    ///
    /// Worked out here rather than read off the wire. We already hold the
    /// board, the buildings and the robber, so the answer is derivable, and
    /// the counts themselves still come from the server's `ResourceUpdate` -
    /// this only drives the log line and the animation. Deriving it means no
    /// new protocol and no per-hex bookkeeping on the backend.
    pub fn roll_payout(&self, total: u8) -> Vec<((i32, i32), Uuid, shared::ResourceType)> {
        // Seven pays nobody; it moves the robber instead.
        if total == 7 {
            return Vec::new();
        }
        let robber = self.robber_pos.get_untracked();
        let settlements = self.settlements.get_untracked();
        let cities = self.cities.get_untracked();

        let mut paid = Vec::new();
        for hex in self.hexes.get_untracked() {
            if hex.number != total
                || hex.resource == shared::ResourceType::Desert
                || robber == Some((hex.q, hex.r))
            {
                continue;
            }
            for v in hex_vertices(hex.q, hex.r) {
                for b in settlements.iter().filter(|b| (b.x, b.y) == v) {
                    paid.push(((hex.q, hex.r), b.player_id, hex.resource));
                }
                // A city is two cards, a settlement one.
                for b in cities.iter().filter(|b| (b.x, b.y) == v) {
                    paid.push(((hex.q, hex.r), b.player_id, hex.resource));
                    paid.push(((hex.q, hex.r), b.player_id, hex.resource));
                }
            }
        }
        paid
    }

    pub fn send(&self, req: ClientRequest) {
        if let Some(tx) = self.ws_sender.get_untracked() {
            if let Ok(json) = serde_json::to_string(&req) {
                logging::log!("Sending: {}", json);
                let _ = tx.unbounded_send(Message::Text(json));
            }
        } else {
            logging::error!("Cannot send: WebSocket not connected!");
        }
    }

    pub fn process_message(&self, text: String) {
        if let Ok(msg) = serde_json::from_str::<ServerMessage>(&text) {
            match msg {
                ServerMessage::LobbyUpdate { games } => {
                    logging::log!("Lobbies updated: {:?}", games);
                    self.lobby_games.set(games);
                }
                ServerMessage::Session { player_id, token } => {
                    logging::log!("Session established for player {}", player_id);
                    self.player_id.set(Some(player_id));

                    // Persist the credential so a reload reclaims this seat.
                    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                        let _ = storage.set_item("catan_session_token", &token.to_string());
                    }
                }
                ServerMessage::Joined { game_id, .. } => {
                    self.game_id.set(Some(game_id));
                    self.is_in_game.set(true);
                    // Sitting down at a new table clears the last one's
                    // result. Without this, joining after a game you saw the
                    // end of opens straight onto its statistics.
                    self.winner_player_id.set(None);
                    self.game_stats.set(None);
                    self.secret_victory_points.set(0);
                }
                ServerMessage::PlayersUpdate { players: updated_players } => {
                    logging::log!("Players update received");
                    let my_id = self.player_id.get_untracked();
                    self.players.update(|players| {
                        for updated_player in updated_players {
                            if let Some(player) = players.iter_mut().find(|p| p.player_id == updated_player.player_id) {
                                // Update my_ports if this is the current player
                                if Some(updated_player.player_id) == my_id {
                                    self.my_ports.set(updated_player.ports.clone());
                                }
                                *player = updated_player;
                            }
                        }
                    });
                }
                ServerMessage::GameStarted { your_player_id, players, board, game_phase, your_resources, your_dev_cards } => {
                    logging::log!("Game started! Your ID: {}, Phase: {:?}", your_player_id, game_phase);
                    self.player_id.set(Some(your_player_id));
                    self.players.set(players.clone());
                    self.hexes.set(board.hexes);
                    self.settlements.set(board.settlements);
                    self.cities.set(board.cities);
                    self.roads.set(board.roads);
                    self.robber_pos.set(Some(board.robber_pos));
                    self.robber_asset.set(board.robber_asset.clone());
                    self.board_ports.set(board.ports);
                    self.game_phase.set(game_phase.clone());

                    // Nothing owed carries from one table to the next. Sitting
                    // down at a new game used to leave whatever the last one
                    // was waiting on still on screen - most visibly a discard
                    // dialog over a hand that owes nothing, which cannot be
                    // dismissed because the server never asked for it.
                    self.clear_pending_prompts();

                    // Find my resources, dev cards, and ports from the players list
                    if let Some(my_player) = players.iter().find(|p| p.player_id == your_player_id) {
                        self.current_turn_player.set(players.first().map(|p| p.player_id).unwrap_or(Uuid::nil()));

                        // DEBUG LOGGING: Verify each player has unique ID
                        logging::log!("=== DEBUG: Player List ===");
                        for (idx, p) in players.iter().enumerate() {
                            logging::log!("  Player {}: ID={}, Name={}", idx, p.player_id, p.name);
                        }
                        logging::log!("  Your ID: {}", your_player_id);
                        logging::log!("========================");

                        self.my_ports.set(my_player.ports.clone());
                    }

                    self.my_resources.set(your_resources);
                    self.my_dev_cards.set(your_dev_cards);

                    let phase_msg = match game_phase {
                        GamePhase::InitialPlacement {round: InitialRound::First, ..} => "Starting initial placement - Round 1!",
                        GamePhase::InitialPlacement {round: InitialRound::Second, ..} => "Initial placement - Round 2!",
                        GamePhase::RegularPlay => "Game started!",
                        GamePhase::SpecialBuilding { .. } => "Special building phase",
                        GamePhase::WaitingForPlayers => "Waiting for players...",
                    };
                    self.messages.update(|m| m.push(format!("{} ({} players)", phase_msg, players.len())));
                }
                ServerMessage::DiceRolled { player_id, dice_1, dice_2, discards_pending } => {
                    let total = dice_1 + dice_2;
                    logging::log!("Player {} rolled: {} + {} = {}, discards_pending: {}", player_id, dice_1, dice_2, total, discards_pending);
                    self.last_dice_roll.set(Some((dice_1, dice_2)));
                    self.table_last_roll.set(Some((dice_1, dice_2)));
                    self.last_roll_event.update(|e| {
                        let seq = e.map_or(0, |(_, n)| n + 1);
                        *e = Some((total, seq));
                    });

                    // If I rolled a 7 and others need to discard, show waiting message
                    if total == 7 && Some(player_id) == self.player_id.get_untracked() && discards_pending > 0 {
                        self.waiting_for_discards.set(true);
                        logging::log!("I rolled 7! {} players must discard...", discards_pending);
                        self.messages.update(|m| m.push(format!(
                            "You rolled 7! Waiting for {} player{} to discard...",
                            discards_pending,
                            if discards_pending == 1 { "" } else { "s" }
                        )));
                    }

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    self.messages.update(|m| m.push(format!("{} rolled {} ({}+{})", player_name, total, dice_1, dice_2)));

                    // Who the roll paid, and in what. Without this the log
                    // says a number came up and nothing about what it did,
                    // which is the half people actually watch for - a card
                    // count on somebody's row goes up and you cannot tell
                    // whether they took wheat or ore off the tile you wanted.
                    let mut tally: Vec<(Uuid, Vec<(shared::ResourceType, u32)>)> = Vec::new();
                    for (_, pid, res) in self.roll_payout(total) {
                        let per_player = match tally.iter_mut().find(|(who, _)| *who == pid) {
                            Some((_, cards)) => cards,
                            None => {
                                tally.push((pid, Vec::new()));
                                &mut tally.last_mut().expect("just pushed").1
                            }
                        };
                        match per_player.iter_mut().find(|(kind, _)| *kind == res) {
                            Some((_, n)) => *n += 1,
                            None => per_player.push((res, 1)),
                        }
                    }
                    for (pid, cards) in tally {
                        let got = cards
                            .into_iter()
                            .map(|(res, n)| format!("{n} {}", resource_label(res)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let line = format!("{} got {}", self.player_name(pid), got);
                        self.messages.update(|m| m.push(line));
                    }
                }
                ServerMessage::ResourceUpdate { player_id, resources } => {
                    logging::log!("Resource update for player {}: {:?}", player_id, resources);

                    // Update my resources if it's me
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.my_resources.set(resources.clone());
                    }

                    // Only ever our own hand; the roster carries counts only.
                    self.players.update(|players| {
                        if let Some(player) = players.iter_mut().find(|p| p.player_id == player_id) {
                            player.resource_count = resources.brick
                                + resources.lumber
                                + resources.wool
                                + resources.grain
                                + resources.ore;
                        }
                    });
                }
                ServerMessage::Built { player_id, structure_type, coords } => {
                    logging::log!("Player {} built {:?} at {:?}", player_id, structure_type, coords);

                    let building = BuildingInfo {
                        player_id,
                        x: coords.0,
                        y: coords.1,
                    };

                    match structure_type {
                        shared::StructureType::Settlement => self.settlements.update(|s| s.push(building)),
                        // A city replaces the settlement that stood there. The
                        // server holds one building per vertex and its syncs
                        // say so, but this incremental path only ever added -
                        // so the vertex kept a settlement that no longer
                        // existed, and any client that resynced afterwards
                        // disagreed with one that had not.
                        shared::StructureType::City => {
                            self.settlements
                                .update(|s| s.retain(|b| (b.x, b.y) != (building.x, building.y)));
                            self.cities.update(|c| c.push(building));
                        }
                        shared::StructureType::Road => {
                            self.roads.update(|r| r.push(building));
                            // If it's me and I was using free roads, decrement the counter
                            if Some(player_id) == self.player_id.get_untracked() {
                                let remaining = self.free_roads_remaining.get_untracked();
                                if remaining > 0 {
                                    let new_remaining = remaining - 1;
                                    self.free_roads_remaining.set(new_remaining);
                                    // If no more free roads, clear build mode
                                    if new_remaining == 0 {
                                        self.build_mode.set(BuildMode::None);
                                    }
                                }
                            }
                        }
                    }

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    self.messages.update(|m| m.push(format!("{} built a {}", player_name, structure_type.label())));
                }
                ServerMessage::NextTurn { player_id } => {
                    logging::log!("Next turn: Player {}", player_id);
                    self.current_turn_player.set(player_id);
                    self.turn_epoch.update(|n| *n += 1);
                    self.last_dice_roll.set(None); // Clear dice roll for new turn
                    self.fresh_dev_cards.set(Vec::new());
                    self.dev_card_played_this_turn.set(false);
                    self.waiting_for_discards.set(false); // Clear waiting state on turn change

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    self.messages.update(|m| m.push(format!("It's {}'s turn", player_name)));
                }
                ServerMessage::PhaseChanged { new_phase } => {
                    logging::log!("Phase changed to: {:?}", new_phase);
                    self.game_phase.set(new_phase.clone());

                    let phase_msg = match new_phase {
                        GamePhase::InitialPlacement {round: InitialRound::First, ..} => "Initial placement - Round 1",
                        GamePhase::InitialPlacement {round: InitialRound::Second, ..} => "Initial placement - Round 2",
                        GamePhase::RegularPlay => "Regular play started!",
                        GamePhase::SpecialBuilding { .. } => {
                            if self.is_my_special_build() {
                                "Special building phase - build or buy, then pass"
                            } else {
                                "Special building phase"
                            }
                        }
                        GamePhase::WaitingForPlayers => "Waiting for players",
                    };
                    self.messages.update(|m| m.push(phase_msg.to_string()));
                }
                ServerMessage::ChatMessage { player_id, text } => {
                    logging::log!("Chat from player {}: {}", player_id, text);
                    self.chat.update(|lines| {
                        let seq = lines.last().map_or(0, |l: &ChatLine| l.seq + 1);
                        lines.push(ChatLine { player_id, text, seq });
                        // A transcript nobody scrolls back through is just a
                        // memory leak; keep the recent history only.
                        if lines.len() > 200 {
                            lines.remove(0);
                        }
                    });
                }
                ServerMessage::BankUpdate { bank } => {
                    self.bank.set(bank);
                }
                ServerMessage::DevCardBought { player_id } => {
                    logging::log!("Player {} bought a dev card", player_id);

                    self.players.update(|players| {
                        if let Some(player) = players.iter_mut().find(|p| p.player_id == player_id) {
                            player.dev_card_count += 1;
                        }
                    });

                    if Some(player_id) == self.player_id.get_untracked() {
                        return;
                    }

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    self.messages.update(|m| m.push(format!("{} bought a development card", player_name)));
                }
                ServerMessage::DevCardDrawn { card_type } => {
                    // Private: only we are told which card we drew.
                    let card_name = card_label(&card_type);
                    self.my_dev_cards.update(|cards| cards.push(card_type.clone()));
                    self.fresh_dev_cards.update(|cards| cards.push(card_type));
                    self.messages.update(|m| m.push(format!("You drew a {} card", card_name)));
                }
                ServerMessage::DevCardPlayed { player_id, card_type } => {
                    logging::log!("Player {} played dev card: {:?}", player_id, card_type);

                    // If it's me, remove from my cards
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.my_dev_cards.update(|cards| {
                            if let Some(pos) = cards.iter().position(|c| c == &card_type) {
                                cards.remove(pos);
                            }
                        });
                        self.dev_card_played_this_turn.set(true);
                    }

                    self.players.update(|players| {
                        if let Some(player) = players.iter_mut().find(|p| p.player_id == player_id) {
                            player.dev_card_count = player.dev_card_count.saturating_sub(1);
                        }
                    });

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    let card_name = match card_type {
                        shared::DevCardType::Knight => "Knight",
                        shared::DevCardType::VictoryPoint => "Victory Point",
                        shared::DevCardType::RoadBuilding => "Road Building",
                        shared::DevCardType::Monopoly => "Monopoly",
                        shared::DevCardType::YearOfPlenty => "Year of Plenty",
                    };

                    if !matches!(
                        card_type,
                        shared::DevCardType::Monopoly | shared::DevCardType::RoadBuilding | shared::DevCardType::YearOfPlenty
                    ) {
                        self.messages.update(|m| m.push(format!("{} played a {} card", player_name, card_name)));
                    }
                }
                ServerMessage::Error { message } => {
                    logging::error!("Server error: {}", message);
                    self.messages.update(|m| m.push(format!("Error: {}", message)));
                }
                ServerMessage::RobberMoved { player_id, new_q, new_r } => {
                    logging::log!("Player {} moved robber to ({}, {})", player_id, new_q, new_r);
                    self.robber_pos.set(Some((new_q, new_r)));

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    self.messages.update(|m| m.push(format!("{} moved the robber", player_name)));
                }
                ServerMessage::PlayerRobbed { thief_id, victim_id, resource, stole_a_card } => {
                    logging::log!("Player {} robbed from Player {}", thief_id, victim_id);

                    let thief_name = self.player_name(thief_id);
                    let victim_name = self.player_name(victim_id);

                    // `resource` is only filled in for the two players
                    // involved; onlookers are told a card moved, not which.
                    let line = match (stole_a_card, resource) {
                        (true, Some(res)) => {
                            format!("{} stole {:?} from {}", thief_name, res, victim_name)
                        }
                        (true, None) => format!("{} stole a card from {}", thief_name, victim_name),
                        (false, _) => {
                            format!("{} couldn't rob {} (no cards)", thief_name, victim_name)
                        }
                    };
                    self.messages.update(|m| m.push(line));

                    // Nothing moved if there was nothing to take, and an
                    // animation of nothing is just a flicker.
                    if stole_a_card {
                        self.last_steal_event.update(|e| {
                            let seq = e.map_or(0, |s: StealEvent| s.seq + 1);
                            *e = Some(StealEvent { thief: thief_id, victim: victim_id, resource, seq });
                        });
                    }
                }
                ServerMessage::MustDiscardCards { player_id, count } => {
                    logging::log!("Player {} must discard {} cards", player_id, count);

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    self.messages.update(|m| m.push(format!("{} must discard {} cards", player_name, count)));

                    // Show discard UI if it's me
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.must_discard_count.set(Some(count));
                    }
                }
                ServerMessage::CardsDiscarded { player_id, count, resources } => {
                    logging::log!("✅ Player {} discarded {} cards: {:?}", player_id, count, resources);

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    let mut parts: Vec<String> = Vec::new();
                    let resource_labels = [
                        (shared::ResourceType::Wood, resources.lumber, "wood"),
                        (shared::ResourceType::Brick, resources.brick, "brick"),
                        (shared::ResourceType::Sheep, resources.wool, "sheep"),
                        (shared::ResourceType::Wheat, resources.grain, "wheat"),
                        (shared::ResourceType::Ore, resources.ore, "ore"),
                    ];

                    for (_, amount, label) in resource_labels {
                        if amount > 0 {
                            parts.push(format!("{} {}", amount, label));
                        }
                    }

                    let details = if parts.is_empty() {
                        "nothing".to_string()
                    } else if parts.len() == 1 {
                        parts[0].clone()
                    } else {
                        let last = parts.last().cloned().unwrap();
                        let rest = parts[..parts.len() - 1].join(", ");
                        format!("{}, and {}", rest, last)
                    };

                    self.messages.update(|m| m.push(format!("{} discarded {}", player_name, details)));

                    // If this is MY discard confirmation, close the discard modal
                    if Some(player_id) == self.player_id.get_untracked() {
                        logging::log!("✅ My discard confirmed! Closing modal.");
                        self.must_discard_count.set(None);
                    }
                }
                ServerMessage::MustMoveRobber { player_id } => {
                    logging::log!("Player {} must move robber", player_id);

                    // Show robber movement UI if it's me
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.waiting_for_discards.set(false);  // Done waiting, now can move robber
                        self.must_move_robber.set(true);
                        self.messages.update(|m| m.push("You must move the robber!".to_string()));
                    }
                }
                ServerMessage::CanRobPlayers { player_ids } => {
                    logging::log!("Can rob from players: {:?}", player_ids);

                    if player_ids.is_empty() {
                        self.messages.update(|m| m.push("No players to rob".to_string()));
                    } else {
                        self.must_steal_from_players.set(player_ids);
                    }
                }
                ServerMessage::MustPlaceRoads { player_id, roads_remaining } => {
                    logging::log!("Player {} must place {} roads", player_id, roads_remaining);

                    // If it's me, enter road building mode
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.free_roads_remaining.set(roads_remaining);
                        self.build_mode.set(BuildMode::Road);
                        self.messages.update(|m| m.push(format!(
                            "Place {} free road{}!",
                            roads_remaining,
                            if roads_remaining == 1 { "" } else { "s" }
                        )));
                    }
                }
                ServerMessage::MustChooseYearOfPlentyResources { player_id } => {
                    logging::log!("Player {} must choose 2 resources for Year of Plenty", player_id);

                    // If it's me, show the resource picker
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.year_of_plenty_pending.set(true);
                        self.messages.update(|m| m.push("Choose 2 resources to receive from the bank!".to_string()));
                    }
                }
                ServerMessage::YearOfPlentyResourcesReceived { player_id, resource1, resource2 } => {
                    logging::log!("Player {} received {:?} and {:?} from Year of Plenty", player_id, resource1, resource2);

                    let me = self.player_id.get_untracked();

                    if Some(player_id) == me {
                        self.year_of_plenty_pending.set(false);
                    }

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Someone".to_string());
                
                    self.messages.update(|m| m.push(format!(
                        "{} received {:?} and {:?} from Year of Plenty.",
                        player_name, resource1, resource2
                    )));
                }
                ServerMessage::MustChooseMonopolyResource { player_id } => {
                    logging::log!("Player {} must choose a resource for Monopoly", player_id);

                    // If it's me, show the resource picker
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.monopoly_pending.set(true);
                        self.messages.update(|m| m.push("Choose a resource type to steal from all players!".to_string()));
                    }
                }
                ServerMessage::MonopolyResourcesStolen { player_id, resource, total_stolen, victims } => {
                    logging::log!("Player {} stole {} {:?} via Monopoly", player_id, total_stolen, resource);

                    // If it's me, close the modal
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.monopoly_pending.set(false);
                    }

                    let victim_copy = victims.clone();
                    self.last_monopoly_event.update(|e| {
                        let seq = e.as_ref().map_or(0, |ev| ev.seq + 1);
                        *e = Some(MonopolyEvent {
                            thief: player_id,
                            resource,
                            victims: victim_copy.clone(),
                            total_stolen,
                            seq,
                        });
                    });

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Someone".to_string());

                    self.messages.update(|m| m.push(format!(
                        "{} used Monopoly and stole {} {:?} from other players!",
                        player_name, total_stolen, resource
                    )));
                }
                ServerMessage::BankTradeCompleted { player_id, gave, received, gave_count, received_count } => {
                    logging::log!("Player {} traded with bank: gave {:?} x{}, received {:?} x{}", player_id, gave, gave_count, received, received_count);

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));

                    let gave_name = match gave {
                        shared::ResourceType::Brick => "Brick",
                        shared::ResourceType::Wood => "Wood",
                        shared::ResourceType::Sheep => "Sheep",
                        shared::ResourceType::Wheat => "Wheat",
                        shared::ResourceType::Ore => "Ore",
                        shared::ResourceType::Desert => "Desert",
                    };

                    let received_name = match received {
                        shared::ResourceType::Brick => "Brick",
                        shared::ResourceType::Wood => "Wood",
                        shared::ResourceType::Sheep => "Sheep",
                        shared::ResourceType::Wheat => "Wheat",
                        shared::ResourceType::Ore => "Ore",
                        shared::ResourceType::Desert => "Desert",
                    };

                    let gave_count = gave_count.max(1);
                    let received_count = received_count.max(1);

                    let trade_line = format!(
                        "{} traded {} {} for {} {}",
                        player_name,
                        gave_count,
                        gave_name,
                        received_count,
                        received_name
                    );

                    self.messages.update(|m| {
                        if let Some(last) = m.last_mut() {
                            if last.starts_with(&format!("{player_name} traded ")) {
                                *last = merge_bank_trade_lines(last, &trade_line, &player_name);
                                return;
                            }
                        }
                        m.push(trade_line);
                    });
                }
                ServerMessage::PortsUpdate { player_id, ports } => {
                    logging::log!("Player {} ports updated: {:?}", player_id, ports);

                    // If it's me, update my ports
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.my_ports.set(ports.clone());

                        // Log what port was gained
                        if let Some(new_port) = ports.last() {
                            let port_desc = match new_port {
                                shared::PortType::ThreeToOne => "3:1 port".to_string(),
                                shared::PortType::TwoToOne(res) => format!("2:1 {:?} port", res),
                            };
                            self.messages.update(|m| m.push(format!("You gained access to a {}!", port_desc)));
                        }
                    }
                }
                ServerMessage::TradeProposed { offer_id, proposer_id, target_player_id, offering, requesting, offering_any, requesting_any, counters } => {
                    logging::log!("Trade proposed: {} from player {}", offer_id, proposer_id);

                    let my_id = self.player_id.get_untracked();

                    // My own offer. A counter I sent is not "my offer" in the
                    // panel sense - it is my answer to somebody else's.
                    if Some(proposer_id) == my_id && counters.is_none() {
                        self.my_offers.update(|offers| {
                            offers.retain(|o| o.offer_id != offer_id);
                            offers.push(MyOffer {
                                offer_id,
                                offering: offering.clone(),
                                requesting: requesting.clone(),
                                offering_any,
                                requesting_any,
                                accepters: Vec::new(),
                                decliners: Vec::new(),
                            });
                        });
                    }

                    // Countering replaces whatever I had said before, so drop
                    // my bid and any earlier counter of mine on the same offer.
                    if let Some(original) = counters {
                        if Some(proposer_id) == my_id {
                            self.my_accepted_offers.update(|ids| ids.retain(|id| *id != original));
                            self.my_counter_offers.update(|ids| {
                                ids.retain(|(orig, _)| *orig != original);
                                ids.push((original, offer_id));
                            });
                        } else {
                            self.my_offers.update(|offers| {
                                if let Some(o) = offers.iter_mut().find(|o| o.offer_id == original) {
                                    o.accepters.retain(|id| *id != proposer_id);
                                }
                            });
                        }
                    }

                    // If this trade is for me (or open to everyone) and I'm not the proposer, add to incoming
                    let is_for_me = target_player_id.is_none() || target_player_id == my_id;
                    if is_for_me && Some(proposer_id) != my_id {
                        // Get current time for the timer
                        let now = js_sys::Date::now();
                        self.incoming_trades.update(|trades| {
                            trades.push(PendingTradeOffer {
                                offer_id,
                                proposer_id,
                                target_player_id,
                                offering,
                                requesting,
                                offering_any,
                                requesting_any,
                                received_at: now,
                                counters,
                            });
                        });
                    }

                    let proposer_name = self.player_name(proposer_id);
                    let line = if counters.is_some() {
                        format!("{} made a counter-offer", proposer_name)
                    } else {
                        format!("{} proposed a trade", proposer_name)
                    };
                    self.messages.update(|m| m.push(line));
                }
                ServerMessage::TradeAccepted { offer_id, accepter_id } => {
                    logging::log!("Trade {} accepted by {}", offer_id, accepter_id);
                    let my_id = self.player_id.get_untracked();

                    // Nothing has moved yet: this only adds a candidate the
                    // proposer can settle with.
                    self.my_offers.update(|offers| {
                        if let Some(o) = offers.iter_mut().find(|o| o.offer_id == offer_id) {
                            if !o.accepters.contains(&accepter_id) {
                                o.accepters.push(accepter_id);
                            }
                            o.decliners.retain(|id| *id != accepter_id);
                        }
                    });

                    if Some(accepter_id) == my_id {
                        self.my_accepted_offers.update(|ids| {
                            if !ids.contains(&offer_id) {
                                ids.push(offer_id);
                            }
                        });
                    }

                    let name = self.player_name(accepter_id);
                    self.messages.update(|m| m.push(format!("{} is willing to trade", name)));
                }
                ServerMessage::TradeAcceptanceWithdrawn { offer_id, accepter_id } => {
                    logging::log!("Trade {} acceptance withdrawn by {}", offer_id, accepter_id);
                    let my_id = self.player_id.get_untracked();

                    self.my_offers.update(|offers| {
                        if let Some(o) = offers.iter_mut().find(|o| o.offer_id == offer_id) {
                            o.accepters.retain(|id| *id != accepter_id);
                        }
                    });

                    if Some(accepter_id) == my_id {
                        self.my_accepted_offers.update(|ids| ids.retain(|id| *id != offer_id));
                        self.incoming_trades.update(|trades| {
                            trades.retain(|t| t.offer_id != offer_id);
                        });
                    }
                }
                ServerMessage::TradeCompleted { offer_id, proposer_id, accepter_id, proposer_gave: _, accepter_gave: _, also_closed } => {
                    logging::log!("Trade completed: {} between {} and {}", offer_id, proposer_id, accepter_id);

                    // The settled offer, plus everything the server dropped
                    // along with it: the offer a counter was answering, and
                    // any sibling counters on it.
                    for id in std::iter::once(offer_id).chain(also_closed.into_iter()) {
                        self.incoming_trades.update(|trades| {
                            trades.retain(|t| t.offer_id != id);
                        });
                        self.my_accepted_offers.update(|ids| ids.retain(|x| *x != id));
                        self.forget_counters(id);
                        self.my_offers.update(|offers| offers.retain(|o| o.offer_id != id));
                    }

                    // Show message
                    let proposer_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == proposer_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", proposer_id));

                    let accepter_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == accepter_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", accepter_id));

                    self.messages.update(|m| m.push(format!("Trade completed between {} and {}", proposer_name, accepter_name)));
                }
                ServerMessage::TradeCancelled { offer_id } => {
                    logging::log!("Trade cancelled: {}", offer_id);

                    // Remove from incoming trades
                    self.incoming_trades.update(|trades| {
                        trades.retain(|t| t.offer_id != offer_id);
                    });
                    self.my_accepted_offers.update(|ids| ids.retain(|id| *id != offer_id));
                    self.forget_counters(offer_id);

                    // The server is the only thing that can retire an offer,
                    // so this is also how an expired one leaves my list.
                    let was_mine = self
                        .my_offers
                        .get_untracked()
                        .iter()
                        .any(|o| o.offer_id == offer_id);
                    if was_mine {
                        self.my_offers.update(|offers| offers.retain(|o| o.offer_id != offer_id));
                        self.messages.update(|m| m.push("Your trade offer closed.".to_string()));
                    }
                }
                ServerMessage::TradeDeclined { offer_id, proposer_id, decliner_id } => {
                    logging::log!("Trade declined: {} by player {}", offer_id, decliner_id);

                    let decliner_name = self.player_name(decliner_id);
                    let my_id = self.player_id.get_untracked();

                    if Some(proposer_id) == my_id {
                        self.my_offers.update(|offers| {
                            if let Some(o) = offers.iter_mut().find(|o| o.offer_id == offer_id) {
                                o.accepters.retain(|id| *id != decliner_id);
                                if !o.decliners.contains(&decliner_id) {
                                    o.decliners.push(decliner_id);
                                }
                            }
                        });
                    }

                    // If I'm the one who declined, remove from my incoming trades
                    if Some(decliner_id) == my_id {
                        self.incoming_trades.update(|trades| {
                            trades.retain(|t| t.offer_id != offer_id);
                        });
                        self.my_accepted_offers.update(|ids| ids.retain(|id| *id != offer_id));
                    }

                    self.messages.update(|m| m.push(format!("{} declined the trade", decliner_name)));
                }
                ServerMessage::FullStateSync {
                    player_id,
                    players,
                    board,
                    game_phase,
                    current_turn_player_id,
                    robber_pos,
                    last_dice_roll,
                    your_resources,
                    your_dev_cards,
                    pending_trades,
                    bank,
                } => {
                    logging::log!("Full state sync received for player {}", player_id);
                    // A sync is the whole truth about what you owe. Clear the
                    // prompts first: the server re-sends the ones that still
                    // stand immediately after this, so anything it has settled
                    // in the meantime - by the turn clock, say - goes with
                    // them instead of hanging about unanswerable.
                    self.clear_pending_prompts();
                    self.player_id.set(Some(player_id));
                    self.players.set(players.clone());
                    self.hexes.set(board.hexes);
                    self.settlements.set(board.settlements);
                    self.cities.set(board.cities);
                    self.roads.set(board.roads);
                    self.robber_pos.set(Some(robber_pos));
                    self.robber_asset.set(board.robber_asset.clone());
                    self.board_ports.set(board.ports);
                    self.game_phase.set(game_phase.clone());
                    self.current_turn_player.set(current_turn_player_id);
                    if let Some(my_player) = players.iter().find(|p| p.player_id == player_id) {
                        self.my_ports.set(my_player.ports.clone());
                    }
                    self.messages.update(|m| m.push("Game state synchronized.".to_string()));
                    self.my_resources.set(your_resources);
                    self.my_dev_cards.set(your_dev_cards);
                    self.last_dice_roll.set(last_dice_roll);
                    if last_dice_roll.is_some() {
                        self.table_last_roll.set(last_dice_roll);
                    }
                    self.bank.set(bank);
                    self.restore_trades(player_id, pending_trades);
                    // We cannot tell from a sync which cards were drawn this
                    // turn, so treat them all as fresh: refusing a legal play
                    // for one turn beats offering an illegal one.
                    self.fresh_dev_cards.set(self.my_dev_cards.get_untracked());
                }
                ServerMessage::PlayerWon {
                    player_id,
                    secret_victory_points,
                    stats,
                } => {
                    logging::log!("Player {} has won the game!", player_id);

                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));
                    // The tally first: the statistics screen opens off the
                    // winner, and would flash an empty table for a frame if
                    // that arrived before its data did.
                    self.game_stats.set(Some(stats));
                    self.winner_player_id.set(Some(player_id));

                    self.messages.update(|m| m.push(format!(
                        "{} has won the game with {} secret victory points!",
                        player_name,
                        secret_victory_points
                    )));
                }
                ServerMessage::PlayerSecretVictoryPointsUpdated { secret_victory_points} =>
                    {
                        self.secret_victory_points.set(secret_victory_points);
                    }
                ServerMessage::SystemNote { text } => {
                    logging::log!("System note: {}", text);
                    self.messages.update(|m| m.push(text));
                }
                ServerMessage::Left {player_id, game_id} => {
                    logging::log!("Player {} has left the game {}", player_id, game_id);

                    // This arrives twice over: once to the player leaving, and
                    // once to everyone still at the table so their roster
                    // drops them. Only the first should tear down a session -
                    // clearing it for everybody would throw the whole table
                    // out when one person quit.
                    if Some(player_id) == self.player_id.get_untracked() {
                        self.is_in_game.set(false);
                        self.game_id.set(None);
                    }
                    let player_name = self.players.get_untracked()
                        .iter()
                        .find(|p| p.player_id == player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("Player {}", player_id));
                    self.players.update(|players| {
                        players.retain(|p| p.player_id != player_id);
                    });
                    self.messages.update(|m| m.push(format!(
                        "{} has left the game.",
                        player_name
                    )));
                }
            }
        }
    }

    /// Calculate the best trading ratio for a given resource based on player's ports
    pub fn get_best_ratio(&self, resource: shared::ResourceType) -> u8 {
        let ports = self.my_ports.get();
        let mut ratio = 4u8; // Default 4:1

        for port in &ports {
            match port {
                shared::PortType::ThreeToOne => {
                    ratio = ratio.min(3);
                }
                shared::PortType::TwoToOne(res) => {
                    // Check if this 2:1 port matches the resource
                    let matches = match (res, &resource) {
                        (shared::ResourceType::Brick, shared::ResourceType::Brick) => true,
                        (shared::ResourceType::Wood, shared::ResourceType::Wood) => true,
                        (shared::ResourceType::Sheep, shared::ResourceType::Sheep) => true,
                        (shared::ResourceType::Wheat, shared::ResourceType::Wheat) => true,
                        (shared::ResourceType::Ore, shared::ResourceType::Ore) => true,
                        _ => false,
                    };
                    if matches {
                        return 2; // 2:1 is the best possible
                    }
                }
            }
        }

        ratio
    }
}

/// Where the game socket lives: the same host and port the page came from.
///
/// It used to be `ws://127.0.0.1:8080`, which only works when the browser is
/// on the same machine as the server. Over a remote-development port forward
/// - or from anywhere else - `127.0.0.1` is the *viewer's* machine, so the
/// page loaded and then quietly failed to reach any game at all. Deriving it
/// from the page's own origin means one address to reach, and one port to
/// forward. `trunk serve` proxies `/ws` to the backend while developing; in
/// production the backend serves the page and the socket itself.
fn websocket_url() -> String {
    let fallback = "ws://127.0.0.1:8080/ws".to_string();
    let Some(location) = web_sys::window().map(|w| w.location()) else {
        return fallback;
    };
    let (Ok(protocol), Ok(host)) = (location.protocol(), location.host()) else {
        return fallback;
    };
    // A page served over TLS may only open a TLS socket.
    let scheme = if protocol == "https:" { "wss" } else { "ws" };
    format!("{scheme}://{host}/ws")
}

pub fn provide_game_state() {
    // The server decides who we are; all we may do is present a token it
    // issued us earlier. A missing or stale token simply gets a new identity.
    let saved_name = (|| {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        storage.get_item("catan_player_name").ok().flatten()
    })().unwrap_or_default();

    let saved_token = (|| {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        storage.get_item("catan_session_token").ok().flatten()
    })();
    let state = GameState {
        // Lobby state
        messages: create_rw_signal(Vec::new()),
        chat: create_rw_signal(Vec::new()),
        last_roll_event: create_rw_signal(None),
        last_steal_event: create_rw_signal(None),
        last_monopoly_event: create_rw_signal(None),
        turn_epoch: create_rw_signal(0),
        bank: create_rw_signal(shared::BankInfo::default()),
        is_in_game: create_rw_signal(false),
        lobby_games: create_rw_signal(Vec::new()),
        my_name: create_rw_signal(saved_name),
        ws_sender: create_rw_signal(None),
        connection_error: create_rw_signal(None),
        is_connecting: create_rw_signal(true),

        // Game state
        game_id: create_rw_signal(None),
        player_id: create_rw_signal(None),
        players: create_rw_signal(Vec::new()),
        current_turn_player: create_rw_signal(Uuid::nil()),
        my_resources: create_rw_signal(Resources::default()),
        game_phase: create_rw_signal(GamePhase::WaitingForPlayers),

        // Board state
        hexes: create_rw_signal(Vec::new()),
        settlements: create_rw_signal(Vec::new()),
        cities: create_rw_signal(Vec::new()),
        roads: create_rw_signal(Vec::new()),
        robber_pos: create_rw_signal(None),
        robber_asset: create_rw_signal("robber".to_string()),
        board_ports: create_rw_signal(Vec::new()),

        // UI state
        last_dice_roll: create_rw_signal(None),
        table_last_roll: create_rw_signal(None),
        build_mode: create_rw_signal(BuildMode::None),
        my_dev_cards: create_rw_signal(Vec::new()),
        fresh_dev_cards: create_rw_signal(Vec::new()),
        dev_card_played_this_turn: create_rw_signal(false),

        // Robber state
        must_discard_count: create_rw_signal(None),
        must_move_robber: create_rw_signal(false),
        must_steal_from_players: create_rw_signal(Vec::new()),
        waiting_for_discards: create_rw_signal(false),

        // Road Builder state
        free_roads_remaining: create_rw_signal(0),

        // Year of Plenty state
        year_of_plenty_pending: create_rw_signal(false),

        // Monopoly state
        monopoly_pending: create_rw_signal(false),

        // Port trading state
        my_ports: create_rw_signal(Vec::new()),

        // Player-to-player trading state
        incoming_trades: create_rw_signal(Vec::new()),
        my_offers: create_rw_signal(Vec::new()),
        my_accepted_offers: create_rw_signal(Vec::new()),
        my_counter_offers: create_rw_signal(Vec::new()),
        // Winner
        winner_player_id: create_rw_signal(None),
        secret_victory_points: create_rw_signal(0),
        game_stats: create_rw_signal(None),
    };

    let state_clone = state;
    spawn_local(async move {
        let ws_url = match saved_token {
            Some(token) => format!("{}?token={}", websocket_url(), token),
            None => websocket_url(),
        };
        logging::log!("Connecting to WebSocket at {}", ws_url);
        match WebSocket::open(&ws_url) {
            Ok(ws) => {
                logging::log!("WebSocket Connected!");
                let (mut write, mut read) = ws.split();
                let (tx, mut rx) = futures::channel::mpsc::unbounded::<Message>();

                state_clone.ws_sender.set(Some(tx));
                state_clone.is_connecting.set(false);
                state_clone.connection_error.set(None);

                state_clone.send(ClientRequest::GetLobbyList);

                spawn_local(async move {
                    while let Some(msg) = rx.next().await {
                        let _ = write.send(msg).await;
                    }
                });

                spawn_local(async move {
                    while let Some(msg) = read.next().await {
                        if let Ok(Message::Text(text)) = msg {
                            state_clone.process_message(text);
                        }
                    }
                    state_clone.connection_error.set(Some("Connection to server closed.".into()));
                    state_clone.ws_sender.set(None);
                });
            }
            Err(e) => {
                state_clone.is_connecting.set(false);
                state_clone.connection_error.set(Some(format!("Failed to connect: {:?}", e)));
            }
        }
    });

    provide_context(state);
}