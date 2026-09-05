use leptos::*;
use uuid::Uuid;
use crate::components::board::{player_color_hex, Board};
use crate::state::{BuildMode, GameState};
use crate::components::icons::{Icon, IconKind};
use shared::{ClientRequest, GamePhase, PlayerColour};

#[component]
pub fn GamePage() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let (log_open, set_log_open) = create_signal(true);
    let (trade_open, set_trade_open) = create_signal(false);

    // Check if any modal is open - if so, disable pointer events on main UI
    let is_modal_open = move || {
        state.must_discard_count.get().is_some() ||
            !state.must_steal_from_players.get().is_empty() ||
            state.winner_player_id.get().is_some()
    };

    view! {
        <div class="h-screen flex flex-col overflow-hidden bg-slate-950 font-sans relative">
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
            <header class="flex justify-between items-center bg-slate-900/50 border-b border-slate-800 p-4 backdrop-blur-md z-10">
                <div class="flex items-center gap-6">
                    <div>
                        <h1 class="text-2xl font-black italic text-orange-600 tracking-tighter leading-none">"COLONIST"</h1>
                        <span class="text-[9px] text-slate-500 font-mono tracking-widest uppercase">"Live Session"</span>
                    </div>

                </div>
                <div class="flex items-center gap-4">
                    // Offered while the table is short of a full house, so a
                    // game of three does not wait forever for a fourth.
                    <Show when=move || {
                        state.game_phase.get() == GamePhase::WaitingForPlayers
                            && state.players.get().len() >= 3
                    }>
                        <button
                            class="bg-emerald-600 hover:bg-emerald-500 text-white px-4 py-2 rounded-lg font-bold transition-all shadow-lg active:scale-95 text-xs"
                            on:click=move |_| {
                                if let Some(game_id) = state.game_id.get() {
                                    state.send(ClientRequest::StartGame { game_id });
                                }
                            }
                        >
                            "START NOW"
                        </button>
                    </Show>

                    // `pointer-events-auto` because a modal switches the rest
                    // of the UI off, and a player who owes a discard must
                    // still be able to walk away.
                    <button
                    class="pointer-events-auto bg-red-900/40 hover:bg-red-700 text-red-200 px-4 py-2 rounded-lg font-bold transition-all border border-red-800/50 active:scale-95 text-xs"
                    on:click=move |_| {
                        if let Some(game_id) = state.game_id.get() {
                            state.send(ClientRequest::LeaveGame { game_id });
                            //let _ = window().location().set_href("/");
                        }
                    }
                        >
                    "LEAVE GAME"
                    </button>
                    // Debug button - always visible, disabled when not applicable

                    <button
                        class="bg-orange-600 hover:bg-orange-500 text-white px-6 py-2 rounded-lg font-bold transition-all shadow-lg active:scale-95 text-sm"
                        on:click=move |_| state.send(ClientRequest::EndTurn)
                    >
                        {move || if state.is_my_special_build() { "DONE BUILDING" } else { "END TURN" }}
                    </button>
                </div>
            </header>

            <div class="flex-1 flex overflow-hidden min-h-0">
                <aside  class="w-72 bg-slate-900/30 border-r border-slate-800
                                flex flex-col flex-shrink-0 overflow-hidden">
                    <div class="p-4 flex-1 overflow-y-auto min-h-0 custom-scrollbar space-y-6 pb-10">
                        <div>
                            <h3 class="text-slate-500 font-bold text-[10px] uppercase tracking-[0.2em] mb-3">"Players"</h3>
                            <div class="space-y-2">
                                <For
                                    each=move || state.players.get()
                                    key=|player| player.player_id
                                    children=move |player| {
                                        let is_me = Some(player.player_id) == state.player_id.get();
                                        let player_clone = player.clone();
                                        let is_active = move || player_clone.player_id == state.current_turn_player.get();

                                        let score_signal = create_read_slice(
                                        state.players,
                                        move |players| {
                                            players.iter()
                                                .find(|p| p.player_id == player.player_id)
                                                .map(|p| (p.victory_points as i32, state.secret_victory_points.get() ))
                                                .unwrap_or((0, 0))
                                        }
                                        );

                                        let player_id = player.player_id;
                                        let resource_count = create_read_slice(
                                            state.players,
                                            move |players| {
                                                players.iter()
                                                    .find(|p| p.player_id == player_id)
                                                    .map(|p| p.resource_count as i32)
                                                    .unwrap_or(0)
                                            }
                                        );

                                        let dev_card_count = create_read_slice(
                                            state.players,
                                            move |players| {
                                                players.iter()
                                                    .find(|p| p.player_id == player_id)
                                                    .map(|p| p.dev_card_count as i32)
                                                    .unwrap_or(0)
                                            }
                                        );

                                        let knights_played = create_read_slice(
                                            state.players,
                                            move |players| {
                                                players.iter()
                                                    .find(|p| p.player_id == player_id)
                                                    .map(|p| p.knights_played as i32)
                                                    .unwrap_or(0)
                                            }
                                        );

                                        let roads_count = create_read_slice(
                                            state.players,
                                            move |players| {
                                                players.iter()
                                                    .find(|p| p.player_id == player_id)
                                                    .map(|p| p.roads_count as i32)
                                                    .unwrap_or(0)
                                            }
                                        );

                                        let has_longest_road = create_read_slice(
                                            state.players,
                                            move |players| {
                                                players.iter()
                                                    .find(|p| p.player_id == player_id)
                                                    .map(|p| p.has_longest_road)
                                                    .unwrap_or(false)
                                            }
                                        );

                                        let has_largest_army = create_read_slice(
                                            state.players,
                                            move |players| {
                                                players.iter()
                                                    .find(|p| p.player_id == player_id)
                                                    .map(|p| p.has_largest_army)
                                                    .unwrap_or(false)
                                            }
                                        );

                                        let display_name = if is_me {
                                            format!("{} (You)", player.name)
                                        } else {
                                            player.name.clone()
                                        };

                                        view! {
                                            <PlayerTagDynamic
                                                name=display_name
                                                score=score_signal
                                                is_active=is_active
                                                colour=player.colour
                                                is_me=player.player_id == state.player_id.get().unwrap_or(Uuid::nil())
                                                resource_count=resource_count
                                                dev_card_count=dev_card_count
                                                knights_played=knights_played
                                                roads_count=roads_count
                                                has_longest_road=has_longest_road
                                                has_largest_army=has_largest_army
                                            />
                                        }
                                    }
                                />
                            </div>
                        </div>

                        // Development cards used to be listed here; they are
                        // part of your hand under the board now, beside your
                        // resources.
                    </div>

                </aside>

                // The board column fills the space rather than floating in
                // it: no scrolling, no dead band above and below.
                <div class="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden p-3 gap-2">
                    <TurnTimer />
                    <div
                        class="flex-1 min-h-0 min-w-0 rounded-2xl overflow-hidden ring-1 ring-sky-800/40 shadow-[inset_0_2px_24px_rgba(0,0,0,0.55)]"
                        style="background: radial-gradient(ellipse 72% 78% at 50% 45%, #1d5b86 0%, #134366 55%, #0a2136 100%);"
                    >
                        <Board />
                    </div>
                    // What you can build on the left, what you hold on the
                    // right, both under the board where your eyes already are
                    // when you are deciding what to do.
                    <div class="shrink-0 flex items-end justify-between gap-3">
                        <BuildBar />
                        <div class="relative flex items-end gap-2">
                            // The trade drawer opens upward from here, so the
                            // offer you are building sits directly above the
                            // hand you are building it from.
                            <div class="absolute bottom-full right-0 mb-2 w-[340px] z-30">
                                <Show when=move || trade_open.get()>
                                    <div class="rounded-xl border border-slate-700 bg-slate-900/95 backdrop-blur-md shadow-2xl animate-in slide-in-from-bottom-2 duration-150">
                                        <TradePanel on_close=Callback::new(move |_| set_trade_open.set(false)) />
                                    </div>
                                </Show>
                            </div>

                            <button
                                class=move || format!(
                                    "h-12 px-3 rounded-xl border-2 text-[10px] font-bold uppercase tracking-wider transition-all {}",
                                    if trade_open.get() {
                                        "bg-emerald-600 border-emerald-400 text-white"
                                    } else {
                                        "bg-slate-900/70 border-slate-700 text-slate-300 hover:border-slate-500"
                                    }
                                )
                                title="Open the trade panel"
                                on:click=move |_| set_trade_open.update(|o| *o = !*o)
                            >
                                "Trade"
                            </button>

                            <DevCardHand />
                            <ResourceHand />
                        </div>
                    </div>
                </div>

                // The log is worth having but not worth a permanent fifth of
                // the screen, so it collapses to a spine and hands the width
                // back to the board.
                <aside class=move || format!(
                    "bg-slate-900/30 border-l border-slate-800 flex flex-col overflow-hidden shrink-0 transition-[width] duration-200 {}",
                    if log_open.get() { "w-64" } else { "w-10" }
                )>
                    <div class="flex items-center justify-between gap-2 p-2 border-b border-slate-800 shrink-0">
                        <Show when=move || log_open.get()>
                            <h3 class="text-slate-500 font-bold text-[10px] uppercase tracking-[0.2em] pl-2 truncate">"Event Log"</h3>
                        </Show>
                        <button
                            class="w-6 h-6 shrink-0 rounded text-slate-500 hover:text-white hover:bg-slate-800 font-bold text-xs transition-colors"
                            title=move || if log_open.get() { "Hide the event log" } else { "Show the event log" }
                            on:click=move |_| set_log_open.update(|o| *o = !*o)
                        >
                            {move || if log_open.get() { "›" } else { "‹" }}
                        </button>
                    </div>

                    <Show
                        when=move || log_open.get()
                        fallback=move || view! {
                            // Collapsed: a vertical label, so the strip still
                            // says what it is.
                            <div class="flex-1 flex items-start justify-center pt-3">
                                <span class="text-[9px] uppercase tracking-[0.3em] text-slate-600 font-bold [writing-mode:vertical-rl]">
                                    "Event Log"
                                </span>
                            </div>
                        }
                    >
                        <div class="flex-1 overflow-y-auto p-3 space-y-1.5 custom-scrollbar min-h-0">
                            <For
                                each=move || state.messages.get().into_iter().rev()
                                key=|msg| msg.clone()
                                children=move |msg| view! {
                                    <div class="text-[11px] font-mono text-slate-400 border-l-2 border-slate-700 pl-2 py-1 bg-slate-800/20">
                                        {msg}
                                    </div>
                                }
                            />
                            <Show when=move || state.messages.get().is_empty()>
                                <div class="text-xs text-slate-600 italic">"Waiting for actions..."</div>
                            </Show>
                        </div>
                    </Show>
                </aside>
            </div>

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

const AUTO_ROLL_SECS: f64 = shared::TURN_AUTO_ROLL_SECS as f64;
const TURN_LIMIT_SECS: f64 = shared::TURN_LIMIT_SECS as f64;

/// The turn clock, as a bar above the board.
///
/// Display only. The server enforces the deadline and will roll or end the
/// turn itself; this just shows what it is about to do, off the same shared
/// constants so the bar cannot promise a timeout that is not coming.
#[component]
fn TurnTimer() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let (elapsed, set_elapsed) = create_signal(0.0f64);
    let started = store_value(js_sys::Date::now());

    // Restart whenever the turn changes hands.
    create_effect(move |_| {
        let _ = state.current_turn_player.get();
        started.set_value(js_sys::Date::now());
        set_elapsed.set(0.0);
    });

    let running = move || state.game_phase.get() == GamePhase::RegularPlay;
    let my_turn = move || state.player_id.get() == Some(state.current_turn_player.get());
    let rolled = move || state.last_dice_roll.get().is_some();

    let handle = store_value(None::<leptos_dom::helpers::IntervalHandle>);
    create_effect(move |_| {
        if let Some(h) = handle.get_value() {
            h.clear();
        }
        let h = set_interval_with_handle(
            move || {
                if running() {
                    set_elapsed.set((js_sys::Date::now() - started.get_value()) / 1000.0);
                }
            },
            std::time::Duration::from_millis(250),
        )
        .ok();
        handle.set_value(h);
    });
    on_cleanup(move || {
        if let Some(h) = handle.get_value() {
            h.clear();
        }
    });

    // Before the roll the bar counts down the shorter auto-roll window, after
    // it the rest of the turn.
    let fraction = move || {
        let secs = elapsed.get();
        if rolled() {
            (1.0 - (secs / TURN_LIMIT_SECS)).clamp(0.0, 1.0)
        } else {
            (1.0 - (secs / AUTO_ROLL_SECS)).clamp(0.0, 1.0)
        }
    };
    let remaining = move || {
        let secs = elapsed.get();
        let limit = if rolled() { TURN_LIMIT_SECS } else { AUTO_ROLL_SECS };
        (limit - secs).max(0.0).ceil() as i32
    };

    view! {
        <Show when=running>
            <div class="shrink-0 flex items-center gap-3">
                <span class=move || format!(
                    "text-[10px] font-bold uppercase tracking-[0.2em] shrink-0 {}",
                    if my_turn() { "text-orange-400" } else { "text-slate-500" }
                )>
                    {move || if my_turn() {
                        if rolled() { "Your turn".to_string() } else { "Roll".to_string() }
                    } else {
                        format!("{}'s turn", state.player_name(state.current_turn_player.get()))
                    }}
                </span>
                <div class="flex-1 h-1.5 bg-slate-800 rounded-full overflow-hidden">
                    <div
                        class=move || format!(
                            "h-full rounded-full transition-[width] duration-200 {}",
                            if remaining() <= 5 { "bg-red-500" }
                            else if remaining() <= 15 { "bg-amber-500" }
                            else if my_turn() { "bg-orange-500" }
                            else { "bg-slate-600" }
                        )
                        style=move || format!("width: {:.1}%", fraction() * 100.0)
                    ></div>
                </div>
                <span class="text-[10px] font-mono text-slate-400 tabular-nums w-8 text-right shrink-0">
                    {move || format!("{}s", remaining())}
                </span>
            </div>
        </Show>
    }
}

