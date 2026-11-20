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
//!
//! ## Themes and the paint thread
//!
//! A [`ChartTheme`] (see [`ChartView::set_theme`]) decides the paper, the
//! ink, the geometry, the type and the page artwork. Its colours and
//! artwork apply at paint time; its geometry and type re-lay the chart
//! out, so they are part of the layout's cache key.
//!
//! [`ChartView::paint`] lays out synchronously when that key changes,
//! which is the simple path. A host that must never stall its paint
//! thread instead asks for a [`LayoutJob`] ([`ChartView::layout_job`]),
//! runs it elsewhere (a [`LayoutWorker`], or its own pool), hands the
//! result back with [`ChartView::apply_precomputed_layout`] — the same
//! seam keyflow-ui's `ChartLayoutManager::apply_precomputed_layout` is —
//! and paints with [`ChartView::paint_cached`], which never lays out: it
//! paints the last layout it has, in the current theme's colours, until
//! the new one arrives.

use std::sync::{Arc, OnceLock};

use anyrender::PaintScene;
use kurbo::{Affine, Rect, Stroke};
use peniko::{Color, Fill};

use crate::Chart;
use crate::api::pipeline::{ChartPipeline, Preset, PresetOptions};
use crate::engraver::layout::chart::cursor::{ChartCursor, CursorConfig};
use crate::engraver::layout::chart::playback::{PlaybackEvent, PlaybackTrack};
use crate::engraver::layout::chart::{
    ChartLayoutConfig, ChartLayoutEngine, ChartLayoutResult, LayoutMode,
};
use crate::engraver::renderer::cursor_renderer::render_cursor_commands;
use crate::engraver::renderer::scene_renderer::SceneRenderBuilder;
use crate::engraver::renderer::theme::{ArtworkLayer, ChartTheme, InkMap};
use crate::engraver::scene::SceneNode;
use crate::engraver::style::MStyle;
use keyflow_lang::text::bar_times::BarClock;

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

/// One page of a laid-out chart, recorded on its own
/// ([`ChartView::record_pages`]).
#[derive(Clone, Debug)]
pub struct PageRecording {
    /// Its number, 1-indexed, as the layout counts pages.
    pub number: u32,
    /// Where it is, in chart pixels at 100%.
    pub rect: Rect,
    /// What it draws, in the same pixels.
    pub commands: Vec<anyrender::recording::RenderCommand>,
}

/// One measure's box on the laid-out chart, in chart points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureBox {
    /// Global measure index (0-based, across the whole chart).
    pub measure: usize,
    /// Its left and right barlines.
    pub x0: f64,
    pub x1: f64,
    /// Its staff's top line, and the staff's height.
    pub staff_y: f64,
    pub staff_height: f64,
}

/// How a [`ChartView`] shows playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayheadStyle {
    /// MuseScore's: a translucent bar the height of the system on the
    /// symbol that is sounding, jumping to the next at its time
    /// (`layout::chart::playback`).
    #[default]
    Stepped,
    /// A wash over the bar with a thin line gliding across it
    /// ([`CursorConfig::playback`]).
    Smooth,
}

/// The stepped cursor's opacity, of 255: light enough that the slash and
/// the staff lines under it stay black, strong enough to find at a glance
/// from a music stand.
const STEPPED_ALPHA: u8 = 96;

/// A laid-out chart, cached until the content or the width class changes.
pub struct ChartView {
    /// The SAME facade keyflow-ui's own chart renderer builds — see the
    /// module doc. Independent of [`ChartPipeline::shared`] because that
    /// one defaults to the lead-sheet style; this one asks for keyflow-ui's
    /// own style explicitly, the way keyflow-ui's `ChartLayoutManager::new`
    /// does.
    pipeline: ChartPipeline,
    cursor: ChartCursor,
    /// The cursor's own config, before a theme recolours it.
    cursor_base: CursorConfig,
    /// How playback is shown: stepping from symbol to symbol, or the
    /// line gliding across the bar.
    playhead_style: PlayheadStyle,
    theme: ChartTheme,
    /// `theme.render` as a paint-time recolouring; `None` for the default
    /// look, which then paints exactly as laid out.
    ink_map: Option<InkMap>,
    cached: Option<Cached>,
    /// Moves on every change to what [`ChartView::record_pages`] records:
    /// a layout installed, a theme set.
    generation: u64,
}

