//! The bottom HUD: the card tray, the big controls, whose turn it is, and the
//! dice.
//!
//! It is one continuous bar along the foot of the table - a cream tray holding
//! the hand on the left, six chunky controls butted up against it on the right
//! - with the turn panel and the dice floating over the water above them.
//!
//! Trading is not on the bar itself. It lives behind the trade control, in a
//! panel that opens over the tray, because the tray is where the cards you
//! would put up are.

use leptos::*;

use crate::components::board::DiceTray;
use crate::components::icons::{dev_card_art, resource_art, Art};
use crate::state::{BuildMode, GameState};
use shared::{ClientRequest, DevCardType, GamePhase, ResourceType};

/// How many of each piece a player starts with. Used for the counts in the
/// build buttons' corners.
const MAX_ROADS: usize = 15;
const MAX_SETTLEMENTS: usize = 5;
const MAX_CITIES: usize = 4;

// ---------------------------------------------------------------- resources

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Res {
    Wood,
    Brick,
    Sheep,
    Wheat,
    Ore,
}

impl Res {
    pub const ALL: [Res; 5] = [Res::Wood, Res::Brick, Res::Sheep, Res::Wheat, Res::Ore];

    pub fn label(self) -> &'static str {
        match self {
            Res::Wood => "Wood",
            Res::Brick => "Brick",
            Res::Sheep => "Sheep",
            Res::Wheat => "Wheat",
            Res::Ore => "Ore",
        }
    }

    pub fn shared(self) -> ResourceType {
        match self {
            Res::Wood => ResourceType::Wood,
            Res::Brick => ResourceType::Brick,
            Res::Sheep => ResourceType::Sheep,
            Res::Wheat => ResourceType::Wheat,
            Res::Ore => ResourceType::Ore,
        }
    }

    pub fn art(self) -> &'static str {
        resource_art(self.shared())
    }

    pub fn in_hand(self, r: &shared::Resources) -> u8 {
        match self {
            Res::Wood => r.lumber,
            Res::Brick => r.brick,
            Res::Sheep => r.wool,
            Res::Wheat => r.grain,
            Res::Ore => r.ore,
        }
    }
}

/// A pile of resources on one side of a trade.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Basket {
    pub wood: u8,
    pub brick: u8,
    pub sheep: u8,
    pub wheat: u8,
    pub ore: u8,
}

impl Basket {
    pub fn total(&self) -> u8 {
        self.wood + self.brick + self.sheep + self.wheat + self.ore
    }
    pub fn get(&self, k: Res) -> u8 {
        match k {
            Res::Wood => self.wood,
            Res::Brick => self.brick,
            Res::Sheep => self.sheep,
            Res::Wheat => self.wheat,
            Res::Ore => self.ore,
        }
    }
    pub fn set(&mut self, k: Res, v: u8) {
        match k {
            Res::Wood => self.wood = v,
            Res::Brick => self.brick = v,
            Res::Sheep => self.sheep = v,
            Res::Wheat => self.wheat = v,
            Res::Ore => self.ore = v,
        }
    }
    pub fn to_shared(self) -> shared::Resources {
        shared::Resources {
            brick: self.brick,
            lumber: self.wood,
            wool: self.sheep,
            grain: self.wheat,
            ore: self.ore,
        }
    }
    /// Every card in the pile, one entry per card, for laying them out.
    pub fn spread(&self) -> Vec<Res> {
        Res::ALL
            .into_iter()
            .flat_map(|k| std::iter::repeat(k).take(self.get(k) as usize))
            .collect()
    }
}

/// The trade being assembled, plus whether its panel is open. Shared through
/// context because a card clicked in the tray has to land in the panel.
#[derive(Clone, Copy)]
pub struct TradeDraft {
    pub receive: RwSignal<Basket>,
    pub offer: RwSignal<Basket>,
    pub receive_any: RwSignal<u8>,
    pub offer_any: RwSignal<u8>,
    pub open: RwSignal<bool>,
}