/// Everything you can buy, in one row under the board. Hovering a button
/// shows what it costs, so the prices do not have to be memorised or looked
/// up somewhere else on the page.
#[component]
fn BuildBar() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    // (brick, lumber, wool, grain, ore)
    const ROAD: [u8; 5] = [1, 1, 0, 0, 0];
    const SETTLEMENT: [u8; 5] = [1, 1, 1, 1, 0];
    const CITY: [u8; 5] = [0, 0, 0, 2, 3];
    const DEV_CARD: [u8; 5] = [0, 0, 1, 1, 1];

    let affords = move |cost: [u8; 5]| {
        let r = state.my_resources.get();
        r.brick >= cost[0] && r.lumber >= cost[1] && r.wool >= cost[2]
            && r.grain >= cost[3] && r.ore >= cost[4]
    };

    // During initial placement you place for free, and only these two.
    let placing = move || state.game_phase.get().is_initial_phase();
    let my_turn = move || state.player_id.get() == Some(state.current_turn_player.get());

    let enabled = move |mode: Option<BuildMode>, cost: [u8; 5]| {
        if placing() {
            return my_turn() && matches!(mode, Some(BuildMode::Settlement) | Some(BuildMode::Road));
        }
        state.can_build_now() && (affords(cost) || state.free_roads_remaining.get() > 0)
    };

    view! {
        <div class="flex items-end gap-1.5 bg-slate-900/70 backdrop-blur-md px-2 py-1.5 rounded-xl border border-slate-700/70 shadow-2xl">
            <BuildButton
                label="Road" kind=IconKind::Road mode=Some(BuildMode::Road)
                cost=ROAD tint="hover:border-emerald-500 hover:text-emerald-300"
                active_tint="border-emerald-500 text-emerald-300 bg-emerald-600/20"
                enabled=Signal::derive(move || enabled(Some(BuildMode::Road), ROAD))
            />
            <BuildButton
                label="Settlement" kind=IconKind::Settlement mode=Some(BuildMode::Settlement)
                cost=SETTLEMENT tint="hover:border-orange-500 hover:text-orange-300"
                active_tint="border-orange-500 text-orange-300 bg-orange-600/20"
                enabled=Signal::derive(move || enabled(Some(BuildMode::Settlement), SETTLEMENT))
            />
            <BuildButton
                label="City" kind=IconKind::City mode=Some(BuildMode::City)
                cost=CITY tint="hover:border-purple-500 hover:text-purple-300"
                active_tint="border-purple-500 text-purple-300 bg-purple-600/20"
                enabled=Signal::derive(move || enabled(Some(BuildMode::City), CITY))
            />
            <BuildButton
                label="Dev Card" kind=IconKind::DevCard mode=None
                cost=DEV_CARD tint="hover:border-sky-500 hover:text-sky-300"
                active_tint=""
                enabled=Signal::derive(move || enabled(None, DEV_CARD))
            />
        </div>
    }
}

