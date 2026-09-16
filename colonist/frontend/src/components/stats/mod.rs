//! The screen the game ends on.
//!
//! A layer over the table rather than a page of its own: the board stays
//! where it was, darkened, because the first thing anybody wants after a game
//! is to look at the position it finished in. The sheet on top carries the
//! result, a row of tabs, and whichever page of figures is showing.
//!
//! Everything on it comes from the tally the server sends with `PlayerWon`.
//! Nothing is worked out here, and nothing is invented: a page with no data
//! behind it says so.

pub(crate) mod chart;
pub(crate) mod grid;
pub(crate) mod pages;

use leptos::*;
use shared::{ClientRequest, GameStats};

use crate::components::icons::Art;
use crate::components::stats::grid::StatsDisc;
use crate::components::stats::pages::{
    ActivityStats, DevelopmentCardStats, DiceStats, OverviewStats, ResourceCardStats, ResourceStats,
};
use crate::state::GameState;

/// The pages, in the order they are listed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StatsTab {
    Overview,
    Dice,
    ResourceCards,
    DevCards,
    Activity,
    Resources,
}

impl StatsTab {
    const ALL: [StatsTab; 6] = [
        StatsTab::Overview,
        StatsTab::Dice,
        StatsTab::ResourceCards,
        StatsTab::DevCards,
        StatsTab::Activity,
        StatsTab::Resources,
    ];

    fn label(&self) -> &'static str {
        match self {
            StatsTab::Overview => "Overview",
            StatsTab::Dice => "Dice Stats",
            StatsTab::ResourceCards => "Res Card Stats",
            StatsTab::DevCards => "Dev Card Stats",
            StatsTab::Activity => "Activity Stats",
            StatsTab::Resources => "Resource Stats",
        }
    }
}

/// The whole end-of-game layer. Draws nothing until the server says somebody
/// has won.
#[component]
pub fn GameEndStats() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let tab = create_rw_signal(StatsTab::Overview);

    // The tally arrives with the victory message. Defaulted rather than
    // guarded so a server that somehow sent none still gets a readable
    // screen: every page copes with an empty table.
    let stats = move || state.game_stats.get().unwrap_or_default();

    let leave = move |_| {
        if let Some(game_id) = state.game_id.get_untracked() {
            state.send(ClientRequest::LeaveGame { game_id });
        }
        // Do not wait for the round trip to clear the screen. Leaving is also
        // what the server's `Left` does, but a dropped connection must not
        // leave the player staring at a statistics sheet they cannot dismiss.
        state.winner_player_id.set(None);
        state.game_stats.set(None);
        state.is_in_game.set(false);
    };

    view! {
        <Show when=move || state.winner_player_id.get().is_some()>
            <div class="stats-veil fixed inset-0 z-[10000] flex items-center justify-center p-3 pointer-events-auto">
                // A fixed height, not one that follows the content: the sheet
                // must not resize under the cursor as somebody moves between
                // tabs with more and less on them.
                <div
                    class="stats-sheet w-full flex flex-col min-h-0 px-5 py-4"
                    style="max-width: min(1180px, 96vw); height: min(94vh, 900px);"
                >
                    <VictoryHeader stats=Signal::derive(stats) />

                    <StatsTabs tab=tab />

                    // The pages scroll inside the sheet, never the window:
                    // the header and the tabs have to stay put while you move
                    // between them.
                    <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar py-4 pr-1 flex flex-col">
                        {move || {
                            let stats = stats();
                            match tab.get() {
                                StatsTab::Overview => view! { <OverviewStats stats=stats /> }.into_view(),
                                StatsTab::Dice => view! { <DiceStats stats=stats /> }.into_view(),
                                StatsTab::ResourceCards => view! { <ResourceCardStats stats=stats /> }.into_view(),
                                StatsTab::DevCards => view! { <DevelopmentCardStats stats=stats /> }.into_view(),
                                StatsTab::Activity => view! { <ActivityStats stats=stats /> }.into_view(),
                                StatsTab::Resources => view! { <ResourceStats stats=stats /> }.into_view(),
                            }
                        }}
                    </div>

                    <div class="shrink-0 flex justify-center pt-3 border-t border-white/10">
                        <button
                            class="stats-home w-[min(320px,60%)] h-11 text-[14px]"
                            on:click=leave
                        >
                            "Home"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}

/// The result, and how long it took to get there.
#[component]
fn VictoryHeader(stats: Signal<GameStats>) -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");

    let winner = move || {
        let stats = stats.get();
        stats.winner.and_then(|id| stats.player(id).cloned())
    };

    // "Victory" is only true for one person at the table. Everybody else is
    // looking at the same screen, and telling them they won would be a lie.
    let i_won = move || state.winner_player_id.get().is_some()
        && state.winner_player_id.get() == state.player_id.get();

    view! {
        <header class="shrink-0 flex flex-col items-center gap-1 pb-3">
            // The game's own trophy, the same mark the victory-point column
            // is headed with. Nothing is drawn here that the game does not
            // already own.
            <Art name="stats-points" alt="" class="w-[52px] h-[52px] object-contain" />

            <h2 class="text-[clamp(28px,3.6vw,46px)] font-black text-white leading-none ink tracking-tight">
                {move || if i_won() { "Victory!!!" } else { "Game over" }}
            </h2>

            // Who won, in their own colour and behind their own disc. On a
            // screen everybody at the table sees, the title alone does not
            // say it.
            {move || winner().map(|player| {
                let colour = player.colour.hex();
                view! {
                    <p class="flex items-center gap-2 text-[15px] font-black" style=format!("color: {colour}")>
                        <StatsDisc colour=colour name=player.name.clone() size=24 />
                        {player.name.clone()}
                        <span class="text-[#c5dfec]/70 font-bold">
                            {format!("- {} points", player.victory_points)}
                        </span>
                    </p>
                }
            })}

            <p class="text-[13px] font-bold text-[#a9cddd]/80 tabular-nums">
                {move || {
                    let stats = stats.get();
                    format!("Time: {} - Turns: {}", stats.duration_label(), stats.turns)
                }}
            </p>
        </header>
    }
}

/// The row of pages, with a cyan rule under the one you are on.
#[component]
fn StatsTabs(tab: RwSignal<StatsTab>) -> impl IntoView {
    view! {
        <nav class="shrink-0 flex items-stretch justify-between gap-1 border-b border-white/15 overflow-x-auto">
            {StatsTab::ALL
                .map(|option| {
                    view! {
                        <button
                            class=move || format!(
                                "stats-tab flex-1 text-[clamp(11px,1.05vw,14px)] {}",
                                if tab.get() == option { "stats-tab-on" } else { "" }
                            )
                            on:click=move |_| tab.set(option)
                        >
                            {option.label()}
                        </button>
                    }
                })
                .to_vec()}
        </nav>
    }
}
