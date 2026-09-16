//! The right-hand dashboard: what happened, what people said, what the bank
//! has, and where everyone stands.
//!
//! Each of these is a cream panel on the water, stacked with a small gap.
//! They are separate components because they are separate concerns and each
//! wants its own scrolling.

use leptos::*;
use uuid::Uuid;

use crate::components::icons::{asset_url, resource_art, Art};
use crate::state::{ChatLine, GameState};
use shared::{ClientRequest, PlayerColour, PlayerInfo, ResourceType};

/// The five resources in board order, with the label the artwork uses.
const RESOURCES: [(ResourceType, &str); 5] = [
    (ResourceType::Wood, "Wood"),
    (ResourceType::Brick, "Brick"),
    (ResourceType::Sheep, "Sheep"),
    (ResourceType::Wheat, "Wheat"),
    (ResourceType::Ore, "Ore"),
];

fn colour_hex(colour: PlayerColour) -> &'static str {
    colour.hex()
}

fn log_resource_icon(res: ResourceType) -> View {
    let art = resource_art(res);
    view! {
        <Art
            name=art
            alt=res.label()
            class="inline-block h-[18px] w-[14px] object-cover rounded-[2px] game-card align-middle mx-0.5"
        />
    }
}

fn log_build_icon(label: &str) -> Option<&'static str> {
    match label.trim().to_ascii_lowercase().as_str() {
        "road" => Some("build-road"),
        "settlement" => Some("build-settlement"),
        "city" => Some("build-city"),
        "development" | "development-card" => Some("build-dev-card"),
        "knight" => Some("dev-knight"),
        "year" | "year-of-plenty" => Some("dev-plenty"),
        "monopoly" => Some("dev-monopoly"),
        "road-building" => Some("dev-road"),
        "victory" | "victory-point" => Some("dev-victory"),
        _ => None,
    }
}

fn log_event_icon_for_player_action(text: &str) -> Option<(&'static str, &'static str, bool)> {
    let phrase = text.trim();

    for (prefix, label) in [
        ("built a ", " built a "),
        ("bought a ", " bought a "),
        ("played a ", " played a "),
        ("used a ", " used a "),
    ] {
        if let Some(rest) = phrase.strip_prefix(prefix) {
            let lower = rest.to_ascii_lowercase();

            if lower.contains("road building") {
                return Some((label, "dev-road", false));
            }
            if lower.contains("year of plenty") {
                return Some((label, "dev-plenty", false));
            }
            if lower.contains("monopoly") {
                return Some((label, "dev-monopoly", false));
            }
            if lower.contains("knight") {
                return Some((label, "dev-knight", false));
            }

            let token = rest
                .split_whitespace()
                .next()
                .unwrap_or(rest)
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '-');

            if let Some(icon) = log_build_icon(token) {
                let tint = !icon.starts_with("dev-") && icon != "build-dev-card";
                return Some((label, icon, tint));
            }

            if rest.contains("development") {
                return Some((label, "build-dev-card", false));
            }
        }
    }

    if phrase.contains("moved the robber") {
        return Some((" moved the robber", "robber", false));
    }

    None
}

fn tinted_asset_icon(icon: &str, hex: &str) -> View {
    let url = asset_url(icon);
    view! {
        <span
            aria-hidden="true"
            class="inline-block align-middle"
            style=format!(
                "width: 18px; height: 18px; display: inline-block; background-color: {hex}; mask-image: url('{}'); -webkit-mask-image: url('{}'); mask-repeat: no-repeat; -webkit-mask-repeat: no-repeat; mask-size: contain; -webkit-mask-size: contain; mask-position: center; -webkit-mask-position: center;",
                url, url
            )
        />
    }
    .into_view()
}

