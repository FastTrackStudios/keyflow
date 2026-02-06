//! Chart Layout Manager
//!
//! Manages the chart layout and rendering pipeline for desktop use.
//! Ported from the web app's renderer.rs, adapted for native rendering
//! (no WASM canvas methods — produces `vello::Scene` for the app to render).

use engraver_proto::engraver::fonts::ChartFontBundle;
use engraver_proto::engraver::layout::chart::{
    ChartLayoutConfig, ChartLayoutEngine, ChartLayoutResult, LayoutMode,
};
use engraver_proto::engraver::renderer::scene_renderer::SceneRenderBuilder;
use engraver_proto::engraver::style::MStyle;
use keyflow_proto::Chart;
use kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};
use vello::Scene;

/// Screen DPI for rendering.
const SCREEN_DPI: f64 = 96.0;
/// Points per inch (typographical standard).
const POINTS_PER_INCH: f64 = 72.0;
/// DPI scaling factor: converts points to screen pixels.
pub const DPI_SCALE: f64 = SCREEN_DPI / POINTS_PER_INCH;

/// A4 page dimensions in points.
const A4_WIDTH: f64 = 595.0;
const A4_HEIGHT: f64 = 842.0;

/// Chart layout and rendering engine for desktop.
///
/// Manages fonts, layout engine, and Vello scene rendering.
/// Produces `vello::Scene` objects that the app renders to its WGPU surface.
///
/// # Caching Strategy
///
/// Three cache levels to avoid redundant work:
/// 1. **Parse cache** (`cached_chart`): Keyed by source text hash. Avoids re-parsing
///    (~10-20ms) when only the layout mode or viewport changes.
/// 2. **Layout cache** (`layout_result`): Keyed by (source, snippet_mode) hash. Avoids
///    re-layout (~440-500ms) when the same chart is re-rendered.
/// 3. **Scene cache** (`cached_vello_scene`): Built once per layout change via
///    `render_with_transform` at identity. Per-frame rendering uses `Scene::append`
///    with the viewport transform for instant pan/zoom (~1-2ms vs ~67ms).
pub struct ChartLayoutManager {
    /// Font bundle (single source of truth for all chart fonts).
    font_bundle: ChartFontBundle,
    /// Layout engine.
    layout_engine: ChartLayoutEngine,
    /// Cached layout result.
    layout_result: Option<ChartLayoutResult>,
    /// Last layout hash — covers (source, snippet_mode) for layout invalidation.
    last_chart_hash: u64,
    /// Cached vello Scene — rebuilt only when layout changes.
    cached_vello_scene: Option<Scene>,
    /// Cached parsed chart — rebuilt only when source text changes.
    cached_chart: Option<Chart>,
    /// Hash of just the source text (for parse cache invalidation).
    last_source_hash: u64,
    /// Whether the last layout was in snippet mode (affects fit-to-width calculation).
    last_snippet_mode: bool,
}

impl ChartLayoutManager {
    /// Create a new chart layout manager with embedded fonts.
    pub fn new() -> Result<Self, String> {
        let font_bundle = ChartFontBundle::new()?;

        let style = Box::leak(Box::new(MStyle::new()));
        let layout_engine = font_bundle.create_layout_engine(style);

        Ok(Self {
            font_bundle,
            layout_engine,
            layout_result: None,
            last_chart_hash: 0,
            cached_vello_scene: None,
            cached_chart: None,
            last_source_hash: 0,
            last_snippet_mode: false,
        })
    }

    /// Parse source and layout the chart, using caches at each level.
    ///
    /// This is the main entry point for the layout effect. It:
    /// 1. Checks the layout hash — if (source, snippet_mode) haven't changed, returns false
    /// 2. Checks the parse cache — if source hasn't changed, reuses the cached Chart
    /// 3. Runs the layout engine and caches the result
    ///
    /// Returns `Ok(true)` if the layout was updated, `Ok(false)` if skipped (no change).
    pub fn parse_and_layout(
        &mut self,
        source: &str,
        viewport_width: f64,
        snippet_mode: bool,
    ) -> Result<bool, String> {
        // Check layout hash — skip everything if nothing changed
        let chart_hash = self.compute_chart_hash(source, snippet_mode);
        if self.layout_result.is_some() && chart_hash == self.last_chart_hash {
            return Ok(false);
        }

        // Check parse cache — only re-parse if source text changed
        let source_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            source.hash(&mut hasher);
            hasher.finish()
        };

        if self.cached_chart.is_none() || source_hash != self.last_source_hash {
            let chart = keyflow::parse(source).map_err(|e| format!("{}", e))?;
            self.cached_chart = Some(chart);
            self.last_source_hash = source_hash;
        }

        // Layout using the cached chart
        let chart = self.cached_chart.as_ref().unwrap();
        let (mode, config) = if snippet_mode {
            let config = ChartLayoutConfig::snippet().with_page_offsets(true);
            let mode = LayoutMode::Snippet {
                page_width: viewport_width / DPI_SCALE,
            };
            (mode, config)
        } else {
            let config = ChartLayoutConfig::master_rhythm().with_page_offsets(true);
            let mode = LayoutMode::Paginated {
                page_width: A4_WIDTH,
                page_height: A4_HEIGHT,
            };
            (mode, config)
        };

        let result = self
            .layout_engine
            .layout_chart_with_config(chart, &mode, &config);

