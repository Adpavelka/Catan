//! Inline SVG icons.
//!
//! The UI used emoji until these existed, and every one of them rendered as a
//! tofu box on a machine with no colour emoji font - which includes plenty of
//! Linux desktops and every headless browser we test in.
//!
//! The shapes are Lucide's (<https://lucide.dev>, ISC licence): a real icon
//! set, drawn on a consistent 24x24 grid with a 2px stroke, rather than
//! something freehand. They are inlined instead of pulled from a package so
//! the wasm bundle stays dependency-free, and they inherit `currentColor` and
//! `em` sizing so they match whatever text they sit beside.

use leptos::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    /// Cards in hand.
    Resource,
    /// Development cards.
    DevCard,
    Knight,
    Road,
    Settlement,
    City,
    Trophy,
    Robber,
    Warning,
    Refresh,
    Wheat,
    /// Monopoly.
    Coins,
}

#[component]
pub fn Icon(
    kind: IconKind,
    /// Extra classes, e.g. a colour or a margin.
    #[prop(default = "")]
    class: &'static str,
    /// Override the default 1em square.
    #[prop(default = "w-[1em] h-[1em]")]
    size: &'static str,
) -> impl IntoView {
    // Every shape is stroke-only on the same grid, so one <svg> wrapper with
    // shared stroke attributes covers all of them.
    let body = match kind {
        // layers
        IconKind::Resource => view! {
            <>
                <path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/>
                <path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/>
                <path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>
            </>
        }.into_view(),

        // scroll-text
        IconKind::DevCard => view! {
            <>
                <path d="M15 12h-5"/>
                <path d="M15 8h-5"/>
                <path d="M19 17V5a2 2 0 0 0-2-2H4"/>
                <path d="M8 21h12a2 2 0 0 0 2-2v-1a1 1 0 0 0-1-1H11a1 1 0 0 0-1 1v1a2 2 0 1 1-4 0V5a2 2 0 1 0-4 0v2a1 1 0 0 0 1 1h3"/>
            </>
        }.into_view(),

        // swords
        IconKind::Knight => view! {
            <>
                <polyline points="14.5 17.5 3 6 3 3 6 3 17.5 14.5"/>
                <line x1="13" x2="19" y1="19" y2="13"/>
                <line x1="16" x2="20" y1="16" y2="20"/>
                <line x1="19" x2="21" y1="21" y2="19"/>
                <polyline points="14.5 6.5 18 3 21 3 21 6 17.5 9.5"/>
                <line x1="5" x2="9" y1="14" y2="18"/>
                <line x1="7" x2="4" y1="17" y2="20"/>
                <line x1="3" x2="5" y1="19" y2="21"/>
            </>
        }.into_view(),

        // route
        IconKind::Road => view! {
            <>
                <circle cx="6" cy="19" r="3"/>
                <path d="M9 19h8.5a3.5 3.5 0 0 0 0-7h-11a3.5 3.5 0 0 1 0-7H15"/>
                <circle cx="18" cy="5" r="3"/>
            </>
        }.into_view(),

        // house
        IconKind::Settlement => view! {
            <>
                <path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/>
                <path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
            </>
        }.into_view(),

        // building-2
        IconKind::City => view! {
            <>
                <path d="M6 22V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v18Z"/>
                <path d="M6 12H4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2"/>
                <path d="M18 9h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-2"/>
                <path d="M10 6h4"/><path d="M10 10h4"/><path d="M10 14h4"/><path d="M10 18h4"/>
            </>
        }.into_view(),

        // trophy
        IconKind::Trophy => view! {
            <>
                <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6"/>
                <path d="M18 9h1.5a2.5 2.5 0 0 0 0-5H18"/>
                <path d="M4 22h16"/>
                <path d="M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22"/>
                <path d="M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22"/>
                <path d="M18 2H6v7a6 6 0 0 0 12 0V2Z"/>
            </>
        }.into_view(),

        // venetian-mask
        IconKind::Robber => view! {
            <>
                <path d="M18 11c-1.5 0-2.5.5-3 2"/>
                <path d="M4 6a2 2 0 0 0-2 2v4a5 5 0 0 0 5 5 8 8 0 0 1 5 2 8 8 0 0 1 5-2 5 5 0 0 0 5-5V8a2 2 0 0 0-2-2h-3a8 8 0 0 0-5 2 8 8 0 0 0-5-2z"/>
                <path d="M6 11c1.5 0 2.5.5 3 2"/>
            </>
        }.into_view(),

        // triangle-alert
        IconKind::Warning => view! {
            <>
                <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/>
                <path d="M12 9v4"/>
                <path d="M12 17h.01"/>
            </>
        }.into_view(),

        // refresh-cw
        IconKind::Refresh => view! {
            <>
                <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/>
                <path d="M21 3v5h-5"/>
                <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/>
                <path d="M8 16H3v5"/>
            </>
        }.into_view(),

        // coins
        IconKind::Coins => view! {
            <>
                <circle cx="8" cy="8" r="6"/>
                <path d="M18.09 10.37A6 6 0 1 1 10.34 18"/>
                <path d="M7 6h1v4"/>
                <path d="m16.71 13.88.7.71-2.82 2.82"/>
            </>
        }.into_view(),

        // wheat
        IconKind::Wheat => view! {
            <>
                <path d="M2 22 16 8"/>
                <path d="M3.47 12.53 5 11l1.53 1.53a3.5 3.5 0 0 1 0 4.94L5 19l-1.53-1.53a3.5 3.5 0 0 1 0-4.94Z"/>
                <path d="M7.47 8.53 9 7l1.53 1.53a3.5 3.5 0 0 1 0 4.94L9 15l-1.53-1.53a3.5 3.5 0 0 1 0-4.94Z"/>
                <path d="M11.47 4.53 13 3l1.53 1.53a3.5 3.5 0 0 1 0 4.94L13 11l-1.53-1.53a3.5 3.5 0 0 1 0-4.94Z"/>
                <path d="M20 2h2v2a4 4 0 0 1-4 4h-2V6a4 4 0 0 1 4-4Z"/>
            </>
        }.into_view(),
    };

    view! {
        <svg
            class=format!("inline-block align-[-0.15em] shrink-0 {size} {class}")
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
        >
            {body}
        </svg>
    }
}

