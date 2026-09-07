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
    Trophy,
    Robber,
    Warning,
    Refresh,
    Wheat,
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

    // A unique id per die, so two on screen do not share one gradient.
    let uid = format!("die{}", js_sys::Math::random().to_bits());
    let body = format!("{uid}-body");
    let pip = format!("{uid}-pip");

    view! {
        <svg
            class=format!("inline-block align-[-0.2em] shrink-0 {size} {class}")
            viewBox="0 0 16 16"
            aria-label=format!("die showing {value}")
        >
            <defs>
                <linearGradient id=body.clone() x1="0" y1="0" x2="0.3" y2="1">
                    <stop offset="0%" stop-color="#f2f6f7"/>
                    <stop offset="55%" stop-color="#d9e1e3"/>
                    <stop offset="100%" stop-color="#bcc7ca"/>
                </linearGradient>
                <radialGradient id=pip.clone() cx="0.35" cy="0.3" r="0.85">
                    <stop offset="0%" stop-color="#5a646b"/>
                    <stop offset="100%" stop-color="#20272c"/>
                </radialGradient>
            </defs>

            <rect x="1.1" y="1.1" width="13.8" height="13.8" rx="2.6"
                  fill=format!("url(#{body})") stroke="#8d9aa1" stroke-width="0.3"/>
            // A highlight along the top edge: the die catches the light.
            <rect x="2.2" y="2.2" width="11.6" height="10.6" rx="1.8"
                  fill="none" stroke="#ffffff" stroke-opacity="0.55" stroke-width="0.4"/>

            {pips.iter()
                .map(|(cx, cy)| view! {
                    <circle cx=*cx cy=*cy r="1.5" fill=format!("url(#{pip})")/>
                })
                .collect_view()}
        </svg>
    }
}

/// Artwork from `frontend/assets`, copied into the bundle by Trunk and served
/// at `/assets/<name>`.
///
/// These are full-colour illustrations, so unlike `Icon` they cannot inherit
/// `currentColor`. Where the UI needs a disabled look it applies a `grayscale`
/// filter, which does work on an `<img>`.
#[component]
pub fn Art(
    /// File name inside `assets`, without the extension.
    name: &'static str,
    #[prop(default = "")] class: &'static str,
    #[prop(default = "")] alt: &'static str,
) -> impl IntoView {
    view! {
        <img
            src=format!("/assets/{name}.svg")
            alt=alt
            draggable="false"
            class=format!("select-none pointer-events-none {class}")
        />
    }
}

/// The card face for a resource.
pub fn resource_art(res: shared::ResourceType) -> &'static str {
    match res {
        shared::ResourceType::Brick => "brick",
        shared::ResourceType::Wood => "wood",
        shared::ResourceType::Sheep => "sheep",
        shared::ResourceType::Wheat => "wheat",
        shared::ResourceType::Ore => "ore",
        shared::ResourceType::Desert => "brick",
    }
}

/// The card face for a development card.
pub fn dev_card_art(card: &shared::DevCardType) -> &'static str {
    match card {
        shared::DevCardType::Knight => "dev-knight",
        shared::DevCardType::VictoryPoint => "dev-victory",
        shared::DevCardType::RoadBuilding => "dev-road",
        shared::DevCardType::Monopoly => "dev-monopoly",
        shared::DevCardType::YearOfPlenty => "dev-plenty",
    }
}

/// The harbour badge for a port.
pub fn port_art(port: &shared::PortType) -> &'static str {
    match port {
        shared::PortType::ThreeToOne => "port-generic",
        shared::PortType::TwoToOne(res) => match res {
            shared::ResourceType::Brick => "port-brick",
            shared::ResourceType::Wood => "port-wood",
            shared::ResourceType::Sheep => "port-sheep",
            shared::ResourceType::Wheat => "port-wheat",
            shared::ResourceType::Ore => "port-ore",
            shared::ResourceType::Desert => "port-generic",
        },
    }
}