impl TradeDraft {
    fn new() -> Self {
        Self {
            receive: create_rw_signal(Basket::default()),
            offer: create_rw_signal(Basket::default()),
            receive_any: create_rw_signal(0),
            offer_any: create_rw_signal(0),
            open: create_rw_signal(false),
        }
    }

    pub fn clear(&self) {
        self.receive.set(Basket::default());
        self.offer.set(Basket::default());
        self.receive_any.set(0);
        self.offer_any.set(0);
    }

    pub fn is_empty(&self) -> bool {
        self.receive.get().total()
            + self.offer.get().total()
            + self.receive_any.get()
            + self.offer_any.get()
            == 0
    }
}

// ------------------------------------------------------------------- cards

/// One physical card: thick dark edge, the artwork filling the face, and the
/// count folded over the top-right corner so it never covers the picture.
#[component]
pub fn CardFace(
    art: &'static str,
    alt: &'static str,
    /// Shown as a badge when above one.
    #[prop(into, default = 0.into())]
    count: MaybeSignal<u8>,
    #[prop(into, default = false.into())] dimmed: MaybeSignal<bool>,
    #[prop(into, default = false.into())] selected: MaybeSignal<bool>,
    #[prop(default = "w-[63px] h-[88px]")] size: &'static str,
) -> impl IntoView {
    let show_badge = move || count.get() > 0;

    view! {
        <div
            class=move || format!(
                "relative rounded-md overflow-visible game-card {size} {} {}",
                if dimmed.get() { "game-card-off" } else { "" },
                if selected.get() { "ring-[3px] ring-[#ffd44d] -translate-y-0.5" } else { "" },
            )
            title=alt
        >
            <Art name=art alt=alt class="w-full h-full object-cover rounded-[4px]" />
            <Show when=show_badge>
                <span class="card-badge absolute -top-1 -right-1 min-w-[23px] h-[26px] px-1
                             text-[17px] font-black leading-none
                             flex items-center justify-center tabular-nums">
                    {move || count.get()}
                </span>
            </Show>
        </div>
    }
}

/// The unknown card. Deliberately unlike a resource: a blue back with a big
/// question mark, so it never reads as another commodity.
#[component]
pub fn MysteryCard(
    #[prop(into, default = 0.into())] count: MaybeSignal<u8>,
    #[prop(default = "w-[63px] h-[88px]")] size: &'static str,
    #[prop(default = "Any card - they choose which")] alt: &'static str,
) -> impl IntoView {
    let show_badge = move || count.get() > 0;

    view! {
        <div
            class=format!("relative rounded-md game-card flex items-center justify-center {size}")
            style="background: radial-gradient(circle at 50% 35%, #4fd0f2 0%, #159de1 45%, #0668a8 100%);"
            title=alt
        >
            <span class="text-[38px] leading-none font-black text-white ink">"?"</span>
            <Show when=show_badge>
                <span class="card-badge absolute -top-1 -right-1 min-w-[23px] h-[26px] px-1
                             text-[17px] font-black leading-none
                             flex items-center justify-center tabular-nums">
                    {move || count.get()}
                </span>
            </Show>
        </div>
    }
}

// --------------------------------------------------------------- the layer

#[component]
pub fn BottomLayer() -> impl IntoView {
    let draft = TradeDraft::new();
    provide_context(draft);

    view! {
        // Breaks out of the column's side padding so the tray runs to the
        // edges. The floor clearance stays: the controls stand proud of the
        // surface and their bevel needs somewhere to fall.
        <div class="shrink-0 relative -mx-2 pb-1.5">

            // Floats over the water above the controls: dice, then the turn
            // panel, both hugging the right edge above the button row.
            <div class="absolute right-3 bottom-[126px] z-20 flex flex-col items-end gap-[38px] pointer-events-none">
                <div class="pointer-events-auto"><DiceTray /></div>
                <div class="pointer-events-auto"><TurnRow /></div>
            </div>

            // The trade table, over the tray, because that is where the cards
            // you would put up already are.
            <Show when=move || draft.open.get()>
                <TradePanel />
            </Show>

            <div class="flex items-end gap-[7px]">
                <HandTray />
                <ActionBar />
            </div>
        </div>
    }
}

