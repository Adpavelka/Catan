use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_player_count() -> usize { 4 }

/// What is left in the bank. Public: everyone at a real table can see how
/// tall the resource piles are and how thick the development deck is.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct BankInfo {
    pub resources: Resources,
    pub dev_cards: usize,
}

/// An open trade offer as it looks to one player, for restoring the trade
/// panel after a reconnect. Carries only what that player is entitled to see:
/// acceptances are public, but who declined is not.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TradeSnapshot {
    pub offer_id: u64,
    pub proposer_id: Uuid,
    pub target_player_id: Option<Uuid>,
    pub offering: Resources,
    pub requesting: Resources,
    /// Players who have bid, oldest first.
    pub accepted_by: Vec<Uuid>,
    /// Wildcards on each side of the offer.
    #[serde(default)]
    pub offering_any: u8,
    #[serde(default)]
    pub requesting_any: u8,
    /// Seconds until the server withdraws the offer, so a reconnecting client
    /// resumes the countdown instead of restarting it.
    pub seconds_remaining: u64,
    /// Whether the player receiving this sync has already refused it.
    pub you_declined: bool,
    /// Set when this is a counter to another offer.
    #[serde(default)]
    pub counters: Option<u64>,
}

/// How long a player has to roll before the server rolls for them, and how
/// long a whole turn may run before the server ends it.
///
/// The server is the authority: a client that closes its tab must not be able
/// to stall the table. Clients read the same constants purely to draw a bar
/// that agrees with the deadline being enforced.
pub const TURN_AUTO_ROLL_SECS: u64 = 10;
pub const TURN_LIMIT_SECS: u64 = 60;

/// How long a trade offer stays open before the server withdraws it.
///
/// The server is the authority here; the client only counts down so the
/// players can see it coming. Both sides read this one constant so a UI that
/// says "expired" always agrees with a server that has actually expired it.
pub const TRADE_LIFETIME_SECS: u64 = 120;

/// Which physical board a game is played on.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BoardLayout {
    /// The base game: 19 hexes, one desert.
    Standard,
    /// The 5-6 player extension: 30 hexes, two deserts.
    Extended,
}

/// Everything about a game that depends on how many people are playing.
///
/// Two- to four-player games use the base set. Five and six use the larger
/// extension board with a deeper bank and development deck, because six
/// players competing over nineteen hexes is not a game. Two players race to a
/// higher target, since points come far quicker with nobody in the way.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct GameRules {
    pub player_count: usize,
    pub victory_points_to_win: u8,
    pub board: BoardLayout,
    /// How many cards of each resource the bank starts with.
    pub bank_per_resource: u32,
    pub knight_cards: usize,
    pub victory_point_cards: usize,
    pub road_building_cards: usize,
    pub monopoly_cards: usize,
    pub year_of_plenty_cards: usize,
    /// Harbours around the coast.
    pub port_count: usize,
}

impl GameRules {
    pub const MIN_PLAYERS: usize = 2;
    pub const MAX_PLAYERS: usize = 6;

    /// Every table size we support, for building a lobby-size picker.
    pub const SUPPORTED_PLAYER_COUNTS: [usize; 5] = [2, 3, 4, 5, 6];

    pub fn for_player_count(player_count: usize) -> Self {
        let player_count = player_count.clamp(Self::MIN_PLAYERS, Self::MAX_PLAYERS);

        // Five and six players need the extension: more land, more cards.
        let extended = player_count >= 5;

        Self {
            player_count,
            // A duel reaches ten points far too quickly to be interesting.
            victory_points_to_win: if player_count == 2 { 15 } else { 10 },
            board: if extended { BoardLayout::Extended } else { BoardLayout::Standard },
            bank_per_resource: if extended { 24 } else { 19 },
            knight_cards: if extended { 20 } else { 14 },
            victory_point_cards: if extended { 6 } else { 5 },
            road_building_cards: if extended { 3 } else { 2 },
            monopoly_cards: if extended { 3 } else { 2 },
            year_of_plenty_cards: 2,
            port_count: if extended { 11 } else { 9 },
        }
    }

    /// The 5-6 player game inserts a special building phase between turns, so
    /// a bigger table does not mean an unbearable wait between builds.
    pub fn uses_special_building(&self) -> bool {
        self.player_count >= 5
    }

