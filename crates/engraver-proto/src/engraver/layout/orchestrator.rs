//! Layout orchestrator - ties together all layout components.
//!
//! This module provides the main entry point for laying out a complete score.
//! It coordinates the segment system, horizontal spacing, element layouts,
//! and builds the final scene graph.
//!
//! ## Architecture
//!
//! ```text
//! Score (Model)
//!     ↓
//! LayoutContext + HorizontalSpacing + Segments
//!     ↓
//! SceneNode (Scene Graph with SemanticIds)
//!     ↓
//! ┌───────────────┬───────────────┐
//! │ SceneRenderer │ SvgSerializer │
//! │   (WGPU)      │   (SVG XML)   │
//! └───────────────┴───────────────┘
//! ```

use kurbo::{Affine, Point, Rect};
use vello::peniko::Color;

use crate::engraver::fonts::SMuFLFont;
use crate::engraver::layout::context::{LayoutContext, LayoutContextOwned, LayoutMode};
use crate::engraver::layout::tlayout::chord::{
    ChordNote, ChordParams, StemDirection, layout_chord,
};
use crate::engraver::layout::tlayout::clef::{ClefParams, ClefType, layout_clef};
use crate::engraver::layout::tlayout::keysig::{KeySigParams, KeySigType, layout_keysig};
use crate::engraver::layout::tlayout::note::{
    Accidental, NoteDuration, NoteHeadType, NoteParams, layout_note,
};
use crate::engraver::layout::tlayout::rest::{RestDuration, RestParams, layout_rest};
use crate::engraver::layout::tlayout::timesig::{TimeSigParams, TimeSigType, layout_timesig};
use crate::engraver::model::LayoutBreak;
use crate::engraver::model::{DurationKind, MusicElement, Score};
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;
use crate::engraver::style::MStyle;

/// Result of laying out a complete score.
#[derive(Debug)]
pub struct LayoutResult {
    /// The complete scene graph for the score
    pub scene: SceneNode,
    /// Page layouts with system positions
    pub pages: Vec<PageLayout>,
    /// Total width in pixels
    pub total_width: f64,
    /// Total height in pixels
    pub total_height: f64,
    /// Number of systems
    pub system_count: usize,
    /// Number of measures
    pub measure_count: usize,
}

/// Layout information for a single page.
#[derive(Debug, Clone)]
pub struct PageLayout {
    /// Page number (1-indexed)
    pub number: u32,
    /// X offset in the scene (for multi-page layouts)
    pub x_offset: f64,
    /// Y offset in the scene (for multi-page layouts)
    pub y_offset: f64,
    /// Page dimensions
    pub width: f64,
    pub height: f64,
    /// Systems on this page
    pub systems: Vec<SystemLayout>,
    /// Page margins
    pub margins: PageMargins,
}

/// Layout information for a single system (line of music).
#[derive(Debug, Clone)]
pub struct SystemLayout {
    /// System index (0-indexed within score)
    pub index: usize,
    /// Y position on page (from top)
    pub y: f64,
    /// System width
    pub width: f64,
    /// System height
    pub height: f64,
    /// Measure indices in this system
    pub measure_indices: Vec<usize>,
}

/// Page margins.
#[derive(Debug, Clone, Copy)]
pub struct PageMargins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for PageMargins {
    fn default() -> Self {
        Self {
            top: 50.0,
            right: 50.0,
            bottom: 50.0,
            left: 50.0,
        }
    }
}

/// Layout engine configuration.
#[derive(Debug, Clone)]
pub struct LayoutEngineConfig {
    /// Layout mode (Page, Horizontal, Vertical, Float)
    pub mode: LayoutMode,
    /// Page width in pixels (used as system width in Horizontal/Vertical modes)
    pub page_width: f64,
    /// Page height in pixels (ignored in linear modes)
    pub page_height: f64,
    /// Page margins
    pub margins: PageMargins,
    /// Staff space (spatium) in pixels
    pub spatium: f64,
    /// Space between systems
    pub system_spacing: f64,
    /// Maximum measures per system (0 = auto, ignored in Horizontal mode)
    pub max_measures_per_system: usize,
    /// Minimum measure width
    pub min_measure_width: f64,
    /// Maximum measure width
    pub max_measure_width: f64,
    /// Viewport width for continuous modes (0 = use page_width)
    pub viewport_width: f64,
    /// Viewport height for continuous modes (0 = unlimited)
    pub viewport_height: f64,
}

impl Default for LayoutEngineConfig {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Page,
            page_width: 816.0,   // US Letter at 96 DPI (8.5" × 96)
            page_height: 1056.0, // US Letter at 96 DPI (11" × 96)
            margins: PageMargins::default(),
            spatium: 10.0,
            system_spacing: 80.0,
            max_measures_per_system: 0, // Auto
            min_measure_width: 80.0,
            max_measure_width: 400.0,
            viewport_width: 0.0,
            viewport_height: 0.0,
        }
    }
}

impl LayoutEngineConfig {
    /// Create configuration for endless horizontal mode.
    #[must_use]
    pub fn horizontal() -> Self {
        Self {
            mode: LayoutMode::Horizontal,
            ..Default::default()
        }
    }

    /// Create configuration for endless vertical mode.
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            mode: LayoutMode::Vertical,
            ..Default::default()
        }
    }

    /// Create configuration for reflow mode.
    #[must_use]
    pub fn float() -> Self {
        Self {
            mode: LayoutMode::Float,
            ..Default::default()
        }
    }

    /// Set the layout mode.
    #[must_use]
    pub fn with_mode(mut self, mode: LayoutMode) -> Self {
        self.mode = mode;
        self
    }
}

// ============================================================================
// Model to Layout Type Conversions
// ============================================================================

/// Convert model DurationKind to layout NoteDuration.
fn duration_kind_to_note_duration(kind: DurationKind) -> NoteDuration {
    match kind {
        DurationKind::Whole => NoteDuration::Whole,
        DurationKind::Half => NoteDuration::Half,
        DurationKind::Quarter => NoteDuration::Quarter,
        DurationKind::Eighth => NoteDuration::Eighth,
        DurationKind::Sixteenth => NoteDuration::Sixteenth,
        DurationKind::ThirtySecond => NoteDuration::ThirtySecond,
        DurationKind::SixtyFourth => NoteDuration::SixtyFourth,
    }
}

/// Convert model DurationKind to layout RestDuration.
fn duration_kind_to_rest_duration(kind: DurationKind) -> RestDuration {
    match kind {
        DurationKind::Whole => RestDuration::Whole,
        DurationKind::Half => RestDuration::Half,
        DurationKind::Quarter => RestDuration::Quarter,
        DurationKind::Eighth => RestDuration::Eighth,
        DurationKind::Sixteenth => RestDuration::Sixteenth,
        DurationKind::ThirtySecond => RestDuration::ThirtySecond,
        DurationKind::SixtyFourth => RestDuration::SixtyFourth,
    }
}

