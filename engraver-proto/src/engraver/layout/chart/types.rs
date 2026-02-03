//! Chart layout types and data structures.
//!
//! This module contains the core types used by the chart layout engine,
//! including result types, position data, melody processing, and push spillback.

use std::collections::HashMap;

use crate::engraver::layout::orchestrator::{PageLayout, PageMargins};
use crate::engraver::model::{DurationKind, NoteHead};
use crate::engraver::notation::Duration;
use crate::engraver::scene::node::SceneNode;

// ============================================================================
// Layout Mode
// ============================================================================

/// Layout mode for chart rendering.
#[derive(Debug, Clone)]
pub enum LayoutMode {
    /// Paginated layout with discrete pages and page breaks.
    Paginated { page_width: f64, page_height: f64 },
    /// Continuous scroll with unbounded vertical layout.
    ContinuousScroll { width: f64 },
    /// Snippet mode: fits content height automatically.
    /// For titleless charts that should be compact without whitespace.
    Snippet { page_width: f64 },
}

impl LayoutMode {
    /// Create a snippet layout mode for the given width.
    /// Height will be calculated to fit the content.
    #[must_use]
    pub fn snippet(width: f64) -> Self {
        Self::Snippet { page_width: width }
    }

    /// Create a snippet layout mode for Letter-size width (8.5").
    #[must_use]
    pub fn snippet_letter() -> Self {
        Self::Snippet { page_width: 612.0 }
    }

    /// Get the page width for this layout mode.
    #[must_use]
    pub fn page_width(&self) -> f64 {
        match self {
            LayoutMode::Paginated { page_width, .. } => *page_width,
            LayoutMode::ContinuousScroll { width } => *width,
            LayoutMode::Snippet { page_width } => *page_width,
        }
    }
}

impl Default for LayoutMode {
    fn default() -> Self {
        // Default to Letter size pages
        Self::Paginated {
            page_width: 612.0,  // 8.5" in points
            page_height: 792.0, // 11" in points
        }
    }
}

// ============================================================================
// Beat Position
// ============================================================================

/// Beat-level position data for precise cursor/highlight positioning.
///
/// This stores the actual computed X position for each beat segment,
/// accounting for proportional spacing (32nd notes get more space than whole notes).
#[derive(Debug, Clone)]
pub struct BeatPosition {
    /// Page number (1-indexed).
    pub page: u32,
    /// System index on this page (0-indexed).
    pub system: usize,
    /// Global measure index (0-indexed across entire chart).
    pub measure: usize,
    /// Beat/segment index within measure (0-indexed).
    pub beat: usize,
    /// Tick position relative to measure start (480 ticks per quarter note).
    pub tick: i32,
    /// Duration in ticks.
    pub duration_ticks: i32,
    /// Absolute tick position from song start (for tempo-independent lookup).
    pub absolute_tick: i64,
    /// Absolute X position on page (including margins and measure offset).
    pub x: f64,
    /// Width of this beat segment.
    pub width: f64,
    /// Y position of the staff (top line).
    pub staff_y: f64,
    /// Staff height (for cursor rendering).
    pub staff_height: f64,
    /// Time in seconds from song start (pre-computed with layout tempo).
    pub time_start: f64,
    /// Time in seconds when this beat ends (pre-computed with layout tempo).
    pub time_end: f64,
    /// Glyph codepoint for the primary element at this beat (for highlight rendering).
    /// This is typically a notehead (slash, normal, etc.) or rest glyph.
    pub glyph_codepoint: Option<char>,
    /// Size of the glyph in spatiums (for scaling the outline).
    pub glyph_size: f64,
    /// Y position of the glyph center (for notehead/rest vertical positioning).
    pub glyph_y: f64,
    /// Whether this note has a stem.
    pub has_stem: bool,
    /// Stem direction: true = up, false = down. Only meaningful if has_stem is true.
    pub stem_up: bool,
    /// Number of flags (0 for quarter and longer, 1 for 8th, 2 for 16th, 3 for 32nd).
    pub flag_count: u8,
}

impl BeatPosition {
    /// Check if a given time falls within this beat.
    #[must_use]
    pub fn contains_time(&self, time: f64) -> bool {
        time >= self.time_start && time < self.time_end
    }

    /// Check if a given absolute tick falls within this beat.
    #[must_use]
    pub fn contains_tick(&self, tick: i64) -> bool {
        tick >= self.absolute_tick && tick < self.absolute_tick + self.duration_ticks as i64
    }

