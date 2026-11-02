//! Chart themes, rendered: the default look against a golden captured
//! before themes existed, and two themes that differ from it — and from
//! each other — in geometry, type, colour and artwork.
//!
//! Goldens live in `tests/golden/chart_theme/`. To regenerate after an
//! intended change, run with `KEYFLOW_UPDATE_GOLDEN=1` and review the PNGs.
//! `default.png` is the one that matters most: it was rendered by the
//! pre-theme `ChartView` (keyflow `fd88851`), so matching it is the proof
//! that the default look did not regress — and the switch never rewrites
//! it.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyrender::{ImageRenderer as _, PaintScene as _};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use engraver::api::pipeline::Paper;
use engraver::layout::PageMargins;
use engraver::renderer::{
    ArtworkLayer, ChartGeometry, ChartTheme, ChartTypography, ChartView, LayoutWorker,
    MARKER_COLOR, PageArtwork, REPEAT_COLOR, RenderConfig,
};
use kurbo::Rect;
use peniko::Color;

const SONG: &str =
    include_str!("../../../examples/chordsheet-compat/kf/ED SHEERAN - THINKING OUT LOUD.kf");
const KEY: u64 = 7;
/// Device scale for the goldens: small enough to keep the PNGs light,
/// large enough that a glyph-level change shows.
const SCALE: f64 = 1.0;

fn chart() -> keyflow_proto::Chart {
    keyflow_lang::text::chart::parse_chart(SONG).expect("the example chart parses")
}

/// Dark paper, cream ink, slate staff lines, A4 with wide margins and
/// airy systems, a larger staff and chords, sans titles, and a ruled
/// frame with a corner mark.
fn night() -> ChartTheme {
    ChartTheme {
        render: RenderConfig {
            background_color: Color::from_rgb8(0x15, 0x18, 0x1f),
            foreground_color: Color::from_rgb8(0xee, 0xe6, 0xd3),
            staff_color: Color::from_rgb8(0x7d, 0x8a, 0x9c),
            cursor_color: Some(Color::from_rgba8(0xf5, 0xa5, 0x24, 0xe6)),
            accents: vec![
                (REPEAT_COLOR, Color::from_rgb8(0xff, 0x7a, 0x6b)),
                (MARKER_COLOR, Color::from_rgb8(0xff, 0x7a, 0x6b)),
            ],
            staff_space: Some(6.0),
            ..RenderConfig::default()
        },
        geometry: ChartGeometry {
            paper: Paper::A4,
            margins: Some(PageMargins {
                top: 64.0,
                right: 56.0,
                bottom: 64.0,
                left: 56.0,
            }),
            system_spacing: Some(56.0),
        },
        typography: ChartTypography {
            chord_scale: 1.25,
            font_aliases: vec![
                ("title-bold".into(), "FreeSans".into()),
                ("Chicago".into(), "FreeSans".into()),
            ],
            style: None,
        },
        artwork: vec![
            PageArtwork::from_svg(
                "M24 24 H571 V818 H24 Z M30 30 H565 V812 H30 Z",
                Rect::new(0.0, 0.0, 595.0, 842.0),
                ArtworkLayer::UnderMusic,
            )
            .expect("frame path")
            .with_stroke(Color::from_rgb8(0x5d, 0x6c, 0x82), 1.5),
            PageArtwork::from_svg(
                "M571 818 L541 818 L571 788 Z",
                Rect::new(0.0, 0.0, 595.0, 842.0),
                ArtworkLayer::OverMusic,
            )
            .expect("corner path")
            .with_fill(Color::from_rgb8(0xf5, 0xa5, 0x24)),
        ],
    }
}

/// Blueprint: deep blue paper, white ink, pale-cyan lines, Letter with
/// tight margins, a smaller staff and chords, and a header band.
fn blueprint() -> ChartTheme {
    ChartTheme {
        render: RenderConfig {
            background_color: Color::from_rgb8(0x1d, 0x3b, 0x66),
            foreground_color: Color::WHITE,
            staff_color: Color::from_rgb8(0x9e, 0xd8, 0xf0),
            staff_space: Some(4.5),
            ..RenderConfig::default()
        },
        geometry: ChartGeometry {
            paper: Paper::Letter,
            margins: Some(PageMargins {
                top: 36.0,
                right: 30.0,
                bottom: 36.0,
                left: 30.0,
            }),
            system_spacing: Some(30.0),
        },
        typography: ChartTypography {
            chord_scale: 0.9,
            ..ChartTypography::default()
        },
        artwork: vec![
            PageArtwork::from_svg(
                "M0 0 H100 V3 H0 Z",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                ArtworkLayer::UnderMusic,
            )
            .expect("band path")
            .with_fill(Color::from_rgb8(0x2a, 0x55, 0x8c)),
        ],
    }
}

