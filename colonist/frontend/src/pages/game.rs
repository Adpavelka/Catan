use leptos::*;
use uuid::Uuid;
use crate::components::board::Board;
use crate::state::GameState;
use crate::components::icons::Art;
use crate::components::bottom::BottomLayer;
use crate::components::flight::ResourceFlight;
use crate::components::sidebar::{ChatPanel, EventLog, PlayerPanels, ResourceBank};
use crate::components::stats::GameEndStats;
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
                    style="width: var(--rail-w); background: linear-gradient(180deg, #066191 0%, #05537f 100%);"
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
            // The end of the game: the board stays where it is, darkened,
            // with the statistics on a sheet over it.
            <GameEndStats />
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

/// What you hand back when a seven is rolled.
///
/// Click the cards you are giving up. No counters, no plus and minus buttons:
/// discarding is choosing cards, so the interface is the cards, and the ones
/// you have picked lift out of the row.
#[component]
fn DiscardCardsModal() -> impl IntoView {
    use crate::components::bottom::{Basket, CardFace, Res};

    let state = use_context::<GameState>().expect("GameState missing");

    let owed = move || state.must_discard_count.get().unwrap_or(0) as u8;
    let picked = create_rw_signal(Basket::default());

    create_effect(move |_| {
        if state.must_discard_count.get().is_none() {
            picked.set(Basket::default());
        }
    });

    let chosen = move || picked.get().total();
    let can_confirm = move || chosen() == owed();
    let left = move || owed().saturating_sub(chosen());

    // Thirty seconds, then the server is entitled to act. Ticking down in
    // front of the player is kinder than a table that suddenly moves on.
    let (time_left, set_time_left) = create_signal(30i32);
    let timer = store_value(None::<leptos_dom::helpers::IntervalHandle>);
    create_effect(move |_| {
        if let Some(h) = timer.get_value() {
            h.clear();
        }
        if state.must_discard_count.get().is_none() {
            return;
        }
        set_time_left.set(30);
        // The hand-back fires once. `now` is negative on every tick after the
        // countdown hits zero, so testing `now > 0` alone let the auto-discard
        // go out again every second until the server's reply arrived, each
        // repeat coming back as "You are not required to discard right now".
        let sent = store_value(false);
        timer.set_value(
            set_interval_with_handle(
                move || {
                    let now = time_left.get_untracked() - 1;
                    set_time_left.set(now.max(0));
                    if now > 0
                        || sent.get_value()
                        || state.must_discard_count.get_untracked().is_none()
                    {
                        return;
                    }
                    sent.set_value(true);
                    // Out of time: give up cards at random rather than
                    // leaving the table waiting on somebody who has walked
                    // away from the keyboard.
                    let mut bag = picked.get_untracked();
                    let mut short = owed().saturating_sub(bag.total());
                    while short > 0 {
                        let held = state.my_resources.get_untracked();
                        let spare: Vec<Res> = Res::ALL
                            .into_iter()
                            .filter(|k| k.in_hand(&held) > bag.get(*k))
                            .collect();
                        let Some(k) = spare.first().copied() else { break };
                        bag.set(k, bag.get(k) + 1);
                        short -= 1;
                    }
                    state.send(ClientRequest::DiscardCards { resources: bag.to_shared() });
                    picked.set(Basket::default());
                },
                std::time::Duration::from_secs(1),
            )
            .ok(),
        );
    });
    on_cleanup(move || {
        if let Some(h) = timer.get_value() {
            h.clear();
        }
    });

    let confirm = move |_| {
        if can_confirm() {
            state.send(ClientRequest::DiscardCards {
                resources: picked.get_untracked().to_shared(),
            });
        }
    };

    let urgent = move || time_left.get() <= 10;

    view! {
        <GameDialog title="The robber takes his cut">
            <div class="flex items-center gap-3 mb-3">
                <p class="text-[14px] text-[#5c5445] flex-1">
                    "A seven was rolled. Choose "
                    <span class="font-black text-[#8a5a00]">{owed}</span>
                    " cards to give up."
                </p>
                <span class=move || format!(
                    "px-2.5 py-1 rounded-md border-2 text-[16px] font-black tabular-nums shrink-0 {}",
                    if urgent() {
                        "bg-[#ffe2e2] border-[#c0392b] text-[#8f1f1f] animate-pulse"
                    } else {
                        "bg-[#faf4e7] border-[#a89b81] text-[#4a4335]"
                    }
                )>
                    {move || format!("{}s", time_left.get())}
                </span>
            </div>

            // Your hand, every card drawn. Clicking one moves it to the pile
            // you are giving up; clicking it again takes it back.
            <div class="hud-tray p-2.5 flex items-center gap-[3px] flex-wrap min-h-[96px]">
                {Res::ALL.map(|k| {
                    let held = move || k.in_hand(&state.my_resources.get());
                    let up = move || picked.get().get(k);
                    view! {
                        {move || (0..held()).map(|i| {
                            let is_up = i >= held().saturating_sub(up());
                            view! {
                                <button
                                    class="game-card-pick"
                                    title=move || if is_up {
                                        format!("{} - click to keep it", k.label())
                                    } else {
                                        format!("{} - click to give it up", k.label())
                                    }
                                    on:click=move |_| picked.update(|b| {
                                        if is_up {
                                            b.set(k, b.get(k).saturating_sub(1));
                                        } else if b.total() < owed() {
                                            b.set(k, b.get(k) + 1);
                                        }
                                    })
                                >
                                    <CardFace
                                        art=k.art()
                                        alt=k.label()
                                        size="w-[54px] h-[76px]"
                                        dimmed=is_up
                                        selected=is_up
                                    />
                                </button>
                            }
                        }).collect_view()}
                    }
                }).to_vec()}
            </div>

            <div class="flex items-center gap-2 mt-3">
                <span class="text-[13px] text-[#7a7263]">
                    {move || if left() == 0 {
                        "That is the lot.".to_string()
                    } else {
                        format!("{} more to choose", left())
                    }}
                </span>
                <button
                    class="game-btn game-btn-red ml-auto px-6 h-11 text-[14px] font-black uppercase tracking-wider"
                    disabled=move || !can_confirm()
                    on:click=confirm
                >
                    "Hand them over"
                </button>
            </div>
        </GameDialog>
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
            <div class="flex flex-col gap-1.5">
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

/// An offer somebody has put to you.
///
/// Deliberately the same object as the panel the proposer sees: their avatar,
/// a green arrow down for what would come to you, a red arrow up for what
/// would leave you, and square controls on the right. The only differences
/// are whose face is on it and what the buttons do.
#[component]
fn IncomingTradeItem(trade: crate::state::PendingTradeOffer) -> impl IntoView {
    use crate::components::offer_status::{to_basket, Arrow, PlayerMark, SmallButton, TermCards};

    let state = use_context::<GameState>().expect("GameState missing");
    let offer_id = trade.offer_id;
    let received_at = trade.received_at;
    let proposer = trade.proposer_id;
    let proposer_name = state.player_name(proposer);

    // A counter is aimed at me alone, so accepting settles it outright rather
    // than joining a queue of bidders.
    let is_counter = trade.counters.is_some();

    let (seconds_left, set_seconds_left) = create_signal(TRADE_TIMEOUT_SECONDS);
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

    // Whether I have already answered, and how.
    let i_accepted = move || state.my_accepted_offers.get().contains(&offer_id);
    let my_counter = move || {
        state
            .my_counter_offers
            .get()
            .iter()
            .find(|(original, _)| *original == offer_id)
            .map(|(_, counter)| *counter)
    };
    let answered = move || i_accepted() || my_counter().is_some();
    let unanswered = move || !answered();

    // Can I cover what they are asking for?
    let asking = trade.requesting.clone();
    let can_afford = move || {
        let r = state.my_resources.get();
        asking.brick <= r.brick
            && asking.lumber <= r.lumber
            && asking.wool <= r.wool
            && asking.grain <= r.grain
            && asking.ore <= r.ore
    };
    // An offer with an unnamed card on it cannot be taken as it stands; the
    // only answer is to counter with the card named.
    let has_wildcard = trade.offering_any > 0 || trade.requesting_any > 0;
    let can_accept = move || can_afford() && !has_wildcard;
    let short = move || !can_afford() && !answered();
    let may_counter = move || !is_counter;

    let (countering, set_countering) = create_signal(false);

    let accept = move |_| {
        if is_counter {
            state.send(ClientRequest::ConfirmTrade { offer_id, partner_id: proposer });
        } else {
            state.send(ClientRequest::TradeResponse { offer_id, accept: true });
        }
    };
    let decline = move |_| {
        if is_counter {
            state.send(ClientRequest::CancelTrade { offer_id });
        } else {
            state.send(ClientRequest::TradeResponse { offer_id, accept: false });
        }
    };
    let withdraw = move |_| {
        if let Some(counter_id) = my_counter() {
            state.send(ClientRequest::CancelTrade { offer_id: counter_id });
        } else {
            state.send(ClientRequest::TradeResponse { offer_id, accept: false });
        }
    };

    // What comes to me is what they are offering; what leaves me is what they
    // are asking for. The proposer's panel says the same deal the other way
    // round, which is exactly what the arrows are for.
    let incoming = to_basket(&trade.offering);
    let incoming_any = trade.offering_any;
    let outgoing = to_basket(&trade.requesting);
    let outgoing_any = trade.requesting_any;

    let urgent = move || seconds_left.get() <= 10.0;

    view! {
        <div class="flex flex-col gap-1.5">
            // Height and avatar match `OfferCard`'s header, so an offer is the
            // same size whether you sent it or received it.
            <div class="hud-tray flex items-center gap-2.5 px-3" style="height: 60px;">
                <PlayerMark who=Signal::derive(move || Some(proposer)) size=44 />
                <span class="text-[14px] font-black text-[#413a2c] truncate">
                    {proposer_name}
                    {if is_counter { " counters" } else { " offers" }}
                </span>

                // The offer dies on a clock somebody else started, so the time
                // left is part of the offer, not a detail.
                <span class=move || format!(
                    "ml-auto px-2.5 py-1 rounded-md border-2 text-[15px] font-black tabular-nums {}",
                    if urgent() {
                        "bg-[#ffe2e2] border-[#c0392b] text-[#8f1f1f] animate-pulse"
                    } else {
                        "bg-[#faf4e7] border-[#a89b81] text-[#4a4335]"
                    }
                )>
                    {move || format!("{}s", seconds_left.get().ceil() as i32)}
                </span>
            </div>

            <div class="hud-tray px-3 py-2 flex flex-col justify-center gap-1"
                 style="min-height: 155px;">
                <div class="flex items-center gap-2.5 min-h-[68px]">
                    <PlayerMark who=Signal::derive(move || state.player_id.get()) size=44 />
                    <Arrow up=false />
                    <TermCards
                        basket=Signal::derive(move || incoming)
                        wild=Signal::derive(move || incoming_any)
                    />
                </div>

                <div class="flex items-center gap-2.5 min-h-[68px]">
                    <PlayerMark who=Signal::derive(move || state.player_id.get()) size=44 />
                    <Arrow up=true />
                    <TermCards
                        basket=Signal::derive(move || outgoing)
                        wild=Signal::derive(move || outgoing_any)
                    />

                    <div class="ml-auto flex items-center gap-1.5 shrink-0">
                        <Show
                            when=unanswered
                            fallback=move || view! {
                                // Answered. It is their move now - they may
                                // still pick somebody else - so this stays
                                // until they settle it.
                                <div class="flex items-center gap-1.5">
                                    <span class="text-[12px] font-bold text-[#1c7a33]">
                                        {move || if my_counter().is_some() {
                                            "Countered"
                                        } else {
                                            "Waiting on them"
                                        }}
                                    </span>
                                    <SmallButton
                                        label="Take your answer back"
                                        enabled=Signal::derive(|| true)
                                        on_click=Callback::new(withdraw)
                                    >
                                        <path d="M9 14 4 9l5-5"/>
                                        <path d="M4 9h11a5 5 0 0 1 0 10h-1"/>
                                    </SmallButton>
                                </div>
                            }
                        >
                            <SmallButton
                                label="Turn it down"
                                enabled=Signal::derive(|| true)
                                on_click=Callback::new(decline)
                            >
                                <path d="M6 6 18 18"/><path d="M18 6 6 18"/>
                            </SmallButton>

                            // A counter is already a reply to me alone;
                            // countering a counter is not a move.
                            <Show when=may_counter>
                                <SmallButton
                                    label="Answer with terms of your own"
                                    enabled=Signal::derive(|| true)
                                    on_click=Callback::new(move |_| set_countering.set(true))
                                >
                                    <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16z"/>
                                    <path d="M14.5 5.5 18.5 9.5"/>
                                </SmallButton>
                            </Show>

                            <SmallButton
                                label=if has_wildcard {
                                    "Counter to name the unspecified card"
                                } else {
                                    "Take it"
                                }
                                enabled=Signal::derive(can_accept)
                                on_click=Callback::new(accept)
                            >
                                <path d="M4 12.5 9.5 18 20 6"/>
                            </SmallButton>
                        </Show>
                    </div>
                </div>

                <Show when=short>
                    <div class="text-[12px] text-[#8a5a00]">
                        "You do not hold what they are asking for."
                    </div>
                </Show>
            </div>

            <Show when=move || countering.get()>
                <CounterOfferForm
                    offer_id=offer_id
                    on_done=Callback::new(move |_| set_countering.set(false))
                />
            </Show>
        </div>
    }
}

/// One side of a counter: the cards you have put on it, and the five kinds to
/// put there.
///
/// Counting cards, not typing numbers - the same way every other pile in the
/// game is built. Clicking a kind in the strip lays one down; clicking a card
/// you have already laid picks it back up.
#[component]
fn CounterSide(
    /// Green down for what would come to you, red up for what would leave you.
    /// The same two arrows the offer panels use, so a counter reads as the
    /// same kind of object as the offer it answers.
    up: bool,
    bucket: RwSignal<crate::components::bottom::Basket>,
    /// Whether this side is limited by what you are actually holding.
    from_hand: bool,
) -> impl IntoView {
    use crate::components::bottom::{CardFace, Res};
    use crate::components::offer_status::Arrow;

    let state = use_context::<GameState>().expect("GameState missing");
    let empty = move || bucket.get().total() == 0;

    view! {
        <div class="flex items-start gap-2.5">
            <div class="pt-1.5"><Arrow up=up /></div>

            <div class="flex-1 min-w-0 flex flex-col gap-1.5">
                <div class="flex items-center gap-[3px] flex-wrap min-h-[65px]">
                    {move || bucket.get().spread().into_iter().map(|k| view! {
                        <button
                            class="game-card-pick shrink-0"
                            title=format!("Take this {} back off", k.label())
                            on:click=move |_| bucket.update(|b| {
                                let n = b.get(k);
                                if n > 0 { b.set(k, n - 1); }
                            })
                        >
                            <CardFace art=k.art() alt=k.label() size="w-[55px] h-[65px]" />
                        </button>
                    }).collect_view()}

                    <Show when=empty>
                        <span class="text-[12px] italic text-[#a89e8b] self-center">
                            "nothing yet"
                        </span>
                    </Show>
                </div>

                <div class="flex items-center gap-1">
                    {Res::ALL.into_iter().map(|k| {
                        // You cannot promise a card you do not hold, so the
                        // giving side runs out where your hand does.
                        let spent = move || {
                            from_hand
                                && bucket.get().get(k) >= k.in_hand(&state.my_resources.get())
                        };
                        view! {
                            <button
                                class="game-card-pick shrink-0 disabled:opacity-30"
                                title=move || if spent() {
                                    format!("No more {} in your hand", k.label())
                                } else {
                                    format!("Put a {} on this side", k.label())
                                }
                                disabled=spent
                                on:click=move |_| bucket.update(|b| b.set(k, b.get(k) + 1))
                            >
                                <CardFace
                                    art=k.art()
                                    alt=k.label()
                                    size="w-[34px] h-[44px]"
                                    dimmed=Signal::derive(spent)
                                />
                            </button>
                        }
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}

/// Answering an offer with terms of your own.
///
/// Built as the same object as the offer above it - a cream tray, the two
/// arrows, cards you count - rather than the row of steppers it used to be.
#[component]
fn CounterOfferForm(offer_id: u64, on_done: Callback<()>) -> impl IntoView {
    use crate::components::bottom::{Basket, Res};
    use crate::components::offer_status::SmallButton;

    let state = use_context::<GameState>().expect("GameState missing");

    let give = create_rw_signal(Basket::default());
    let want = create_rw_signal(Basket::default());

    let can_afford = move || {
        let (g, mine) = (give.get(), state.my_resources.get());
        Res::ALL.into_iter().all(|k| g.get(k) <= k.in_hand(&mine))
    };
    // A trade needs something on both sides; the server refuses it otherwise.
    let is_valid = move || give.get().total() > 0 && want.get().total() > 0 && can_afford();

    let send = move |_| {
        state.send(ClientRequest::CounterOffer {
            offer_id,
            offer: give.get_untracked().to_shared(),
            request: want.get_untracked().to_shared(),
        });
        on_done.call(());
    };

    view! {
        <div class="hud-tray px-3 py-2 flex flex-col gap-1.5">
            <span class="text-[11px] font-black uppercase tracking-[0.15em] text-[#7a7263]">
                "Your counter"
            </span>

            <CounterSide up=false bucket=want from_hand=false />
            <CounterSide up=true bucket=give from_hand=true />

            <div class="flex items-center justify-end gap-1.5">
                <SmallButton
                    label="Drop this counter"
                    enabled=Signal::derive(|| true)
                    on_click=Callback::new(move |_| on_done.call(()))
                >
                    <path d="M6 6 18 18"/><path d="M18 6 6 18"/>
                </SmallButton>

                <SmallButton
                    label="Send this counter back to them"
                    enabled=Signal::derive(is_valid)
                    on_click=Callback::new(send)
                >
                    <path d="M4 12.5 9.5 18 20 6"/>
                </SmallButton>
            </div>
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
