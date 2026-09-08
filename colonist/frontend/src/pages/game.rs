use leptos::*;
use uuid::Uuid;
use crate::components::board::Board;
use crate::state::GameState;
use crate::components::icons::{Art, Icon, IconKind};
use crate::components::bottom::BottomLayer;
use crate::components::flight::ResourceFlight;
use crate::components::sidebar::{ChatPanel, EventLog, PlayerPanels, ResourceBank};
use crate::components::toolbar::LeftToolbar;
use shared::{ClientRequest, GamePhase};

#[component]
pub fn GamePage() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");


    // Check if any modal is open - if so, disable pointer events on main UI
    let is_modal_open = move || {
        state.must_discard_count.get().is_some() ||
            !state.must_steal_from_players.get().is_empty() ||
            state.winner_player_id.get().is_some()
    };

    view! {
        <div class="h-screen flex flex-col overflow-hidden bg-slate-950 font-sans relative">
            // Cards travelling from the tiles that produced them to the people
            // who got them. Above everything, and never in the way.
            <ResourceFlight />

            // Discard cards modal
            <Show when=move || state.must_discard_count.get().is_some()>
                <DiscardCardsModal />
            </Show>

            // Rob player modal
            <Show when=move || !state.must_steal_from_players.get().is_empty()>
                <RobPlayerModal />
            </Show>

            // Year of Plenty modal
            <Show when=move || state.year_of_plenty_pending.get()>
                <YearOfPlentyModal />
            </Show>

            // Monopoly modal
            <Show when=move || state.monopoly_pending.get()>
                <MonopolyModal />
            </Show>

            // Main game UI - disable pointer events when modal is open
            <div class=move || if is_modal_open() { "flex-1 flex flex-col min-h-0 pointer-events-none" } else { "flex-1 flex flex-col min-h-0 pointer-events-auto" }>
            <TurnClockProvider>
            <div class="flex-1 flex min-h-0 water">
                <LeftToolbar />

                // ---- centre: the board, with the HUD along its foot. The
                // board keeps its breathing room; the HUD breaks out of it so
                // the tray runs to the edges of the window.
                <div class="flex-1 flex flex-col min-h-0 min-w-0 relative p-2 gap-2">

                    <div class="flex-1 min-h-0 min-w-0 relative">
                        <Board />
                    </div>

                    <BottomLayer />
                </div>

                // ---- right: the dashboard
                <aside
                    class="shrink-0 flex flex-col gap-1.5 p-2 min-h-0 border-l-2 border-[#04547f]"
                    style="width: 400px; background: linear-gradient(180deg, #066191 0%, #05537f 100%);"
                >
                    <EventLog />
                    <ChatPanel />
                    <ResourceBank />
                    <PlayerPanels />
                </aside>
            </div>
            </TurnClockProvider>

            // Bottom status bar - show when waiting for discards (even if I'm also discarding)
            <Show when=move || state.waiting_for_discards.get() && !state.must_move_robber.get()>
                <div class="fixed bottom-0 left-0 right-0 z-[9998] bg-yellow-900/80 border-t border-yellow-600 px-4 py-3 flex items-center justify-center gap-3 backdrop-blur-sm">
                    <div class="animate-pulse w-3 h-3 bg-yellow-400 rounded-full"></div>
                    <span class="text-yellow-200 font-bold text-sm">"Waiting for other players to discard cards..."</span>
                    <div class="animate-pulse w-3 h-3 bg-yellow-400 rounded-full"></div>
                </div>
            </Show>
            <Show when=move || state.winner_player_id.get().is_some()>
                    <div class="fixed inset-0 z-[10000] flex items-center justify-center bg-black/60 backdrop-blur-md pointer-events-auto">
                        <div class="bg-slate-900 border-4 border-yellow-500 rounded-3xl p-10 flex flex-col items-center shadow-[0_0_50px_rgba(234,179,8,0.3)] animate-in fade-in zoom-in duration-300">
                            <div class="text-6xl mb-4 text-amber-400 flex justify-center"><Icon kind=IconKind::Trophy /></div>
                            <h2 class="text-5xl font-black text-white mb-2 tracking-tighter">"VICTORY"</h2>
                            <div class="h-1 w-32 bg-yellow-500 mb-6"></div>

                            <p class="text-2xl text-slate-300 mb-8 text-center">
                                {move || {
                                    let winner_id = state.winner_player_id.get().unwrap_or_default();
                                    state.players.get().iter()
                                        .find(|p| p.player_id == winner_id)
                                        .map(|p| p.name.clone())
                                        .unwrap_or_else(|| "Unknown Explorer".to_string())
                                }}
                                <span class="block text-yellow-500 font-bold mt-2">"has colonized the island!"</span>
                            </p>

                            <button
                                class="bg-yellow-600 hover:bg-yellow-500 text-white px-10 py-4 rounded-xl font-black transition-all shadow-lg active:scale-95"
                                on:click=|_| {
                                    // Logic to return to lobby or refresh
                                    let _ = window().location().set_href("/");
                                }
                            >
                                "RETURN TO LOBBY"
                            </button>
                        </div>
                    </div>
                </Show>
            </div> // Close pointer-events wrapper
        </div>
    }
}