/// Page 1 of `chart` under `view`'s theme, as `(width, height, rgba8)`.
fn render_page_one(view: &mut ChartView, chart: &keyflow_proto::Chart) -> (u32, u32, Vec<u8>) {
    // Lay out (into a throwaway scene) to learn the page box.
    view.paint(
        &mut anyrender::Scene::new(),
        chart,
        KEY,
        816.0,
        SCALE,
        1.0,
        (0.0, 0.0),
        None,
    );
    let (x, y, w, h) = view.page(1).expect("a page");
    let k = engraver::renderer::points_to_px(SCALE);
    let (pw, ph) = ((w * k).ceil() as u32, (h * k).ceil() as u32);
    let mut renderer = VelloCpuImageRenderer::new(pw, ph);
    let mut buf = Vec::new();
    renderer.render_to_vec(
        |scene| {
            view.paint(scene, chart, KEY, f64::from(pw), SCALE, 1.0, (x, y), None);
        },
        &mut buf,
    );
    (pw, ph, buf)
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/chart_theme")
        .join(format!("{name}.png"))
}

fn write_png(path: &PathBuf, w: u32, h: u32, rgba: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("golden dir");
    let file = std::fs::File::create(path).expect("create golden");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

fn read_png(path: &PathBuf) -> (u32, u32, Vec<u8>) {
    let file = std::fs::File::open(path)
        .unwrap_or_else(|e| panic!("{}: {e} (run with KEYFLOW_UPDATE_GOLDEN=1)", path.display()));
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("png info");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("png frame");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

/// Compare against the golden, allowing anti-aliasing noise: a pixel
/// "differs" when any channel is off by more than 8, and at most 0.2% of
/// pixels may differ.
fn assert_golden(name: &str, (w, h, rgba): &(u32, u32, Vec<u8>)) {
    let path = golden_path(name);
    // `default` is the pre-theme capture; regenerating it from this code
    // would prove nothing, so the switch never touches it.
    if name != "default" && std::env::var_os("KEYFLOW_UPDATE_GOLDEN").is_some() {
        write_png(&path, *w, *h, rgba);
        return;
    }
    let (gw, gh, golden) = read_png(&path);
    assert_eq!((gw, gh), (*w, *h), "{name}: page size changed");
    let differing = rgba
        .chunks_exact(4)
        .zip(golden.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .count();
    let total = (*w * *h) as usize;
    if differing * 500 > total {
        let actual = path.with_extension("actual.png");
        write_png(&actual, *w, *h, rgba);
        panic!(
            "{name}: {differing} of {total} pixels differ from the golden; wrote {}",
            actual.display()
        );
    }
}

/// Mean luminance over the page, 0..1.
fn luminance((_, _, rgba): &(u32, u32, Vec<u8>)) -> f64 {
    let sum: f64 = rgba
        .chunks_exact(4)
        .map(|p| 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]))
        .sum();
    sum / (rgba.len() / 4) as f64 / 255.0
}

#[test]
fn default_theme_matches_the_pre_theme_golden() {
    let chart = chart();
    let mut view = ChartView::new().expect("view");
    let page = render_page_one(&mut view, &chart);
    assert_golden("default", &page);

    // An explicit default theme is the same thing.
    let mut themed = ChartView::new()
        .expect("view")
        .with_theme(ChartTheme::default());
    assert_eq!(render_page_one(&mut themed, &chart).2, page.2);
}

#[test]
fn two_themes_render_materially_differently() {
    let chart = chart();
    let default = render_page_one(&mut ChartView::new().expect("view"), &chart);
    let night = render_page_one(
        &mut ChartView::new().expect("view").with_theme(night()),
        &chart,
    );
    let blueprint = render_page_one(
        &mut ChartView::new().expect("view").with_theme(blueprint()),
        &chart,
    );
    assert_golden("night", &night);
    assert_golden("blueprint", &blueprint);

    // Geometry: A4 is narrower and taller than Letter.
    assert_eq!((default.0, default.1), (816, 1056));
    assert_eq!((night.0, night.1), (794, 1123));
    assert_eq!((blueprint.0, blueprint.1), (816, 1056));
    // Colour: both are dark pages, the default a light one.
    assert!(
        luminance(&default) > 0.85,
        "default {}",
        luminance(&default)
    );
    assert!(luminance(&night) < 0.25, "night {}", luminance(&night));
    assert!(
        luminance(&blueprint) < 0.35,
        "blueprint {}",
        luminance(&blueprint)
    );
    assert_ne!(night.2, blueprint.2);
}

#[test]
fn geometry_and_type_change_the_layout_and_colours_do_not() {
    let chart = chart();
    let mut view = ChartView::new().expect("view");
    view.paint(
        &mut anyrender::Scene::new(),
        &chart,
        KEY,
        816.0,
        1.0,
        1.0,
        (0.0, 0.0),
        None,
    );
    assert!(!view.needs_layout(KEY));
    let default_boxes = view.measure_boxes();

    // A colour-only theme repaints the cached layout.
    let mut recolour = ChartTheme::default();
    recolour.render = night().render;
    recolour.render.staff_space = None;
    view.set_theme(recolour);
    assert!(
        !view.needs_layout(KEY),
        "a colour change must not re-lay out"
    );

    // Geometry and type do re-lay out, and move the music.
    view.set_theme(night());
    assert!(view.needs_layout(KEY));
    view.paint(
        &mut anyrender::Scene::new(),
        &chart,
        KEY,
        816.0,
        1.0,
        1.0,
        (0.0, 0.0),
        None,
    );
    assert_ne!(view.measure_boxes(), default_boxes);

    // A shared style table is honoured (and not leaked per change).
    let style = Arc::new(engraver::MStyle::default());
    let mut styled = night();
    styled.typography.style = Some(Arc::clone(&style));
    view.set_theme(styled);
    assert!(view.needs_layout(KEY), "a new style table re-lays out");
    assert!(
        Arc::strong_count(&style) >= 2,
        "the view holds the shared style"
    );
}

#[test]
fn layout_runs_off_the_paint_thread() {
    let chart = chart();
    let mut view = ChartView::new().expect("view").with_theme(night());
    let worker = LayoutWorker::spawn().expect("worker");

    // Nothing laid out yet: a cached paint paints nothing, and says so.
    assert!(
        view.paint_cached(&mut anyrender::Scene::new(), 1.0, 1.0, (0.0, 0.0), None)
            .is_none()
    );

    worker.submit(view.layout_job(chart.clone(), KEY));
    let prepared = worker.wait().expect("a layout");
    assert_eq!(prepared.key(), KEY);
    assert!(view.apply_precomputed_layout(prepared));
    assert!(!view.needs_layout(KEY));
    let mut scene = anyrender::Scene::new();
    assert!(
        view.paint_cached(&mut scene, 1.0, 1.0, (0.0, 0.0), Some(1.0))
            .is_some()
    );
    assert!(!scene.commands.is_empty());

    // A layout made under an older geometry is refused, not painted.
    worker.submit(view.layout_job(chart, KEY + 1));
    view.set_theme(blueprint());
    let stale = worker.wait().expect("a layout");
    assert!(!view.apply_precomputed_layout(stale));
}

fn report(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    let at = |q: f64| samples[((q * n as f64).ceil() as usize).clamp(1, n) - 1];
    let mean = samples.iter().sum::<f64>() / n as f64;
    println!(
        "{{\"bench\":\"{label}\",\"n\":{n},\"p50_ms\":{:.4},\"p99_ms\":{:.4},\
         \"max_ms\":{:.4},\"mean_ms\":{mean:.4}}}",
        at(0.50),
        at(0.99),
        samples[n - 1]
    );
}

/// Themed paint timings, alongside `chart_paint_bench`'s default ones.
///
/// ```sh
/// cargo test --workspace --test chart_theme -- --ignored --nocapture
/// ```
#[test]
#[ignore = "benchmark — run explicitly with --ignored --nocapture"]
fn chart_paint_themed_bench() {
    const WARMUP: usize = 30;
    const FRAMES: usize = 600;
    let chart = chart();
    let mut scene = anyrender::Scene::new();

    // Warm paint under a full theme (recolour, artwork, aliases).
    let mut view = ChartView::new().expect("view").with_theme(night());
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
    report("paint_warm_night", samples);

    // A live colour edit every frame: set_theme + paint, no re-layout.
    let colours = [night().render, blueprint().render];
    let mut samples = Vec::with_capacity(FRAMES);
    for frame in 0..WARMUP + FRAMES {
        scene.reset();
        let mut theme = night();
        theme.render = colours[frame % 2].clone();
        theme.render.staff_space = night().render.staff_space;
        let start = Instant::now();
        view.set_theme(theme);
        let size = view.paint(
            &mut scene,
            &chart,
            1,
            816.0,
            2.0,
            1.0,
            (0.0, 0.0),
            Some(1.0),
        );
        let elapsed = start.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box((size, scene.commands.len()));
        if frame >= WARMUP {
            samples.push(elapsed);
        }
    }
    report("theme_colour_edit_paint", samples);

    // The paint thread while layouts run on the worker: every frame
    // submits a new layout (a chart edit per frame, the worst case) and
    // paints whatever layout it has, never waiting on one.
    let worker = LayoutWorker::spawn().expect("worker");
    let mut view = ChartView::new().expect("view").with_theme(night());
    worker.submit(view.layout_job(chart.clone(), 0));
    let first = worker.wait().expect("first layout");
    view.apply_precomputed_layout(first);
    let mut samples = Vec::with_capacity(FRAMES);
    let mut applied = 0usize;
    for frame in 0..WARMUP + FRAMES {
        scene.reset();
        worker.submit(view.layout_job(chart.clone(), frame as u64 + 1));
        let start = Instant::now();
        if let Some(prepared) = worker.poll() {
            applied += usize::from(view.apply_precomputed_layout(prepared));
        }
        let size = view.paint_cached(&mut scene, 2.0, 1.0, (0.0, 0.0), Some(1.0));
        let elapsed = start.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box((size, scene.commands.len()));
        if frame >= WARMUP {
            samples.push(elapsed);
        }
    }
    println!("{{\"bench\":\"paint_cached_layouts_applied\",\"count\":{applied}}}");
    report("paint_cached_during_background_relayout", samples);
}