// ----------------------------------------------------------------- the tray

/// The player's own cards, in a wide tray along the bottom-left. Duplicates
/// stack so a fat hand looks like a fat hand, and the tray stays wide and
/// mostly empty because a hand grows into it.
#[component]
fn HandTray() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let draft = use_context::<TradeDraft>().expect("TradeDraft missing");

    // What is left after whatever is already committed to the offer.
    let spare = move |k: Res| {
        k.in_hand(&state.my_resources.get())
            .saturating_sub(draft.offer.get().get(k))
    };

    let dev_cards = move || state.my_dev_cards.get();
    // Numbered outside the view macro: a turbofish inside it parses as tags.
    let dev_cards_keyed = move || {
        dev_cards()
            .into_iter()
            .enumerate()
            .collect::<Vec<(usize, DevCardType)>>()
    };
    let has_dev_cards = move || !dev_cards().is_empty();
    let hand_is_empty =
        move || state.my_resources.get() == shared::Resources::default() && dev_cards().is_empty();

    view! {
        <div
            class="hud-tray flex-1 min-w-0 flex items-center gap-3 px-3.5 overflow-x-auto custom-scrollbar"
            style="height: 119px;"
        >
            {Res::ALL.map(|k| {
                let held = move || k.in_hand(&state.my_resources.get());
                let show = move || held() > 0;
                // Duplicates fan out to the right; past four the stack stops
                // growing and the badge carries the rest.
                let fan = move || held().min(4) as i32;
                let width = move || 63 + 12 * (fan() - 1).max(0);

                view! {
                    <Show when=show>
                        <button
                            class="relative shrink-0 game-card-pick self-center"
                            style=move || format!("width: {}px; height: 88px;", width())
                            title=move || format!("{} {} - click to put one up for trade", held(), k.label())
                            on:click=move |_| {
                                if spare(k) > 0 {
                                    draft.open.set(true);
                                    draft.offer.update(|b| b.set(k, b.get(k) + 1));
                                }
                            }
                            on:contextmenu=move |ev| {
                                ev.prevent_default();
                                draft.offer.update(|b| b.set(k, b.get(k).saturating_sub(1)));
                            }
                        >
                            {move || (0..fan()).map(|i| view! {
                                <div
                                    class="absolute top-0"
                                    style=format!("left: {}px; z-index: {}", i * 12, i)
                                >
                                    <CardFace
                                        art=k.art()
                                        alt=k.label()
                                        dimmed=Signal::derive(move || spare(k) == 0)
                                    />
                                </div>
                            }).collect_view()}

                            // One badge for the whole stack, on the last card.
                            <span
                                class="card-badge absolute -top-1 z-20 min-w-[23px] h-[26px] px-1
                                       text-[17px] font-black leading-none
                                       flex items-center justify-center tabular-nums"
                                style=move || format!("left: {}px", width() - 22)
                            >
                                {held}
                            </span>
                        </button>
                    </Show>
                }
            }).to_vec()}

            // Development cards sit at the end of the hand, as they do on a
            // real table, and are played from here.
            <Show when=has_dev_cards>
                <div class="flex items-center gap-2 pl-3 ml-1 border-l-2 border-[#ddd2ba] shrink-0 self-center">
                    <For
                        each=dev_cards_keyed
                        key=|(i, c)| (*i, format!("{c:?}"))
                        children=move |(_, card)| view! { <DevCardInHand card=card /> }
                    />
                </div>
            </Show>

            <Show when=hand_is_empty>
                <span class="text-[13px] italic text-[#a89e8b] pl-1">"No cards in hand."</span>
            </Show>
        </div>
    }
}

