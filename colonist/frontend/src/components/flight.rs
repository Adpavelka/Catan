//! Cards in flight: from the tile that produced them to the player who got
//! them, and from a robbed player to whoever robbed them.
//!
//! What a roll pays out is derived by `GameState::roll_payout` rather than
//! read off the wire - see there for why. A steal does come off the wire,
//! because only the server knows which card the robber's hand happened to
//! find.
//!
//! Positions come from the live DOM: the hex `<g>` elements carry `data-hex`
//! and the player rows carry `data-player`, so a card starts where its tile is
//! on screen right now, zoom and pan included.

use leptos::*;
use uuid::Uuid;
use wasm_bindgen::JsCast;

use crate::components::icons::resource_art;
use crate::state::GameState;
use shared::ResourceType;

/// How long one card is in the air. Slow enough to follow a card from the tile
/// that produced it to the player it belongs to - the point of the animation is
/// that you can see where your income came from.
const FLIGHT_MS: u64 = 1500;
/// Cards from the same roll leave in quick succession rather than as a clump.
const STAGGER_MS: u64 = 100;
/// Beyond this many cards the animation stops being readable and starts being
/// a swarm, so the tail is dropped.
const MAX_CARDS: usize = 18;

#[derive(Clone, PartialEq)]
struct Flyer {
    id: u64,
    art: &'static str,
    label: &'static str,
    from: (f64, f64),
    to: (f64, f64),
    delay: u64,
}

/// Where a hex sits on screen right now, in viewport coordinates.
fn hex_centre(q: i32, r: i32) -> Option<(f64, f64)> {
    let doc = document();
    let el = doc
        .query_selector(&format!("[data-hex=\"{q},{r}\"]"))
        .ok()
        .flatten()?
        .dyn_into::<web_sys::Element>()
        .ok()?;
    let b = el.get_bounding_client_rect();
    Some((b.x() + b.width() / 2.0, b.y() + b.height() / 2.0))
}

/// Where a player's row sits on screen right now.
fn player_centre(pid: Uuid) -> Option<(f64, f64)> {
    let doc = document();
    let el = doc
        .query_selector(&format!("[data-player=\"{pid}\"]"))
        .ok()
        .flatten()?
        .dyn_into::<web_sys::Element>()
        .ok()?;
    let b = el.get_bounding_client_rect();
    Some((b.x() + b.width() / 2.0, b.y() + b.height() / 2.0))
}

#[component]
pub fn ResourceFlight() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let flyers = create_rw_signal(Vec::<Flyer>::new());
    let next_id = store_value(0u64);

    // Put a batch of cards in the air and take them down again when the last
    // of them has landed. Each batch clears itself: one shared timer would cut
    // short cards from a roll that landed while the previous lot were still
    // up.
    let launch = move |batch: Vec<Flyer>| {
        if batch.is_empty() {
            return;
        }
        let ids: Vec<u64> = batch.iter().map(|f| f.id).collect();
        let longest = batch.last().map_or(0, |f| f.delay) + FLIGHT_MS + 100;
        flyers.update(|v| v.extend(batch));
        set_timeout(
            move || flyers.update(|v| v.retain(|f| !ids.contains(&f.id))),
            std::time::Duration::from_millis(longest),
        );
    };

    // What a roll paid out, tile by tile.
    create_effect(move |_| {
        let Some((total, _seq)) = state.last_roll_event.get() else {
            return;
        };
        let mut planned = state.roll_payout(total);
        if planned.is_empty() {
            return;
        }
        planned.truncate(MAX_CARDS);

        // The DOM has to have caught up with this roll before anything can be
        // measured, so the work happens a frame later rather than inline.
        set_timeout(
            move || {
                let mut batch = Vec::with_capacity(planned.len());
                for (i, ((q, r), pid, res)) in planned.iter().enumerate() {
                    let (Some(from), Some(to)) = (hex_centre(*q, *r), player_centre(*pid)) else {
                        continue;
                    };
                    let id = next_id.get_value();
                    next_id.set_value(id + 1);
                    batch.push(Flyer {
                        id,
                        art: resource_art(*res),
                        label: label_of(*res),
                        from,
                        to,
                        delay: (i as u64) * STAGGER_MS,
                    });
                }
                launch(batch);
            },
            std::time::Duration::from_millis(60),
        );
    });

    // One card crossing from the robbed player to the robber. Face down for
    // everyone but the two of them, because that is all anybody else is told.
    create_effect(move |_| {
        let Some(steal) = state.last_steal_event.get() else {
            return;
        };
        set_timeout(
            move || {
                let (Some(from), Some(to)) =
                    (player_centre(steal.victim), player_centre(steal.thief))
                else {
                    return;
                };
                let id = next_id.get_value();
                next_id.set_value(id + 1);
                launch(vec![Flyer {
                    id,
                    art: steal.resource.map_or("any-card", resource_art),
                    label: steal.resource.map_or("A card", label_of),
                    from,
                    to,
                    delay: 0,
                }]);
            },
            std::time::Duration::from_millis(60),
        );
    });

    view! {
        <div class="fixed inset-0 pointer-events-none z-[9000]">
            <For
                each=move || flyers.get()
                key=|f| f.id
                children=move |f: Flyer| {
                    // The card is drawn at the origin and moved by transform,
                    // so the browser animates it on the compositor.
                    // The negative margins are half the card, so the card rides
                    // centred on the line from tile to player rather than
                    // hanging off it by its top-left corner. They have to keep
                    // pace with the size below.
                    let style = format!(
                        "--x0: {:.1}px; --y0: {:.1}px; --x1: {:.1}px; --y1: {:.1}px; \
                         --dur: {}ms; --delay: {}ms; margin-left: -29px; margin-top: -39px;",
                        f.from.0, f.from.1, f.to.0, f.to.1, FLIGHT_MS, f.delay,
                    );
                    view! {
                        <img
                            class="flyer w-[58px] h-[78px] rounded-[6px] object-cover
                                   border-2 border-[#2b2418] shadow-[0_3px_6px_rgba(0,0,0,0.45)]"
                            src=format!("/assets/{}.svg", f.art)
                            alt=f.label
                            style=style
                        />
                    }
                }
            />
        </div>
    }
}

fn label_of(res: ResourceType) -> &'static str {
    match res {
        ResourceType::Wood => "Wood",
        ResourceType::Brick => "Brick",
        ResourceType::Sheep => "Sheep",
        ResourceType::Wheat => "Wheat",
        ResourceType::Ore => "Ore",
        ResourceType::Desert => "Desert",
    }
}
