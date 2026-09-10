//! The statistics pages.
//!
//! Each one does the same small job: turn the tally the server sent into
//! columns for `StatsGrid` or bars for `StatsBarChart`. None of them draws a
//! matrix or a chart itself, so the pages stay short and every page looks
//! like the others whether or not anyone remembers to make it.

use leptos::*;
use shared::{DevCardTally, GameStats};

use crate::components::icons::{dev_card_art, resource_art};
use crate::components::stats::chart::{ChartBar, StatsBarChart};
use crate::components::stats::grid::{StatColumn, StatsGrid};

/// The bar colour for anything that is not a player: the cyan the rest of the
/// interface uses for its own furniture.
const TABLE_CYAN: &str = "#56c6e1";
/// A seven pays nobody and moves the robber, so it is not the same kind of
/// result as the other ten and is not drawn as though it were.
const SEVEN_RED: &str = "#c96a4e";

// ---------------------------------------------------------------- overview

/// Where the points came from. These sum to the score the game itself used to
/// decide the winner, which is why they are points rather than counts of
/// pieces - the pieces are in the block underneath.
const POINT_COLUMNS: [StatColumn; 6] = [
    StatColumn {
        art: "stats-points",
        title: "Victory points",
        tile: true,
        read: |p| p.victory_points as i64,
    },
    StatColumn {
        art: "points-from-settlements",
        title: "Points from settlements",
        tile: true,
        read: |p| p.points.settlements as i64,
    },
    StatColumn {
        art: "points-from-cities",
        title: "Points from cities",
        tile: true,
        read: |p| p.points.cities as i64,
    },
    StatColumn {
        art: "points-from-VP-cards",
        title: "Points from development cards",
        tile: false,
        read: |p| p.points.dev_cards as i64,
    },
    StatColumn {
        art: "points-from-longest-road",
        title: "Points from the longest road",
        tile: false,
        read: |p| p.points.longest_road as i64,
    },
    StatColumn {
        art: "points-from-biggest-army",
        title: "Points from the largest army",
        tile: false,
        read: |p| p.points.largest_army as i64,
    },
];

#[component]
pub fn OverviewStats(stats: GameStats) -> impl IntoView {
    let winner = stats.winner;

    view! {
        <div class="flex flex-col gap-5">
            <StatsGrid
                caption="Victory points"
                players=stats.players.clone()
                columns=POINT_COLUMNS.to_vec()
                highlight=winner
                lead_first=true
            />
        </div>
    }
}

// -------------------------------------------------------------------- dice

#[component]
pub fn DiceStats(stats: GameStats) -> impl IntoView {
    // Eleven bars, 2 through 12. Drawn even where a total never came up: the
    // gap is the interesting part of a dice distribution.
    let totals: Vec<ChartBar> = stats
        .dice_rolls
        .iter()
        .enumerate()
        .map(|(index, count)| {
            let total = shared::dice_total(index);
            let colour = if total == 7 { SEVEN_RED } else { TABLE_CYAN };
            ChartBar::new(format!("{total}"), *count as i64, colour)
        })
        .collect();

    view! {
        <div class="flex flex-col gap-5">
            <StatsBarChart
                title="Every roll at the table"
                bars=totals
                empty_note="No dice were rolled."
                height=240
            />
        </div>
    }
}

// ------------------------------------------------------------ resource cards

/// One chart, and deliberately only one: how many of each resource the island
/// turned out over the whole game. Who got them, and what became of them, is
/// what the resource page is for.
#[component]
pub fn ResourceCardStats(stats: GameStats) -> impl IntoView {
    // Every resource, including any that never came up. A gap in the row is
    // the interesting part - a board where nobody ever drew ore played very
    // differently from one where they did.
    let bars: Vec<ChartBar> = shared::ResourceType::CARDS
        .iter()
        .map(|kind| {
            ChartBar::new(kind.label(), stats.draws_of(*kind) as i64, TABLE_CYAN)
                .with_art(resource_art(*kind))
        })
        .collect();

    view! {
        <div class="flex-1 flex items-center justify-center">
            <div class="w-full" style="max-width: 720px">
                <StatsBarChart
                    title="Resource cards drawn"
                    bars=bars
                    empty_note="No resource cards were drawn."
                    height=210
                />
            </div>
        </div>
    }
}

// --------------------------------------------------------- development cards

/// One chart: how many of each development card came out of the deck all
/// game. The card face sits under its own bar, so the bar names itself.
#[component]
pub fn DevelopmentCardStats(stats: GameStats) -> impl IntoView {
    // Every kind, including any the deck never turned up. Dropping the empty
    // ones would make two games' charts impossible to compare.
    let bars: Vec<ChartBar> = DevCardTally::KINDS
        .iter()
        .map(|kind| {
            let total: u32 = stats
                .players
                .iter()
                .map(|player| player.dev_cards_bought.get(kind))
                .sum();
            ChartBar::new(crate::state::card_label(kind), total as i64, TABLE_CYAN)
                .with_art(dev_card_art(kind))
        })
        .collect();

    view! {
        <div class="flex-1 flex items-center justify-center">
            <div class="w-full" style="max-width: 720px">
                <StatsBarChart
                    title="Development cards drawn"
                    bars=bars
                    empty_note="No development cards were bought."
                    height=210
                />
            </div>
        </div>
    }
}