/// One purchase. `mode` of `None` buys a development card outright; anything
/// else arms a placement mode for the next board click.
#[component]
fn BuildButton(
    label: &'static str,
    kind: IconKind,
    mode: Option<BuildMode>,
    cost: [u8; 5],
    tint: &'static str,
    active_tint: &'static str,
    enabled: Signal<bool>,
) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let armed = move || mode.is_some_and(|m| state.build_mode.get() == m);

    let parts: Vec<(&'static str, &'static str, u8)> = vec![
        ("Brick", "bg-orange-400 border-orange-600", cost[0]),
        ("Wood", "bg-emerald-500 border-emerald-700", cost[1]),
        ("Sheep", "bg-lime-400 border-lime-600", cost[2]),
        ("Wheat", "bg-amber-400 border-amber-600", cost[3]),
        ("Ore", "bg-slate-400 border-slate-600", cost[4]),
    ];

    view! {
        <div class="relative group">
            <button
                class=move || format!(
                    "w-[62px] h-[54px] flex flex-col items-center justify-center gap-0.5 rounded-lg border-2 transition-all                      disabled:grayscale disabled:opacity-40 disabled:!border-slate-800 disabled:!text-slate-600 disabled:cursor-not-allowed {} {}",
                    if armed() { active_tint } else { "border-slate-700 text-slate-300" },
                    tint,
                )
                disabled=move || !enabled.get()
                on:click=move |_| match mode {
                    Some(m) => state.build_mode.update(|current| {
                        *current = if *current == m { BuildMode::None } else { m };
                    }),
                    None => state.send(ClientRequest::BuyDevelopmentCard),
                }
            >
                <Icon kind=kind size="w-5 h-5" />
                <span class="text-[8px] font-bold uppercase tracking-wider leading-none">{label}</span>
            </button>

            // Cost card, on hover. `pointer-events-none` so it can never sit
            // between the cursor and the button underneath it.
            <div class="pointer-events-none absolute bottom-full left-0 mb-2 hidden group-hover:block z-50">
                <div class="bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-2 shadow-2xl whitespace-nowrap">
                    <div class="text-[9px] font-bold uppercase tracking-wider text-slate-400 mb-1.5">
                        {label} " costs"
                    </div>
                    // One card per unit, drawn like the cards in your hand
                    // only slightly smaller: "brick brick" reads as a price
                    // faster than "2 x brick" does.
                    <div class="flex items-end gap-1">
                        {parts.into_iter()
                            .filter(|(_, _, n)| *n > 0)
                            .flat_map(|(name, colour, n)| {
                                (0..n).map(move |_| view! {
                                    <span
                                        class=format!(
                                            "flex items-end justify-center w-10 h-10 rounded-md border-b-4 shadow {colour}"
                                        )
                                        title=name
                                    >
                                        <span class="text-[7px] font-bold uppercase tracking-wide text-black/70 pb-0.5">
                                            {name}
                                        </span>
                                    </span>
                                }).collect::<Vec<_>>()
                            })
                            .collect_view()}
                    </div>
                </div>
            </div>
        </div>
    }
}

/// The trade panel: build an offer, look at it, send it.
///
/// One builder for both kinds of trade, because they are the same gesture -
/// put resources on each side. Lives in a drawer above your hand, so the
/// cards you are spending are in view while you spend them.
#[component]
fn TradePanel(on_close: Callback<()>) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let can_trade = move || {
        state.can_build_now() && state.game_phase.get() == GamePhase::RegularPlay
    };

    view! {
        <div class="p-3 space-y-2 max-h-[60vh] overflow-y-auto custom-scrollbar">
            <div class="flex items-center justify-between">
                <span class="font-bold text-[10px] uppercase tracking-[0.2em] text-slate-400">"Trade"</span>
                <button
                    class="w-5 h-5 rounded text-slate-500 hover:text-white hover:bg-slate-800 text-xs font-bold leading-none"
                    title="Close"
                    on:click=move |_| on_close.call(())
                >"\u{00d7}"</button>
            </div>

            <Show
                when=can_trade
                fallback=move || view! {
                    <div class="text-[10px] text-slate-500 italic py-2 text-center">
                        "You can trade on your turn, after rolling."
                    </div>
                }
            >
                <TradeBuilder />
            </Show>
            <IncomingTrades />
        </div>
    }
}