    pub fn dev_card_total(&self) -> usize {
        self.knight_cards
            + self.victory_point_cards
            + self.road_building_cards
            + self.monopoly_cards
            + self.year_of_plenty_cards
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClientRequest {
    CreateGame {
        #[serde(default = "default_player_count")]
        player_count: usize,
        /// Displayed to the other players. Trimmed and length-capped by the
        /// server, which substitutes a default if it comes through blank.
        seat: SeatRequest,
    },
    JoinGame {
        game_id: String,
        seat: SeatRequest,
    },
    LeaveGame
    {
        game_id: String,
    },
    /// Keep-alive. Carries no game meaning.
    Ping,
    RollDice,
    EndTurn,
    BuildSettlement { x: i32, y: i32 },
    BuildCity { x: i32, y: i32 },
    BuildRoad { x1: i32, y1: i32 },
    BuyDevelopmentCard,
    PlayDevCard {
        card: DevCardType,
        target: Option<DevCardTarget>,
    },
    TradeOffer {
        /// If None, trade is open to all players. If Some, only that player can accept.
        target_player_id: Option<Uuid>,
        offer: Resources,
        request: Resources,
        /// Unspecified cards on each side - "any card". An offer holding one
        /// cannot be settled as it stands: the other player has to counter
        /// with something concrete in its place. See `TradeBasket`.
        #[serde(default)]
        offer_any: u8,
        #[serde(default)]
        request_any: u8,
    },
    /// Answer someone else's offer. `accept` registers willingness to trade -
    /// it does not move any resources. The proposer still has to pick you with
    /// `ConfirmTrade`. Sending `accept: false` afterwards withdraws again.
    TradeResponse {
        offer_id: u64,
        accept: bool,
    },
    /// Reply to the active player's offer with terms of your own. The counter
    /// is a trade from you to them; they settle it, the same as any other.
    /// Countering withdraws any acceptance you had on the original.
    CounterOffer {
        /// The offer being countered.
        offer_id: u64,
        offer: Resources,
        request: Resources,
    },
    // A counter is always concrete: naming the cards is the whole point of
    // countering a wildcard, so `CounterOffer` deliberately has no `any`.
    /// Settle a trade. Only the player whose turn it is may do this, which is
    /// what keeps every trade between them and one other player: for their own
    /// offer they pick one of the accepters, for a counter they pick the
    /// player who countered.
    ConfirmTrade {
        offer_id: u64,
        partner_id: Uuid,
    },
    CancelTrade {
        offer_id: u64,
    },
    Chat {
        message: String,
    },
    GetLobbyList,
    /// Start a game that has reached the minimum player count but is not full.
    StartGame {
        game_id: String,
    },
    MoveRobber {
        q: i32,
        r: i32,
    },
    StealFromPlayer {
        victim_id: Uuid,
    },
    DiscardCards {
        resources: Resources,
    },
    BankTrade {
        give: ResourceType,
        receive: ResourceType,
    },
    YearOfPlentyChoice {
        resource1: ResourceType,
        resource2: ResourceType,
    },
    MonopolyChoice {
        resource: ResourceType,
    },
}

#[derive(Debug, Serialize, Clone, Deserialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServerMessage {
    /// Sent immediately on connect. `token` is the client's credential: it is
    /// issued by the server, and is the only thing that proves which player
    /// this connection is. Clients must persist it and send it back on
    /// reconnect. `player_id` is public and proves nothing on its own.
    Session {
        player_id: Uuid,
        token: Uuid,
    },
    Joined {
        player_id: Uuid,
        game_id: String,
    },
    GameStarted {
        your_player_id: Uuid,
        players: Vec<PlayerInfo>,
        board: BoardState,
        game_phase: GamePhase,
        /// Your own hand. Never carries another player's cards.
        your_resources: Resources,
        your_dev_cards: Vec<DevCardType>,
    },
    /// Update all players' info (victory points, dev cards, etc)
    PlayersUpdate {
        players: Vec<PlayerInfo>,
    },
    /// What the bank has left, sent whenever resources move.
    BankUpdate {
        bank: BankInfo,
    },
    LobbyUpdate {
        games: Vec<LobbyGameInfo>,
    },
    DiceRolled {
        player_id: Uuid,
        dice_1: u8,
        dice_2: u8,
        /// Number of players who must discard (only set when 7 is rolled)
        #[serde(default)]
        discards_pending: usize,
    },
    ResourceUpdate {
        player_id: Uuid,
        resources: Resources,
    },
    Built {
        player_id: Uuid,
        #[serde(rename = "type")]
        structure_type: StructureType,
        coords: (i32, i32),
    },
    NextTurn {
        player_id: Uuid,
    },
    PhaseChanged {
        new_phase: GamePhase,
    },
    ChatMessage {
        player_id: Uuid,
        text: String,
    },
    /// Public: somebody bought a development card. Which card it was is sent
    /// only to the buyer, as `DevCardDrawn`.
    DevCardBought {
        player_id: Uuid,
    },
    /// Private to the buyer: the card they just drew.
    DevCardDrawn {
        card_type: DevCardType,
    },
    DevCardPlayed {
        player_id: Uuid,
        card_type: DevCardType,
    },
    Error {
        message: String,
    },
    RobberMoved {
        player_id: Uuid,
        new_q: i32,
        new_r: i32,
    },
    PlayerRobbed {
        thief_id: Uuid,
        victim_id: Uuid,
        /// Which card moved. Only filled in for the thief and the victim -
        /// everyone else is told a card was taken, not which one, the same
        /// way hands are hidden in `PlayerInfo`.
        resource: Option<ResourceType>,
        /// Whether anything was taken at all. Distinguishes "the victim had
        /// nothing" from "you are not allowed to see what it was".
        #[serde(default)]
        stole_a_card: bool,
    },
    MustDiscardCards {
        player_id: Uuid,
        count: usize,
    },
    CardsDiscarded {
        player_id: Uuid,
        count: usize,
    },
    MustMoveRobber {
        player_id: Uuid,
    },
    MustPlaceRoads {
        player_id: Uuid,
        roads_remaining: u8,
    },
    MustChooseYearOfPlentyResources {
        player_id: Uuid,
    },
    YearOfPlentyResourcesReceived {
        player_id: Uuid,
        resource1: ResourceType,
        resource2: ResourceType,
    },
    MustChooseMonopolyResource {
        player_id: Uuid,
    },
    MonopolyResourcesStolen {
        player_id: Uuid,
        resource: ResourceType,
        total_stolen: u32,
    },
    CanRobPlayers {
        player_ids: Vec<Uuid>,
    },
    BankTradeCompleted {
        player_id: Uuid,
        gave: ResourceType,
        received: ResourceType,
    },
    PortsUpdate {
        player_id: Uuid,
        ports: Vec<PortType>,
    },
    /// A player has proposed a trade
    TradeProposed {
        offer_id: u64,
        proposer_id: Uuid,
        /// If Some, only this player can accept
        target_player_id: Option<Uuid>,
        offering: Resources,
        requesting: Resources,
        /// Wildcards on each side. A trade with any of these can only be
        /// answered with a counter, never accepted as it stands.
        #[serde(default)]
        offering_any: u8,
        #[serde(default)]
        requesting_any: u8,
        /// Set when this is a counter to an earlier offer, so clients can show
        /// it against the offer it answers.
        #[serde(default)]
        counters: Option<u64>,
    },
    /// Someone is willing to take the offer. Nothing has moved yet: the
    /// proposer picks one of these players with `ConfirmTrade` to settle.
    TradeAccepted {
        offer_id: u64,
        accepter_id: Uuid,
    },
    /// An earlier acceptance was taken back, so this player is no longer a
    /// candidate for the offer.
    TradeAcceptanceWithdrawn {
        offer_id: u64,
        accepter_id: Uuid,
    },
    /// A trade was settled and the resources have moved.
    TradeCompleted {
        offer_id: u64,
        proposer_id: Uuid,
        accepter_id: Uuid,
        /// What the proposer gave
        proposer_gave: Resources,
        /// What the accepter gave
        accepter_gave: Resources,
    },
    /// A trade offer is no longer open - withdrawn by the proposer, expired,
    /// or dropped because the proposer can no longer cover it.
    TradeCancelled {
        offer_id: u64,
    },
    /// A trade offer was declined. Only sent to the proposer and the decliner,
    /// so the rest of the table cannot read who is refusing what.
    TradeDeclined {
        offer_id: u64,
        proposer_id: Uuid,
        decliner_id: Uuid,
    },
    FullStateSync {
        player_id: Uuid,
        players: Vec<PlayerInfo>,
        board: BoardInfo,
        game_phase: GamePhase,
        current_turn_player_id: Uuid,
        robber_pos: (i32, i32),
        last_dice_roll: Option<(u8, u8)>,
        /// Your own hand. Never carries another player's cards.
        your_resources: Resources,
        your_dev_cards: Vec<DevCardType>,
        /// Offers still open that you can see: your own, and any you may
        /// answer. Without these a reconnect leaves a live negotiation
        /// invisible until it expires.
        #[serde(default)]
        pending_trades: Vec<TradeSnapshot>,
        #[serde(default)]
        bank: BankInfo,
    },
    /// The game is over. Carries the finished statistics with it: the end
    /// screen has to show counters nobody could reconstruct from the message
    /// stream, and a second message would let the screen open without them.
    PlayerWon {
        player_id: Uuid,
        secret_victory_points: u8,
        #[serde(default)]
        stats: GameStats,
    },
    PlayerSecretVictoryPointsUpdated{secret_victory_points: i32},

    Left { player_id: Uuid, game_id: String },
}

/// What was just built. An enum rather than a string: the initial-placement
/// state machine branches on this, and a typo there would silently break setup.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StructureType {
    Settlement,
    City,
    Road,
}

