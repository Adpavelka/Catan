//! The right-hand dashboard: what happened, what people said, what the bank
//! has, and where everyone stands.
//!
//! Each of these is a cream panel on the water, stacked with a small gap.
//! They are separate components because they are separate concerns and each
//! wants its own scrolling.

use leptos::*;
use uuid::Uuid;

use crate::components::icons::{resource_art, Art};
use crate::state::{ChatLine, GameState};
use shared::{ClientRequest, PlayerColour, ResourceType};

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
                    children=move |(_, msg)| view! {
                        <div class="text-[13px] leading-snug text-[#413a2c] border-b border-[#e2d7c0] py-1.5 last:border-0">
                            {msg}
                        </div>
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
                                // is never shown to anybody else.
                                <div class="flex items-center gap-2.5 mt-0.5 text-[12px] font-bold text-[#4a4335]">
                                    <span class="flex items-center gap-1" title="Resource cards">
                                        <Art name="brick" class="w-3 h-4 object-cover rounded-[2px]" />
                                        {p.resource_count}
                                    </span>
                                    <span class="flex items-center gap-1" title="Development cards">
                                        <Art name="stat-devcards" class="w-3 h-4 object-contain" />
                                        {p.dev_card_count}
                                    </span>
                                    <span class="flex items-center gap-1" title="Knights played">
                                        <Art name="stat-knights" class="w-4 h-4 object-contain" />
                                        {p.knights_played}
                                    </span>
                                    <span class="flex items-center gap-1" title="Longest road">
                                        <Art name="stat-road" class="w-4 h-4 object-contain" />
                                        {p.roads_count}
                                    </span>
                                </div>
                            </div>

                            <div class="flex flex-col gap-1 shrink-0">
                                <Show when=move || p.has_longest_road>
                                    <span
                                        class="px-1.5 py-0.5 rounded text-[9px] font-black uppercase bg-[#b98624] text-white border border-black/30"
                                        title="Longest road"
                                    >"Road"</span>
                                </Show>
                                <Show when=move || p.has_largest_army>
                                    <span
                                        class="px-1.5 py-0.5 rounded text-[9px] font-black uppercase bg-[#c0392b] text-white border border-black/30"
                                        title="Largest army"
                                    >"Army"</span>
                                </Show>
                            </div>
                        </div>
                    </div>
                }
            }}
        </Show>
    }
}