/// The turn clock, shared with whatever wants to draw it.
///
/// Display only. The server enforces the deadline and will roll or end the
/// turn itself; these are read off the same shared constants so the bar
/// cannot promise a timeout nobody is enforcing.
#[derive(Clone, Copy)]
pub struct TurnClock {
    /// Seconds left before the server acts.
    pub remaining: Signal<i32>,
}

const AUTO_ROLL_SECS: f64 = shared::TURN_AUTO_ROLL_SECS as f64;
const TURN_LIMIT_SECS: f64 = shared::TURN_LIMIT_SECS as f64;

/// Runs the clock and puts it in context. Draws nothing itself.
#[component]
fn TurnClockProvider(children: Children) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let (elapsed, set_elapsed) = create_signal(0.0f64);
    let started = store_value(js_sys::Date::now());

    // Restart on every turn the server hands out.
    //
    // Not on `current_turn_player`: a signal only notifies when its value
    // changes, so when the same player takes two turns in a row - the last
    // one left in the game, say - that never fires, `started` keeps pointing
    // at the previous turn, and the countdown reads zero for the rest of the
    // game. Entering regular play counts as a fresh start too, since the
    // server only arms its clock once setup is over.
    create_effect(move |_| {
        let _ = state.turn_epoch.get();
        let _ = state.game_phase.get();
        started.set_value(js_sys::Date::now());
        set_elapsed.set(0.0);
    });

    let handle = store_value(None::<leptos_dom::helpers::IntervalHandle>);
    create_effect(move |_| {
        if let Some(h) = handle.get_value() {
            h.clear();
        }
        handle.set_value(
            set_interval_with_handle(
                move || {
                    if state.game_phase.get() == GamePhase::RegularPlay {
                        set_elapsed.set((js_sys::Date::now() - started.get_value()) / 1000.0);
                    }
                },
                std::time::Duration::from_millis(250),
            )
            .ok(),
        );
    });
    on_cleanup(move || {
        if let Some(h) = handle.get_value() {
            h.clear();
        }
    });

    // Before the roll the clock counts the short auto-roll window; after it,
    // the rest of the turn.
    let rolled = move || state.last_dice_roll.get().is_some();
    let limit = move || if rolled() { TURN_LIMIT_SECS } else { AUTO_ROLL_SECS };

    let remaining = Signal::derive(move || (limit() - elapsed.get()).max(0.0).ceil() as i32);

    provide_context(TurnClock { remaining });

    children()
}

