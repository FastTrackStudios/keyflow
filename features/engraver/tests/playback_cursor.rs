//! The stepped playback cursor, rendered: a translucent bar the height of
//! the system on the symbol sounding (`layout::chart::playback`).
//!
//! Two stills mid-song, on light paper and on dark, against goldens in
//! `tests/golden/playback_cursor/` (regenerate with
//! `KEYFLOW_UPDATE_GOLDEN=1` and look at them), and what a still cannot
//! show: that the cursor is the only thing a moment of playback changes
//! on the page, and that it does not move inside a symbol.

#![cfg(feature = "paint")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use anyrender::ImageRenderer as _;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use engraver::renderer::{ChartTheme, ChartView, PlayheadStyle, RenderConfig, points_to_px};
use peniko::Color;

/// Mixed durations, a tie, bars of rest, a repeat with two endings and a
/// bar of 3/4, at 120 bpm (a quarter is half a second).
const CHART: &str = "Cursor - Test\n#C 120bpm 4/4\n\nVS 9\nC\nG_1\nC_2 G_4 ~ G_8 F_8\nr1\nr1\nAm_1\n%\n!T3/4 F_2 G_4\nC_1\n\nCH 4\n|: C_1 | [1] G_2. r4 :| [2] F_1 | C_1\n";
const KEY: u64 = 11;

fn chart() -> keyflow_proto::Chart {
    keyflow_lang::text::chart::parse_chart(CHART).expect("the chart parses")
}

fn dark() -> ChartTheme {
    ChartTheme {
        render: RenderConfig {
            background_color: Color::from_rgb8(0x12, 0x14, 0x1a),
            foreground_color: Color::from_rgb8(0xf2, 0xee, 0xe2),
            staff_color: Color::from_rgb8(0x8a, 0x93, 0xa6),
            ..RenderConfig::default()
        },
        ..ChartTheme::default()
    }
}

/// The top of page 1 under `view`, with the cursor at `secs`:
/// `(width, height, rgba8)`.
fn still(view: &mut ChartView, secs: Option<f64>) -> (u32, u32, Vec<u8>) {
    let chart = chart();
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
    let (x, y, w, h) = view.page(1).expect("a page");
    let k = points_to_px(1.0);
    // The chart is three systems: the top two fifths of the page.
    let (pw, ph) = ((w * k).ceil() as u32, (h * k * 0.42).ceil() as u32);
    let mut renderer = VelloCpuImageRenderer::new(pw, ph);
    let mut buf = Vec::new();
    renderer.render_to_vec(
        |scene| {
            view.paint(scene, &chart, KEY, f64::from(pw), 1.0, 1.0, (x, y), secs);
        },
        &mut buf,
    );
    (pw, ph, buf)
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/playback_cursor")
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

/// Against the golden, allowing anti-aliasing noise (a channel off by
/// more than 8 in at most 0.2% of pixels).
fn assert_golden(name: &str, (w, h, rgba): &(u32, u32, Vec<u8>)) {
    let path = golden_path(name);
    if std::env::var_os("KEYFLOW_UPDATE_GOLDEN").is_some() {
        write_png(&path, *w, *h, rgba);
        return;
    }
    let (gw, gh, golden) = read_png(&path);
    assert_eq!((gw, gh), (*w, *h), "{name}: size changed");
    let differing = differing(rgba, &golden).count();
    if differing * 500 > (*w * *h) as usize {
        let actual = path.with_extension("actual.png");
        write_png(&actual, *w, *h, rgba);
        panic!(
            "{name}: {differing} pixels differ from the golden; wrote {}",
            actual.display()
        );
    }
}

/// The indices of the pixels that differ between two images.
fn differing<'a>(a: &'a [u8], b: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .enumerate()
        .filter(|(_, (a, b))| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .map(|(i, _)| i)
}

#[test]
fn mid_song_on_light_paper() {
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    // 5.6 s: bar 3's tied eighth (the tie's continuation, beat 4).
    assert_golden("tied-eighth-light", &still(&mut view, Some(5.6)));
}

#[test]
fn mid_song_on_dark_paper() {
    let mut view = ChartView::new().unwrap();
    view.set_theme(dark());
    // 10.9 s: held on bar 6's whole-bar slash, nearly a beat into it.
    assert_golden("held-whole-dark", &still(&mut view, Some(10.9)));
}

#[test]
fn the_cursor_is_one_blue_bar_over_the_symbol_and_nothing_else_moves() {
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let plain = still(&mut view, None);
    let playing = still(&mut view, Some(5.1));
    let (w, _, _) = plain;

    let (_, event) = view.playhead_event(5.1).expect("an event at 5.1 s");
    assert_eq!((event.bar, event.beat), (2, 2.0), "bar 3, beat 3");
    let (page_x, page_y, _, _) = view.page(1).unwrap();
    let k = points_to_px(1.0);
    let (x0, y0, x1, y1) = event.cursor;
    let inside = |i: usize| {
        let (px, py) = ((i as u32 % w) as f64, (i as u32 / w) as f64);
        px >= ((x0 - page_x) * k).floor() - 1.0
            && px <= ((x1 - page_x) * k).ceil() + 1.0
            && py >= ((y0 - page_y) * k).floor() - 1.0
            && py <= ((y1 - page_y) * k).ceil() + 1.0
    };
    let changed: Vec<usize> = differing(&plain.2, &playing.2).collect();
    assert!(!changed.is_empty(), "a cursor is drawn");
    assert!(
        changed.iter().all(|&i| inside(i)),
        "playback changed pixels outside the cursor's box"
    );
    // Most of the box is tinted (the black symbol under it stays black),
    // and what is tinted is blue on the paper.
    let area = ((x1 - x0) * k * (y1 - y0) * k) as usize;
    assert!(changed.len() * 10 > area * 6, "{} of {area}", changed.len());
    let blue = changed
        .iter()
        .filter(|&&i| {
            let p = &playing.2[i * 4..i * 4 + 4];
            p[2] > p[0].saturating_add(20) && p[2] >= p[1]
        })
        .count();
    assert!(blue * 10 > changed.len() * 8, "{blue} of {}", changed.len());

    // The same symbol a moment later: the same picture, to the pixel.
    let later = still(&mut view, Some(5.4));
    assert_eq!(differing(&playing.2, &later.2).count(), 0, "it glided");
    // The next symbol: it has moved.
    let next = still(&mut view, Some(5.5));
    assert!(differing(&playing.2, &next.2).count() > 0);
}

#[test]
fn the_smooth_line_is_still_there_behind_the_setting() {
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    assert_eq!(view.playhead_style(), PlayheadStyle::Stepped, "the default");
    let stepped = still(&mut view, Some(2.5));
    view.set_playhead_style(PlayheadStyle::Smooth);
    let a = still(&mut view, Some(2.5));
    let b = still(&mut view, Some(2.9));
    assert!(differing(&stepped.2, &a.2).count() > 0);
    assert!(
        differing(&a.2, &b.2).count() > 0,
        "the line glides in a bar"
    );
}

#[test]
fn there_is_no_cursor_before_bar_one_or_after_the_end() {
    let mut view = ChartView::new().unwrap();
    view.set_theme(ChartTheme::default());
    let plain = still(&mut view, None);
    for secs in [-1.0, 1.0e6] {
        let at = still(&mut view, Some(secs));
        assert_eq!(differing(&plain.2, &at.2).count(), 0, "at {secs}");
        assert!(view.playhead_event(secs).is_none());
    }
}
