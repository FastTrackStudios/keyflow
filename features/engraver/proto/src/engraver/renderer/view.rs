//! `ChartView`: a laid-out, cached chart plus its playback cursor, ready to
//! paint into any `anyrender::PaintScene` — no window, no wgpu pipeline.
//!
//! The embeddable seam a host app's chart panel wraps: a Blitz custom
//! widget on desktop/iOS, a `vello_hybrid` canvas painter on the web. Both
//! call the same `paint`, so the chart reads identically everywhere —
//! which is the point of rendering it live rather than shipping an image.
//!
//! Goes through [`ChartPipeline`] for the layout — the SAME facade
//! keyflow-ui's own chart renderer calls (`crates/keyflow/keyflow-ui/src/
//! chart_renderer.rs`), with [`Preset::Page`] (Letter, page offsets on —
//! keyflow-ui's own default: `ChartLayoutManager::new`'s `paper:
//! Paper::Letter`, `last_preview_mode: PreviewMode::Page`) and the SAME
//! style (`MStyle::new()`/`default()` — keyflow-ui's own doc comment: it
//! and the CLI render with `new()`, `editor-keyflow` alone uses the
//! lead-sheet preset). Using the pipeline's own `resolve_preset` rather
//! than rebuilding the `(LayoutMode, ChartLayoutConfig)` pair by hand is
//! what keeps this from drifting out of step with keyflow's own rendering
//! the way a hand-copied table eventually would — see the pipeline's own
//! doc comment on why it exists (the CLI and the app used to each keep
//! their own copy of this table, and two of the three disagreed).
//!
//! Owns the layout cache and nothing about the surface, the input, or the
//! transport: `paint` takes a pan offset and a zoom the caller drives (a
//! drag, a wheel or pinch — the Page preset's own layout is a fixed
//! physical page size independent of the panel's width, exactly the
//! document a PDF viewer pans and zooms over), and a playhead already
//! converted to the chart's own timeline (its 0 is the first real measure
//! — see the note on `playhead_secs`). Re-lays-out only when the chart
//! itself changes; a pan or a zoom alone is a paint of what is already
//! there, at a new transform.

use anyrender::PaintScene;
use kurbo::{Affine, Rect};
use peniko::{Color, Fill};

use crate::Chart;
use crate::api::pipeline::{ChartPipeline, Paper, Preset, PresetOptions};
use crate::api::style::leak_default_style;
use crate::engraver::layout::chart::cursor::{ChartCursor, CursorConfig};
use crate::engraver::layout::chart::ChartLayoutResult;
use crate::engraver::renderer::cursor_renderer::render_cursor_commands;
use crate::engraver::renderer::scene_renderer::SceneRenderBuilder;

/// 96 CSS px per 72pt inch — the engraver's own scale between points (its
/// layout unit) and pixels. Matches keyflow-ui's `DPI_SCALE`.
const DPI_SCALE: f64 = 96.0 / 72.0;

/// Device pixels per chart point at `scale` (the window's own device
/// scale) — what a caller converts a screen-pixel drag or a zoom-to-cursor
/// offset by, to stay in the same units [`ChartView::paint`]'s `scroll_pt`
/// and `zoom` use. `zoom` is the view's own multiplier on top of this —
/// multiply it in separately (`points_to_px(scale) * zoom`), since it
/// changes far more often than `scale` does.
#[must_use]
pub fn points_to_px(scale: f64) -> f64 {
    DPI_SCALE * scale
}

/// A laid-out chart, cached until the content or the width class changes.
pub struct ChartView {
    /// The SAME facade keyflow-ui's own chart renderer builds — see the
    /// module doc. Independent of [`ChartPipeline::shared`] because that
    /// one defaults to the lead-sheet style; this one asks for keyflow-ui's
    /// own style explicitly, the way keyflow-ui's `ChartLayoutManager::new`
    /// does.
    pipeline: ChartPipeline,
    cursor: ChartCursor,
    cached: Option<Cached>,
}

struct Cached {
    /// The caller's chart key (its own hash — see [`ChartView::paint`]).
    /// The Page preset's `(LayoutMode, ChartLayoutConfig)` pair is a fixed
    /// physical page size, independent of the panel's width or the view's
    /// zoom — unlike the Responsive preset this used to use — so nothing
    /// else needs to be part of the cache key.
    key: u64,
    layout: ChartLayoutResult,
    /// The layout config's own staff-space unit, in points — what the
    /// RENDERER has to be told to match, or every SMuFL glyph (noteheads,
    /// slashes, clefs — anything sized in staff spaces rather than an
    /// absolute point size) comes out the wrong size relative to the
    /// barlines and text the layout placed around it. Varies by
    /// [`Breakpoint`] (`responsive_for`'s own table), so this is read off
    /// the SAME config the layout used, never a constant — see
    /// keyflow-ui's `chart_renderer.rs`, which is the reference this
    /// followed.
    spatium: f64,
}