fn parse_resource_token(token: &str) -> Option<ResourceType> {
    let resources = [
        (ResourceType::Wood, "wood"),
        (ResourceType::Wood, "lumber"),
        (ResourceType::Brick, "brick"),
        (ResourceType::Sheep, "sheep"),
        (ResourceType::Wheat, "wheat"),
        (ResourceType::Ore, "ore"),
    ];

    let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
    let lower = cleaned.to_ascii_lowercase();
    let normalized = lower.strip_suffix('s').unwrap_or(&lower);

    resources
        .iter()
        .find(|(_, name)| normalized == *name)
        .map(|(res, _)| *res)
}

fn render_trade_pair(count: usize, res: ResourceType) -> Vec<View> {
    let mut parts = Vec::new();
    if count > 0 {
        parts.push(view! { <span>{count}</span> }.into_view());
    }
    parts.push(log_resource_icon(res));
    parts
}

fn render_message_tokens(text: &str) -> Vec<View> {
    let mut parts: Vec<View> = Vec::new();
    let mut pending = String::new();

    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        let matched_resource = parse_resource_token(cleaned);
        let matched_build = log_build_icon(cleaned);

        if let Some(res) = matched_resource {
            if !pending.is_empty() {
                parts.push(view! { <span>{pending.clone()}</span> }.into_view());
                pending.clear();
            }
            parts.push(log_resource_icon(res));
        } else if let Some(asset) = matched_build {
            if !pending.is_empty() {
                parts.push(view! { <span>{pending.clone()}</span> }.into_view());
                pending.clear();
            }
            parts.push(view! { <Art name=asset alt="" class="inline-block h-[18px] w-[18px] object-contain align-middle mx-0.5" /> });
        } else {
            if pending.is_empty() {
                pending = token.to_string();
            } else {
                pending.push(' ');
                pending.push_str(token);
            }
        }
    }

    if !pending.is_empty() {
        parts.push(view! { <span>{pending.clone()}</span> }.into_view());
    }

    parts
}

fn render_log_message(msg: &str, players: &[PlayerInfo], robber_asset: &str) -> Vec<View> {
    if let Some(player) = players.iter().find(|player| msg.starts_with(&player.name)) {
        let player_name = player.name.clone();
        let colour = player.colour.hex().to_string();
        let tail = &msg[player_name.len()..];

        let mut parts: Vec<View> = vec![view! {
            <span style=format!("color: {}", colour)>{player_name}</span>
        }.into_view()];

        let remainder = tail.trim_start();
        let lower = remainder.to_ascii_lowercase();
        if lower.contains(" traded ") {
            let mut words = remainder.split_whitespace();
            let mut sweep: Vec<View> = Vec::new();

            while let Some(word) = words.next() {
                if word.eq_ignore_ascii_case("traded") {
                    sweep.push(view! { <span> traded </span> }.into_view());
                    continue;
                }
                if word.eq_ignore_ascii_case("for") {
                    sweep.push(view! { <span> for </span> }.into_view());
                    continue;
                }

                if let Ok(count) = word.parse::<usize>() {
                    let next = words.next();
                    if let Some(resource_word) = next.and_then(|w| parse_resource_token(w).map(|res| (w, res))) {
                        let (_, res) = resource_word;
                        sweep.extend(render_trade_pair(count, res));
                        continue;
                    }
                    sweep.push(view! { <span>{word.to_string()}</span> }.into_view());
                    continue;
                }

                if let Some(res) = parse_resource_token(word) {
                    sweep.extend(render_trade_pair(1, res));
                    continue;
                }

                sweep.push(view! { <span>{word.to_string()}</span> }.into_view());
            }

            parts.extend(sweep);
            return parts;
        }

        if let Some((prefix, icon, tint)) = log_event_icon_for_player_action(remainder) {
            parts.push(view! { <span>{prefix}</span> }.into_view());

            if icon == "robber" {
                let robber_name = robber_asset.trim();
                let asset = if robber_name.is_empty() { "robber" } else { robber_name };
                parts.push(view! {
                    <img
                        src=asset_url(asset)
                        alt=""
                        draggable="false"
                        class="inline-block h-[18px] w-[18px] object-contain align-middle mx-0.5 select-none pointer-events-none"
                    />
                }.into_view());
                return parts;
            }

            if tint {
                parts.push(tinted_asset_icon(icon, &colour));
            } else {
                parts.push(view! {
                    <img
                        src=asset_url(icon)
                        alt=""
                        draggable="false"
                        class="inline-block h-[18px] w-[18px] object-contain align-middle mx-0.5 select-none pointer-events-none"
                    />
                }.into_view());
            }
            return parts;
        }

        parts.extend(render_message_tokens(remainder));
        return parts;
    }

    render_message_tokens(msg)
}