/// One development card in hand, playable or not.
#[component]
fn DevCardInHand(card: DevCardType) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let is_point = card == DevCardType::VictoryPoint;
    // Signals rather than closures: these are read from several places, and a
    // closure that captures is not Copy.
    let for_fresh = card.clone();
    let fresh = Signal::derive(move || state.fresh_dev_cards.get().contains(&for_fresh));
    let my_turn = Signal::derive(move || {
        state.player_id.get() == Some(state.current_turn_player.get())
    });
    let playable = Signal::derive(move || {
        !is_point
            && !fresh.get()
            && my_turn.get()
            && !state.dev_card_played_this_turn.get()
            && state.game_phase.get() == GamePhase::RegularPlay
    });

    let label = crate::state::card_label(&card);
    let to_play = card.clone();

    view! {
        <button
            class=move || format!(
                "relative rounded-md overflow-hidden game-card w-[68px] h-[88px] shrink-0 {}",
                if is_point { "ring-2 ring-[#ffd44d]" }
                else if playable.get() { "game-card-pick ring-1 ring-[#8e45d5]" }
                else { "game-card-off" }
            )
            title=move || if is_point { "Counts towards victory".to_string() }
                else if fresh.get() { "Drawn this turn - playable next turn".to_string() }
                else if !my_turn.get() { "Playable on your turn".to_string() }
                else if state.dev_card_played_this_turn.get() { "One development card per turn".to_string() }
                else { format!("Play {label}") }
            disabled=move || !playable.get()
            on:click=move |_| {
                state.send(ClientRequest::PlayDevCard { card: to_play.clone(), target: None });
            }
        >
            <Art name=dev_card_art(&card) alt=label class="w-full h-full object-cover" />
        </button>
    }
}

// ---------------------------------------------------------- turn and timer

/// Whose turn it is, and how long is left in it. Two slabs rather than one:
/// the clock changes every second and the name does not, so they are read at
/// different rates and kept apart.
#[component]
fn TurnRow() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let clock = use_context::<crate::pages::game::TurnClock>().expect("TurnClock missing");

    let name = move || state.player_name(state.current_turn_player.get());
    let colour = move || {
        state
            .players
            .get()
            .iter()
            .find(|p| p.player_id == state.current_turn_player.get())
            .map(|p| p.colour.hex())
            .unwrap_or("#94a3b8")
    };
    let mine = move || state.player_id.get() == Some(state.current_turn_player.get());
    let running = move || state.game_phase.get() == GamePhase::RegularPlay;

    let critical = move || clock.remaining.get() <= 5;
    let warning = move || clock.remaining.get() <= 15;

    view! {
        <Show when=running>
            <div class="flex items-stretch gap-[7px]">
                <div
                    class="hud-tray flex items-center gap-3 pl-2.5 pr-4"
                    style="width: 365px; height: 58px;"
                >
                    // The avatar: a ring in the player's colour around a
                    // simple figure, so a glance is enough to know whose turn.
                    <div
                        class="w-[46px] h-[46px] rounded-full shrink-0 flex items-center justify-center
                               border-[3px] border-white shadow-[0_1px_3px_rgba(0,0,0,0.35)]"
                        style=move || format!("background-color: {}", colour())
                    >
                        <svg class="w-7 h-7" viewBox="0 0 24 24" fill="#fff">
                            <circle cx="12" cy="8" r="4"/>
                            <path d="M4 21a8 8 0 0 1 16 0z"/>
                        </svg>
                    </div>

                    <span class="flex-1 min-w-0 text-center text-[25px] leading-none text-[#333029] truncate">
                        {move || if mine() { "Your Turn".to_string() } else { format!("{}'s Turn", name()) }}
                    </span>
                </div>

                <div
                    class=move || format!(
                        "hud-tray flex items-center justify-center text-[25px] leading-none tabular-nums {}",
                        if critical() { "text-[#c92a2a] animate-pulse font-bold" }
                        else if warning() { "text-[#c47510] font-semibold" }
                        else { "text-[#333029]" }
                    )
                    style="width: 118px; height: 58px;"
                    title="Time left in this turn"
                >
                    {move || format!("{:02}:{:02}", clock.remaining.get().max(0) / 60, clock.remaining.get().max(0) % 60)}
                </div>
            </div>
        </Show>
    }
}

