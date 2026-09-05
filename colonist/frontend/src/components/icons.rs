//! Inline SVG icons.
//!
//! These used to be emoji. Emoji are not a dependency you can rely on: they
//! need a colour emoji font installed, and on a machine without one - which
//! includes plenty of Linux desktops and every headless browser we test in -
//! every one of them renders as a tofu box. Drawn inline, they cost nothing,
//! scale with the text and inherit `currentColor`.

use leptos::*;

/// Shared attributes: sized in `em` so an icon matches whatever text it sits
/// next to, and painted in `currentColor` so it follows the surrounding class.
const BOX: &str = "0 0 16 16";

#[component]
pub fn Icon(
    /// One of the shapes below, by name.
    kind: IconKind,
    /// Extra classes, e.g. a colour or a margin.
    #[prop(default = "")]
    class: &'static str,
) -> impl IntoView {
    let body = match kind {
        IconKind::Resource => view! {
            <>
                <rect x="2.5" y="1.5" width="8" height="12" rx="1.5"
                      fill="none" stroke="currentColor" stroke-width="1.3"/>
                <path d="M11.5 3.2A1.5 1.5 0 0 1 13.5 4.6v8A1.5 1.5 0 0 1 12 14H5"
                      fill="none" stroke="currentColor" stroke-width="1.3"
                      stroke-linecap="round"/>
            </>
        }.into_view(),

        IconKind::DevCard => view! {
            <>
                <rect x="2" y="2" width="12" height="12" rx="1.5"
                      fill="none" stroke="currentColor" stroke-width="1.3"/>
                <path d="M8 4.8v6.4M4.8 8h6.4" stroke="currentColor"
                      stroke-width="1.3" stroke-linecap="round"/>
            </>
        }.into_view(),

        IconKind::Knight => view! {
            <>
                <path d="M11.8 2.2 6.4 7.6M4 12l2.6-2.6M2.6 13.4 5.2 10.8"
                      stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                <path d="M10.4 2.2h3.4v3.4L8.4 11 5 7.6z" fill="none"
                      stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                <path d="M2 14l2-2" stroke="currentColor" stroke-width="1.6"
                      stroke-linecap="round"/>
            </>
        }.into_view(),

        IconKind::Road => view! {
            <>
                <path d="M1.5 12.5 6 3.5M14.5 12.5 10 3.5" stroke="currentColor"
                      stroke-width="1.3" stroke-linecap="round"/>
                <path d="M8 4v1.6M8 7.2v1.6M8 10.4V12" stroke="currentColor"
                      stroke-width="1.3" stroke-linecap="round"/>
            </>
        }.into_view(),

        IconKind::Trophy => view! {
            <>
                <path d="M4.5 2h7v3.5a3.5 3.5 0 0 1-7 0z" fill="none"
                      stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                <path d="M4.5 3H2.6v1.2A2.4 2.4 0 0 0 5 6.6M11.5 3h1.9v1.2a2.4 2.4 0 0 1-2.4 2.4"
                      fill="none" stroke="currentColor" stroke-width="1.2"/>
                <path d="M8 9.2V11M5.6 14h4.8M6.6 11h2.8l.6 3H6z"
                      fill="none" stroke="currentColor" stroke-width="1.3"
                      stroke-linejoin="round" stroke-linecap="round"/>
            </>
        }.into_view(),

        IconKind::Robber => view! {
            <>
                <circle cx="8" cy="5" r="2.6" fill="currentColor"/>
                <path d="M2.6 14c0-3 2.4-5 5.4-5s5.4 2 5.4 5z" fill="currentColor"/>
                <path d="M4.2 5.6h7.6" stroke="#0b1120" stroke-width="1.6"
                      stroke-linecap="round"/>
            </>
        }.into_view(),

        IconKind::Warning => view! {
            <>
                <path d="M8 1.8 15 14H1z" fill="none" stroke="currentColor"
                      stroke-width="1.3" stroke-linejoin="round"/>
                <path d="M8 6v3.4" stroke="currentColor" stroke-width="1.4"
                      stroke-linecap="round"/>
                <circle cx="8" cy="11.6" r="0.85" fill="currentColor"/>
            </>
        }.into_view(),

        IconKind::Refresh => view! {
            <>
                <path d="M13.5 8a5.5 5.5 0 1 1-1.7-4" fill="none"
                      stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                <path d="M13.6 1.4v3.2h-3.2" fill="none" stroke="currentColor"
                      stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
            </>
        }.into_view(),

        IconKind::Wheat => view! {
            <>
                <path d="M8 14V5.5" stroke="currentColor" stroke-width="1.3"
                      stroke-linecap="round"/>
                <path d="M8 5.2c0-1.6.8-2.8 1.9-3.4.5 1.5.2 3-1.9 3.4z" fill="currentColor"/>
                <path d="M8 5.2c0-1.6-.8-2.8-1.9-3.4-.5 1.5-.2 3 1.9 3.4z" fill="currentColor"/>
                <path d="M8 9c0-1.5.8-2.6 1.9-3.2.5 1.4.2 2.8-1.9 3.2z" fill="currentColor"/>
                <path d="M8 9c0-1.5-.8-2.6-1.9-3.2-.5 1.4-.2 2.8 1.9 3.2z" fill="currentColor"/>
            </>
        }.into_view(),

    };

    view! {
        <svg
            class=format!("inline-block align-[-0.125em] w-[1em] h-[1em] shrink-0 {class}")
            viewBox=BOX
            fill="none"
            aria-hidden="true"
            focusable="false"
        >
            {body}
        </svg>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Resource,
    DevCard,
    Knight,
    Road,
    Trophy,
    Robber,
    Warning,
    Refresh,
    Wheat,
}

/// A die face, 1-6, drawn as pips. Used instead of the Unicode die characters,
/// which are missing from most default font stacks.
#[component]
pub fn DieFace(value: u8, #[prop(default = "")] class: &'static str) -> impl IntoView {
    // Pip positions on a 3x3 grid, per face.
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
            class=format!("inline-block align-[-0.2em] w-[1.35em] h-[1.35em] shrink-0 {class}")
            viewBox=BOX
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