// ------------------------------------------------------------------ activity

/// What each player did, as opposed to what they ended up with. Every one of
/// these is counted as the game runs - see the backend's `statistics` module -
/// so none of it is derived here.
const ACTIVITY_COLUMNS: [StatColumn; 6] = [
    StatColumn {
        art: "stats-activity-amount-of-devcards-bought",
        title: "Development cards bought",
        tile: false,
        read: |p| p.activity.dev_cards_bought as i64,
    },
    StatColumn {
        art: "stats-activity-amount-of-devcards-used",
        title: "Development cards played",
        tile: false,
        read: |p| p.activity.dev_cards_played as i64,
    },
    StatColumn {
        art: "stats-activity-trades-proposed",
        title: "Trades proposed",
        tile: false,
        read: |p| p.activity.trades_proposed as i64,
    },
    StatColumn {
        art: "stats-activity-trades-done",
        title: "Trades settled",
        tile: false,
        read: |p| p.activity.trades_completed as i64,
    },
    StatColumn {
        art: "stats-activity-amount-of-resources-used",
        title: "Resources spent on buildings and development cards",
        tile: false,
        read: |p| p.activity.resources_spent as i64,
    },
    StatColumn {
        art: "stats-activity-amount-of-resources-blocked-by-robber",
        title: "Resources a roll would have paid, but for the robber",
        tile: false,
        read: |p| p.activity.resources_blocked_by_robber as i64,
    },
];

#[component]
pub fn ActivityStats(stats: GameStats) -> impl IntoView {
    let winner = stats.winner;

    view! {
        <div class="flex flex-col gap-3">
            <StatsGrid
                caption="Activity statistics"
                players=stats.players.clone()
                columns=ACTIVITY_COLUMNS.to_vec()
                highlight=winner
            />
        </div>
    }
}

// --------------------------------------------------------------- resources

/// Where every card came from and where it went.
///
/// The first three are the totals - in, out, and what that leaves - and the
/// rest break those totals down by cause. Spending on buildings is part of
/// the total lost but has no column of its own: it belongs on the activity
/// page, beside the things that were built with it.
const RESOURCE_COLUMNS: [StatColumn; 11] = [
    StatColumn {
        art: "stats-total-resources-income",
        title: "Cards gained, all told",
        tile: false,
        read: |p| p.resources.gained_total as i64,
    },
    StatColumn {
        art: "stats-total-resources-lost",
        title: "Cards lost, all told",
        tile: false,
        read: |p| p.resources.lost_total as i64,
    },
    StatColumn {
        art: "stats-total-resources-score",
        title: "Cards left over: everything gained less everything lost",
        tile: false,
        read: |p| p.resources.score(),
    },
    StatColumn {
        art: "stats-resources-gained-by-rolling",
        title: "Gained from the dice",
        tile: false,
        read: |p| p.resources.gained_by_rolling as i64,
    },
    StatColumn {
        art: "stats-resources-gained-by-trading",
        title: "Gained by trading",
        tile: false,
        read: |p| p.resources.gained_by_trading as i64,
    },
    StatColumn {
        art: "stats-resources-gained-by-devcards",
        title: "Gained from development cards",
        tile: false,
        read: |p| p.resources.gained_by_dev_cards as i64,
    },
    StatColumn {
        art: "stats-resources-gained-by-robbing",
        title: "Gained by robbing",
        tile: false,
        read: |p| p.resources.gained_by_robbing as i64,
    },
    StatColumn {
        art: "stats-resources-lost-by-trading",
        title: "Lost by trading",
        tile: false,
        read: |p| p.resources.lost_by_trading as i64,
    },
    StatColumn {
        art: "stats-resources-lost-by-devcards",
        title: "Lost to development cards",
        tile: false,
        read: |p| p.resources.lost_by_dev_cards as i64,
    },
    StatColumn {
        art: "stats-resources-lost-by-robber",
        title: "Lost to the robber",
        tile: false,
        read: |p| p.resources.lost_by_robber as i64,
    },
    StatColumn {
        art: "stats-resources-lost-by-seven",
        title: "Handed back on a seven",
        tile: false,
        read: |p| p.resources.lost_by_seven as i64,
    },
];

#[component]
pub fn ResourceStats(stats: GameStats) -> impl IntoView {
    let winner = stats.winner;

    view! {
        <div class="flex flex-col gap-3">
            <StatsGrid
                caption="Resource statistics"
                players=stats.players.clone()
                columns=RESOURCE_COLUMNS.to_vec()
                highlight=winner
                lead_first=true
            />
        </div>
    }
}
