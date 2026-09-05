//! The appendix — the tables, at their own URL.
//!
//! A separate vault from the guide, not a stage of it. Roots, Qualities,
//! Extensions, Alterations and Sections are looked up rather than read,
//! and nothing about them belongs in a reading order: nobody arrives at
//! Alterations because they finished Extensions.
//!
//! Same rendering as a chapter — the pages carry engraved charts and go
//! through the editor's markdown pass at build time — with the chapter
//! furniture dropped. There is no previous or next, because there is no
//! next.

use dioxus::prelude::*;
use ssg_ui::VaultArticle;

use crate::Route;
use crate::guide;
use crate::routes::Shell;

/// `/appendix` — opens at the first table.
#[component]
pub fn AppendixIndex() -> Element {
    let Some(first) = guide::appendix().first() else {
        return rsx! {
            Shell {
                section { class: "kf-prose", h1 { "The appendix is empty" } }
            }
        };
    };
    rsx! {
        AppendixPage { slug: first.slug.to_string() }
    }
}

/// `/appendix/:slug` — one table.
#[component]
pub fn AppendixPage(slug: String) -> Element {
    let Some(page) = guide::appendix().page(&slug) else {
        return rsx! {
            Shell {
                section { class: "kf-prose",
                    h1 { "No such appendix page" }
                    Link { to: Route::AppendixIndex {}, "Back to the appendix" }
                }
            }
        };
    };

    rsx! {
        Shell {
            document::Stylesheet { href: guide::CHART_FONTS }
            document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
            div { class: "kf-guide",
                AppendixToc { current: page.slug }
                article { class: "kf-guide-page",
                    VaultArticle { page, class: "kf-prose editor-root" }
                }
            }
        }
    }
}

/// The appendix's own contents.
///
/// Flat, and deliberately: the guide's rail carries stages because a
/// guide has an order. Five tables do not.
#[component]
fn AppendixToc(current: &'static str) -> Element {
    rsx! {
        nav { class: "kf-guide-toc", "aria-label": "Appendix",
            Link { to: Route::GuideIndex {}, class: "kf-toc-graph", "← The guide" }
            span { class: "kf-toc-stage", "Appendix" }
            for entry in guide::appendix().pages {
                Link {
                    key: "{entry.slug}",
                    to: Route::AppendixPage { slug: entry.slug.to_string() },
                    class: if entry.slug == current { "kf-toc-current" } else { "" },
                    "{entry.title}"
                }
            }
        }
    }
}
