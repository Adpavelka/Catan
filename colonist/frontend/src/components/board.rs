use leptos::*;
use crate::state::{GameState, BuildMode};
use crate::components::icons::{asset_url, port_art, DieFace};
use shared::{BuildingInfo, ClientRequest, ResourceType, HexInfo, PortInfo};
use uuid::Uuid;

// Convert axial coordinates (q, r) to pixel coordinates for SVG
fn axial_to_pixel(q: i32, r: i32, size: f32) -> (f32, f32) {
    let x = size * (3f32.sqrt() * q as f32 + 3f32.sqrt() / 2.0 * r as f32);
    let y = size * (3.0 / 2.0 * r as f32);
    (x, y)
}

// Get color for a resource type
fn resource_color(resource: &ResourceType) -> &'static str {
    match resource {
        ResourceType::Wood => "fill-emerald-600",
        ResourceType::Brick => "fill-orange-400",
        ResourceType::Sheep => "fill-lime-500",
        ResourceType::Wheat => "fill-yellow-500",
        ResourceType::Ore => "fill-slate-500",
        ResourceType::Desert => "fill-amber-200",
    }
}

// Get label for a resource type
fn resource_label(resource: &ResourceType) -> &'static str {
    match resource {
        ResourceType::Wood => "WOOD",
        ResourceType::Brick => "BRICK",
        ResourceType::Sheep => "SHEEP",
        ResourceType::Wheat => "WHEAT",
        ResourceType::Ore => "ORE",
        ResourceType::Desert => "DESERT",
    }
}


// Get player color (for rendering buildings)
/// The player's colour as a literal hex, applied via the SVG `fill` attribute.
/// Deliberately not a Tailwind class: those only exist if Tailwind happens to
/// scan the file the string literal lives in.
fn player_color(player_id: Uuid, state: &GameState) -> &'static str {
    state.players.get_untracked()
        .iter()
        .find(|p| p.player_id == player_id)
        .map(|p| p.colour.hex())
        .unwrap_or("#a855f7")
}

// Calculate vertex positions for a hex (matches backend logic)
// Backend formula: vertex[i] = base + neighbours[i] + neighbours[(i+1)%6]
fn get_hex_vertices(q: i32, r: i32) -> Vec<(i32, i32)> {
    let base = (q * 3, r * 3);
    let neighbors = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];

    let mut vertices = Vec::with_capacity(6);
    for i in 0..6 {
        let curr = neighbors[i];
        let next = neighbors[(i + 1) % 6];
        vertices.push((
            base.0 + curr.0 + next.0,
            base.1 + curr.1 + next.1,
        ));
    }
    vertices
}

// Convert vertex coordinates (in vertex space) to pixel coordinates
// Vertex coordinates are in units where hex base = (q*3, r*3)
// Convert to axial coordinates by dividing by 3, then use standard axial-to-pixel conversion
fn vertex_to_pixel(vx: i32, vy: i32, hex_size: f32) -> (f32, f32) {
    let q = vx as f32 / 3.0;
    let r = vy as f32 / 3.0;
    let x = hex_size * (3f32.sqrt() * q + 3f32.sqrt() / 2.0 * r);
    let y = hex_size * (3.0 / 2.0 * r);
    (x, y)
}



/// Vertices where `me` may put a settlement.
///
/// Mirrors the server: the space must be empty, no neighbouring vertex may be
/// built on, and outside initial placement it has to touch one of your roads.
/// Offering anything else just invites a click that comes back as an error.
fn legal_settlements(
    hexes: &[HexInfo],
    settlements: &[BuildingInfo],
    cities: &[BuildingInfo],
    roads: &[BuildingInfo],
    me: Uuid,
    initial: bool,
) -> Vec<(i32, i32)> {
    use std::collections::HashSet;

    let taken: HashSet<(i32, i32)> = settlements
        .iter()
        .chain(cities.iter())
        .map(|b| (b.x, b.y))
        .collect();

    let edges = calculate_all_edges(hexes);
    let my_edges: HashSet<(i32, i32)> = roads
        .iter()
        .filter(|r| r.player_id == me)
        .map(|r| (r.x, r.y))
        .collect();

    // Vertices touched by one of my roads.
    let mine_reach: HashSet<(i32, i32)> = edges
        .iter()
        .filter(|(_, _, e)| my_edges.contains(e))
        .flat_map(|(a, b, _)| [*a, *b])
        .collect();

    calculate_all_vertices(hexes)
        .into_iter()
        .filter(|v| !taken.contains(v))
        .filter(|v| {
            // Distance rule: no neighbour along a shared edge may be occupied.
            !edges.iter().any(|(a, b, _)| {
                (a == v && taken.contains(b)) || (b == v && taken.contains(a))
            })
        })
        .filter(|v| initial || mine_reach.contains(v))
        .collect()
}

