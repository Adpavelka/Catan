//! The status panel for an offer you have put to the table.
//!
//! It sits in the top-right corner over the board and answers the only three
//! questions you have after making an offer: what did I put up, who has said
//! yes, and can I close it. The two rows read as one transaction - a green
//! arrow down for what would come to you, a red arrow up for what would leave
//! you - and the avatars along the right carry a tick or a cross for how each
//! player answered.
//!
//! Accepting is a bid, not a settlement: several people can say yes, so you
//! pick which one to deal with by clicking their avatar, and only then does
//! the confirm control come alive.

use leptos::*;
use uuid::Uuid;

use crate::components::bottom::{Basket, CardFace, MysteryCard, TradeDraft};
use crate::state::{GameState, MyOffer};
use shared::{ClientRequest, Resources};

/// Turn the wire's resource bag into the local one.
fn to_basket(r: &Resources) -> Basket {
    Basket {
        wood: r.lumber,
        brick: r.brick,
        sheep: r.wool,
        wheat: r.grain,
        ore: r.ore,
    }
}

/// Every offer you currently have on the table, stacked in the top-right.
#[component]
pub fn OfferStatus() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let any = move || !state.my_offers.get().is_empty();

    view! {
        <Show when=any>
            // Fixed, not absolute: this belongs to the top-right of the
            // window, and an absolute box would be positioned against
            // whichever ancestor happened to be relative.
            <div class="fixed top-3 z-[60] flex flex-col gap-1.5 items-end"
                 style="right: 416px; width: 485px;">
                <For
                    each=move || state.my_offers.get()
                    key=|o| o.offer_id
                    children=move |offer: MyOffer| view! { <OfferCard offer=offer /> }
                />
            </div>
        </Show>
    }
}

