//! The guide — a vault page, rendered by the editor.
//!
//! The page is not pre-rendered HTML. It is the note's markdown, handed to
//! the real editor in read-only mode, which is what buys:
//!
//! - **`[[wikilink]]` navigation.** The editor renders them as links and
//!   fires `on_link_click` with the target; the site turns that into a
//!   route. No markdown rewriting, no link syntax of our own.
//! - **The ```kf fence family, natively.** `kf`, `kf+` and `kf-` are
//!   already the editor's convention *and* the guides' — a `kf+` fence
//!   shows source and chart, `kf-` shows highlighted source, `kf` shows
//!   the chart with a source toggle. `editor-keyflow` registers the
//!   renderer that engraves them (see [`crate::App`]).
//! - Everything else the editor does with markdown, for free, and staying
//!   in step with it rather than drifting.
//!
//! Three columns: the table of contents on the left, the note in the
//! middle, and on the right the **local graph** — this concept and
//! everything one hop from it, drawn and clickable. That is the view that
//! makes a vault feel like a vault rather than a chapter list: you can see
//! what a page touches, and go there, without reading it first.

use dioxus::prelude::*;
use editor::{Editor, EditorState};
use editor_state::doc::Doc;
use editor_state::selection::Selection;

use view_knowledge_graph::{KnowledgeGraphView, model::ColorMode};

use crate::Route;
use crate::guide;
use crate::routes::Shell;

/// `/guide` — opens at the first page.
#[component]
pub fn GuideIndex() -> Element {
    rsx! {
        GuidePage { slug: guide::first_page().slug.to_string() }
    }
}

/// `/guide/:slug` — one note, with its contents and backlinks.
#[component]
pub fn GuidePage(slug: String) -> Element {
    let nav = navigator();

    let Some(page) = guide::page(&slug) else {
        return rsx! {
            Shell {
                section { class: "kf-prose",
                    h1 { "No such guide page" }
                    Link { to: Route::GuideIndex {}, "Back to the guide" }
                }
            }
        };
    };

    // Reading mode, not just `editable: false`: the former also keeps
    // every source marker hidden regardless of caret position, which is
    // what makes this read as a document rather than an editor someone
    // switched off.
    let state = use_signal(|| EditorState {
        doc: Doc::from_str(page.body),
        selection: Selection::caret(0),
        folds: Vec::new(),
        reading_mode: true,
    });

    let graph = use_memo(guide::graph);
    let slug_for_local = page.slug;
    let backlinks = use_memo(move || guide::backlinks(&graph.read(), &slug));
    let local = use_memo(move || guide::local_graph(&graph.read(), slug_for_local));

    rsx! {
        Shell {
            document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
            div { class: "kf-guide",
                GuideToc { current: page.slug }

                article { class: "kf-guide-page",
                    div { class: "kf-page-actions",
                        Link {
                            to: Route::Workbench { slug: page.slug.to_string() },
                            class: "kf-button kf-button-primary",
                            "Try this chapter"
                        }
                    }
                    Editor {
                        state,
                        // Read-only: contenteditable off, no keymap, no vim.
                        // Link clicks still fire, which is the whole point.
                        editable: false,
                        decorations: editor::editor_view::DecorationSource::ptr(editor::combined_decorations),
                        on_link_click: move |href: String| {
                            // A wikilink hands back its target. Anything
                            // that names a guide page routes internally;
                            // everything else is left to the browser.
                            let target = href.split('|').next().unwrap_or(&href).trim().to_owned();
                            if guide::page(&target).is_some() {
                                nav.push(Route::GuidePage { slug: target });
                            }
                        },
                    }

                    Backlinks { pages: backlinks() }
                }

                LocalGraph { graph: local(), current: page.slug }
            }
        }
    }
}

/// This page and everything one hop from it.
///
/// Nodes are coloured by community, matching the full graph view, so the
/// two reads agree about which cluster a concept belongs to. Clicking a
/// node navigates.
#[component]
fn LocalGraph(graph: view_knowledge_graph::model::WikiGraph, current: &'static str) -> Element {
    let nav = navigator();

    // A page with no links has nothing to draw, and an empty box beside
    // the text reads as broken rather than as "no connections".
    if graph.nodes.len() < 2 {
        return rsx! {};
    }

    rsx! {
        aside { class: "kf-local-graph",
            h2 { "Connections" }
            div { class: "kf-local-graph-canvas",
                KnowledgeGraphView {
                    graph,
                    color_mode: ColorMode::Community,
                    // Marks the page you are on without dimming the rest —
                    // `highlighted` would, and here everything IS relevant.
                    active: Some(current.to_string()),
                    // Tighter than the full view: this is a handful of
                    // nodes in a narrow rail, not a whole map. Node
                    // labels are drawn beside their node, so the layout
                    // has to leave room for them inside the frame —
                    // at the default spacing the outer labels clipped.
                    spacing: 0.45,
                    node_scale: 0.8,
                    on_node_click: move |id: String| {
                        if guide::page(&id).is_some() {
                            nav.push(Route::GuidePage { slug: id });
                        }
                    },
                }
            }
            Link { to: Route::GuideGraph {}, class: "kf-note", "See the whole graph" }
        }
    }
}

/// The guide's table of contents, in reading order.
#[component]
fn GuideToc(current: &'static str) -> Element {
    rsx! {
        nav { class: "kf-guide-toc",
            for entry in guide::GUIDE_PAGES {
                Link {
                    to: Route::GuidePage { slug: entry.slug.to_string() },
                    class: if entry.slug == current { "kf-toc-current" } else { "" },
                    "{entry.title}"
                }
            }
            Link { to: Route::GuideGraph {}, class: "kf-toc-graph", "Graph" }
        }
    }
}

/// What leads here.
///
/// Rendered only when there is something to show — an empty "Referenced
/// by" heading is worse than none.
#[component]
fn Backlinks(pages: Vec<&'static guide::GuidePage>) -> Element {
    if pages.is_empty() {
        return rsx! {};
    }
    rsx! {
        footer { class: "kf-backlinks",
            h2 { "Referenced by" }
            ul {
                for p in pages {
                    li { key: "{p.slug}",
                        Link { to: Route::GuidePage { slug: p.slug.to_string() }, "{p.title}" }
                    }
                }
            }
        }
    }
}