/// A transcript of the game: who built what, who rolled what.
#[component]
pub fn EventLog() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    // Newest first, numbered so repeated lines still key apart. Built
    // outside the view macro: a turbofish inside it parses as tags.
    let events = move || {
        let all = state.messages.get();
        let n = all.len();
        all.into_iter()
            .rev()
            .enumerate()
            .map(|(i, msg)| (n - i, msg))
            .collect::<Vec<(usize, String)>>()
    };

    let no_events = move || state.messages.get().is_empty();

    view! {
        <div class="panel flex flex-col overflow-hidden" style="height: 215px;">
            <div class="px-3 py-1.5 border-b-2 border-[#d8ccb4] shrink-0">
                <span class="text-[11px] font-black uppercase tracking-[0.18em] text-[#6b6354]">
                    "Game Log"
                </span>
            </div>
            <div class="flex-1 overflow-y-auto custom-scrollbar px-3 py-1 min-h-0">
                <For
                    each=events
                    key=|(i, msg)| (*i, msg.clone())
                    children=move |(_, msg)| {
                        let rendered = render_log_message(&msg, &state.players.get(), &state.robber_asset.get());
                        view! {
                            <div class="text-[13px] leading-snug text-[#413a2c] border-b border-[#e2d7c0] py-1.5 last:border-0 flex flex-wrap items-center gap-x-1">
                                {rendered}
                            </div>
                        }
                    }
                />
                <Show when=no_events>
                    <div class="text-xs text-[#9a917f] italic py-3">"Nothing has happened yet."</div>
                </Show>
            </div>
        </div>
    }
}

/// Player talk. Its own panel, with an input pinned to the bottom.
#[component]
pub fn ChatPanel() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let (draft, set_draft) = create_signal(String::new());

    let send = move || {
        let text = draft.get_untracked().trim().to_string();
        if text.is_empty() {
            return;
        }
        state.send(ClientRequest::Chat { message: text });
        set_draft.set(String::new());
    };

    let no_chat = move || state.chat.get().is_empty();

    view! {
        <div class="panel flex flex-col overflow-hidden" style="height: 250px;">
            <div class="px-3 py-1.5 border-b-2 border-[#d8ccb4] shrink-0">
                <span class="text-[11px] font-black uppercase tracking-[0.18em] text-[#6b6354]">
                    "Chat"
                </span>
            </div>

            <div class="flex-1 overflow-y-auto custom-scrollbar px-3 py-2 space-y-2 min-h-0">
                <For
                    each=move || state.chat.get()
                    key=|line| line.seq
                    children=move |line: ChatLine| {
                        let who = state
                            .players
                            .get()
                            .iter()
                            .find(|p| p.player_id == line.player_id)
                            .map(|p| (p.name.clone(), colour_hex(p.colour)))
                            .unwrap_or_else(|| ("Someone".to_string(), "#94a3b8"));

                        view! {
                            <div class="flex items-start gap-2">
                                <span
                                    class="mt-1 w-4 h-4 rounded-full border-2 border-black/40 shrink-0"
                                    style=format!("background-color: {}", who.1)
                                ></span>
                                <div class="min-w-0">
                                    <span
                                        class="text-[13px] font-black mr-1.5"
                                        style=format!("color: {}", who.1)
                                    >
                                        {who.0}
                                    </span>
                                    <span class="text-[13px] text-[#413a2c] break-words">{line.text}</span>
                                </div>
                            </div>
                        }
                    }
                />
                <Show when=no_chat>
                    <div class="text-xs text-[#9a917f] italic">"Say something to the table."</div>
                </Show>
            </div>

            <div class="p-2 border-t-2 border-[#d8ccb4] shrink-0">
                <input
                    class="w-full h-9 px-3 rounded-md bg-white border-2 border-[#cdc2ac] text-[13px] text-[#2f2a1f]
                           placeholder:text-[#a79d89] outline-none focus:border-[#24bde5]"
                    placeholder="Say hello"
                    prop:value=draft
                    on:input=move |ev| set_draft.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            send();
                        }
                    }
                />
            </div>
        </div>
    }
}