/// Edges where `me` may put a road: empty, and touching something of theirs.
/// During initial placement it must touch the settlement just placed.
fn legal_roads(
    hexes: &[HexInfo],
    settlements: &[BuildingInfo],
    cities: &[BuildingInfo],
    roads: &[BuildingInfo],
    me: Uuid,
    must_touch: Option<(i32, i32)>,
) -> Vec<(i32, i32)> {
    use std::collections::HashSet;

    let occupied: HashSet<(i32, i32)> = roads.iter().map(|r| (r.x, r.y)).collect();
    let my_buildings: HashSet<(i32, i32)> = settlements
        .iter()
        .chain(cities.iter())
        .filter(|b| b.player_id == me)
        .map(|b| (b.x, b.y))
        .collect();

    let edges = calculate_all_edges(hexes);
    let my_road_ends: HashSet<(i32, i32)> = edges
        .iter()
        .filter(|(_, _, e)| roads.iter().any(|r| r.player_id == me && (r.x, r.y) == *e))
        .flat_map(|(a, b, _)| [*a, *b])
        .collect();

    edges
        .into_iter()
        .filter(|(_, _, e)| !occupied.contains(e))
        .filter(|(a, b, _)| match must_touch {
            Some(v) => *a == v || *b == v,
            None => {
                my_buildings.contains(a)
                    || my_buildings.contains(b)
                    || my_road_ends.contains(a)
                    || my_road_ends.contains(b)
            }
        })
        .map(|(_, _, e)| e)
        .collect()
}

// Calculate all unique vertices from hexes
fn calculate_all_vertices(hexes: &[HexInfo]) -> Vec<(i32, i32)> {
    use std::collections::HashSet;
    let mut vertices = HashSet::new();

    for hex in hexes {
        for vertex in get_hex_vertices(hex.q, hex.r) {
            vertices.insert(vertex);
        }
    }

    vertices.into_iter().collect()
}

// Calculate edge coordinate from two vertices (matches backend edge_key formula)
fn edge_key(v1: (i32, i32), v2: (i32, i32)) -> (i32, i32) {
    ((v1.0 + v2.0) / 3, (v1.1 + v2.1) / 3)
}

// Given an edge coordinate, find the two vertices it connects
// Returns None if the edge doesn't exist in the hex grid
fn find_vertices_for_edge(edge_coord: (i32, i32), hexes: &[HexInfo]) -> Option<((i32, i32), (i32, i32))> {
    let all_edges = calculate_all_edges(hexes);
    all_edges.into_iter()
        .find(|(_, _, e)| *e == edge_coord)
        .map(|(v1, v2, _)| (v1, v2))
}

// Calculate all unique edges from hexes with their vertex endpoints
// Returns: Vec<(vertex1, vertex2, edge_coord)>
fn calculate_all_edges(hexes: &[HexInfo]) -> Vec<((i32, i32), (i32, i32), (i32, i32))> {
    use std::collections::HashMap;
    let mut edges: HashMap<(i32, i32), ((i32, i32), (i32, i32))> = HashMap::new();

    for hex in hexes {
        let vertices = get_hex_vertices(hex.q, hex.r);

        // Create edges between consecutive vertices
        for i in 0..6 {
            let v1 = vertices[i];
            let v2 = vertices[(i + 1) % 6];
            let edge_coord = edge_key(v1, v2);

            // Store the edge with its vertices (avoid duplicates with HashMap)
            edges.insert(edge_coord, (v1, v2));
        }
    }

    edges.into_iter()
        .map(|(edge_coord, (v1, v2))| (v1, v2, edge_coord))
        .collect()
}