#[component]
fn DiscardCardsModal() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let discard_count = move || state.must_discard_count.get().unwrap_or(0);

    // Track how many of each resource to discard
    let (discard_brick, set_discard_brick) = create_signal(0u8);
    let (discard_lumber, set_discard_lumber) = create_signal(0u8);
    let (discard_wool, set_discard_wool) = create_signal(0u8);
    let (discard_grain, set_discard_grain) = create_signal(0u8);
    let (discard_ore, set_discard_ore) = create_signal(0u8);

    // Log resources reactively
    create_effect(move |_| {
        let my_res = state.my_resources.get();
        logging::log!("Discard modal - Resources: Brick={}, Lumber={}, Wool={}, Grain={}, Ore={}",
            my_res.brick, my_res.lumber, my_res.wool, my_res.grain, my_res.ore);
        logging::log!("Must discard: {} cards", discard_count());
    });

    // Reset selections when modal closes
    create_effect(move |_| {
        if state.must_discard_count.get().is_none() {
            set_discard_brick.set(0);
            set_discard_lumber.set(0);
            set_discard_wool.set(0);
            set_discard_grain.set(0);
            set_discard_ore.set(0);
        }
    });

    // Timer: 30 seconds
    let (time_left, set_time_left) = create_signal(30);

    let total_discarded = move || {
        discard_brick.get() + discard_lumber.get() + discard_wool.get() + discard_grain.get() + discard_ore.get()
    };

    let can_confirm = move || total_discarded() == discard_count() as u8;

    let confirm_discard = move |_| {
        if can_confirm() {
            logging::log!("📤 Sending DiscardCards request to server...");
            state.send(ClientRequest::DiscardCards {
                resources: shared::Resources {
                    brick: discard_brick.get(),
                    lumber: discard_lumber.get(),
                    wool: discard_wool.get(),
                    grain: discard_grain.get(),
                    ore: discard_ore.get(),
                }
            });
            // Don't close modal yet - wait for ServerMessage::CardsDiscarded confirmation
            // Modal will be closed in state.rs when we receive CardsDiscarded
            logging::log!("⏳ Waiting for server confirmation...");
            // Reset counters
            set_discard_brick.set(0);
            set_discard_lumber.set(0);
            set_discard_wool.set(0);
            set_discard_grain.set(0);
            set_discard_ore.set(0);
        }
    };

    // Start countdown timer and handle auto-discard
    create_effect(move |prev_handle: Option<Option<leptos_dom::helpers::IntervalHandle>>| {
        // Clean up previous interval if it exists
        if let Some(Some(handle)) = prev_handle {
            handle.clear();
            logging::log!("🧹 Cleared previous timer interval");
        }

        if state.must_discard_count.get().is_some() {
            // Reset timer when modal opens
            set_time_left.set(30);
            logging::log!("⏱️ Starting 30 second discard timer");

            let state_timer = state;
            match set_interval_with_handle(
                move || {
                    // Check if modal is still open
                    if state_timer.must_discard_count.get_untracked().is_none() {
                        // Modal closed, just return without updating signals
                        return;
                    }

                    let current = time_left.get_untracked();
                    if current > 0 {
                        set_time_left.set(current - 1);
                        if current % 5 == 0 {
                            logging::log!("⏱️ Timer: {} seconds remaining", current);
                        }
                    } else {
                        // Time's up! Auto-discard random cards
                        logging::log!("⏰ Timer expired! Auto-discarding cards...");
                        logging::log!("Player ID: {:?}", state_timer.player_id.get_untracked());

                        let my_res = state_timer.my_resources.get_untracked();
                        let target = state_timer.must_discard_count.get_untracked().unwrap_or(0) as u8;

                        logging::log!("Discard target: {} cards", target);
                        logging::log!("My resources: Brick={}, Lumber={}, Wool={}, Grain={}, Ore={}",
                            my_res.brick, my_res.lumber, my_res.wool, my_res.grain, my_res.ore);

                        if target == 0 {
                            logging::log!("❌ No cards to discard, closing modal");
                            state_timer.must_discard_count.set(None);
                            return;
                        }

                        logging::log!("Target: {} cards, Current resources: Brick={}, Lumber={}, Wool={}, Grain={}, Ore={}",
                            target, my_res.brick, my_res.lumber, my_res.wool, my_res.grain, my_res.ore);

                        // Randomly select resources to discard
                        let mut resources = vec![
                            (my_res.brick, "brick"),
                            (my_res.lumber, "lumber"),
                            (my_res.wool, "wool"),
                            (my_res.grain, "grain"),
                            (my_res.ore, "ore"),
                        ];

                        // Filter out resources we don't have
                        resources.retain(|(count, _)| *count > 0);
                        logging::log!("Available resources to discard from: {} types", resources.len());

                        if resources.is_empty() {
                            logging::log!("❌ No resources available to discard!");
                            state_timer.must_discard_count.set(None);
                            return;
                        }

                        let mut discarded = 0u8;
                        let mut brick_d = 0u8;
                        let mut lumber_d = 0u8;
                        let mut wool_d = 0u8;
                        let mut grain_d = 0u8;
                        let mut ore_d = 0u8;

                        // Keep discarding until we reach the target
                        while discarded < target && !resources.is_empty() {
                            // Pick a random resource
                            let idx = (js_sys::Math::random() * resources.len() as f64).floor() as usize;
                            let (_, name) = resources[idx];

                            match name {
                                "brick" => { brick_d += 1; resources[idx].0 -= 1; }
                                "lumber" => { lumber_d += 1; resources[idx].0 -= 1; }
                                "wool" => { wool_d += 1; resources[idx].0 -= 1; }
                                "grain" => { grain_d += 1; resources[idx].0 -= 1; }
                                "ore" => { ore_d += 1; resources[idx].0 -= 1; }
                                _ => {}
                            }

                            if resources[idx].0 == 0 {
                                resources.remove(idx);
                            }
                            discarded += 1;
                        }

                        logging::log!("✅ Auto-discarding: Brick={}, Lumber={}, Wool={}, Grain={}, Ore={}",
                            brick_d, lumber_d, wool_d, grain_d, ore_d);

                        state_timer.send(ClientRequest::DiscardCards {
                            resources: shared::Resources {
                                brick: brick_d,
                                lumber: lumber_d,
                                wool: wool_d,
                                grain: grain_d,
                                ore: ore_d,
                            }
                        });
                        state_timer.must_discard_count.set(None);
                    }
                },
                std::time::Duration::from_secs(1),
            ) {
                Ok(handle) => Some(handle),
                Err(_) => {
                    logging::error!("Failed to create interval");
                    None
                }
            }
        } else {
            logging::log!("Modal closed, no timer needed");
            None
        }
    });

    // Helper to create discard counter row - closures with >= are defined outside view! macro
    // to avoid Leptos parsing issues with >= being interpreted as HTML tag closing
    let discard_counter = move |name: &'static str, color: &'static str,
                                 value: ReadSignal<u8>, setter: WriteSignal<u8>,
                                 get_max: Signal<u8>| {
        // Define closures outside the view! macro to avoid parsing issues with >=
        let dec_disabled = move || value.get() == 0;
        let inc_disabled = move || value.get() >= get_max.get();
        let on_dec = move |_| {
            if value.get() > 0 {
                setter.set(value.get() - 1);
            }
        };
        let on_inc = move |_| {
            if value.get() < get_max.get() {
                setter.set(value.get() + 1);
            }
        };

        view! {
            <div class="flex items-center justify-between bg-slate-800/50 p-3 rounded-lg">
                <div class="flex items-center gap-3">
                    <span class=format!("font-bold {}", color)>{name}</span>
                    <span class="text-xs text-slate-500">"(have " {move || get_max.get()} ")"</span>
                </div>
                <div class="flex items-center gap-2">
                    <button
                        class="w-8 h-8 bg-red-700 hover:bg-red-600 rounded font-bold text-white disabled:opacity-30 disabled:cursor-not-allowed text-lg"
                        on:click=on_dec
                        disabled=dec_disabled
                    >
                        "-"
                    </button>
                    <span class="w-12 text-center font-bold text-white text-lg">{value}</span>
                    <button
                        class="w-8 h-8 bg-green-700 hover:bg-green-600 rounded font-bold text-white disabled:opacity-30 disabled:cursor-not-allowed text-lg"
                        on:click=on_inc
                        disabled=inc_disabled
                    >
                        "+"
                    </button>
                </div>
            </div>
        }
    };

    // Create signals for max values (from player's resources)
    let max_brick = Signal::derive(move || state.my_resources.get().brick);
    let max_lumber = Signal::derive(move || state.my_resources.get().lumber);
    let max_wool = Signal::derive(move || state.my_resources.get().wool);
    let max_grain = Signal::derive(move || state.my_resources.get().grain);
    let max_ore = Signal::derive(move || state.my_resources.get().ore);

    view! {
        <div class="fixed inset-0 bg-black/80 flex items-center justify-center z-[9999] backdrop-blur-sm pointer-events-auto">
            <div class="bg-slate-900 border-2 border-red-600 rounded-2xl p-6 max-w-md w-full mx-4 shadow-2xl">
                <div class="flex justify-between items-center mb-2">
                    <h2 class="text-2xl font-bold text-red-500 flex items-center gap-2"><Icon kind=IconKind::Warning /> "Discard Cards"</h2>
                    <div class=move || {
                        let t = time_left.get();
                        if t <= 10 {
                            "text-2xl font-black text-red-500 animate-pulse"
                        } else {
                            "text-2xl font-black text-yellow-500"
                        }
                    }>
                        {time_left} "s"
                    </div>
                </div>
                <p class="text-slate-300 mb-4">
                    "You must discard " <span class="text-red-400 font-bold">{discard_count}</span> " cards. Select which resources to discard:"
                </p>

                <div class="space-y-3 mb-4">
                    {discard_counter("Brick", "text-red-500", discard_brick, set_discard_brick, max_brick)}
                    {discard_counter("Wood", "text-green-500", discard_lumber, set_discard_lumber, max_lumber)}
                    {discard_counter("Sheep", "text-lime-400", discard_wool, set_discard_wool, max_wool)}
                    {discard_counter("Wheat", "text-yellow-400", discard_grain, set_discard_grain, max_grain)}

                    {discard_counter("Ore", "text-slate-400", discard_ore, set_discard_ore, max_ore)}
                </div>

                <div class="flex justify-between items-center pt-4 border-t border-slate-700">
                    <div class="text-sm text-slate-400">
                        "Selected: " <span class=move || if can_confirm() { "text-green-400 font-bold" } else { "text-red-400 font-bold" }>{total_discarded} " / " {discard_count}</span>
                    </div>
                    <button
                        class="bg-red-600 hover:bg-red-500 text-white px-6 py-2 rounded-lg font-bold transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                        on:click=confirm_discard
                        disabled=move || !can_confirm()
                    >
                        "DISCARD"
                    </button>
                </div>
            </div>
        </div>
    }
}

