//! The workbench — read the guide and play with it at the same time.
//!
//! A chapter of the guide teaches one idea, and the only way to actually
//! learn a notation is to type it. Making someone alternate between a
//! guide tab and an editor tab is how that loop gets dropped.
//!
//! So the three things sit on one screen:
//!
//! ```text
//! ┌──────────────────────────┬──────────────┐
//! │ editor — you type here   │              │
//! ├──────────────────────────┤ live chart   │
//! │ the guide chapter        │              │
//! └──────────────────────────┴──────────────┘
//! ```
//!
//! The editor is seeded from the chapter's first ```kf+ fence — the one
//! the guide itself marks as a real chart — so "try this page" starts you
//! on the example you were just reading rather than a blank buffer. Every
//! other engraved example on the page is a button that loads it in.

use dioxus::prelude::*;
use editor::{Editor, EditorState};
use editor_state::doc::Doc;
use editor_state::selection::Selection;

use crate::Route;
use crate::chart::ChartFonts;
use crate::chart_preview::ChartPreview;
use crate::guide;
use crate::keyflow_editor::KeyflowEditor;
use crate::routes::Shell;

/// Fallback when a chapter has no engraved example of its own.
const STARTER: &str = "VS 1: | 1 4 | 5 6- |\n";

/// `/learn/:slug` — the guide chapter, an editor, and a live chart.
#[component]
pub fn Workbench(slug: String) -> Element {
    let nav = navigator();

    let Some(page) = guide::vault().page(&slug) else {
        return rsx! {
            Shell {
                section { class: "kf-prose",
                    h1 { "No such guide page" }
                    Link { to: Route::GuideIndex {}, "Back to the guide" }
                }
            }
        };
    };

    let examples = engraved_fences(page.source);
    let mut source = use_signal(|| {
        examples
            .first()
            .cloned()
            .unwrap_or_else(|| STARTER.to_string())
    });

    // The chapter, read-only, in the lower pane.
    let guide_state = use_signal(|| EditorState {
        doc: Doc::new(page.body),
        selection: Selection::caret(0),
        folds: Vec::new(),
        reading_mode: true,
    });

    rsx! {
        Shell {
            ChartFonts {}
            document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }

            div { class: "kf-workbench",
                header { class: "kf-workbench-bar",
                    h1 { "{page.title}" }
                    span { class: "kf-note", "Edit on the left — the chart follows." }
                    Link {
                        to: Route::GuidePage { slug: page.slug.to_string() },
                        class: "kf-button",
                        "Read without the editor"
                    }
                }

                section { class: "kf-workbench-editor",
                    KeyflowEditor {
                        // `key` so switching chapters rebuilds the editor
                        // with the new chapter's example, rather than
                        // keeping the previous buffer.
                        key: "{page.slug}",
                        initial: source(),
                        on_change: move |text| source.set(text),
                    }
                }

                section { class: "kf-workbench-guide",
                    if !examples.is_empty() {
                        div { class: "kf-example-picker",
                            span { class: "kf-note", "Load an example:" }
                            for (i, ex) in examples.iter().enumerate() {
                                button {
                                    key: "{i}",
                                    class: "kf-button",
                                    onclick: {
                                        let ex = ex.clone();
                                        move |_| source.set(ex.clone())
                                    },
                                    "{i + 1}"
                                }
                            }
                        }
                    }
                    article { class: "kf-prose",
                        Editor {
                            state: guide_state,
                            editable: false,
                            decorations: editor::editor_view::DecorationSource::ptr(
                                editor::combined_decorations,
                            ),
                            on_link_click: move |href: String| {
                                let target =
                                    href.split('|').next().unwrap_or(&href).trim().to_owned();
                                // Follow a wikilink WITHOUT leaving the
                                // workbench — the whole point is to keep
                                // reading while the editor stays put.
                                if guide::vault().page(&target).is_some() {
                                    nav.push(Route::Workbench { slug: target });
                                }
                            },
                        }
                    }
                }

                aside { class: "kf-workbench-preview",
                    ChartPreview { source: source() }
                }
            }
        }
    }
}

/// Bodies of the ```kf+ fences in a note — the examples the guide marks as
/// real charts rather than syntax illustrations.
fn engraved_fences(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(i) = rest.find("```kf+\n") {
        let after = &rest[i + 7..];
        let Some(j) = after.find("\n```") else { break };
        out.push(after[..j].to_string());
        rest = &after[j..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_engraved_examples_in_a_page() {
        let md = "text\n```kf+\nVS: | 1 4 |\n```\nmore\n```kf-\nnot this one\n```\n";
        assert_eq!(engraved_fences(md), vec!["VS: | 1 4 |".to_string()]);
    }

    #[test]
    fn an_unterminated_fence_does_not_hang_or_swallow() {
        assert!(engraved_fences("```kf+\nno closing fence\n").is_empty());
    }

    #[test]
    fn every_chapter_but_the_prose_ones_opens_on_a_real_example() {
        // The workbench is much less useful if it starts blank, so a
        // chapter should carry an example to seed it with.
        //
        // Three pages carry none, and should not: two are about things
        // other than notation, and one is a rendering fixture. Naming
        // them beats a "how many may be missing" threshold, which erodes
        // by one every time somebody adds a prose page — which is
        // exactly how this test came to be passing with a count.
        const PROSE_ONLY: [&str; 3] = ["alternatives", "writing-a-chart", "rendering-test"];

        let missing: Vec<&str> = guide::vault()
            .pages
            .iter()
            .filter(|p| !PROSE_ONLY.contains(&p.slug))
            .filter(|p| engraved_fences(p.source).is_empty())
            .map(|p| p.slug)
            .collect();
        assert!(
            missing.is_empty(),
            "these chapters have no engraved example to open on: {missing:?}. \
             Add one, or add the page to PROSE_ONLY if it is deliberately not about notation."
        );

        // And the exemptions stay honest: a page that gains an example
        // should come off the list rather than sit on it forever.
        let stale: Vec<&str> = PROSE_ONLY
            .iter()
            .filter(|slug| {
                guide::vault()
                    .page(slug)
                    .is_some_and(|p| !engraved_fences(p.source).is_empty())
            })
            .copied()
            .collect();
        assert!(
            stale.is_empty(),
            "these now have examples; drop them from PROSE_ONLY: {stale:?}"
        );
    }

    #[test]
    fn every_seeded_example_parses() {
        // The editor seeds from these, so a broken one greets the reader
        // with an error the moment they click "try it".
        for p in guide::vault().pages {
            for ex in engraved_fences(p.source) {
                assert!(
                    keyflow::parse(&ex).is_ok(),
                    "guide page `{}` would seed the workbench with a chart that does not parse:\n{ex}",
                    p.slug
                );
            }
        }
    }

    #[test]
    fn the_fallback_starter_parses() {
        assert!(keyflow::parse(STARTER).is_ok());
    }
}
