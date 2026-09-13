//! The statistics matrix: a run of players down the side, a run of icons
//! across the top, and a figure where they meet.
//!
//! Every cell of every row lives in one CSS grid, which is what keeps the
//! icon, the heading and each player's figure in the same column no matter
//! how long the names are. Nothing here knows what any of the numbers mean:
//! a page hands it a list of columns, each of which is an icon and a way to
//! read one figure out of a player's statistics.

use leptos::*;
use shared::PlayerStats;
use uuid::Uuid;

use crate::components::icons::Art;

/// One column of a matrix.
///
/// `read` is a plain function pointer rather than a boxed closure: every
/// column so far is a field read, and keeping it `Copy` means a column list
/// can be built as an ordinary array.
#[derive(Clone, Copy)]
pub struct StatColumn {
    /// File name in `assets`, without the extension.
    pub art: &'static str,
    /// What the icon means, shown on hover. The icons carry no captions, so
    /// this is the only place the meaning is spelled out.
    pub title: &'static str,
    /// Whether to draw the teal plate behind the artwork.
    ///
    /// The resource marks are painted with one already - that is the house
    /// style for a statistics icon - but the pieces and the victory-point
    /// marks are bare shapes. Set this for those, so a heading row mixing the
    /// two still looks like one row, and so a dark piece is not left as a
    /// smudge against the dark sheet.
    pub tile: bool,
    pub read: fn(&PlayerStats) -> i64,
}

/// A player's disc, in their colour.
///
/// Drawn from the end-of-game snapshot rather than from the live roster the
/// rest of the UI uses: the roster drops anybody who leaves, and the final
/// table has to keep listing them.
#[component]
pub fn StatsDisc(colour: &'static str, name: String, #[prop(default = 38)] size: u32) -> impl IntoView {
    view! {
        <span
            class="stats-disc"
            style=format!("width: {size}px; height: {size}px; background-color: {colour}")
            title=name
        >
            <img
                src=crate::components::icons::asset_url("trade-offerer")
                alt=""
                draggable="false"
                class="select-none pointer-events-none"
                style=format!("width: {}px; height: {}px; object-fit: contain", size * 7 / 10, size * 7 / 10)
            />
        </span>
    }
}

/// A block of figures: a caption, a row of icons, and one row per player.
#[component]
pub fn StatsGrid(
    /// The small heading above the block.
    caption: &'static str,
    /// In seat order. The same order on every page, so a player's row does
    /// not move about as the tabs change.
    players: Vec<PlayerStats>,
    columns: Vec<StatColumn>,
    /// Picked out in gold. The winner, where there is one.
    #[prop(default = None)] highlight: Option<Uuid>,
    /// Draw the first column's figures large. Used where that column is the
    /// headline - the score - rather than one detail among many.
    #[prop(default = false)] lead_first: bool,
) -> impl IntoView {
    // A player's name needs real room; a figure needs only enough for three
    // digits. `1fr` on the figures spreads whatever is left over evenly, so
    // the columns stay even whether there are five of them or eleven.
    //
    // The name column is a share of the width rather than a share of what is
    // left over, so two matrices stacked on one page start their figures in
    // the same place even when they do not have the same number of columns.
    let template = format!(
        "minmax(9rem, 20%) repeat({}, minmax(3rem, 1fr))",
        columns.len()
    );

    let last_row = players.len().saturating_sub(1);

    view! {
        <section class="w-full">
            <div class="stats-caption">{caption}</div>

            <div class="stats-matrix" style=format!("grid-template-columns: {template}")>
                // The heading strip. The first cell is deliberately empty:
                // the player column needs no icon, and a word there would
                // pull the eye away from the figures.
                <div class="stats-cell stats-head stats-name-cell"></div>
                {columns
                    .iter()
                    .map(|column| {
                        view! {
                            <div class="stats-cell stats-head" title=column.title>
                                <span class=if column.tile { "stats-plate" } else { "" }>
                                    <Art name=column.art alt=column.title class="stats-icon" />
                                </span>
                            </div>
                        }
                    })
                    .collect_view()}

                {players
                    .iter()
                    .enumerate()
                    .map(|(row, player)| {
                        let edge = if row == last_row { "stats-cell-last" } else { "" };
                        let win = if highlight == Some(player.player_id) { "stats-row-win" } else { "" };
                        let cell = format!("stats-cell {edge} {win}");
                        let colour = player.colour.hex();

                        view! {
                            // The gold edge belongs to the row, so only the
                            // cell that starts it draws one. Putting it on
                            // every cell ruled the row into columns.
                            <div class=format!("{cell} stats-name-cell {}", if win.is_empty() { "" } else { "stats-row-win-lead" })>
                                <StatsDisc colour=colour name=player.name.clone() />
                                <span
                                    class="text-[17px] font-black truncate"
                                    style=format!("color: {colour}")
                                    title=player.name.clone()
                                >
                                    {player.name.clone()}
                                </span>
                            </div>

                            {columns
                                .iter()
                                .enumerate()
                                .map(|(index, column)| {
                                    let value = (column.read)(player);
                                    // A nought is noise on a dense page; it
                                    // stays legible but stops competing with
                                    // the figures that say something.
                                    let weight = match (lead_first && index == 0, value == 0) {
                                        (true, _) => "stats-figure stats-figure-lead",
                                        (false, true) => "stats-figure stats-figure-zero",
                                        (false, false) => "stats-figure",
                                    };
                                    view! {
                                        <div class=cell.clone()>
                                            <span class=weight>{value}</span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        }
                    })
                    .collect_view()}
            </div>
        </section>
    }
}
