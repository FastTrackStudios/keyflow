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
                    ChapterNav { slug: page.slug, compact: true }

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

                    ChapterNav { slug: page.slug, compact: false }

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

/// The guide's table of contents, in reading order, under its stages.
///
/// The chapters are one path from an absolute beginner to the end, and a
/// flat list of eleven links hides that — it reads as reference material
/// you dip into. The stage headings say where the path is going and,
/// more usefully, where someone can stop: "Start here" is the three
/// chapters that get a complete chart on the page, and everything after
/// it is a layer on top of a chart that already works.
///
/// The stage comes from each note's frontmatter, and a heading is
/// emitted whenever it changes — so the order of the headings is the
/// reading order by construction, and a chapter cannot appear under a
/// stage it does not belong to. The index has no stage and so sits above
/// the first heading, which is right: it is the front door, not a step.
#[component]
fn GuideToc(current: &'static str) -> Element {
    let mut stage = "";
    rsx! {
        nav { class: "kf-guide-toc",
            for entry in guide::GUIDE_PAGES {
                if entry.stage != stage {
                    {
                        stage = entry.stage;
                        rsx! { span { class: "kf-toc-stage", "{entry.stage}" } }
                    }
                }
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

/// Previous and next, above and below the page.
///
/// The chapters are one path, and until now the only way to walk it was
/// a line of prose at the very bottom of the note — you had to reach the
/// end to find out where to go, and there was nothing at the top at all.
///
/// Both copies read from [`guide::neighbours`], which reads
/// `GUIDE_PAGES`, which is what the table of contents renders — so the
/// buttons, the sidebar and the reading order are one fact with one
/// source. The prose footer these replace still lives in each note's
/// `source` for the graph; `build.rs` strips it from the rendered body.
///
/// `compact` is the top copy: the same two links with the labels and the
/// boxes dropped, because a full pair of cards above the title competes
/// with the title.
#[component]
fn ChapterNav(slug: &'static str, compact: bool) -> Element {
    let (prev, next) = guide::neighbours(slug);
    if prev.is_none() && next.is_none() {
        return rsx! {};
    }
    rsx! {
        nav {
            class: if compact { "kf-chapter-nav kf-chapter-nav-top" } else { "kf-chapter-nav" },
            "aria-label": "Chapters",
            if let Some(p) = prev {
                Link {
                    to: Route::GuidePage { slug: p.slug.to_string() },
                    class: "kf-chapter-link kf-chapter-prev",
                    span { class: "kf-chapter-dir", "Previous" }
                    span { class: "kf-chapter-title", "{p.title}" }
                }
            }
            if let Some(n) = next {
                Link {
                    to: Route::GuidePage { slug: n.slug.to_string() },
                    class: "kf-chapter-link kf-chapter-next",
                    span { class: "kf-chapter-dir", "Next" }
                    span { class: "kf-chapter-title", "{n.title}" }
                }
            }
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
