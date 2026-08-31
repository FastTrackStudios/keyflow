//! The landing page's chart, typed out.
//!
//! [`ChartTypewriter`] renders nothing. It drives a `Signal<String>` on a
//! timer — type a chart in a character at a time, hold it, delete it,
//! move to the next, loop — so whatever reads that signal animates. On
//! the hero that is the source pane *and* the engraved chart, side by
//! side, both fed from the same signal: you watch the notation appear as
//! the text does.
//!
//! Ported from the old fasttrackstudio site (`apps/site`, deleted in
//! `367c3f348`), where it drove a fake editor window in a showcase
//! carousel. Three things changed on the way in, each of them a bug the
//! original was carrying:
//!
//! - **Timers go through `gloo-timers`.** The original hand-rolled
//!   `js_sys::Promise` + `set_timeout` under a `cfg(wasm32)`, with
//!   `tokio::time::sleep` as the fallback. `tokio`'s timer needs a
//!   reactor and panics in the browser — the exact trap the repo's
//!   editor debounce is target-split to avoid.
//! - **Indices are chars, not bytes.** The original compared a char index
//!   against `str::len()`, which counts bytes, and its sibling
//!   `Typewriter` then sliced `word[..char_index + 1]` — a byte range
//!   built from a char count. Any non-ASCII character in a chart panics
//!   on a slice boundary. Here the chart is a `Vec<char>` and there is no
//!   slicing to get wrong.
//! - **It runs in a `use_future`, not a `use_effect`.** An effect that
//!   spawns an endless loop spawns *another* one every time it re-runs,
//!   and they all keep writing the same signal.

use dioxus::prelude::*;

/// The charts the hero cycles through.
///
/// Carried over from the old site, in the same order, because the order
/// is the argument: a plain pop chart reads as obvious, and by the fourth
/// you are looking at pushes, hits, and slash rhythm without having been
/// told a syntax. `Thriller` goes last for that reason — it is the one
/// that looks like something you could not have typed in a text editor.
pub const DEMO_CHARTS: &[&str] = &[
    // Pop ballad in A — the "oh, that's all it is" chart.
    "Midnight Dreams\n72bpm 4/4 #A\n\nVS\nA D F#m E\n\nCH\nD E A F#m\nD E C#m F#m\n\nBR\nF#m E D A/C#\n\nOUT\nA\n",
    // Jazz standard in Bb — sevenths, a half-diminished, a ii-V.
    "Autumn Leaves\n140bpm 4/4 #Bb\n\nVS\nCm7 F7 Bbmaj7 Ebmaj7\nAm7b5 D7 Gm //\n\nCH\nAm7b5 D7 Gm //\nCm7 F7 Bbmaj7 //\n\nOUT\nGm\n",
    // R&B in Cm — extensions and an altered dominant.
    "City Lights\n95bpm 4/4 #Cm\n\nVS\nCm9 Fm9 Bb13 Ebmaj7\nAbmaj7 G7#9 Cm9 //\n\nCH\nFm9 G7 Cm9 //\nAbmaj7 Bb9 Cm9 //\n\nBR\nAbmaj7 G7 Cm //\n\nOUT\nCm9\n",
    // Funk — triplet pushes, a hits line, bar-level rhythm.
    "Thriller\n120bpm 4/4 #Eb\n/push = triplet\n\nHITS\nr8t >Ab9_8t r8t r8t r8t >F9_8t r2\ns1\n\nIN\n>'Cm . . .\n\nVS\n>'F/C . Cm .\n\nCH\n>Cm/Eb / 'Eb /// | 'Eb / 'F/C / 'Cm // |\n'F/A //// | 'Fm9  ////\n\nINST\nCm . F6 // Abdim7 'Csus2\n",
];

/// Milliseconds per character while typing.
const TYPE_MS: u32 = 38;

/// Milliseconds per character while deleting. Backspacing is boring to
/// watch, so it runs several times faster than typing.
const DELETE_MS: u32 = 12;

/// How long a finished chart stays on screen before it is deleted.
const HOLD_MS: u32 = 3800;

/// A beat of stillness on an empty pane before the next chart starts, so
/// the two charts read as separate rather than as one long edit.
const BLANK_MS: u32 = 400;

