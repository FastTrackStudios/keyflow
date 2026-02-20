//! Chart Layout Manager
//!
//! Manages the chart layout and rendering pipeline for desktop use.
//! Ported from the web app's renderer.rs, adapted for native rendering
//! (no WASM canvas methods — produces `vello::Scene` for the app to render).

use crate::signals::{PageMeta, SystemMeta};
use anyrender::PaintScene;
use engraver_proto::engraver::fonts::ChartFontBundle;
use engraver_proto::engraver::layout::chart::cursor::{
    ChartCursor, CursorState, HighlightCommand, Rgba,
};
use engraver_proto::engraver::layout::chart::{
    ChartLayoutConfig, ChartLayoutEngine, ChartLayoutResult, LayoutMode,
};
use engraver_proto::engraver::renderer::cursor_renderer::render_cursor_commands;
use engraver_proto::engraver::renderer::scene_renderer::SceneRenderBuilder;
use engraver_proto::engraver::scene::node::{metadata_keys, SceneNode};
use engraver_proto::engraver::style::MStyle;
use keyflow_proto::{Chart, ChartPosition};
use kurbo::{Affine, Point, Rect};
use vello::peniko::{Color, Compose, Fill};

/// Screen DPI for rendering.
const SCREEN_DPI: f64 = 96.0;
/// Points per inch (typographical standard).
const POINTS_PER_INCH: f64 = 72.0;
/// DPI scaling factor: converts points to screen pixels.
pub const DPI_SCALE: f64 = SCREEN_DPI / POINTS_PER_INCH;

/// A4 page dimensions in points.
const A4_WIDTH: f64 = 595.0;
const A4_HEIGHT: f64 = 842.0;

/// Convert a time signature denominator to ticks per beat.
///
/// Uses standard 480 PPQ (ticks per quarter note):
/// - denominator 2 (half note) = 960 ticks
/// - denominator 4 (quarter note) = 480 ticks
/// - denominator 8 (eighth note) = 240 ticks
fn ticks_per_beat_for_denom(denominator: u8) -> i64 {
    match denominator {
        2 => 960,
        4 => 480,
        8 => 240,
        _ => 480,
    }
}

/// Result of a scene graph hit-test.
///
/// Contains the musical position metadata from the deepest matching node,
/// plus the resolved absolute tick from the `BeatPosition` array.
#[derive(Debug, Clone)]
pub struct SceneHitResult {
    /// The musical position stored on the hit node.
    pub position: ChartPosition,
    /// Absolute tick resolved from the BeatPosition array.
    pub absolute_tick: i64,
    /// World-space bounding box of the hit node.
    pub bounds: Rect,
    /// Element type (e.g., "chord", "slash", "rest").
    pub element_type: Option<String>,
}

/// Walk a SceneNode tree and collect all nodes whose world-space bounds
/// contain `point` and that carry `ChartPosition` metadata.
///
/// Results are ordered deepest-first (leaf nodes before parents), so
/// the first result is the most specific hit.
fn hit_test_scene_recursive<'a>(
    node: &'a SceneNode,
    point: Point,
    parent_transform: Affine,
    results: &mut Vec<(&'a SceneNode, Affine)>,
) {
    if !node.visible {
        return;
    }

    let world_transform = parent_transform * node.transform;

    // Recurse into children first (depth-first, so deeper nodes come first)
    for child in &node.children {
        hit_test_scene_recursive(child, point, world_transform, results);
    }

    // Check if this node has chart position metadata and its bounds contain the point
    if node.has_metadata(metadata_keys::CHART_POSITION) {
        let world_bounds = world_transform.transform_rect_bbox(node.bounds);
        // Expand bounds slightly for easier clicking (music symbols can be small)
        let padded = world_bounds.inflate(2.0, 4.0);
        if padded.contains(point) {
            results.push((node, world_transform));
        }
    }
}