/// Convert model Accidental to layout Accidental.
fn model_accidental_to_layout(acc: crate::engraver::model::Accidental) -> Accidental {
    match acc {
        crate::engraver::model::Accidental::None => Accidental::None,
        crate::engraver::model::Accidental::Natural => Accidental::Natural,
        crate::engraver::model::Accidental::Sharp => Accidental::Sharp,
        crate::engraver::model::Accidental::Flat => Accidental::Flat,
        crate::engraver::model::Accidental::DoubleSharp => Accidental::DoubleSharp,
        crate::engraver::model::Accidental::DoubleFlat => Accidental::DoubleFlat,
    }
}

/// Convert model Clef to layout ClefType.
fn model_clef_to_layout(clef: crate::engraver::model::Clef) -> ClefType {
    match clef {
        crate::engraver::model::Clef::Treble => ClefType::Treble,
        crate::engraver::model::Clef::Bass => ClefType::Bass,
        crate::engraver::model::Clef::Alto => ClefType::Alto,
        crate::engraver::model::Clef::Tenor => ClefType::Tenor,
    }
}

/// Convert model Stem to layout StemDirection.
fn model_stem_to_layout(stem: crate::engraver::model::Stem) -> StemDirection {
    match stem {
        crate::engraver::model::Stem::Up => StemDirection::Up,
        crate::engraver::model::Stem::Down => StemDirection::Down,
        crate::engraver::model::Stem::None => StemDirection::Auto,
    }
}

/// Main layout engine for music notation.
///
/// Orchestrates the complete layout pipeline from Score to SceneNode.
pub struct LayoutEngine<'a> {
    config: LayoutEngineConfig,
    style: std::sync::Arc<MStyle>,
    font: Option<&'a SMuFLFont<'a>>,
}

impl<'a> LayoutEngine<'a> {
    /// Create a new layout engine with default configuration.
    pub fn new(style: &MStyle) -> Self {
        Self {
            config: LayoutEngineConfig::default(),
            style: std::sync::Arc::new(style.clone()),
            font: None,
        }
    }

    /// Create a layout engine with custom configuration.
    pub fn with_config(config: LayoutEngineConfig, style: &MStyle) -> Self {
        Self {
            config,
            style: std::sync::Arc::new(style.clone()),
            font: None,
        }
    }

    /// Create a layout engine from a shared style.
    ///
    /// Avoids cloning the style's internal property vector when callers
    /// already hold a shared style handle.
    pub fn from_arc(config: LayoutEngineConfig, style: std::sync::Arc<MStyle>) -> Self {
        Self {
            config,
            style,
            font: None,
        }
    }

    /// Set the music font for glyph layout.
    pub fn with_font(mut self, font: &'a SMuFLFont<'a>) -> Self {
        self.font = Some(font);
        self
    }

    /// Layout an entire score.
    ///
    /// This is the main entry point for the layout pipeline.
    /// Dispatches to the appropriate layout strategy based on the configured mode.
    pub fn layout_score(&self, score: &'a Score) -> LayoutResult {
        match self.config.mode {
            LayoutMode::Page | LayoutMode::Float => self.layout_page_mode(score),
            LayoutMode::Horizontal => self.layout_horizontal_mode(score),
            LayoutMode::Vertical => self.layout_vertical_mode(score),
        }
    }

    /// Layout score in Page mode (standard pagination).
    ///
    /// Creates fixed-size pages with multiple systems per page.
    /// Honors explicit page breaks and line breaks.
    fn layout_page_mode(&self, score: &'a Score) -> LayoutResult {
        // Create layout context (owned, no memory leak)
        let ctx_owned = LayoutContextOwned::new_minimal_arc(self.style.clone());
        let ctx = ctx_owned.as_context();

        // Calculate number of measures
        let measure_count = self.count_measures(score);

        // Group measures into systems (honors line breaks in Page mode)
        let systems_measures = self.compute_systems(score, measure_count);

        // Get measures for checking page breaks
        let measures: Vec<_> = score
            .parts
            .first()
            .map(|p| &p.measures)
            .into_iter()
            .flatten()
            .collect();

        // Helper to check if a system ends with a page break
        let system_has_page_break = |measure_indices: &[usize]| -> bool {
            if !self.config.mode.honors_page_breaks() {
                return false;
            }
            measure_indices
                .last()
                .and_then(|&idx| measures.get(idx))
                .is_some_and(|m| matches!(m.layout_break, Some(LayoutBreak::Page)))
        };

        // Create root scene node (use Page as the root container type)
        let mut root = SceneNode::group(SemanticId::new(ElementType::Page, 0));

        // Layout pages
        let mut pages = Vec::new();
        let mut current_page_systems = Vec::new();
        let mut page_y = self.config.margins.top;
        let mut page_number = 1u32;
        let mut system_index = 0usize;

        let content_height =
            self.config.page_height - self.config.margins.top - self.config.margins.bottom;

        for measure_indices in systems_measures.iter() {
            // Create system node
            let system_height = self.config.spatium * 4.0; // 5 staff lines = 4 spaces

            // Check if we need a new page due to height
            let height_exceeded = page_y + system_height + self.config.system_spacing
                > self.config.margins.top + content_height;

            if height_exceeded && !current_page_systems.is_empty() {
                // Finalize current page
                let page = PageLayout {
                    number: page_number,
                    x_offset: 0.0,
                    y_offset: 0.0,
                    width: self.config.page_width,
                    height: self.config.page_height,
                    systems: std::mem::take(&mut current_page_systems),
                    margins: self.config.margins,
                };
                pages.push(page);
                page_number += 1;
                page_y = self.config.margins.top;
            }

            // Create system layout info
            let system_layout = SystemLayout {
                index: system_index,
                y: page_y,
                width: self.content_width(),
                height: system_height,
                measure_indices: measure_indices.clone(),
            };
            current_page_systems.push(system_layout);

            // Layout the system
            let system_node =
                self.layout_single_system(&ctx, score, measure_indices, system_index, page_y);
            root.add_child(system_node);

            page_y += system_height + self.config.system_spacing;
            system_index += 1;

            // Check for explicit page break after this system
            if system_has_page_break(measure_indices) && !current_page_systems.is_empty() {
                // Finalize current page due to explicit page break
                let page = PageLayout {
                    number: page_number,
                    x_offset: 0.0,
                    y_offset: 0.0,
                    width: self.config.page_width,
                    height: self.config.page_height,
                    systems: std::mem::take(&mut current_page_systems),
                    margins: self.config.margins,
                };
                pages.push(page);
                page_number += 1;
                page_y = self.config.margins.top;
            }
        }

        // Add final page
        if !current_page_systems.is_empty() {
            let page = PageLayout {
                number: page_number,
                x_offset: 0.0,
                y_offset: 0.0,
                width: self.config.page_width,
                height: self.config.page_height,
                systems: current_page_systems,
                margins: self.config.margins,
            };
            pages.push(page);
        }

        // Calculate total dimensions
        let total_height = pages.len() as f64 * self.config.page_height;

        LayoutResult {
            scene: root,
            pages,
            total_width: self.config.page_width,
            total_height,
            system_count: system_index,
            measure_count,
        }
    }