impl StructureType {
    pub fn label(&self) -> &'static str {
        match self {
            StructureType::Settlement => "settlement",
            StructureType::City => "city",
            StructureType::Road => "road",
        }
    }
}

/// How a player wants to be seated: the name they picked and the colour they
/// chose from the palette. The server has the final say on both.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SeatRequest {
    pub name: String,
    pub colour: PlayerColour,
}

/// The player palette. Deliberately larger than the number of seats, so a
/// joining player has a real choice rather than being handed the last colour
/// left over. Hues are kept well apart from each other; the board's own greens,
/// yellows and oranges are handled by the black outline on every building.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayerColour {
    Blue,
    Red,
    Green,
    Yellow,
    Purple,
    Cyan,
    Pink,
    White,
}

impl PlayerColour {
    pub const ALL: [PlayerColour; 8] = [
        PlayerColour::Blue,
        PlayerColour::Red,
        PlayerColour::Green,
        PlayerColour::Yellow,
        PlayerColour::Purple,
        PlayerColour::Cyan,
        PlayerColour::Pink,
        PlayerColour::White,
    ];

    /// Longest name the server will keep; anything more is truncated.
    pub const MAX_NAME_LEN: usize = 16;

    pub fn label(&self) -> &'static str {
        match self {
            PlayerColour::Blue => "Blue",
            PlayerColour::Red => "Red",
            PlayerColour::Green => "Green",
            PlayerColour::Yellow => "Yellow",
            PlayerColour::Purple => "Purple",
            PlayerColour::Cyan => "Cyan",
            PlayerColour::Pink => "Pink",
            PlayerColour::White => "White",
        }
    }

    /// The one place a player colour is turned into pixels. Used directly as
    /// an SVG `fill`, so it does not depend on a CSS class being generated.
    pub fn hex(&self) -> &'static str {
        match self {
            PlayerColour::Blue => "#3b82f6",
            PlayerColour::Red => "#ef4444",
            PlayerColour::Green => "#22c55e",
            PlayerColour::Yellow => "#eab308",
            PlayerColour::Purple => "#a855f7",
            PlayerColour::Cyan => "#06b6d4",
            PlayerColour::Pink => "#ec4899",
            PlayerColour::White => "#e2e8f0",
        }
    }

    /// A default name, used when a player submits a blank one.
    pub fn default_name(&self) -> &'static str {
        match self {
            PlayerColour::Blue => "Steve",
            PlayerColour::Red => "Bob",
            PlayerColour::Green => "Kevin",
            PlayerColour::Yellow => "George",
            PlayerColour::Purple => "Wanda",
            PlayerColour::Cyan => "Cyrus",
            PlayerColour::Pink => "Rosa",
            PlayerColour::White => "Pearl",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DevCardType {
    Knight,
    VictoryPoint,
    RoadBuilding,
    Monopoly,
    YearOfPlenty,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DevCardTarget {
    pub hex_index: Option<usize>,
    pub resource_type: Option<ResourceType>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceType {
    Brick,
    Wood,
    Wheat,
    Ore,
    Sheep,
    Desert
}

impl ResourceType {
    /// The five resources a hand can hold, in the order the interface lists
    /// them everywhere else - the bank strip, the card tray, the statistics.
    /// The desert is not among them: it produces nothing, so no card exists.
    pub const CARDS: [ResourceType; 5] = [
        ResourceType::Wood,
        ResourceType::Brick,
        ResourceType::Sheep,
        ResourceType::Wheat,
        ResourceType::Ore,
    ];

    /// This resource's slot in a `[_; 5]` tally, or `None` for the desert.
    pub fn card_index(&self) -> Option<usize> {
        Self::CARDS.iter().position(|kind| kind == self)
    }

    /// The name as a label, capitalised. `resource_label` in the client is
    /// the same word for running prose, in lower case.
    pub fn label(&self) -> &'static str {
        match self {
            ResourceType::Wood => "Wood",
            ResourceType::Brick => "Brick",
            ResourceType::Sheep => "Sheep",
            ResourceType::Wheat => "Wheat",
            ResourceType::Ore => "Ore",
            ResourceType::Desert => "Desert",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct Resources {
    pub brick: u8,
    pub lumber: u8,
    pub wool: u8,
    pub grain: u8,
    pub ore: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GamePhase {
    WaitingForPlayers,

    InitialPlacement {
        round: InitialRound,
        step: PlacementStep,
    },

    RegularPlay,

    /// The 5-6 player special building phase: between turns, each other player
    /// in turn order gets one chance to build or buy. `builder` is the only
    /// player who may act, and they may only build - no rolling, trading or
    /// playing development cards.
    SpecialBuilding {
        builder: Uuid,
    },
}

impl GamePhase {
    pub fn is_initial_phase(&self) -> bool {
        matches!(self, GamePhase::InitialPlacement { .. })
    }

    /// Phases in which buying and building are allowed at all.
    pub fn allows_building(&self) -> bool {
        matches!(self, GamePhase::RegularPlay | GamePhase::SpecialBuilding { .. })
    }

    pub fn special_builder(&self) -> Option<Uuid> {
        match self {
            GamePhase::SpecialBuilding { builder } => Some(*builder),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum InitialRound {
    First,
    Second,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum PlacementStep {
    BuildSettlement,
    BuildRoad {
        settlement: (i32, i32),
    },
}



#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAction {
    Discard,                    // player must discard cards
    MoveRobber,                  // player must move robber
    PlayKnight,                  // player played knight, needs to move robber
    RoadBuilding { remaining: u8 }, // player has free roads
    YearOfPlenty,                // player must pick 2 resources
    Monopoly,                    // player must pick a resource type
    Steal,                       // player moved the robber and may rob one victim
}



/// Port types for maritime trading
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PortType {
    /// Generic 3:1 port - trade 3 of any resource for 1 of any other
    ThreeToOne,
    /// Specific 2:1 port - trade 2 of specific resource for 1 of any other
    TwoToOne(ResourceType),
}

/// Port info with vertex coordinates and type (for board state)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PortInfo {
    /// Two vertex coordinates that define the port edge
    pub vertices: [(i32, i32); 2],
    /// The type of port (3:1 or 2:1 for specific resource)
    pub port_type: PortType,
}

/// A game as advertised in the lobby list, including which colours are still
/// free so a joining player can be offered a real choice.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LobbyGameInfo {
    pub game_id: String,
    pub players: usize,
    pub max_players: usize,
    pub available_colours: Vec<PlayerColour>,
    pub victory_points_to_win: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BoardInfo{
    pub hexes: Vec<HexInfo>,
    pub settlements: Vec<BuildingInfo>,
    pub cities: Vec<BuildingInfo>,
    pub roads: Vec<BuildingInfo>,
    pub robber_pos: (i32, i32),
    pub ports: Vec<PortInfo>
}

/// What every player is allowed to know about another player.
///
/// Hands are hidden information in Catan, so this deliberately carries only
/// *counts* of resource and development cards. The cards themselves are sent
/// to their owner alone, via `ResourceUpdate` / `DevCardDrawn` / the
/// `your_resources` and `your_dev_cards` fields of the state messages.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    pub player_id: Uuid,
    pub name: String,
    pub colour: PlayerColour,
    pub victory_points: u8,
    /// Ports this player has access to (from settlements/cities on port vertices)
    pub ports: Vec<PortType>,
    /// How many resource cards they hold - never which ones.
    pub resource_count: u8,
    /// How many development cards they hold - never which ones.
    pub dev_card_count: usize,
    pub knights_played: usize,
    pub roads_count: usize,
    pub has_longest_road: bool,
    pub has_largest_army: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BoardState {
    pub hexes: Vec<HexInfo>,
    pub settlements: Vec<BuildingInfo>,
    pub cities: Vec<BuildingInfo>,
    pub roads: Vec<BuildingInfo>,
    pub robber_pos: (i32, i32),
    /// Ports on the board with their vertex coordinates and types
    pub ports: Vec<PortInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct HexInfo {
    pub q: i32,
    pub r: i32,
    pub resource: ResourceType,
    pub number: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BuildingInfo {
    pub player_id: Uuid,
    pub x: i32,
    pub y: i32,
}

// ------------------------------------------------------------- statistics
//
// What the end-of-game screen reads. The server keeps the counters while the
// game runs and sends the finished tally once, with `PlayerWon`: a client that
// tried to derive these from the message stream would get a different answer
// depending on when it connected, and would see nothing at all of the hands it
// is not entitled to watch.

/// How many distinct totals two dice can show: 2 through 12.
pub const DICE_TOTALS: usize = 11;

/// The index a dice total occupies in a `[_; DICE_TOTALS]` tally, or `None`
/// for a total two dice cannot produce.
pub fn dice_index(total: u8) -> Option<usize> {
    (2..=12).contains(&total).then(|| total as usize - 2)
}

/// The dice total a tally index stands for. The inverse of `dice_index`.
pub fn dice_total(index: usize) -> u8 {
    index as u8 + 2
}

/// Where a player's resource cards came from, and where they went.
///
/// Every card that reaches a hand is counted once in `gained_total` and once
/// in whichever source bucket applies, so the buckets are a breakdown of the
/// total rather than a separate reckoning. `spent` has no matching bucket on
/// the resource page - paying the bank for a building is an *activity*, not a
/// loss to another player - but it is still part of `lost_total`.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceFlow {
    pub gained_total: u32,
    pub lost_total: u32,

    pub gained_by_rolling: u32,
    pub gained_by_trading: u32,
    pub gained_by_dev_cards: u32,
    pub gained_by_robbing: u32,

    pub lost_by_trading: u32,
    pub lost_by_dev_cards: u32,
    pub lost_by_robber: u32,
    pub lost_by_seven: u32,

    /// Handed back to the bank to pay for a building or a development card.
    pub spent: u32,
}

impl ResourceFlow {
    /// Cards still in hand: everything that came in, less everything that went
    /// out. Signed only so a bug shows up as a negative number rather than
    /// wrapping to four billion.
    pub fn score(&self) -> i64 {
        self.gained_total as i64 - self.lost_total as i64
    }
}

/// A count of development cards, split by kind.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct DevCardTally {
    pub knight: u32,
    pub victory_point: u32,
    pub road_building: u32,
    pub monopoly: u32,
    pub year_of_plenty: u32,
}

impl DevCardTally {
    /// The kinds in the order the UI lists them.
    pub const KINDS: [DevCardType; 5] = [
        DevCardType::Knight,
        DevCardType::VictoryPoint,
        DevCardType::RoadBuilding,
        DevCardType::Monopoly,
        DevCardType::YearOfPlenty,
    ];

    pub fn get(&self, kind: &DevCardType) -> u32 {
        match kind {
            DevCardType::Knight => self.knight,
            DevCardType::VictoryPoint => self.victory_point,
            DevCardType::RoadBuilding => self.road_building,
            DevCardType::Monopoly => self.monopoly,
            DevCardType::YearOfPlenty => self.year_of_plenty,
        }
    }

    pub fn add(&mut self, kind: &DevCardType, n: u32) {
        match kind {
            DevCardType::Knight => self.knight += n,
            DevCardType::VictoryPoint => self.victory_point += n,
            DevCardType::RoadBuilding => self.road_building += n,
            DevCardType::Monopoly => self.monopoly += n,
            DevCardType::YearOfPlenty => self.year_of_plenty += n,
        }
    }

    pub fn total(&self) -> u32 {
        self.knight
            + self.victory_point
            + self.road_building
            + self.monopoly
            + self.year_of_plenty
    }
}

/// Counters for the activity page. Collected as the game runs so that page can
/// be built later without replaying anything.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActivityTally {
    pub dev_cards_bought: u32,
    pub dev_cards_played: u32,
    pub trades_proposed: u32,
    pub trades_completed: u32,
    /// Cards paid to the bank for buildings and development cards.
    pub resources_spent: u32,
    /// Cards a roll would have paid this player, but for the robber standing
    /// on the tile.
    pub resources_blocked_by_robber: u32,
}

/// Where a player's victory points came from. Sums to `victory_points`.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct PointBreakdown {
    pub settlements: u8,
    /// Two per city: the point the settlement already carried, and the one the
    /// upgrade added.
    pub cities: u8,
    pub dev_cards: u8,
    pub longest_road: u8,
    pub largest_army: u8,
}

impl PointBreakdown {
    pub fn total(&self) -> u8 {
        self.settlements + self.cities + self.dev_cards + self.longest_road + self.largest_army
    }
}

/// Everything the statistics screen knows about one player.
///
/// Carries the player's own name and colour so the screen keeps working after
/// somebody leaves the table: the roster shrinks, this snapshot does not.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PlayerStats {
    pub player_id: Uuid,
    pub name: String,
    pub colour: PlayerColour,

    pub victory_points: u8,
    pub points: PointBreakdown,

    /// This player's own rolls, indexed 2..=12 by `dice_index`.
    pub dice_rolls: [u32; DICE_TOTALS],

    pub resources: ResourceFlow,
    pub dev_cards_bought: DevCardTally,
    pub dev_cards_played: DevCardTally,
    pub activity: ActivityTally,

    pub settlements_built: u32,
    pub cities_built: u32,
    pub roads_built: u32,
    pub knights_played: u32,
    /// The longest unbroken run of their roads, whether or not it won the card.
    pub longest_road_length: u32,
}

impl PlayerStats {
    pub fn rolls_made(&self) -> u32 {
        self.dice_rolls.iter().sum()
    }
}

/// The finished tally for one game.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct GameStats {
    /// Wall clock from the first placement to the winning move.
    pub duration_secs: u64,
    /// Turns played after setup.
    pub turns: u64,
    pub winner: Option<Uuid>,
    pub victory_points_to_win: u8,
    /// Every roll at the table, indexed 2..=12 by `dice_index`.
    pub dice_rolls: [u32; DICE_TOTALS],
    /// Cards drawn from the bank over the whole game, by resource, indexed by
    /// `ResourceType::card_index`. Production and the bank's side of a trade:
    /// what the island actually turned out. Cards changing hands between
    /// players are not draws - they were drawn once already.
    #[serde(default)]
    pub resource_draws: [u32; 5],
    /// In seat order, so every page lists players the same way.
    pub players: Vec<PlayerStats>,
}

impl GameStats {
    pub fn total_rolls(&self) -> u32 {
        self.dice_rolls.iter().sum()
    }

    /// How many of one resource were drawn all game.
    pub fn draws_of(&self, resource: ResourceType) -> u32 {
        resource
            .card_index()
            .map_or(0, |index| self.resource_draws[index])
    }

    /// The duration as `mm:ss`, counting past sixty minutes rather than
    /// wrapping - a game that ran for two hours should say so.
    pub fn duration_label(&self) -> String {
        format!("{:02}:{:02}", self.duration_secs / 60, self.duration_secs % 60)
    }

    pub fn player(&self, id: Uuid) -> Option<&PlayerStats> {
        self.players.iter().find(|p| p.player_id == id)
    }
}