#[component]
pub fn Board() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");


    // Check if it's my turn
    let is_my_turn = move || {
        if let Some(my_id) = state.player_id.get() {
            state.current_turn_player.get() == my_id
        } else {
            false
        }
    };


    // During setup there is exactly one thing you may do at any moment, and
    // the phase already says which. Making the player arm a build mode for it
    // is a click that can only be made one way, so the board arms itself and
    // moves on to the road the moment the settlement lands.
    create_effect(move |_| {
        let phase = state.game_phase.get();
        let mine = state.player_id.get() == Some(state.current_turn_player.get());

        let wanted = match (phase, mine) {
            (shared::GamePhase::InitialPlacement { step, .. }, true) => match step {
                shared::PlacementStep::BuildSettlement => BuildMode::Settlement,
                shared::PlacementStep::BuildRoad { .. } => BuildMode::Road,
            },
            // Somebody else's placement, or setup is over: leave whatever the
            // player has armed alone once we are in regular play.
            (shared::GamePhase::InitialPlacement { .. }, false) => BuildMode::None,
            _ => return,
        };

        if state.build_mode.get_untracked() != wanted {
            state.build_mode.set(wanted);
        }
    });

    // View transform on top of the fit-to-viewport one. Kept here rather than
    // in GameState because nothing outside the board cares about it.
    let (zoom, set_zoom) = create_signal(1.0f64);
    let (pan, set_pan) = create_signal((0.0f64, 0.0f64));
    let (dragging, set_dragging) = create_signal(Option::<(f64, f64)>::None);

    const MIN_ZOOM: f64 = 0.6;
    const MAX_ZOOM: f64 = 3.0;
    let clamp_zoom = |z: f64| z.clamp(MIN_ZOOM, MAX_ZOOM);

    let reset_view = move || {
        set_zoom.set(1.0);
        set_pan.set((0.0, 0.0));
    };

    view! {
        <div class="relative w-full h-full flex flex-col min-h-0 min-w-0 gap-2">
            // The board takes every pixel the column can spare; the control
            // bar below is the only fixed-height part.
            <svg
                viewBox="0 0 1000 800"
                class=move || format!(
                    "w-full flex-1 min-h-0 drop-shadow-2xl relative z-0 {}",
                    if dragging.get().is_some() { "cursor-grabbing" } else { "cursor-grab" }
                )
                preserveAspectRatio="xMidYMid meet"
                on:wheel=move |ev| {
                    ev.prevent_default();
                    let factor = if ev.delta_y() < 0.0 { 1.12 } else { 1.0 / 1.12 };
                    set_zoom.update(|z| *z = clamp_zoom(*z * factor));
                }
                on:pointerdown=move |ev| set_dragging.set(Some((ev.client_x() as f64, ev.client_y() as f64)))
                on:pointermove=move |ev| {
                    let Some((lx, ly)) = dragging.get() else { return };
                    let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
                    // Undo the zoom so a drag moves the board with the cursor
                    // rather than racing ahead of it when zoomed in.
                    let z = zoom.get().max(0.01);
                    set_pan.update(|(px, py)| {
                        *px += (x - lx) / z;
                        *py += (y - ly) / z;
                    });
                    set_dragging.set(Some((x, y)));
                }
                on:pointerup=move |_| set_dragging.set(None)
                on:pointerleave=move |_| set_dragging.set(None)
            >

                // One tint per player colour. The pieces are drawn from
                // the same artwork as the buy buttons, so a filter is what
                // makes a settlement *yours*: flood the shape with your
                // colour, then multiply the artwork's own shading back over
                // it so it keeps its modelling instead of going flat.
                <defs>
                    {shared::PlayerColour::ALL.map(|c| {
                        let hex = c.hex();
                        view! {
                            <filter id=tint_id(hex) color-interpolation-filters="sRGB">
                                <feFlood flood-color=hex result="flat" />
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
                        }
                    }).to_vec()}
                </defs>

                // Center the board horizontally, move up vertically
                // The board is drawn at a fixed hex size and then scaled as a
                // whole, so the 30-hex extension board fits the same viewport
                // as the 19-hex one without touching every coordinate.
                <g transform=move || {
                    let (px, py) = pan.get();
                    format!("translate(500, 400) scale({:.4}) translate({:.2}, {:.2}) translate(-500, -400)",
                            zoom.get(), px, py)
                }>
                <g transform=move || {
                    let hexes = state.hexes.get();
                    if hexes.is_empty() {
                        return "translate(500, 320)".to_string();
                    }

                    let points: Vec<(f32, f32)> = hexes
                        .iter()
                        .map(|h| axial_to_pixel(h.q, h.r, 60.0))
                        .collect();

                    // Room for the hex itself plus the harbour markers outside it.
                    let margin = 115.0;
                    let min_x = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min) - margin;
                    let max_x = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) + margin;
                    let min_y = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min) - margin;
                    let max_y = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) + margin;

                    // No upper clamp: a 19-hex board is smaller than the
                    // viewport, and capping at 1.0 left it marooned in the
                    // middle of a mostly empty column.
                    let scale = (980.0 / (max_x - min_x))
                        .min(780.0 / (max_y - min_y));

                    format!(
                        "translate(500, 400) scale({:.4}) translate({:.1}, {:.1})",
                        scale,
                        -(min_x + max_x) / 2.0,
                        -(min_y + max_y) / 2.0,
                    )
                }>
                    // The island itself. Drawn as one layer of oversized
                    // hexes before any tile, so the gaps between tiles read as
                    // ground rather than sea showing through. It has to be its
                    // own pass: done inside each tile, a later tile's ground
                    // would paint over the previous tile's face.
                    // Hex centres are 60*sqrt(3) apart but the tiles are only
                    // drawn at radius 50, so they never touch. These fill the
                    // grid cell and then some: at radius 62 neighbouring
                    // ground hexes overlap, so the island is one continuous
                    // mass with no sea showing through the joins.
                    <g class="pointer-events-none">
                        <For
                            each=move || state.hexes.get()
                            key=|hex| (hex.q, hex.r)
                            children=move |hex: HexInfo| {
                                let (px, py) = axial_to_pixel(hex.q, hex.r, 60.0);
                                view! {
                                    <polygon
                                        points="0,-62 53.7,-31 53.7,31 0,62 -53.7,31 -53.7,-31"
                                        transform=format!("translate({}, {})", px, py)
                                        fill="#5f5433"
                                    />
                                }
                            }
                        />
                        // A lighter wash inside it, so the ground has some
                        // depth and the tile joins read as furrows rather than
                        // as a flat brown mat.
                        <For
                            each=move || state.hexes.get()
                            key=|hex| (hex.q, hex.r)
                            children=move |hex: HexInfo| {
                                let (px, py) = axial_to_pixel(hex.q, hex.r, 60.0);
                                view! {
                                    <polygon
                                        points="0,-59 51,-29.5 51,29.5 0,59 -51,29.5 -51,-29.5"
                                        transform=format!("translate({}, {})", px, py)
                                        fill="#7a6d44"
                                    />
                                }
                            }
                        />
                    </g>

                    // Render all hexes.
                    //
                    // The key carries the tile's contents, not just its place.
                    // `For` is keyed: for a key it has already drawn it keeps
                    // the existing nodes and never re-runs this child, and
                    // `HexTile` reads its resource and number once, when built.
                    // Every board uses the same coordinates, so keying on
                    // (q, r) alone meant a second game in the same page
                    // redrew nothing - you sat looking at the previous game's
                    // land while the server dealt you another.
                    <For
                        each=move || state.hexes.get()
                        key=|hex| (hex.q, hex.r, hex.resource, hex.number)
                        children=move |hex: HexInfo| {
                            let (px, py) = axial_to_pixel(hex.q, hex.r, 60.0);
                            let (hex_q, hex_r) = (hex.q, hex.r);
                            let hex_clone = hex.clone();
                            let state_click = state.clone();

                            view! {
                                <g>
                                    <HexTile
                                        x=px
                                        y=py
                                        hex=hex
                                    />
                                    // Robber placement targets. Only the hexes
                                    // that are actually legal light up: the one
                                    // the robber already sits on is not a move,
                                    // and filling every hex turned the whole
                                    // board red instead of pointing anywhere.
                                    <Show when=move || {
                                        state_click.must_move_robber.get()
                                            && state_click.robber_pos.get() != Some((hex_q, hex_r))
                                    }>
                                        <polygon
                                            points="0,-50 43,-25 43,25 0,50 -43,25 -43,-25"
                                            transform=format!("translate({}, {})", px, py)
                                            class="fill-transparent hover:fill-red-500/30 stroke-red-400/70 hover:stroke-red-400 [stroke-width:3] [stroke-dasharray:6_5] hover:[stroke-dasharray:none] cursor-pointer transition-all"
                                            on:click=move |_| {
                                                state_click.send(ClientRequest::MoveRobber {
                                                    q: hex_clone.q,
                                                    r: hex_clone.r,
                                                });
                                                state_click.must_move_robber.set(false);
                                            }
                                        />
                                    </Show>
                                </g>
                            }
                        }
                    />

                    // Render ports around the coast (from server data)
                    <g class="ports" style="pointer-events: none;">
                        <For
                            each=move || state.board_ports.get()
                            // The type belongs in the key for the same reason
                            // it does on the hexes: harbours sit on the same
                            // coastline every game, only what they trade
                            // changes.
                            key=|port| (port.vertices[0], port.vertices[1], port.port_type)
                            children=move |port: PortInfo| {
                                let (v1x, v1y) = vertex_to_pixel(port.vertices[0].0, port.vertices[0].1, 60.0);
                                let (v2x, v2y) = vertex_to_pixel(port.vertices[1].0, port.vertices[1].1, 60.0);

                                // Calculate midpoint between the two vertices
                                let mid_x = (v1x + v2x) / 2.0;
                                let mid_y = (v1y + v2y) / 2.0;

                                // Calculate outward normal (perpendicular to edge, pointing away from center)
                                let edge_dx = v2x - v1x;
                                let edge_dy = v2y - v1y;
                                let edge_len = (edge_dx * edge_dx + edge_dy * edge_dy).sqrt();

                                // Normal vector (perpendicular)
                                let nx = -edge_dy / edge_len;
                                let ny = edge_dx / edge_len;

                                // "Outward" is away from the hex this harbour sits on, not away
                                // from the board's centre: on a coastline with any concavity -
                                // as the extension board has - those are not the same direction,
                                // and the marker ends up drawn on top of the land.
                                let (hx, hy) = state.hexes.get_untracked()
                                    .iter()
                                    .map(|h| axial_to_pixel(h.q, h.r, 60.0))
                                    .min_by(|a, b| {
                                        let da = (a.0 - mid_x).powi(2) + (a.1 - mid_y).powi(2);
                                        let db = (b.0 - mid_x).powi(2) + (b.1 - mid_y).powi(2);
                                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                                    })
                                    .unwrap_or((0.0, 0.0));

                                let away = (mid_x - hx) * nx + (mid_y - hy) * ny;
                                let (out_nx, out_ny) = if away >= 0.0 { (nx, ny) } else { (-nx, -ny) };

                                // Push port marker outward from the edge
                                let port_x = mid_x + out_nx * 62.0;
                                let port_y = mid_y + out_ny * 62.0;

                                // The walkways meet the underside of the
                                // hull. The boat is drawn upright whatever
                                // direction the harbour faces, so "under" is
                                // straight down the screen from its centre -
                                // aiming at the centre instead buried the
                                // planks in the middle of the boat.
                                let dock_x = port_x;
                                let dock_y = port_y + 19.0;

                                view! {
                                    <g>
                                        // A plank walkway out to each of the
                                        // two corners this harbour serves.
                                        <Pier from=(v1x, v1y) to=(dock_x, dock_y) />
                                        <Pier from=(v2x, v2y) to=(dock_x, dock_y) />

                                        <g transform=format!("translate({}, {})", port_x, port_y)>
                                            // The harbour badge and nothing
                                            // else. The sail already carries
                                            // both halves of the deal - which
                                            // resource, at what rate - so a
                                            // caption under it said the same
                                            // thing twice. The artwork sits
                                            // straight on the water; a disc
                                            // behind it only boxed it in.
                                            <image
                                                href=asset_url(port_art(&port.port_type))
                                                x="-24" y="-26" width="48" height="48"
                                                style="filter: drop-shadow(0 2px 3px rgb(0 0 0 / 0.45));"
                                            />
                                        </g>
                                    </g>
                                }
                            }
                        />
                    </g>

                    // Render robber
                    {move || {
                        if let Some((robber_q, robber_r)) = state.robber_pos.get() {
                            let robber_asset = state.robber_asset.get();
                            let (px, py) = axial_to_pixel(robber_q, robber_r, 60.0);
                            view! {
                                <g transform=format!("translate({}, {})", px, py)>
                                    // Just the piece, standing on the tile the
                                    // way it stands on a real board. It used to
                                    // sit in a dark disc with a red ring, which
                                    // read as a warning badge rather than as a
                                    // playing piece. A shadow keeps it legible
                                    // against the pale desert instead.
                                    <image
                                        href=asset_url(&robber_asset)
                                        x="-19" y="-19" width="38" height="38"
                                        class="pointer-events-none"
                                        style="filter: drop-shadow(0 2px 3px rgb(0 0 0 / 0.55));"
                                    />
                                </g>
                            }.into_view()
                        } else {
                            view! { <g></g> }.into_view()
                        }
                    }}

                    // Render interactive vertices (for placing settlements/cities)
                    <Show when=move || {
                        let mode = state.build_mode.get();
                        mode == BuildMode::Settlement || mode == BuildMode::City
                    }>
                        <For
                            each=move || {
                                let me = state.player_id.get().unwrap_or_else(Uuid::nil);
                                let hexes = state.hexes.get();
                                let settlements = state.settlements.get();
                                let cities = state.cities.get();

                                // Upgrading shows your own settlements; placing
                                // shows only the spots the server will accept.
                                if state.build_mode.get() == BuildMode::City {
                                    settlements
                                        .iter()
                                        .filter(|b| b.player_id == me)
                                        .map(|b| (b.x, b.y))
                                        .collect::<Vec<_>>()
                                } else {
                                    legal_settlements(
                                        &hexes,
                                        &settlements,
                                        &cities,
                                        &state.roads.get(),
                                        me,
                                        state.game_phase.get().is_initial_phase(),
                                    )
                                }
                            }
                            key=|v| *v
                            children=move |vertex| {
                                let (vx, vy) = vertex_to_pixel(vertex.0, vertex.1, 60.0);
                                let state_click = state.clone();

                                // Zjistíme, jestli tady už je osada (pro upgrade na město)
                                let has_my_settlement = move || {
                                    let my_id = state_click.player_id.get();
                                    state_click.settlements.get().iter().any(|s|
                                        (s.x, s.y) == vertex && Some(s.player_id) == my_id
                                    )
                                };

                                let upgrading = move || {
                                    state_click.build_mode.get() == BuildMode::City
                                        && has_my_settlement()
                                };

                                view! {
                                    // A ring when upgrading, so it reads as a
                                    // halo around the settlement it replaces;
                                    // a dot when placing, where there is
                                    // nothing on the spot yet.
                                    <circle
                                        cx=vx
                                        cy=vy
                                        r=move || if upgrading() { "21" } else { "10" }
                                        stroke-width=move || if upgrading() { "5" } else { "1" }
                                        class=move || if upgrading() {
                                            "cursor-pointer fill-transparent stroke-[#ffb718] \
                                             hover:stroke-[#ffd76b] animate-pulse transition-all"
                                        } else {
                                            "cursor-pointer stroke-black fill-white/30 \
                                             hover:fill-white/80 transition-all"
                                        }
                                        on:click=move |_| {
                                            match state_click.build_mode.get() {
                                                BuildMode::Settlement => {
                                                    state_click.send(ClientRequest::BuildSettlement { x: vertex.0, y: vertex.1 });
                                                    state_click.build_mode.set(BuildMode::None);
                                                },
                                                BuildMode::City => {
                                                    state_click.send(ClientRequest::BuildCity { x: vertex.0, y: vertex.1 });
                                                    state_click.build_mode.set(BuildMode::None);
                                                },
                                                _ => {}
                                            }
                                        }
                                    />
                                }
                            }
                        />
                    </Show>
                    // Render interactive edges (for placing roads)
                    <Show when=move || state.build_mode.get() == BuildMode::Road>
                        {move || {
                            let hexes = state.hexes.get();
                            let roads = state.roads.get();
                            let me = state.player_id.get().unwrap_or_else(Uuid::nil);

                            // In initial placement the road has to touch the
                            // settlement just placed, and the phase carries
                            // which one that was.
                            let must_touch = match state.game_phase.get() {
                                shared::GamePhase::InitialPlacement {
                                    step: shared::PlacementStep::BuildRoad { settlement }, ..
                                } => Some(settlement),
                                _ => None,
                            };

                            let legal = legal_roads(
                                &hexes,
                                &state.settlements.get(),
                                &state.cities.get(),
                                &roads,
                                me,
                                must_touch,
                            );

                            let all_edges = calculate_all_edges(&hexes);
                            let available_edges: Vec<_> = all_edges.into_iter()
                                .filter(|(_, _, edge_coord)| legal.contains(edge_coord))
                                .collect();

                            view! {
                                <For
                                    each=move || available_edges.clone()
                                    key=|e| e.2
                                    children=move |(v1, v2, edge_coord)| {
                                        let (v1x, v1y) = vertex_to_pixel(v1.0, v1.1, 60.0);
                                        let (v2x, v2y) = vertex_to_pixel(v2.0, v2.1, 60.0);

                                        let state_click = state.clone();

                                        view! {
                                            <line
                                                x1=v1x
                                                y1=v1y
                                                x2=v2x
                                                y2=v2y
                                                class="stroke-white/50 hover:stroke-green-500 stroke-[4] cursor-pointer transition-all"
                                                on:click=move |_| {
                                                    state_click.send(ClientRequest::BuildRoad { x1: edge_coord.0, y1: edge_coord.1 });
                                                    state_click.build_mode.set(BuildMode::None);
                                                }
                                            />
                                        }
                                    }
                                />
                            }
                        }}
                    </Show>

                    // Roads, drawn before the buildings on purpose: a
                    // road ends at a vertex somebody has built on, and drawn
                    // afterwards it would cover their piece.
                    <For
                        each=move || state.roads.get()
                        key=|r| (r.x, r.y, r.player_id)
                        children=move |road| {
                            let hexes = state.hexes.get();
                            let Some((v1, v2)) = find_vertices_for_edge((road.x, road.y), &hexes)
                            else {
                                return view! { <g></g> }.into_view();
                            };

                            let (v1x, v1y) = vertex_to_pixel(v1.0, v1.1, 60.0);
                            let (v2x, v2y) = vertex_to_pixel(v2.0, v2.1, 60.0);
                            let colour = player_color(road.player_id, &state);

                            view! {
                                <PlacedRoad
                                    from=(v1x, v1y)
                                    to=(v2x, v2y)
                                    colour=colour
                                />
                            }.into_view()
                        }
                    />

                    // Render settlements
                    <For
                        each=move || state.settlements.get()
                        key=|s| (s.x, s.y, s.player_id)
                        children=move |settlement| {
                            let (px, py) = vertex_to_pixel(settlement.x, settlement.y, 60.0);
                            view! {
                                <Settlement
                                    x=px
                                    y=py
                                    player_id=settlement.player_id
                                />
                            }
                        }
                    />

                    // Render cities
                    <For
                        each=move || state.cities.get()
                        key=|c| (c.x, c.y, c.player_id)
                        children=move |city| {
                            let (px, py) = vertex_to_pixel(city.x, city.y, 60.0);
                            view! {
                                <City
                                    x=px
                                    y=py
                                    player_id=city.player_id
                                />
                            }
                        }
                    />

                </g>
                </g>
            </svg>

            // Zoom controls, floated over the top-right of the water. The dice
            // are not here: they belong to the bottom HUD, stacked above the
            // turn panel, so the whole right-hand column reads top to bottom.


            <div class="absolute top-2 right-2 z-10 flex flex-col gap-1">
                <button
                    class="game-btn game-btn-cream w-8 h-8 font-black leading-none"
                    title="Zoom in"
                    on:click=move |_| set_zoom.update(|z| *z = clamp_zoom(*z * 1.25))
                >"+"</button>
                <button
                    class="game-btn game-btn-cream w-8 h-8 font-black leading-none"
                    title="Zoom out"
                    on:click=move |_| set_zoom.update(|z| *z = clamp_zoom(*z / 1.25))
                >"\u{2212}"</button>
                <button
                    class="game-btn game-btn-cream w-8 h-8 text-[9px] font-black"
                    title="Reset the view"
                    on:click=move |_| reset_view()
                >"FIT"</button>
            </div>

            // Status banner, floated over the foot of the board. Only rendered
            // when it has something to say - the dice and build buttons moved
            // out from under here, and an always-on container left an empty
            // pill sitting on the water.
            <Show when=move || {
                let phase = state.game_phase.get();
                (phase.is_initial_phase() && !is_my_turn()) || phase.special_builder().is_some()
            }>
            <div class="absolute bottom-3 left-1/2 -translate-x-1/2 z-10 flex justify-center">
                <div class="panel flex flex-wrap items-center justify-center gap-3 px-4 py-2.5">
                // Show initial placement instructions
                // On your own setup turn the board arms itself and lights up
                // the legal spots, so there is nothing to say. It is only worth
                // a line when you are waiting on somebody else.
                {move || {
                    if state.game_phase.get().is_initial_phase() && !is_my_turn() {
                        view! {
                            <div class="text-[#6b6354] text-sm font-bold px-4 py-2">
                                "Waiting for other players..."
                            </div>
                        }.into_view()
                    } else {
                        view! { <div></div> }.into_view()
                    }
                }}

                // Special building phase banner (5-6 player games)
                <Show when=move || state.game_phase.get().special_builder().is_some()>
                    <div class=move || if state.is_my_special_build() {
                        "px-4 py-2 rounded-xl border-2 border-emerald-500 bg-emerald-600/15 text-emerald-300 font-bold text-sm"
                    } else {
                        "px-4 py-2 rounded-xl border border-slate-700 bg-slate-900/70 text-slate-400 font-bold text-sm"
                    }>
                        {move || {
                            if state.is_my_special_build() {
                                "SPECIAL BUILD - build or buy, then pass".to_string()
                            } else {
                                let who = state
                                    .game_phase
                                    .get()
                                    .special_builder()
                                    .and_then(|id| {
                                        state.players.get().iter()
                                            .find(|p| p.player_id == id)
                                            .map(|p| p.name.clone())
                                    })
                                    .unwrap_or_else(|| "Someone".to_string());
                                format!("SPECIAL BUILD - waiting for {who}")
                            }
                        }}
                    </div>
                </Show>


                </div>
            </div>
            </Show>
        </div>
    }
}