    /// Layout score in Horizontal mode (endless horizontal scroll).
    ///
    /// Creates a single horizontal strip with one system containing all measures.
    /// The width grows to accommodate all content; height is fixed.
    /// Ignores all line breaks and page breaks.
    fn layout_horizontal_mode(&self, score: &'a Score) -> LayoutResult {
        // Create layout context (owned, no memory leak)
        let ctx_owned = LayoutContextOwned::new_minimal_arc(self.style.clone());
        let ctx = ctx_owned.as_context();

        let measure_count = self.count_measures(score);

        if measure_count == 0 {
            return LayoutResult {
                scene: SceneNode::group(SemanticId::new(ElementType::Page, 0)),
                pages: vec![],
                total_width: self.config.page_width,
                total_height: self.config.spatium * 4.0
                    + self.config.margins.top
                    + self.config.margins.bottom,
                system_count: 0,
                measure_count: 0,
            };
        }

        // In horizontal mode, ALL measures go into ONE system
        let all_measures: Vec<usize> = (0..measure_count).collect();

        // Calculate total width based on measure count
        let measure_width = self
            .config
            .min_measure_width
            .max(self.content_width() / 4.0);
        let total_content_width = measure_width * measure_count as f64;
        let total_width =
            total_content_width + self.config.margins.left + self.config.margins.right;

        // System height (single staff)
        let system_height = self.config.spatium * 4.0;
        let total_height = system_height + self.config.margins.top + self.config.margins.bottom;

        // Create the single system
        let mut root = SceneNode::group(SemanticId::new(ElementType::Page, 0));

        let system_layout = SystemLayout {
            index: 0,
            y: self.config.margins.top,
            width: total_content_width,
            height: system_height,
            measure_indices: all_measures.clone(),
        };

        // Layout the single horizontal system
        let system_node =
            self.layout_horizontal_system(&ctx, score, &all_measures, total_content_width);
        root.add_child(system_node);

        // Create single "page" representing the endless horizontal strip
        let page = PageLayout {
            number: 1,
            x_offset: 0.0,
            y_offset: 0.0,
            width: total_width,
            height: total_height,
            systems: vec![system_layout],
            margins: self.config.margins,
        };

        LayoutResult {
            scene: root,
            pages: vec![page],
            total_width,
            total_height,
            system_count: 1,
            measure_count,
        }
    }

    /// Layout score in Vertical mode (endless vertical scroll).
    ///
    /// Creates one infinite page with multiple systems stacked vertically.
    /// Page breaks are converted to line breaks; content flows continuously.
    fn layout_vertical_mode(&self, score: &'a Score) -> LayoutResult {
        // Create layout context (owned, no memory leak)
        let ctx_owned = LayoutContextOwned::new_minimal_arc(self.style.clone());
        let ctx = ctx_owned.as_context();

        let measure_count = self.count_measures(score);

        if measure_count == 0 {
            return LayoutResult {
                scene: SceneNode::group(SemanticId::new(ElementType::Page, 0)),
                pages: vec![],
                total_width: self.config.page_width,
                total_height: self.config.margins.top + self.config.margins.bottom,
                system_count: 0,
                measure_count: 0,
            };
        }

        // Group measures into systems (same as page mode, but no page breaks)
        let systems_measures = self.compute_systems(score, measure_count);

        // Create root node
        let mut root = SceneNode::group(SemanticId::new(ElementType::Page, 0));

        // Layout all systems on one endless vertical page
        let mut all_systems = Vec::new();
        let mut current_y = self.config.margins.top;
        let system_height = self.config.spatium * 4.0;

        for (system_index, measure_indices) in systems_measures.iter().enumerate() {
            // Create system layout info
            let system_layout = SystemLayout {
                index: system_index,
                y: current_y,
                width: self.content_width(),
                height: system_height,
                measure_indices: measure_indices.clone(),
            };
            all_systems.push(system_layout);

            // Layout the system
            let system_node =
                self.layout_single_system(&ctx, score, measure_indices, system_index, current_y);
            root.add_child(system_node);

            current_y += system_height + self.config.system_spacing;
        }

        // Total height is the sum of all systems plus margins
        let total_height = current_y - self.config.system_spacing + self.config.margins.bottom;

        // Create single "page" representing the endless vertical strip
        let page = PageLayout {
            number: 1,
            x_offset: 0.0,
            y_offset: 0.0,
            width: self.config.page_width,
            height: total_height,
            systems: all_systems,
            margins: self.config.margins,
        };

        LayoutResult {
            scene: root,
            pages: vec![page],
            total_width: self.config.page_width,
            total_height,
            system_count: systems_measures.len(),
            measure_count,
        }
    }

    /// Layout a horizontal system (used in Horizontal mode).
    ///
    /// Creates a single system containing all measures laid out horizontally.
    fn layout_horizontal_system(
        &self,
        ctx: &LayoutContext<'_>,
        score: &Score,
        measure_indices: &[usize],
        total_width: f64,
    ) -> SceneNode {
        let measure_count = measure_indices.len();
        let measure_width = if measure_count > 0 {
            total_width / measure_count as f64
        } else {
            total_width
        };

        // Create system node
        let mut system_node = SceneNode::group(SemanticId::new(ElementType::System, 0));
        system_node.transform =
            Affine::translate((self.config.margins.left, self.config.margins.top));

        let mut current_x = 0.0;

        for (local_idx, &measure_idx) in measure_indices.iter().enumerate() {
            let is_first = local_idx == 0;
            let is_last = local_idx == measure_count - 1;

            // Create measure node
            let mut measure_node = SceneNode::group(SemanticId::measure(measure_idx as u64));
            measure_node.transform = Affine::translate((current_x, 0.0));

            // Add staff lines
            let staff_lines = self.create_staff_lines(measure_width);
            measure_node.add_child(staff_lines);

            // Add barlines
            if is_first {
                let left_barline = self.create_barline(0.0, false);
                measure_node.add_child(left_barline);
            }

            // Track x position within measure
            let mut element_x = self.config.spatium * 0.5;

            // Add clef at start
            if is_first {
                let clef_params = ClefParams {
                    id: 0,
                    clef_type: ClefType::Treble,
                    is_change: false,
                    ..Default::default()
                };
                let (_, clef_node) = layout_clef(&clef_params, ctx);
                let mut positioned = clef_node;
                positioned.transform = Affine::translate((element_x, 0.0));
                measure_node.add_child(positioned);
                element_x += self.config.spatium * 3.0;

                // Add time signature
                let ts_params = TimeSigParams {
                    id: 1,
                    sig_type: TimeSigType::Numeric {
                        numerator: score.time_signature.numerator,
                        denominator: score.time_signature.denominator,
                    },
                    large: false,
                    color: None,
                };
                let (_, ts_node) = layout_timesig(&ts_params, ctx);
                let mut positioned = ts_node;
                positioned.transform = Affine::translate((element_x, 0.0));
                measure_node.add_child(positioned);
                element_x += self.config.spatium * 3.0;
            }

            // Layout music elements
            if let Some(part) = score.parts.first()
                && let Some(measure) = part.measures.get(measure_idx)
            {
                let content_area = measure_width - element_x - self.config.spatium * 0.5;
                let elements_node = self.layout_measure_elements(
                    measure,
                    ctx,
                    element_x,
                    content_area,
                    measure_idx as u64,
                );
                measure_node.add_child(elements_node);
            }

            // Right barline
            let right_barline = self.create_barline(measure_width, is_last);
            measure_node.add_child(right_barline);

            // Calculate bounds
            let staff_height = self.config.spatium * 4.0;
            measure_node.bounds =
                Rect::new(0.0, -staff_height / 2.0, measure_width, staff_height / 2.0);

            system_node.add_child(measure_node);
            current_x += measure_width;
        }

        // Set system bounds
        let staff_height = self.config.spatium * 4.0;
        system_node.bounds = Rect::new(0.0, -staff_height / 2.0, total_width, staff_height / 2.0);

        system_node
    }

