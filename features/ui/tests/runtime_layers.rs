//! The chart as a runtime extension draws it (FastTrackStudios/architect#840).
//!
//! `ChartPaintView` returns layers: each page its own, the selection and
//! the playhead over them. In a process, the guest sends them through an
//! `architect_scene::ops::Sender` and the app's host decodes and composes
//! them (`Receiver`); linked, the painter composes the same layers.
//!
//! - **Real text.** A real chart's pages carry glyph runs in the engraver's
//!   own fonts (Leland, MuseJazz, FreeSans…), which cross once by hash. Sent,
//!   decoded and composed, then rasterised on the CPU, they are the same
//!   pixels as the engraver's whole-chart paint (`paint_cached`, what the
//!   view drew before it had layers).
//! - **Bytes.** What crosses for the first paint, a scroll, a selection and
//!   an edit to one bar, printed (`--nocapture`) and bounded.

#![cfg(feature = "paint-view")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::time::{Duration, Instant};

use architect::action::{ActionCaller, ActionList};
use architect_scene::ops::{Receiver, Sender};
use architect_workspace::paint::peniko::kurbo::Affine;
use architect_workspace::paint::{Layer, LayerKey, PaintView, Scene};
use architect_workspace::{
    Button, FrameClock, Geometry, HostData, KindId, PaneId, Placement, ViewCtx, ViewHost, ViewInput,
};
use keyflow::engraver::renderer::{self, ChartTheme, points_to_px};
use keyflow_ui::paint_view::{
    ChartSelection, ChartSnapshot, ChartView, HighlightColours, chart_mount,
};

const WIDTH: u32 = 900;
const HEIGHT: u32 = 1200;