/// What is left in the bank. Public information at a real table, so it is
/// public here.
#[component]
pub fn ResourceBank() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let count_of = move |res: ResourceType| {
        let r = state.bank.get().resources;
        match res {
            ResourceType::Wood => r.lumber,
            ResourceType::Brick => r.brick,
            ResourceType::Sheep => r.wool,
            ResourceType::Wheat => r.grain,
            ResourceType::Ore => r.ore,
            ResourceType::Desert => 0,
        }
    };

    view! {
        <div class="panel px-2 py-1.5 shrink-0">
            <div class="flex items-center justify-between gap-1">
                <Art name="bank" class="w-8 h-8 object-contain shrink-0" alt="Bank" />

                {RESOURCES.map(|(res, label)| view! {
                    <div class="flex flex-col items-center gap-0.5">
                        <Art
                            name=resource_art(res)
                            alt=label
                            class="w-7 h-10 object-cover rounded-[3px] game-card"
                        />
                        <span class="text-[13px] font-black text-[#413a2c] tabular-nums leading-none">
                            {move || count_of(res)}
                        </span>
                    </div>
                }).to_vec()}

                <div class="flex flex-col items-center gap-0.5 pl-1 border-l-2 border-[#ddd2ba]">
                    <Art
                        name="build-dev-card"
                        alt="Development cards"
                        class="w-7 h-10 object-cover rounded-[3px] game-card"
                    />
                    <span class="text-[13px] font-black text-[#413a2c] tabular-nums leading-none">
                        {move || state.bank.get().dev_cards}
                    </span>
                </div>
            </div>
        </div>
    }
}

/// Everyone at the table, one row each. The player on turn is picked out.
#[component]
pub fn PlayerPanels() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    view! {
        <div class="flex flex-col gap-1.5 overflow-y-auto custom-scrollbar min-h-0">
            <For
                each=move || state.players.get()
                key=|p| p.player_id
                children=move |player| view! { <PlayerPanel player_id=player.player_id /> }
            />
        </div>
    }
}

