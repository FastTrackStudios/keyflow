//! Chart Layout Engine
//!
//! Converts Keyflow Chart data to engraver SceneNode trees for rendering.
//! Supports both paginated (MuseScore-style) and continuous scroll modes.

pub mod adapters;
mod annotations;
mod bar_chords;
pub mod chord_layout;
pub mod chord_renderer;
pub mod collision;
pub mod config;
pub mod config_presets;
pub mod constants;
mod continuous;
pub mod count_in_renderer;
pub mod cursor;
pub mod engine_helpers;
pub mod layout_state;
pub mod lyric_lane;
pub mod measure_layout;
pub mod measure_pass;
pub mod measure_render;
pub mod notation_renderer;
pub mod page_rendering;
mod paginated;
pub mod pipeline;
pub mod prefix_renderer;
pub mod rhythm_builder;
pub mod section_layout;
mod section_plan;
pub mod spacing;
pub mod symbol_spacing;
mod system_breaking;
mod system_widths;
pub mod tie_pass;
mod timeline;
pub mod types;
mod vertical_spacing;
pub mod width_dist;

// Re-export new config types (ChartLayoutConfig is still defined locally for backward compatibility)
pub use config::{BehavioralFlags, DEFAULT_MIN_CHORD_SYMBOL_GAP, LayoutParams, RenderOptions};
// Note: config::ChartLayoutConfig and config::FlatChartLayoutConfig are available
// but not re-exported to avoid conflict with the legacy struct below

// Re-export main types for convenience
pub use types::{
    BeatPosition, ChartLayoutResult, LayoutMode, MeasureMelodyData, MelodyNoteSegment,
    PageLayoutMetrics, expand_melodies_across_measures, melody_note_extent, melody_pitch_to_line,
    slash_glyph_for_ticks,
};

// Re-export rhythm types from chart module (canonical source)
// These were previously in types.rs but now live in chart::rhythm
pub use crate::chart::rhythm::{
    BeatStructure, ResolvedRhythm, SectionRhythms, Spillback as PushSpillback,
    detect_push_spillbacks, detect_section_start_spillback, resolve_measure_rhythm,
    resolve_section_rhythms,
};

// Re-export measurement pass types for multi-pass layout
pub use measure_pass::{
    CachedHarmonyLayout, ChartMeasurements, ChordLayoutData, HarmonyKey, MeasureMeasurements,
    MeasurementCache, compute_measure_weight, measure_chart, measure_measure,
};

// Re-export cursor types for renderer-agnostic playback highlighting
pub use cursor::{
    ChartCursor, CursorConfig, CursorState, CursorStyle, HighlightCommand, Rgba as CursorRgba,
};

// Re-export collision detection types
pub use collision::{ChordCollisionContext, resolve_chord_positions};

pub(crate) use system_breaking::is_dense_rhythm;

use annotations::volta_at;
use section_plan::{key_signature_fifths, prevailing_fifths_at};

use std::sync::Arc;

use crate::Chart;
use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::orchestrator::PageMargins;
use crate::engraver::layout::tlayout::HarmonyStyle;
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;
use crate::engraver::style::MStyle;
use crate::sections::SectionType;
use kurbo::{Affine, Rect};

