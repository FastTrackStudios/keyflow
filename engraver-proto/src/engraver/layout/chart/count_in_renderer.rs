//! Count-in mini-snippet renderer.
//!
//! Renders a scaled-down count-in display for the header area, positioned next to
//! the tempo indicator. This replaces inline count-in measures on the first system.
//!
//! The snippet uses the same proportions as normal staffs, scaled down uniformly:
//! - 5 staff lines at scaled spatium spacing (half thickness for readability)
//! - Barlines at start, between measures, and at end
//! - Properly sized slash noteheads using SMuFL glyphs
//! - Beat numbers below slashes (1, 2, 3, 4)

use super::page_rendering::draw_staff_lines_with_thickness;
use super::types::slash_glyph_for_ticks;
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::{FontStyle, FontWeight, PaintCommand, TextAnchor};
use kurbo::Point;
use vello::peniko::Color;

/// Configuration for count-in snippet rendering.
#[derive(Debug, Clone)]
pub struct CountInSnippetConfig {
    /// Number of beats per measure (e.g., 4 for 4/4 time).
    pub beats_per_measure: u8,
    /// Beat unit (denominator, e.g., 4 for quarter note).
    pub beat_unit: u8,
    /// Number of count-in measures (typically 1 or 2).
    pub num_measures: usize,
    /// Base spatium from the main chart (staff space in points).
    pub spatium: f64,
    /// Scale factor for the snippet (e.g., 0.6 for 60% size).
    /// Applied uniformly to all dimensions to maintain proportions.
    pub scale: f64,
}

impl Default for CountInSnippetConfig {
    fn default() -> Self {
        Self {
            beats_per_measure: 4,
            beat_unit: 4,
            num_measures: 1,
            spatium: 5.0,
            scale: 0.6, // 60% of normal size
        }
    }
}

/// Geometry of a single count-in beat, for creating `BeatPosition` entries.
#[derive(Debug, Clone)]
pub struct CountInBeatGeometry {
    /// X position of this beat (in page coordinates).
    pub x: f64,
    /// Width of this beat slot.
    pub width: f64,
    /// Y position of the staff top.
    pub staff_y: f64,
    /// Staff height.
    pub staff_height: f64,
    /// Measure index within the count-in (0-indexed).
    pub measure_index: usize,
    /// Beat index within the measure (0-indexed).
    pub beat_index: usize,
    /// SMuFL glyph codepoint for the slash notehead.
    pub glyph_codepoint: char,
    /// Glyph size in spatiums (scaled).
    pub glyph_size: f64,
    /// Glyph Y center position.
    pub glyph_y: f64,
}

/// Result of rendering a count-in snippet.
#[derive(Debug)]
pub struct CountInSnippetResult {
    /// The rendered scene node.
    pub node: SceneNode,
    /// Total width consumed by the snippet.
    pub width: f64,
    /// Total height of the snippet.
    pub height: f64,
    /// Beat geometry for each beat in the count-in, for cursor highlighting.
    pub beat_geometries: Vec<CountInBeatGeometry>,
}

