//! The guide — a vault page, statically generated.
//!
//! Three columns: the table of contents on the left, the note in the
//! middle, and on the right the **local graph** — this concept and
//! everything one hop from it, drawn and clickable. That last is the
//! view that makes a vault feel like a vault rather than a chapter list:
//! you can see what a page touches, and go there, without reading it
//! first.
//!
//! ## What changed, and what deliberately did not
//!
//! The note is no longer handed to the editor in read-only mode. That
//! gave the guide `[[wikilink]]` navigation and engraved ```kf fences —
//! and charged every reader the editor, its state machine, its
//! decoration pipeline and a WebGL2 chart surface before a paragraph
//! appeared. All of it produced markup that could not change after the
//! build, so `build.rs` produces that markup instead: prose to HTML,
//! wikilinks to real links, and every chart engraved to inline SVG by
//! the host engraver, with no GPU anywhere.
//!
//! Everything else stays. The contents rail, the chapter walk, the
//! backlinks, the local graph and the workbench link are what make this
//! a guide rather than a page of text, and they are all still here — the
//! graph still interactive, because `dx build --ssg` pre-renders the
//! page and the bundle then hydrates it into the ordinary app.
//!
//! The editor is still in the site. It lives at `/learn/:slug`, the
//! workbench, where someone has asked to edit something.

use dioxus::prelude::*;
use ssg_ui::VaultArticle;
use view_knowledge_graph::model::ColorMode;
use view_knowledge_graph::{KnowledgeGraphView, model::WikiGraph};

use crate::Route;
use crate::guide::{self, vault};
use crate::routes::Shell;

/// `/guide` — opens at the first page.
#[component]
pub fn GuideIndex() -> Element {
    let Some(first) = vault().first() else {
        return rsx! {
            Shell {
                section { class: "kf-prose", h1 { "The guide is empty" } }
            }
        };
    };
    rsx! {
        GuidePage { slug: first.slug.to_string() }
    }
}

/// `/guide/:slug` — one note, with its contents, backlinks and graph.
#[component]
pub fn GuidePage(slug: String) -> Element {
    let Some(page) = vault().page(&slug) else {
        return rsx! {
            Shell {
                section { class: "kf-prose",
                    h1 { "No such guide page" }
                    Link { to: Route::GuideIndex {}, "Back to the guide" }
                }
            }
        };
    };

    // A `#fragment` scrolls the browser on a full page load, and does
    // nothing on a client-side route change — the router swaps the
    // content without the browser ever seeing a navigation. So a link to
    // a section landed at the top of the right page, which reads as a
    // broken link rather than a working one.
    scroll_to_hash();

    rsx! {
        Shell {
            // The engraving fonts, declared once for every chart on the
            // page — the SVGs reference families by name rather than
            // each embedding a copy.
            //
            // Under `dev-guide` the dev server subsets the faces for the
            // charts it just re-rendered and sends them with the pages,
            // so a chart that starts using a symbol no other chart uses
            // draws it immediately. The baked stylesheet is what the
            // published site uses, and what the first paint uses here.
            GuideFonts {}

            document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
            div { class: "kf-guide",
                GuideToc { current: page.slug }

                article { class: "kf-guide-page",
                    ChapterNav { slug: page.slug, compact: true }

                    // The note: HTML and SVG produced by build.rs, which
                    // renders it through the EDITOR's markdown pass. That
                    // emits the editor's own classes — `cm-line`, `md-h1`,
                    // `md-callout` — and the editor's stylesheet scopes
                    // nearly all of them under `.editor-root`, so the
                    // article has to carry that class or the whole chapter
                    // arrives as unstyled divs.
                    //
                    // `kf-prose` stays alongside it for the site's own
                    // document rhythm.
                    VaultArticle { page, class: "kf-prose editor-root" }

                    ChapterNav { slug: page.slug, compact: false }

                    Backlinks { current: page.slug }
                }

                LocalGraph { current: page.slug }
            }
        }
    }
}

/// Scroll to `location.hash` once the page has rendered.
///
/// Runs after every render of a guide page rather than once on mount:
/// the content a fragment points at may arrive after the first paint —
/// under `dev-guide` the live vault replaces it a moment later — and a
/// scroll to an element that does not exist yet is a scroll to nothing.
fn scroll_to_hash() {
    #[cfg(target_arch = "wasm32")]
    use_effect(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(hash) = window.location().hash() else {
            return;
        };
        let Some(id) = hash.strip_prefix('#').filter(|s| !s.is_empty()) else {
            return;
        };
        let Some(target) = window
            .document()
            .and_then(|d| d.get_element_by_id(&urlencoding_decode(id)))
        else {
            return;
        };
        target.scroll_into_view();
    });
}

/// Percent-decode a fragment.
///
/// A heading id is slugged from its text, so an id is plain ASCII — but
/// the browser percent-encodes what it puts in `location.hash`, and
/// `%2D` never matches `-` in `getElementById`.
#[cfg(target_arch = "wasm32")]
fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