/// Chart layout engine configuration.
#[derive(Debug, Clone)]
pub struct ChartLayoutConfig {
    /// Page margins.
    pub margins: PageMargins,
    /// Staff space (spatium) in points.
    pub spatium: f64,
    /// Spacing between systems.
    pub system_spacing: f64,
    /// Maximum measures per system.
    pub max_measures_per_system: usize,
    /// Minimum measure width.
    pub min_measure_width: f64,
    /// Harmony style for chord symbols.
    pub harmony_style: HarmonyStyle,
    /// Hide repeated consecutive chord symbols.
    /// When true, if a chord is the same as the previous chord, it won't be displayed.
    /// This reduces visual clutter in charts with many repeated chords.
    pub hide_repeated_chords: bool,
    /// Whether a bar of changes re-names its beat-1 chord when the bar
    /// before held that same chord (`\\repeat_chord_symbols = clusters`).
    /// Off by default: a chord still sounding is not named again.
    pub name_cluster_beat_one: bool,
    /// Use stemmed rhythm notation (for charts with push/pull timing).
    /// When false (default), uses stemless slash notation.
    /// When true, uses stemmed rhythmic notation with beams and triplet brackets.
    pub use_stems: bool,
    /// Automatically fill whole/half notes with quarter note slashes.
    /// When true (default), chords with whole note or half note durations
    /// are automatically expanded to quarter note slashes:
    /// - A whole note chord (4 beats in 4/4) becomes 4 quarter slashes
    /// - A half note chord (2 beats) becomes 2 quarter slashes
    ///
    /// This is standard notation for master rhythm charts.
    /// Can be disabled with `\AUTO_RHYTHM_SLASHES=false` in the chart.
    pub auto_rhythm_slashes: bool,
    /// Show measure numbers above the first measure of each system.
    /// When true, displays the measure number (accounting for offset) above bars.
    pub show_measure_numbers: bool,
    /// Offset for measure numbering (from REAPER's projmeasoffs or song start).
    /// If the project starts at measure 5, set this to 4 so measure 1 displays as "5".
    pub measure_number_offset: i32,
    /// Number of count-in measures (0, 1, or 2).
    /// Determined by the distance between Count-In marker and SONGSTART marker.
    /// Count-in measures are the N measures immediately before measure 1.
    pub count_in_measures: u8,
    /// How count-in measures participate in measure numbering. See
    /// [`config::RenderOptions::count_in_uses_negative_numbers`].
    pub count_in_uses_negative_numbers: bool,
    /// Snippet mode: use simple white background without shadow.
    pub snippet_mode: bool,
    /// Minimum horizontal gap between adjacent chord symbols (in points).
    /// When chord symbols would overlap or be closer than this gap,
    /// they are automatically pushed apart during rendering.
    /// Default is 4.0 points.
    pub min_chord_symbol_gap: f64,
    /// Whether push/pull notation alters the rhythm display.
    /// When true (default), pushed chords create triplet/syncopated notation.
    /// When false, pushed chords show apostrophe markers on chord symbols.
    pub push_alters_rhythm: bool,
    /// Whether to add page offsets for multi-page viewing.
    /// When true (default), pages are positioned at (20, 20) with gaps for multi-page layouts.
    /// When false, the page is positioned at (0, 0) for single-page PDF export or edge-to-edge rendering.
    pub use_page_offsets: bool,
    /// Duration-to-space power law slope (default 1.2).
    /// Controls how aggressively longer notes get more space.
    pub spacing_slope: f64,
    /// Draw a simile bar as the `repeat1Bar` mark rather than as its chords.
    /// See [`BehavioralFlags::draw_similes`](super::chart::config::BehavioralFlags).
    pub draw_similes: bool,
    /// Draw a folded section as a titled rule instead of its bars.
    /// See [`BehavioralFlags::fold_sections`](super::chart::config::BehavioralFlags).
    pub fold_sections: bool,
    /// Pack systems by their ink rather than holding the annotation band open
    /// under every one of them.
    ///
    /// Off by default: `system_spacing` is deliberately generous so that text,
    /// figures and endings between two lines never crowd either, and a chart
    /// being read off a stand wants that air. Compact mode turns it on, where
    /// fitting the chart on one page is the whole point — a line that carries
    /// nothing below it then sits at the bare gap, close enough to keep its
    /// ink off its neighbour's and no further.
    ///
    /// A folded row is tight either way. It is a rule and a word, and the
    /// space it saves is the reason to fold.
    pub tight_system_spacing: bool,
    /// Let a section that runs one bar past the line cap stay on one line.
    ///
    /// Off by default: four bars to a line is the grid a chart is read by.
    /// But a five-bar section split in two costs a whole system of height for
    /// one bar of music, and when the point is to fit the chart — folded and
    /// compact both ask for this — a line of five is the cheaper answer. Only
    /// when the whole section fits, and only when the chord symbols do.
    pub fit_whole_section_on_one_system: bool,
    /// Spacing density (default 1.0). Higher values = tighter spacing.
    pub spacing_density: f64,
    /// Fill limit for last system justification (default 0.3).
    /// Systems below this fill ratio are left ragged.
    pub last_system_fill_limit: f64,
    /// Whether to draw tie arcs across barlines for melody notes that were
    /// split by `expand_melodies_across_measures`. Default `true` (standard
    /// notation). Set to `false` for lead-sheet styles where the second piece
    /// is rendered as a fresh attack without a tie.
    pub draw_melody_barline_ties: bool,
    /// Beam-grouping strategy for melody eighths/sixteenths within a measure.
    /// Default `Standard` (break at every beat in 4/4). `JazzHalfBar` groups
    /// across beat-2 / beat-4 boundaries (eighths in beats 1-2 and 3-4 form
    /// single beam groups), as is common in jazz lead sheets. `FullBar`
    /// groups all eighths in a measure into one beam (highly compressed).
    pub beam_grouping: BeamGroupingMode,
}