    /// Get interpolated X position for a time within this beat.
    #[must_use]
    pub fn x_at_time(&self, time: f64) -> f64 {
        if self.time_end <= self.time_start {
            return self.x;
        }
        let progress =
            ((time - self.time_start) / (self.time_end - self.time_start)).clamp(0.0, 1.0);
        self.x + self.width * progress
    }

    /// Get interpolated X position for an absolute tick within this beat.
    /// This is tempo-independent and provides smooth cursor movement.
    #[must_use]
    pub fn x_at_tick(&self, tick: i64) -> f64 {
        if self.duration_ticks <= 0 {
            return self.x;
        }
        let progress =
            ((tick - self.absolute_tick) as f64 / self.duration_ticks as f64).clamp(0.0, 1.0);
        self.x + self.width * progress
    }
}

// ============================================================================
// Slash Glyph Helper
// ============================================================================

/// Get the appropriate slash notehead glyph codepoint for a given duration in ticks.
///
/// Uses the standard 480 ticks per quarter note resolution:
/// - 1920 ticks = whole note
/// - 960 ticks = half note
/// - 480 ticks = quarter note
/// - 240 ticks = eighth note
/// - etc.
#[must_use]
pub fn slash_glyph_for_ticks(ticks: i32) -> char {
    let kind = match ticks {
        t if t >= 1920 => DurationKind::Whole,
        t if t >= 960 => DurationKind::Half,
        _ => DurationKind::Quarter, // Quarter and shorter all use filled slash
    };
    NoteHead::slash_for_duration(kind).smufl_codepoint()
}

// ============================================================================
// Chart Layout Result
// ============================================================================

/// Result of chart layout calculation.
#[derive(Debug, Clone)]
pub struct ChartLayoutResult {
    /// Root scene node containing all rendered content.
    pub scene: SceneNode,
    /// Page layouts (for paginated mode).
    pub pages: Vec<PageLayout>,
    /// Total height of content (for continuous scroll).
    pub total_height: f64,
    /// Total width of content.
    pub total_width: f64,
    /// Beat-level positions for cursor/highlight rendering.
    /// Sorted by time_start for efficient binary search.
    pub beat_positions: Vec<BeatPosition>,
}

impl ChartLayoutResult {
    /// Get layout metrics for a specific page.
    ///
    /// Returns detailed spacing information for debugging and verification.
    pub fn page_metrics(&self, page_number: u32) -> Option<PageLayoutMetrics> {
        let page = self.pages.iter().find(|p| p.number == page_number)?;
        Some(PageLayoutMetrics::from_page(page))
    }

    /// Get layout metrics for all pages.
    pub fn all_page_metrics(&self) -> Vec<PageLayoutMetrics> {
        self.pages
            .iter()
            .map(PageLayoutMetrics::from_page)
            .collect()
    }

    /// Find the beat position at a given time.
    ///
    /// Uses binary search for efficient lookup when beat_positions is sorted by time_start.
    #[must_use]
    pub fn beat_at_time(&self, time: f64) -> Option<&BeatPosition> {
        // Binary search for the beat containing this time
        let idx = self.beat_positions.partition_point(|b| b.time_end <= time);
        self.beat_positions
            .get(idx)
            .filter(|b| b.contains_time(time))
    }

    /// Get the interpolated X position and page for a given time.
    ///
    /// Returns (page, x, y, height) for cursor rendering.
    #[must_use]
    pub fn cursor_position_at_time(&self, time: f64) -> Option<(u32, f64, f64, f64)> {
        let beat = self.beat_at_time(time)?;
        let x = beat.x_at_time(time);
        Some((beat.page, x, beat.staff_y, beat.staff_height))
    }

    /// Get all beat positions for a specific measure.
    #[must_use]
    pub fn beats_in_measure(&self, measure_index: usize) -> Vec<&BeatPosition> {
        self.beat_positions
            .iter()
            .filter(|b| b.measure == measure_index)
            .collect()
    }

    /// Find the beat position at a given absolute tick.
    ///
    /// Uses binary search for efficient lookup. This is tempo-independent.
    #[must_use]
    pub fn beat_at_tick(&self, tick: i64) -> Option<&BeatPosition> {
        // Binary search for the beat containing this tick
        let idx = self
            .beat_positions
            .partition_point(|b| b.absolute_tick + b.duration_ticks as i64 <= tick);
        self.beat_positions
            .get(idx)
            .filter(|b| b.contains_tick(tick))
    }

