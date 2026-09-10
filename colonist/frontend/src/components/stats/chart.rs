//! The one bar chart the statistics pages share.
//!
//! It takes bars and draws them. It knows nothing about dice, resources or
//! development cards - each page works out what its bars mean and hands them
//! over already labelled and coloured, which is what lets a new page reuse
//! this instead of growing a chart of its own.

use leptos::*;

/// One bar: how tall, what colour, and what to write under it.
#[derive(Clone)]
pub struct ChartBar {
    pub label: String,
    pub value: i64,
    /// Any CSS colour. Player colours where the bars are players, so a bar
    /// belongs to the same person it does everywhere else in the game.
    pub colour: String,
    /// Artwork under the bar in place of the label - a card face, say. The
    /// label still travels, as the hover text.
    pub art: Option<&'static str>,
}

impl ChartBar {
    pub fn new(label: impl Into<String>, value: i64, colour: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value,
            colour: colour.into(),
            art: None,
        }
    }

    pub fn with_art(mut self, art: &'static str) -> Self {
        self.art = Some(art);
        self
    }
}

/// A column chart. Empty data draws the note instead, rather than an axis
/// with nothing under it.
#[component]
pub fn StatsBarChart(
    title: &'static str,
    bars: Vec<ChartBar>,
    /// Shown in place of the chart when every bar would be zero.
    #[prop(default = "Nothing happened here.")] empty_note: &'static str,
    /// Height of the plot area in pixels, excluding the labels.
    #[prop(default = 132)] height: u32,
) -> impl IntoView {
    // Scale to the tallest bar rather than to a round number: these are small
    // counts, and a chart that always ran to 100 would leave every bar as a
    // stub.
    let peak = bars.iter().map(|bar| bar.value).max().unwrap_or(0);
    let has_data = peak > 0;

    /// How much of the plot the tallest bar fills. The rest is headroom for
    /// the figure that sits on top of it.
    const TALLEST: f64 = 86.0;

    view! {
        <section class="flex flex-col min-w-0 flex-1">
            <div class="stats-caption">{title}</div>

            <Show
                when=move || has_data
                fallback=move || view! {
                    <div
                        class="flex items-center justify-center text-[14px] italic text-[#8fb6c9]/70
                               border border-dashed border-[#56c6e1]/25 rounded-lg px-3"
                        style=format!("height: {}px", height + 26)
                    >
                        {empty_note}
                    </div>
                }
            >
                <div class="stats-plot flex items-end gap-[3px] px-1" style=format!("height: {height}px")>
                    {bars
                        .iter()
                        .map(|bar| {
                            // `peak` is at least 1 here, so this cannot divide
                            // by zero; a negative figure would be a bug
                            // upstream, and is floored rather than drawn
                            // upside down.
                            let share = (bar.value.max(0) as f64 / peak as f64) * TALLEST;
                            let colour = bar.colour.clone();
                            let hint = format!("{}: {}", bar.label, bar.value);
                            // Nothing is nothing. A bar's minimum height keeps
                            // a small count visible, and would otherwise draw
                            // a sliver where a player did not do the thing at
                            // all.
                            let drawn = bar.value > 0;

                            view! {
                                <div class="stats-bar-track flex-1 min-w-0" title=hint>
                                    // The figure rides on the bar rather than
                                    // sitting in a row of its own, so which
                                    // number belongs to which bar is never a
                                    // question.
                                    <span class="text-[17px] font-black text-[#eaf6fb] tabular-nums leading-none pb-1">
                                        {bar.value}
                                    </span>
                                    <Show when=move || drawn>
                                        <div
                                            class="stats-bar"
                                            style=format!("--bar-height: {share:.1}%; background-color: {colour}")
                                        ></div>
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                // What each bar is, under the axis: the artwork where there is
                // any, and the name under it either way.
                <div class="flex items-start gap-[3px] px-1 pt-1.5">
                    {bars
                        .iter()
                        .map(|bar| {
                            view! {
                                <div class="flex-1 min-w-0 flex flex-col items-center gap-1">
                                    {bar.art.map(|art| view! {
                                        <img
                                            src=format!("/assets/{art}.svg")
                                            alt=bar.label.clone()
                                            draggable="false"
                                            class="w-[40px] h-[56px] object-cover rounded-[4px] game-card
                                                   select-none pointer-events-none"
                                        />
                                    })}
                                    <span class="text-[14px] font-bold text-[#a9cddd] truncate max-w-full leading-tight">
                                        {bar.label.clone()}
                                    </span>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>
            </Show>
        </section>
    }
}
