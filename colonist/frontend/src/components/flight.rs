//! Resource cards flying from the tile that produced them to the player who
//! got them.
//!
//! The payout is worked out here rather than read off the wire. The client
//! already knows the board, the buildings and the robber, so it can derive
//! exactly what a roll produces - and this is decoration, so if it ever
//! disagreed with the server the resource counts would still be the server's.
//! Deriving it locally means no new protocol and no per-hex bookkeeping on the
//! backend.
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

/// How long one card is in the air.
const FLIGHT_MS: u64 = 850;
/// Cards from the same roll leave in quick succession rather than as a clump.
const STAGGER_MS: u64 = 70;
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

/// The six vertices of a hex, in the same axial-times-three coordinates the
/// board uses for buildings.
fn hex_vertices(q: i32, r: i32) -> [(i32, i32); 6] {
    let base = (q * 3, r * 3);
    const NB: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];
    std::array::from_fn(|i| {
        let a = NB[i];
        let b = NB[(i + 1) % 6];
        (base.0 + a.0 + b.0, base.1 + a.1 + b.1)
    })
}

#[component]
pub fn ResourceFlight() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let flyers = create_rw_signal(Vec::<Flyer>::new());
    let next_id = store_value(0u64);

    create_effect(move |_| {
        let Some((total, _seq)) = state.last_roll_event.get() else {
            return;
        };
        // Seven pays nobody; it moves the robber instead.
        if total == 7 {
            return;
        }

        let robber = state.robber_pos.get_untracked();
        let hexes = state.hexes.get_untracked();
        let settlements = state.settlements.get_untracked();
        let cities = state.cities.get_untracked();

        // The DOM has to have caught up with this roll before anything can be
        // measured, so the work happens on the next frame rather than inline.
        let mut planned: Vec<(i32, i32, Uuid, ResourceType)> = Vec::new();
        for hex in hexes.iter() {
            if hex.number != total
                || hex.resource == ResourceType::Desert
                || robber == Some((hex.q, hex.r))
            {
                continue;
            }
            for v in hex_vertices(hex.q, hex.r) {
                // A city is two cards, a settlement one.
                let mut owed: Vec<Uuid> = settlements
                    .iter()
                    .filter(|b| (b.x, b.y) == v)
                    .map(|b| b.player_id)
                    .collect();
                for c in cities.iter().filter(|b| (b.x, b.y) == v) {
                    owed.push(c.player_id);
                    owed.push(c.player_id);
                }
                for pid in owed {
                    planned.push((hex.q, hex.r, pid, hex.resource));
                }
            }
        }

        if planned.is_empty() {
            return;
        }
        planned.truncate(MAX_CARDS);

        set_timeout(
            move || {
                let mut batch = Vec::with_capacity(planned.len());
                for (i, (q, r, pid, res)) in planned.iter().enumerate() {
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
                if batch.is_empty() {
                    return;
                }
                let ids: Vec<u64> = batch.iter().map(|f| f.id).collect();
                let longest = batch.last().map_or(0, |f| f.delay) + FLIGHT_MS + 100;
                flyers.update(|v| v.extend(batch));

                // Each batch clears itself: a shared timer would cut short
                // cards from a roll that landed while these were still up.
                set_timeout(
                    move || flyers.update(|v| v.retain(|f| !ids.contains(&f.id))),
                    std::time::Duration::from_millis(longest),
                );
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
                    let style = format!(
                        "--x0: {:.1}px; --y0: {:.1}px; --x1: {:.1}px; --y1: {:.1}px; \
                         --dur: {}ms; --delay: {}ms; margin-left: -19px; margin-top: -26px;",
                        f.from.0, f.from.1, f.to.0, f.to.1, FLIGHT_MS, f.delay,
                    );
                    view! {
                        <img
                            class="flyer w-[38px] h-[52px] rounded-[4px] object-cover
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