/// Viewport size class for responsive chart layout.
///
/// Each class maps to a [`ChartLayoutConfig`] tuned for that device's
/// reading distance and screen real estate. Use [`Breakpoint::from_viewport_pt`]
/// to classify a viewport width, or pick one directly for testing.
///
/// Thresholds are in points (72pt = 1 inch). At 96 DPI:
/// - Phone: < 480pt (~640 CSS px)
/// - Tablet: 480-900pt (~640-1200 CSS px)
/// - Desktop: ≥ 900pt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Breakpoint {
    /// Small screen: phones in portrait, narrow popovers.
    /// Optimized for one-handed reading at arm's length.
    Phone,
    /// Medium screen: tablets, laptops in split view.
    /// Standard rehearsal-screen sizing.
    Tablet,
    /// Large screen: monitors, full-window desktop.
    Desktop,
}

impl Breakpoint {
    /// Classify a viewport width (points) into a size class.
    #[must_use]
    pub fn from_viewport_pt(width_pt: f64) -> Self {
        if width_pt < 480.0 {
            Self::Phone
        } else if width_pt < 900.0 {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }

    /// Target measures per system for this breakpoint.
    ///
    /// iReal Pro convention: 2 cells (4 measures) on phone, 4 cells
    /// (4 measures) on tablet/desktop. Returns measure count, not cell count.
    #[must_use]
    pub fn measures_per_system(self) -> usize {
        match self {
            Self::Phone => 2,
            Self::Tablet | Self::Desktop => 4,
        }
    }
}

// `BeamGroupingMode` lives next to the beam pass in `notation`; re-exported
// here so `ChartLayoutConfig` consumers keep their existing import path.
pub use crate::engraver::notation::BeamGroupingMode;

// `impl Default for ChartLayoutConfig` and all preset / builder methods
// live in `config_presets.rs`.

/// Parameters for `ChartLayoutEngine::layout_measure`.
struct MeasureLayoutParams<'a> {
    measure: &'a crate::chart::types::Measure,
    melody_data: Option<&'a MeasureMelodyData>,
    spillbacks: Option<&'a [PushSpillback]>,
    measure_width: f64,
    include_clef: bool,
    include_time_sig: bool,
    time_signature: (u8, u8),
    /// When `include_time_sig` is set, the color of the rendered meter glyph.
    /// `None` = default black; a mid-chart meter change passes red.
    time_sig_color: Option<peniko::Color>,
    /// Chart-level clef used to map melody pitches to staff lines so the
    /// rendered note sits at the correct height for the chosen clef.
    clef: crate::chart::ChartClef,
    ctx: &'a LayoutContext<'a>,
    id_base: u64,
    is_boundary: bool,
    /// The harmony style the symbols render in, with its font metrics —
    /// what the measure measures its symbols against.
    harmony_style: &'a HarmonyStyle,
}

