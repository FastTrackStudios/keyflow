//! Chart layout and `ChartView::paint`, timed over the Lord of the Fight
//! chart (88 measures, melody + chords + annotations).
//!
//! ```sh
//! KEYFLOW_BENCH_CHART=<LOTF.musicxml> cargo bench --workspace --bench chart
//! ```
//!
//! Prints one JSON object per case — p50/p99/worst in milliseconds over
//! `KEYFLOW_BENCH_ITERS` timed iterations (default 1000) after a warm-up —
//! which is what `docs/perf/baselines/` records. The verdict is the 60 Hz
//! frame (16.67 ms): layout runs on every edit, a paint on every pan, zoom
//! and playhead tick.
//!
//! Was the ignored `layout_chart_meets_60hz_budget` test; a number that
//! is the deliverable belongs in a bench, not a test nobody runs.

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use anyrender::PaintScene;
use engraver::layout::{ChartLayoutEngine, ChartLayoutMode};
use engraver::renderer::ChartView;

const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
const WARMUP: usize = 10;

fn main() {
    let iterations: usize = std::env::var("KEYFLOW_BENCH_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    // The chart the layout tests use. It is not committed to this repo;
    // point `KEYFLOW_BENCH_CHART` at a copy when the in-repo path is absent.
    let fixture = std::env::var_os("KEYFLOW_BENCH_CHART")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../features/examples/png-project-charts/02 LORD OF THE FIGHT Master RS.musicxml",
            )
        });
    let chart = keyflow_lang::musicxml::import_file(&fixture)
        .expect("Lord of the Fight fixture should import");
    let measures: usize = chart.sections.iter().map(|s| s.measures().len()).sum();

    // The engine as the old test built it: default style, no fonts.
    let style: &'static engraver::MStyle = Box::leak(Box::default());
    let engine = ChartLayoutEngine::new(style, Arc::new(Vec::new()), Arc::new(Vec::new()));
    report(
        "layout_chart/paged",
        measures,
        time(iterations, |_| {
            black_box(engine.layout_chart(&chart, &ChartLayoutMode::default()));
        }),
    );
    report(
        "layout_chart/continuous-scroll",
        measures,
        time(iterations, |_| {
            black_box(engine.layout_chart(
                &chart,
                &ChartLayoutMode::ContinuousScroll { width: 1200.0 },
            ));
        }),
    );

    // `ChartView::paint` as a host panel calls it: 1600 px wide, device
    // scale 1, recording into an anyrender scene (what Blitz hands it).
    let mut view = ChartView::new().expect("chart fonts");
    let mut scene = anyrender::Scene::new();
    // A pan with the layout cached: the per-frame cost of scroll, zoom
    // and a moving playhead.
    report(
        "chart_view_paint/cached",
        measures,
        time(iterations, |i| {
            scene.reset();
            let y = (i % 200) as f64 * 10.0;
            black_box(view.paint(
                &mut scene,
                &chart,
                1,
                1600.0,
                1.0,
                1.0,
                (0.0, y),
                Some(i as f64 * 0.1),
            ));
        }),
    );
    // A new key every frame: the cost of an edit, relayout plus paint.
    report(
        "chart_view_paint/relayout",
        measures,
        time(iterations, |i| {
            scene.reset();
            black_box(view.paint(
                &mut scene,
                &chart,
                i as u64 + 2,
                1600.0,
                1.0,
                1.0,
                (0.0, 0.0),
                None,
            ));
        }),
    );
}

/// Sorted per-iteration wall times, in milliseconds.
fn time(iterations: usize, mut f: impl FnMut(usize)) -> Vec<f64> {
    for i in 0..WARMUP {
        f(i);
    }
    let mut samples: Vec<f64> = (0..iterations)
        .map(|i| {
            let start = Instant::now();
            f(i);
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples
}

fn report(case: &str, measures: usize, samples: Vec<f64>) {
    let at = |q: f64| samples[((samples.len() as f64 * q) as usize).min(samples.len() - 1)];
    let p50 = at(0.50);
    println!(
        "{}",
        serde_json::json!({
            "case": case,
            "workload": format!("Lord of the Fight, {measures} measures"),
            "iterations": samples.len(),
            "p50_ms": p50,
            "p99_ms": at(0.99),
            "worst_ms": samples[samples.len() - 1],
            "frame_budget_ms": FRAME_BUDGET_MS,
            "p50_of_frame": p50 / FRAME_BUDGET_MS,
        })
    );
}
