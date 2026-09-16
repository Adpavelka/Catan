//! The narrow rail down the left edge: leave, rules, sound, settings, help.
//!
//! Fixed width and always present, so the rest of the interface can change
//! underneath it without the window's furniture moving.

use leptos::*;

use crate::state::GameState;
use shared::ClientRequest;

#[component]
pub fn LeftToolbar() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let (muted, set_muted) = create_signal(false);
    let (showing, set_showing) = create_signal(Option::<&'static str>::None);

    let leave = move |_| {
        if let Some(game_id) = state.game_id.get_untracked() {
            state.send(ClientRequest::LeaveGame { game_id });
        }
    };

    view! {
        <aside
            class="shrink-0 flex flex-col items-center gap-5 py-4 border-r-2 border-[#04547f]"
            style="width: 62px; background: linear-gradient(180deg, #066191 0%, #05537f 100%);"
        >
            // Leaving is the one destructive control here, so it is set apart
            // at the top rather than sitting in the same run as the rest.
            <ToolButton label="Leave the game" on_click=Callback::new(leave)>
                <path d="M9 5 3 12l6 7"/>
                <path d="M3 12h13"/>
                <path d="M15 3h3a3 3 0 0 1 3 3v12a3 3 0 0 1-3 3h-3"/>
            </ToolButton>

            <div class="w-7 h-px bg-white/25"></div>

            <ToolButton
                label="Rules"
                on_click=Callback::new(move |_| set_showing.update(|s| {
                    *s = if *s == Some("rules") { None } else { Some("rules") }
                }))
            >
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
            </ToolButton>

            <ToolButton
                label="Sound"
                on_click=Callback::new(move |_| set_muted.update(|m| *m = !*m))
            >
                {move || if muted.get() {
                    view! {
                        <>
                            <path d="M11 4.7 6.6 9H3v6h3.6L11 19.3z"/>
                            <path d="m22 9-6 6"/><path d="m16 9 6 6"/>
                        </>
                    }.into_view()
                } else {
                    view! {
                        <>
                            <path d="M11 4.7 6.6 9H3v6h3.6L11 19.3z"/>
                            <path d="M16 8.5a5 5 0 0 1 0 7"/>
                            <path d="M19.4 5.6a9 9 0 0 1 0 12.8"/>
                        </>
                    }.into_view()
                }}
            </ToolButton>

            <ToolButton
                label="Settings"
                on_click=Callback::new(move |_| set_showing.update(|s| {
                    *s = if *s == Some("settings") { None } else { Some("settings") }
                }))
            >
                <circle cx="12" cy="12" r="3"/>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </ToolButton>

            <ToolButton
                label="Help"
                on_click=Callback::new(move |_| set_showing.update(|s| {
                    *s = if *s == Some("help") { None } else { Some("help") }
                }))
            >
                <path d="M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3"/>
            </ToolButton>

            // The rail's panels are notes rather than settings screens: there
            // is nothing behind them to configure yet.
            <Show when=move || showing.get().is_some()>
                <div class="absolute left-[70px] bottom-4 z-40 w-64 panel p-3 text-[12px] text-[#413a2c]">
                    {move || match showing.get() {
                        Some("rules") => "Build settlements and cities, take the longest road or the largest army, and race to the victory target shown on your lobby. Roll a seven and the robber moves.",
                        Some("settings") => "Nothing to configure yet. Sound is the switch above.",
                        Some("help") => "Hover any button to see what it costs or why it is unavailable. Right-click a card in the trade panel to take it back.",
                        _ => "",
                    }}
                </div>
            </Show>
        </aside>
    }
}

/// One rail button: a big, soft-edged glyph that lights up on hover.
#[component]
fn ToolButton(
    label: &'static str,
    on_click: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            class="w-10 h-10 flex items-center justify-center rounded-lg text-[#bfe9fb]
                   hover:text-white hover:bg-white/15 active:scale-95 transition-all"
            title=label
            aria-label=label
            on:click=move |_| on_click.call(())
        >
            <svg
                class="w-[26px] h-[26px]"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.2"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                {children()}
            </svg>
        </button>
    }
}