/// Chart layout engine.
///
/// Converts Keyflow Chart data structures to engraver SceneNode trees.
#[derive(Clone)]
pub struct ChartLayoutEngine {
    config: ChartLayoutConfig,
    /// Shared rather than `'static`, so a theme can swap the style table
    /// at runtime without leaking one per edit.
    style: Arc<MStyle>,
    text_font_data: Arc<Vec<u8>>,
    symbol_font_data: Arc<Vec<u8>>,
}

impl ChartLayoutEngine {
    /// Create a new chart layout engine.
    pub fn new(
        style: &'static MStyle,
        text_font_data: Arc<Vec<u8>>,
        symbol_font_data: Arc<Vec<u8>>,
    ) -> Self {
        Self::new_shared(Arc::new(style.clone()), text_font_data, symbol_font_data)
    }

    /// Create an engine over a shared, swappable style — what a themed
    /// view uses, since a `'static` style would leak one per theme edit.
    pub fn new_shared(
        style: Arc<MStyle>,
        text_font_data: Arc<Vec<u8>>,
        symbol_font_data: Arc<Vec<u8>>,
    ) -> Self {
        Self {
            config: ChartLayoutConfig::default(),
            style,
            text_font_data,
            symbol_font_data,
        }
    }

    /// The style this engine lays out with.
    #[must_use]
    pub fn style(&self) -> &Arc<MStyle> {
        &self.style
    }

    /// Create with custom configuration.
    pub fn with_config(
        config: ChartLayoutConfig,
        style: &'static MStyle,
        text_font_data: Arc<Vec<u8>>,
        symbol_font_data: Arc<Vec<u8>>,
    ) -> Self {
        Self {
            config,
            style: Arc::new(style.clone()),
            text_font_data,
            symbol_font_data,
        }
    }

    /// Set the harmony style for chord symbols.
    pub fn set_harmony_style(&mut self, style: HarmonyStyle) {
        self.config.harmony_style = style;
    }

    /// Build an MStyle override carrying `self.config.spatium`.
    ///
    /// Glyph-rendering paths (clef, time signature, slash, note heads)
    /// read `ctx.spatium()` which resolves to `MStyle::Sid::Spatium`
    /// — defaulting to 5pt. Without this override they ignore the
    /// per-breakpoint config.spatium and render at the wrong scale.
    fn styled_for_config(&self) -> MStyle {
        use crate::engraver::style::{Sid, StyleValue};
        let mut style = (*self.style).clone();
        style.set(Sid::Spatium, StyleValue::Real(self.config.spatium as f32));
        style
    }

    /// Layout a chart in the specified mode.
    pub fn layout_chart(&self, chart: &Chart, mode: &LayoutMode) -> ChartLayoutResult {
        // A chart with a header and no music still engraves. Everything
        // downstream — the title header included — hangs off a system,
        // and a chart with no sections produces no systems, so it used
        // to lay out to zero pages and every export answered "No chart
        // layout available to export". On screen that reads as an
        // editor that ignores you until you have typed enough.
        //
        // Giving it one empty measure is the whole fix: the header, the
        // clef, the key signature and the meter all render through the
        // paths they already use, and the measure is the one the next
        // chord typed will land in.
        let synthesized;
        let chart = if chart.sections.is_empty() {
            synthesized = Self::with_opening_measure(chart);
            &synthesized
        } else {
            chart
        };

        // Apply chart settings to a temporary config
        let config_with_settings = self.config.clone().with_chart_settings(&chart.settings);
        let temp_engine = ChartLayoutEngine {
            config: config_with_settings,
            style: Arc::clone(&self.style),
            text_font_data: self.text_font_data.clone(),
            symbol_font_data: self.symbol_font_data.clone(),
        };

        match mode {
            LayoutMode::Paginated {
                page_width,
                page_height,
            } => temp_engine.layout_paginated(chart, *page_width, *page_height),
            LayoutMode::ContinuousScroll { width } => temp_engine.layout_continuous(chart, *width),
            LayoutMode::Snippet { page_width } => {
                // Use tighter margins for snippets
                temp_engine.layout_snippet(chart, *page_width)
            }
        }
    }