#[component]
fn RobPlayerModal() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let rob_player = move |victim_id: Uuid| {
        state.send(ClientRequest::StealFromPlayer { victim_id });
        state.must_steal_from_players.set(Vec::new());
    };

    view! {
        <GameDialog title="Take a card">
            <p class="text-[14px] text-[#5c5445] mb-3">
                "The robber is on their land. Pick who loses a card - you will
                 not know which until it is in your hand."
            </p>

            <div class="flex flex-wrap gap-2.5">
                <For
                    each=move || state.must_steal_from_players.get()
                    key=|id| *id
                    children=move |victim_id| {
                        let name = state.player_name(victim_id);
                        let colour = state
                            .players
                            .get()
                            .iter()
                            .find(|p| p.player_id == victim_id)
                            .map(|p| p.colour.hex())
                            .unwrap_or("#94a3b8");
                        let held = state
                            .players
                            .get()
                            .iter()
                            .find(|p| p.player_id == victim_id)
                            .map(|p| p.resource_count)
                            .unwrap_or(0);

                        view! {
                            <button
                                class="hud-tray flex items-center gap-3 px-3 py-2.5 min-w-[190px]
                                       hover:-translate-y-0.5 active:translate-y-0 transition-transform"
                                on:click=move |_| rob_player(victim_id)
                            >
                                <span
                                    class="w-11 h-11 rounded-full shrink-0 flex items-center justify-center
                                           border-[3px] border-white shadow-[0_0_0_2px_rgba(13,62,92,0.6)]"
                                    style=format!("background-color: {colour}")
                                >
                                    <Art name="trade-offerer" alt="" class="w-7 h-7 object-contain" />
                                </span>
                                <span class="text-left">
                                    <span class="block text-[15px] font-black" style=format!("color: {colour}")>
                                        {name}
                                    </span>
                                    <span class="block text-[12px] text-[#7a7263]">
                                        {held} " cards"
                                    </span>
                                </span>
                            </button>
                        }
                    }
                />
            </div>
        </GameDialog>
    }
}