/// A pile of resources on one side of a trade being assembled.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Basket {
    brick: u8,
    lumber: u8,
    wool: u8,
    grain: u8,
    ore: u8,
}

impl Basket {
    fn total(&self) -> u8 {
        self.brick + self.lumber + self.wool + self.grain + self.ore
    }
    fn get(&self, k: Res) -> u8 {
        match k {
            Res::Brick => self.brick,
            Res::Wood => self.lumber,
            Res::Sheep => self.wool,
            Res::Wheat => self.grain,
            Res::Ore => self.ore,
        }
    }
    fn set(&mut self, k: Res, v: u8) {
        match k {
            Res::Brick => self.brick = v,
            Res::Wood => self.lumber = v,
            Res::Sheep => self.wool = v,
            Res::Wheat => self.grain = v,
            Res::Ore => self.ore = v,
        }
    }
    fn to_shared(self) -> shared::Resources {
        shared::Resources {
            brick: self.brick,
            lumber: self.lumber,
            wool: self.wool,
            grain: self.grain,
            ore: self.ore,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Res {
    Brick,
    Wood,
    Sheep,
    Wheat,
    Ore,
}

impl Res {
    const ALL: [Res; 5] = [Res::Brick, Res::Wood, Res::Sheep, Res::Wheat, Res::Ore];

    fn label(self) -> &'static str {
        match self {
            Res::Brick => "Brick",
            Res::Wood => "Wood",
            Res::Sheep => "Sheep",
            Res::Wheat => "Wheat",
            Res::Ore => "Ore",
        }
    }
    fn tint(self) -> &'static str {
        match self {
            Res::Brick => "bg-orange-400 border-orange-600",
            Res::Wood => "bg-emerald-500 border-emerald-700",
            Res::Sheep => "bg-lime-400 border-lime-600",
            Res::Wheat => "bg-amber-400 border-amber-600",
            Res::Ore => "bg-slate-400 border-slate-600",
        }
    }
    fn shared(self) -> shared::ResourceType {
        match self {
            Res::Brick => shared::ResourceType::Brick,
            Res::Wood => shared::ResourceType::Wood,
            Res::Sheep => shared::ResourceType::Sheep,
            Res::Wheat => shared::ResourceType::Wheat,
            Res::Ore => shared::ResourceType::Ore,
        }
    }
    fn in_hand(self, r: &shared::Resources) -> u8 {
        match self {
            Res::Brick => r.brick,
            Res::Wood => r.lumber,
            Res::Sheep => r.wool,
            Res::Wheat => r.grain,
            Res::Ore => r.ore,
        }
    }
}

/// One resource card. Click adds it to its side, right-click takes it back.
/// The count rides on a badge so the card stays a recognisable colour block
/// rather than turning into a line of text.
#[component]
fn TradeCard(
    kind: Res,
    count: Signal<u8>,
    enabled: Signal<bool>,
    on_add: Callback<Res>,
    on_remove: Callback<Res>,
) -> impl IntoView {
    // Hoisted: `>` inside the view macro parses as a closing tag.
    let has_any = move || count.get() > 0;

    view! {
        <button
            class=move || format!(
                "relative flex items-end justify-center w-11 h-14 rounded-md border-b-4 shadow transition-all {} {}",
                kind.tint(),
                if enabled.get() { "hover:-translate-y-0.5" } else { "grayscale opacity-40 cursor-not-allowed" },
            )
            disabled=move || !enabled.get()
            title=move || format!("{} - click to add, right-click to remove", kind.label())
            on:click=move |_| on_add.call(kind)
            on:contextmenu=move |ev| {
                ev.prevent_default();
                on_remove.call(kind);
            }
        >
            <span class="text-[7px] font-bold uppercase tracking-wide text-black/70 pb-1">
                {kind.label()}
            </span>
            <Show when=has_any>
                <span class="absolute -top-1.5 -right-1.5 min-w-[18px] h-[18px] px-1 rounded-full bg-slate-950 border border-slate-600 text-[10px] font-black text-white flex items-center justify-center tabular-nums">
                    {move || count.get()}
                </span>
            </Show>
        </button>
    }
}

/// The wildcard: a card you have not named. Putting one in a trade is an
/// invitation to negotiate - the other player answers by countering with
/// something concrete in its place, rather than accepting as-is.
#[component]
fn AnyCard(
    count: Signal<u8>,
    on_add: Callback<()>,
    on_remove: Callback<()>,
) -> impl IntoView {
    let has_any = move || count.get() > 0;

    view! {
        <button
            class="relative flex flex-col items-center justify-center w-11 h-14 rounded-md border-b-4 border-slate-500 bg-gradient-to-br from-slate-300 to-slate-400 shadow transition-all hover:-translate-y-0.5"
            title="Any card - they choose which, by countering"
            on:click=move |_| on_add.call(())
            on:contextmenu=move |ev| {
                ev.prevent_default();
                on_remove.call(());
            }
        >
            <span class="text-lg font-black text-slate-700 leading-none">"?"</span>
            <span class="text-[6px] font-bold uppercase tracking-wide text-black/60">"Any"</span>
            <Show when=has_any>
                <span class="absolute -top-1.5 -right-1.5 min-w-[18px] h-[18px] px-1 rounded-full bg-slate-950 border border-slate-600 text-[10px] font-black text-white flex items-center justify-center tabular-nums">
                    {move || count.get()}
                </span>
            </Show>
        </button>
    }
}

/// One half of the offer under construction, drawn as the cards themselves.
#[component]
fn TradeSide(
    label: &'static str,
    accent: &'static str,
    basket: Signal<Basket>,
) -> impl IntoView {
    let has_any = move || basket.get().total() > 0;

    view! {
        <div class="flex items-center gap-2 min-h-[26px]">
            <span class=format!("text-[9px] font-bold uppercase tracking-wider w-14 shrink-0 {accent}")>
                {label}
            </span>
            <Show
                when=has_any
                fallback=move || view! {
                    <span class="text-[10px] text-slate-600 italic">"nothing yet"</span>
                }
            >
                <div class="flex flex-wrap gap-0.5">
                    {move || Res::ALL.into_iter().flat_map(|k| {
                        (0..basket.get().get(k)).map(move |_| view! {
                            <span
                                class=format!("w-4 h-6 rounded-sm border-b-2 {}", k.tint())
                                title=k.label()
                            ></span>
                        }).collect::<Vec<_>>()
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}

/// Assemble an offer: what you give, what you want, then send it. If the two
/// sides happen to be a legal bank ratio it goes to the bank instead of the
/// table, so there is nothing extra to learn.
#[component]
fn TradeBuilder() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let give = create_rw_signal(Basket::default());
    let want = create_rw_signal(Basket::default());
    // "Any card": a card you have not named, for the other player to fill in.
    let give_any = create_rw_signal(0u8);
    let want_any = create_rw_signal(0u8);

    let clear = move || {
        give.set(Basket::default());
        want.set(Basket::default());
        give_any.set(0);
        want_any.set(0);
    };

    // You cannot offer what you do not hold.
    let spare = move |k: Res| {
        k.in_hand(&state.my_resources.get()).saturating_sub(give.get().get(k))
    };

    let add_give = Callback::new(move |k: Res| {
        if spare(k) > 0 {
            give.update(|b| b.set(k, b.get(k) + 1));
        }
    });
    let del_give = Callback::new(move |k: Res| {
        give.update(|b| b.set(k, b.get(k).saturating_sub(1)));
    });
    let add_want = Callback::new(move |k: Res| want.update(|b| b.set(k, b.get(k) + 1)));
    let del_want = Callback::new(move |k: Res| {
        want.update(|b| b.set(k, b.get(k).saturating_sub(1)));
    });

    // A bank trade is just the special case of n of one kind for exactly one
    // of another, at whatever ratio your harbours allow.
    let bank_deal = move || -> Option<(Res, Res)> {
        if give_any.get() > 0 || want_any.get() > 0 {
            return None;
        }
        let (g, w) = (give.get(), want.get());
        if w.total() != 1 {
            return None;
        }
        let taken = Res::ALL.into_iter().find(|k| w.get(*k) == 1)?;
        let given: Vec<Res> = Res::ALL.into_iter().filter(|k| g.get(*k) > 0).collect();
        if given.len() != 1 {
            return None;
        }
        let only = given[0];
        let ratio = state.get_best_ratio(only.shared());
        (g.get(only) == ratio && only != taken).then_some((only, taken))
    };

    // A wildcard counts as content, but not on both sides at once - that
    // would be an offer with nothing named in it at all.
    let ready = move || {
        let g = give.get().total() + give_any.get();
        let w = want.get().total() + want_any.get();
        g > 0 && w > 0 && !(give_any.get() > 0 && want_any.get() > 0)
    };
    let nothing_picked = move || {
        give.get().total() + want.get().total() + give_any.get() + want_any.get() == 0
    };
    let not_ready = move || !ready();
    let any_accepters = move || !state.my_trade_accepters.get().is_empty();
    let has_offer = move || state.my_pending_trade.get().is_some();

    view! {
        <div class="space-y-2">
            <div class="rounded-lg bg-slate-950/60 border border-slate-800 p-2 space-y-1.5">
                <TradeSide label="You give" accent="text-red-300" basket=give.into() />
                <div class="flex items-center gap-2">
                    <div class="flex-1 h-px bg-slate-800"></div>
                    <span class="text-slate-500 text-sm font-black leading-none">"\u{21c5}"</span>
                    <div class="flex-1 h-px bg-slate-800"></div>
                </div>
                <TradeSide label="You get" accent="text-emerald-300" basket=want.into() />
            </div>

            <div class="space-y-1">
                <div class="text-[9px] font-bold uppercase tracking-wider text-emerald-400">"Ask for"</div>
                <div class="flex gap-1">
                    {Res::ALL.into_iter().map(|k| view! {
                        <TradeCard
                            kind=k
                            count=Signal::derive(move || want.get().get(k))
                            enabled=Signal::derive(|| true)
                            on_add=add_want
                            on_remove=del_want
                        />
                    }).collect_view()}
                    <AnyCard
                        count=want_any.into()
                        on_add=Callback::new(move |_| want_any.update(|n| *n += 1))
                        on_remove=Callback::new(move |_| want_any.update(|n| *n = n.saturating_sub(1)))
                    />
                </div>
            </div>

            <div class="space-y-1">
                <div class="text-[9px] font-bold uppercase tracking-wider text-red-400">
                    "Offer from your hand"
                </div>
                <div class="flex gap-1">
                    {Res::ALL.into_iter().map(|k| view! {
                        <div class="flex flex-col items-center">
                            <TradeCard
                                kind=k
                                count=Signal::derive(move || give.get().get(k))
                                enabled=Signal::derive(move || spare(k) > 0)
                                on_add=add_give
                                on_remove=del_give
                            />
                            // What is left in hand, so you can see the cost.
                            <span class="text-[9px] text-slate-500 tabular-nums mt-0.5">
                                {move || spare(k)}
                            </span>
                        </div>
                    }).collect_view()}
                    <div class="flex flex-col items-center">
                        <AnyCard
                            count=give_any.into()
                            on_add=Callback::new(move |_| give_any.update(|n| *n += 1))
                            on_remove=Callback::new(move |_| give_any.update(|n| *n = n.saturating_sub(1)))
                        />
                        <span class="text-[9px] text-slate-500 mt-0.5">"?"</span>
                    </div>
                </div>
            </div>

            <div class="flex gap-1.5 pt-0.5">
                <button
                    class="px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 text-[10px] font-bold uppercase tracking-wider transition-colors disabled:opacity-40"
                    disabled=nothing_picked
                    on:click=move |_| clear()
                >
                    "Clear"
                </button>

                <Show
                    when=has_offer
                    fallback=move || view! {
                        <button
                            class=move || format!(
                                "flex-1 py-2 rounded-lg text-[11px] font-black uppercase tracking-wider transition-all {}",
                                if bank_deal().is_some() {
                                    "bg-sky-600 hover:bg-sky-500 text-white"
                                } else if ready() {
                                    "bg-emerald-600 hover:bg-emerald-500 text-white"
                                } else {
                                    "bg-slate-800 text-slate-600 cursor-not-allowed"
                                }
                            )
                            disabled=not_ready
                            on:click=move |_| {
                                if let Some((g, w)) = bank_deal() {
                                    state.send(ClientRequest::BankTrade {
                                        give: g.shared(),
                                        receive: w.shared(),
                                    });
                                } else {
                                    state.send(ClientRequest::TradeOffer {
                                        target_player_id: None,
                                        offer: give.get_untracked().to_shared(),
                                        request: want.get_untracked().to_shared(),
                                        offer_any: give_any.get_untracked(),
                                        request_any: want_any.get_untracked(),
                                    });
                                }
                                clear();
                            }
                        >
                            {move || if bank_deal().is_some() {
                                "Trade with bank"
                            } else if ready() {
                                "Offer to players"
                            } else {
                                "Pick both sides"
                            }}
                        </button>
                    }
                >
                    <div class="flex-1 flex gap-1.5">
                        <span class="flex-1 flex items-center justify-center text-[10px] font-bold text-amber-400">
                            {move || {
                                let n = state.my_trade_accepters.get().len();
                                if n == 0 { "Offer sent...".to_string() } else { format!("{n} accepted") }
                            }}
                        </span>
                        <button
                            class="px-3 py-2 rounded-lg bg-red-800 hover:bg-red-700 text-white text-[10px] font-bold uppercase tracking-wider"
                            on:click=move |_| {
                                if let Some(id) = state.my_pending_trade.get_untracked() {
                                    state.send(ClientRequest::CancelTrade { offer_id: id });
                                }
                            }
                        >
                            "Cancel"
                        </button>
                    </div>
                </Show>
            </div>

            <Show when=any_accepters>
                <div class="space-y-1 pt-1 border-t border-slate-800">
                    <div class="text-[9px] font-bold uppercase tracking-wider text-slate-400">
                        "Pick who to trade with"
                    </div>
                    <For
                        each=move || state.my_trade_accepters.get()
                        key=|id| *id
                        children=move |accepter_id| {
                            let name = state.player_name(accepter_id);
                            view! {
                                <button
                                    class="w-full py-1.5 px-2 rounded-md bg-emerald-700 hover:bg-emerald-600 text-white text-[10px] font-bold flex items-center justify-between"
                                    on:click=move |_| {
                                        if let Some(id) = state.my_pending_trade.get_untracked() {
                                            state.send(ClientRequest::ConfirmTrade {
                                                offer_id: id,
                                                partner_id: accepter_id,
                                            });
                                        }
                                    }
                                >
                                    <span>{name}</span>
                                    <span class="opacity-70">"Trade \u{25b8}"</span>
                                </button>
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

/// Development cards you hold, as a hand beside your resources.
///
/// Hidden entirely when you have none, rather than showing an empty shelf.
/// Cards you cannot play right now are dimmed: one card per turn, and never
/// the turn you drew it. Victory points are the exception - they are never
/// played, so they are never dimmed and never clickable, they just sit there
/// being worth a point.
#[component]
fn DevCardHand() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let icon_for = |card: &shared::DevCardType| match card {
        shared::DevCardType::Knight => IconKind::Knight,
        shared::DevCardType::VictoryPoint => IconKind::Trophy,
        shared::DevCardType::RoadBuilding => IconKind::Road,
        shared::DevCardType::Monopoly => IconKind::Coins,
        shared::DevCardType::YearOfPlenty => IconKind::Wheat,
    };
    let short = |card: &shared::DevCardType| match card {
        shared::DevCardType::Knight => "Knight",
        shared::DevCardType::VictoryPoint => "Point",
        shared::DevCardType::RoadBuilding => "Roads",
        shared::DevCardType::Monopoly => "Monopoly",
        shared::DevCardType::YearOfPlenty => "Plenty",
    };

    // Pair each card with whether it was drawn this turn. The hand is a plain
    // list of types with no identity, so mark the newest of each type: those
    // are necessarily the ones just drawn.
    let cards = move || {
        let hand = state.my_dev_cards.get();
        let fresh = state.fresh_dev_cards.get();
        let mut marks = vec![false; hand.len()];

        for kind in &fresh {
            if let Some(idx) = hand
                .iter()
                .enumerate()
                .rev()
                .find(|(i, c)| *c == kind && !marks[*i])
                .map(|(i, _)| i)
            {
                marks[idx] = true;
            }
        }

        hand.into_iter().zip(marks).enumerate().collect::<Vec<_>>()
    };

    let my_turn = move || state.player_id.get() == Some(state.current_turn_player.get());

    view! {
        <Show when=move || !state.my_dev_cards.get().is_empty()>
            <div class="flex items-end gap-1.5 bg-slate-900/70 backdrop-blur-md px-2 py-1.5 rounded-xl border border-slate-700/70 shadow-2xl">
                <For
                    each=cards
                    key=|(i, (card, fresh))| (*i, format!("{card:?}"), *fresh)
                    children=move |(_, (card, fresh))| {
                        let is_point = card == shared::DevCardType::VictoryPoint;
                        let playable = move || {
                            !is_point
                                && !fresh
                                && my_turn()
                                && !state.dev_card_played_this_turn.get()
                                && state.game_phase.get() == GamePhase::RegularPlay
                        };
                        let name = short(&card);
                        let why = move || {
                            if is_point { "Counts towards victory. Nothing to play.".to_string() }
                            else if fresh { "Drawn this turn - playable next turn".to_string() }
                            else if !my_turn() { "Playable on your turn".to_string() }
                            else if state.dev_card_played_this_turn.get() {
                                "One development card per turn".to_string()
                            } else { format!("Play {name}") }
                        };
                        let to_play = card.clone();

                        view! {
                            <button
                                class=move || format!(
                                    "flex flex-col items-center justify-center gap-0.5 w-12 h-12 rounded-lg border-b-4 transition-all {}",
                                    if is_point {
                                        "bg-amber-300 border-amber-500 text-amber-950 cursor-default"
                                    } else if playable() {
                                        "bg-violet-400 border-violet-600 text-violet-950 hover:-translate-y-1 cursor-pointer"
                                    } else {
                                        "bg-slate-600 border-slate-700 text-slate-400 opacity-60 cursor-not-allowed"
                                    }
                                )
                                title=why
                                disabled=move || !playable()
                                on:click=move |_| {
                                    state.send(ClientRequest::PlayDevCard {
                                        card: to_play.clone(),
                                        target: None,
                                    });
                                }
                            >
                                <Icon kind=icon_for(&card) size="w-4 h-4" />
                                <span class="text-[7px] font-bold uppercase tracking-wide leading-none">
                                    {name}
                                </span>
                            </button>
                        }
                    }
                />
            </div>
        </Show>
    }
}

/// The cards in your hand. Reads as a row of cards rather than a table of
/// numbers, so the size of your hand is legible at a glance.
#[component]
fn ResourceHand() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let card = move |label: &'static str, tint: &'static str, count: u8| {
        view! {
            <div
                class=format!(
                    "flex flex-col items-center justify-center w-12 h-12 rounded-lg border-b-4 shadow-lg transition-transform hover:-translate-y-0.5 {tint} {}",
                    if count == 0 { "opacity-40" } else { "" }
                )
                title=format!("{count} {label}")
            >
                <span class="text-[9px] font-bold uppercase tracking-wider text-black/60">{label}</span>
                <span class="text-lg font-black leading-none text-black/85 tabular-nums">{count}</span>
            </div>
        }
    };

    view! {
        <div class="flex items-end gap-1.5 bg-slate-900/70 backdrop-blur-md px-2 py-1.5 rounded-xl border border-slate-700/70 shadow-2xl">
            {move || {
                let r = state.my_resources.get();
                view! {
                    {card("Brick", "bg-orange-400 border-orange-600", r.brick)}
                    {card("Wood", "bg-emerald-500 border-emerald-700", r.lumber)}
                    {card("Sheep", "bg-lime-400 border-lime-600", r.wool)}
                    {card("Wheat", "bg-amber-400 border-amber-600", r.grain)}
                    {card("Ore", "bg-slate-400 border-slate-600", r.ore)}
                }
            }}
            <div class="ml-1 pl-3 border-l border-slate-700 flex flex-col items-center justify-center h-12">
                <span class="text-[9px] font-bold uppercase tracking-wider text-slate-500">"Total"</span>
                <span class="text-lg font-black leading-none text-slate-200 tabular-nums">
                    {move || {
                        let r = state.my_resources.get();
                        r.brick as u32 + r.lumber as u32 + r.wool as u32 + r.grain as u32 + r.ore as u32
                    }}
                </span>
            </div>
        </div>
    }
}

#[component]
fn ResourceIcon(label: &'static str, color: &'static str, count: i32) -> impl IntoView {
    view! {
        <div class="flex flex-col items-center">
            <span class=format!("text-[10px] font-bold uppercase {color}")>{label}</span>
            <span class="text-lg font-black leading-none">{count}</span>
        </div>
    }
}

#[component]
fn PlayerTag(name: &'static str, score: i32, is_active: bool) -> impl IntoView {
    let active_class = if is_active { "border-orange-600 bg-orange-600/10" } else { "border-slate-800 bg-transparent" };
    view! {
        <div class=format!("flex justify-between items-center p-3 border rounded-xl transition-all {active_class}")>
            <div class="flex items-center gap-2">
                <div class=format!("w-2 h-2 rounded-full {}", if is_active { "bg-orange-500 animate-pulse" } else { "bg-slate-600" })></div>
                <span class="text-sm font-bold">{name}</span>
            </div>
            <span class="bg-black/40 px-2 py-1 rounded text-[10px] font-mono font-bold text-orange-400">{score} " VP"</span>
        </div>
    }
}

#[component]
fn PlayerTagDynamic(
    name: String,
    score: Signal<(i32, i32)>,
    is_active: impl Fn() -> bool + 'static + Clone,
    colour: PlayerColour,
    is_me: bool,
    resource_count: Signal<i32>,
    dev_card_count: Signal<i32>,
    knights_played: Signal<i32>,
    roads_count: Signal<i32>,
    has_longest_road: Signal<bool>,
    has_largest_army: Signal<bool>
) -> impl IntoView {
    let hex_color = player_color_hex(colour);
    let is_active_for_style = is_active.clone();
    let is_active_for_class = is_active.clone();
    let active_class = move || {
        if is_active_for_class() {
            "border-orange-600 bg-orange-600/10"
        } else {
            "border-slate-800"
        }
    };
    let inactive_style = move || {
        if !is_active_for_style() {
            format!("background-color: {}33;", hex_color)
        } else {
            "".to_string()
        }
    };
    view! {
        <div
            class=move || format!("flex flex-col p-3 border rounded-xl transition-all {}", active_class())
            style=inactive_style
        >
            <div class="flex justify-between items-center">
                <div class="flex items-center gap-2 min-w-0">
                    <div
                        class="w-2.5 h-2.5 rounded-full border border-black/40 shrink-0"
                        style=move || format!("background-color: {};", hex_color)
                    ></div>
                    <span
                        class="text-sm font-bold truncate"
                        style=move || format!("color: {};", hex_color)
                        title=name.clone()
                    >{name}</span>
                    <Show when=is_active>
                        <span class="bg-orange-600 text-white px-2 py-0.5 rounded-full text-[9px] font-bold shrink-0">"TURN"</span>
                    </Show>
                    <Show when=move || has_longest_road.get()>
                        <span class="bg-yellow-700 text-white px-1.5 py-0.5 rounded-full text-[10px] shrink-0" title="Longest Road (2 VP)">
                            <Icon kind=IconKind::Road />
                        </span>
                    </Show>
                    <Show when=move || has_largest_army.get()>
                        <span class="bg-red-700 text-white px-1.5 py-0.5 rounded-full text-[10px] shrink-0" title="Largest Army (2 VP)">
                            <Icon kind=IconKind::Knight />
                        </span>
                    </Show>
                </div>
                // Never wraps: the score is the one thing that has to stay
                // readable however long the name is.
                <span
                    class="bg-black/40 px-2 py-1 rounded text-[10px] font-mono font-bold text-orange-400 whitespace-nowrap shrink-0"
                    title=move || {
                        let (vp, secret) = score.get();
                        if is_me && secret > 0 {
                            format!("{} public + {secret} hidden victory point(s)", vp - secret)
                        } else {
                            format!("{vp} victory points")
                        }
                    }
                >
                    {move || {
                        let (vp, secret) = score.get();
                        if is_me && secret > 0 {
                            format!("{vp} VP ({secret} hidden)")
                        } else {
                            format!("{vp} VP")
                        }
                    }}
                </span>
            </div>
            <div class="grid grid-cols-2 gap-x-3 gap-y-1 mt-2 text-[10px] text-slate-400">
                <span class="flex items-center gap-1.5 whitespace-nowrap" title="Resource cards in hand">
                    <Icon kind=IconKind::Resource />
                    <span class="text-slate-200 font-semibold">{move || resource_count.get()}</span>
                    "resources"
                </span>
                <span class="flex items-center gap-1.5 whitespace-nowrap" title="Development cards in hand">
                    <Icon kind=IconKind::DevCard />
                    <span class="text-slate-200 font-semibold">{move || dev_card_count.get()}</span>
                    "dev cards"
                </span>
                <span class="flex items-center gap-1.5 whitespace-nowrap" title="Knights played">
                    <Icon kind=IconKind::Knight />
                    <span class="text-slate-200 font-semibold">{move || knights_played.get()}</span>
                    "knights"
                </span>
                <span class="flex items-center gap-1.5 whitespace-nowrap" title="Longest road length">
                    <Icon kind=IconKind::Road />
                    <span class="text-slate-200 font-semibold">{move || roads_count.get()}</span>
                    "road"
                </span>
            </div>
        </div>
    }
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
        <div class="fixed inset-0 bg-black/80 flex items-center justify-center z-[9999] backdrop-blur-sm pointer-events-auto">
            <div class="bg-slate-900 border-2 border-orange-600 rounded-2xl p-6 max-w-md w-full mx-4 shadow-2xl">
                <h2 class="text-2xl font-bold text-orange-500 mb-2 flex items-center gap-2"><Icon kind=IconKind::Robber /> "Rob a Player"</h2>
                <p class="text-slate-300 mb-4">
                    "Select a player to steal a random resource card from:"
                </p>

                <div class="space-y-2 mb-4">
                    <For
                        each=move || state.must_steal_from_players.get()
                        key=|id| *id
                        children=move |victim_id| {
                            let player_name = state.players.get()
                                .iter()
                                .find(|p| p.player_id == victim_id)
                                .map(|p| p.name.clone())
                                .unwrap_or_else(|| format!("Player {}", victim_id));

                            view! {
                                <button
                                    class="w-full py-3 px-4 bg-slate-800 hover:bg-orange-600 border border-slate-700 rounded-lg text-sm font-bold transition-colors text-left"
                                    on:click=move |_| rob_player(victim_id)
                                >
                                    {player_name}
                                </button>
                            }
                        }
                    />
                </div>
            </div>
        </div>
    }
}

/// Offers waiting on an answer from you. Pulled out of the propose-a-trade
/// form because they arrive on other players' turns too, when that form is
/// correctly hidden.
#[component]
fn IncomingTrades() -> impl IntoView {
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

    let (pick1, set_pick1) = create_signal::<Option<shared::ResourceType>>(None);
    let (pick2, set_pick2) = create_signal::<Option<shared::ResourceType>>(None);

    // Reset the modal selections when it closes
    create_effect(move |_| {
        if !state.year_of_plenty_pending.get() {
            set_pick1.set(None);
            set_pick2.set(None);
        }
    });

    let can_confirm = move || pick1.get().is_some() && pick2.get().is_some();

    let confirm = move |_| {
        if let (Some(r1), Some(r2)) = (pick1.get(), pick2.get()) {
            state.send(ClientRequest::YearOfPlentyChoice {
                resource1: r1,
                resource2: r2,
            });

            // UX: optimistically close/reset; server will also send YearOfPlentyResourcesReceived
            set_pick1.set(None);
            set_pick2.set(None);
        }
    };

    let resource_btn = move |res: shared::ResourceType, label: &'static str, color: &'static str| {
        let selected1 = Signal::derive(move || pick1.get() == Some(res));
        let selected2 = Signal::derive(move || pick2.get() == Some(res));

        let select = move |_| {
            match (pick1.get(), pick2.get()) {
                (None, _) => set_pick1.set(Some(res)),
                (Some(_), None) => set_pick2.set(Some(res)),
                (Some(_), Some(_)) => {
                    // If both filled, replace second (simple UX)
                    set_pick2.set(Some(res));
                }
            }
        };

        view! {
            <button
                class=move || {
                    let base = "px-3 py-2 rounded-lg border text-[11px] font-bold transition-all";
                    let is_selected = selected1.get() || selected2.get();
                    if is_selected {
                        format!("{base} bg-green-700/40 border-green-500 text-white")
                    } else {
                        format!("{base} bg-slate-800/60 border-slate-700 text-slate-200 hover:bg-slate-700/60")
                    }
                }
                on:click=select
                title=label
            >
                <span class=color>{label}</span>
            </button>
        }
    };

    let clear1 = move |_| set_pick1.set(None);
    let clear2 = move |_| set_pick2.set(None);

    let pick_label = move |p: Option<shared::ResourceType>| -> &'static str {
        match p {
            Some(shared::ResourceType::Brick) => "Brick",
            Some(shared::ResourceType::Wood) => "Wood",
            Some(shared::ResourceType::Sheep) => "Sheep",
            Some(shared::ResourceType::Wheat) => "Wheat",
            Some(shared::ResourceType::Ore) => "Ore",
            _ => "—",
        }
    };

    view! {
        <div class="fixed inset-0 bg-black/80 flex items-center justify-center z-[9999] backdrop-blur-sm pointer-events-auto">
            <div class="bg-slate-900 border-2 border-emerald-600 rounded-2xl p-6 max-w-md w-full mx-4 shadow-2xl">
                <div class="flex justify-between items-start mb-2">
                    <div>
                        <h2 class="text-2xl font-bold text-emerald-400 flex items-center gap-2"><Icon kind=IconKind::Wheat /> "Year of Plenty"</h2>
                        <p class="text-slate-300 text-sm">"Choose 2 resources to receive from the bank."</p>
                    </div>
                    <div class="text-[10px] text-slate-400">
                        <div class="flex items-center gap-2">
                            <span class="font-mono">"1:"</span>
                            <span class="font-bold text-slate-200">{move || pick_label(pick1.get())}</span>
                            <button class="text-slate-400 hover:text-white" on:click=clear1 title="Clear pick 1">"×"</button>
                        </div>
                        <div class="flex items-center gap-2">
                            <span class="font-mono">"2:"</span>
                            <span class="font-bold text-slate-200">{move || pick_label(pick2.get())}</span>
                            <button class="text-slate-400 hover:text-white" on:click=clear2 title="Clear pick 2">"×"</button>
                        </div>
                    </div>
                </div>

                <div class="grid grid-cols-2 gap-2 mt-4">
                    {resource_btn(shared::ResourceType::Brick, "Brick", "text-red-400")}
                    {resource_btn(shared::ResourceType::Wood, "Wood", "text-green-400")}
                    {resource_btn(shared::ResourceType::Sheep, "Sheep", "text-lime-300")}
                    {resource_btn(shared::ResourceType::Wheat, "Wheat", "text-yellow-300")}
                    {resource_btn(shared::ResourceType::Ore, "Ore", "text-slate-300")}
                </div>

                <div class="flex justify-end gap-2 pt-4 mt-4 border-t border-slate-800">
                    <button
                        class="px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-[11px] font-bold text-slate-200"
                        on:click=move |_| { set_pick1.set(None); set_pick2.set(None); }
                    >
                        "Reset"
                    </button>
                    <button
                        class="px-3 py-2 rounded-lg bg-emerald-700 hover:bg-emerald-600 text-[11px] font-bold text-white disabled:opacity-40 disabled:cursor-not-allowed"
                        disabled=move || !can_confirm()
                        on:click=confirm
                    >
                        "Confirm"
                    </button>
                </div>
            </div>
        </div>
    }
}

#[component]
fn MonopolyModal() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let (selected, set_selected) = create_signal::<Option<shared::ResourceType>>(None);

    // Reset selection when modal closes
    create_effect(move |_| {
        if !state.monopoly_pending.get() {
            set_selected.set(None);
        }
    });

    let confirm = move |_| {
        if let Some(resource) = selected.get() {
            state.send(ClientRequest::MonopolyChoice { resource });
            set_selected.set(None);
        }
    };

    let resource_btn = move |res: shared::ResourceType, label: &'static str, color: &'static str| {
        let is_selected = Signal::derive(move || selected.get() == Some(res));

        view! {
            <button
                class=move || {
                    let base = "px-4 py-3 rounded-lg border text-sm font-bold transition-all";
                    if is_selected.get() {
                        format!("{base} bg-purple-700/40 border-purple-500 text-white")
                    } else {
                        format!("{base} bg-slate-800/60 border-slate-700 text-slate-200 hover:bg-slate-700/60")
                    }
                }
                on:click=move |_| set_selected.set(Some(res))
                title=label
            >
                <span class=color>{label}</span>
            </button>
        }
    };

    view! {
        <div class="fixed inset-0 bg-black/80 flex items-center justify-center z-[9999] backdrop-blur-sm pointer-events-auto">
            <div class="bg-slate-900 border-2 border-purple-600 rounded-2xl p-6 max-w-md w-full mx-4 shadow-2xl">
                <div class="mb-4">
                    <h2 class="text-2xl font-bold text-purple-400">"Monopoly"</h2>
                    <p class="text-slate-300 text-sm">"Choose a resource to steal from ALL other players."</p>
                </div>

                <div class="grid grid-cols-2 gap-2">
                    {resource_btn(shared::ResourceType::Brick, "Brick", "text-red-400")}
                    {resource_btn(shared::ResourceType::Wood, "Wood", "text-green-400")}
                    {resource_btn(shared::ResourceType::Sheep, "Sheep", "text-lime-300")}
                    {resource_btn(shared::ResourceType::Wheat, "Wheat", "text-yellow-300")}
                    {resource_btn(shared::ResourceType::Ore, "Ore", "text-slate-300")}
                </div>

                <div class="flex justify-end pt-4 mt-4 border-t border-slate-800">
                    <button
                        class="px-4 py-2 rounded-lg bg-purple-700 hover:bg-purple-600 text-sm font-bold text-white disabled:opacity-40 disabled:cursor-not-allowed"
                        disabled=move || selected.get().is_none()
                        on:click=confirm
                    >
                        "Confirm"
                    </button>
                </div>
            </div>
        </div>
    }
}