// -------------------------------------------------------------- the controls

/// The six controls: trade, buy a development card, and the three pieces, then
/// end the turn. Icon-only and all the same size, so the row reads as a bank
/// of physical buttons rather than a toolbar.
#[component]
fn ActionBar() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let draft = use_context::<TradeDraft>().expect("TradeDraft missing");

    // (wood, brick, sheep, wheat, ore)
    const ROAD: [u8; 5] = [1, 1, 0, 0, 0];
    const SETTLEMENT: [u8; 5] = [1, 1, 1, 1, 0];
    const CITY: [u8; 5] = [0, 0, 0, 2, 3];
    const DEV_CARD: [u8; 5] = [0, 0, 1, 1, 1];

    let affords = move |cost: [u8; 5]| {
        let r = state.my_resources.get();
        r.lumber >= cost[0]
            && r.brick >= cost[1]
            && r.wool >= cost[2]
            && r.grain >= cost[3]
            && r.ore >= cost[4]
    };
    let placing = move || state.game_phase.get().is_initial_phase();
    let my_turn = move || state.player_id.get() == Some(state.current_turn_player.get());

    // Pieces still in the box. During setup the cost is waived, so the count
    // is the only thing that can stop you.
    let mine = move |built: RwSignal<Vec<shared::BuildingInfo>>| {
        let me = state.player_id.get();
        built.get().iter().filter(|b| Some(b.player_id) == me).count()
    };
    let roads_left = move || MAX_ROADS.saturating_sub(mine(state.roads));
    let settlements_left = move || MAX_SETTLEMENTS.saturating_sub(mine(state.settlements));
    let cities_left = move || MAX_CITIES.saturating_sub(mine(state.cities));

    let can_build = move |mode: BuildMode, cost: [u8; 5], left: usize| {
        if left == 0 {
            return false;
        }
        if placing() {
            return my_turn() && matches!(mode, BuildMode::Settlement | BuildMode::Road);
        }
        state.can_build_now() && (affords(cost) || state.free_roads_remaining.get() > 0)
    };

    let can_buy_dev = move || {
        !placing() && state.can_build_now() && affords(DEV_CARD) && state.bank.get().dev_cards > 0
    };
    let can_trade = move || !placing() && state.can_build_now();

    view! {
        <div class="shrink-0 flex items-end gap-[12px] pr-3">
            <HudButton
                label="Trade"
                enabled=Signal::derive(can_trade)
                armed=Signal::derive(move || draft.open.get())
                on_click=Callback::new(move |_| draft.open.update(|o| *o = !*o))
            >
                <Art name="trading" alt="Trade" class="hud-icon w-[58px] h-[58px] object-contain" />
            </HudButton>

            <HudButton
                label="Buy a development card"
                enabled=Signal::derive(can_buy_dev)
                badge=Signal::derive(move || state.bank.get().dev_cards.to_string())
                on_click=Callback::new(move |_| state.send(ClientRequest::BuyDevelopmentCard))
            >
                <Art name="build-dev-card" alt="Development card"
                     class="hud-icon w-[45px] h-[60px] object-contain" />
            </HudButton>

            <HudButton
                label="Build a road"
                enabled=Signal::derive(move || can_build(BuildMode::Road, ROAD, roads_left()))
                armed=Signal::derive(move || state.build_mode.get() == BuildMode::Road)
                badge=Signal::derive(move || roads_left().to_string())
                on_click=Callback::new(move |_| state.build_mode.update(|c| {
                    *c = if *c == BuildMode::Road { BuildMode::None } else { BuildMode::Road }
                }))
            >
                <Art name="build-road" alt="Road" class="hud-icon w-[26px] h-[60px] object-contain" />
            </HudButton>

            <HudButton
                label="Build a settlement"
                enabled=Signal::derive(move || can_build(BuildMode::Settlement, SETTLEMENT, settlements_left()))
                armed=Signal::derive(move || state.build_mode.get() == BuildMode::Settlement)
                badge=Signal::derive(move || settlements_left().to_string())
                on_click=Callback::new(move |_| state.build_mode.update(|c| {
                    *c = if *c == BuildMode::Settlement { BuildMode::None } else { BuildMode::Settlement }
                }))
            >
                <Art name="build-settlement" alt="Settlement"
                     class="hud-icon w-[55px] h-[55px] object-contain" />
            </HudButton>

            <HudButton
                label="Upgrade to a city"
                enabled=Signal::derive(move || can_build(BuildMode::City, CITY, cities_left()))
                armed=Signal::derive(move || state.build_mode.get() == BuildMode::City)
                badge=Signal::derive(move || cities_left().to_string())
                on_click=Callback::new(move |_| state.build_mode.update(|c| {
                    *c = if *c == BuildMode::City { BuildMode::None } else { BuildMode::City }
                }))
            >
                <Art name="build-city" alt="City" class="hud-icon w-[60px] h-[55px] object-contain" />
            </HudButton>

            <HudButton
                label="End your turn"
                enabled=Signal::derive(move || my_turn() && !placing())
                on_click=Callback::new(move |_| state.send(ClientRequest::EndTurn))
            >
                // Two big chevrons: skip ahead, end the turn.
                <svg class="hud-icon w-[72px] h-[72px]" viewBox="0 0 24 24" fill="#e8f7fd"
                     stroke="#0d3e5c" stroke-width="1.6" stroke-linejoin="round">
                    <path d="M2.5 4.5 11 12l-8.5 7.5z"/>
                    <path d="M12.5 4.5 21 12l-8.5 7.5z"/>
                </svg>
            </HudButton>
        </div>
    }
}