/// Render a count-in snippet at the given position.
///
/// Uses the same proportions as normal staff rendering, uniformly scaled:
/// - All dimensions derived from `spatium * scale`
/// - Maintains correct glyph-to-staff relationships
/// - Slash codepoints from `slash_glyph_for_ticks()`
/// - Barlines at start, between measures, and at end
/// - Staff lines at half thickness for better readability at small sizes
///
/// # Arguments
/// * `config` - Configuration for the count-in snippet
/// * `position` - Top-left position for the snippet
///
/// # Returns
/// The rendered scene node with width/height information.
#[must_use]
pub fn render_count_in_snippet(
    config: &CountInSnippetConfig,
    position: Point,
) -> CountInSnippetResult {
    let mut commands = Vec::new();

    // Scale the spatium - this ensures all proportions remain correct
    let spatium = config.spatium * config.scale;

    // Staff line thickness: use standard ratio with base spatium
    let staff_line_thickness = config.spatium * 0.05;

    // Barline thickness: same base value as main sheet (0.5), scaled proportionally
    // This keeps the same visual weight relative to the scaled staff
    let barline_thickness = 0.5 * config.scale;

    // Layout constants - same proportions as main chart, using scaled spatium
    let beat_spacing = spatium * 4.0; // Space between beat centers
    let staff_height = spatium * 4.0; // 5 lines, 4 spaces
    let beat_number_offset = spatium * 2.0; // Below staff

    // Measure layout: beats positioned with padding at measure boundaries
    // Slash glyphs are wide diagonal shapes that extend significantly to the right,
    // so we need substantial end padding to prevent touching the barline
    let start_padding = beat_spacing * 0.5; // Padding before first beat
    let end_padding = beat_spacing * 1.0; // Padding after last beat
    let beats_content_width = beat_spacing * (config.beats_per_measure - 1) as f64;
    let measure_width = start_padding + beats_content_width + end_padding;

    // Total width: all measures side by side
    let staff_width = measure_width * config.num_measures as f64;
    // Label height + padding + staff + beat numbers below
    let label_height = spatium * 1.8 + spatium * 0.5; // label_font_size + label_padding
    let total_height = label_height + staff_height + beat_number_offset + spatium * 1.5;

    // Font size for beat numbers - proportional to scaled spatium
    let beat_number_font_size = spatium * 2.0;

    // Font size for "Count-In" label - small text above the staff
    let label_font_size = spatium * 1.8;
    let label_padding = spatium * 0.5; // Space between label and staff

    // Position calculations
    // Leave room above for the "Count-In" label
    let label_y = position.y + label_font_size;
    let staff_top_y = label_y + label_padding;
    let staff_center_y = staff_top_y + staff_height / 2.0; // Center line of staff
    let beat_number_y = staff_top_y + staff_height + beat_number_font_size;
    let staff_start_x = position.x;

    // Render "Count-In" label above the staff, aligned to the left
    commands.push(PaintCommand::Text {
        text: "Count-In".to_string(),
        font_family: "FreeSans".to_string(),
        font_size: label_font_size,
        position: Point::new(staff_start_x, label_y),
        color: Color::BLACK,
        anchor: TextAnchor::Start,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    });

    // Draw 5 staff lines
    commands.extend(draw_staff_lines_with_thickness(
        staff_start_x,
        staff_top_y,
        staff_width,
        spatium,
        staff_line_thickness,
    ));

    // Quarter note = 480 ticks (standard resolution)
    let slash_codepoint = slash_glyph_for_ticks(480);

    // Draw barlines at measure boundaries: start, between measures, and end
    // Uses the same rendering approach as draw_barline() in page_rendering.rs,
    // but with thickness scaled to match the snippet scale
    // Barline positions: 0, measure_width, 2*measure_width, ..., num_measures*measure_width
    for i in 0..=config.num_measures {
        let barline_x = staff_start_x + measure_width * i as f64;
        commands.push(PaintCommand::line(
            Point::new(barline_x, staff_top_y),
            Point::new(barline_x, staff_top_y + staff_height),
            Color::BLACK,
            barline_thickness,
        ));
    }

    // Collect beat geometry for cursor highlighting
    let mut beat_geometries = Vec::new();

    // Render slashes and beat numbers for each measure
    for measure_idx in 0..config.num_measures {
        let measure_start_x = staff_start_x + measure_width * measure_idx as f64;

        // Determine beat text style based on measure position
        // For 2-measure count-in: first measure shows half-time (1, _, 2, _)
        // For last/only measure: full count (1, 2, 3, 4)
        let is_half_time = config.num_measures == 2 && measure_idx == 0;

        // Render beats within this measure
        for beat_idx in 0..config.beats_per_measure as usize {
            // Position beat with start padding, then evenly spaced
            let beat_x = measure_start_x + start_padding + beat_spacing * beat_idx as f64;

            // Slash glyph - using scaled spatium for size (maintains proportion)
            commands.push(PaintCommand::Glyph {
                codepoint: slash_codepoint,
                position: Point::new(beat_x, staff_center_y),
                size: spatium,
                color: Color::BLACK,
            });

            // Record beat geometry for cursor positioning
            beat_geometries.push(CountInBeatGeometry {
                x: beat_x,
                width: beat_spacing,
                staff_y: staff_top_y,
                staff_height,
                measure_index: measure_idx,
                beat_index: beat_idx,
                glyph_codepoint: slash_codepoint,
                glyph_size: spatium,
                glyph_y: staff_center_y,
            });

            // Beat number below slash
            let beat_text = if is_half_time {
                // Half-time: only show on beats 1 and 3
                match beat_idx {
                    0 => Some("1".to_string()),
                    2 => Some("2".to_string()),
                    _ => None,
                }
            } else {
                // Full count: show all beats
                Some((beat_idx + 1).to_string())
            };

            if let Some(text) = beat_text {
                commands.push(PaintCommand::Text {
                    text,
                    font_family: "FreeSans".to_string(),
                    font_size: beat_number_font_size,
                    position: Point::new(beat_x, beat_number_y),
                    color: Color::BLACK,
                    anchor: TextAnchor::Middle,
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                });
            }
        }
    }

    let node = SceneNode::anonymous_leaf(commands);

    CountInSnippetResult {
        node,
        width: staff_width,
        height: total_height,
        beat_geometries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CountInSnippetConfig::default();
        assert_eq!(config.beats_per_measure, 4);
        assert_eq!(config.beat_unit, 4);
        assert_eq!(config.num_measures, 1);
        assert!((config.spatium - 5.0).abs() < f64::EPSILON);
        assert!((config.scale - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_render_single_measure() {
        let config = CountInSnippetConfig {
            beats_per_measure: 4,
            beat_unit: 4,
            num_measures: 1,
            spatium: 5.0,
            scale: 0.6,
        };
        let result = render_count_in_snippet(&config, Point::new(0.0, 0.0));
        assert!(result.width > 0.0);
        assert!(result.height > 0.0);
    }

    #[test]
    fn test_render_two_measures() {
        let config = CountInSnippetConfig {
            beats_per_measure: 4,
            beat_unit: 4,
            num_measures: 2,
            spatium: 5.0,
            scale: 0.6,
        };
        let result = render_count_in_snippet(&config, Point::new(0.0, 0.0));
        // Two measures should be wider than one
        let single_config = CountInSnippetConfig {
            num_measures: 1,
            ..config.clone()
        };
        let single_result = render_count_in_snippet(&single_config, Point::new(0.0, 0.0));
        assert!(result.width > single_result.width);
    }

    #[test]
    fn test_scale_affects_dimensions() {
        let base_config = CountInSnippetConfig {
            beats_per_measure: 4,
            beat_unit: 4,
            num_measures: 1,
            spatium: 5.0,
            scale: 1.0, // Full size
        };
        let scaled_config = CountInSnippetConfig {
            scale: 0.5, // Half size
            ..base_config.clone()
        };

        let full_result = render_count_in_snippet(&base_config, Point::new(0.0, 0.0));
        let scaled_result = render_count_in_snippet(&scaled_config, Point::new(0.0, 0.0));

        // Scaled result should be approximately half the width and height
        // (not exact due to barline padding adjustments)
        assert!(scaled_result.width < full_result.width);
        assert!(scaled_result.height < full_result.height);
    }

    #[test]
    fn test_uses_correct_slash_glyph() {
        // Quarter note = 480 ticks should return a filled slash
        let codepoint = slash_glyph_for_ticks(480);
        // The codepoint should be a valid SMuFL slash notehead
        assert!(codepoint as u32 >= 0xE100); // SMuFL noteheads range
    }

    #[test]
    fn test_barlines_rendered() {
        let config = CountInSnippetConfig {
            beats_per_measure: 4,
            beat_unit: 4,
            num_measures: 2,
            spatium: 5.0,
            scale: 1.0,
        };
        let result = render_count_in_snippet(&config, Point::new(0.0, 0.0));

        // Count line commands (barlines + staff lines)
        // Should have: 5 staff lines + 3 barlines (start, middle, end) = 8 lines
        let line_count = result
            .node
            .commands
            .iter()
            .filter(|cmd| matches!(cmd, PaintCommand::Line { .. }))
            .count();
        assert_eq!(line_count, 8, "Expected 5 staff lines + 3 barlines");
    }
}