/// keyflow-ui's own style, built once and shared by every view that has
/// no theme style of its own.
fn default_style() -> Arc<MStyle> {
    static STYLE: OnceLock<Arc<MStyle>> = OnceLock::new();
    Arc::clone(STYLE.get_or_init(|| Arc::new(MStyle::default())))
}

/// A layout to run off the paint thread: everything it needs, owned, so
/// it is `Send`. From [`ChartView::layout_job`]; its result goes back
/// through [`ChartView::apply_precomputed_layout`].
pub struct LayoutJob {
    engine: ChartLayoutEngine,
    chart: Chart,
    mode: LayoutMode,
    config: ChartLayoutConfig,
    key: u64,
    layout_key: u64,
}

impl LayoutJob {
    /// Lay the chart out. The expensive part; call it anywhere.
    #[must_use]
    pub fn run(self) -> PreparedLayout {
        let layout = self
            .engine
            .layout_chart_with_config(&self.chart, &self.mode, &self.config);
        let track = PlaybackTrack::new(&layout, &self.chart, BarClock::at(0.0));
        PreparedLayout {
            key: self.key,
            layout_key: self.layout_key,
            spatium: self.config.spatium,
            layout,
            track,
        }
    }

    /// The caller's chart key this job lays out.
    #[must_use]
    pub fn key(&self) -> u64 {
        self.key
    }
}

/// A finished [`LayoutJob`], ready for [`ChartView::apply_precomputed_layout`].
pub struct PreparedLayout {
    key: u64,
    layout_key: u64,
    spatium: f64,
    layout: ChartLayoutResult,
    /// The layout's rhythmic events in playing order, for the cursor.
    track: PlaybackTrack,
}

impl PreparedLayout {
    /// The caller's chart key it was laid out for.
    #[must_use]
    pub fn key(&self) -> u64 {
        self.key
    }
}

/// A background thread that runs [`LayoutJob`]s, newest first: a job
/// submitted while another is queued replaces it, so a burst of edits (or
/// theme tweaks) lays out once, for the last one. The thread ends when
/// the worker is dropped.
#[cfg(not(target_arch = "wasm32"))]
pub struct LayoutWorker {
    jobs: Option<std::sync::mpsc::Sender<LayoutJob>>,
    done: std::sync::mpsc::Receiver<PreparedLayout>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LayoutWorker {
    /// Start the worker thread.
    ///
    /// # Errors
    ///
    /// The OS refused to spawn the thread.
    pub fn spawn() -> std::io::Result<Self> {
        let (jobs, job_rx) = std::sync::mpsc::channel::<LayoutJob>();
        let (done_tx, done) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("chart-layout".into())
            .spawn(move || {
                while let Ok(mut job) = job_rx.recv() {
                    while let Ok(newer) = job_rx.try_recv() {
                        job = newer;
                    }
                    if done_tx.send(job.run()).is_err() {
                        return;
                    }
                }
            })?;
        Ok(Self {
            jobs: Some(jobs),
            done,
            thread: Some(thread),
        })
    }

    /// Queue `job`, superseding any job not yet started.
    pub fn submit(&self, job: LayoutJob) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// The newest finished layout since the last call, if any. Never
    /// blocks — what a paint callback calls each frame.
    #[must_use]
    pub fn poll(&self) -> Option<PreparedLayout> {
        self.done.try_iter().last()
    }

    /// Block until the next layout finishes (tests, export).
    #[must_use]
    pub fn wait(&self) -> Option<PreparedLayout> {
        self.done.recv().ok()
    }
}

/// In a browser there is one thread: a job submitted is laid out there and
/// then, and the next [`LayoutWorker::poll`] hands it over. The same
/// calls as the thread's, so a view is written once.
#[cfg(target_arch = "wasm32")]
pub struct LayoutWorker {
    done: std::sync::Mutex<Option<PreparedLayout>>,
}

#[cfg(target_arch = "wasm32")]
impl LayoutWorker {
    /// A worker on the page's one thread.
    ///
    /// # Errors
    ///
    /// None: the signature is the thread's.
    pub fn spawn() -> std::io::Result<Self> {
        Ok(Self {
            done: std::sync::Mutex::new(None),
        })
    }

