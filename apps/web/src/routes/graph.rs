//! The guide's knowledge graph.
//!
//! The guide is a vault, and a vault's shape is its link web. This is that
//! web, drawn: a force-directed map of how the concepts reference each
//! other, with communities detected from the link structure rather than
//! declared by hand.
//!
//! It is a genuinely useful way in. A linear table of contents says
//! "chords is chapter three"; the graph says chords is the hub that
//! rhythm, notation systems and melody all lean on, which is a different
//! and more honest description of what to read next.
//!
//! `view-knowledge-graph` does the work — the same crate that draws Task's
//! wiki. It is pure and FS-free, so it runs here in wasm unchanged.

use dioxus::prelude::*;
use view_knowledge_graph::{GraphLegend, KnowledgeGraphView, apply_search, model::ColorMode};

use crate::Route;
use crate::guide;
use crate::routes::Shell;

/// `/guide/graph` — the whole guide as a link web.
#[component]
pub fn GuideGraph() -> Element {
    let nav = navigator();
    let graph = use_memo(guide::graph);
    let mut query = use_signal(String::new);

    // Search highlights rather than filters: dropping unmatched nodes
    // would also drop the edges that give a match its context, which is
    // the thing you came to the graph for.
    let highlighted = use_memo(move || {
        let q = query.read().clone();
        if q.trim().is_empty() {
            return Vec::new();
        }
        let g = graph.read();
        let mut ids: Vec<String> = apply_search(&g.nodes, &g.edges, &q)
            .matched_ids
            .into_iter()
            .collect();
        // A HashSet's order is not stable across runs, and this feeds a
        // prop that drives rendering — sort so the view does not churn.
        ids.sort_unstable();
        ids
    });

    rsx! {
        Shell {
            div { class: "kf-graph-screen",
                header { class: "kf-graph-bar",
                    h1 { "The guide, as a graph" }
                    input {
                        class: "kf-graph-search",
                        r#type: "search",
                        placeholder: "Highlight…",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                    Link { to: Route::GuideIndex {}, class: "kf-button", "Read it in order" }
                }

                div { class: "kf-graph-canvas",
                    KnowledgeGraphView {
                        graph: graph(),
                        color_mode: ColorMode::Community,
                        highlighted: highlighted(),
                        on_node_click: move |id: String| {
                            if guide::vault().page(&id).is_some() {
                                nav.push(Route::GuidePage { slug: id });
                            }
                        },
                    }
                }

                aside { class: "kf-graph-legend",
                    GraphLegend {
                        nodes: graph().nodes,
                        communities: graph().communities,
                        color_mode: ColorMode::Community,
                        // The legend can filter by kind, but every guide
                        // page is `type: concept` — there is nothing to
                        // filter, so these are inert on purpose.
                        on_toggle_kind: move |_| {},
                        on_show_all: move |()| {},
                    }
                }
            }
        }
    }
}
