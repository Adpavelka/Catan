use leptos::*;
use crate::state::{GameState, BuildMode};
use crate::components::icons::DieFace;
use shared::{ClientRequest, ResourceType, HexInfo, PlayerColour, PortType, PortInfo};
use uuid::Uuid;

// Convert axial coordinates (q, r) to pixel coordinates for SVG
fn axial_to_pixel(q: i32, r: i32, size: f32) -> (f32, f32) {
    let x = size * (3f32.sqrt() * q as f32 + 3f32.sqrt() / 2.0 * r as f32);
    let y = size * (3.0 / 2.0 * r as f32);
    (x, y)
}

// ===== Port rendering helpers (using shared::PortType from server) =====

fn port_label(port_type: &PortType) -> &'static str {
    match port_type {
        PortType::ThreeToOne => "3:1",
        PortType::TwoToOne(_) => "2:1",
    }
}

fn port_resource_label(port_type: &PortType) -> Option<&'static str> {
    match port_type {
        PortType::ThreeToOne => None,
        PortType::TwoToOne(res) => Some(match res {
            ResourceType::Brick => "BRICK",
            ResourceType::Wood => "WOOD",
            ResourceType::Sheep => "SHEEP",
            ResourceType::Wheat => "WHEAT",
            ResourceType::Ore => "ORE",
            ResourceType::Desert => "?",
        }),
    }
}

fn port_color(port_type: &PortType) -> &'static str {
    match port_type {
        PortType::ThreeToOne => "#ffffff",
        PortType::TwoToOne(res) => match res {
            ResourceType::Brick => "#fb923c",   // orange-400
            ResourceType::Wood => "#059669",    // emerald-600
            ResourceType::Sheep => "#84cc16",   // lime-500
            ResourceType::Wheat => "#eab308",   // yellow-500
            ResourceType::Ore => "#64748b",     // slate-500
            ResourceType::Desert => "#ffffff",
        },
    }
}

fn port_bg_color(port_type: &PortType) -> &'static str {
    match port_type {
        PortType::ThreeToOne => "#1e293b", // slate-800
        PortType::TwoToOne(res) => match res {
            ResourceType::Brick => "#431407",   // orange-950
            ResourceType::Wood => "#022c22",    // emerald-950
            ResourceType::Sheep => "#1a2e05",   // lime-950
            ResourceType::Wheat => "#422006",   // yellow-950
            ResourceType::Ore => "#020617",     // slate-950
            ResourceType::Desert => "#1e293b",
        },
    }
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