impl ChartView {
    /// Build a view over keyflow's own chart pipeline
    /// ([`ChartPipeline::with_style`], keyflow-ui's own style) — the fonts
    /// are ~2 MB embedded and are loaded once for the whole process (the
    /// pipeline shares [`crate::engraver::fonts::ChartFontBundle::shared`]),
    /// not once per chart panel.
    ///
    /// # Errors
    ///
    /// The font bundle failed to build.
    pub fn new() -> Result<Self, String> {
        let pipeline =
            ChartPipeline::with_style(leak_default_style()).map_err(|e| e.to_string())?;
        Ok(Self {
            pipeline,
            cursor: ChartCursor::new(CursorConfig::playback()),
            cached: None,
        })
    }

    /// A view whose cursor never draws — for a chart shown without a
    /// transport (a print preview, a shared read-only page).
    #[must_use]
    pub fn without_cursor(mut self) -> Self {
        self.cursor = ChartCursor::new(CursorConfig {
            show_when_stopped: false,
            ..CursorConfig::playback()
        });
        self
    }

    /// Re-lay-out `chart` (the Page preset — a fixed Letter page, taking
    /// neither `width_px` nor `zoom`: those two pan and scale the VIEW over
    /// it, the way a PDF viewer zooms a page rather than re-flowing it) if
    /// `key` changed since the last call, then paint it — and, when
    /// `playhead_secs` is given, the cursor at that time — into `scene`.
    /// `width_px`/`scale` are still taken (a caller measures its panel in
    /// pixels, not points) but only used to convert `scroll_pt`/`zoom` into
    /// the paint transform, never the layout.
    ///
    /// `key` identifies the chart's content: its own hash, a song id plus a
    /// revision counter, whatever the caller already has cheaply — this
    /// never hashes the chart itself, so it is the caller's to get right.
    ///
    /// `scroll_pt` is a pan offset in the chart's own units (points, the
    /// SAME units `total_width`/`total_height` are in) — zoom-independent,
    /// so a caller can change `zoom` without also rescaling its pan.
    ///
    /// `playhead_secs` is on the CHART's own timeline, whose 0 is the
    /// first real measure — not the DAW's project timeline, whose 0 is
    /// earlier (before any count-in). A caller driving this from a DAW
    /// transport subtracts the SONGSTART marker's position first, or the
    /// cursor never reaches the count-in header at all (it lies at
    /// NEGATIVE chart time) and is offset by the count-in's length
    /// everywhere else.
    ///
    /// Returns the laid-out content size in points (`total_width`,
    /// `total_height` — un-scaled, so a caller can clamp `scroll_pt`
    /// itself without knowing `DPI_SCALE`).
    #[expect(clippy::too_many_arguments, reason = "a view, a pan and a zoom")]
    pub fn paint(
        &mut self,
        scene: &mut impl PaintScene,
        chart: &Chart,
        key: u64,
        width_px: f64,
        scale: f64,
        zoom: f64,
        scroll_pt: (f64, f64),
        playhead_secs: Option<f64>,
    ) -> (f64, f64) {
        let _ = (width_px, scale); // the Page preset's size does not depend on the panel
        let stale = self.cached.as_ref().is_none_or(|c| c.key != key);
        if stale {
            // `viewport_pt`/`zoom` are `PresetOptions` fields `Preset::Page`
            // never reads (`resolve_preset` picks the paper size and
            // `master_rhythm()` outright) — `for_screen`'s defaults are a
            // placeholder for them, not a real viewport.
            let options = PresetOptions::for_screen(612.0, 1.0).with_paper(Paper::Letter);
            let (mode, config) = ChartPipeline::resolve_preset(Preset::Page, options);
            let spatium = config.spatium;
            let layout = self.pipeline.layout_with_config(chart, &mode, &config);
            self.cached = Some(Cached { key, layout, spatium });
        }
        let Some(cached) = &self.cached else {
            return (0.0, 0.0);
        };
        // Points → device pixels, THEN pan: `scroll_pt` is in the chart's
        // own units so it stays put in content terms as `zoom` changes; a
        // pixel-space scroll would have needed rescaling on every zoom.
        let k = DPI_SCALE * scale * zoom;
        let transform =
            Affine::scale(k).then_translate((-scroll_pt.0 * k, -scroll_pt.1 * k).into());

        // Each page, behind everything: sheet music reads on paper, not on
        // the host app's own background — and drawn PER PAGE, with the
        // gap between them left as the panel's own background, so a
        // multi-page chart reads as separate sheets rather than one long
        // continuous roll. `pages` is always populated for the Page
        // preset; a Responsive/Snippet layout (nothing currently asks for
        // one, but `ChartView` does not rule it out) has none, so this
        // falls back to the whole scene box as one sheet.
        let page_color = Color::from_rgba8(0xff, 0xff, 0xff, 0xff);
        if cached.layout.pages.is_empty() {
            let page = Rect::new(0.0, 0.0, cached.layout.total_width, cached.layout.total_height);
            scene.fill(Fill::NonZero, transform, page_color, None, &page);
        } else {
            for page in &cached.layout.pages {
                let rect = Rect::new(
                    page.x_offset,
                    page.y_offset,
                    page.x_offset + page.width,
                    page.y_offset + page.height,
                );
                scene.fill(Fill::NonZero, transform, page_color, None, &rect);
            }
        }

        // The pipeline's own font bundle; `configure_renderer` borrows it
        // and hands back a renderer that only lives as long as this call.
        // `spatium` MUST match the layout config's own — see the note on
        // `Cached::spatium` — the render-time default (10pt) is not it.
        let bundle = self.pipeline.fonts();
        let base = SceneRenderBuilder::new().spatium(cached.spatium).build();
        let mut renderer = bundle.configure_renderer(base);
        renderer.render_with_transform(scene, &cached.layout.scene, transform);

        if let Some(secs) = playhead_secs
            && let Some(state) = self.cursor.compute_at_time(&cached.layout, secs)
        {
            // The SMuFL font, for a cursor that highlights a notehead
            // glyph — keyflow-ui's own cursor render passes the same.
            let font = bundle.smufl_font();
            render_cursor_commands(scene, &state.commands, transform, Some(font));
        }

        (cached.layout.total_width, cached.layout.total_height)
    }

