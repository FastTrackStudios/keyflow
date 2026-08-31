//! The landing page.
//!
//! One screen, no scrolling. It has exactly one job — show that Keyflow
//! is text you write and a chart you can play from, then send you to the
//! editor or the guide — and a page with one job should not make anyone
//! scroll to find out what it is.
//!
//! # The hero is a live demo, not a picture of one
//!
//! The claim is "as fast as you can hear them". Asserting that in a
//! headline over a screenshot is the version of this page anyone could
//! have made, so instead the source types itself (see
//! [`crate::typewriter`]) and the page beside it re-engraves as it goes —
//! same renderer, same layout engine, same fonts as the editor. Source
//! and page are two halves of one framed window because that is what they
//! are: the same document, before and after.
//!
//! The chart is laid out at true A4, with real page margins and the
//! system breaks a printed chart would get, rather than cropped to the
//! music. What someone sees here is what comes out of the PDF export.

use dioxus::prelude::*;

use crate::Route;
use crate::chart::{Chart, ChartFonts, ChartShape, filename_for};
use crate::highlight::HighlightedSource;
use crate::routes::Shell;
use crate::typewriter::{ChartTypewriter, DEMO_CHARTS};

#[component]
pub fn Home() -> Element {
    // Both panes read this. `ChartTypewriter` seeds it with the first
    // demo chart before its first tick, so the hero is never blank — not
    // even for one frame on load.
    let hero = use_signal(|| DEMO_CHARTS[0].to_string());

    // The window's title bar names the file being typed, and the name
    // changes with the chart — a small thing that makes the frame read as
    // an editor someone is working in rather than a picture of one.
    let filename = use_memo(move || format!("{}.kf", filename_for(&hero())));

    rsx! {
        Shell {
            ChartFonts {}
            // Outside the hero's grid on purpose: it renders an empty
            // fragment, and an empty fragment still leaves a placeholder
            // node behind — inside a grid that is a stray grid item.
            ChartTypewriter {
                output: hero,
                charts: DEMO_CHARTS.iter().map(|c| (*c).to_string()).collect::<Vec<_>>(),
            }

            section { class: "kf-hero",
                div { class: "kf-hero-copy",
                    h1 { class: "kf-display",
                        "Charts you can write as fast as you can hear them."
                    }
                    p { class: "kf-lede",
                        "Keyflow is a text format for the harmonic and rhythmic skeleton of a "
                        "song. Type it in Nashville numbers, Roman numerals or letter names — "
                        "it engraves as you go."
                    }
                    div { class: "kf-cta",
                        Link { to: Route::Editor {}, class: "kf-button kf-button-primary",
                            "Open the editor"
                        }
                        Link { to: Route::GuideIndex {}, class: "kf-button", "Read the guide" }
                    }
                    p { class: "kf-note",
                        "No account. A chart lives in its own URL, so sharing one is sharing a link."
                    }
                }

                div { class: "kf-window",
                    div { class: "kf-window-bar",
                        span { class: "kf-window-name", "{filename}" }
                        span { class: "kf-window-tag", "engraving live" }
                    }
                    div { class: "kf-window-body",
                        HighlightedSource { source: hero() }
                        // `Page`, not `Inline`: the A4 layout, with real
                        // page margins and the system breaks a printed
                        // chart would get.
                        //
                        // `hold_last_good`: mid-word the source is half a
                        // chord symbol and does not parse. Without it the
                        // chart flashes an error between almost every
                        // keystroke.
                        Chart {
                            source: hero(),
                            shape: ChartShape::Page,
                            hold_last_good: true,
                        }
                    }
                }
            }
        }
    }
}