/// One control on the bar. Icon only, with the count in the corner when there
/// is one - text inside a button this size fights the icon for attention.
#[component]
fn HudButton(
    label: &'static str,
    enabled: Signal<bool>,
    #[prop(optional, into)] armed: MaybeSignal<bool>,
    /// Pieces left, or cards left in the bank. Kept visible when the button is
    /// disabled: what you cannot afford, you can still count. A `Signal`
    /// rather than a `MaybeSignal` because the latter is only `Copy` when its
    /// contents are, and this one holds a `String`.
    #[prop(optional)]
    badge: Option<Signal<String>>,
    on_click: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            class=move || format!(
                "hud-btn relative w-[112px] h-[112px] flex items-center justify-center {}",
                if armed.get() { "hud-btn-armed" } else { "" }
            )
            title=label
            aria-label=label
            disabled=move || !enabled.get()
            on:click=move |_| on_click.call(())
        >
            {children()}

            // Whether a button carries a count is fixed when it is built, so
            // this is a plain `Option` rather than a `Show`.
            {badge.map(|b| view! {
                <span class="hud-badge absolute top-1 right-1 min-w-[28px] h-[27px] px-1
                             text-[17px] font-black leading-none
                             flex items-center justify-center tabular-nums">
                    {move || b.get()}
                </span>
            })}
        </button>
    }
}

// ------------------------------------------------------------- trade panel