/// Types charts into `output`, forever.
///
/// Renders nothing — mount it anywhere inside the component that owns the
/// signal. `output` is seeded with the first chart immediately, so the
/// page has something engraved on it before the first tick rather than
/// opening on an empty pane.
#[component]
pub fn ChartTypewriter(
    /// The signal to type into.
    output: Signal<String>,
    /// The charts to cycle, in order.
    charts: Vec<String>,
) -> Element {
    // `use_future`, not `use_effect` + `spawn`: this future never
    // finishes, and an effect would start a second copy of it on every
    // re-run — several loops, all writing `output`, at different offsets.
    use_future(move || {
        let charts = charts.clone();
        let mut output = output;

        async move {
            // Nothing to type. Not an error: a caller that passes an
            // empty list gets a static pane, which is what it asked for.
            if charts.is_empty() {
                return;
            }

            // Chars, not bytes. A chart is not guaranteed ASCII — the
            // guide's examples already carry en-dashes and arrows — and
            // indexing a `str` by a char count is how the original
            // panicked.
            let charts: Vec<Vec<char>> = charts.iter().map(|c| c.chars().collect()).collect();

            // Open on the first chart already typed. Waiting a full
            // typing pass to show anything makes the page look broken on
            // load, so the animation starts by deleting what is there.
            output.set(charts[0].iter().collect());

            // An animation that never stops is the exact thing
            // `prefers-reduced-motion` is for — it runs for as long as
            // the tab is open, in the reader's peripheral vision, on the
            // page they have to read to find the link they came for.
            // Leave the first chart on screen and do nothing else.
            if prefers_reduced_motion() {
                return;
            }

            let mut index = 0_usize;
            loop {
                let chart = &charts[index];

                // Hold the finished chart, then take it apart.
                sleep(HOLD_MS).await;
                for len in (0..chart.len()).rev() {
                    output.set(chart[..len].iter().collect());
                    sleep(DELETE_MS).await;
                }

                // On to the next one.
                index = (index + 1) % charts.len();
                let chart = &charts[index];
                sleep(BLANK_MS).await;
                for len in 1..=chart.len() {
                    output.set(chart[..len].iter().collect());
                    sleep(TYPE_MS).await;
                }
            }
        }
    });

    rsx! {}
}

/// Whether the reader has asked for less movement.
///
/// Defaults to "no" everywhere the question cannot be asked — off wasm,
/// or in a browser that does not answer — because the animation is the
/// page's whole argument and silently dropping it would be the worse
/// failure.
fn prefers_reduced_motion() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| {
                w.match_media("(prefers-reduced-motion: reduce)")
                    .ok()
                    .flatten()
            })
            .is_some_and(|q| q.matches())
    }

    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Wait, in the browser.
///
/// Off wasm this returns immediately, which would spin the loop above
/// into a busy wait — so the loop is only ever driven in a browser. The
/// host build of this crate exists for `cargo check` and the tests, and
/// never mounts a component.
async fn sleep(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(ms).await;

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ms;
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_demo_chart_engraves() {
        // The animation types these one character at a time, so a chart
        // that does not parse at FULL length is a chart the hero would
        // hold on screen, broken, for four seconds.
        for source in DEMO_CHARTS {
            let title = source.lines().next().unwrap_or("<empty>");
            for shape in [
                crate::chart::ChartShape::Inline,
                crate::chart::ChartShape::Page,
            ] {
                assert!(
                    crate::chart::engrave(source, shape).is_ok(),
                    "demo chart `{title}` does not engrave as {shape:?}",
                );
            }
        }
    }

    #[test]
    fn every_demo_chart_fits_on_one_page() {
        // The hero frames exactly one A4 page. A chart that runs to two
        // puts a second page in the same fixed-height box, and both get
        // squeezed to half height — which reads as the engraver being
        // broken rather than the chart being long.
        for source in DEMO_CHARTS {
            let title = source.lines().next().unwrap_or("<empty>");
            let pages = crate::chart::engrave(source, crate::chart::ChartShape::Page)
                .expect("the demo charts engrave");
            assert_eq!(
                pages.len(),
                1,
                "demo chart `{title}` lays out to {} pages",
                pages.len(),
            );
        }
    }

    #[test]
    fn a_demo_chart_is_worth_watching_typed() {
        // Two ways this animation stops being interesting: a chart so
        // short the typing is over before you look at it, or one so long
        // the loop feels stuck. Neither is a correctness bug, which is
        // exactly why it needs a test — nothing else would catch it.
        for source in DEMO_CHARTS {
            let title = source.lines().next().unwrap_or("<empty>");
            let chars = source.chars().count();
            assert!(
                (60..=700).contains(&chars),
                "demo chart `{title}` is {chars} chars — {:.1}s to type",
                chars as f64 * f64::from(TYPE_MS) / 1000.0,
            );
        }
    }

    #[test]
    fn the_charts_are_all_different_songs() {
        // A duplicate in the list reads as the animation being stuck.
        let mut titles: Vec<&str> = DEMO_CHARTS
            .iter()
            .map(|c| c.lines().next().unwrap_or_default())
            .collect();
        let before = titles.len();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(before, titles.len(), "two demo charts share a title");
    }
}