/// A die face, 1-6, drawn as pips. The Unicode die characters are missing from
/// most default font stacks, so they get the same treatment as the emoji did.
#[component]
pub fn DieFace(
    value: u8,
    #[prop(default = "")] class: &'static str,
    #[prop(default = "w-[1.35em] h-[1.35em]")] size: &'static str,
) -> impl IntoView {
    let pips: &[(f32, f32)] = match value {
        1 => &[(8.0, 8.0)],
        2 => &[(5.0, 5.0), (11.0, 11.0)],
        3 => &[(5.0, 5.0), (8.0, 8.0), (11.0, 11.0)],
        4 => &[(5.0, 5.0), (11.0, 5.0), (5.0, 11.0), (11.0, 11.0)],
        5 => &[(5.0, 5.0), (11.0, 5.0), (8.0, 8.0), (5.0, 11.0), (11.0, 11.0)],
        _ => &[(5.0, 4.5), (11.0, 4.5), (5.0, 8.0), (11.0, 8.0), (5.0, 11.5), (11.0, 11.5)],
    };

    view! {
        <svg
            class=format!("inline-block align-[-0.2em] shrink-0 {size} {class}")
            viewBox="0 0 16 16"
            aria-label=format!("die showing {value}")
        >
            <rect x="1" y="1" width="14" height="14" rx="3"
                  fill="#f8fafc" stroke="#0f172a" stroke-width="1"/>
            {pips.iter()
                .map(|(cx, cy)| view! { <circle cx=*cx cy=*cy r="1.4" fill="#0f172a"/> })
                .collect_view()}
        </svg>
    }
}
