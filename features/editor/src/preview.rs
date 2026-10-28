//! The chart preview's model — everything a live preview does that is not
//! a DOM.
//!
//! Turning chart *source* into *pages* ([`engrave`]) and the camera over
//! them ([`fit_zoom`], [`reset_view`], [`step_zoom`], [`wheel_zoom`]) are
//! pure functions here, so the browser and a Blitz window draw the same
//! pages and move them the same way. What stays with the host is the
//! mounting: measuring its column (pass the width in), the pointer
//! events, and putting each [`Page::svg`] into the document — next to
//! [`font_face_css`], emitted once, because the pages reference the chart
//! fonts by name rather than carrying ~485 KB of them each.
//!
//! The site's `ChartPreview` and its static `Chart` engrave through
//! [`engrave`]; the debounced Dioxus hook over it is
//! `view::use_engraved_pages` (the `view` feature).

use std::sync::OnceLock;

use engraver::api::pipeline::{ChartPipeline, Paper, Preset, PresetOptions};
use engraver::style::MStyle;

/// Points → CSS pixels. Screen DPI over the typographic 72.
pub const DPI_SCALE: f64 = 96.0 / 72.0;

/// Layout width handed to the engine, in CSS px.
///
/// It decides how much music fits on a system in [`ChartShape::Inline`],
/// and is ignored in [`ChartShape::Page`], where paper size decides. One
/// value, so a chart cannot break differently depending on which caller
/// engraved it.
const LAYOUT_WIDTH_PX: f64 = 900.0;

/// Breathing room around a content-cropped snippet, in points.
const SNIPPET_PADDING: f64 = 6.0;

/// Zoom bounds, matching keyflow-ui's native chart viewports.
pub const ZOOM_MIN: f64 = 0.1;
/// The upper zoom bound.
pub const ZOOM_MAX: f64 = 8.0;
/// The zoom a preview opens at, and Reset returns to where it fits.
pub const DEFAULT_ZOOM: f64 = 0.75;
/// The pan a preview opens at, in CSS px from the stage's top left.
pub const DEFAULT_PAN: (f64, f64) = (24.0, 24.0);
/// Breathing room left around a page that has been fitted to its column.
pub const FIT_MARGIN_PX: f64 = 12.0;
/// Gap between pages laid side by side, in CSS px.
pub const PAGE_GAP_PX: f64 = 24.0;
/// One zoom-button step.
pub const ZOOM_STEP: f64 = 1.25;

/// How a chart is laid out and framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartShape {
    /// Continuous, cropped to the music, scaled to its container. For a
    /// chart inside prose.
    Inline,
    /// Paginated at true paper size (A4). For a working surface, where
    /// the point is to see the page you are about to print.
    Page,
}

/// One engraved page: its real size in CSS px, and its markup.
///
/// `svg` starts at `<svg` (no XML prolog) so it can be dropped into an
/// element's inner HTML, and references the chart fonts by family — see
/// [`font_face_css`].
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub width_px: f64,
    pub height_px: f64,
    pub svg: String,
}

/// A blank A4 sheet, for while the source does not lay out yet — still
/// typing the title, or a parse error. The page should never simply vanish
/// while someone is mid-thought.
#[must_use]
pub fn blank_page() -> Page {
    Page {
        width_px: 595.0 * DPI_SCALE,
        height_px: 842.0 * DPI_SCALE,
        svg: String::new(),
    }
}