/// The trade table. Opens over the tray: pick what you want along the top,
/// click cards in your hand to put them up, then send it.
#[component]
fn TradePanel() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let draft = use_context::<TradeDraft>().expect("TradeDraft missing");

    let waiting = move || state.my_pending_trade.get();
    let accepters = move || state.my_trade_accepters.get();
    let has_accepters = move || !accepters().is_empty();

    // A trade is ready when both sides have something, and a wildcard cannot
    // stand on both sides at once - that is an offer with nothing named.
    let trade_ready = move || {
        let r = draft.receive.get().total() + draft.receive_any.get();
        let o = draft.offer.get().total() + draft.offer_any.get();
        r > 0 && o > 0 && !(draft.receive_any.get() > 0 && draft.offer_any.get() > 0)
    };
    let can_send =
        move || trade_ready() && state.can_build_now() && state.my_pending_trade.get().is_none();
    let can_clear = move || !draft.is_empty() || state.my_pending_trade.get().is_some();

    // A bank trade is the special case of n of one kind for one of another.
    let bank_deal = move || -> Option<(Res, Res)> {
        if draft.receive_any.get() > 0 || draft.offer_any.get() > 0 {
            return None;
        }
        let (give, want) = (draft.offer.get(), draft.receive.get());
        if want.total() != 1 {
            return None;
        }
        let taken = Res::ALL.into_iter().find(|k| want.get(*k) == 1)?;
        let given: Vec<Res> = Res::ALL.into_iter().filter(|k| give.get(*k) > 0).collect();
        if given.len() != 1 {
            return None;
        }
        let only = given[0];
        let ratio = state.get_best_ratio(only.shared());
        (give.get(only) == ratio && only != taken).then_some((only, taken))
    };

    let send = move |_| {
        if let Some((g, w)) = bank_deal() {
            state.send(ClientRequest::BankTrade { give: g.shared(), receive: w.shared() });
        } else {
            state.send(ClientRequest::TradeOffer {
                target_player_id: None,
                offer: draft.offer.get_untracked().to_shared(),
                request: draft.receive.get_untracked().to_shared(),
                offer_any: draft.offer_any.get_untracked(),
                request_any: draft.receive_any.get_untracked(),
            });
        }
        draft.clear();
    };

    let clear = move |_| {
        if let Some(offer_id) = state.my_pending_trade.get_untracked() {
            state.send(ClientRequest::CancelTrade { offer_id });
        }
        draft.clear();
    };

    view! {
        <div class="absolute left-0 bottom-[129px] z-30 hud-tray p-3.5 flex flex-col gap-3"
             style="width: 800px; max-width: 60vw;">

            <div class="flex items-center justify-between">
                <span class="text-[11px] font-black uppercase tracking-[0.15em] text-[#7a7263]">
                    "Ask for"
                </span>
                <button
                    class="w-7 h-7 rounded-md text-[#7a7263] hover:bg-black/10 text-lg leading-none"
                    title="Close the trade table"
                    on:click=move |_| draft.open.set(false)
                >"\u{00d7}"</button>
            </div>

            // What you want. Click to add one, right-click to take one back.
            <div class="flex items-center gap-3">
                {Res::ALL.map(|k| {
                    let picked = move || draft.receive.get().get(k);
                    view! {
                        <button
                            class="game-card-pick"
                            on:click=move |_| draft.receive.update(|b| b.set(k, b.get(k).saturating_add(1)))
                            on:contextmenu=move |ev| {
                                ev.prevent_default();
                                draft.receive.update(|b| b.set(k, b.get(k).saturating_sub(1)));
                            }
                        >
                            <CardFace
                                art=k.art()
                                alt=k.label()
                                size="w-[56px] h-[78px]"
                                count=Signal::derive(picked)
                                selected=Signal::derive(move || picked() > 0)
                            />
                        </button>
                    }
                }).to_vec()}

                // A card you have not named, for them to fill in by countering.
                <button
                    class="game-card-pick"
                    on:click=move |_| draft.receive_any.update(|n| *n = n.saturating_add(1))
                    on:contextmenu=move |ev| {
                        ev.prevent_default();
                        draft.receive_any.update(|n| *n = n.saturating_sub(1));
                    }
                >
                    <MysteryCard
                        size="w-[56px] h-[78px]"
                        count=Signal::derive(move || draft.receive_any.get())
                    />
                </button>

                <span class="ml-auto text-[12px] italic text-[#8a8071] max-w-[240px] text-right">
                    "Click cards in your hand below to put them up."
                </span>
            </div>

            <div class="h-px bg-[#ddd2ba]"></div>

            // The deal as it stands.
            <Show
                when=has_accepters
                fallback=move || view! {
                    <>
                        <TradeRow
                            direction=Direction::Receive
                            basket=Signal::derive(move || draft.receive.get())
                            wild=Signal::derive(move || draft.receive_any.get())
                        />
                        <TradeRow
                            direction=Direction::Offer
                            basket=Signal::derive(move || draft.offer.get())
                            wild=Signal::derive(move || draft.offer_any.get())
                        />
                    </>
                }
            >
                <div class="text-[11px] font-black uppercase tracking-[0.15em] text-[#7a7263]">
                    "Pick who to trade with"
                </div>
                <div class="flex flex-wrap gap-2">
                    <For
                        each=accepters
                        key=|id| *id
                        children=move |accepter_id| {
                            let name = state.player_name(accepter_id);
                            view! {
                                <button
                                    class="game-btn game-btn-green px-4 py-2 text-[13px] font-black"
                                    on:click=move |_| {
                                        if let Some(offer_id) = waiting() {
                                            state.send(ClientRequest::ConfirmTrade {
                                                offer_id,
                                                partner_id: accepter_id,
                                            });
                                        }
                                    }
                                >
                                    {name}
                                </button>
                            }
                        }
                    />
                </div>
            </Show>

            <Show when=move || waiting().is_some() && !has_accepters()>
                <div class="text-[12px] italic text-[#8a8071] text-center">
                    "Offer sent. Waiting for an answer..."
                </div>
            </Show>

            <div class="flex items-center gap-2">
                <button
                    class="game-btn game-btn-green flex-1 h-11 text-[14px] font-black uppercase tracking-wider"
                    disabled=move || !can_send()
                    title="Send this offer to the table"
                    on:click=send
                >
                    "Offer"
                </button>
                <button
                    class="game-btn game-btn-red px-5 h-11 text-[14px] font-black uppercase tracking-wider"
                    disabled=move || !can_clear()
                    title="Take everything back off the table"
                    on:click=clear
                >
                    "Clear"
                </button>
            </div>

            <crate::pages::game::IncomingTrades />
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Receive,
    Offer,
}