/// The frame every in-game dialog uses: cream card stock on a dimmed table,
/// with the title in the same small caps as the side panels.
#[component]
fn GameDialog(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-[#052f47]/70 pointer-events-auto">
            <div class="hud-tray p-5 max-w-lg w-full mx-4" style="border-width: 3px;">
                <div class="text-[12px] font-black uppercase tracking-[0.18em] text-[#7a7263] mb-2.5">
                    {title}
                </div>
                {children()}
            </div>
        </div>
    }
}

/// A row of resource cards to pick from, used by the cards that ask you to
/// name one. Cards, not buttons with words on them: the rest of the game
/// says "wheat" with a picture of wheat.
#[component]
fn ResourcePicker(
    on_pick: Callback<shared::ResourceType>,
    /// Ringed when it matches.
    #[prop(optional)]
    selected: Option<Signal<Option<shared::ResourceType>>>,
) -> impl IntoView {
    use crate::components::bottom::{CardFace, Res};

    view! {
        <div class="flex items-center justify-center gap-2.5">
            {Res::ALL.map(|k| {
                let res = k.shared();
                let is_on = move || selected.is_some_and(|s| s.get() == Some(res));
                view! {
                    <button
                        class="game-card-pick"
                        title=k.label()
                        on:click=move |_| on_pick.call(res)
                    >
                        <CardFace
                            art=k.art()
                            alt=k.label()
                            size="w-[62px] h-[86px]"
                            selected=Signal::derive(is_on)
                        />
                    </button>
                }
            }).to_vec()}
        </div>
    }
}

/// Offers waiting on an answer from you. Pulled out of the propose-a-trade
/// form because they arrive on other players' turns too, when that form is
/// correctly hidden.
#[component]
pub fn IncomingTrades() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    view! {
        <Show when=move || !state.incoming_trades.get().is_empty()>
            <div class="space-y-2 mt-2 pt-2 border-t border-slate-800">
                <div class="text-[9px] text-slate-400 font-bold uppercase tracking-wider">"Incoming"</div>
                <For
                    each=move || state.incoming_trades.get()
                    key=|trade| trade.offer_id
                    children=move |trade: crate::state::PendingTradeOffer| {
                        view! { <IncomingTradeItem trade=trade /> }
                    }
                />
            </div>
        </Show>
    }
}

/// How long an offer lives, shown as a countdown. The server owns expiry and
/// will send `TradeCancelled`; this is display only, so the two cannot drift.
const TRADE_TIMEOUT_SECONDS: f64 = shared::TRADE_LIFETIME_SECS as f64;

