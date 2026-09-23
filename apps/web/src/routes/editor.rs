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
use crate::auth::{AuthState, use_auth};
use crate::chart::ChartFonts;
use crate::chart_preview::ChartPreview;
use crate::chart_url;
use crate::collab::{ChartCollabSession, CollabStatus, use_chart_collab, use_org_connection};
use crate::keyflow_editor::KeyflowEditor;
use crate::library;
use crate::routes::library::BoundChart;
use crate::routes::{SaveToLibrary, Shell};
use editor::EditorState;

/// `/editor` — the editor seeded with the default example.
#[component]
pub fn Editor() -> Element {
    rsx! {
        EditorScreen { initial: examples::DEFAULT_CHART.to_string(), from_link: false, bound: None }
    }
}

/// `/c/:data` — the editor seeded from a chart encoded in the URL.
#[component]
pub fn Chart(data: String) -> Element {
    match chart_url::decode(&data) {
        Ok(source) => rsx! { EditorScreen { initial: source, from_link: true, bound: None } },
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

/// Which pane a narrow screen is showing.
///
/// Only meaningful below the breakpoint where the two panes stop fitting
/// side by side. Above it both are on screen and this rides along unused —
/// the value stays valid, so rotating a phone back to landscape restores
/// the split without losing which tab was picked.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Source,
    Chart,
}

impl Pane {
    /// The value of the `data-pane` attribute the stylesheet switches on.
    fn key(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Chart => "chart",
        }
    }
}

/// The editor over one document. `bound` names the stored chart it is
/// an edit of, when it is one — then Save updates that chart in place
/// rather than keeping a new one. See [`crate::routes::library`].
#[component]
pub fn EditorScreen(initial: String, from_link: bool, bound: Option<BoundChart>) -> Element {
    // Component-local, deliberately. `keyflow_ui::signals::CHART_SOURCE` is
    // a *global* editor buffer, which is right for a single-window desktop
    // app and wrong here: seeding it from the route meant every navigation
    // wrote global state the outgoing screen was still subscribed to, and
    // `/c/:data` wedged the renderer. The chart belongs to the screen
    // showing it.
    let mut source = use_signal(|| initial.clone());
    // Chart first on a phone too. Reading a chart is the common errand —
    // certainly so arriving from a shared link — and writing one is a
    // deliberate act with its tab right there.
    let mut pane = use_signal(|| Pane::Chart);

    // ── A library chart is a shared document ────────────────────────
    // Everyone with it open edits one Loro document on Task (see
    // `crate::collab`). The buffer is owned here so the session can write
    // other people's edits into it; every hook below runs whether or not
    // the chart is bound, as hooks must, and does nothing when it is not.
    let editor_state = use_signal(|| EditorState::new(initial.clone()));
    let auth = use_auth();
    let who = match (auth.state)() {
        AuthState::SignedIn(account) => account.label(),
        _ => "Someone".to_owned(),
    };
    let collab = use_chart_collab(who);
    use_org_connection(bound.as_ref().map(|chart| chart.org.clone()));
    let doc_id = {
        let bound = bound.clone();
        use_resource(move || {
            let bound = bound.clone();
            async move {
                match bound {
                    Some(chart) => library::open_chart_collab(&chart.org, &chart.slug)
                        .await
                        .map(Some),
                    None => Ok(None),
                }
            }
        })
    };
    // Local edits and caret moves reach the shared document through the
    // editor's transaction sink (`ChartCollab::on_transaction`), wired in
    // `KeyflowEditor` — synchronously, not from an effect.
    let shared = bound.is_some();

    // Who else is here is read by `CollabStatus` alone: reading presence
    // in this component re-rendered the whole editor on every caret move.
    let note = match &bound {
        Some(chart) => Some(format!("Editing “{}” in your library", chart.title)),
        None => from_link.then(|| "Opened from a link".to_string()),
    };
    let session = match (&bound, &*doc_id.read_unchecked()) {
        (Some(_), Some(Ok(Some(id)))) => {
            let id = *id;
            rsx! {
                ChartCollabSession {
                    key: "{id}",
                    doc_id: id,
                    collab,
                    state: editor_state,
                    opened: initial.clone(),
                }
            }
        }
        (Some(_), Some(Err(error))) => {
            tracing::warn!("collab unavailable for this chart: {error}");
            rsx! {}
        }
        _ => rsx! {},
    };

    rsx! {
        Shell {
            ChartFonts {}
            div { class: "kf-editor", "data-pane": pane().key(),
                // A phone cannot show two panes at once and be useful at
                // either — side by side gives each about 160px, stacked
                // gives each half a screen and puts the chart below the
                // fold. So below the breakpoint they become tabs and the
                // chosen one takes the whole viewport. Above it this row
                // is `display: none` and both panes are on screen, so the
                // desktop layout is untouched.
                div { class: "kf-pane-tabs", role: "tablist",
                    button {
                        class: if pane() == Pane::Chart { "kf-tab kf-tab-on" } else { "kf-tab" },
                        role: "tab",
                        "aria-selected": if pane() == Pane::Chart { "true" } else { "false" },
                        onclick: move |_| pane.set(Pane::Chart),
                        "Chart"
                    }
                    button {
                        class: if pane() == Pane::Source { "kf-tab kf-tab-on" } else { "kf-tab" },
                        role: "tab",
                        "aria-selected": if pane() == Pane::Source { "true" } else { "false" },
                        onclick: move |_| pane.set(Pane::Source),
                        "Source"
                    }
                }
                // No toolbar row of its own. Both panes carry their own
                // header instead, which is where their controls belong
                // and keeps the two pane tops on one line.
                //
                // Chart first, in the markup and so on the left. It is the
                // thing being made, and the thing anyone opening a shared
                // link came for; the source is how you change it. DOM order
                // rather than `order:` on the grid, so the tab order and a
                // screen reader agree with the eye.
                div { class: "kf-editor-split",
                    ChartPreview { source: source() }
                    {session}
                    KeyflowEditor {
                        initial: source(),
                        on_change: move |text| source.set(text),
                        state: Some(editor_state),
                        collab: shared.then_some(collab),
                        note,
                        // Keeping a chart is the one thing the URL
                        // cannot do. Signed out this is an invitation
                        // and nothing more — the editor is never gated
                        // behind an account.
                        // A chart of a song also leads back to the song:
                        // its other arrangements, its lists, its details.
                        actions: rsx! {
                            if shared {
                                CollabStatus { collab }
                            }
                            if let Some(chart) = bound.clone() {
                                if let Some(song) = chart.song.clone() {
                                    Link {
                                        class: "kf-account-link",
                                        to: Route::LibrarySong { org: chart.org.clone(), slug: song },
                                        title: "This song's charts, lists and details",
                                        "Song"
                                    }
                                }
                            }
                            SaveToLibrary { source: source(), bound: bound.clone() }
                        },
                    }
                }
            }
        }
    }
}