    /// Layout a snippet with bounds-based sizing.
    /// Renders content once, measures bounds, then adjusts page background to fit.
    fn layout_snippet(&self, chart: &Chart, page_width: f64) -> ChartLayoutResult {
        // layout_paginated uses a fixed page_offset of 20.0 for positioning
        let page_offset = 20.0;

        // Step 1: Layout with snippet mode and small margins
        let snippet_margins = PageMargins {
            top: 5.0,
            bottom: 5.0,
            left: 50.0,
            right: 5.0,
        };

        let measure_config = ChartLayoutConfig {
            margins: snippet_margins,
            snippet_mode: true, // Simple white background
            ..self.config.clone()
        };

        let measure_engine = ChartLayoutEngine {
            config: measure_config,
            style: Arc::clone(&self.style),
            text_font_data: self.text_font_data.clone(),
            symbol_font_data: self.symbol_font_data.clone(),
        };

        // Layout with the provided page width (constrain measure widths to this)
        // Use a tall page height to fit all content vertically
        let layout_width = if page_width > 0.0 { page_width } else { 612.0 }; // Default to letter width
        let mut result = measure_engine.layout_paginated(chart, layout_width, 10000.0);

        // Step 2: Compute actual content bounds by examining children
        // Skip the first child which is page background (white rect in snippet mode)
        let mut content_bounds = Rect::ZERO;
        for (i, child) in result.scene.children.iter().enumerate() {
            // Skip first child (page background)
            if i < 1 {
                continue;
            }
            let child_bounds = child.compute_bounds();
            if !child_bounds.is_zero_area() {
                let transformed = child.transform.transform_rect_bbox(child_bounds);
                if content_bounds.is_zero_area() {
                    content_bounds = transformed;
                } else {
                    content_bounds = content_bounds.union(transformed);
                }
            }
        }

        // Step 3: Calculate final page dimensions from content bounds
        let padding = 5.0;
        let clef_bottom_padding = 8.0;

        // Content might extend above page_offset (chord symbols)
        let content_above_page = (page_offset - content_bounds.y0).max(0.0);

        // Final dimensions based on actual content
        let final_width = content_bounds.x1 + padding;
        let final_height = content_bounds.y1 + padding + clef_bottom_padding;

        // Step 4: Replace the page background with correctly sized one
        // The first child is the white background rect - replace it
        if !result.scene.children.is_empty() {
            let new_background = SceneNode::anonymous_leaf(vec![PaintCommand::filled_rect(
                Rect::new(page_offset, page_offset, final_width, final_height),
                peniko::Color::WHITE,
            )]);
            result.scene.children[0] = new_background;
        }

        // If content extends above page_offset, shift all content down
        if content_above_page > 0.0 {
            let shift = Affine::translate((0.0, content_above_page));
            for (i, child) in result.scene.children.iter_mut().enumerate() {
                if i > 0 {
                    // Skip background
                    child.transform = shift * child.transform;
                }
            }
            // Update background to account for shifted content
            if !result.scene.children.is_empty() {
                let new_background = SceneNode::anonymous_leaf(vec![PaintCommand::filled_rect(
                    Rect::new(
                        page_offset,
                        page_offset,
                        final_width,
                        final_height + content_above_page,
                    ),
                    peniko::Color::WHITE,
                )]);
                result.scene.children[0] = new_background;
            }
            // Update total dimensions with shifted content
            result.total_width = final_width + page_offset;
            result.total_height = final_height + content_above_page + page_offset;
        } else {
            // Update total dimensions to match actual content
            result.total_width = final_width + page_offset;
            result.total_height = final_height + page_offset;
        }

        result
    }

