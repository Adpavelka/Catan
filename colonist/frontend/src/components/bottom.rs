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
use crate::components::icons::{asset_url, dev_card_art, resource_art, Art};
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

/// The most of one resource a trade may ask for. No game holds more than the
/// deepest bank stack, and without a ceiling the ask palette let a single kind
/// climb to 255 - two of those overflowed `Basket::total`, which panics in a
/// debug build and silently wraps in a release one.
pub const MAX_ASK_PER_KIND: u8 = 24;

impl Basket {
    pub fn total(&self) -> u8 {
        // Saturating, not wrapping: a pile this size is already nonsense, and
        // a wrong-but-large total is far better than a panic mid-trade.
        self.wood
            .saturating_add(self.brick)
            .saturating_add(self.sheep)
            .saturating_add(self.wheat)
            .saturating_add(self.ore)
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

            // Offers waiting on you. Never behind the trade button: an offer
            // you have to go looking for is an offer you miss, and it expires
            // whether or not you opened the panel.
            <OfferDock />

            // And the other direction: how your own offer is being answered.
            <crate::components::offer_status::OfferStatus />

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

/// Resource card width, the gap wanted between two of them, and the sliver of
/// a card that is left showing once the run has closed up as far as it goes.
///
/// The sliver is small on purpose. A resource card that has closed up to a
/// stripe is still countable, and is still reachable from the trade window; a
/// card pushed past the end of the tray is neither, because the tray clips.
const CARD_W: f64 = 63.0;
const GAP: f64 = 3.0;
const MIN_VISIBLE: f64 = 6.0;
/// The tray's own padding.
const TRAY_PAD: f64 = 30.0;
/// The most of the tray the development cards may take when the two runs
/// cannot both have what they want. They come first, but not to the point of
/// leaving the resources nowhere to go.
const DEV_SHARE: f64 = 0.5;
/// The same three for a development card, plus the rule and padding that set
/// the run apart from the resources.
///
/// These have to be the real measurements. The room kept for them used to be
/// one flat 74px whatever the hand held, so a second card was already wider
/// than its own allowance and the run was pushed off the end of the tray,
/// where it could not be clicked.
const DEV_W: f64 = 68.0;
const DEV_GAP: f64 = 8.0;
const DEV_LEAD: f64 = 18.0;
const DEV_MIN_VISIBLE: f64 = 26.0;

/// How much each card after the first has to give up for `n` of them to fit in
/// `avail`, never past the point where one is too slim to aim at.
fn closing(n: usize, avail: f64, w: f64, gap: f64, min_visible: f64) -> f64 {
    if n < 2 || avail <= 0.0 {
        return 0.0;
    }
    let natural = n as f64 * w + (n - 1) as f64 * gap;
    if natural <= avail {
        return 0.0;
    }
    ((natural - avail) / (n - 1) as f64).min(w + gap - min_visible)
}

/// What a run of `n` cards occupies once it has closed up by `overlap`.
fn run_width(n: usize, w: f64, gap: f64, overlap: f64) -> f64 {
    if n == 0 {
        0.0
    } else {
        n as f64 * w + (n - 1) as f64 * (gap - overlap)
    }
}

/// What the development cards occupy in a tray with `usable` width to spend.
///
/// They are laid out before the resources and against a budget of their own,
/// rather than against whatever the resources have left over. A development
/// card is only playable from the tray, so one pushed past the end is a card
/// you have lost; a resource card closed up to a stripe is still countable and
/// can still be picked up in the trade window. Hence the order - and hence the
/// budget, so a fistful of them cannot leave the hand nowhere to go.
fn dev_run(n: usize, resources: usize, usable: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let wanted = DEV_LEAD + run_width(n, DEV_W, DEV_GAP, 0.0);
    // What the hand needs once it has closed up as far as it is allowed to.
    // The development cards give that back first, and only then start eating
    // into their own share.
    let hand_floor = run_width(resources, CARD_W, GAP, CARD_W + GAP - MIN_VISIBLE);
    let budget = wanted.min((usable - hand_floor).max(usable * DEV_SHARE));
    let overlap = closing(n, budget - DEV_LEAD, DEV_W, DEV_GAP, DEV_MIN_VISIBLE);
    DEV_LEAD + run_width(n, DEV_W, DEV_GAP, overlap)
}

/// The player's own cards, in a wide tray along the bottom-left.
///
/// Every card is drawn: three sheep are three sheep side by side, not one
/// sheep with a "3" on it. Counting cards is how you read a hand at a table,
/// and the tray is wide enough to let you.
#[component]
fn HandTray() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let draft = use_context::<TradeDraft>().expect("TradeDraft missing");

    let dev_cards = move || state.my_dev_cards.get();
    // Each card with whether *it* was drawn this turn.
    //
    // Freshness has to be per card, not per kind: buying a second knight used
    // to grey out the first one as well, because the test was "is a knight
    // among this turn's draws". The cards of a kind are interchangeable, so
    // the last N of each kind are taken to be the N drawn this turn.
    //
    // Built outside the view macro: a turbofish inside it parses as tags.
    let dev_cards_keyed = move || {
        let hand = dev_cards();
        let drawn = state.fresh_dev_cards.get();
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        hand.iter()
            .enumerate()
            .map(|(i, card)| {
                let kind = format!("{card:?}");
                let held = hand.iter().filter(|c| format!("{c:?}") == kind).count();
                let fresh_of_kind = drawn.iter().filter(|c| format!("{c:?}") == kind).count();
                let ordinal = seen.entry(kind).or_insert(0);
                let is_fresh = *ordinal >= held.saturating_sub(fresh_of_kind);
                *ordinal += 1;
                (i, card.clone(), is_fresh)
            })
            .collect::<Vec<(usize, DevCardType, bool)>>()
    };
    let has_dev_cards = move || !dev_cards().is_empty();

    // A big hand closes up rather than running off the end of the tray.
    //
    // Scrolling was the old answer and it was the wrong one: cards you have to
    // scroll to are cards you forget you hold. So the row is measured and the
    // cards overlap by exactly as much as it takes to fit, and no more - a
    // small hand still sits out flat with a gap between each card.
    //
    // The resource cards are one flat run rather than a group per kind, which
    // is what makes the arithmetic exact: with per-kind wrappers the gaps
    // between groups were not in the sum, so the row overflowed anyway.
    let tray: NodeRef<html::Div> = create_node_ref();
    let width = create_rw_signal(0.0f64);
    let resize_tick = create_rw_signal(0u32);
    window_event_listener(ev::resize, move |_| resize_tick.update(|n| *n += 1));

    // One entry per resource card, in board order, flagged if it is already
    // up for trade. Which copy of a kind is flagged does not matter, so the
    // committed ones are taken off the right-hand end of each run.
    let hand = move || {
        let held = state.my_resources.get();
        let up = draft.offer.get();
        Res::ALL
            .into_iter()
            .flat_map(|k| {
                let n = k.in_hand(&held);
                // The draft can outlive the cards it was built from - an
                // opponent's monopoly, or being robbed, while the window is
                // open. Clamping first stops `n - committed` saturating to
                // zero and marking the whole run as already up for trade.
                let committed = up.get(k).min(n);
                (0..n).map(move |i| (k, i >= n.saturating_sub(committed)))
            })
            .collect::<Vec<(Res, bool)>>()
    };

    // Re-measure after anything that could change the fit, on the next frame
    // so the browser has laid the row out first.
    create_effect(move |_| {
        let _ = hand().len();
        let _ = dev_cards().len();
        let _ = resize_tick.get();
        request_animation_frame(move || {
            if let Some(el) = tray.get_untracked() {
                width.set(el.client_width() as f64);
            }
        });
    });

    let usable = move || width.get() - TRAY_PAD;
    let dev_taken = create_memo(move |_| dev_run(dev_cards().len(), hand().len(), usable()));

    // Recovered from the run's width so the margin between two cards can be
    // set without `dev_run` having to hand back two numbers.
    let dev_overlap = create_memo(move |_| match dev_cards().len() {
        n if n < 2 => 0.0,
        n => DEV_GAP - (dev_taken.get() - DEV_LEAD - n as f64 * DEV_W) / (n - 1) as f64,
    });

    let overlap = create_memo(move |_| {
        closing(
            hand().len(),
            usable() - dev_taken.get(),
            CARD_W,
            GAP,
            MIN_VISIBLE,
        )
    });

    // Numbered outside the view macro: a turbofish inside it parses as tags.
    let hand_numbered = move || {
        hand()
            .into_iter()
            .enumerate()
            .collect::<Vec<(usize, (Res, bool))>>()
    };

    view! {
        <div
            node_ref=tray
            // Where your own income lands. `ResourceFlight` measures this to
            // fly your cards into your own hand, while everybody else's go to
            // their row in the rail - see `hand_centre`.
            data-my-tray=""
            class="hud-tray flex-1 min-w-0 flex items-center px-3.5 overflow-hidden"
            style="height: 119px;"
        >
            <div class="flex items-center shrink-0">
                <For
                    each=hand_numbered
                    key=|(i, (k, up))| (*i, k.label(), *up)
                    children=move |(i, (k, is_up))| {
                        view! {
                            <button
                                class="shrink-0 game-card-pick relative"
                                // Later cards sit above earlier ones, so a
                                // closed-up run still reads left to right.
                                style=move || format!(
                                    "z-index: {i}; margin-left: {:.1}px",
                                    if i == 0 { 0.0 } else { GAP - overlap.get() },
                                )
                                title=move || if is_up {
                                    format!("{} - up for trade, click to take it back", k.label())
                                } else {
                                    format!("{} - click to put it up for trade", k.label())
                                }
                                on:click=move |_| {
                                    if is_up {
                                        draft.offer.update(|b| b.set(k, b.get(k).saturating_sub(1)));
                                    } else {
                                        draft.open.set(true);
                                        draft.offer.update(|b| b.set(k, b.get(k) + 1));
                                    }
                                }
                                on:contextmenu=move |ev| {
                                    ev.prevent_default();
                                    draft.offer.update(|b| b.set(k, b.get(k).saturating_sub(1)));
                                }
                            >
                                <CardFace art=k.art() alt=k.label() dimmed=is_up />
                            </button>
                        }
                    }
                />
            </div>

            // Development cards sit at the end of the hand, as they do on a
            // real table, and are played from here.
            <Show when=has_dev_cards>
                <div class="flex items-center pl-3 ml-1 border-l-2 border-[#ddd2ba] shrink-0 self-center">
                    <For
                        each=dev_cards_keyed
                        key=|(i, c, fresh)| (*i, format!("{c:?}"), *fresh)
                        children=move |(i, card, fresh)| view! {
                            <span
                                class="shrink-0"
                                style=move || format!(
                                    "z-index: {i}; margin-left: {:.1}px",
                                    if i == 0 { 0.0 } else { DEV_GAP - dev_overlap.get() },
                                )
                            >
                                <DevCardInHand card=card fresh=fresh />
                            </span>
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

/// One development card in hand, playable or not.
#[component]
fn DevCardInHand(card: DevCardType, fresh: bool) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let is_point = card == DevCardType::VictoryPoint;
    // A signal rather than a closure: read from several places, and a closure
    // that captures is not Copy.
    let fresh = Signal::derive(move || fresh);
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
                        <Art name="trade-offerer" alt="" class="w-8 h-8 object-contain" />
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

    // `free` marks the road control: Road Building pays for roads, and only
    // roads. Sharing that escape hatch with the settlement and city controls
    // lit all three up on an empty hand, and a click armed a build the server
    // then refused for want of resources.
    let can_build = move |cost: [u8; 5], left: usize, free: bool| {
        if left == 0 {
            return false;
        }
        // Setup drives itself: the board arms the right mode from the phase
        // and offers only the legal spots. Leaving these live would let a
        // click here disarm the very thing the player has to do next.
        if placing() {
            return false;
        }
        let paid_for = affords(cost) || (free && state.free_roads_remaining.get() > 0);
        state.can_build_now() && paid_for
    };

    let can_buy_dev = move || {
        !placing() && state.can_build_now() && affords(DEV_CARD) && state.bank.get().dev_cards > 0
    };
    let can_trade = move || !placing() && state.can_build_now();

    view! {
        <div class="shrink-0 flex items-end gap-[12px] pr-3">
            <HudButton
                label="Trade with the table or the bank"
                // Answering an offer is not a turn action, so this stays live
                // even when it is somebody else's turn.
                enabled=Signal::derive(move || can_trade() || !state.incoming_trades.get().is_empty())
                armed=Signal::derive(move || draft.open.get())
                on_click=Callback::new(move |_| draft.open.update(|o| *o = !*o))
            >
                <Art name="trading" alt="Trade" class="hud-icon w-[58px] h-[58px] object-contain" />
            </HudButton>

            <HudButton
                label="Buy a development card"
                enabled=Signal::derive(can_buy_dev)
                badge=Signal::derive(move || state.bank.get().dev_cards.to_string())
                cost=DEV_CARD
                on_click=Callback::new(move |_| state.send(ClientRequest::BuyDevelopmentCard))
            >
                <Art name="build-dev-card" alt="Development card"
                     class="hud-icon w-[45px] h-[60px] object-contain" />
            </HudButton>

            <HudButton
                label="Build a road"
                enabled=Signal::derive(move || can_build(ROAD, roads_left(), true))
                armed=Signal::derive(move || state.build_mode.get() == BuildMode::Road)
                badge=Signal::derive(move || roads_left().to_string())
                cost=ROAD
                on_click=Callback::new(move |_| state.build_mode.update(|c| {
                    *c = if *c == BuildMode::Road { BuildMode::None } else { BuildMode::Road }
                }))
            >
                <PieceIcon art="build-road" alt="Road" class="w-[26px] h-[60px]" />
            </HudButton>

            <HudButton
                label="Build a settlement"
                enabled=Signal::derive(move || can_build(SETTLEMENT, settlements_left(), false))
                armed=Signal::derive(move || state.build_mode.get() == BuildMode::Settlement)
                badge=Signal::derive(move || settlements_left().to_string())
                cost=SETTLEMENT
                on_click=Callback::new(move |_| state.build_mode.update(|c| {
                    *c = if *c == BuildMode::Settlement { BuildMode::None } else { BuildMode::Settlement }
                }))
            >
                <PieceIcon art="build-settlement" alt="Settlement" class="w-[55px] h-[55px]" />
            </HudButton>

            <HudButton
                label="Upgrade to a city"
                enabled=Signal::derive(move || can_build(CITY, cities_left(), false))
                armed=Signal::derive(move || state.build_mode.get() == BuildMode::City)
                badge=Signal::derive(move || cities_left().to_string())
                cost=CITY
                on_click=Callback::new(move |_| state.build_mode.update(|c| {
                    *c = if *c == BuildMode::City { BuildMode::None } else { BuildMode::City }
                }))
            >
                <PieceIcon art="build-city" alt="City" class="w-[60px] h-[55px]" />
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

/// A build piece's artwork in a player's colour, retaining the source linework.
#[component]
fn PieceIcon(
    art: &'static str,
    alt: &'static str,
    /// Tailwind sizing for the box the piece is drawn in.
    class: &'static str,
) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let colour = move || {
        state
            .players
            .get()
            .iter()
            .find(|p| Some(p.player_id) == state.player_id.get())
            .map(|p| p.colour.hex())
            .unwrap_or("#3f4a55")
    };

    let art_url = asset_url(art);
    let filter_id = format!("hud-tint-{}", art);

    view! {
        <svg class="absolute w-0 h-0" aria-hidden="true" focusable="false">
            <defs>
                <filter id=filter_id.clone() color-interpolation-filters="sRGB">
                    <feFlood flood-color=move || colour() result="flat" />
                    <feComposite in="flat" in2="SourceAlpha" operator="in" result="solid" />
                    <feColorMatrix in="SourceGraphic" type="saturate" values="0" result="grey" />
                    <feComponentTransfer in="grey" result="soft">
                        <feFuncR type="linear" slope="0.42" intercept="0.58" />
                        <feFuncG type="linear" slope="0.42" intercept="0.58" />
                        <feFuncB type="linear" slope="0.42" intercept="0.58" />
                    </feComponentTransfer>
                    <feBlend in="soft" in2="solid" mode="multiply" result="shaded" />
                    <feComposite in="shaded" in2="SourceAlpha" operator="in" />
                </filter>
            </defs>
        </svg>
        <img
            src=art_url
            alt=alt
            draggable="false"
            class=format!("hud-icon select-none pointer-events-none {class}")
            style=move || format!(
                "filter: url(#{}) drop-shadow(0 1px 1px rgb(0 0 0 / 0.35));",
                filter_id
            )
        />
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
    /// What it costs, as (wood, brick, sheep, wheat, ore). Shown on hover.
    #[prop(optional)]
    cost: Option<[u8; 5]>,
    on_click: Callback<()>,
    children: Children,
) -> impl IntoView {
    // A priced control you can afford right now wears a gold rim. The plain
    // cyan ones - trade, end turn - are always available on your turn, so a
    // rim on those would say nothing; the rim is there to answer "what can I
    // build", and it has to be visible without hovering anything.
    let priced = cost.is_some();

    view! {
        <button
            class=move || format!(
                "hud-btn group relative z-[40] w-[112px] h-[112px] flex items-center justify-center {}",
                if armed.get() {
                    "hud-btn-armed"
                } else if priced && enabled.get() {
                    "hud-btn-ready"
                } else {
                    ""
                }
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

            {cost.map(|c| view! { <CostHint label=label cost=c /> })}
        </button>
    }
}

/// The price of a control, shown above it on hover.
///
/// One card per unit: two wheat is two wheat cards side by side, not a wheat
/// card with a two on it. You read a price the way you would count it out of
/// your hand, and the cards are big enough to tell apart at a glance.
#[component]
fn CostHint(label: &'static str, cost: [u8; 5]) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    // Each card carries which copy of its kind it is, so "two wheat" can grey
    // only the second one when you hold exactly one.
    let cards: Vec<(Res, u8)> = Res::ALL
        .into_iter()
        .zip(cost)
        .flat_map(|(k, n)| (0..n).map(move |i| (k, i)))
        .collect();

    view! {
        <div
            class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2.5 z-[999]
                   px-2.5 py-2 flex flex-col items-center gap-1.5
                   opacity-0 invisible translate-y-1
                   group-hover:opacity-100 group-hover:visible group-hover:translate-y-0
                   transition-all duration-100"
            style="background: #f5f0e6; border: 2px solid #cdc4b2; border-radius: 8px; box-shadow: 0 6px 14px rgba(0,0,0,0.28);"
        >
            <span class="text-[10px] font-black uppercase tracking-[0.14em] text-[#7a7263] whitespace-nowrap">
                {label}
            </span>
            <div class="flex items-end gap-1">
                {cards.into_iter().map(|(k, nth)| {
                    // A card you cannot cover is greyed, so the hint says not
                    // just what it costs but what you are still missing.
                    let short = move || k.in_hand(&state.my_resources.get()) <= nth;
                    view! {
                        <CardFace
                            art=k.art()
                            alt=k.label()
                            size="w-[42px] h-[58px]"
                            dimmed=Signal::derive(short)
                        />
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

// -------------------------------------------------------------- offer dock

/// Offers other people have put to you, stacked above the controls.
///
/// This sits outside the trade panel on purpose. An offer arrives whether or
/// not you have the panel open, it expires on a clock you did not start, and
/// it is answered with two buttons - so it belongs on screen, not behind one.
#[component]
fn OfferDock() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let any = move || !state.incoming_trades.get().is_empty();

    view! {
        <Show when=any>
            // The same width and the same corner as the proposer's own panel
            // in `OfferStatus`: one offer should be the same object on both
            // sides of the table. The cards are not wrapped in a tray of their
            // own either - a tray inside a tray inset them by its padding and
            // left the receiving side reading as a smaller, tighter thing than
            // the side that sent it.
            <div
                class="fixed z-[59] flex flex-col gap-1.5 items-stretch"
                style="right: calc(var(--rail-w) + var(--rail-gap)); top: 12px; width: 485px;"
            >
                <div class="text-[11px] font-black uppercase tracking-[0.15em] text-[#f0e6d2]
                            drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)] pl-1">
                    {move || {
                        let n = state.incoming_trades.get().len();
                        if n == 1 { "An offer for you".to_string() } else { format!("{n} offers for you") }
                    }}
                </div>
                <crate::pages::game::IncomingTrades />
            </div>
        </Show>
    }
}

// ------------------------------------------------------------- trade panel

/// The trade window.
///
/// Four parts: the palette of resource types across the top, two directional
/// drop zones in the middle, your own cards along the bottom, and the three
/// big controls down the right. Cards move between the zones by clicking -
/// the palette fills the "they give" side, your hand fills the "you give"
/// side - and the arrows, not words, say which way each row flows.
#[component]
fn TradePanel() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let draft = use_context::<TradeDraft>().expect("TradeDraft missing");

    // ---- the bank deal.
    //
    // Every kind you put up must be an exact multiple of *its own* rate - the
    // rate depends on the ports you hold, so four brick and two ore can both
    // be one card's worth in the same deal - and the number of cards you ask
    // for has to match the number of exchanges that buys. Four brick plus four
    // ore for a sheep and a wood is a perfectly ordinary two-exchange deal.
    let bank_exchanges = move || -> Option<Vec<(Res, Res)>> {
        if draft.receive_any.get() > 0 || draft.offer_any.get() > 0 {
            return None;
        }
        let (give, want) = (draft.offer.get(), draft.receive.get());
        if give.total() == 0 || want.total() == 0 {
            return None;
        }

        // What you are giving, expanded into one entry per exchange it buys.
        let mut credits: Vec<Res> = Vec::new();
        for k in Res::ALL {
            let n = give.get(k);
            if n == 0 {
                continue;
            }
            let rate = state.get_best_ratio(k.shared());
            if rate == 0 || n % rate != 0 {
                return None;
            }
            credits.extend(std::iter::repeat(k).take((n / rate) as usize));
        }

        let taken = want.spread();
        if credits.len() != taken.len() {
            return None;
        }
        // The bank will not swap a card for the same kind of card.
        let pairs: Vec<(Res, Res)> = credits.into_iter().zip(taken).collect();
        pairs.iter().all(|(g, t)| g != t).then_some(pairs)
    };

    // ---- the table deal: both sides hold something, and a wildcard cannot
    // stand on both at once - that is an offer with nothing named at all.
    let table_ready = move || {
        let r = draft.receive.get().total() + draft.receive_any.get();
        let o = draft.offer.get().total() + draft.offer_any.get();
        r > 0 && o > 0 && !(draft.receive_any.get() > 0 && draft.offer_any.get() > 0)
    };

    let on_turn = move || state.can_build_now();
    let can_bank = move || on_turn() && bank_exchanges().is_some();
    let can_offer = move || on_turn() && table_ready() && !state.offers_are_full();

    let send_to_bank = move |_| {
        let Some(pairs) = bank_exchanges() else { return };
        // The protocol settles one exchange at a time. Each is independently
        // affordable - that is what `bank_exchanges` just checked - so sending
        // them in sequence is the same deal, and the server validates each.
        for (g, w) in pairs {
            state.send(ClientRequest::BankTrade { give: g.shared(), receive: w.shared() });
        }
        draft.clear();
    };

    let send_to_table = move |_| {
        state.send(ClientRequest::TradeOffer {
            target_player_id: None,
            offer: draft.offer.get_untracked().to_shared(),
            request: draft.receive.get_untracked().to_shared(),
            offer_any: draft.offer_any.get_untracked(),
            request_any: draft.receive_any.get_untracked(),
        });
        // Deliberately not cleared here. The server can still refuse the
        // offer - the hand may have shrunk since the cards were laid out -
        // and clearing on send threw away the composed trade instead of
        // letting the player fix it. The bench empties below, once the offer
        // has actually reached the table.
    };

    // The offer landed: the roster of my open offers just grew.
    create_effect(move |previous: Option<usize>| {
        let open_now = state.my_offers.get().len();
        if previous.is_some_and(|before| open_now > before) {
            draft.clear();
        }
        open_now
    });

    let close = move |_| {
        draft.clear();
        draft.open.set(false);
    };

    view! {
        <div
            class="absolute left-0 bottom-[129px] z-30 flex items-stretch gap-2.5"
            style="width: 1075px; max-width: 78vw;"
        >
            // ---- the window itself
            <div class="flex-1 min-w-0 flex flex-col gap-2.5">

                // TOP: the palette. Every resource type, plus the unnamed
                // card. Clicking one asks for it.
                <div class="hud-tray flex items-center gap-2.5 px-3" style="height: 104px;">
                    {Res::ALL.map(|k| {
                        let picked = move || draft.receive.get().get(k);
                        view! {
                            <button
                                class="game-card-pick shrink-0"
                                title=move || format!("Ask for {}", k.label().to_lowercase())
                                on:click=move |_| draft.receive.update(|b| {
                                    b.set(k, b.get(k).saturating_add(1).min(MAX_ASK_PER_KIND))
                                })
                                on:contextmenu=move |ev| {
                                    ev.prevent_default();
                                    draft.receive.update(|b| b.set(k, b.get(k).saturating_sub(1)));
                                }
                            >
                                <CardFace
                                    art=k.art()
                                    alt=k.label()
                                    size="w-[58px] h-[80px]"
                                    count=Signal::derive(picked)
                                    selected=Signal::derive(move || picked() > 0)
                                />
                            </button>
                        }
                    }).to_vec()}

                    <button
                        class="game-card-pick shrink-0"
                        title="Any card - they choose which"
                        on:click=move |_| draft.receive_any.update(|n| *n = n.saturating_add(1))
                        on:contextmenu=move |ev| {
                            ev.prevent_default();
                            draft.receive_any.update(|n| *n = n.saturating_sub(1));
                        }
                    >
                        <MysteryCard
                            size="w-[58px] h-[80px]"
                            count=Signal::derive(move || draft.receive_any.get())
                        />
                    </button>
                </div>

                // MIDDLE: the two directions. Deliberately roomy - the cards
                // on the table are the point, and the empty half of each row
                // is where they land.
                <div class="hud-tray px-3 py-2.5 flex flex-col justify-center gap-1" style="height: 200px;">
                    <DropZone
                        direction=Direction::Receive
                        basket=Signal::derive(move || draft.receive.get())
                        wild=Signal::derive(move || draft.receive_any.get())
                        on_take_back=Callback::new(move |k: Res| {
                            draft.receive.update(|b| b.set(k, b.get(k).saturating_sub(1)))
                        })
                        on_take_back_wild=Callback::new(move |_| {
                            draft.receive_any.update(|n| *n = n.saturating_sub(1))
                        })
                    />
                    <DropZone
                        direction=Direction::Offer
                        basket=Signal::derive(move || draft.offer.get())
                        wild=Signal::derive(move || draft.offer_any.get())
                        on_take_back=Callback::new(move |k: Res| {
                            draft.offer.update(|b| b.set(k, b.get(k).saturating_sub(1)))
                        })
                        on_take_back_wild=Callback::new(move |_| {
                            draft.offer_any.update(|n| *n = n.saturating_sub(1))
                        })
                    />
                </div>

            </div>

            // ---- RIGHT: bank it, offer it, or drop it.
            <div class="shrink-0 flex flex-col gap-2.5">
                <TradeActionButton
                    label="Trade with the bank"
                    enabled=Signal::derive(can_bank)
                    on_click=Callback::new(send_to_bank)
                    tick=true
                >
                    <Art name="bank" alt="Bank" class="hud-icon w-[56px] h-[56px] object-contain" />
                </TradeActionButton>

                <TradeActionButton
                    label="Offer this to the table"
                    enabled=Signal::derive(can_offer)
                    on_click=Callback::new(send_to_table)
                    tick=true
                >
                    <GroupMark class="hud-icon w-[58px] h-[58px]" />
                </TradeActionButton>

                <TradeActionButton
                    label="Close and clear the trade"
                    enabled=Signal::derive(|| true)
                    on_click=Callback::new(close)
                    tick=false
                >
                    <svg class="hud-icon w-[62px] h-[62px]" viewBox="0 0 24 24" fill="none"
                         stroke="#0d3e5c" stroke-width="4" stroke-linecap="round">
                        <path d="M5 5 19 19"/><path d="M19 5 5 19"/>
                    </svg>
                </TradeActionButton>
            </div>
        </div>
    }
}

/// Everyone else at the table, as one mark. White artwork, so it reads on
/// both the cream panel and the cyan button.
#[component]
fn GroupMark(#[prop(default = "w-6 h-6")] class: &'static str) -> impl IntoView {
    view! {
        <Art
            name="trade-sender"
            alt="Everyone else at the table"
            class=Box::leak(format!("object-contain {class}").into_boxed_str())
        />
    }
}

/// A single player, in their own colour: the artwork on a coloured disc.
#[component]
fn SelfMark(#[prop(default = 46)] size: u32) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let colour = move || {
        state
            .players
            .get()
            .iter()
            .find(|p| Some(p.player_id) == state.player_id.get())
            .map(|p| p.colour.hex())
            .unwrap_or("#94a3b8")
    };

    view! {
        <span
            class="rounded-full shrink-0 flex items-center justify-center border-[3px] border-white
                   shadow-[0_0_0_2px_rgba(13,62,92,0.7)]"
            style=move || format!("width: {size}px; height: {size}px; background-color: {}", colour())
            title="You"
        >
            <Art
                name="trade-offerer"
                alt="You"
                class=Box::leak(
                    format!("object-contain w-[{}px] h-[{}px]", size * 7 / 10, size * 7 / 10)
                        .into_boxed_str(),
                )
            />
        </span>
    }
}

/// One of the three controls down the right of the trade window.
#[component]
fn TradeActionButton(
    label: &'static str,
    enabled: Signal<bool>,
    on_click: Callback<()>,
    /// Whether this control settles the deal. Settling controls carry a small
    /// tick in the corner so the destructive one is never mistaken for them.
    tick: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            class="hud-btn relative w-[112px] h-[110px] flex items-center justify-center"
            title=label
            aria-label=label
            disabled=move || !enabled.get()
            on:click=move |_| on_click.call(())
        >
            {children()}

            {tick.then(|| view! {
                <span class="hud-badge absolute top-1 right-1 w-[28px] h-[27px]
                             flex items-center justify-center">
                    <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="#fff"
                         stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M4 12.5 9.5 18 20 6"/>
                    </svg>
                </span>
            })}
        </button>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Cards coming to you from everyone else.
    Receive,
    /// Cards going from you to them.
    Offer,
}

/// One side of the deal: who it concerns, which way it flows, and the cards
/// on it.
///
/// The arrow is the sentence, not decoration - green down for what comes to
/// you, red up for what leaves you - so the row needs no verb. The right-hand
/// two thirds stay empty until cards land there, which is what makes it read
/// as somewhere to put things.
#[component]
fn DropZone(
    direction: Direction,
    basket: Signal<Basket>,
    wild: Signal<u8>,
    on_take_back: Callback<Res>,
    on_take_back_wild: Callback<()>,
) -> impl IntoView {
    let incoming = direction == Direction::Receive;
    let empty = move || basket.get().total() == 0 && wild.get() == 0;

    view! {
        <div class="flex items-center gap-3 h-[88px]">
            // Who this side is about.
            <div class="w-[52px] shrink-0 flex items-center justify-center">
                <Show
                    when=move || incoming
                    fallback=move || view! { <SelfMark size=46 /> }
                >
                    <GroupMark class="w-[46px] h-[46px]" />
                </Show>
            </div>

            // Which way the cards move.
            <div class="w-[38px] shrink-0 flex justify-center">
                <svg class="w-[34px] h-[40px]" viewBox="0 0 24 28"
                     fill=if incoming { "#16c33a" } else { "#ef3030" }
                     stroke="#0d3e5c" stroke-width="1.8" stroke-linejoin="round">
                    {if incoming {
                        view! { <path d="M9 1h6v14h6l-9 12-9-12h6z"/> }
                    } else {
                        view! { <path d="M9 27h6V13h6L12 1 3 13h6z"/> }
                    }}
                </svg>
            </div>

            // Where the cards land. Empty until something is put here.
            <div class="flex-1 min-w-0 flex items-center gap-2 flex-wrap">
                <Show when=move || !empty()>
                    {move || basket.get().spread().into_iter().map(|k| view! {
                        <button
                            class="game-card-pick"
                            title=move || format!("{} - click to take it back", k.label())
                            on:click=move |_| on_take_back.call(k)
                        >
                            <CardFace art=k.art() alt=k.label() size="w-[52px] h-[72px]" />
                        </button>
                    }).collect_view()}

                    {move || (0..wild.get()).map(|_| view! {
                        <button
                            class="game-card-pick"
                            title="Any card - click to take it back"
                            on:click=move |_| on_take_back_wild.call(())
                        >
                            <MysteryCard size="w-[52px] h-[72px]" />
                        </button>
                    }).collect_view()}
                </Show>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 387px is what the tray actually measures at a 1600px window, once the
    /// action bar and the sidebar have taken their share. 942px is the width
    /// the HUD was drawn for.
    const TRAYS: [f64; 4] = [387.0, 500.0, 700.0, 942.0];

    fn usable(tray: f64) -> f64 {
        tray - TRAY_PAD
    }

    fn resource_run(tray: f64, resources: usize, dev: usize) -> f64 {
        let avail = usable(tray) - dev_run(dev, resources, usable(tray));
        run_width(
            resources,
            CARD_W,
            GAP,
            closing(resources, avail, CARD_W, GAP, MIN_VISIBLE),
        )
    }

    #[test]
    fn a_small_hand_sits_out_flat() {
        assert_eq!(closing(4, 400.0, CARD_W, GAP, MIN_VISIBLE), 0.0);
        assert_eq!(closing(1, 10.0, CARD_W, GAP, MIN_VISIBLE), 0.0);
        assert_eq!(closing(0, 10.0, CARD_W, GAP, MIN_VISIBLE), 0.0);
        assert_eq!(resource_run(942.0, 5, 0), run_width(5, CARD_W, GAP, 0.0));
        // ...and so does a small pile of development cards.
        assert_eq!(
            dev_run(3, 2, usable(942.0)),
            DEV_LEAD + run_width(3, DEV_W, DEV_GAP, 0.0)
        );
    }

    #[test]
    fn closing_up_takes_the_run_to_exactly_the_room_available() {
        let overlap = closing(10, 400.0, CARD_W, GAP, MIN_VISIBLE);
        assert!(overlap > 0.0, "ten cards do not fit flat in 400px");
        assert!((run_width(10, CARD_W, GAP, overlap) - 400.0).abs() < 0.001);
    }

    #[test]
    fn a_card_never_closes_up_past_being_worth_aiming_at() {
        assert_eq!(
            closing(30, 120.0, CARD_W, GAP, MIN_VISIBLE),
            CARD_W + GAP - MIN_VISIBLE
        );
    }

    /// The regression. The room kept for development cards was a flat 74px
    /// whatever the hand held, so from the second card on the run was wider
    /// than its own allowance and hung off the end of the tray, where the tray
    /// clips it and it cannot be clicked.
    #[test]
    fn a_development_card_is_never_the_one_pushed_out() {
        const OLD_FLAT_RESERVATION: f64 = 74.0;
        for dev in 1..=8 {
            let wanted = DEV_LEAD + run_width(dev, DEV_W, DEV_GAP, 0.0);
            if dev > 1 {
                assert!(
                    wanted > OLD_FLAT_RESERVATION,
                    "{dev} cards want {wanted}px, more than the old flat reservation gave"
                );
            }
            for tray in TRAYS {
                for resources in 0..=19 {
                    let run = dev_run(dev, resources, usable(tray));
                    assert!(
                        run <= usable(tray) + 0.001,
                        "{dev} development cards and {resources} resources push \
                         the development run off a {tray}px tray"
                    );
                    // Every card still shows enough of itself to be aimed at.
                    let shown = (run - DEV_LEAD - DEV_W) / (dev.max(2) - 1) as f64;
                    assert!(
                        dev == 1 || shown >= DEV_MIN_VISIBLE - 0.001,
                        "a development card closed up to {shown:.1}px"
                    );
                }
            }
        }
    }

    /// Everything fits, over the hands a game actually produces. Past this -
    /// five or more development cards *and* a very large hand in the narrowest
    /// tray - no arrangement fits, and the resources are the ones that give,
    /// by the priority above.
    #[test]
    fn a_realistic_hand_fits_the_tray_it_is_given() {
        for tray in TRAYS {
            for dev in 0..=4 {
                for resources in 0..=19 {
                    let total =
                        resource_run(tray, resources, dev) + dev_run(dev, resources, usable(tray));
                    assert!(
                        total <= usable(tray) + 0.001,
                        "{resources} resources and {dev} development cards overflow \
                         a {tray}px tray by {:.1}px",
                        total - usable(tray)
                    );
                }
            }
        }
    }
}