    /// Get the interpolated X position and page for a given absolute tick.
    ///
    /// Returns (page, x, y, height) for cursor rendering. This is tempo-independent.
    #[must_use]
    pub fn cursor_position_at_tick(&self, tick: i64) -> Option<(u32, f64, f64, f64)> {
        let beat = self.beat_at_tick(tick)?;
        let x = beat.x_at_tick(tick);
        Some((beat.page, x, beat.staff_y, beat.staff_height))
    }

    /// Get the total duration of the chart in ticks.
    #[must_use]
    pub fn total_ticks(&self) -> i64 {
        self.beat_positions
            .last()
            .map(|b| b.absolute_tick + b.duration_ticks as i64)
            .unwrap_or(0)
    }

    /// Get all beat positions on a specific page.
    #[must_use]
    pub fn beats_on_page(&self, page: u32) -> Vec<&BeatPosition> {
        self.beat_positions
            .iter()
            .filter(|b| b.page == page)
            .collect()
    }
}

// ============================================================================
// Page Layout Metrics
// ============================================================================

/// Layout metrics for a single page - used for debugging and verification.
///
/// Similar to LilyPond's debug mode, this provides detailed information about
/// system placement on a page.
#[derive(Debug, Clone)]
pub struct PageLayoutMetrics {
    /// Page number (1-indexed).
    pub page_number: u32,
    /// Page dimensions.
    pub page_width: f64,
    pub page_height: f64,
    /// Number of systems on this page.
    pub system_count: usize,
    /// Y positions of each system (from top of page).
    pub system_y_positions: Vec<f64>,
    /// Spacing between consecutive systems.
    pub inter_system_spacing: Vec<f64>,
    /// Distance from top margin to first system.
    pub top_margin_to_first_system: f64,
    /// Distance from last system bottom to page bottom.
    pub last_system_to_bottom: f64,
    /// Total content height (all systems + spacing).
    pub content_height: f64,
    /// Available content height (page height - top margin - bottom margin).
    pub available_height: f64,
    /// Page margins.
    pub margins: PageMargins,
}

impl PageLayoutMetrics {
    /// Create metrics from a PageLayout.
    pub fn from_page(page: &PageLayout) -> Self {
        let system_count = page.systems.len();
        let system_y_positions: Vec<f64> = page.systems.iter().map(|s| s.y).collect();

        // Calculate inter-system spacing
        let inter_system_spacing: Vec<f64> = if system_count > 1 {
            page.systems
                .windows(2)
                .map(|pair| {
                    let first_bottom = pair[0].y + pair[0].height;
                    let second_top = pair[1].y;
                    second_top - first_bottom
                })
                .collect()
        } else {
            Vec::new()
        };

        // Calculate distances
        let top_margin_to_first_system = if let Some(first) = page.systems.first() {
            first.y - page.margins.top
        } else {
            0.0
        };

        let last_system_to_bottom = if let Some(last) = page.systems.last() {
            let last_bottom = last.y + last.height;
            page.height - page.margins.bottom - last_bottom
        } else {
            page.height - page.margins.top - page.margins.bottom
        };

        let content_height =
            if let (Some(first), Some(last)) = (page.systems.first(), page.systems.last()) {
                (last.y + last.height) - first.y
            } else {
                0.0
            };

        let available_height = page.height - page.margins.top - page.margins.bottom;

        Self {
            page_number: page.number,
            page_width: page.width,
            page_height: page.height,
            system_count,
            system_y_positions,
            inter_system_spacing,
            top_margin_to_first_system,
            last_system_to_bottom,
            content_height,
            available_height,
            margins: page.margins,
        }
    }

