//! The guide.
//!
//! Pages come from `docs/guides/keyflow/*.md`, rendered to HTML at build
//! time (see `build.rs`). What stays live is the part that has to be: a
//! ` ```kf+ ` fence — the guides' own marker for "this is a real chart" —
//! is engraved by the chart renderer and carries a link that opens it in
//! the editor. So every example in the guide is a chart you can take away
//! and start editing, which is the point of teaching a language this way.

use dioxus::prelude::*;

use crate::Route;
use crate::chart_url;
use crate::chart_view::{ChartFonts, ChartSvg};
use crate::guide::{self, Block};
use crate::routes::Shell;

/// `/guide` — opens at the first page.
#[component]
pub fn GuideIndex() -> Element {
    rsx! {
        GuidePage { slug: guide::first_page().slug.to_string() }
    }
}

/// `/guide/:slug` — one guide page, with the contents alongside.
#[component]
pub fn GuidePage(slug: String) -> Element {
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

    rsx! {
        Shell {
            ChartFonts {}
            div { class: "kf-guide",
                nav { class: "kf-guide-toc",
                    for entry in guide::GUIDE_PAGES {
                        Link {
                            to: Route::GuidePage { slug: entry.slug.to_string() },
                            class: if entry.slug == page.slug { "kf-toc-current" } else { "" },
                            "{entry.title}"
                        }
                    }
                }
                article { class: "kf-prose",
                    for (i, block) in page.blocks.iter().enumerate() {
                        GuideBlock { key: "{page.slug}-{i}", block: block }
                    }
                }
            }
        }
    }
}

#[component]
fn GuideBlock(block: &'static Block) -> Element {
    match block {
        // Build-time output from our own guide sources, not user input.
        Block::Html(html) => rsx! { div { dangerous_inner_html: "{html}" } },

        // `kf-`: a syntax illustration. Often not valid Keyflow on its own
        // (the guides annotate these with `→`), so it is shown as source and
        // never handed to the parser.
        Block::Keyflow {
            source,
            engrave: false,
        } => rsx! { pre { class: "kf-source", "{source}" } },

        // `kf+`: a real chart. Engrave it, and let the reader take it.
        Block::Keyflow {
            source,
            engrave: true,
        } => rsx! {
            figure { class: "kf-example",
                ChartSvg { source: (*source).to_string() }
                figcaption {
                    pre { class: "kf-source", "{source}" }
                    Link {
                        to: Route::Chart { data: chart_url::encode(source) },
                        class: "kf-button",
                        "Open in the editor"
                    }
                }
            }
        },
    }
}