/// The dice. Always on screen, showing whatever was last rolled - by anybody -
/// so the table never has to go hunting in the log for it. On your own turn,
/// before you have rolled, they are also the roll button, and they tumble for
/// a moment rather than snapping straight to the answer.
///
/// They sit loose on the water rather than in a panel.
#[component]
pub fn DiceTray() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let (tumbling, set_tumbling) = create_signal(false);
    let (shown, set_shown) = create_signal((1u8, 1u8));

    let is_my_turn = move || state.player_id.get() == Some(state.current_turn_player.get());
    let can_roll = move || {
        is_my_turn()
            && state.last_dice_roll.get().is_none()
            && state.game_phase.get() == shared::GamePhase::RegularPlay
            && !tumbling.get()
    };

    // While tumbling, show nonsense; the effect below settles on the real
    // numbers the moment the server's answer lands.
    create_effect(move |prev: Option<Option<leptos_dom::helpers::IntervalHandle>>| {
        if let Some(Some(h)) = prev {
            h.clear();
        }
        if !tumbling.get() {
            return None;
        }
        set_interval_with_handle(
            move || {
                let t = js_sys::Date::now() as u64;
                set_shown.set(((t % 6) as u8 + 1, ((t / 7) % 6) as u8 + 1));
            },
            std::time::Duration::from_millis(70),
        )
        .ok()
    });

    create_effect(move |_| {
        if let Some((a, b)) = state.table_last_roll.get() {
            set_tumbling.set(false);
            set_shown.set((a, b));
        }
    });

    let roll = move |_| {
        if !can_roll() {
            return;
        }
        set_tumbling.set(true);
        state.send(ClientRequest::RollDice);
        // A floor on the animation, so a fast reply still reads as a roll.
        set_timeout(move || set_tumbling.set(false), std::time::Duration::from_millis(550));
    };

    view! {
        <button
            class=move || format!(
                "flex items-center gap-3 rounded-2xl transition-transform {}",
                if can_roll() { "hover:scale-[1.04] cursor-pointer" } else { "cursor-default" }
            )
            disabled=move || !can_roll()
            title=move || if can_roll() {
                "Roll the dice".to_string()
            } else {
                let (a, b) = shown.get();
                format!("The last roll: {}", a + b)
            }
            on:click=roll
        >
            {move || {
                let (a, b) = shown.get();
                // Waiting to be thrown: a slow breath. Mid-throw: a tumble.
                // Otherwise still, because they are just showing a result.
                let motion = if tumbling.get() {
                    "animate-bounce"
                } else if can_roll() {
                    "dice-waiting"
                } else {
                    ""
                };
                view! {
                    <div class=format!("drop-shadow-[0_3px_5px_rgba(0,0,0,0.3)] {motion}")>
                        <DieFace value=a size="w-[136px] h-[136px]" />
                    </div>
                    <div class=format!("drop-shadow-[0_3px_5px_rgba(0,0,0,0.3)] {motion}")>
                        <DieFace value=b size="w-[136px] h-[136px]" />
                    </div>
                }
            }}
        </button>
    }
}

