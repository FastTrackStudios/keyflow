//! The landing page.
//!
//! One job: show, in the first screenful, that Keyflow is text you can read
//! and a chart you can play from — side by side, both real, neither a
//! screenshot. The chart on this page is engraved by the same renderer the
//! editor uses, from the source shown next to it.

use dioxus::prelude::*;

use crate::Route;
use crate::chart::{Chart, ChartFonts};
use crate::routes::Shell;

/// The chart shown on the landing page.
///
/// Deliberately short: it has to read as *obvious* to a musician who has
/// never seen Keyflow, in about four seconds.
const HERO_CHART: &str = "\
Hero - Keyflow
120bpm 4/4 #G

IN: | 1 4 |

VS 1: | 1 4 | 5 6- |

CH: | 4 1 | 5 1 |
";

#[component]
pub fn Home() -> Element {
    rsx! {
        Shell {
            ChartFonts {}
            section { class: "kf-hero",
                div { class: "kf-hero-copy",
                    h1 { "Charts you can write as fast as you can hear them." }
                    p {
                        "Keyflow is a text format for musical charts — the harmonic and "
                        "rhythmic skeleton of a song, in Nashville numbers, Roman numerals, "
                        "or letter names. It reads like something you would scribble on a "
                        "chart, and parses like something a computer can follow in real time."
                    }
                    div { class: "kf-cta",
                        Link { to: Route::Editor {}, class: "kf-button kf-button-primary",
                            "Open the editor"
                        }
                        Link { to: Route::GuideIndex {}, class: "kf-button", "Read the guide" }
                    }
                    p { class: "kf-note",
                        "No account. A chart lives in its own URL — sharing one is sharing a link."
                    }
                }

                div { class: "kf-hero-demo",
                    pre { class: "kf-source", "{HERO_CHART}" }
                    Chart { source: HERO_CHART.to_string() }
                }
            }

            section { class: "kf-features",
                Feature {
                    title: "Every notation system",
                    body: "Nashville numbers, Roman numerals, and letter names are the same
                           chart in three views. Transpose by changing one line.",
                }
                Feature {
                    title: "Lyrics that stay in sync",
                    body: "ChordPro blocks merge into the chart as lyric tracks, synced by
                           line or by syllable — so the words and the changes move together.",
                }
                Feature {
                    title: "Engraved, not drawn",
                    body: "Engraver lays a chart out the way a copyist would: system breaks,
                           width distribution, symbol placement. Screen, SVG, or PDF.",
                }
                Feature {
                    title: "Made to be embedded",
                    body: "The same engine runs in this page, in the desktop app, and in a
                           DAW — the language is a library first and a product second.",
                }
            }
        }
    }
}

#[component]
fn Feature(title: String, body: String) -> Element {
    rsx! {
        article { class: "kf-feature",
            h2 { "{title}" }
            p { "{body}" }
        }
    }
}