pub(crate) fn player_color_hex(colour: PlayerColour) -> &'static str {
    colour.hex()
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

    let state_roll = state.clone();
    let state_build_settlement = state.clone();
    let state_build_city = state.clone();
    let state_build_road = state.clone();

    // Check if it's my turn
    let is_my_turn = move || {
        if let Some(my_id) = state.player_id.get() {
            state.current_turn_player.get() == my_id
        } else {
            false
        }
    };

    // Check if dice has been rolled this turn
    let has_rolled = move || state.last_dice_roll.get().is_some();

    view! {
        <div class="relative w-full h-full flex flex-col min-h-0 min-w-0 gap-2">
            // The board takes every pixel the column can spare; the control
            // bar below is the only fixed-height part.
            <svg
                viewBox="0 0 1000 800"
                class="w-full flex-1 min-h-0 drop-shadow-2xl relative z-0"
                preserveAspectRatio="xMidYMid meet"
            >
                // A few slack rings for a suggestion of water, kept faint so
                // they never compete with the tiles. The sea itself is painted
                // on the container: the svg is letterboxed inside it, so an
                // svg-sized rect would leave dark bands down either side.
                <g class="pointer-events-none" opacity="0.16">
                    <circle cx="500" cy="380" r="250" fill="none" stroke="#7dd3fc" stroke-width="1.5"/>
                    <circle cx="500" cy="380" r="305" fill="none" stroke="#7dd3fc" stroke-width="1"/>
                    <circle cx="500" cy="380" r="358" fill="none" stroke="#7dd3fc" stroke-width="0.75"/>
                </g>
                // Center the board horizontally, move up vertically
                // The board is drawn at a fixed hex size and then scaled as a
                // whole, so the 30-hex extension board fits the same viewport
                // as the 19-hex one without touching every coordinate.
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
                    // Render all hexes
                    <For
                        each=move || state.hexes.get()
                        key=|hex| (hex.q, hex.r)
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
                            key=|port| (port.vertices[0], port.vertices[1])
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
                                let port_x = mid_x + out_nx * 56.0;
                                let port_y = mid_y + out_ny * 56.0;

                                // Get port display properties from server-sent port type
                                let label = port_label(&port.port_type);
                                let resource = port_resource_label(&port.port_type);
                                let color = port_color(&port.port_type);
                                let bg_color = port_bg_color(&port.port_type);

                                view! {
                                    <g>
                                        // Two piers running out to the coast,
                                        // so it is obvious which corners the
                                        // harbour actually serves.
                                        <line
                                            x1=v1x y1=v1y x2=port_x y2=port_y
                                            stroke=color stroke-width="2.5" stroke-linecap="round"
                                            opacity="0.65"
                                        />
                                        <line
                                            x1=v2x y1=v2y x2=port_x y2=port_y
                                            stroke=color stroke-width="2.5" stroke-linecap="round"
                                            opacity="0.65"
                                        />
                                        <circle cx=v1x cy=v1y r="3.5" fill=color opacity="0.9" />
                                        <circle cx=v2x cy=v2y r="3.5" fill=color opacity="0.9" />

                                        <g transform=format!("translate({}, {})", port_x, port_y)>
                                            // A round buoy rather than a
                                            // floating rectangle: it sits on
                                            // the water without looking like a
                                            // stray label.
                                            <circle r="21" fill="#0b1b2b" opacity="0.55" />
                                            <circle r="19" fill=bg_color stroke=color stroke-width="2.5" />
                                            <text
                                                y=if resource.is_some() { "-3" } else { "1" }
                                                text-anchor="middle"
                                                dominant-baseline="central"
                                                fill=color
                                                class="text-[13px] font-black"
                                            >
                                                {label}
                                            </text>
                                            {resource.map(|res| view! {
                                                <text
                                                    y="9"
                                                    text-anchor="middle"
                                                    dominant-baseline="central"
                                                    fill=color
                                                    class="text-[7px] font-bold tracking-wider"
                                                >
                                                    {res}
                                                </text>
                                            })}
                                        </g>
                                    </g>
                                }
                            }
                        />
                    </g>

                    // Render robber
                    {move || {
                        if let Some((robber_q, robber_r)) = state.robber_pos.get() {
                            let (px, py) = axial_to_pixel(robber_q, robber_r, 60.0);
                            view! {
                                <g transform=format!("translate({}, {})", px, py)>
                                    // Semi-transparent dark circle background
                                    <circle
                                        cx="0"
                                        cy="0"
                                        r="20"
                                        class="fill-black/40 stroke-red-600 stroke-2"
                                    />
                                    // Drawn, not lettered: an emoji here is a
                                    // tofu box on any machine without an emoji
                                    // font, and the robber has to be legible.
                                    <g class="pointer-events-none" transform="translate(-9, -9) scale(1.125)">
                                        <circle cx="8" cy="5" r="2.6" class="fill-red-500"/>
                                        <path d="M2.6 14c0-3 2.4-5 5.4-5s5.4 2 5.4 5z" class="fill-red-500"/>
                                        <path d="M4.2 5.6h7.6" stroke="#0b1120" stroke-width="1.6" stroke-linecap="round"/>
                                    </g>
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
                                let hexes = state.hexes.get();
                                calculate_all_vertices(&hexes)
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

                                view! {
                                    <circle
                                        cx=vx
                                        cy=vy
                                        r="10"
                                        class=move || {
                                            let mode = state_click.build_mode.get();
                                            let base = "cursor-pointer stroke-black stroke-1 transition-all ";
                                            if mode == BuildMode::City && has_my_settlement() {
                                                format!("{} fill-orange-500 animate-pulse", base)
                                            } else {
                                                format!("{} fill-white/30 hover:fill-white/80", base)
                                            }
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

                            // Calculate all edges from hexes (proper backend formula)
                            let all_edges = calculate_all_edges(&hexes);

                            // Filter out edges that already have roads
                            let available_edges: Vec<_> = all_edges.into_iter()
                                .filter(|(_, _, edge_coord)| {
                                    !roads.iter().any(|r| (r.x, r.y) == *edge_coord)
                                })
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

                    // Render roads (as lines between vertices)
                    <For
                        each=move || state.roads.get()
                        key=|r| (r.x, r.y, r.player_id)
                        children=move |road| {
                            let hexes = state.hexes.get();
                            // Find the two vertices this edge connects
                            if let Some((v1, v2)) = find_vertices_for_edge((road.x, road.y), &hexes) {
                                let (v1x, v1y) = vertex_to_pixel(v1.0, v1.1, 60.0);
                                let (v2x, v2y) = vertex_to_pixel(v2.0, v2.1, 60.0);

                                // Get stroke color based on player (look up from player list)
                                let stroke_color = state.players.get()
                                    .iter()
                                    .find(|p| p.player_id == road.player_id)
                                    .map(|p| p.colour.hex())
                                    .unwrap_or("#a855f7");

                                view! {
                                    <line
                                        x1=v1x
                                        y1=v1y
                                        x2=v2x
                                        y2=v2y
                                        stroke=stroke_color
                                        stroke-width="5"
                                        stroke-linecap="round"
                                    />
                                }.into_view()
                            } else {
                                view! { <g></g> }.into_view()
                            }
                        }
                    />
                </g>
            </svg>

            // Control bar: one row, pinned under the board, so the dice and
            // the build buttons sit with the thing they act on.
            <div class="shrink-0 w-full flex justify-center pb-1">
                <div class="flex flex-wrap items-center justify-center gap-3 bg-gray-900/80 backdrop-blur-md px-4 py-2.5 rounded-2xl border border-white/10 shadow-2xl">
                // Show initial placement instructions
                {move || {
                    let phase = move || state.game_phase.get();


                    if phase().is_initial_phase() {
                        if is_my_turn() {
                            view! {
                                <div class="text-orange-400 font-bold text-sm bg-orange-900/30 px-4 py-2 rounded-lg border border-orange-700/50">
                                    "Place 1 settlement, then 1 road"
                                </div>
                            }.into_view()
                        } else {
                            view! {
                                <div class="text-slate-400 text-sm px-4 py-2">
                                    "Waiting for other players..."
                                </div>
                            }.into_view()
                        }
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

                // Dice roll section (only in regular play)
                <Show when=move || state.game_phase.get() == shared::GamePhase::RegularPlay>
                    <div class="flex gap-3 items-center">
                        <Show when=move || is_my_turn() && !has_rolled()>
                            <button
                                class="bg-blue-600 hover:bg-blue-500 text-white px-6 py-3 rounded-xl font-bold transition-all"
                                on:click=move |_| state_roll.send(ClientRequest::RollDice)
                            >
                                "ROLL DICE"
                            </button>
                        </Show>

                        <Show when=move || has_rolled()>
                            <div class="flex gap-2 items-center text-white font-bold">
                                {move || state.last_dice_roll.get().map(|(d1, d2)| view! {
                                    <DieFace value=d1 />
                                    <DieFace value=d2 />
                                    <span class="ml-1 text-lg tabular-nums">{d1 + d2}</span>
                                })}
                            </div>
                        </Show>
                    </div>
                </Show>

                // Build buttons during initial placement (show if it's your turn)
                <Show when=move || {
                    let phase = move || state.game_phase.get();
                    is_my_turn() && phase().is_initial_phase()
                }>
                    <div class="flex gap-2">
                        <button
                            class=move || {
                                let base = "px-4 py-2 rounded-xl font-bold transition-all";
                                if state.build_mode.get() == BuildMode::Settlement {
                                    format!("{} bg-orange-600 text-white", base)
                                } else {
                                    format!("{} bg-white/10 hover:bg-white/20 text-white", base)
                                }
                            }
                            on:click=move |_| {
                                let current = state_build_settlement.build_mode.get();
                                if current == BuildMode::Settlement {
                                    state_build_settlement.build_mode.set(BuildMode::None);
                                } else {
                                    state_build_settlement.build_mode.set(BuildMode::Settlement);
                                }
                            }
                        >
                            "Settlement"
                        </button>
                        <button
                            class=move || {
                                let base = "px-4 py-2 rounded-xl font-bold transition-all";
                                if state.build_mode.get() == BuildMode::Road {
                                    format!("{} bg-green-600 text-white", base)
                                } else {
                                    format!("{} bg-white/10 hover:bg-white/20 text-white", base)
                                }
                            }
                            on:click=move |_| {
                                let current = state_build_road.build_mode.get();
                                if current == BuildMode::Road {
                                    state_build_road.build_mode.set(BuildMode::None);
                                } else {
                                    state_build_road.build_mode.set(BuildMode::Road);
                                }
                            }
                        >
                            "Road"
                        </button>
                    </div>
                </Show>

                // Build buttons during regular play (only show after rolling)
                <Show when=move || state.can_build_now()>
                    <div class="flex gap-2">
                        <button
                            class=move || {
                                let base = "px-4 py-2 rounded-xl font-bold transition-all";
                                if state.build_mode.get() == BuildMode::Settlement {
                                    format!("{} bg-orange-600 text-white", base)
                                } else {
                                    format!("{} bg-white/10 hover:bg-white/20 text-white", base)
                                }
                            }
                            on:click=move |_| {
                                let current = state_build_settlement.build_mode.get();
                                if current == BuildMode::Settlement {
                                    state_build_settlement.build_mode.set(BuildMode::None);
                                } else {
                                    state_build_settlement.build_mode.set(BuildMode::Settlement);
                                }
                            }
                        >
                            "Settlement"
                        </button>
                        <button
                            class=move || {
                                let base = "px-4 py-2 rounded-xl font-bold transition-all";
                                if state.build_mode.get() == BuildMode::City {
                                    format!("{} bg-purple-600 text-white", base)
                                } else {
                                    format!("{} bg-white/10 hover:bg-white/20 text-white", base)
                                }
                            }
                            on:click=move |_| {
                                let current = state_build_city.build_mode.get();
                                if current == BuildMode::City {
                                    state_build_city.build_mode.set(BuildMode::None);
                                } else {
                                    state_build_city.build_mode.set(BuildMode::City);
                                }
                            }
                        >
                            "City"
                        </button>
                        <button
                            class=move || {
                                let base = "px-4 py-2 rounded-xl font-bold transition-all";
                                if state.build_mode.get() == BuildMode::Road {
                                    format!("{} bg-green-600 text-white", base)
                                } else {
                                    format!("{} bg-white/10 hover:bg-white/20 text-white", base)
                                }
                            }
                            on:click=move |_| {
                                let current = state_build_road.build_mode.get();
                                if current == BuildMode::Road {
                                    state_build_road.build_mode.set(BuildMode::None);
                                } else {
                                    state_build_road.build_mode.set(BuildMode::Road);
                                }
                            }
                        >
                            "Road"
                        </button>
                    </div>
                </Show>
                </div>
            </div>
        </div>
    }
}

#[component]
fn HexTile(x: f32, y: f32, hex: HexInfo) -> impl IntoView {
    let points = "0,-50 43,-25 43,25 0,50 -43,25 -43,-25";
    let color = resource_color(&hex.resource);
    let label = resource_label(&hex.resource);

    // How many of the 36 dice combinations make this number: 6 and 8 are the
    // richest, 2 and 12 the poorest. Shown as pips, the way the real tokens do.
    let pips = if hex.number == 0 { 0 } else { 6u8.saturating_sub((7i8 - hex.number as i8).unsigned_abs()) };
    let hot = hex.number == 6 || hex.number == 8;

    view! {
        <g transform=format!("translate({}, {})", x, y) class="group">
            <polygon points=points class=format!("{} stroke-black/30 [stroke-width:2]", color) />
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
                    <circle r="20" fill="#f4ecd8" stroke="#0f172a" stroke-opacity="0.35" stroke-width="1.5" />
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

#[component]
fn Settlement(x: f32, y: f32, player_id: Uuid) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let color = player_color(player_id, &state);

    view! {
        <g transform=format!("translate({}, {})", x, y)>
            // House shape (scaled up for better visibility)
            <polygon
                points="0,-12 9,0 9,12 -9,12 -9,0"
                fill=color
                class="stroke-black stroke-2"
            />
        </g>
    }
}

#[component]
fn City(x: f32, y: f32, player_id: Uuid) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let color = player_color(player_id, &state);

    view! {
        <g transform=format!("translate({}, {})", x, y)>
            // Larger building with tower (scaled up for better visibility)
            <rect
                x="-12"
                y="-6"
                width="24"
                height="18"
                fill=color
                class="stroke-black stroke-2"
            />
            <rect
                x="-4"
                y="-18"
                width="8"
                height="12"
                fill=color
                class="stroke-black stroke-2"
            />
        </g>
    }
}

#[component]
fn Road(x: f32, y: f32, player_id: Uuid) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let color = player_color(player_id, &state);

    view! {
        <g transform=format!("translate({}, {})", x, y)>
            <line
                x1="-15"
                y1="0"
                x2="15"
                y2="0"
                stroke=color
                stroke-width="4"
                stroke-linecap="round"
            />
        </g>
    }
}