fn chart_text(name: &str) -> String {
    let path = format!(
        "{}/../../apps/architect/vault/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// One bar's chord changed: the last chord before a slash gets another
/// root (`Emaj7 /` to `Amaj7 /`).
fn edited(text: &str) -> String {
    let at = text.rfind(" / ").expect("a chord before a slash");
    let start = text[..at].rfind([' ', '|', '\n']).map_or(0, |i| i + 1);
    let root = text[start..].chars().next().unwrap();
    let other = if root == 'A' { 'D' } else { 'A' };
    let mut out = text.to_owned();
    out.replace_range(start..start + root.len_utf8(), &other.to_string());
    out
}

struct Pane {
    view: Box<dyn PaintView>,
    ctx: ViewCtx,
    data: HostData<ChartSnapshot>,
    selection: HostData<ChartSelection>,
}

fn pane(text: &str) -> Pane {
    let chart = keyflow::parse(text).unwrap();
    let data = HostData::new(ChartSnapshot {
        chart,
        revision: 1,
        playhead_secs: None,
        theme: Some(ChartTheme::default()),
        view: ChartView::default(),
        input: None,
    });
    let mount = chart_mount(data.clone());
    let ctx = ViewCtx::new(
        PaneId::new("chart"),
        KindId::paint(),
        (ViewHost::Blitz, Placement::Pane),
        (ActionList::new(), ActionCaller::app()),
        FrameClock::new(Duration::from_millis(4), || Duration::ZERO),
    )
    .with_extras(mount.mount.host_extras())
    .with_geometry(Geometry {
        width: WIDTH as f32,
        height: HEIGHT as f32,
        scale: 1.0,
    });
    let view = mount.mount.make_view(&ctx);
    Pane {
        view,
        ctx,
        data,
        selection: mount.selection,
    }
}

impl Pane {
    /// Its layers once its layout (off its thread) has landed.
    fn settled(&mut self) -> Vec<(LayerKey, Layer)> {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let layers = self.view.layers(&self.ctx).expect("a view of layers");
            if !self.view.animating(&self.ctx) && !layers.is_empty() {
                return layers;
            }
            assert!(Instant::now() < deadline, "the chart never laid out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn raster(scene: &Scene) -> Vec<u8> {
    anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |painter| {
            use anyrender::PaintScene as _;
            painter.fill(
                architect_workspace::paint::peniko::Fill::NonZero,
                Affine::IDENTITY,
                architect_workspace::paint::peniko::Color::from_rgb8(128, 128, 128),
                None,
                &architect_workspace::paint::peniko::kurbo::Rect::new(
                    0.0,
                    0.0,
                    f64::from(WIDTH),
                    f64::from(HEIGHT),
                ),
            );
            painter.append_scene(scene.clone(), Affine::IDENTITY);
        },
        WIDTH,
        HEIGHT,
    )
}

/// The share of pixels that differ by more than a little in any channel.
fn differing(a: &[u8], b: &[u8]) -> f64 {
    let pixels = a.len() / 4;
    let off = a
        .chunks(4)
        .zip(b.chunks(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 24))
        .count();
    off as f64 / pixels as f64
}

fn glyph_runs(layers: &[(LayerKey, Layer)]) -> usize {
    layers
        .iter()
        .flat_map(|(_, l)| l.commands.iter())
        .filter(|c| matches!(c, anyrender::recording::RenderCommand::GlyphRun(_)))
        .count()
}

/// The engraver's whole chart as the view painted it before it had
/// layers, framed as `layers` were (fit to the width, no scroll).
fn whole_chart(text: &str) -> Scene {
    let chart = keyflow::parse(text).unwrap();
    let mut view = renderer::ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let mut scene = Scene::new();
    // Laid out, then painted at the fit-to-width zoom the pane uses.
    let _ = view.paint(
        &mut scene,
        &chart,
        1,
        f64::from(WIDTH),
        1.0,
        1.0,
        (0.0, 0.0),
        None,
    );
    let page_w = view.page(1).unwrap().2;
    let zoom = (f64::from(WIDTH) / (page_w * points_to_px(1.0))).min(1.0);
    let mut scene = Scene::new();
    view.paint_cached(&mut scene, 1.0, zoom, (0.0, 0.0), None);
    scene
}

fn measure(name: &str) {
    let text = chart_text(name);
    let mut pane = pane(&text);
    let mut sender = Sender::default();
    let mut receiver = Receiver::new();
    let mut send = |layers: &[(LayerKey, Layer)], receiver: &mut Receiver| -> usize {
        let batch = sender.frame(layers).unwrap_or_default();
        if !batch.is_empty() {
            receiver.apply(&batch).unwrap();
        }
        batch.len()
    };

    // First paint: every page, its fonts and its fragments.
    let layers = pane.settled();
    let pages = layers.iter().filter(|(k, _)| *k < (1 << 32)).count();
    let runs = glyph_runs(&layers);
    // The fonts those runs draw with: each crosses once.
    let mut fonts: Vec<(u64, usize)> = Vec::new();
    for c in layers.iter().flat_map(|(_, l)| l.commands.iter()) {
        if let anyrender::recording::RenderCommand::GlyphRun(g) = c {
            // By content: the engraver makes a new blob for every run,
            // and the encoder names a font by its bytes' hash.
            let blob = &g.font_data.data;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(blob.data(), &mut hasher);
            let id = std::hash::Hasher::finish(&hasher);
            if !fonts.iter().any(|(seen, _)| *seen == id) {
                fonts.push((id, blob.len()));
            }
        }
    }
    let font_bytes: usize = fonts.iter().map(|(_, n)| n).sum();
    assert!(runs > 0, "{name}: no text drawn as glyph runs");
    let first = send(&layers, &mut receiver);

    // Composed by the host from what crossed, against the engraver's own.
    let mut from_host = Scene::new();
    receiver.layers().compose(&mut from_host, Affine::IDENTITY);
    let mut linked = Scene::new();
    let mut held = architect_scene::Layers::new();
    for (key, layer) in &layers {
        held.set(*key, layer.clone());
    }
    held.compose(&mut linked, Affine::IDENTITY);
    let (host, local, engraved) = (
        raster(&from_host),
        raster(&linked),
        raster(&whole_chart(&text)),
    );
    let same = differing(&host, &local);
    let as_before = differing(&host, &engraved);
    assert!(
        same == 0.0,
        "{name}: the host's composition differs from the linked one ({same})"
    );
    assert!(
        as_before < 0.002,
        "{name}: the layers differ from the whole chart in {:.3}% of pixels",
        as_before * 100.0
    );
    if std::env::var_os("KEYFLOW_SHOTS").is_some() {
        let out =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/runtime-layers");
        std::fs::create_dir_all(&out).unwrap();
        let file = out.join(format!("{}.png", name.replace(['/', ' '], "_")));
        let mut encoder = png::Encoder::new(std::fs::File::create(&file).unwrap(), WIDTH, HEIGHT);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&host)
            .unwrap();
    }

    // A scroll: the same pages at another transform. The Page preset
    // lays pages side by side, so a long chart scrolls across.
    pane.data.update(|s| s.view.scroll.0 += 240.0);
    let layers = pane.settled();
    let scroll = send(&layers, &mut receiver);

    // A selection: a click on the first chord.
    let fit = {
        let mut view = renderer::ChartView::new().unwrap();
        view.set_theme(ChartTheme::default());
        let mut s = Scene::new();
        let chart = keyflow::parse(&text).unwrap();
        let _ = view.paint(
            &mut s,
            &chart,
            1,
            f64::from(WIDTH),
            1.0,
            1.0,
            (0.0, 0.0),
            None,
        );
        let page_w = view.page(1).unwrap().2;
        let mut chords = view
            .selectables()
            .into_iter()
            .filter(|s| s.key.starts_with("chord"));
        let chord = chords.next().expect("a chord to select");
        let other = chords.next().expect("a second chord to hover");
        (
            (f64::from(WIDTH) / (page_w * points_to_px(1.0))).min(1.0),
            chord,
            other,
        )
    };
    pane.data.update(|s| s.view.scroll = (0.0, 0.0));
    let _ = send(&pane.settled(), &mut receiver);
    let plain = composed(&receiver);
    // The colours the view draws them in: with no theme tokens given, its
    // fallbacks, made to read on the default paper.
    let colours = HighlightColours::from_tokens(
        |_| None,
        architect_workspace::paint::peniko::Color::WHITE,
        architect_workspace::paint::peniko::Color::BLACK,
    );
    let rgb = |c: architect_workspace::paint::peniko::Color| {
        let c = c.to_rgba8();
        [c.r, c.g, c.b]
    };
    let (zoom, chord, other) = fit;
    let to_pane = |p: kurbo::Point| {
        (
            (p.x * points_to_px(1.0) * zoom) as f32,
            (p.y * points_to_px(1.0) * zoom) as f32,
        )
    };
    let at = to_pane(chord.bounds.center());
    for input in [
        ViewInput::PointerDown {
            x: at.0,
            y: at.1,
            button: Button::Primary,
            mods: vec![],
        },
        ViewInput::PointerUp {
            x: at.0,
            y: at.1,
            button: Button::Primary,
            mods: vec![],
        },
    ] {
        pane.view.input(&pane.ctx, &input);
    }
    let selected = pane.selection.read(|s| s.keys.clone());
    assert_eq!(
        selected,
        vec![chord.key.clone()],
        "{name}: the click selected"
    );
    let select = send(&pane.settled(), &mut receiver);
    recoloured(name, &plain, &receiver, rgb(colours.selection));
    let selected = composed(&receiver);

    // A hover: the pointer onto another chord, which is drawn again as
    // itself in the hover colour, in a layer of its own; then a move
    // within it (nothing), then off onto the margin (the layer goes).
    let mut hover_move =
        |pane: &mut Pane, p: (f32, f32), receiver: &mut Receiver| -> (usize, Duration) {
            let started = Instant::now();
            pane.view.input(
                &pane.ctx,
                &ViewInput::PointerMove {
                    x: p.0,
                    y: p.1,
                    mods: vec![],
                },
            );
            let layers = pane.view.layers(&pane.ctx).expect("a view of layers");
            let bytes = send(&layers, receiver);
            (bytes, started.elapsed())
        };
    let o = other.bounds.center();
    let (hover_on, hover_time) = hover_move(&mut pane, to_pane(o), &mut receiver);
    assert_eq!(
        pane.selection.read(|s| s.hover.clone()),
        Some(other.key.clone()),
        "{name}: the pointer hovers the second chord"
    );
    recoloured(name, &selected, &receiver, rgb(colours.hover));
    let (hover_within, within_time) = hover_move(
        &mut pane,
        to_pane(kurbo::Point::new(o.x + 0.3, o.y)),
        &mut receiver,
    );
    let (hover_off, _) = hover_move(&mut pane, (2.0, 2.0), &mut receiver);
    assert_eq!(
        pane.selection.read(|s| s.hover.clone()),
        None,
        "{name}: the margin is nothing"
    );

    // An edit to one bar: laid out again, only the pages that changed cross.
    pane.data.update(|s| {
        s.chart = keyflow::parse(&edited(&text)).unwrap();
        s.revision += 1;
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let edit = loop {
        let layers = pane.settled();
        let bytes = send(&layers, &mut receiver);
        if bytes > 0 {
            break bytes;
        }
        assert!(Instant::now() < deadline, "the edit never laid out");
        std::thread::sleep(Duration::from_millis(5));
    };

    println!(
        "{name}: {pages} page(s), {runs} glyph runs in {} fonts ({font_bytes} bytes of them); bytes sent: \
         first paint {first} ({} past the fonts), scroll {scroll}, selection {select}, \
         hover onto a chord {hover_on} in {hover_time:?}, a move within it {hover_within} in {within_time:?}, \
         hover off {hover_off}, one bar edited {edit}; pixels off the whole chart {:.4}%",
        fonts.len(),
        first.saturating_sub(font_bytes),
        as_before * 100.0
    );
    assert!(hover_on > 0, "{name}: the hover drew nothing");
    assert_eq!(
        hover_within, 0,
        "{name}: a move within one element sent bytes"
    );
    assert!(
        hover_on < first / 20,
        "{name}: a hover sent {hover_on} of {first}: it re-sent the pages"
    );
    assert!(scroll > 0, "{name}: the scroll moved nothing");
    // Each page moves on its own (the layer tree is flat): a transform
    // a page, never its content.
    assert!(scroll < 80 * pages, "{name}: a scroll sent {scroll} bytes");
    assert!(
        select < first / 20,
        "{name}: a selection sent {select} of {first}"
    );
}

/// What the host composes from what crossed, rasterised.
fn composed(receiver: &Receiver) -> Vec<u8> {
    let mut scene = Scene::new();
    receiver.layers().compose(&mut scene, Affine::IDENTITY);
    raster(&scene)
}

/// One element highlighted as the host composes it: its ink in `rgb`
/// (the view's own colours for it), and only its
/// ink: the pixels in that colour are few and do not fill the box around
/// them, so it is the element recoloured, not a box laid over it.
fn recoloured(name: &str, before: &[u8], receiver: &Receiver, rgb: [u8; 3]) {
    let pixels = composed(receiver);
    if std::env::var_os("KEYFLOW_SHOTS").is_some() {
        let out =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/runtime-layers");
        std::fs::create_dir_all(&out).unwrap();
        let file = out.join(format!(
            "{}-{:02x}{:02x}{:02x}.png",
            name.replace(['/', ' '], "_"),
            rgb[0],
            rgb[1],
            rgb[2]
        ));
        let mut encoder = png::Encoder::new(std::fs::File::create(&file).unwrap(), WIDTH, HEIGHT);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    }
    // What the highlight changed, and what of that is its colour.
    let close = |p: &[u8]| p.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) < 30);
    let inked: Vec<(u32, u32)> = pixels
        .chunks(4)
        .zip(before.chunks(4))
        .enumerate()
        .filter(|(_, (p, q))| p != q && close(p))
        .map(|(i, _)| (i as u32 % WIDTH, i as u32 / WIDTH))
        .collect();
    assert!(inked.len() > 10, "{name}: nothing drawn in {rgb:?}");
    let (x0, x1) = (
        inked.iter().map(|p| p.0).min().unwrap(),
        inked.iter().map(|p| p.0).max().unwrap(),
    );
    let (y0, y1) = (
        inked.iter().map(|p| p.1).min().unwrap(),
        inked.iter().map(|p| p.1).max().unwrap(),
    );
    let area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as usize;
    assert!(
        area < 80 * 40,
        "{name}: the {rgb:?} pixels span {}x{}: more than one element",
        x1 - x0 + 1,
        y1 - y0 + 1
    );
    assert!(
        inked.len() * 10 < area * 7,
        "{name}: {} of the {area} pixels around it are {rgb:?}: a box, not ink",
        inked.len()
    );
}

#[test]
fn a_one_page_chart_crosses_once_and_draws_as_the_engraver_does() {
    measure("Charts/COLDPLAY - YELLOW.kf");
}

#[test]
fn a_long_chart_resends_only_the_pages_an_edit_changed() {
    measure("JT Recital/02 Drive To LA.kf");
}