#[component]
fn PlayerPanel(player_id: Uuid) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    // Read through the roster rather than capturing a snapshot, so the row
    // tracks the player as their score and hand change.
    let info = create_memo(move |_| {
        state.players.get().into_iter().find(|p| p.player_id == player_id)
    });

    let is_me = move || state.player_id.get() == Some(player_id);
    let is_active = move || state.current_turn_player.get() == player_id;

    view! {
        <Show when=move || info.get().is_some()>
            {move || {
                let p = info.get().expect("checked");
                let colour = colour_hex(p.colour);
                let active = is_active();
                let road_active = p.has_longest_road;
                let army_active = p.has_largest_army;

                view! {
                    <div
                        // The resource animation flies cards to this row, and
                        // finds it by id rather than by position.
                        data-player=player_id.to_string()
                        class=move || format!(
                            "panel px-2 py-1.5 shrink-0 relative {}",
                            if active { "ring-[3px] ring-[#ffb718]" } else { "" }
                        )
                        style=move || if is_me() {
                            "background: linear-gradient(180deg, #fffaf0 0%, #f7edd9 100%);"
                        } else {
                            ""
                        }
                    >
                        <div class="flex items-center gap-2">
                            // Avatar, in the player's colour.
                            <div
                                class="w-11 h-11 rounded-full border-[3px] border-black/45 shrink-0 flex items-center justify-center
                                       shadow-[inset_0_2px_0_rgba(255,255,255,0.45)]"
                                style=format!("background-color: {colour}")
                            >
                                // Your own total includes the points sitting
                                // hidden in your hand. Everyone else's cannot:
                                // that is the whole point of hiding them.
                                <span class="text-base font-black text-white ink tabular-nums">
                                    {move || {
                                        let public = p.victory_points as i32;
                                        if is_me() {
                                            public + state.secret_victory_points.get()
                                        } else {
                                            public
                                        }
                                    }}
                                </span>
                            </div>

                            <div class="min-w-0 flex-1">
                                <div class="flex items-center gap-1.5">
                                    <span
                                        class="text-[14px] font-black truncate"
                                        style=format!("color: {colour}")
                                        title=p.name.clone()
                                    >
                                        {p.name.clone()}
                                    </span>
                                    <Show when=move || is_me()>
                                        <span class="text-[9px] font-bold uppercase tracking-wider text-[#8a8071]">
                                            "you"
                                        </span>
                                    </Show>
                                </div>

                                // Their public holdings. Counts only - a hand
                                // is never shown to anybody else. The mockup keeps the
                                // avatar left, the name above, and the four values in a
                                // compact icon-and-number strip beneath it.
                                <div class="mt-1 ml-auto mr-1 flex items-end justify-end gap-2">
                                    <div
                                        class="flex min-w-[4.5rem] flex-col items-center justify-center rounded-[8px] bg-transparent text-[#2e3e46]"
                                        title="Resource cards"
                                    >
                                        <Art name="any-card" class="h-[clamp(2.4rem,2.1vw,3.6rem)] w-[clamp(2.4rem,2.1vw,3.6rem)] object-contain" />
                                        <span class="mt-0.5 text-[12px] font-black tabular-nums">{p.resource_count}</span>
                                    </div>
                                    <div
                                        class="flex min-w-[4.5rem] flex-col items-center justify-center rounded-[8px] bg-transparent text-[#2e3e46]"
                                        title="Development cards"
                                    >
                                        <Art name="stat-devcards" class="h-[clamp(2.4rem,2.1vw,3.6rem)] w-[clamp(2.4rem,2.1vw,3.6rem)] object-contain" />
                                        <span class="mt-0.5 text-[12px] font-black tabular-nums">{p.dev_card_count}</span>
                                    </div>
                                    <div
                                        class="flex min-w-[4.5rem] flex-col items-center justify-center rounded-[8px] bg-transparent"
                                        title="Knights played"
                                    >
                                        <Show
                                            when=move || army_active
                                            fallback=|| view! {
                                                <Art name="stat-knights" class="h-[clamp(2.4rem,2.1vw,3.6rem)] w-[clamp(2.4rem,2.1vw,3.6rem)] object-contain" />
                                            }
                                        >
                                            <Art name="stat-knights_actived" class="h-[clamp(2.4rem,2.1vw,3.6rem)] w-[clamp(2.4rem,2.1vw,3.6rem)] object-contain" />
                                        </Show>
                                        <span class="mt-0.5 text-[12px] font-black tabular-nums text-[#2e3e46]">{p.knights_played}</span>
                                    </div>
                                    <div
                                        class="flex min-w-[4.5rem] flex-col items-center justify-center rounded-[8px] bg-transparent"
                                        title="Longest road"
                                    >
                                        <Show
                                            when=move || road_active
                                            fallback=|| view! {
                                                <Art name="stat-road" class="h-[clamp(2.4rem,2.1vw,3.6rem)] w-[clamp(2.4rem,2.1vw,3.6rem)] object-contain" />
                                            }
                                        >
                                            <Art name="stat-road_actived" class="h-[clamp(2.4rem,2.1vw,3.6rem)] w-[clamp(2.4rem,2.1vw,3.6rem)] object-contain" />
                                        </Show>
                                        <span class="mt-0.5 text-[12px] font-black tabular-nums text-[#2e3e46]">{p.roads_count}</span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            }}
        </Show>
    }
}
