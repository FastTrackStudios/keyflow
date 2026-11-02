//! `ChartView::paint` timings, p50/p99, on a real chart.
//!
//! Ignored by default (timings belong to a quiet machine, not CI). Run:
//!
//! ```sh
//! cargo test --workspace --test chart_paint_bench -- --ignored --nocapture
//! ```
//!
//! Uses only `ChartView::new` and `ChartView::paint`, the API the chart
//! had before themes, so the same file measures an older checkout too and
//! the two sets of numbers compare like for like.
//!
//! Paints record into `anyrender::Scene`, the recording scene Blitz hosts
//! replay — so this is the chart's own cost per frame (walking the scene
//! graph, outlining glyphs, emitting commands), not a rasteriser's.

use std::time::Instant;

use anyrender::PaintScene as _;
use engraver::renderer::ChartView;

/// A real chart from the repo's own examples: repeats, sections, text
/// cues, rhythm slashes — about a page and a half.
pub const SONG: &str =
    include_str!("../../../examples/chordsheet-compat/kf/ED SHEERAN - THINKING OUT LOUD.kf");

pub fn chart() -> keyflow_proto::Chart {
    keyflow_lang::text::chart::parse_chart(SONG).expect("the example chart parses")
}

/// p50/p99/max of `samples` (milliseconds), as one JSON line.
pub fn report(label: &str, mut samples: Vec<f64>) -> (f64, f64) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    let at = |q: f64| samples[((q * n as f64).ceil() as usize).clamp(1, n) - 1];
    let mean = samples.iter().sum::<f64>() / n as f64;
    let (p50, p99) = (at(0.50), at(0.99));
    println!(
        "{{\"bench\":\"{label}\",\"n\":{n},\"p50_ms\":{p50:.4},\"p99_ms\":{p99:.4},\
         \"max_ms\":{:.4},\"mean_ms\":{mean:.4}}}",
        samples[n - 1]
    );
    (p50, p99)
}

const WARMUP: usize = 30;
const FRAMES: usize = 600;

#[test]
#[ignore = "benchmark — run explicitly with --ignored --nocapture"]
fn chart_paint_default_theme() {
    let chart = chart();
    let mut view = ChartView::new().expect("view");
    let mut scene = anyrender::Scene::new();

    // Warm: the layout is cached, each frame is a repaint at a moving
    // playhead and pan — what a playing transport costs.
    let mut samples = Vec::with_capacity(FRAMES);
    for frame in 0..WARMUP + FRAMES {
        scene.reset();
        let secs = (frame % 240) as f64 * 0.25;
        let start = Instant::now();
        let size = view.paint(
            &mut scene,
            &chart,
            1,
            816.0,
            2.0,
            1.0,
            (0.0, (frame % 50) as f64),
            Some(secs),
        );
        let elapsed = start.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box((size, scene.commands.len()));
        if frame >= WARMUP {
            samples.push(elapsed);
        }
    }
    report("paint_warm_default", samples);

    // Cold: a new key every frame, so each paint re-lays the chart out
    // first — what the paint thread paid on every edit when layout ran
    // inside `paint`.
    let mut samples = Vec::with_capacity(FRAMES / 4);
    for frame in 0..WARMUP / 3 + FRAMES / 4 {
        scene.reset();
        let start = Instant::now();
        let size = view.paint(
            &mut scene,
            &chart,
            1000 + frame as u64,
            816.0,
            2.0,
            1.0,
            (0.0, 0.0),
            Some(1.0),
        );
        let elapsed = start.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box((size, scene.commands.len()));
        if frame >= WARMUP / 3 {
            samples.push(elapsed);
        }
    }
    report("paint_relayout_default", samples);
}