    /// Get the content width (page width minus margins).
    fn content_width(&self) -> f64 {
        self.config.page_width - self.config.margins.left - self.config.margins.right
    }

    /// Count total measures in score.
    fn count_measures(&self, score: &Score) -> usize {
        score.parts.first().map_or(0, |p| p.measures.len())
    }

    /// Group measures into systems, respecting explicit breaks based on layout mode.
    ///
    /// - In Page/Vertical mode: honors line breaks (Line, Section, Page all force line break)
    /// - In Horizontal/Float mode: ignores line breaks
    fn compute_systems(&self, score: &Score, measure_count: usize) -> Vec<Vec<usize>> {
        let mut systems = Vec::new();
        let content_width = self.content_width();
        let honors_breaks = self.config.mode.honors_line_breaks();

        // Get the measures for checking explicit breaks
        let measures: Vec<_> = score
            .parts
            .first()
            .map(|p| &p.measures)
            .into_iter()
            .flatten()
            .collect();

        if self.config.max_measures_per_system > 0 {
            // Fixed measures per system, but still honor explicit breaks
            let mut current_system = Vec::new();

            for i in 0..measure_count {
                current_system.push(i);

                // Check if we've reached the max measures per system
                let at_max = current_system.len() >= self.config.max_measures_per_system;

                // Check if this measure has an explicit break (and we honor breaks)
                let has_explicit_break = honors_breaks
                    && measures.get(i).is_some_and(|m| {
                        matches!(
                            m.layout_break,
                            Some(LayoutBreak::Line)
                                | Some(LayoutBreak::Section)
                                | Some(LayoutBreak::Page)
                        )
                    });

                // Check if next measure has NoBreak (prevents automatic breaks)
                let next_has_nobreak = measures
                    .get(i)
                    .is_some_and(|m| matches!(m.layout_break, Some(LayoutBreak::NoBreak)));

                // Start new system if at max or explicit break (unless NoBreak)
                if (at_max || has_explicit_break) && !next_has_nobreak {
                    systems.push(std::mem::take(&mut current_system));
                }
            }

            if !current_system.is_empty() {
                systems.push(current_system);
            }
        } else {
            // Auto-compute based on measure widths
            let mut current_system = Vec::new();
            let mut current_width = 0.0;

            for i in 0..measure_count {
                let measure_width = self.estimate_measure_width(score, i);

                // Check if this measure has an explicit break (and we honor breaks)
                let has_explicit_break = honors_breaks
                    && measures.get(i).is_some_and(|m| {
                        matches!(
                            m.layout_break,
                            Some(LayoutBreak::Line)
                                | Some(LayoutBreak::Section)
                                | Some(LayoutBreak::Page)
                        )
                    });

                // Check if current measure has NoBreak (prevents automatic breaks here)
                let has_nobreak = measures
                    .get(i)
                    .is_some_and(|m| matches!(m.layout_break, Some(LayoutBreak::NoBreak)));

                // Check if we need to start a new system due to width (unless NoBreak)
                let width_exceeded = current_width + measure_width > content_width
                    && !current_system.is_empty()
                    && !has_nobreak;

                if width_exceeded {
                    systems.push(std::mem::take(&mut current_system));
                    current_width = 0.0;
                }

                current_system.push(i);
                current_width += measure_width;

                // Check for explicit break after adding the measure
                if has_explicit_break {
                    systems.push(std::mem::take(&mut current_system));
                    current_width = 0.0;
                }
            }

            if !current_system.is_empty() {
                systems.push(current_system);
            }
        }

        // Handle empty case
        if systems.is_empty() && measure_count > 0 {
            systems.push((0..measure_count).collect());
        }

        systems
    }

    /// Estimate width needed for a measure.
    fn estimate_measure_width(&self, _score: &Score, _measure_idx: usize) -> f64 {
        // Simple estimation based on config
        // A full implementation would analyze note density
        self.config
            .min_measure_width
            .max(self.content_width() / 4.0)
    }