    /// Layout a chart with explicit configuration.
    ///
    /// This temporarily applies the given config for layout, then restores
    /// the engine's default config. Useful for charts with rhythmic complexity
    /// that need stemmed notation.
    /// Lay out as few pages as this chart can be persuaded into.
    ///
    /// Tries a ladder of settings, most readable first: the ordinary four bars
    /// a line, then wider systems, then wider systems at a smaller scale. It
    /// stops at the first one that reaches a single page, so a chart that
    /// already fits is never shrunk, and one that needs six bars a line is not
    /// also scaled down.
    ///
    /// Nothing fits on one page for every chart, and this does not pretend to:
    /// when no rung gets there, the one that produced the fewest pages wins,
    /// ties going to the more readable. Bounded at [`COMPACT_LADDER`]'s length
    /// of layout passes, which is the cost of asking.
    ///
    /// Only paginated modes have pages to count. Under a continuous or snippet
    /// mode this lays out once at `config` and returns that.
    ///
    /// [`COMPACT_LADDER`]: Self::COMPACT_LADDER
    #[must_use]
    pub fn layout_chart_compact(
        &self,
        chart: &Chart,
        mode: &LayoutMode,
        config: &ChartLayoutConfig,
    ) -> ChartLayoutResult {
        // Fitting the chart on one page is the whole point here, so the
        // annotation band is only held open where something uses it, and a
        // section one bar past the line cap stays on one line.
        let mut config = config.clone();
        config.tight_system_spacing = true;
        config.fit_whole_section_on_one_system = true;
        let config = &config;

        if !matches!(mode, LayoutMode::Paginated { .. }) {
            return self.layout_chart_with_config(chart, mode, config);
        }

        let mut best: Option<ChartLayoutResult> = None;
        for (measures, scale) in Self::COMPACT_LADDER {
            let mut rung = config.clone();
            rung.max_measures_per_system = measures;
            if scale < 1.0 {
                rung = rung.with_scale(scale);
            }
            let result = self.layout_chart_with_config(chart, mode, &rung);
            if result.pages.len() <= 1 {
                return result;
            }
            if best
                .as_ref()
                .is_none_or(|current| result.pages.len() < current.pages.len())
            {
                best = Some(result);
            }
        }
        best.unwrap_or_else(|| self.layout_chart_with_config(chart, mode, config))
    }

    /// `(measures per system, scale)` pairs for [`layout_chart_compact`],
    /// ordered most readable first.
    ///
    /// Wider systems come before smaller type: a chart reads better at eight
    /// bars a line than at four bars a line three-quarters the size.
    ///
    /// [`layout_chart_compact`]: Self::layout_chart_compact
    pub const COMPACT_LADDER: [(usize, f64); 13] = [
        (4, 1.0),
        (5, 1.0),
        (6, 1.0),
        (8, 1.0),
        (6, 0.9),
        (8, 0.9),
        (8, 0.8),
        (10, 0.8),
        (10, 0.7),
        (12, 0.7),
        (12, 0.65),
        (14, 0.6),
        (16, 0.55),
    ];

    pub fn layout_chart_with_config(
        &self,
        chart: &Chart,
        mode: &LayoutMode,
        config: &ChartLayoutConfig,
    ) -> ChartLayoutResult {
        // Create a temporary engine with the custom config
        let temp_engine = ChartLayoutEngine {
            config: config.clone(),
            style: Arc::clone(&self.style),
            text_font_data: self.text_font_data.clone(),
            symbol_font_data: self.symbol_font_data.clone(),
        };
        temp_engine.layout_chart(chart, mode)
    }

    /// The chart with one empty measure, for a chart that has none.
    ///
    /// The section is IMPLICIT — the writer typed no section header, so it
    /// must not draw a section card. `as_implicit` is what suppresses it;
    /// without that a bare `#E` would engrave a "Verse 1" label nobody
    /// asked for.
    ///
    /// `SectionType::Verse` is arbitrary and unused: an implicit section
    /// renders no name, and the type only matters once something labels it.
    fn with_opening_measure(chart: &Chart) -> Chart {
        use crate::chart::types::{ChartSection, Measure};
        use crate::sections::Section;

        let mut out = chart.clone();
        out.sections = vec![
            ChartSection::new(Section::new(SectionType::Verse))
                .as_implicit()
                .with_measures(vec![Measure::new()]),
        ];
        out
    }
}

// region:    --- Test Utilities

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod tests;

// endregion: --- Test Utilities