/// Terms to answer somebody else's offer with. Sends a counter, which is a
/// trade from us to the active player - never to another waiting player.
#[component]
fn CounterOfferForm(offer_id: u64, on_done: Callback<()>) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let give = create_rw_signal(shared::Resources::default());
    let want = create_rw_signal(shared::Resources::default());

    let can_afford = move || {
        let (g, mine) = (give.get(), state.my_resources.get());
        g.brick <= mine.brick && g.lumber <= mine.lumber && g.wool <= mine.wool
            && g.grain <= mine.grain && g.ore <= mine.ore
    };
    let non_empty = |r: &shared::Resources| {
        r.brick + r.lumber + r.wool + r.grain + r.ore > 0
    };
    let is_valid = move || non_empty(&give.get()) && non_empty(&want.get()) && can_afford();

    // (label, colour, read the field, write the field)
    type Field = (&'static str, &'static str, fn(&shared::Resources) -> u8, fn(&mut shared::Resources, u8));
    let fields: [Field; 5] = [
        ("Brick", "text-red-400", |r| r.brick, |r, v| r.brick = v),
        ("Wood", "text-green-400", |r| r.lumber, |r, v| r.lumber = v),
        ("Sheep", "text-lime-400", |r| r.wool, |r, v| r.wool = v),
        ("Wheat", "text-yellow-400", |r| r.grain, |r, v| r.grain = v),
        ("Ore", "text-slate-300", |r| r.ore, |r, v| r.ore = v),
    ];

    let row = move |label: &'static str, colour: &'static str,
                    read: fn(&shared::Resources) -> u8,
                    write: fn(&mut shared::Resources, u8),
                    bucket: RwSignal<shared::Resources>,
                    cap: Option<fn(&shared::Resources) -> u8>| {
        view! {
            <div class="flex items-center justify-between text-[10px]">
                <span class=format!("font-bold {}", colour)>{label}</span>
                <div class="flex items-center gap-1">
                    <button
                        class="w-5 h-5 bg-red-700 hover:bg-red-600 rounded text-white font-bold text-xs disabled:opacity-30"
                        disabled=move || read(&bucket.get()) == 0
                        on:click=move |_| bucket.update(|r| {
                            let v = read(r);
                            if v > 0 { write(r, v - 1); }
                        })
                    >"-"</button>
                    <span class="w-4 text-center text-white">{move || read(&bucket.get())}</span>
                    <button
                        class="w-5 h-5 bg-green-700 hover:bg-green-600 rounded text-white font-bold text-xs disabled:opacity-30"
                        disabled=move || match cap {
                            Some(limit) => read(&bucket.get()) >= limit(&state.my_resources.get()),
                            None => read(&bucket.get()) >= 19,
                        }
                        on:click=move |_| bucket.update(|r| {
                            let v = read(r);
                            write(r, v + 1);
                        })
                    >"+"</button>
                </div>
            </div>
        }
    };

    view! {
        <div class="mt-2 bg-slate-800/60 rounded p-2 space-y-2">
            <div class="text-[9px] text-amber-400 font-bold">"YOUR COUNTER - YOU GIVE:"</div>
            <div class="space-y-1">
                {fields.map(|(l, c, r, w)| row(l, c, r, w, give, Some(r))).to_vec()}
            </div>

            <div class="text-[9px] text-amber-400 font-bold">"YOU WANT:"</div>
            <div class="space-y-1">
                {fields.map(|(l, c, r, w)| row(l, c, r, w, want, None)).to_vec()}
            </div>

            <div class="flex gap-1">
                <button
                    class="flex-1 py-1 bg-slate-700 hover:bg-slate-600 rounded text-[10px] font-bold"
                    on:click=move |_| on_done.call(())
                >
                    "CANCEL"
                </button>
                <button
                    class="flex-1 py-1 bg-amber-700 hover:bg-amber-600 rounded text-[10px] font-bold disabled:opacity-40"
                    disabled=move || !is_valid()
                    on:click=move |_| {
                        state.send(ClientRequest::CounterOffer {
                            offer_id,
                            offer: give.get_untracked(),
                            request: want.get_untracked(),
                        });
                        on_done.call(());
                    }
                >
                    "SEND COUNTER"
                </button>
            </div>
        </div>
    }
}