/// One side of the deal: an arrow saying which way the cards move, then the
/// cards themselves.
#[component]
fn TradeRow(direction: Direction, basket: Signal<Basket>, wild: Signal<u8>) -> impl IntoView {
    let incoming = direction == Direction::Receive;
    let label = if incoming { "Receive" } else { "Give" };
    let colour = if incoming { "#16c33a" } else { "#ef3030" };
    let empty = move || basket.get().total() == 0 && wild.get() == 0;

    view! {
        <div class="flex items-center gap-3 min-h-[62px]">
            // The arrow carries the meaning; the word only confirms it.
            <div class="flex flex-col items-center w-14 shrink-0">
                <svg class="w-9 h-9" viewBox="0 0 24 24" fill="none"
                     stroke=colour stroke-width="3.5"
                     stroke-linecap="round" stroke-linejoin="round">
                    {if incoming {
                        view! { <><path d="M12 4v15"/><path d="m5 13 7 7 7-7"/></> }
                    } else {
                        view! { <><path d="M12 20V5"/><path d="m5 12 7-7 7 7"/></> }
                    }}
                </svg>
                <span
                    class="text-[9px] font-black uppercase tracking-wider"
                    style=format!("color: {colour}")
                >
                    {label}
                </span>
            </div>

            <Show
                when=move || !empty()
                fallback=move || view! {
                    <span class="text-[13px] italic text-[#a89e8b]">
                        {if incoming { "nothing asked for yet" } else { "nothing offered yet" }}
                    </span>
                }
            >
                <div class="flex items-center gap-2 flex-wrap">
                    {move || basket.get().spread().into_iter().enumerate().map(|(i, k)| view! {
                        <div style=format!("margin-left: {}px", if i == 0 { 0 } else { -18 })>
                            <CardFace art=k.art() alt=k.label() size="w-[44px] h-[58px]" />
                        </div>
                    }).collect_view()}

                    {move || (0..wild.get()).map(|_| view! {
                        <MysteryCard size="w-[44px] h-[58px]" />
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}