    /// Print a debug summary of the page layout (similar to LilyPond debug mode).
    pub fn print_debug(&self) {
        println!("=== Page {} Layout Metrics ===", self.page_number);
        println!(
            "Page size: {:.1} × {:.1} points",
            self.page_width, self.page_height
        );
        println!(
            "Margins: top={:.1}, bottom={:.1}, left={:.1}, right={:.1}",
            self.margins.top, self.margins.bottom, self.margins.left, self.margins.right
        );
        println!("Available height: {:.1} points", self.available_height);
        println!("Systems: {}", self.system_count);
        println!();

        if !self.system_y_positions.is_empty() {
            println!("System positions (from page top):");
            for (i, y) in self.system_y_positions.iter().enumerate() {
                println!("  System {}: y={:.1}", i + 1, y);
            }
            println!();
        }

        if !self.inter_system_spacing.is_empty() {
            println!("Inter-system spacing:");
            for (i, spacing) in self.inter_system_spacing.iter().enumerate() {
                println!("  Between {} and {}: {:.1} points", i + 1, i + 2, spacing);
            }
            println!();
        }

        println!(
            "Top margin to first system: {:.1} points",
            self.top_margin_to_first_system
        );
        println!(
            "Last system to bottom: {:.1} points",
            self.last_system_to_bottom
        );
        println!("Total content height: {:.1} points", self.content_height);
        println!(
            "Remaining space: {:.1} points",
            self.available_height - self.content_height
        );
        println!();
    }

    /// Check if the page has reasonable spacing.
    ///
    /// Returns a list of warnings if spacing issues are detected.
    pub fn check_spacing(&self, min_system_spacing: f64, max_system_spacing: f64) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check inter-system spacing
        for (i, &spacing) in self.inter_system_spacing.iter().enumerate() {
            if spacing < min_system_spacing {
                warnings.push(format!(
                    "Systems {} and {} are too close: {:.1} points (min: {:.1})",
                    i + 1,
                    i + 2,
                    spacing,
                    min_system_spacing
                ));
            }
            if spacing > max_system_spacing {
                warnings.push(format!(
                    "Systems {} and {} are too far apart: {:.1} points (max: {:.1})",
                    i + 1,
                    i + 2,
                    spacing,
                    max_system_spacing
                ));
            }
        }

        // Check bottom margin
        if self.last_system_to_bottom < 0.0 {
            warnings.push(format!(
                "Content extends past bottom margin by {:.1} points",
                -self.last_system_to_bottom
            ));
        }

        // Check if too much empty space at bottom
        let excessive_bottom = self.available_height * 0.3; // More than 30% empty
        if self.last_system_to_bottom > excessive_bottom {
            warnings.push(format!(
                "Excessive empty space at bottom: {:.1} points ({:.0}% of available height)",
                self.last_system_to_bottom,
                (self.last_system_to_bottom / self.available_height) * 100.0
            ));
        }

        warnings
    }
}

// ============================================================================
// Melody Spillover Types
// ============================================================================

/// A segment of a melody note that fits within a single measure.
///
/// When a melody spans multiple measures, notes are split at barlines.
/// Each segment represents the portion of a note (or complete note) that
/// fits within one measure.
#[derive(Debug, Clone)]
pub struct MelodyNoteSegment {
    /// The pitch (e.g., "C", "F#", "r" for rest)
    pub pitch: String,
    /// Duration of this segment in beats (quarter note = 1.0)
    pub beats: f64,
    /// True if this is dotted
    pub dotted: bool,
    /// True if this is a continuation from previous measure (needs incoming tie)
    pub tie_from_previous: bool,
    /// True if this continues into next measure (needs outgoing tie)
    pub tie_to_next: bool,
    /// True if this is a rest
    pub is_rest: bool,
}

impl MelodyNoteSegment {
    /// Convert beats to a Duration enum value
    pub fn to_duration(&self) -> Duration {
        // Map beats to duration (approximating to nearest standard duration)
        // For dotted notes, divide by 1.5 to get the base duration
        let base_beats = if self.dotted {
            self.beats / 1.5
        } else {
            self.beats
        };

        let base_duration = match base_beats {
            b if b >= 3.5 => Duration::Whole,
            b if b >= 1.75 => Duration::Half,
            b if b >= 0.875 => Duration::Quarter,
            b if b >= 0.4375 => Duration::Eighth,
            b if b >= 0.21875 => Duration::Sixteenth,
            _ => Duration::ThirtySecond,
        };

        // Return dotted version if appropriate
        if self.dotted {
            match base_duration {
                Duration::Half => Duration::DottedHalf,
                Duration::Quarter => Duration::DottedQuarter,
                Duration::Eighth => Duration::DottedEighth,
                Duration::Sixteenth => Duration::DottedSixteenth,
                // No DottedWhole or DottedThirtySecond in engraver
                other => other,
            }
        } else {
            base_duration
        }
    }
}

