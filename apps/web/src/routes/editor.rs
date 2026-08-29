//! The editor, and the shareable chart URL it produces.
//!
//! There is no backend. The document *is* the URL: every edit re-encodes
//! the chart into the address bar (see [`crate::chart_url`]), so the browser
//! back button is undo-of-navigation, a bookmark is a saved chart, and a
//! shared link is a shared chart. That is the whole persistence story until
//! accounts exist, and it is deliberately enough to be useful without one.

use dioxus::prelude::*;
use keyflow_ui::examples;

use crate::Route;
use crate::chart::ChartFonts;
use crate::chart_preview::ChartPreview;
use crate::chart_url;
use crate::keyflow_editor::KeyflowEditor;
use crate::routes::Shell;

/// `/editor` — the editor seeded with the default example.
#[component]
pub fn Editor() -> Element {
    rsx! {
        EditorScreen { initial: examples::DEFAULT_CHART.to_string(), from_link: false }
    }
}

/// `/c/:data` — the editor seeded from a chart encoded in the URL.
#[component]
pub fn Chart(data: String) -> Element {
    match chart_url::decode(&data) {
        Ok(source) => rsx! { EditorScreen { initial: source, from_link: true } },
        // A truncated or mangled link is the common case here — a chat client
        // that broke the URL across a line, say. Say so plainly and offer a
        // way forward rather than showing an empty editor.
        Err(e) => rsx! {
            Shell {
                section { class: "kf-prose",
                    h1 { "That chart link is broken" }
                    p { "{e}" }
                    p {
                        "If it arrived in a message, it may have been split across "
                        "lines — try copying the whole link."
                    }
                    Link { to: Route::Editor {}, "Start a new chart" }
                }
            }
        },
    }
}

#[component]
fn EditorScreen(initial: String, from_link: bool) -> Element {
    // Component-local, deliberately. `keyflow_ui::signals::CHART_SOURCE` is
    // a *global* editor buffer, which is right for a single-window desktop
    // app and wrong here: seeding it from the route meant every navigation
    // wrote global state the outgoing screen was still subscribed to, and
    // `/c/:data` wedged the renderer. The chart belongs to the screen
    // showing it.
    let mut source = use_signal(|| initial);
    let encoded = use_memo(move || chart_url::encode(&source.read()));

    rsx! {
        Shell {
            ChartFonts {}
            div { class: "kf-editor",
                div { class: "kf-editor-bar",
                    ShareLink { encoded: encoded() }
                    if from_link {
                        span { class: "kf-note", "Opened from a link" }
                    }
                }
                div { class: "kf-editor-split",
                    KeyflowEditor {
                        initial: source(),
                        on_change: move |text| source.set(text),
                    }
                    ChartPreview { source: source() }
                }
            }
        }
    }
}

/// The share control.
///
/// Shows the link when the chart fits in one, and says so plainly when it
/// does not — rather than handing over a URL that will be truncated
/// somewhere between here and the recipient.
#[component]
fn ShareLink(encoded: String) -> Element {
    if chart_url::fits_in_url(&encoded) {
        rsx! {
            Link { to: Route::Chart { data: encoded.clone() }, class: "kf-button",
                "Link to this chart"
            }
        }
    } else {
        rsx! {
            span { class: "kf-note",
                "This chart is too long to share as a link "
                "({encoded.len()} characters). Saving charts needs an account — coming."
            }
        }
    }
}