/// One offer: what it is, who has answered, and what you can do about it.
#[component]
fn OfferCard(offer: MyOffer) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let draft = use_context::<TradeDraft>().expect("TradeDraft missing");

    let offer_id = offer.offer_id;
    let (collapsed, set_collapsed) = create_signal(false);
    let chosen = create_rw_signal(Option::<Uuid>::None);

    // Read the offer back out of state rather than trusting the copy this
    // component was built with: bids arrive after it is on screen.
    let live = create_memo(move |_| {
        state.my_offers.get().into_iter().find(|o| o.offer_id == offer_id)
    });
    let accepters = move || live.get().map(|o| o.accepters).unwrap_or_default();
    let decliners = move || live.get().map(|o| o.decliners).unwrap_or_default();

    // One bidder needs no picking; the choice is already made.
    create_effect(move |_| {
        let a = accepters();
        chosen.update(|c| match a.len() {
            0 => *c = None,
            1 => *c = Some(a[0]),
            _ => {
                if !c.is_some_and(|id| a.contains(&id)) {
                    *c = None;
                }
            }
        });
    });

    let can_confirm = move || chosen.get().is_some();
    let must_pick = move || accepters().len() > 1 && chosen.get().is_none();

    let confirm = move |_| {
        if let Some(partner) = chosen.get_untracked() {
            state.send(ClientRequest::ConfirmTrade { offer_id, partner_id: partner });
        }
    };
    let cancel = move |_| state.send(ClientRequest::CancelTrade { offer_id });
    // Editing takes it off the table and puts its cards back on the bench, so
    // the window opens showing what you had rather than blank.
    let edit = move |_| {
        let Some(o) = live.get_untracked() else { return };
        state.send(ClientRequest::CancelTrade { offer_id });
        draft.offer.set(to_basket(&o.offering));
        draft.receive.set(to_basket(&o.requesting));
        draft.offer_any.set(o.offering_any);
        draft.receive_any.set(o.requesting_any);
        draft.open.set(true);
    };

    let asked = Signal::derive(move || live.get().map(|o| to_basket(&o.requesting)).unwrap_or_default());
    let asked_any = Signal::derive(move || live.get().map_or(0, |o| o.requesting_any));
    let given = Signal::derive(move || live.get().map(|o| to_basket(&o.offering)).unwrap_or_default());
    let given_any = Signal::derive(move || live.get().map_or(0, |o| o.offering_any));

    view! {
        <div class="w-full flex flex-col gap-1.5">
            <div class="hud-tray flex items-center px-3" style="height: 60px;">
                <span class="text-[11px] font-black uppercase tracking-[0.15em] text-[#7a7263]">
                    "Your offer"
                </span>

                <span class="flex-1 flex justify-center">
                    <PlayerMark who=Signal::derive(move || state.player_id.get()) size=44 />
                </span>

                <button
                    class="w-9 h-9 flex items-center justify-center rounded-md hover:bg-black/10"
                    title=move || if collapsed.get() { "Show the offer" } else { "Collapse" }
                    on:click=move |_| set_collapsed.update(|c| *c = !*c)
                >
                    <svg
                        class=move || format!(
                            "w-5 h-4 transition-transform {}",
                            if collapsed.get() { "rotate-180" } else { "" }
                        )
                        viewBox="0 0 24 18" fill="none" stroke="#1d2430"
                        stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"
                    >
                        <path d="M3 13 12 4l9 9"/>
                    </svg>
                </button>
            </div>

            <Show when=move || !collapsed.get()>
                <div class="hud-tray px-3 py-2 flex flex-col justify-center gap-1"
                     style="min-height: 155px;">

                    // What would come to you, and how people answered.
                    <div class="flex items-center gap-2.5 min-h-[68px]">
                        <PlayerMark who=Signal::derive(move || state.player_id.get()) size=44 />
                        <Arrow up=false />
                        <TermCards basket=asked wild=asked_any />

                        <div class="ml-auto flex items-center gap-1.5 shrink-0">
                            <For
                                each=move || {
                                    let mut v: Vec<(Uuid, bool)> =
                                        accepters().into_iter().map(|id| (id, true)).collect();
                                    v.extend(decliners().into_iter().map(|id| (id, false)));
                                    v
                                }
                                key=|(id, ok)| (*id, *ok)
                                children=move |(pid, accepted)| view! {
                                    <Responder who=pid accepted=accepted chosen=chosen />
                                }
                            />
                        </div>
                    </div>

                    // What would leave you, and the controls.
                    <div class="flex items-center gap-2.5 min-h-[68px]">
                        <PlayerMark who=Signal::derive(move || state.player_id.get()) size=44 />
                        <Arrow up=true />
                        <TermCards basket=given wild=given_any />

                        <div class="ml-auto flex items-center gap-1.5 shrink-0">
                            <SmallButton
                                label="Change this offer"
                                enabled=Signal::derive(|| true)
                                on_click=Callback::new(edit)
                            >
                                <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16z"/>
                                <path d="M14.5 5.5 18.5 9.5"/>
                            </SmallButton>

                            <SmallButton
                                label="Take the offer back"
                                enabled=Signal::derive(|| true)
                                on_click=Callback::new(cancel)
                            >
                                <path d="M6 6 18 18"/><path d="M18 6 6 18"/>
                            </SmallButton>

                            <SmallButton
                                label="Settle with the player you picked"
                                enabled=Signal::derive(can_confirm)
                                on_click=Callback::new(confirm)
                            >
                                <path d="M4 12.5 9.5 18 20 6"/>
                            </SmallButton>
                        </div>
                    </div>

                    <Show when=must_pick>
                        <div class="text-[11px] text-[#8a5a00] text-center">
                            "Click whose bid you want to take."
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

/// A player's colour disc with a simple figure in it.
#[component]
fn PlayerMark(who: Signal<Option<Uuid>>, #[prop(default = 44)] size: u32) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let colour = move || {
        who.get()
            .and_then(|id| {
                state
                    .players
                    .get()
                    .iter()
                    .find(|p| p.player_id == id)
                    .map(|p| p.colour.hex())
            })
            .unwrap_or("#94a3b8")
    };
    let name = move || who.get().map(|id| state.player_name(id)).unwrap_or_default();

    view! {
        <span
            class="rounded-full shrink-0 flex items-center justify-center border-[3px] border-white
                   shadow-[0_0_0_2px_rgba(13,62,92,0.75)]"
            style=move || format!("width: {size}px; height: {size}px; background-color: {}", colour())
            title=name
        >
            <img
                src="/assets/trade-offerer.svg" alt=""
                style=format!("width: {}px; height: {}px; object-fit: contain",
                              size * 7 / 10, size * 7 / 10)
            />
        </span>
    }
}

