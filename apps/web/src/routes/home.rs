//! The landing page.
//!
//! One screen, no scrolling. It has exactly one job — show that Keyflow
//! is text you write and a chart you can play from, then send you to the
//! editor or the guide — and a page with one job should not make anyone
//! scroll to find out what it is.
//!
//! # The hero is a live demo, not a picture of one
//!
//! The claim is that this is chart writing accelerated. Asserting that in
//! a headline over a screenshot is the version of this page anyone could
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
                        "Chart Writing, "
                        // The gradient sweeps across this word alone, so it
                        // stays one word — a line break inside it would
                        // restart the gradient on the second line box.
                        span { class: "kf-accel", "Accelerated" }
                    }
                    // Three forms of one song, and every edge runs
                    // both ways — parse and export, import and write.
                    // The two diagonals are the headline claim (you can
                    // start from either side); the base says the two
                    // starting points convert to each other too.
                    div { class: "kf-flow",
                        svg {
                            class: "kf-flow-svg",
                            "viewBox": "0 0 340 184",
                            role: "img",
                            "aria-label": "Simple text and a DAW session each convert to a chart, \
                                           and to each other. Every arrow points both ways.",
                            defs {
                                // Markers cannot inherit the referencing
                                // line's stroke without `context-stroke`,
                                // which is not dependable yet — so there
                                // is one head per colour instead.
                                marker {
                                    id: "kf-head-strong",
                                    "viewBox": "0 0 10 10",
                                    "refX": "9", "refY": "5",
                                    "markerWidth": "5", "markerHeight": "5",
                                    "orient": "auto-start-reverse",
                                    path { class: "kf-flow-head-strong", d: "M0,0 L10,5 L0,10 Z" }
                                }
                                marker {
                                    id: "kf-head-soft",
                                    "viewBox": "0 0 10 10",
                                    "refX": "9", "refY": "5",
                                    "markerWidth": "5", "markerHeight": "5",
                                    "orient": "auto-start-reverse",
                                    path { class: "kf-flow-head-soft", d: "M0,0 L10,5 L0,10 Z" }
                                }
                            }

                            // Simple Text ↔ Beautiful Chart
                            line {
                                class: "kf-flow-edge kf-flow-edge-strong",
                                x1: "74.7", y1: "141.5", x2: "157.5", y2: "37.6",
                                "marker-start": "url(#kf-head-strong)",
                                "marker-end": "url(#kf-head-strong)",
                            }
                            // DAW Session ↔ Beautiful Chart
                            line {
                                class: "kf-flow-edge kf-flow-edge-strong",
                                x1: "265.3", y1: "141.5", x2: "182.5", y2: "37.6",
                                "marker-start": "url(#kf-head-strong)",
                                "marker-end": "url(#kf-head-strong)",
                            }
                            // Simple Text ↔ DAW Session
                            line {
                                class: "kf-flow-edge kf-flow-edge-soft",
                                x1: "100", y1: "165", x2: "240", y2: "165",
                                "marker-start": "url(#kf-head-soft)",
                                "marker-end": "url(#kf-head-soft)",
                            }

                            text {
                                class: "kf-flow-node kf-flow-node-out",
                                x: "170", y: "26", "text-anchor": "middle",
                                "Beautiful Chart"
                            }
                            text {
                                class: "kf-flow-node",
                                x: "56", y: "170", "text-anchor": "middle",
                                "Simple Text"
                            }
                            text {
                                class: "kf-flow-node",
                                x: "284", y: "170", "text-anchor": "middle",
                                "DAW Session"
                            }
                        }
                    }
                    div { class: "kf-cta",
                        Link { to: Route::Editor {}, class: "kf-button kf-button-primary",
                            "Open the editor"
                        }
                        Link { to: Route::GuideIndex {}, class: "kf-button", "Read the guide" }
                    }
                    p { class: "kf-note", "Open Source, No Account Required" }
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