#[component]
fn IncomingTradeItem(trade: crate::state::PendingTradeOffer) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let offer_id = trade.offer_id;
    let received_at = trade.received_at;

    // Timer state - seconds remaining
    let (seconds_left, set_seconds_left) = create_signal(TRADE_TIMEOUT_SECONDS);

    // Tick the countdown. The handle has to be cleared when this row goes
    // away: leptos only runs the effect's own cleanup when it re-runs, so an
    // interval left behind here would keep firing after the row is gone.
    let timer = store_value(None::<leptos_dom::helpers::IntervalHandle>);

    create_effect(move |_| {
        if let Some(handle) = timer.get_value() {
            handle.clear();
        }

        let handle = set_interval_with_handle(
            move || {
                let elapsed = (js_sys::Date::now() - received_at) / 1000.0;
                set_seconds_left.set((TRADE_TIMEOUT_SECONDS - elapsed).max(0.0));
            },
            std::time::Duration::from_secs(1),
        );

        timer.set_value(handle.ok());
    });

    on_cleanup(move || {
        if let Some(handle) = timer.get_value() {
            handle.clear();
        }
    });

    // Whether I have already bid on this offer and am waiting to be picked.
    let i_accepted = move || state.my_accepted_offers.get().contains(&offer_id);

    // A counter is aimed at me alone, so accepting it settles it outright
    // rather than joining a queue of bidders.
    let is_counter = trade.counters.is_some();
    let counterer = trade.proposer_id;

    // Have I already answered this offer with terms of my own?
    let my_counter = move || {
        state.my_counter_offers.get()
            .iter()
            .find(|(original, _)| *original == offer_id)
            .map(|(_, counter)| *counter)
    };

    let (show_counter_form, set_show_counter_form) = create_signal(false);

    let proposer_name = state.players.get()
        .iter()
        .find(|p| p.player_id == trade.proposer_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| format!("Player {}", trade.proposer_id));

    // Check if I can afford what they're requesting
    let can_afford = move || {
        let res = state.my_resources.get();
        trade.requesting.brick <= res.brick &&
        trade.requesting.lumber <= res.lumber &&
        trade.requesting.wool <= res.wool &&
        trade.requesting.grain <= res.grain &&
        trade.requesting.ore <= res.ore
    };

    // Format resources
    let format_resources = |r: &shared::Resources| -> String {
        let mut parts = Vec::new();
        if r.brick > 0 { parts.push(format!("{}x Brick", r.brick)); }
        if r.lumber > 0 { parts.push(format!("{}x Wood", r.lumber)); }
        if r.wool > 0 { parts.push(format!("{}x Sheep", r.wool)); }
        if r.grain > 0 { parts.push(format!("{}x Wheat", r.grain)); }
        if r.ore > 0 { parts.push(format!("{}x Ore", r.ore)); }
        if parts.is_empty() { "nothing".to_string() } else { parts.join(" ") }
    };

    let offering_str = format_resources(&trade.offering);
    let requesting_str = format_resources(&trade.requesting);

    // Calculate progress bar width (percentage remaining)
    let progress_width = move || {
        let remaining = seconds_left.get();
        let percentage = (remaining / TRADE_TIMEOUT_SECONDS) * 100.0;
        format!("{}%", percentage.max(0.0))
    };

    // Timer color based on remaining time
    let timer_color = move || {
        let remaining = seconds_left.get();
        if remaining <= 5.0 {
            "bg-red-600"
        } else if remaining <= 10.0 {
            "bg-yellow-600"
        } else {
            "bg-blue-600"
        }
    };

    view! {
        <div class="bg-blue-900/30 border border-blue-700/50 rounded p-2 text-[9px] relative overflow-hidden">
            // Timer progress bar background
            <div class="absolute bottom-0 left-0 right-0 h-1 bg-slate-700">
                <div
                    class=move || format!("h-full transition-all duration-1000 {}", timer_color())
                    style=move || format!("width: {}", progress_width())
                ></div>
            </div>

            <div class="flex justify-between items-center mb-1">
                <div class="font-bold text-blue-300">
                    {proposer_name}
                    {if is_counter { " (counter)" } else { "" }}
                </div>
                <div class=move || {
                    let remaining = seconds_left.get();
                    if remaining <= 5.0 {
                        "text-red-400 font-bold"
                    } else if remaining <= 10.0 {
                        "text-yellow-400 font-bold"
                    } else {
                        "text-slate-400"
                    }
                }>
                    {move || format!("{}s", seconds_left.get().ceil() as i32)}
                </div>
            </div>
            <div class="text-slate-300">
                <span class="text-green-400">"Gives: "</span>{offering_str}
            </div>
            <div class="text-slate-300">
                <span class="text-red-400">"Wants: "</span>{requesting_str}
            </div>
            {if is_counter {
                // Somebody's counter to my offer: I settle it directly.
                view! {
                    <div class="flex gap-1 mt-2">
                        <button
                            class="flex-1 py-1 bg-red-700 hover:bg-red-600 rounded font-bold"
                            on:click=move |_| {
                                state.send(ClientRequest::CancelTrade { offer_id });
                            }
                        >
                            "REJECT"
                        </button>
                        <button
                            class="flex-1 py-1 bg-green-700 hover:bg-green-600 rounded font-bold disabled:opacity-40"
                            disabled=move || !can_afford()
                            on:click=move |_| {
                                state.send(ClientRequest::ConfirmTrade {
                                    offer_id,
                                    partner_id: counterer,
                                });
                            }
                        >
                            "TRADE"
                        </button>
                    </div>
                }.into_view()
            } else {
                view! {
                    <Show
                        when=move || i_accepted() || my_counter().is_some()
                        fallback=move || view! {
                            <div class="flex gap-1 mt-2">
                                <button
                                    class="flex-1 py-1 bg-red-700 hover:bg-red-600 rounded font-bold"
                                    on:click=move |_| {
                                        state.send(ClientRequest::TradeResponse { offer_id, accept: false });
                                    }
                                >
                                    "DECLINE"
                                </button>
                                <button
                                    class="flex-1 py-1 bg-amber-700 hover:bg-amber-600 rounded font-bold"
                                    on:click=move |_| set_show_counter_form.set(true)
                                >
                                    "COUNTER"
                                </button>
                                <button
                                    class="flex-1 py-1 bg-green-700 hover:bg-green-600 rounded font-bold disabled:opacity-40"
                                    disabled=move || !can_afford()
                                    on:click=move |_| {
                                        state.send(ClientRequest::TradeResponse { offer_id, accept: true });
                                    }
                                >
                                    "ACCEPT"
                                </button>
                            </div>
                        }
                    >
                        // Answered. It is their move now - they may pick
                        // somebody else, so this stays until they settle.
                        <div class="mt-2 space-y-1">
                            <div class="text-green-400 font-bold text-center">
                                {move || if my_counter().is_some() {
                                    "✓ Countered - waiting for them to choose"
                                } else {
                                    "✓ Accepted - waiting for them to choose"
                                }}
                            </div>
                            <button
                                class="w-full py-1 bg-slate-700 hover:bg-slate-600 rounded font-bold"
                                on:click=move |_| {
                                    if let Some(counter_id) = my_counter() {
                                        state.send(ClientRequest::CancelTrade { offer_id: counter_id });
                                    } else {
                                        state.send(ClientRequest::TradeResponse { offer_id, accept: false });
                                    }
                                }
                            >
                                "WITHDRAW"
                            </button>
                        </div>
                    </Show>

                    <Show when=move || show_counter_form.get()>
                        <CounterOfferForm
                            offer_id=offer_id
                            on_done=Callback::new(move |_| set_show_counter_form.set(false))
                        />
                    </Show>
                }.into_view()
            }}
        </div>
    }
}