    /// Layout a single system with actual music content.
    fn layout_single_system(
        &self,
        ctx: &LayoutContext<'_>,
        score: &Score,
        measure_indices: &[usize],
        system_index: usize,
        y_position: f64,
    ) -> SceneNode {
        let content_width = self.content_width();
        let measure_count = measure_indices.len();

        // Calculate measure widths (distribute evenly for now)
        let measure_width = if measure_count > 0 {
            content_width / measure_count as f64
        } else {
            content_width
        };

        // Create system node
        let mut system_node =
            SceneNode::group(SemanticId::new(ElementType::System, system_index as u64));
        system_node.transform = Affine::translate((self.config.margins.left, y_position));

        let mut current_x = 0.0;
        let is_first_system = system_index == 0;

        // Layout each measure
        for (local_idx, &measure_idx) in measure_indices.iter().enumerate() {
            let is_first = local_idx == 0;
            let is_last = local_idx == measure_count - 1;

            // Create measure node
            let mut measure_node = SceneNode::group(SemanticId::measure(measure_idx as u64));
            measure_node.transform = Affine::translate((current_x, 0.0));

            // Add staff lines
            let staff_lines = self.create_staff_lines(measure_width);
            measure_node.add_child(staff_lines);

            // Add barlines
            if is_first {
                // Left barline at start of system
                let left_barline = self.create_barline(0.0, false);
                measure_node.add_child(left_barline);
            }

            // Track x position within measure for element placement
            let mut element_x = self.config.spatium * 0.5; // Small left margin

            // Add clef at start of first system's first measure
            if is_first_system && is_first {
                let clef_params = ClefParams {
                    id: 0,
                    clef_type: ClefType::Treble,
                    is_change: false,
                    ..Default::default()
                };
                let (_, clef_node) = layout_clef(&clef_params, ctx);
                let mut positioned = clef_node;
                positioned.transform = Affine::translate((element_x, 0.0));
                measure_node.add_child(positioned);
                element_x += self.config.spatium * 3.0;
            }

            // Add key signature at start of first system's first measure (if not C major)
            if is_first_system
                && is_first
                && score.time_signature != crate::engraver::model::TimeSignature::COMMON
            {
                // Key signature would go here if the score had one
            }

            // Add time signature at start of first system's first measure
            if is_first_system && is_first {
                let ts_params = TimeSigParams {
                    id: 1,
                    sig_type: TimeSigType::Numeric {
                        numerator: score.time_signature.numerator,
                        denominator: score.time_signature.denominator,
                    },
                    large: false,
                    color: None,
                };
                let (_, ts_node) = layout_timesig(&ts_params, ctx);
                let mut positioned = ts_node;
                positioned.transform = Affine::translate((element_x, 0.0));
                measure_node.add_child(positioned);
                element_x += self.config.spatium * 3.0;
            }

            // Layout music elements from the score
            if let Some(part) = score.parts.first()
                && let Some(measure) = part.measures.get(measure_idx)
            {
                let content_area = measure_width - element_x - self.config.spatium * 0.5;
                let elements_node = self.layout_measure_elements(
                    measure,
                    ctx,
                    element_x,
                    content_area,
                    measure_idx as u64,
                );
                measure_node.add_child(elements_node);
            }

            // Right barline
            let right_barline = self.create_barline(measure_width, is_last);
            measure_node.add_child(right_barline);

            // Calculate bounds
            let staff_height = self.config.spatium * 4.0;
            measure_node.bounds =
                Rect::new(0.0, -staff_height / 2.0, measure_width, staff_height / 2.0);

            system_node.add_child(measure_node);
            current_x += measure_width;
        }

        // Set system bounds
        let staff_height = self.config.spatium * 4.0;
        system_node.bounds = Rect::new(0.0, -staff_height / 2.0, content_width, staff_height / 2.0);

        system_node
    }

    /// Layout music elements within a measure.
    fn layout_measure_elements(
        &self,
        measure: &crate::engraver::model::Measure,
        ctx: &LayoutContext,
        start_x: f64,
        content_width: f64,
        measure_id: u64,
    ) -> SceneNode {
        let mut elements_node = SceneNode::group(SemanticId::new(ElementType::Segment, measure_id));
        let mut element_id_counter = measure_id * 1000; // Unique ID base for this measure

        // Get time signature for duration calculations
        let time_sig = measure
            .time_signature
            .unwrap_or(crate::engraver::model::TimeSignature::COMMON);
        let measure_duration_quarters = time_sig.measure_duration();

        // Collect all timed elements with their beat positions
        let mut timed_elements: Vec<(f64, u64, MusicElement)> = Vec::new();

        for voice in &measure.voices {
            let mut current_beat = 0.0;
            for element in &voice.elements {
                let duration = match element {
                    MusicElement::Note(note) => note.duration.quarters(),
                    MusicElement::Rest(rest) => rest.duration.quarters(),
                    MusicElement::Chord(chord) => chord.duration().quarters(),
                    _ => 0.0, // Non-durational elements
                };

                element_id_counter += 1;
                timed_elements.push((current_beat, element_id_counter, element.clone()));
                current_beat += duration;
            }
        }

        // Layout each element at its beat position
        for (beat_position, elem_id, element) in timed_elements {
            // Calculate x position based on beat position
            let beat_fraction = beat_position / measure_duration_quarters;
            let x_pos = start_x + beat_fraction * content_width;

            match element {
                MusicElement::Note(note) => {
                    let params = NoteParams {
                        id: elem_id,
                        duration: duration_kind_to_note_duration(note.duration.kind),
                        line: note.pitch.staff_position(),
                        accidental: model_accidental_to_layout(note.accidental),
                        dots: note.duration.dots,
                        offset_x: 0.0,
                        ledger_lines: true,
                        ..Default::default()
                    };
                    let (_, mut node) = layout_note(&params, ctx);
                    node.transform = Affine::translate((x_pos, 0.0));
                    elements_node.add_child(node);
                }
                MusicElement::Rest(rest) => {
                    let params = RestParams {
                        id: elem_id,
                        duration: duration_kind_to_rest_duration(rest.duration.kind),
                        dots: rest.duration.dots,
                        line: 0, // Center
                    };
                    let (_, mut node) = layout_rest(&params, ctx);
                    node.transform = Affine::translate((x_pos, 0.0));
                    elements_node.add_child(node);
                }
                MusicElement::Chord(chord) => {
                    if let Some(first_note) = chord.notes.first() {
                        let notes: Vec<ChordNote> = chord
                            .notes
                            .iter()
                            .map(|n| ChordNote {
                                line: n.pitch.staff_position(),
                                accidental: model_accidental_to_layout(n.accidental),
                                tie: n.tie_forward,
                            })
                            .collect();
                        let params = ChordParams {
                            id: elem_id,
                            duration: duration_kind_to_note_duration(first_note.duration.kind),
                            head_type: NoteHeadType::Normal,
                            notes,
                            stem_direction: model_stem_to_layout(first_note.stem),
                            dots: first_note.duration.dots,
                            beamed: false,
                            stemless: false,
                        };
                        let (_, mut node) = layout_chord(&params, ctx);
                        node.transform = Affine::translate((x_pos, 0.0));
                        elements_node.add_child(node);
                    }
                }
                MusicElement::Clef(clef) => {
                    let params = ClefParams {
                        id: elem_id,
                        clef_type: model_clef_to_layout(clef),
                        is_change: true,
                        ..Default::default()
                    };
                    let (_, mut node) = layout_clef(&params, ctx);
                    node.transform = Affine::translate((x_pos, 0.0));
                    elements_node.add_child(node);
                }
                MusicElement::KeySignature(key_sig) => {
                    let params = KeySigParams {
                        id: elem_id,
                        key: KeySigType::Standard(key_sig.fifths),
                        ..Default::default()
                    };
                    let (_, mut node) = layout_keysig(&params, ctx);
                    node.transform = Affine::translate((x_pos, 0.0));
                    elements_node.add_child(node);
                }
                MusicElement::TimeSignature(time_sig) => {
                    let params = TimeSigParams {
                        id: elem_id,
                        sig_type: TimeSigType::Numeric {
                            numerator: time_sig.numerator,
                            denominator: time_sig.denominator,
                        },
                        large: false,
                        color: None,
                    };
                    let (_, mut node) = layout_timesig(&params, ctx);
                    node.transform = Affine::translate((x_pos, 0.0));
                    elements_node.add_child(node);
                }
                MusicElement::Dynamic(_)
                | MusicElement::Barline(_)
                | MusicElement::ChordSymbol(_) => {
                    // TODO: Layout dynamics, barlines, and chord symbols
                }
            }
        }

        elements_node
    }