/// Chart layout and rendering engine for desktop.
///
/// Manages fonts, layout engine, and Vello scene rendering.
/// Produces `vello::Scene` objects that the app renders to its WGPU surface.
///
/// # Caching Strategy
///
/// Two cache levels to avoid redundant work:
/// 1. **Parse cache** (`cached_chart`): Keyed by source text hash. Avoids re-parsing
///    (~10-20ms) when only the layout mode or viewport changes.
/// 2. **Layout cache** (`layout_result`): Keyed by (source, snippet_mode) hash. Avoids
///    re-layout (~440-500ms) when the same chart is re-rendered.
///
/// Rendering uses the `PaintScene` trait abstraction (via anyrender) so the
/// chart rendering pipeline is backend-agnostic.
pub struct ChartLayoutManager {
    /// Font bundle (single source of truth for all chart fonts).
    font_bundle: ChartFontBundle,
    /// Layout engine.
    layout_engine: ChartLayoutEngine,
    /// Cached layout result.
    layout_result: Option<ChartLayoutResult>,
    /// Last layout hash — covers (source, snippet_mode) for layout invalidation.
    last_chart_hash: u64,
    /// Cached parsed chart — rebuilt only when source text changes.
    cached_chart: Option<Chart>,
    /// Hash of just the source text (for parse cache invalidation).
    last_source_hash: u64,
    /// Whether the last layout was in snippet mode (affects fit-to-width calculation).
    last_snippet_mode: bool,
    /// Renderer-agnostic cursor for computing highlight commands.
    cursor: ChartCursor,
    /// Last computed cursor state (cached to avoid recomputing every frame when tick hasn't changed).
    cached_cursor_state: Option<CursorState>,
    /// The tick value used to compute the cached cursor state.
    cached_cursor_tick: i64,
}

impl ChartLayoutManager {
    /// Render only the static chart layer (background + chart glyphs).
    ///
    /// This should be used when cursor/hover overlays are rendered separately
    /// to avoid rebuilding static content on every transport tick.
    pub fn render_static_layer_to_scene(
        &mut self,
        scene: &mut impl PaintScene,
        width: f64,
        height: f64,
        offset: Affine,
        transform: Affine,
    ) {
        // Fill background (gray workspace) — varies with viewport size.
        scene.fill(
            Fill::NonZero,
            offset,
            Color::from_rgb8(55, 65, 81),
            None,
            &Rect::new(0.0, 0.0, width, height),
        );

        if self.layout_result.is_none() {
            return;
        }

        let clip_rect = Rect::new(0.0, 0.0, width, height);
        scene.push_layer(Compose::SrcOver, 1.0, offset, &clip_rect);

        // Render the chart scene graph directly with the viewport transform.
        let base_renderer = SceneRenderBuilder::new().spatium(5.0).build();
        let mut renderer = self.font_bundle.configure_renderer(base_renderer);
        renderer.render_with_transform(
            scene,
            &self.layout_result.as_ref().unwrap().scene,
            offset * transform,
        );

        scene.pop_layer();
    }