/// The preview's pipeline: the shared fonts, with keyflow-ui's on-screen
/// style (`MStyle::new()`), built once per process.
///
/// Not [`ChartPipeline::shared`], which engraves with the lead-sheet
/// preset — the style the `.kf` fences use. The two engrave differently,
/// and the preview has always shown the on-screen one.
fn pipeline() -> Result<&'static ChartPipeline, String> {
    static PIPELINE: OnceLock<Result<ChartPipeline, String>> = OnceLock::new();
    PIPELINE
        .get_or_init(|| {
            let style: &'static MStyle = Box::leak(Box::new(MStyle::new()));
            ChartPipeline::with_style(style).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Lay a chart out and serialise it for the screen.
///
/// # Errors
///
/// The source is empty, does not parse, or lays out to nothing. A live
/// preview shows [`blank_page`] instead; a finished chart says what is
/// wrong.
pub fn engrave(source: &str, shape: ChartShape) -> Result<Vec<Page>, String> {
    if source.trim().is_empty() {
        return Err("Nothing to engrave yet.".to_string());
    }
    let chart = keyflow_lang::text::chart::parse_document(source)
        .map(|(chart, _doc)| chart)
        // Worded as the facade's `KeyflowSourceError::Text` says it, which
        // is what a static chart has always shown under a broken source.
        .map_err(|e| format!("text parse error: {e}"))?;
    let pipeline = pipeline()?;
    let preset = match shape {
        ChartShape::Inline => Preset::Snippet,
        ChartShape::Page => Preset::Page,
    };
    // Screen defaults, at the one layout width, on A4: the paper the
    // blank page and the downloads are.
    let options = PresetOptions::for_screen(LAYOUT_WIDTH_PX / DPI_SCALE, 1.0).with_paper(Paper::A4);
    let (mode, config) = ChartPipeline::resolve_preset(preset, options);
    let result = pipeline
        .engine()
        .layout_chart_with_config(&chart, &mode, &config);
    if result.pages.is_empty() {
        return Err("No chart layout available to export".to_string());
    }

    Ok(match shape {
        ChartShape::Inline => vec![Page {
            width_px: 0.0,
            height_px: 0.0,
            svg: inline_ready(&pipeline.export_svg_snippet(&result, SNIPPET_PADDING)),
        }],
        ChartShape::Page => {
            let pages: Vec<Page> = pipeline
                .export_svg_pages_sized(&result)
                .into_iter()
                .map(|(w_pt, h_pt, svg)| Page {
                    width_px: w_pt * DPI_SCALE,
                    height_px: h_pt * DPI_SCALE,
                    svg: inline_ready(&svg),
                })
                .collect();
            if pages.is_empty() {
                vec![blank_page()]
            } else {
                pages
            }
        }
    })
}

/// `@font-face` rules for the fonts the pages reference. Emit once per
/// document (a `<style>`), not once per chart.
///
/// # Errors
///
/// The font bundle could not be built.
pub fn font_face_css() -> Result<String, String> {
    Ok(pipeline()?.font_face_css())
}

/// Strip the XML prolog so the document can be inlined into HTML — inside
/// an element `<?xml …?>` renders as stray text above the chart.
fn inline_ready(svg: &str) -> String {
    match svg.find("<svg") {
        Some(at) => svg[at..].to_string(),
        None => svg.to_string(),
    }
}

/// The zoom at which a page `page_width_px` wide fits a column
/// `column_width_px` wide, less [`FIT_MARGIN_PX`] each side. `None` when
/// either is unknown (zero) or the column has no room at all.
///
/// The host measures the column — the browser its element, a Blitz
/// window its layout — and passes it in.
#[must_use]
pub fn fit_zoom(page_width_px: f64, column_width_px: f64) -> Option<f64> {
    if page_width_px <= 0.0 || column_width_px <= 0.0 {
        return None;
    }
    let usable = column_width_px - FIT_MARGIN_PX * 2.0;
    if usable <= 0.0 {
        return None;
    }
    Some((usable / page_width_px).clamp(ZOOM_MIN, ZOOM_MAX))
}

/// The first fit: the zoom and pan to switch to when the default zoom is
/// wider than the column, or `None` to keep the camera as it is. Only
/// ever downward — after the first fit the zoom belongs to the reader.
#[must_use]
pub fn first_fit(page_width_px: f64, column_width_px: f64, zoom: f64) -> Option<(f64, (f64, f64))> {
    let fit = fit_zoom(page_width_px, column_width_px)?;
    (fit < zoom).then_some((fit, (FIT_MARGIN_PX, FIT_MARGIN_PX)))
}

/// Reset: the default camera, or the fit when the default does not fit
/// the column (a phone, a narrow pane). `column_width_px` of `None` (not
/// measured) resets to the default.
#[must_use]
pub fn reset_view(page_width_px: Option<f64>, column_width_px: Option<f64>) -> (f64, (f64, f64)) {
    let fit = page_width_px
        .zip(column_width_px)
        .and_then(|(page, column)| fit_zoom(page, column));
    match fit {
        Some(f) if f < DEFAULT_ZOOM => (f, (FIT_MARGIN_PX, FIT_MARGIN_PX)),
        _ => (DEFAULT_ZOOM, DEFAULT_PAN),
    }
}

/// One zoom-button step in (`true`) or out, clamped to the bounds.
#[must_use]
pub fn step_zoom(zoom: f64, zoom_in: bool) -> f64 {
    if zoom_in {
        (zoom * ZOOM_STEP).min(ZOOM_MAX)
    } else {
        (zoom / ZOOM_STEP).max(ZOOM_MIN)
    }
}

/// A wheel tick: zoom by 5% toward (`delta_y < 0`) or away, anchored at
/// `at` (the pointer, in the stage's coordinates) so the chart grows
/// around what is under the pointer rather than the corner. Returns the
/// new `(zoom, pan)`.
#[must_use]
pub fn wheel_zoom(zoom: f64, pan: (f64, f64), at: (f64, f64), delta_y: f64) -> (f64, (f64, f64)) {
    let factor = if delta_y < 0.0 { 1.05 } else { 0.95 };
    let new = (zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    let change = new / zoom;
    (
        new,
        (
            at.0 - (at.0 - pan.0) * change,
            at.1 - (at.1 - pan.1) * change,
        ),
    )
}

/// The CSS transform that places the pages under the camera
/// (`transform-origin: 0 0`).
#[must_use]
pub fn camera_transform(zoom: f64, pan: (f64, f64)) -> String {
    format!(
        "transform: translate({}px, {}px) scale({zoom}); transform-origin: 0 0;",
        pan.0, pan.1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_BARS: &str = "VS: | 1 4 | 5 6- |\n";

    #[test]
    fn pages_are_a4_at_screen_size() {
        let pages = engrave(TWO_BARS, ChartShape::Page).unwrap();
        let (w, h) = (pages[0].width_px, pages[0].height_px);
        assert!((w - 595.0 * DPI_SCALE).abs() < 1.0 && (h - 842.0 * DPI_SCALE).abs() < 1.0);
        assert!(pages[0].svg.starts_with("<svg"));
        assert!(
            !pages[0].svg.contains("@font-face"),
            "on-screen pages link their fonts"
        );
    }

    #[test]
    fn an_empty_chart_is_an_error() {
        for shape in [ChartShape::Inline, ChartShape::Page] {
            assert!(engrave("  \n", shape).is_err());
        }
    }

    #[test]
    fn a_page_fits_a_narrow_column_only_downward() {
        // A phone: ~390px across, a page ~793px wide.
        let (zoom, pan) = first_fit(793.0, 390.0, DEFAULT_ZOOM).unwrap();
        assert!((zoom - (390.0 - 24.0) / 793.0).abs() < 1e-9);
        assert_eq!(pan, (FIT_MARGIN_PX, FIT_MARGIN_PX));
        // A desktop column already fits at the default: nothing changes.
        assert_eq!(first_fit(793.0, 1600.0, DEFAULT_ZOOM), None);
        // Unmeasured: nothing to fit to.
        assert_eq!(fit_zoom(793.0, 0.0), None);
    }

    #[test]
    fn reset_is_the_default_unless_that_does_not_fit() {
        assert_eq!(
            reset_view(Some(793.0), Some(1600.0)),
            (DEFAULT_ZOOM, DEFAULT_PAN)
        );
        assert_eq!(reset_view(Some(793.0), None), (DEFAULT_ZOOM, DEFAULT_PAN));
        assert!(reset_view(Some(793.0), Some(390.0)).0 < DEFAULT_ZOOM);
    }

    #[test]
    fn the_wheel_zooms_around_the_pointer() {
        let (zoom, pan) = wheel_zoom(1.0, (0.0, 0.0), (100.0, 100.0), -1.0);
        assert!((zoom - 1.05).abs() < 1e-9);
        // The point under the pointer stays under it.
        assert!(((100.0 - pan.0) / zoom - 100.0).abs() < 1e-9);
        assert_eq!(step_zoom(ZOOM_MAX, true), ZOOM_MAX);
        assert_eq!(step_zoom(ZOOM_MIN, false), ZOOM_MIN);
    }
}