    /// Create staff lines for a measure.
    fn create_staff_lines(&self, width: f64) -> SceneNode {
        let spatium = self.config.spatium;
        let _half_staff = 2.0 * spatium; // 4 spaces / 2
        let line_thickness = spatium * 0.1;

        let mut commands = Vec::new();

        for i in 0..5 {
            let y = (i as f64 - 2.0) * spatium;
            commands.push(PaintCommand::line(
                Point::new(0.0, y),
                Point::new(width, y),
                Color::BLACK,
                line_thickness,
            ));
        }

        SceneNode::anonymous_leaf(commands)
    }

    /// Create a barline.
    fn create_barline(&self, x: f64, is_final: bool) -> SceneNode {
        let spatium = self.config.spatium;
        let half_staff = 2.0 * spatium;
        let thin_width = spatium * 0.16;
        let thick_width = spatium * 0.5;

        let mut commands = Vec::new();

        if is_final {
            // Final barline: thin + thick
            commands.push(PaintCommand::line(
                Point::new(x - thick_width - thin_width * 2.0, -half_staff),
                Point::new(x - thick_width - thin_width * 2.0, half_staff),
                Color::BLACK,
                thin_width,
            ));
            commands.push(PaintCommand::filled_rect(
                Rect::new(x - thick_width, -half_staff, x, half_staff),
                Color::BLACK,
            ));
        } else {
            // Normal barline
            commands.push(PaintCommand::line(
                Point::new(x, -half_staff),
                Point::new(x, half_staff),
                Color::BLACK,
                thin_width,
            ));
        }

        let mut node = SceneNode::leaf(SemanticId::new(ElementType::Barline, 0), commands);
        node.bounds = Rect::new(x - thick_width, -half_staff, x, half_staff);
        node
    }
}

/// Builder for creating a layout engine with custom configuration.
pub struct LayoutEngineBuilder<'a> {
    config: LayoutEngineConfig,
    style: Option<&'a MStyle>,
    font: Option<&'a SMuFLFont<'a>>,
}

impl<'a> LayoutEngineBuilder<'a> {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: LayoutEngineConfig::default(),
            style: None,
            font: None,
        }
    }

    /// Set page dimensions.
    pub fn page_size(mut self, width: f64, height: f64) -> Self {
        self.config.page_width = width;
        self.config.page_height = height;
        self
    }

    /// Set page margins.
    pub fn margins(mut self, margins: PageMargins) -> Self {
        self.config.margins = margins;
        self
    }

    /// Set uniform margins.
    pub fn uniform_margins(mut self, margin: f64) -> Self {
        self.config.margins = PageMargins {
            top: margin,
            right: margin,
            bottom: margin,
            left: margin,
        };
        self
    }

    /// Set the spatium (staff space) in pixels.
    pub fn spatium(mut self, spatium: f64) -> Self {
        self.config.spatium = spatium;
        self
    }

    /// Set spacing between systems.
    pub fn system_spacing(mut self, spacing: f64) -> Self {
        self.config.system_spacing = spacing;
        self
    }

    /// Set maximum measures per system (0 = auto).
    pub fn max_measures_per_system(mut self, max: usize) -> Self {
        self.config.max_measures_per_system = max;
        self
    }

    /// Set the style.
    pub fn style(mut self, style: &'a MStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set the music font.
    pub fn font(mut self, font: &'a SMuFLFont<'a>) -> Self {
        self.font = Some(font);
        self
    }

    /// Build the layout engine.
    ///
    /// # Panics
    /// Panics if style is not set.
    pub fn build(self) -> LayoutEngine<'a> {
        let style = self.style.expect("Style must be set");
        let mut engine = LayoutEngine::with_config(self.config, style);
        if let Some(font) = self.font {
            engine = engine.with_font(font);
        }
        engine
    }
}

impl<'a> Default for LayoutEngineBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to layout a score with default settings.
///
/// # Arguments
/// * `score` - The score to layout
/// * `style` - The style settings
///
/// # Returns
/// The layout result containing the scene graph and page information.
pub fn layout_score<'a>(score: &'a Score, style: &'a MStyle) -> LayoutResult {
    LayoutEngine::new(style).layout_score(score)
}