/// The engraving typefaces for this page's charts.
///
/// The baked, subsetted stylesheet — except in a dev build once the live
/// render has sent its own, which covers glyphs the last build never saw.
#[component]
fn GuideFonts() -> Element {
    #[cfg(feature = "dev-guide")]
    if let Some(css) = crate::guide_live::LIVE_FONT_CSS.read().clone() {
        return rsx! { document::Style { {css} } };
    }
    rsx! { document::Stylesheet { href: guide::CHART_FONTS } }
}

/// This page and everything one hop from it.
///
/// The interactive graph, not a picture of one: it is pre-rendered with
/// the page and the bundle hydrates it, so it arrives drawn and then
/// becomes pannable. Nodes are coloured by community, matching the full
/// graph view, so the two reads agree about which cluster a concept
/// belongs to. Clicking a node navigates.
#[component]
fn LocalGraph(current: &'static str) -> Element {
    let nav = navigator();

    // Built from the notes themselves — `guide::graph` reads each page's
    // verbatim source, which is where `type:` and the wikilinks are.
    // Memoised because the whole-vault build is the expensive half and
    // the slug is the only thing that varies.
    let graph = use_memo(guide::graph);
    let local: Memo<WikiGraph> = use_memo(move || guide::local_graph(&graph.read(), current));

    // A page with no links has nothing to draw, and an empty box beside
    // the text reads as broken rather than as "no connections".
    if local.read().nodes.len() < 2 {
        return rsx! {};
    }

    rsx! {
        aside { class: "kf-local-graph",
            h2 { "Connections" }
            div { class: "kf-local-graph-canvas",
                KnowledgeGraphView {
                    graph: local(),
                    color_mode: ColorMode::Community,
                    // Marks the page you are on without dimming the rest
                    // — `highlighted` would, and here everything IS
                    // relevant.
                    active: Some(current.to_string()),
                    // Tighter than the full view: this is a handful of
                    // nodes in a narrow rail, not a whole map. Node
                    // labels are drawn beside their node, so the layout
                    // has to leave room for them inside the frame — at
                    // the default spacing the outer labels clipped.
                    spacing: 0.45,
                    node_scale: 0.8,
                    on_node_click: move |id: String| {
                        if vault().page(&id).is_some() {
                            nav.push(Route::GuidePage { slug: id });
                        }
                    },
                }
            }
            Link { to: Route::GuideGraph {}, class: "kf-note-link", "See the whole graph" }
        }
    }
}

/// The guide's table of contents, in reading order, under its stages.
///
/// The chapters are one path from an absolute beginner to the end, and a
/// flat list of eleven links hides that — it reads as reference material
/// you dip into. The stage headings say where the path is going and,
/// more usefully, where someone can stop.
///
/// The stage comes from each note's frontmatter, and `StaticVault::stages`
/// groups consecutive pages by it — so the order of the headings *is*
/// the reading order by construction, and a chapter cannot appear under
/// a stage it does not belong to.
#[component]
fn GuideToc(current: &'static str) -> Element {
    rsx! {
        nav { class: "kf-guide-toc", "aria-label": "Contents",
            for (stage , pages) in vault().stages() {
                // A category whose name is one of its own pages — Chords
                // above a page called Chords — listed the word twice, a
                // label and then a link under it. The heading IS that
                // page: it links, and the page drops out of the list
                // below rather than being repeated.
                {
                    let overview = pages.iter().copied().find(|p| p.title == stage);
                    rsx! {
                        if let Some(page) = overview {
                            Link {
                                key: "{page.slug}",
                                to: Route::GuidePage { slug: page.slug.to_string() },
                                class: if page.slug == current {
                                    "kf-toc-stage kf-toc-stage-link kf-toc-current"
                                } else {
                                    "kf-toc-stage kf-toc-stage-link"
                                },
                                "{stage}"
                            }
                        } else if !stage.is_empty() {
                            span { class: "kf-toc-stage", "{stage}" }
                        }
                        for entry in pages.iter().copied().filter(|p| p.title != stage) {
                            Link {
                                key: "{entry.slug}",
                                to: Route::GuidePage { slug: entry.slug.to_string() },
                                class: if entry.slug == current { "kf-toc-current" } else { "" },
                                "{entry.title}"
                            }
                        }
                    }
                }
            }
            // Below the chapters, with the graph: neither is a chapter,
            // both are somewhere you go from here.
            Link { to: Route::GuideGraph {}, class: "kf-toc-graph", "Graph" }
            Link { to: Route::AppendixIndex {}, class: "kf-toc-graph", "Appendix" }
        }
    }
}

/// Previous and next, above and below the page.
///
/// Both copies read the vault's reading order, which is what the table
/// of contents renders — so the buttons, the sidebar and the order are
/// one fact with one source.
///
/// `compact` is the top copy: the same two links with the labels and the
/// boxes dropped, because a full pair of cards above the title competes
/// with the title.
#[component]
fn ChapterNav(slug: &'static str, compact: bool) -> Element {
    let prev = vault().previous(slug);
    let next = vault().next(slug);
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
fn Backlinks(current: &'static str) -> Element {
    let pages = vault().backlinks(current);
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
