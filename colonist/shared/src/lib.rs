use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_player_count() -> usize { 4 }

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
    },
    TradeResponse {
        offer_id: u64,
        accept: bool,
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
        resource: Option<ResourceType>,
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
    },
    /// A trade was completed successfully
    TradeCompleted {
        offer_id: u64,
        proposer_id: Uuid,
        accepter_id: Uuid,
        /// What the proposer gave
        proposer_gave: Resources,
        /// What the accepter gave
        accepter_gave: Resources,
    },
    /// A trade offer was cancelled by the proposer
    TradeCancelled {
        offer_id: u64,
    },
    /// A trade offer was declined (only sent to proposer)
    TradeDeclined {
        offer_id: u64,
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
    },
    PlayerWon { player_id: Uuid, secret_victory_points: u8 },
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
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
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