#[component]
fn HexTile(x: f32, y: f32, hex: HexInfo) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let points = "0,-50 43,-25 43,25 0,50 -43,25 -43,-25";
    let color = resource_color(&hex.resource);
    let label = resource_label(&hex.resource);

    // How many of the 36 dice combinations make this number: 6 and 8 are the
    // richest, 2 and 12 the poorest. Shown as pips, the way the real tokens do.
    let pips = if hex.number == 0 { 0 } else { 6u8.saturating_sub((7i8 - hex.number as i8).unsigned_abs()) };
    let hot = hex.number == 6 || hex.number == 8;

    // Did the table just roll this tile's number? At a real table you look at
    // the dice and then scan the board; this does the scanning for you.
    let (q, r) = (hex.q, hex.r);
    let struck = move || {
        state.last_roll_event.get().is_some_and(|(total, _)| total == hex.number)
    };
    // The robber stops a tile producing, so a tile under it does not light up.
    let blocked = move || state.robber_pos.get() == Some((q, r));
    let producing = move || struck() && !blocked();

    view! {
        <g
            transform=format!("translate({}, {})", x, y)
            class="group"
            data-hex=format!("{q},{r}")
        >
            <polygon points=points class=format!("{} stroke-black/30 [stroke-width:2]", color) />

            // The flash: a bright rim that fades, plus a steady glow while the
            // roll stands, so a tile that paid out stays findable afterwards.
            <Show when=producing>
                <g class="pointer-events-none">
                    <polygon points=points class="fill-white/25 hex-flash" />
                    <polygon
                        points=points fill="none"
                        stroke="#ffe066" stroke-width="5" stroke-linejoin="round"
                        style="filter: drop-shadow(0 0 6px #ffd21e);"
                    />
                </g>
            </Show>
            // A little inner shading so the tiles read as solid, not flat.
            <polygon points=points class="fill-none stroke-white/10" stroke-width="1"
                     transform="scale(0.93)" />

            <text
                y="-26"
                text-anchor="middle"
                class="fill-white/80 text-[11px] font-bold pointer-events-none uppercase tracking-[0.15em]"
                style="text-shadow: 0 1px 3px rgba(0,0,0,0.9)"
            >
                {label}
            </text>

            // The number token, centred in the hex like the cardboard chit it
            // stands in for - it used to be an off-centre dark blob shared
            // with the resource label.
            <Show when=move || hex.number != 0 && hex.number != 7>
                <g transform="translate(0, 8)" class="pointer-events-none">
                    <circle r="21" class="fill-black/25" cy="2" />
                    <circle
                        r="20" fill="#f4ecd8"
                        stroke=move || if producing() { "#e8a300" } else { "#0f172a" }
                        stroke-opacity=move || if producing() { "1" } else { "0.35" }
                        stroke-width=move || if producing() { "3" } else { "1.5" }
                    />
                    <text
                        y="-1"
                        text-anchor="middle"
                        dominant-baseline="central"
                        class=move || if hot {
                            "text-[21px] font-black fill-red-600"
                        } else {
                            "text-[21px] font-black fill-slate-900"
                        }
                    >
                        {hex.number}
                    </text>
                    {(0..pips).map(|i| {
                        let spread = 4.5;
                        let cx = (i as f32 - (pips as f32 - 1.0) / 2.0) * spread;
                        view! {
                            <circle
                                cx=cx cy="12.5" r="1.5"
                                class=move || if hot { "fill-red-600" } else { "fill-slate-900" }
                            />
                        }
                    }).collect_view()}
                </g>
            </Show>
        </g>
    }
}