#[component]
fn YearOfPlentyModal() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    // The two cards you are taking, in the order you picked them.
    let picks = create_rw_signal(Vec::<shared::ResourceType>::new());

    create_effect(move |_| {
        if !state.year_of_plenty_pending.get() {
            picks.set(Vec::new());
        }
    });

    let can_confirm = move || picks.get().len() == 2;

    let confirm = move |_| {
        let p = picks.get_untracked();
        if let [r1, r2] = p[..] {
            state.send(ClientRequest::YearOfPlentyChoice { resource1: r1, resource2: r2 });
            picks.set(Vec::new());
        }
    };

    // Clicking a card adds it; clicking past two replaces the second, so you
    // are never stuck having to clear the whole thing to change your mind.
    let pick = move |res: shared::ResourceType| {
        picks.update(|p| {
            if p.len() < 2 {
                p.push(res);
            } else {
                p[1] = res;
            }
        });
    };

    view! {
        <GameDialog title="Year of plenty">
            <p class="text-[14px] text-[#5c5445] mb-3">
                "Take any two cards from the bank."
            </p>

            <ResourcePicker on_pick=Callback::new(pick) />

            // What you have taken so far, as the cards themselves.
            <div class="flex items-center justify-center gap-2 mt-4 min-h-[76px]">
                <Show
                    when=move || !picks.get().is_empty()
                    fallback=|| view! {
                        <span class="text-[13px] italic text-[#a89e8b]">"Pick two."</span>
                    }
                >
                    {move || picks.get().into_iter().enumerate().map(|(i, res)| {
                        let k = crate::components::bottom::Res::ALL
                            .into_iter()
                            .find(|k| k.shared() == res);
                        view! {
                            <button
                                class="game-card-pick"
                                title="Put it back"
                                on:click=move |_| picks.update(|p| { p.remove(i); })
                            >
                                {k.map(|k| view! {
                                    <crate::components::bottom::CardFace
                                        art=k.art() alt=k.label() size="w-[54px] h-[74px]"
                                    />
                                })}
                            </button>
                        }
                    }).collect_view()}
                </Show>
            </div>

            <div class="flex gap-2 mt-4">
                <button
                    class="game-btn game-btn-cream px-5 h-11 text-[13px] font-black uppercase tracking-wider"
                    on:click=move |_| picks.set(Vec::new())
                >
                    "Clear"
                </button>
                <button
                    class="game-btn game-btn-green flex-1 h-11 text-[14px] font-black uppercase tracking-wider"
                    disabled=move || !can_confirm()
                    on:click=confirm
                >
                    "Take them"
                </button>
            </div>
        </GameDialog>
    }
}

#[component]
fn MonopolyModal() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let selected = create_rw_signal(Option::<shared::ResourceType>::None);

    create_effect(move |_| {
        if !state.monopoly_pending.get() {
            selected.set(None);
        }
    });

    let confirm = move |_| {
        if let Some(resource) = selected.get_untracked() {
            state.send(ClientRequest::MonopolyChoice { resource });
            selected.set(None);
        }
    };

    view! {
        <GameDialog title="Monopoly">
            <p class="text-[14px] text-[#5c5445] mb-3">
                "Name one card. Every other player hands you every one they hold."
            </p>

            <ResourcePicker
                on_pick=Callback::new(move |res| selected.set(Some(res)))
                selected=Signal::derive(move || selected.get())
            />

            <button
                class="game-btn game-btn-green w-full h-11 mt-4 text-[14px] font-black uppercase tracking-wider"
                disabled=move || selected.get().is_none()
                on:click=confirm
            >
                "Call it"
            </button>
        </GameDialog>
    }
}