/// One player's answer: their disc with a tick or a cross clipped to its
/// corner. The ones who said yes are clickable, because picking between
/// bidders is the whole point of the panel.
#[component]
fn Responder(who: Uuid, accepted: bool, chosen: RwSignal<Option<Uuid>>) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let name = state.player_name(who);
    let picked = move || accepted && chosen.get() == Some(who);

    let title = if accepted {
        format!("{name} will take it - click to settle with them")
    } else {
        format!("{name} said no")
    };

    view! {
        <button
            class=move || format!(
                "relative rounded-full transition-transform {}",
                if picked() { "ring-[3px] ring-[#ffd44d] scale-105" } else { "" }
            )
            title=title
            disabled=!accepted
            on:click=move |_| {
                if accepted {
                    chosen.set(Some(who));
                }
            }
        >
            <PlayerMark who=Signal::derive(move || Some(who)) size=40 />

            <span
                class="absolute -bottom-0.5 -right-0.5 w-[18px] h-[18px] rounded-full
                       border-2 border-white flex items-center justify-center"
                style=if accepted { "background-color: #13b83d" } else { "background-color: #e03131" }
            >
                <svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="#fff"
                     stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round">
                    {if accepted {
                        view! { <path d="M4 12.5 9.5 18 20 6"/> }
                    } else {
                        view! { <path d="M6 6 18 18M18 6 6 18"/> }
                    }}
                </svg>
            </span>
        </button>
    }
}

/// The fat directional arrow. Green down for what comes in, red up for what
/// goes out - the row's whole meaning, so it is drawn large.
#[component]
fn Arrow(up: bool) -> impl IntoView {
    view! {
        <svg
            class="w-[28px] h-[34px] shrink-0" viewBox="0 0 24 28"
            fill=if up { "#ef3030" } else { "#16c33a" }
            stroke="#0d3e5c" stroke-width="1.8" stroke-linejoin="round"
        >
            {if up {
                view! { <path d="M9 27h6V13h6L12 1 3 13h6z"/> }
            } else {
                view! { <path d="M9 1h6v14h6l-9 12-9-12h6z"/> }
            }}
        </svg>
    }
}

/// The cards on one side of the offer. One card per card - two brick is two
/// brick cards, so the deal can be read by counting rather than by squinting
/// at a number in a corner.
#[component]
fn TermCards(basket: Signal<Basket>, wild: Signal<u8>) -> impl IntoView {
    let nothing = move || basket.get().total() == 0 && wild.get() == 0;
    let has_wild = move || wild.get() > 0;

    view! {
        <div class="flex items-center gap-[3px] min-w-0">
            {move || basket.get().spread().into_iter().map(|k| view! {
                <CardFace art=k.art() alt=k.label() size="w-[55px] h-[65px]" />
            }).collect_view()}

            <Show when=has_wild>
                {move || (0..wild.get()).map(|_| view! {
                    <MysteryCard size="w-[55px] h-[65px]" />
                }).collect_view()}
            </Show>

            <Show when=nothing>
                <span class="text-[12px] italic text-[#a89e8b]">"nothing"</span>
            </Show>
        </div>
    }
}

/// A small square control in the panel's action row.
#[component]
fn SmallButton(
    label: &'static str,
    enabled: Signal<bool>,
    on_click: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            class="hud-btn w-[52px] h-[52px] flex items-center justify-center"
            style="border-width: 3px; border-radius: 11px;"
            title=label
            aria-label=label
            disabled=move || !enabled.get()
            on:click=move |_| on_click.call(())
        >
            <svg class="hud-icon w-7 h-7" viewBox="0 0 24 24" fill="none" stroke="#0d3e5c"
                 stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                {children()}
            </svg>
        </button>
    }
}