/// Preprocessed melody data for a measure.
///
/// Contains all melody note segments that should appear in this measure,
/// including segments that spilled over from previous measures.
#[derive(Debug, Clone, Default)]
pub struct MeasureMelodyData {
    /// Note segments to render in this measure
    pub segments: Vec<MelodyNoteSegment>,
    /// Total beats consumed by melody segments
    pub total_beats: f64,
}

impl MeasureMelodyData {
    /// Check if this measure has any melody content
    pub fn has_content(&self) -> bool {
        !self.segments.is_empty()
    }
}

/// Expands melodies across measure boundaries.
///
/// This processes all melodies in a chart section and distributes them
/// across measures, splitting notes at barlines and tracking ties.
pub fn expand_melodies_across_measures(
    section_measures: &[crate::chart::types::Measure],
    beats_per_measure: f64,
) -> HashMap<usize, MeasureMelodyData> {
    let mut result: HashMap<usize, MeasureMelodyData> = HashMap::new();

    for (measure_idx, measure) in section_measures.iter().enumerate() {
        if measure.melodies.is_empty() {
            continue;
        }

        tracing::debug!(
            "[melody-spillover] Measure {} has {} melodies",
            measure_idx,
            measure.melodies.len()
        );

        // Process each melody attached to this measure
        for melody in &measure.melodies {
            tracing::debug!(
                "[melody-spillover]   Processing melody with {} notes, total_beats: {:.2}",
                melody.notes.len(),
                melody.notes.iter().map(|n| n.duration_beats()).sum::<f64>()
            );
            let mut current_measure = measure_idx;
            let mut beats_remaining_in_measure = beats_per_measure;

            // Check if this measure already has melody content (from previous spillover)
            if let Some(existing) = result.get(&current_measure) {
                beats_remaining_in_measure = beats_per_measure - existing.total_beats;
            }

            for note in &melody.notes {
                let note_beats = note.duration_beats();
                let mut beats_to_place = note_beats;
                let mut is_first_segment = true;

                while beats_to_place > 0.001 {
                    // Get or create melody data for current measure
                    let measure_data = result.entry(current_measure).or_default();

                    // Check remaining capacity in this measure
                    let capacity = beats_per_measure - measure_data.total_beats;

                    if capacity <= 0.001 {
                        // Measure is full, move to next
                        current_measure += 1;
                        beats_remaining_in_measure = beats_per_measure;
                        continue;
                    }

                    // Determine how much of this note fits in current measure
                    let segment_beats = beats_to_place.min(capacity);
                    let is_last_segment = (beats_to_place - segment_beats) < 0.001;

                    // Create segment
                    let segment = MelodyNoteSegment {
                        pitch: note.pitch.clone(),
                        beats: segment_beats,
                        dotted: note.dotted && is_last_segment, // Only dot the final segment
                        tie_from_previous: !is_first_segment,
                        tie_to_next: !is_last_segment && !note.is_rest(),
                        is_rest: note.is_rest(),
                    };

                    measure_data.segments.push(segment);
                    measure_data.total_beats += segment_beats;

                    beats_to_place -= segment_beats;
                    is_first_segment = false;

                    // If we consumed all capacity, move to next measure for remaining beats
                    if measure_data.total_beats >= beats_per_measure - 0.001 {
                        current_measure += 1;
                        beats_remaining_in_measure = beats_per_measure;
                    }
                }
            }
        }
    }

    // Log summary of melody distribution
    for (measure_idx, data) in result.iter() {
        tracing::debug!(
            "[melody-spillover] Output measure {}: {} segments, {:.2} total beats",
            measure_idx,
            data.segments.len(),
            data.total_beats
        );
        for (i, seg) in data.segments.iter().enumerate() {
            tracing::debug!(
                "[melody-spillover]   Segment {}: pitch={}, beats={:.2}, tie_in={}, tie_out={}",
                i,
                seg.pitch,
                seg.beats,
                seg.tie_from_previous,
                seg.tie_to_next
            );
        }
    }

    result
}

// ============================================================================
// Push Spillback Types (Re-exported from chart::rhythm)
// ============================================================================
//
// The canonical spillback types and detection functions are now in
// crate::chart::rhythm. They are re-exported through this module's parent
// (engraver::layout::chart) for backward compatibility.
//
// See: crate::chart::rhythm::{Spillback, detect_push_spillbacks, detect_section_start_spillback}