    /// Lays `job` out now, in place of any layout not yet polled.
    pub fn submit(&self, job: LayoutJob) {
        let laid = job.run();
        *self
            .done
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(laid);
    }

    /// The newest finished layout since the last call, if any.
    #[must_use]
    pub fn poll(&self) -> Option<PreparedLayout> {
        self.done
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    /// The layout last submitted, if not yet polled.
    #[must_use]
    pub fn wait(&self) -> Option<PreparedLayout> {
        self.poll()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for LayoutWorker {
    fn drop(&mut self) {
        // Closing the queue ends the thread after its current job.
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Cached {
    /// The caller's chart key (its own hash — see [`ChartView::paint`]).
    /// The Page preset's `(LayoutMode, ChartLayoutConfig)` pair is a fixed
    /// physical page size, independent of the panel's width or the view's
    /// zoom — unlike the Responsive preset this used to use — so beyond
    /// the theme's own layout key nothing else is part of the cache key.
    key: u64,
    /// [`ChartTheme::layout_key`] of the theme it was laid out under.
    layout_key: u64,
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
    /// The layout's rhythmic events in playing order: what the stepped
    /// playback cursor sits on.
    track: PlaybackTrack,
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
            ChartPipeline::with_shared_style(default_style()).map_err(|e| e.to_string())?;
        Ok(Self {
            pipeline,
            cursor: ChartCursor::new(CursorConfig::playback()),
            cursor_base: CursorConfig::playback(),
            playhead_style: PlayheadStyle::default(),
            theme: ChartTheme::default(),
            ink_map: None,
            cached: None,
            generation: 0,
        })
    }

    /// A view whose cursor never draws — for a chart shown without a
    /// transport (a print preview, a shared read-only page).
    #[must_use]
    pub fn without_cursor(mut self) -> Self {
        self.cursor_base = CursorConfig {
            show_when_stopped: false,
            ..CursorConfig::playback()
        };
        self.apply_cursor_theme();
        self
    }

    /// This view, under `theme`.
    #[must_use]
    pub fn with_theme(mut self, theme: ChartTheme) -> Self {
        self.set_theme(theme);
        self
    }

    /// Switch to `theme`. Its colours and artwork show on the next paint;
    /// a change to its geometry or type also makes the layout stale (see
    /// [`ChartView::needs_layout`]), which [`ChartView::paint`] redoes
    /// inline and [`ChartView::paint_cached`] leaves to the host's
    /// background layout — painting the old layout, recoloured, meanwhile.
    pub fn set_theme(&mut self, theme: ChartTheme) {
        let same_style = match (&theme.typography.style, &self.theme.typography.style) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same_style {
            let style = theme.typography.style.clone().unwrap_or_else(default_style);
            // Only the shared font bundle can fail, and `new` already
            // built it; keep the old pipeline if it somehow does.
            if let Ok(pipeline) = ChartPipeline::with_shared_style(style) {
                self.pipeline = pipeline;
            }
        }
        self.ink_map = InkMap::from_render_config(&theme.render);
        self.theme = theme;
        self.apply_cursor_theme();
        self.generation = self.generation.wrapping_add(1);
    }

    /// What it would draw now, as a number that moves whenever that may
    /// have changed (a layout installed, a theme set): a host that keeps
    /// [`ChartView::record_pages`]'s recordings records again only then.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The current theme.
    #[must_use]
    pub fn theme(&self) -> &ChartTheme {
        &self.theme
    }

    fn apply_cursor_theme(&mut self) {
        let mut config = self.cursor_base.clone();
        if let Some(color) = self.theme.render.cursor_color {
            let c = color.to_rgba8();
            config.accent_color = [c.r, c.g, c.b, c.a];
        }
        self.cursor = ChartCursor::new(config);
    }

    /// The Page preset's layout parameters under the current theme.
    fn layout_params(&self) -> (LayoutMode, ChartLayoutConfig) {
        // `viewport_pt`/`zoom` are `PresetOptions` fields `Preset::Page`
        // never reads (`resolve_preset` picks the paper size and
        // `master_rhythm()` outright) — `for_screen`'s defaults are a
        // placeholder for them, not a real viewport.
        let geometry = &self.theme.geometry;
        let options = PresetOptions::for_screen(612.0, 1.0).with_paper(geometry.paper);
        let (mode, mut config) = ChartPipeline::resolve_preset(Preset::Page, options);
        if let Some(margins) = geometry.margins {
            config.margins = margins;
        }
        if let Some(spacing) = geometry.system_spacing {
            config.system_spacing = spacing;
        }
        if let Some(spatium) = self.theme.render.staff_space {
            config.spatium = spatium;
        }
        config.harmony_style.root_size *= self.theme.typography.chord_scale;
        (mode, config)
    }

    /// Whether `key` (the caller's chart key, as for [`ChartView::paint`])
    /// needs laying out under the current theme — what a host checks
    /// before asking for a [`ChartView::layout_job`].
    #[must_use]
    pub fn needs_layout(&self, key: u64) -> bool {
        let layout_key = self.theme.layout_key();
        self.cached
            .as_ref()
            .is_none_or(|c| c.key != key || c.layout_key != layout_key)
    }

    /// A [`LayoutJob`] for `chart` under the current theme, to run off the
    /// paint thread. `key` is the caller's chart key, as for
    /// [`ChartView::paint`].
    #[must_use]
    pub fn layout_job(&self, chart: Chart, key: u64) -> LayoutJob {
        let (mode, config) = self.layout_params();
        LayoutJob {
            engine: self.pipeline.engine().clone(),
            chart,
            mode,
            config,
            key,
            layout_key: self.theme.layout_key(),
        }
    }

    /// Install a layout finished off the paint thread. Returns `false`,
    /// and drops it, if the theme's geometry or type changed since the
    /// job was made — painting it would show the old page under the new
    /// theme; the host asks for a fresh job instead.
    pub fn apply_precomputed_layout(&mut self, prepared: PreparedLayout) -> bool {
        if prepared.layout_key != self.theme.layout_key() {
            return false;
        }
        self.cached = Some(Cached {
            key: prepared.key,
            layout_key: prepared.layout_key,
            layout: prepared.layout,
            spatium: prepared.spatium,
            track: prepared.track,
        });
        self.generation = self.generation.wrapping_add(1);
        true
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
        let _ = width_px; // the Page preset's size does not depend on the panel
        if self.needs_layout(key) {
            let (mode, config) = self.layout_params();
            let layout = self.pipeline.layout_with_config(chart, &mode, &config);
            let track = PlaybackTrack::new(&layout, chart, BarClock::at(0.0));
            self.cached = Some(Cached {
                key,
                layout_key: self.theme.layout_key(),
                layout,
                spatium: config.spatium,
                track,
            });
            self.generation = self.generation.wrapping_add(1);
        }
        self.paint_cached(scene, scale, zoom, scroll_pt, playhead_secs)
            .unwrap_or((0.0, 0.0))
    }

    /// Paint the layout already cached — never laying out, so it never
    /// stalls the paint thread — in the current theme. `None` (and nothing
    /// painted) before the first layout. Arguments and return value as for
    /// [`ChartView::paint`].
    pub fn paint_cached(
        &self,
        scene: &mut impl PaintScene,
        scale: f64,
        zoom: f64,
        scroll_pt: (f64, f64),
        playhead_secs: Option<f64>,
    ) -> Option<(f64, f64)> {
        let cached = self.cached.as_ref()?;
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
        let pages: Vec<Rect> = if cached.layout.pages.is_empty() {
            vec![Rect::new(
                0.0,
                0.0,
                cached.layout.total_width,
                cached.layout.total_height,
            )]
        } else {
            cached
                .layout
                .pages
                .iter()
                .map(|page| {
                    Rect::new(
                        page.x_offset,
                        page.y_offset,
                        page.x_offset + page.width,
                        page.y_offset + page.height,
                    )
                })
                .collect()
        };
        let paper = self.theme.render.background_color;
        for page in &pages {
            scene.fill(Fill::NonZero, transform, paper, None, page);
        }
        self.paint_artwork(scene, transform, &pages, ArtworkLayer::UnderMusic);

        // The pipeline's own font bundle; `configure_renderer` borrows it
        // and hands back a renderer that only lives as long as this call.
        // `spatium` MUST match the layout config's own — see the note on
        // `Cached::spatium` — the render-time default (10pt) is not it.
        let bundle = self.pipeline.fonts();
        // Artwork under the music sits on the paper filled above; the
        // layout's own white sheet would cover it, so skip that one.
        let host_paper = if self
            .theme
            .artwork
            .iter()
            .any(|a| a.layer == ArtworkLayer::UnderMusic)
        {
            pages.clone()
        } else {
            Vec::new()
        };
        let base = SceneRenderBuilder::new()
            .spatium(cached.spatium)
            .build()
            .with_ink_map(self.ink_map.clone())
            .with_host_paper(host_paper);
        let mut renderer = bundle.configure_renderer(base);
        for (from, to) in &self.theme.typography.font_aliases {
            renderer = renderer.with_font_alias(from.clone(), to.clone());
        }
        renderer.render_with_transform(scene, &cached.layout.scene, transform);

        self.paint_artwork(scene, transform, &pages, ArtworkLayer::OverMusic);

        if let Some(secs) = playhead_secs {
            self.paint_playhead_with(scene, cached, secs, transform);
        }

        Some((cached.layout.total_width, cached.layout.total_height))
    }

    /// The laid-out chart a page at a time, each its own recording in
    /// chart pixels at 100% ([`points_to_px`]`(1.0)` per point): its paper,
    /// its artwork and the music on it, as [`ChartView::paint_cached`]
    /// draws them at zoom 1 and no scroll. `None` before the first layout.
    ///
    /// What a host that keeps layers draws: a page whose recording is
    /// unchanged is drawn again without being sent again, and a pan or a
    /// zoom moves them all with one transform.
    ///
    /// The layout's root holds its elements flat; each goes to the page
    /// its `page` metadata names, or, without one, to the page its ink is
    /// on (the first, when it has none).
    #[must_use]
    pub fn record_pages(&self) -> Option<Vec<PageRecording>> {
        use crate::engraver::layout::chart::notation_renderer::scene_ink_bounds;
        let cached = self.cached.as_ref()?;
        let layout = &cached.layout;
        let numbered: Vec<(u32, Rect)> = if layout.pages.is_empty() {
            vec![(
                1,
                Rect::new(0.0, 0.0, layout.total_width, layout.total_height),
            )]
        } else {
            layout
                .pages
                .iter()
                .map(|p| {
                    (
                        p.number,
                        Rect::new(
                            p.x_offset,
                            p.y_offset,
                            p.x_offset + p.width,
                            p.y_offset + p.height,
                        ),
                    )
                })
                .collect()
        };
        let root = &layout.scene;
        let mut on: Vec<Vec<&SceneNode>> = vec![Vec::new(); numbered.len()];
        for child in &root.children {
            let by_number = child
                .metadata
                .get("page")
                .and_then(|n| n.parse::<u32>().ok())
                .and_then(|n| numbered.iter().position(|(number, _)| *number == n));
            let by_ink = || {
                let ink = scene_ink_bounds(child, root.transform)?;
                let centre = ink.center();
                numbered.iter().position(|(_, rect)| rect.contains(centre))
            };
            let at = by_number.or_else(by_ink).unwrap_or(0);
            if let Some(page) = on.get_mut(at) {
                page.push(child);
            }
        }
        let transform = Affine::scale(DPI_SCALE);
        let bundle = self.pipeline.fonts();
        let rects: Vec<Rect> = numbered.iter().map(|(_, r)| *r).collect();
        let under = self
            .theme
            .artwork
            .iter()
            .any(|a| a.layer == ArtworkLayer::UnderMusic);
        let paper = self.theme.render.background_color;
        let mut out = Vec::with_capacity(numbered.len());
        for (((number, rect), nodes), _) in numbered.iter().zip(&on).zip(&rects) {
            let mut scene = anyrender::Scene::new();
            scene.fill(Fill::NonZero, transform, paper, None, rect);
            self.paint_artwork(&mut scene, transform, &[*rect], ArtworkLayer::UnderMusic);
            let base = SceneRenderBuilder::new()
                .spatium(cached.spatium)
                .build()
                .with_ink_map(self.ink_map.clone())
                .with_host_paper(if under { vec![*rect] } else { Vec::new() });
            let mut renderer = bundle.configure_renderer(base);
            for (from, to) in &self.theme.typography.font_aliases {
                renderer = renderer.with_font_alias(from.clone(), to.clone());
            }
            if root.visible {
                for node in nodes {
                    renderer.render_with_transform(&mut scene, node, transform * root.transform);
                }
            }
            self.paint_artwork(&mut scene, transform, &[*rect], ArtworkLayer::OverMusic);
            out.push(PageRecording {
                number: *number,
                rect: transform.transform_rect_bbox(*rect),
                commands: scene.commands,
            });
        }
        Some(out)
    }

    /// The elements whose scene nodes `nodes` names ([`Selectable::node`],
    /// from [`ChartView::selectables`]) drawn again as themselves, every
    /// ink in `color` ([`InkMap::solid`]), in chart pixels at 100% as
    /// [`ChartView::record_pages`] draws them: a highlight laid over the
    /// pages, so a hover or a selection recolours an element without the
    /// page it is on being recorded again. `None` before the first layout;
    /// a path the layout no longer has is skipped.
    ///
    /// [`Selectable::node`]: crate::engraver::layout::chart::select::Selectable::node
    #[must_use]
    pub fn record_elements(
        &self,
        nodes: &[&[usize]],
        color: Color,
    ) -> Option<Vec<anyrender::recording::RenderCommand>> {
        use crate::engraver::layout::chart::select::node_at;
        let cached = self.cached.as_ref()?;
        let root = &cached.layout.scene;
        let mut scene = anyrender::Scene::new();
        if root.visible {
            let base = SceneRenderBuilder::new()
                .spatium(cached.spatium)
                .build()
                .with_ink_map(Some(InkMap::solid(color)));
            let mut renderer = self.pipeline.fonts().configure_renderer(base);
            for (from, to) in &self.theme.typography.font_aliases {
                renderer = renderer.with_font_alias(from.clone(), to.clone());
            }
            for path in nodes {
                if let Some((node, parent)) = node_at(root, path) {
                    renderer.render_with_transform(
                        &mut scene,
                        node,
                        Affine::scale(DPI_SCALE) * parent,
                    );
                }
            }
        }
        Some(scene.commands)
    }

    /// The playback cursor at `playhead_secs` (chart time) into `scene`,
    /// in chart pixels at 100%, as [`ChartView::paint_cached`] draws it:
    /// whether there was one to draw.
    pub fn paint_playhead(&self, scene: &mut impl PaintScene, playhead_secs: f64) -> bool {
        let Some(cached) = self.cached.as_ref() else {
            return false;
        };
        self.paint_playhead_with(scene, cached, playhead_secs, Affine::scale(DPI_SCALE))
    }

    fn paint_playhead_with(
        &self,
        scene: &mut impl PaintScene,
        cached: &Cached,
        playhead_secs: f64,
        transform: Affine,
    ) -> bool {
        match self.playhead_style {
            PlayheadStyle::Stepped => {
                let Some(event) = cached.track.at(playhead_secs) else {
                    return false;
                };
                let (x0, y0, x1, y1) = event.cursor;
                scene.fill(
                    Fill::NonZero,
                    transform,
                    self.playhead_colour(),
                    None,
                    &Rect::new(x0, y0, x1, y1),
                );
                true
            }
            PlayheadStyle::Smooth => {
                let Some(state) = self.cursor.compute_at_time(&cached.layout, playhead_secs) else {
                    return false;
                };
                // The SMuFL font, for a cursor that highlights a notehead
                // glyph (keyflow-ui's own cursor render passes the same).
                let font = self.pipeline.fonts().smufl_font();
                render_cursor_commands(scene, &state.commands, transform, Some(font));
                true
            }
        }
    }

    /// The stepped cursor's fill: the theme's `chart.cursor` (else the
    /// playback blue), translucent, so the symbol under it still reads.
    #[must_use]
    pub fn playhead_colour(&self) -> Color {
        let [r, g, b, _] = self.cursor.config.accent_color;
        Color::from_rgba8(r, g, b, STEPPED_ALPHA)
    }

    /// How playback is shown ([`PlayheadStyle::Stepped`] unless set).
    pub fn set_playhead_style(&mut self, style: PlayheadStyle) {
        self.playhead_style = style;
    }

    /// How playback is shown.
    #[must_use]
    pub fn playhead_style(&self) -> PlayheadStyle {
        self.playhead_style
    }

    /// The rhythmic event sounding at `playhead_secs` (chart time) and its
    /// place in playing order: what the stepped cursor sits on. The index
    /// changes exactly when the cursor moves, so a host draws the cursor
    /// again only then. `None` before the first layout and where there is
    /// no cursor (before bar 1, after the end, a bar not drawn).
    #[must_use]
    pub fn playhead_event(&self, playhead_secs: f64) -> Option<(usize, &PlaybackEvent)> {
        let track = &self.cached.as_ref()?.track;
        let index = track.index_at(playhead_secs)?;
        Some((index, &track.events()[index]))
    }

    /// The laid-out chart's events in playing order. `None` before the
    /// first layout.
    #[must_use]
    pub fn playback_track(&self) -> Option<&PlaybackTrack> {
        self.cached.as_ref().map(|c| &c.track)
    }

    /// Where the playback cursor at `playhead_secs` (chart time) is: its
    /// measure (0-indexed, global) and a box around it, in chart points
    /// (`x0, y0, x1, y1`: the staff and room above it for chord symbols),
    /// for a view that keeps the playhead in sight. `None` before the first
    /// layout or outside the chart's time.
    #[must_use]
    pub fn playhead_box(&self, playhead_secs: f64) -> Option<(usize, (f64, f64, f64, f64))> {
        let cached = self.cached.as_ref()?;
        if self.playhead_style == PlayheadStyle::Stepped {
            let event = cached.track.at(playhead_secs)?;
            let (x0, y0, x1, y1) = event.symbol;
            let h = y1 - y0;
            return Some((
                event.bar,
                (x0 - h, h.mul_add(-2.5, y0), x1 + h, h.mul_add(1.75, y0)),
            ));
        }
        let state = self.cursor.compute_at_time(&cached.layout, playhead_secs)?;
        let h = state.cursor_height;
        Some((
            state.measure,
            (
                state.cursor_x - h,
                h.mul_add(-2.5, state.cursor_y),
                state.cursor_x + h,
                h.mul_add(1.75, state.cursor_y),
            ),
        ))
    }

    /// The laid-out chart's size in points (`total_width`,
    /// `total_height`). `None` before the first layout.
    #[must_use]
    pub fn content_size(&self) -> Option<(f64, f64)> {
        self.cached
            .as_ref()
            .map(|c| (c.layout.total_width, c.layout.total_height))
    }

    /// The laid-out chart's scene, the root a [`Selectable::node`] path
    /// starts from. `None` before the first layout.
    ///
    /// [`Selectable::node`]: crate::engraver::layout::chart::select::Selectable::node
    #[must_use]
    pub fn layout_scene(&self) -> Option<&SceneNode> {
        self.cached.as_ref().map(|c| &c.layout.scene)
    }

    /// What can be selected on the laid-out chart, in reading order, its
    /// bounds in chart points (`layout::chart::select`). Empty before the
    /// first layout.
    #[must_use]
    pub fn selectables(&self) -> Vec<crate::engraver::layout::chart::select::Selectable> {
        self.cached
            .as_ref()
            .map(|c| crate::engraver::layout::chart::select::selectables(&c.layout))
            .unwrap_or_default()
    }

    fn paint_artwork(
        &self,
        scene: &mut impl PaintScene,
        transform: Affine,
        pages: &[Rect],
        layer: ArtworkLayer,
    ) {
        for art in self.theme.artwork.iter().filter(|a| a.layer == layer) {
            for page in pages {
                let path = art.placed(*page);
                if let Some(fill) = art.fill {
                    scene.fill(Fill::NonZero, transform, fill, None, &path);
                }
                if let Some((color, width)) = art.stroke {
                    scene.stroke(&Stroke::new(width), transform, color, None, &path);
                }
            }
        }
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

    /// Where the playhead is at `playhead_secs` (chart time): the page it
    /// is on, and the band of that page it covers — `(page, y, height)`,
    /// the cursor's top and height in chart points. For a panel that fits a
    /// page to its width and is shorter than the page, so it follows the
    /// song down the page as well as across the pages.
    /// `None` as for [`ChartView::page_number_at_time`].
    #[must_use]
    pub fn cursor_at_time(&self, playhead_secs: f64) -> Option<(u32, f64, f64)> {
        let cached = self.cached.as_ref()?;
        let state = self.cursor.compute_at_time(&cached.layout, playhead_secs)?;
        Some((state.page, state.cursor_y, state.cursor_height))
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

    /// Every measure's box, in chart points: what a caller needs to name
    /// a spot on the chart musically — this measure, this far through it,
    /// this high against its staff — and to find that spot again in a
    /// view laid out, zoomed or panned differently (a collaborator's
    /// pointer, say). In layout order; a measure that wraps is listed
    /// once per system it sits on. Empty before the first
    /// [`ChartView::paint`].
    #[must_use]
    pub fn measure_boxes(&self) -> Vec<MeasureBox> {
        let Some(cached) = self.cached.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<MeasureBox> = Vec::new();
        for beat in &cached.layout.beat_positions {
            let same = out.last().is_some_and(|m: &MeasureBox| {
                m.measure == beat.measure && m.staff_y == beat.staff_y
            });
            if !same {
                out.push(MeasureBox {
                    measure: beat.measure,
                    x0: beat.measure_x0,
                    x1: beat.measure_x1,
                    staff_y: beat.staff_y,
                    staff_height: beat.staff_height,
                });
            }
        }
        out
    }

    /// How many pages the chart laid out to.
    #[must_use]
    pub fn pages(&self) -> usize {
        self.cached.as_ref().map_or(0, |c| c.layout.pages.len())
    }
}