    /// Render only the dynamic overlay layer (hover + cursor).
    ///
    /// Draws on top of a previously-rendered static layer.
    #[allow(clippy::too_many_arguments)]
    pub fn render_overlay_layer_to_scene(
        &mut self,
        scene: &mut impl PaintScene,
        width: f64,
        height: f64,
        offset: Affine,
        transform: Affine,
        cursor_tick: Option<i64>,
        hover_point: Option<(f64, f64)>,
    ) {
        if self.layout_result.is_none() {
            return;
        }

        let clip_rect = Rect::new(0.0, 0.0, width, height);
        scene.push_layer(Compose::SrcOver, 1.0, offset, &clip_rect);

        if let Some((scene_x, scene_y)) = hover_point {
            self.render_hover_highlight(scene, scene_x, scene_y, offset * transform);
        }

        if let Some(tick) = cursor_tick {
            self.update_cursor(tick);
            if let Some(ref state) = self.cached_cursor_state {
                let font = self.font_bundle.smufl_font();
                render_cursor_commands(scene, &state.commands, offset * transform, Some(font));
            }
        }

        scene.pop_layer();
    }

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
            cached_chart: None,
            last_source_hash: 0,
            last_snippet_mode: false,
            cursor: ChartCursor::default(),
            cached_cursor_state: None,
            cached_cursor_tick: i64::MIN,
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
        self.cached_cursor_state = None; // Invalidate cursor cache
        self.cached_cursor_tick = i64::MIN;

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
        self.cached_cursor_state = None; // Invalidate cursor cache
        self.cached_cursor_tick = i64::MIN;
    }

    /// Render the chart to a Vello scene.
    ///
    /// Uses a cached vello Scene to avoid re-walking the scene graph every frame.
    /// The cached scene is built once at identity transform when the layout changes,
    /// then composited into the output scene with `Scene::append(cached, transform)`
    /// for instant pan/zoom (~1-2ms vs ~67ms for full re-render).
    ///
    /// If `cursor_tick` is `Some`, the cursor highlight commands are rendered on top
    /// of the chart using the same transform. The cursor is NOT part of the cached
    /// scene because it changes independently of the layout.
    ///
    /// If `hover_point` is `Some((scene_x, scene_y))`, a blue highlight is rendered
    /// on the nearest beat to that scene coordinate.
    #[allow(clippy::too_many_arguments)]
    pub fn render_to_scene(
        &mut self,
        scene: &mut impl PaintScene,
        width: f64,
        height: f64,
        offset: Affine,
        transform: Affine,
        cursor_tick: Option<i64>,
        hover_point: Option<(f64, f64)>,
    ) {
        self.render_static_layer_to_scene(scene, width, height, offset, transform);
        self.render_overlay_layer_to_scene(
            scene,
            width,
            height,
            offset,
            transform,
            cursor_tick,
            hover_point,
        );
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

    /// Extract lightweight page+system metadata from the current layout result.
    ///
    /// Returns `None` if no layout has been computed. The metadata is used by
    /// the UI for page navigation and semantic zoom calculations.
    pub fn page_metadata(&self) -> Option<Vec<PageMeta>> {
        self.layout_result.as_ref().map(|result| {
            result
                .pages
                .iter()
                .map(|page| PageMeta {
                    number: page.number,
                    x_offset: page.x_offset,
                    y_offset: page.y_offset,
                    width: page.width,
                    height: page.height,
                    systems: page
                        .systems
                        .iter()
                        .map(|sys| SystemMeta {
                            y: sys.y,
                            height: sys.height,
                        })
                        .collect(),
                })
                .collect()
        })
    }

    /// Determine which page is currently visible given scroll values.
    ///
    /// Pages are laid out **horizontally** (side by side). Converts scroll_x
    /// back to scene coordinates and finds which page's X range contains
    /// the viewport center. Returns 1-indexed page number.
    pub fn current_page_for_scroll(
        &self,
        scroll_x: f64,
        base_scale: f64,
        zoom: f64,
        dpr: f64,
    ) -> u32 {
        let Some(result) = &self.layout_result else {
            return 1;
        };
        if result.pages.is_empty() {
            return 1;
        }

        // Convert scroll_x to scene coordinates.
        // From transform: screen_x = pad - scroll_x * dpr + scene_x * base_scale * zoom
        // At the viewport left (screen_x = pad): scene_x = scroll_x * dpr / (base_scale * zoom)
        let scale = base_scale * zoom;
        if scale <= 0.0 {
            return 1;
        }
        let scene_x_left = (scroll_x * dpr) / scale;

        // Find the page whose X range contains scene_x_left (search from last to first)
        for page in result.pages.iter().rev() {
            if scene_x_left >= page.x_offset {
                return page.number;
            }
        }
        1
    }

    /// Get total page count from current layout.
    pub fn total_pages(&self) -> u32 {
        self.layout_result
            .as_ref()
            .map(|r| r.pages.len() as u32)
            .unwrap_or(1)
    }

    /// Find the page number and system index for a given absolute tick.
    ///
    /// Returns `(page_number, system_index_on_page)` where page_number is 1-indexed.
    /// Uses BeatPosition data to locate the tick, then maps it to the page/system
    /// via page layout metadata.
    pub fn system_for_tick(&self, tick: i64) -> Option<(u32, usize)> {
        let layout = self.layout_result.as_ref()?;

        // Find the BeatPosition closest to (or containing) this tick.
        // beat_positions are sorted by time_start, but we search by absolute_tick.
        let beat = layout
            .beat_positions
            .iter()
            .min_by_key(|bp| (bp.absolute_tick - tick).unsigned_abs())?;

        Some((beat.page, beat.system))
    }

    /// Compute scroll_y to position a specific system at the top of the viewport.
    ///
    /// Given a page number (1-indexed) and system index on that page,
    /// returns the scroll_y value that places that system at the viewport top.
    ///
    /// `base_scale`, `zoom`, and `dpr` are needed to convert from scene coordinates
    /// to scroll coordinates: `scroll_y = scene_y * base_scale * zoom / dpr`.
    pub fn scroll_y_for_system(
        &self,
        page_number: u32,
        system_index: usize,
        base_scale: f64,
        zoom: f64,
        dpr: f64,
    ) -> Option<f64> {
        let layout = self.layout_result.as_ref()?;
        let page = layout.pages.iter().find(|p| p.number == page_number)?;
        let system = page.systems.get(system_index)?;

        // Scene-space Y of the system = page.y_offset + system.y
        let scene_y = page.y_offset + system.y;

        // Convert to scroll_y: from the transform equation
        // screen_y = pad - scroll_y * dpr + scene_y * base_scale * zoom
        // We want screen_y ≈ pad (system at top with a small margin above)
        // → scroll_y = scene_y * base_scale * zoom / dpr
        // Subtract a small margin (e.g. 5pt worth) so system isn't flush against top edge
        let margin_pts = 5.0;
        let scroll_y = (scene_y - margin_pts) * base_scale * zoom / dpr;
        Some(scroll_y.max(0.0))
    }

    /// Compute zoom factor to show N systems in the viewport height.
    ///
    /// Takes the page metadata and picks the first N systems' total height
    /// (including spacing between them) and computes the zoom that fits that
    /// height into `viewport_height_physical` pixels.
    pub fn zoom_for_n_systems(
        &self,
        page_number: u32,
        system_index: usize,
        n: usize,
        base_scale: f64,
        viewport_height_physical: f64,
    ) -> Option<f64> {
        let layout = self.layout_result.as_ref()?;
        let page = layout.pages.iter().find(|p| p.number == page_number)?;

        if page.systems.is_empty() {
            return None;
        }

        // Get the N systems starting from system_index
        let start = system_index.min(page.systems.len().saturating_sub(1));
        let end = (start + n).min(page.systems.len());
        let systems = &page.systems[start..end];

        if systems.is_empty() {
            return None;
        }

        // Total height from top of first system to bottom of last system
        let first_y = systems.first().unwrap().y;
        let last = systems.last().unwrap();
        let content_height = (last.y + last.height) - first_y;

        if content_height <= 0.0 || base_scale <= 0.0 {
            return None;
        }

        // Add padding (10pt above and below)
        let padded_height = content_height + 20.0;

        // Fit N systems vertically: zoom = viewport_height / (content_height * base_scale).
        // This may exceed 1.0, meaning the page is wider than the viewport — the caller
        // is responsible for setting scroll_x/scroll_y to center or left-align the content.
        let zoom = viewport_height_physical / (padded_height * base_scale);
        Some(zoom.clamp(0.1, 8.0))
    }

    // ========================================================================
    // Cursor
    // ========================================================================

    /// Update the cached cursor state for a given tick.
    ///
    /// Skips recomputation if the tick hasn't changed since last call.
    fn update_cursor(&mut self, tick: i64) {
        if tick == self.cached_cursor_tick && self.cached_cursor_state.is_some() {
            return;
        }
        self.cached_cursor_tick = tick;
        self.cached_cursor_state = self
            .layout_result
            .as_ref()
            .and_then(|layout| self.cursor.compute(layout, tick));
    }

    /// Get the current cursor state (if any).
    pub fn cursor_state(&self) -> Option<&CursorState> {
        self.cached_cursor_state.as_ref()
    }

    /// Render a blue hover highlight at (scene_x, scene_y).
    ///
    /// First tries scene graph hit-testing to highlight the exact symbol
    /// under the mouse. Falls back to nearest-beat heuristic if no scene
    /// node was hit directly.
    fn render_hover_highlight(
        &self,
        scene: &mut impl PaintScene,
        scene_x: f64,
        scene_y: f64,
        transform: Affine,
    ) {
        let Some(layout) = &self.layout_result else {
            return;
        };

        // Try scene graph hit-test first for exact symbol highlighting
        if let Some(hit) = self.hit_test_at_point(scene_x, scene_y) {
            // Find the matching BeatPosition for glyph/staff info
            if let Some(beat) = layout.beat_positions.iter().find(|bp| {
                bp.measure == hit.position.measure as usize && bp.beat == hit.position.beat as usize
            }) {
                self.render_beat_hover(scene, beat, scene_x, transform);
                return;
            }
        }

        // Fallback: nearest-beat heuristic
        let page_number = layout
            .pages
            .iter()
            .rev()
            .find(|p| scene_x >= p.x_offset)
            .map(|p| p.number)
            .unwrap_or(1);

        let page_beats = layout.beats_on_page(page_number);
        if page_beats.is_empty() {
            return;
        }

        let mut best = &page_beats[0];
        let mut best_dist = f64::INFINITY;
        for beat in &page_beats {
            let y_dist = if scene_y < beat.staff_y {
                beat.staff_y - scene_y
            } else if scene_y > beat.staff_y + beat.staff_height {
                scene_y - (beat.staff_y + beat.staff_height)
            } else {
                0.0
            };
            let x_center = beat.x + beat.width / 2.0;
            let x_dist = (scene_x - x_center).abs();
            let combined = x_dist + y_dist * 3.0;
            if combined < best_dist {
                best_dist = combined;
                best = beat;
            }
        }

        if best_dist > 80.0 {
            return;
        }

        self.render_beat_hover(scene, best, scene_x, transform);
    }

    /// Render blue hover highlight commands for a specific BeatPosition.
    fn render_beat_hover(
        &self,
        scene: &mut impl PaintScene,
        beat: &engraver_proto::engraver::layout::chart::BeatPosition,
        scene_x: f64,
        transform: Affine,
    ) {
        let hover_color: Rgba = [60, 130, 255, 255];
        let hover_fill: Rgba = [60, 130, 255, 50];
        let glow_color: Rgba = [60, 130, 255, 100];
        let mut commands = Vec::new();

        // Beat box background
        commands.push(HighlightCommand::FillRoundedRect {
            x: beat.x,
            y: beat.staff_y,
            width: beat.width,
            height: beat.staff_height,
            radius: 2.0,
            color: hover_fill,
        });

        // Vertical line at hover X
        let ext = beat.staff_height * 0.15;
        commands.push(HighlightCommand::StrokeLine {
            x: scene_x.clamp(beat.x, beat.x + beat.width),
            y_top: beat.staff_y - ext,
            y_bottom: beat.staff_y + beat.staff_height + ext,
            color: [60, 130, 255, 120],
            width: 2.0,
        });

        // Notehead glyph highlight
        if let Some(codepoint) = beat.glyph_codepoint {
            let font_size = beat.glyph_size * 4.0;
            let glyph_x = beat.x;
            let glyph_y = beat.staff_y + beat.staff_height * 0.5;

            commands.push(HighlightCommand::StrokeGlyph {
                codepoint,
                font_size,
                x: glyph_x,
                y: glyph_y,
                stroke_width: 4.0,
                color: glow_color,
            });
            commands.push(HighlightCommand::FillGlyph {
                codepoint,
                font_size,
                x: glyph_x,
                y: glyph_y,
                color: hover_color,
            });
        }

        let font = self.font_bundle.smufl_font();
        render_cursor_commands(scene, &commands, transform, Some(font));
    }

    /// Get the chart's time signature as (numerator, denominator).
    ///
    /// Returns the initial/current time signature from the parsed chart,
    /// defaulting to 4/4 if not specified.
    pub fn time_signature(&self) -> (u8, u8) {
        self.cached_chart
            .as_ref()
            .and_then(|chart| {
                chart
                    .time_signature
                    .or(chart.initial_time_signature)
                    .map(|ts| (ts.numerator as u8, ts.denominator as u8))
            })
            .unwrap_or((4, 4))
    }

    /// Compute the musical position string for a given tick.
    ///
    /// Format: "M.B.TTT" where M = measure (1-indexed), B = beat (1-indexed),
    /// TTT = sub-beat ticks within that beat.
    ///
    /// Uses each `BeatPosition`'s own `time_signature` field to derive the
    /// musical beat from `BeatPosition.tick` (in-measure tick offset). This
    /// handles mid-chart time signature changes correctly.
    ///
    /// Examples in 4/4 time (480 ticks/quarter):
    /// - Beat 1: tick=0 → "M.1.000"
    /// - First triplet after beat 1: tick=160 → "M.1.160"
    /// - Beat 2: tick=480 → "M.2.000"
    /// - Eighth note of beat 3: tick=960+240=1200 → "M.3.240"
    pub fn musical_position_for_tick(&self, tick: i64) -> String {
        let Some(layout) = &self.layout_result else {
            return "1.1.000".to_string();
        };

        // Find which beat position contains this tick
        if let Some(beat) = layout.beat_at_tick(tick) {
            let measure_display = beat.measure as i64 + 1; // 1-indexed

            // Use this beat's own time signature for correct per-measure calculation
            let ts = beat.time_signature;
            let ticks_per_beat = ticks_per_beat_for_denom(ts.1);

            // Derive musical beat from the in-measure tick offset, not segment index.
            // beat.tick is the tick offset from measure start (e.g., 0, 160, 320, 480...).
            let in_measure_tick = beat.tick as i64;
            let musical_beat = (in_measure_tick / ticks_per_beat) + 1; // 1-indexed
            let sub_ticks = in_measure_tick % ticks_per_beat;

            // Clamp beat to valid range (1..=numerator)
            let musical_beat = musical_beat.clamp(1, ts.0 as i64);

            format!("{}.{}.{:03}", measure_display, musical_beat, sub_ticks)
        } else if tick < 0 {
            // Count-in territory: negative ticks — use chart-level time signature
            let time_sig = self.time_signature();
            let ticks_per_beat = ticks_per_beat_for_denom(time_sig.1);
            let ticks_per_measure = time_sig.0 as i64 * ticks_per_beat;
            let abs_tick = tick.unsigned_abs() as i64;
            let measure_back = abs_tick / ticks_per_measure;
            let remaining = abs_tick % ticks_per_measure;
            let beat_back = remaining / ticks_per_beat;
            let sub = remaining % ticks_per_beat;
            format!("-{}.{}.{:03}", measure_back + 1, beat_back + 1, sub)
        } else {
            // Beyond layout — use chart-level time signature
            let time_sig = self.time_signature();
            let ticks_per_beat = ticks_per_beat_for_denom(time_sig.1);
            let ticks_per_measure = time_sig.0 as i64 * ticks_per_beat;
            let measure = tick / ticks_per_measure;
            let remaining = tick % ticks_per_measure;
            let beat = remaining / ticks_per_beat;
            let sub = remaining % ticks_per_beat;
            format!("{}.{}.{:03}", measure + 1, beat + 1, sub)
        }
    }

    /// Compute zero-indexed musical coordinates for a given absolute tick.
    ///
    /// Returns `(measure, beat, subdivision)` where `subdivision` is 0..=999.
    pub fn musical_position_at_tick(&self, tick: i64) -> Option<(i32, i32, i32)> {
        let layout = self.layout_result.as_ref()?;
        let beat = layout.beat_at_tick(tick)?;

        let ts = beat.time_signature;
        let ticks_per_beat = ticks_per_beat_for_denom(ts.1);
        if ticks_per_beat <= 0 {
            return None;
        }

        // Recover in-measure tick from the located beat anchor and delta.
        let delta = tick - beat.absolute_tick;
        let in_measure_tick = (beat.tick as i64 + delta).max(0);
        let beat_index = (in_measure_tick / ticks_per_beat).max(0);
        let sub_ticks = (in_measure_tick % ticks_per_beat).max(0);
        let subdivision = ((sub_ticks * 1000) / ticks_per_beat).clamp(0, 999);

        Some((beat.measure as i32, beat_index as i32, subdivision as i32))
    }

    /// Convert zero-indexed musical coordinates to an absolute chart tick.
    ///
    /// Inputs are expected to use DAW-style coordinates: measure and beat are
    /// 0-indexed; subdivision is in the range 0..=999.
    pub fn tick_for_musical_position(
        &self,
        measure: i32,
        beat: i32,
        subdivision: i32,
    ) -> Option<i64> {
        let layout = self.layout_result.as_ref()?;
        if measure < 0 || beat < 0 || !(0..=999).contains(&subdivision) {
            return None;
        }

        let measure = measure as usize;
        let beat = beat as i64;

        let ts = layout
            .beat_positions
            .iter()
            .find(|bp| bp.measure == measure)
            .map(|bp| bp.time_signature)
            .unwrap_or((4, 4));

        let ticks_per_beat = ticks_per_beat_for_denom(ts.1);
        if ticks_per_beat <= 0 {
            return None;
        }

        let sub_ticks = ((subdivision as i64 * ticks_per_beat) / 1000).clamp(0, ticks_per_beat);
        let target_in_measure_tick = beat * ticks_per_beat + sub_ticks;

        let anchor = layout
            .beat_positions
            .iter()
            .filter(|bp| bp.measure == measure)
            .min_by_key(|bp| ((bp.tick as i64) - target_in_measure_tick).abs())?;

        Some(anchor.absolute_tick + (target_in_measure_tick - anchor.tick as i64))
    }

    /// Hit-test the scene graph at a point in scene coordinates.
    ///
    /// Walks the scene node tree, accumulating transforms, and finds the
    /// deepest node with `ChartPosition` metadata whose world-space bounds
    /// contain the point. Then resolves the absolute tick by matching the
    /// position's measure/beat to the `BeatPosition` array.
    ///
    /// Returns `None` if no node with chart position metadata contains the point.
    pub fn hit_test_at_point(&self, scene_x: f64, scene_y: f64) -> Option<SceneHitResult> {
        let layout = self.layout_result.as_ref()?;
        let point = Point::new(scene_x, scene_y);

        // Walk the scene graph to find nodes containing this point
        let mut hits = Vec::new();
        hit_test_scene_recursive(&layout.scene, point, Affine::IDENTITY, &mut hits);

        if hits.is_empty() {
            return None;
        }

        // Take the first (deepest) hit that has a ChartPosition
        let (node, world_transform) = &hits[0];
        let chart_pos: ChartPosition = node.get_json_metadata(metadata_keys::CHART_POSITION)?;
        let world_bounds = world_transform.transform_rect_bbox(node.bounds);
        let element_type = node.get_element_type().map(String::from);

        // Resolve absolute tick from the BeatPosition array by matching measure + beat index.
        // The ChartPosition.beat is the chord index within the measure, which maps
        // to the segment index in the BeatPosition array for that measure.
        let absolute_tick = layout
            .beat_positions
            .iter()
            .find(|bp| {
                bp.measure == chart_pos.measure as usize && bp.beat == chart_pos.beat as usize
            })
            .map(|bp| bp.absolute_tick)
            .unwrap_or_else(|| {
                // Fallback: calculate tick from position
                let ts = self.time_signature();
                chart_pos.calculate_tick(ts.0 as u32)
            });

        Some(SceneHitResult {
            position: chart_pos,
            absolute_tick,
            bounds: world_bounds,
            element_type,
        })
    }

    /// Hit-test: find the tick at a point in scene coordinates.
    ///
    /// First tries scene graph hit-testing for exact symbol matching.
    /// Falls back to nearest-beat heuristic if no scene node was hit
    /// (e.g., clicking in whitespace between symbols).
    pub fn tick_at_scene_point(&self, scene_x: f64, scene_y: f64) -> Option<i64> {
        // Try scene graph hit-test first for precise positioning
        if let Some(hit) = self.hit_test_at_point(scene_x, scene_y) {
            tracing::debug!(
                "Scene hit: {:?} at measure={} beat={} → tick={}",
                hit.element_type,
                hit.position.measure + 1,
                hit.position.beat + 1,
                hit.absolute_tick,
            );
            return Some(hit.absolute_tick);
        }

        // Fallback: nearest-beat heuristic for clicks in whitespace
        self.nearest_beat_at_point(scene_x, scene_y)
    }

    /// Nearest-beat heuristic fallback for click-to-position.
    ///
    /// Finds the closest `BeatPosition` to the given scene coordinates.
    /// Used when scene graph hit-testing doesn't find a direct node hit.
    fn nearest_beat_at_point(&self, scene_x: f64, scene_y: f64) -> Option<i64> {
        let layout = self.layout_result.as_ref()?;

        // Find which page contains this X (pages are horizontal, side by side)
        let page_number = layout
            .pages
            .iter()
            .rev()
            .find(|p| scene_x >= p.x_offset)
            .map(|p| p.number)
            .unwrap_or(1);

        let page_beats = layout.beats_on_page(page_number);
        if page_beats.is_empty() {
            return None;
        }

        // Find the beat whose x range is closest to scene_x
        let mut best = &page_beats[0];
        let mut best_dist = f64::INFINITY;
        for beat in &page_beats {
            let y_dist = if scene_y < beat.staff_y {
                beat.staff_y - scene_y
            } else if scene_y > beat.staff_y + beat.staff_height {
                scene_y - (beat.staff_y + beat.staff_height)
            } else {
                0.0
            };

            let x_center = beat.x + beat.width / 2.0;
            let x_dist = (scene_x - x_center).abs();

            // Weight Y distance more heavily to prefer beats on the correct staff line
            let combined = x_dist + y_dist * 3.0;
            if combined < best_dist {
                best_dist = combined;
                best = beat;
            }
        }

        Some(best.absolute_tick)
    }

    /// Get total ticks from the layout.
    pub fn total_ticks(&self) -> i64 {
        self.layout_result
            .as_ref()
            .map(|l| l.total_ticks())
            .unwrap_or(0)
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Compute a hash of the chart source and layout mode for cache invalidation.
    fn compute_chart_hash(&self, source: &str, snippet_mode: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        snippet_mode.hash(&mut hasher);
        hasher.finish()
    }
}