/// Lay a piece of artwork along the line from `from` to `to`.
///
/// The road and pier assets are both drawn standing up, so putting one on an
/// edge is: move to the middle, turn to face along the edge, then draw it
/// centred. Returns the SVG transform for that.
fn along(from: (f32, f32), to: (f32, f32)) -> (String, f32) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    // The artwork's long axis points down the +y axis, which is 90 degrees,
    // so the turn needed is the edge's bearing less that.
    let angle = dy.atan2(dx).to_degrees() - 90.0;
    let (mx, my) = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    (format!("translate({mx:.2}, {my:.2}) rotate({angle:.2})"), length)
}

/// A road on the board: the same artwork the buy button shows, tinted to its
/// owner and laid along the edge.
#[component]
fn PlacedRoad(from: (f32, f32), to: (f32, f32), colour: &'static str) -> impl IntoView {
    let (transform, length) = along(from, to);
    // Short of the full edge, so two roads meeting at a vertex leave the
    // piece standing there room to breathe.
    let long = length * 0.84;
    let thick = 15.0_f32;

    view! {
        <g transform=transform>
            <image
                href=asset_url("build-road")
                x=-thick / 2.0
                y=-long / 2.0
                width=thick
                height=long
                preserveAspectRatio="none"
                style=format!(
                    "filter: url(#{}) drop-shadow(0 1px 2px rgb(0 0 0 / 0.45)); \
                     pointer-events: none;",
                    tint_id(colour)
                )
            />
        </g>
    }
}