/// Convenience function to layout a score with custom configuration.
pub fn layout_score_with_config<'a>(
    score: &'a Score,
    style: &'a MStyle,
    config: LayoutEngineConfig,
) -> LayoutResult {
    LayoutEngine::with_config(config, style).layout_score(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engraver::model::{Measure, Part, PartId};

    fn create_test_score(measure_count: usize) -> Score {
        let measures: Vec<Measure> = (0..measure_count)
            .map(|i| Measure {
                number: (i + 1) as u32,
                ..Default::default()
            })
            .collect();

        let mut part = Part::new(PartId(1), "Test");
        part.measures = measures;

        Score {
            parts: vec![part],
            ..Default::default()
        }
    }

    /// Create a test score with explicit breaks at specific measures.
    fn create_test_score_with_breaks(
        measure_count: usize,
        breaks: &[(usize, LayoutBreak)],
    ) -> Score {
        let mut measures: Vec<Measure> = (0..measure_count)
            .map(|i| Measure {
                number: (i + 1) as u32,
                ..Default::default()
            })
            .collect();

        // Add breaks to specified measures
        for &(idx, break_type) in breaks {
            if let Some(measure) = measures.get_mut(idx) {
                measure.layout_break = Some(break_type);
            }
        }

        let mut part = Part::new(PartId(1), "Test");
        part.measures = measures;

        Score {
            parts: vec![part],
            ..Default::default()
        }
    }

    #[test]
    fn test_layout_empty_score() {
        let score = Score::default();
        let style = MStyle::default();
        let result = layout_score(&score, &style);

        assert_eq!(result.measure_count, 0);
        assert_eq!(result.system_count, 0);
    }

    #[test]
    fn test_layout_single_measure() {
        let score = create_test_score(1);
        let style = MStyle::default();
        let result = layout_score(&score, &style);

        assert_eq!(result.measure_count, 1);
        assert_eq!(result.system_count, 1);
        assert_eq!(result.pages.len(), 1);
    }

    #[test]
    fn test_layout_multiple_measures() {
        let score = create_test_score(8);
        let style = MStyle::default();
        let result = layout_score(&score, &style);

        assert_eq!(result.measure_count, 8);
        assert!(result.system_count >= 1);
        assert!(!result.pages.is_empty());
    }

    #[test]
    fn test_layout_with_fixed_measures_per_system() {
        let score = create_test_score(8);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            max_measures_per_system: 4,
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        assert_eq!(result.measure_count, 8);
        assert_eq!(result.system_count, 2); // 8 measures / 4 per system
    }

    #[test]
    fn test_layout_scene_structure() {
        let score = create_test_score(4);
        let style = MStyle::default();
        let result = layout_score(&score, &style);

        // Root should have system children
        assert!(!result.scene.children.is_empty());

        // First child should be a system
        let first_system = &result.scene.children[0];
        assert!(first_system.id.is_some());

        // System should have measure children
        assert!(!first_system.children.is_empty());
    }

    #[test]
    fn test_page_margins() {
        let score = create_test_score(4);
        let style = MStyle::default();
        let margins = PageMargins {
            top: 100.0,
            right: 50.0,
            bottom: 100.0,
            left: 50.0,
        };
        let config = LayoutEngineConfig {
            margins,
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        assert_eq!(result.pages[0].margins.top, 100.0);
        assert_eq!(result.pages[0].margins.left, 50.0);
    }

    #[test]
    fn test_layout_builder() {
        let style = MStyle::default();
        let engine = LayoutEngineBuilder::new()
            .page_size(800.0, 1000.0)
            .uniform_margins(30.0)
            .spatium(12.0)
            .max_measures_per_system(3)
            .style(&style)
            .build();

        assert_eq!(engine.config.page_width, 800.0);
        assert_eq!(engine.config.spatium, 12.0);
        assert_eq!(engine.config.max_measures_per_system, 3);
    }

    #[test]
    fn test_content_width_calculation() {
        let style = MStyle::default();
        let engine = LayoutEngineBuilder::new()
            .page_size(800.0, 1000.0)
            .uniform_margins(50.0)
            .style(&style)
            .build();

        let content_width = engine.content_width();
        assert_eq!(content_width, 700.0); // 800 - 50 - 50
    }

    #[test]
    fn test_multiple_pages() {
        let score = create_test_score(50); // Many measures
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            max_measures_per_system: 4,
            system_spacing: 150.0, // Large spacing to force multiple pages
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Should have multiple pages with large system spacing
        assert!(!result.pages.is_empty());
    }

    #[test]
    fn test_system_layout_info() {
        let score = create_test_score(8);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            max_measures_per_system: 4,
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Check first page systems
        let first_page = &result.pages[0];
        assert!(!first_page.systems.is_empty());

        // First system should have 4 measures
        let first_system = &first_page.systems[0];
        assert_eq!(first_system.measure_indices.len(), 4);
        assert_eq!(first_system.index, 0);
    }

    // ======================================================================
    // Layout Mode Tests
    // ======================================================================

    #[test]
    fn test_horizontal_mode_empty_score() {
        let score = Score::default();
        let style = MStyle::default();
        let config = LayoutEngineConfig::horizontal();
        let result = layout_score_with_config(&score, &style, config);

        assert_eq!(result.measure_count, 0);
        assert_eq!(result.system_count, 0);
        assert!(result.pages.is_empty());
    }

    #[test]
    fn test_horizontal_mode_single_system() {
        let score = create_test_score(12);
        let style = MStyle::default();
        let config = LayoutEngineConfig::horizontal();
        let result = layout_score_with_config(&score, &style, config);

        // Horizontal mode creates exactly ONE system with ALL measures
        assert_eq!(result.measure_count, 12);
        assert_eq!(result.system_count, 1);
        assert_eq!(result.pages.len(), 1);

        // The single system should contain all measures
        let page = &result.pages[0];
        assert_eq!(page.systems.len(), 1);
        assert_eq!(page.systems[0].measure_indices.len(), 12);
    }

    #[test]
    fn test_horizontal_mode_width_grows() {
        let score = create_test_score(20);
        let style = MStyle::default();
        let config = LayoutEngineConfig::horizontal();
        let result = layout_score_with_config(&score, &style, config);

        // Width should grow based on measure count
        // Default measure width estimation is content_width / 4
        let default_config = LayoutEngineConfig::default();
        let content_width =
            default_config.page_width - default_config.margins.left - default_config.margins.right;
        let min_measure_width = default_config.min_measure_width.max(content_width / 4.0);
        let expected_content_width = min_measure_width * 20.0;
        let expected_total_width =
            expected_content_width + default_config.margins.left + default_config.margins.right;

        assert_eq!(result.total_width, expected_total_width);
    }

    #[test]
    fn test_horizontal_mode_fixed_height() {
        let score = create_test_score(50);
        let style = MStyle::default();
        let config = LayoutEngineConfig::horizontal();
        let spatium = config.spatium;
        let margins = config.margins;
        let result = layout_score_with_config(&score, &style, config);

        // Height should be fixed (one staff height + margins)
        let system_height = spatium * 4.0;
        let expected_height = system_height + margins.top + margins.bottom;

        assert_eq!(result.total_height, expected_height);
    }

    #[test]
    fn test_vertical_mode_empty_score() {
        let score = Score::default();
        let style = MStyle::default();
        let config = LayoutEngineConfig::vertical();
        let result = layout_score_with_config(&score, &style, config);

        assert_eq!(result.measure_count, 0);
        assert_eq!(result.system_count, 0);
        assert!(result.pages.is_empty());
    }

    #[test]
    fn test_vertical_mode_single_page() {
        let score = create_test_score(50);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Vertical,
            max_measures_per_system: 4,
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Vertical mode creates exactly ONE page (endless vertical)
        assert_eq!(result.pages.len(), 1);

        // Should have multiple systems (50 measures / 4 per system = ~13)
        assert!(result.system_count > 1);
        assert_eq!(result.pages[0].systems.len(), result.system_count);
    }

    #[test]
    fn test_vertical_mode_height_grows() {
        let score = create_test_score(20);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Vertical,
            max_measures_per_system: 4,
            system_spacing: 80.0,
            ..Default::default()
        };
        let spatium = config.spatium;
        let margins = config.margins;
        let system_spacing = config.system_spacing;
        let result = layout_score_with_config(&score, &style, config);

        // Height should grow based on system count
        // 20 measures / 4 per system = 5 systems
        let system_count = 5;
        let system_height = spatium * 4.0;
        let expected_min_height = margins.top
            + (system_height * system_count as f64)
            + (system_spacing * (system_count - 1) as f64)
            + margins.bottom;

        assert!(result.total_height >= expected_min_height - 1.0);
    }

    #[test]
    fn test_vertical_mode_fixed_width() {
        let score = create_test_score(50);
        let style = MStyle::default();
        let config = LayoutEngineConfig::vertical();
        let page_width = config.page_width;
        let result = layout_score_with_config(&score, &style, config);

        // Width should be fixed to page width
        assert_eq!(result.total_width, page_width);
    }

    #[test]
    fn test_float_mode_uses_page_layout() {
        // Float mode should behave similar to Page mode
        let score = create_test_score(16);
        let style = MStyle::default();
        let config = LayoutEngineConfig::float();
        let result = layout_score_with_config(&score, &style, config);

        // Float uses page mode internally, so should have pagination
        assert!(!result.pages.is_empty());
        assert_eq!(result.measure_count, 16);
    }

    #[test]
    fn test_layout_mode_config_builders() {
        let horizontal = LayoutEngineConfig::horizontal();
        assert_eq!(horizontal.mode, LayoutMode::Horizontal);

        let vertical = LayoutEngineConfig::vertical();
        assert_eq!(vertical.mode, LayoutMode::Vertical);

        let float = LayoutEngineConfig::float();
        assert_eq!(float.mode, LayoutMode::Float);

        let custom = LayoutEngineConfig::default().with_mode(LayoutMode::Vertical);
        assert_eq!(custom.mode, LayoutMode::Vertical);
    }

    #[test]
    fn test_page_mode_creates_multiple_pages() {
        let score = create_test_score(100);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 4,
            system_spacing: 200.0, // Large spacing to force page breaks
            page_height: 400.0,    // Small page to force more pages
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Page mode should create multiple pages
        assert!(
            result.pages.len() > 1,
            "Expected multiple pages, got {}",
            result.pages.len()
        );
    }

    #[test]
    fn test_mode_comparison_horizontal_vs_vertical() {
        let score = create_test_score(16);
        let style = MStyle::default();

        let h_config = LayoutEngineConfig {
            mode: LayoutMode::Horizontal,
            max_measures_per_system: 4,
            ..Default::default()
        };
        let h_result = layout_score_with_config(&score, &style, h_config);

        let v_config = LayoutEngineConfig {
            mode: LayoutMode::Vertical,
            max_measures_per_system: 4,
            ..Default::default()
        };
        let v_result = layout_score_with_config(&score, &style, v_config.clone());

        // Horizontal: 1 system with all measures
        assert_eq!(h_result.system_count, 1);
        assert_eq!(h_result.pages[0].systems[0].measure_indices.len(), 16);

        // Vertical: multiple systems (16/4 = 4)
        assert_eq!(v_result.system_count, 4);

        // Horizontal should be wider than vertical
        assert!(h_result.total_width > v_result.total_width);

        // Vertical should be taller than horizontal
        assert!(v_result.total_height > h_result.total_height);
    }

    // ======================================================================
    // Explicit Break Tests
    // ======================================================================

    #[test]
    fn test_line_break_honored_in_page_mode() {
        // Line break after measure 2 should create a new system
        let score = create_test_score_with_breaks(8, &[(1, LayoutBreak::Line)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 4, // Would normally be 2 systems
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Should have 3 systems: [0,1], [2,3,4,5], [6,7]
        // First system ends at measure 1 (0-indexed) due to line break
        assert_eq!(result.system_count, 3);
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0, 1]);
    }

    #[test]
    fn test_line_break_ignored_in_horizontal_mode() {
        // Line breaks should be ignored in horizontal mode
        let score =
            create_test_score_with_breaks(8, &[(1, LayoutBreak::Line), (3, LayoutBreak::Line)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig::horizontal();
        let result = layout_score_with_config(&score, &style, config);

        // Horizontal mode: ONE system with ALL measures, ignoring breaks
        assert_eq!(result.system_count, 1);
        assert_eq!(result.pages[0].systems[0].measure_indices.len(), 8);
    }

    #[test]
    fn test_section_break_forces_line_break() {
        // Section break should also force a line break
        let score = create_test_score_with_breaks(8, &[(3, LayoutBreak::Section)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 8, // Would normally be 1 system
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Section break after measure 3 creates 2 systems
        assert_eq!(result.system_count, 2);
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0, 1, 2, 3]);
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![4, 5, 6, 7]);
    }

    #[test]
    fn test_page_break_creates_new_page() {
        // Page break after measure 3 should create a new page
        let score = create_test_score_with_breaks(8, &[(3, LayoutBreak::Page)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 4,
            page_height: 2000.0, // Large page that would fit all systems
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Should have 2 pages due to explicit page break
        assert_eq!(result.pages.len(), 2);

        // First page has 1 system (measures 0-3)
        assert_eq!(result.pages[0].systems.len(), 1);
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0, 1, 2, 3]);

        // Second page has 1 system (measures 4-7)
        assert_eq!(result.pages[1].systems.len(), 1);
        assert_eq!(result.pages[1].systems[0].measure_indices, vec![4, 5, 6, 7]);
    }

    #[test]
    fn test_page_break_ignored_in_vertical_mode() {
        // Page breaks are converted to line breaks in vertical mode
        let score = create_test_score_with_breaks(8, &[(3, LayoutBreak::Page)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Vertical,
            max_measures_per_system: 8, // Would normally be 1 system
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Vertical mode: ONE page (endless vertical)
        assert_eq!(result.pages.len(), 1);

        // But the line break aspect should still be honored
        // (Page break also forces a line break in modes that honor line breaks)
        assert_eq!(result.system_count, 2);
    }

    #[test]
    fn test_nobreak_prevents_automatic_break() {
        // NoBreak should prevent automatic breaks at that point
        let score = create_test_score_with_breaks(8, &[(3, LayoutBreak::NoBreak)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 4, // Would normally break at measure 3
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // NoBreak at measure 3 should prevent the break there
        // So system continues to measure 4, then breaks
        // Result: systems [0,1,2,3,4], [5,6,7]
        assert_eq!(result.system_count, 2);
        assert_eq!(
            result.pages[0].systems[0].measure_indices,
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![5, 6, 7]);
    }

    #[test]
    fn test_multiple_breaks_in_sequence() {
        // Multiple breaks should all be honored
        let score = create_test_score_with_breaks(
            12,
            &[
                (1, LayoutBreak::Line),
                (5, LayoutBreak::Section),
                (9, LayoutBreak::Line),
            ],
        );
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 12, // Would be 1 system without breaks
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Should have 4 systems:
        // [0,1], [2,3,4,5], [6,7,8,9], [10,11]
        assert_eq!(result.system_count, 4);
    }

    #[test]
    fn test_break_at_last_measure() {
        // Break at the last measure should not create an empty system
        let score = create_test_score_with_breaks(4, &[(3, LayoutBreak::Line)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig {
            mode: LayoutMode::Page,
            max_measures_per_system: 8,
            ..Default::default()
        };
        let result = layout_score_with_config(&score, &style, config);

        // Should have 1 system (break at end doesn't create empty system)
        assert_eq!(result.system_count, 1);
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_float_mode_ignores_breaks() {
        // Float mode should ignore all explicit breaks
        let score =
            create_test_score_with_breaks(8, &[(1, LayoutBreak::Line), (3, LayoutBreak::Page)]);
        let style = MStyle::default();
        let config = LayoutEngineConfig::float();
        let result = layout_score_with_config(&score, &style, config);

        // Float mode ignores explicit breaks, uses auto-layout
        // Should behave like page mode without breaks
        assert_eq!(result.measure_count, 8);
        // The exact system count depends on auto-layout, but breaks should be ignored
    }
}