    /// Where the playback cursor sits vertically, in chart points (the
    /// same units [`ChartView::paint`]'s `scroll_pt` and its returned
    /// content size are in) — for a caller keeping it in view (auto-scroll)
    /// or converting it to its own pixel space itself. `None` before the
    /// first `paint` (nothing laid out yet) or when `playhead_secs` falls
    /// outside the chart's range. `playhead_secs` is chart time — see
    /// [`ChartView::paint`]'s note on the count-in offset.
    ///
    /// Reads the cache as it stood after the LAST `paint` — a caller
    /// computing this before that frame's `paint` is one frame behind on a
    /// width or content change, which self-corrects the next frame.
    #[must_use]
    pub fn cursor_y_pt(&self, playhead_secs: f64) -> Option<f64> {
        let cached = self.cached.as_ref()?;
        let state = self.cursor.compute_at_time(&cached.layout, playhead_secs)?;
        Some(state.cursor_y)
    }

    /// The page the playhead is on at `playhead_secs` (chart time),
    /// 1-indexed as the layout counts them — for a panel showing ONE page
    /// at a time, which then asks [`ChartView::page`] where it is.
    ///
    /// By the cursor's own page number, not by a coordinate: the Page
    /// preset lays pages out side by side, so every page shares one `y`
    /// and a lookup by position finds the last page whatever the time.
    /// `None` before the first [`ChartView::paint`], or when the time falls
    /// outside the chart (before the downbeat, after the end).
    #[must_use]
    pub fn page_number_at_time(&self, playhead_secs: f64) -> Option<u32> {
        let cached = self.cached.as_ref()?;
        let state = self.cursor.compute_at_time(&cached.layout, playhead_secs)?;
        Some(state.page)
    }

    /// Page `number` (1-indexed, as the layout counts them), as
    /// `(x, y, width, height)` in chart points; past the end, the last.
    /// A panel's zoom is its size over the page's, and its scroll is the
    /// page's corner.
    /// `None` before the first [`ChartView::paint`], or for a layout with
    /// no pages (a continuous one).
    #[must_use]
    pub fn page(&self, number: u32) -> Option<(f64, f64, f64, f64)> {
        let pages = &self.cached.as_ref()?.layout.pages;
        let page = pages
            .iter()
            .find(|page| page.number == number)
            .or_else(|| pages.last())?;
        Some((page.x_offset, page.y_offset, page.width, page.height))
    }

    /// How many pages the chart laid out to.
    #[must_use]
    pub fn pages(&self) -> usize {
        self.cached.as_ref().map_or(0, |c| c.layout.pages.len())
    }
}