/// A plank walkway from a shore vertex out to a harbour.
#[component]
fn Pier(from: (f32, f32), to: (f32, f32)) -> impl IntoView {
    let (transform, length) = along(from, to);

    view! {
        <g transform=transform>
            <image
                href=asset_url("pier")
                x="-7"
                y=-length / 2.0
                width="14"
                height=length
                preserveAspectRatio="none"
                style="filter: drop-shadow(0 1px 2px rgb(0 0 0 / 0.4)); pointer-events: none;"
            />
        </g>
    }
}

/// A CSS-safe id for a colour's tint filter.
fn tint_id(hex: &str) -> String {
    format!("tint{}", hex.trim_start_matches('#'))
}

/// The artwork for a piece, tinted to its owner. Shared by the settlement and
/// the city so they cannot drift apart.
#[component]
fn Piece(
    x: f32,
    y: f32,
    player_id: Uuid,
    art: &'static str,
    size: f32,
    alt: &'static str,
) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let colour = player_color(player_id, &state);
    let half = size / 2.0;

    view! {
        <g transform=format!("translate({}, {})", x, y)>
            // One chain, not a `filter` attribute plus a CSS `filter`: the
            // CSS property wins outright and would drop the tint.
            <image
                href=asset_url(art)
                x=-half y=-half width=size height=size
                style=format!(
                    "filter: url(#{}) drop-shadow(0 1px 2px rgb(0 0 0 / 0.5)); \
                     pointer-events: none;",
                    tint_id(colour)
                )
            >
                <title>{alt}</title>
            </image>
        </g>
    }
}

#[component]
fn Settlement(x: f32, y: f32, player_id: Uuid) -> impl IntoView {
    view! { <Piece x=x y=y player_id=player_id art="build-settlement" size=32.0 alt="Settlement" /> }
}

#[component]
fn City(x: f32, y: f32, player_id: Uuid) -> impl IntoView {
    view! { <Piece x=x y=y player_id=player_id art="build-city" size=40.0 alt="City" /> }
}