        self.layout_result = Some(result);
        self.last_chart_hash = chart_hash;
        self.last_snippet_mode = snippet_mode;
        self.cached_vello_scene = None; // Force scene rebuild on next render

        Ok(true)
    }

    /// Layout a chart with a specified mode.
    ///
    /// # Arguments
    /// * `chart` - The parsed chart to layout
    /// * `source` - The original source text (used for cache invalidation)
    /// * `viewport_width` - Width of the viewport in CSS pixels
    /// * `snippet_mode` - If true, use snippet mode. If false, use A4 paginated.
    pub fn layout_chart_with_mode(
        &mut self,
        chart: &Chart,
        source: &str,
        viewport_width: f64,
        snippet_mode: bool,
    ) {
        let chart_hash = self.compute_chart_hash(source, snippet_mode);

        // Skip if already laid out with same content
        if self.layout_result.is_some() && chart_hash == self.last_chart_hash {
            return;
        }

        let (mode, config) = if snippet_mode {
            let config = ChartLayoutConfig::snippet().with_page_offsets(true);
            let mode = LayoutMode::Snippet {
                page_width: viewport_width / DPI_SCALE,
            };
            (mode, config)
        } else {
            let config = ChartLayoutConfig::master_rhythm().with_page_offsets(true);
            let mode = LayoutMode::Paginated {
                page_width: A4_WIDTH,
                page_height: A4_HEIGHT,
            };
            (mode, config)
        };

        let result = self
            .layout_engine
            .layout_chart_with_config(chart, &mode, &config);

        self.layout_result = Some(result);
        self.last_chart_hash = chart_hash;
        self.cached_vello_scene = None; // Force scene rebuild on next render
    }

    /// Render the chart to a Vello scene.
    ///
    /// Uses a cached vello Scene to avoid re-walking the scene graph every frame.
    /// The cached scene is built once at identity transform when the layout changes,
    /// then composited into the output scene with `Scene::append(cached, transform)`
    /// for instant pan/zoom (~1-2ms vs ~67ms for full re-render).
    pub fn render_to_scene(
        &mut self,
        scene: &mut Scene,
        width: f64,
        height: f64,
        transform: Affine,
    ) {
        // Fill background (gray workspace) — varies with viewport size, always drawn fresh
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgb8(55, 65, 81),
            None,
            &Rect::new(0.0, 0.0, width, height),
        );

        if self.layout_result.is_some() {
            // Build the cached scene if it doesn't exist (first frame after layout change)
            if self.cached_vello_scene.is_none() {
                let mut cached = Scene::new();
                let base_renderer = SceneRenderBuilder::new().spatium(5.0).build();
                let mut renderer = self.font_bundle.configure_renderer(base_renderer);
                renderer.render_with_transform(
                    &mut cached,
                    &self.layout_result.as_ref().unwrap().scene,
                    Affine::IDENTITY,
                );
                self.cached_vello_scene = Some(cached);
            }

            // Append cached scene with the viewport transform (fast path)
            if let Some(ref cached) = self.cached_vello_scene {
                scene.append(cached, Some(transform));
            }
        }
    }

    /// Get the cached layout result.
    pub fn layout_result(&self) -> Option<&ChartLayoutResult> {
        self.layout_result.as_ref()
    }

    /// Get the current layout hash (for change detection).
    pub fn last_chart_hash(&self) -> u64 {
        self.last_chart_hash
    }

    /// Get the content dimensions in points (with 20pt padding).
    pub fn get_content_dimensions(&self) -> Option<(f64, f64)> {
        self.layout_result.as_ref().map(|layout| {
            let bounds = layout.scene.compute_bounds();
            if bounds.width() > 0.0 && bounds.height() > 0.0 {
                (bounds.x1 + 20.0, bounds.y1 + 20.0)
            } else {
                (A4_WIDTH, A4_HEIGHT)
            }
        })
    }

    /// Compute the base scale factor to fit the chart content within the viewport.
    ///
    /// Returns a scale that converts from layout points to physical pixels such that
    /// the chart content fits horizontally within the viewport with padding.
    ///
    /// - **Paginated mode**: fits a single A4 page width (595pt)
    /// - **Snippet mode**: fits the actual scene content width
    pub fn fit_to_width_scale(&self, viewport_width_physical: f64, device_pixel_ratio: f64) -> f64 {
        let padding_physical = 40.0 * device_pixel_ratio; // 20 CSS px padding on each side
        let available = viewport_width_physical - padding_physical;
        if available <= 0.0 {
            return DPI_SCALE * device_pixel_ratio;
        }

        // In paginated mode, fit to the single page width (not the full multi-page scene).
        // In snippet mode, fit to the actual content width.
        let content_width = if self.last_snippet_mode {
            // Snippet: use actual scene bounds
            self.get_content_dimensions()
                .map(|(w, _)| w)
                .unwrap_or(A4_WIDTH)
        } else {
            // Paginated: use the first page width, or A4 default
            self.layout_result
                .as_ref()
                .and_then(|layout| layout.pages.first().map(|p| p.width))
                .unwrap_or(A4_WIDTH)
        };

        let scale = available / content_width;
        // Clamp: don't scale up beyond natural DPI×dpr (1:1 point-to-pixel mapping)
        scale.min(DPI_SCALE * device_pixel_ratio)
    }

    /// Compute a hash of the chart source and layout mode for cache invalidation.
    fn compute_chart_hash(&self, source: &str, snippet_mode: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        snippet_mode.hash(&mut hasher);
        hasher.finish()
    }
}
