//! Page rendering utilities for chart layout.
//!
//! This module provides functions for drawing page elements:
//! staff lines, barlines, backgrounds, footers, and headers.

use super::count_in_renderer::{self, CountInBeatGeometry, CountInSnippetConfig};
use crate::engraver::layout::orchestrator::PageMargins;
use crate::engraver::layout::tlayout::BarlineType;
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::{FontStyle, FontWeight, PaintCommand, TextAnchor};
use kurbo::Rect;
use vello::peniko::Color;

/// Default staff line thickness as a ratio of spatium.
/// Standard value is 0.05 (5% of spatium).
pub const STAFF_LINE_THICKNESS_RATIO: f64 = 0.05;

/// Draw staff lines at the given position with default thickness.
///
/// Draws 5 horizontal lines at standard staff spacing.
/// Thickness is calculated as `spatium * STAFF_LINE_THICKNESS_RATIO`.
///
/// # Arguments
/// * `x` - Left edge X position
/// * `y` - Top line Y position
/// * `width` - Staff width
/// * `spatium` - Staff space (distance between lines)
#[must_use]
pub fn draw_staff_lines(x: f64, y: f64, width: f64, spatium: f64) -> Vec<PaintCommand> {
    let thickness = spatium * STAFF_LINE_THICKNESS_RATIO;
    draw_staff_lines_with_thickness(x, y, width, spatium, thickness)
}

/// Draw staff lines at the given position with custom thickness.
///
/// Draws 5 horizontal lines at standard staff spacing.
///
/// # Arguments
/// * `x` - Left edge X position
/// * `y` - Top line Y position
/// * `width` - Staff width
/// * `spatium` - Staff space (distance between lines)
/// * `thickness` - Line thickness in points
#[must_use]
pub fn draw_staff_lines_with_thickness(
    x: f64,
    y: f64,
    width: f64,
    spatium: f64,
    thickness: f64,
) -> Vec<PaintCommand> {
    let mut commands = Vec::new();

    for i in 0..5 {
        let line_y = y + i as f64 * spatium;
        commands.push(PaintCommand::line(
            kurbo::Point::new(x, line_y),
            kurbo::Point::new(x + width, line_y),
            Color::BLACK,
            thickness,
        ));
    }

    commands
}

/// Draw a barline at the given position.
///
/// Supports single, double, and end barlines.
#[must_use]
pub fn draw_barline(x: f64, y: f64, height: f64, barline_type: BarlineType) -> SceneNode {
    let mut commands = Vec::new();
    let thin = 0.5;
    let thick = 2.0;

    match barline_type {
        BarlineType::Single => {
            commands.push(PaintCommand::line(
                kurbo::Point::new(x, y),
                kurbo::Point::new(x, y + height),
                Color::BLACK,
                thin,
            ));
        }
        BarlineType::Double => {
            commands.push(PaintCommand::line(
                kurbo::Point::new(x - 3.0, y),
                kurbo::Point::new(x - 3.0, y + height),
                Color::BLACK,
                thin,
            ));
            commands.push(PaintCommand::line(
                kurbo::Point::new(x, y),
                kurbo::Point::new(x, y + height),
                Color::BLACK,
                thin,
            ));
        }
        BarlineType::End => {
            commands.push(PaintCommand::line(
                kurbo::Point::new(x - 5.0, y),
                kurbo::Point::new(x - 5.0, y + height),
                Color::BLACK,
                thin,
            ));
            commands.push(PaintCommand::line(
                kurbo::Point::new(x, y),
                kurbo::Point::new(x, y + height),
                Color::BLACK,
                thick,
            ));
        }
        _ => {
            commands.push(PaintCommand::line(
                kurbo::Point::new(x, y),
                kurbo::Point::new(x, y + height),
                Color::BLACK,
                thin,
            ));
        }
    }

    SceneNode::anonymous_leaf(commands)
}

/// Add page background (paper with shadow).
pub fn add_page_background(
    root: &mut SceneNode,
    page_x: f64,
    page_y: f64,
    page_width: f64,
    page_height: f64,
) {
    let shadow_offset = 4.0;

    // Shadow
    root.add_child(SceneNode::anonymous_leaf(vec![PaintCommand::filled_rect(
        Rect::new(
            page_x + shadow_offset,
            page_y + shadow_offset,
            page_x + shadow_offset + page_width,
            page_y + shadow_offset + page_height,
        ),
        Color::from_rgb8(180, 180, 180),
    )]));

    // White paper
    root.add_child(SceneNode::anonymous_leaf(vec![PaintCommand::filled_rect(
        Rect::new(page_x, page_y, page_x + page_width, page_y + page_height),
        Color::WHITE,
    )]));
}

/// Add simple snippet background (white box only, no shadow).
pub fn add_snippet_background(
    root: &mut SceneNode,
    page_x: f64,
    page_y: f64,
    page_width: f64,
    page_height: f64,
) {
    // Just white paper, no shadow
    root.add_child(SceneNode::anonymous_leaf(vec![PaintCommand::filled_rect(
        Rect::new(page_x, page_y, page_x + page_width, page_y + page_height),
        Color::WHITE,
    )]));
}

/// Add page footer with "Created with FastTrackStudio" text.
pub fn add_page_footer(
    root: &mut SceneNode,
    page_x: f64,
    page_y: f64,
    page_width: f64,
    page_height: f64,
) {
    let footer_text = "Created with FastTrackStudio";
    let font_size = 8.0;
    let footer_y = page_y + page_height - 15.0; // 15 points from bottom
    let center_x = page_x + page_width / 2.0;

    root.add_child(SceneNode::anonymous_leaf(vec![PaintCommand::Text {
        text: footer_text.to_string(),
        font_family: "sans-serif".to_string(),
        font_size,
        position: kurbo::Point::new(center_x, footer_y),
        color: Color::from_rgb8(160, 160, 160), // Light gray
        anchor: TextAnchor::Middle,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    }]));
}

/// Add title header section on first page (Title, Subtitle, Composer, etc.)
///
/// Returns the height consumed by the header for layout adjustment.
///
/// # Arguments
/// * `root` - Scene node to add header to
/// * `page_x` - Page X offset
/// * `page_y` - Page Y offset
/// * `page_width` - Page width
/// * `margins` - Page margins
/// * `spatium` - Staff space for glyph sizing
/// * `metadata` - Song metadata (title, composer, etc.)
/// * `tempo` - Optional tempo information
#[allow(clippy::too_many_arguments)]
pub fn add_title_header(
    root: &mut SceneNode,
    page_x: f64,
    page_y: f64,
    page_width: f64,
    margins: &PageMargins,
    spatium: f64,
    metadata: &crate::SongMetadata,
    tempo: Option<&crate::time::Tempo>,
) -> f64 {
    // If there's no title, skip the entire header (no part name, version, subtitle, etc.)
    if metadata.title.is_none() {
        return 0.0;
    }

    let mut commands = Vec::new();
    let center_x = page_x + page_width / 2.0;
    let right_x = page_x + page_width - margins.right;
    let left_x = page_x + margins.left;

    let frame_top_y = page_y + margins.top;
    let _line_spacing = 4.0;

    // Header padding for better visual breathing room
    let header_top_padding = 8.0; // Extra space before title

    // Title (large, bold, centered) - at the very top of the header
    // Title is wrapped in quotes like "Thriller"
    // Using FreeSans with bold weight for title
    let title_font_size = 34.0;
    let title_y = frame_top_y + header_top_padding + title_font_size; // Baseline position

    if let Some(ref title) = metadata.title {
        let quoted_title = format!("\"{}\"", title);
        commands.push(PaintCommand::Text {
            text: quoted_title,
            font_family: "FreeSans".to_string(),
            font_size: title_font_size,
            position: kurbo::Point::new(center_x, title_y),
            color: Color::BLACK,
            anchor: TextAnchor::Middle,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });
    }

    // Part name (left) and Composer/Artist (right) - aligned with vertical middle of title
    let title_vertical_middle = title_y - (title_font_size * 0.35);
    let composer_text = metadata.composer.as_ref().or(metadata.artist.as_ref());

    // Determine part name - default to "Master Rhythm" if not specified
    let part_name = metadata.part_name.as_deref().unwrap_or("Master Rhythm");
    let is_master_rhythm = part_name.eq_ignore_ascii_case("master rhythm");

    // For Master Rhythm, split into two lines and render in bold caps
    let part_name_lines: Vec<String> = if is_master_rhythm {
        vec!["MASTER".to_string(), "RHYTHM".to_string()]
    } else {
        vec![part_name.to_uppercase()]
    };

    let small_font_size = 11.0;
    let part_start_y = title_vertical_middle + small_font_size * 0.35;

    // Part name - left aligned, bold font
    for (i, line) in part_name_lines.iter().enumerate() {
        let line_y = part_start_y + (i as f64 * (small_font_size + 2.0));
        commands.push(PaintCommand::Text {
            text: line.clone(),
            font_family: "FreeSans".to_string(),
            font_size: small_font_size,
            position: kurbo::Point::new(left_x, line_y),
            color: Color::BLACK,
            anchor: TextAnchor::Start,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });
    }

    // Version - below part name, dark gray, not bold
    let version = metadata.version.unwrap_or(1);
    let version_y = part_start_y + (part_name_lines.len() as f64 * (small_font_size + 2.0));
    commands.push(PaintCommand::Text {
        text: format!("V{}", version),
        font_family: "sans-serif".to_string(),
        font_size: small_font_size,
        position: kurbo::Point::new(left_x, version_y),
        color: Color::from_rgb8(100, 100, 100),
        anchor: TextAnchor::Start,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    });

    // Tempo indicator - below version with padding
    let tempo_padding = 6.0;
    let tempo_y = version_y + small_font_size + tempo_padding;
    let tempo_font_size = 12.0;
    let tempo_symbol_ratio = 5.0 / 3.0;
    let tempo_symbol_pt = tempo_font_size * tempo_symbol_ratio;
    let tempo_glyph_size = tempo_symbol_pt / spatium;

    if let Some(tempo_val) = tempo {
        let glyph_width = tempo_glyph_size * spatium * 0.5;
        let glyph_y = tempo_y + tempo_symbol_pt * 0.15;

        commands.push(PaintCommand::Glyph {
            codepoint: '\u{eca5}', // metNoteQuarterUp
            position: kurbo::Point::new(left_x, glyph_y),
            size: tempo_glyph_size,
            color: Color::BLACK,
        });

        commands.push(PaintCommand::Text {
            text: format!("= {}", tempo_val.bpm as u32),
            font_family: "sans-serif".to_string(),
            font_size: tempo_font_size,
            position: kurbo::Point::new(left_x + glyph_width + 4.0, tempo_y),
            color: Color::BLACK,
            anchor: TextAnchor::Start,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });
    }

    // Composer/Artist - right aligned
    let mut current_y = part_start_y;
    if let Some(composer) = composer_text {
        let artists: Vec<&str> = composer.split(',').map(|s| s.trim()).collect();
        for (i, artist) in artists.iter().enumerate() {
            let line_y = part_start_y + (i as f64 * (small_font_size + 2.0));
            commands.push(PaintCommand::Text {
                text: artist.to_string(),
                font_family: "sans-serif".to_string(),
                font_size: small_font_size,
                position: kurbo::Point::new(right_x, line_y),
                color: Color::BLACK,
                anchor: TextAnchor::End,
                weight: FontWeight::Normal,
                style: FontStyle::Normal,
            });
        }

        let max_lines = part_name_lines.len().max(artists.len());
        current_y = part_start_y + ((max_lines - 1) as f64 * (small_font_size + 2.0));
    } else {
        current_y = part_start_y + ((part_name_lines.len() - 1) as f64 * (small_font_size + 2.0));
    }

    // Subtitle (medium, centered) - below the title with minimal spacing
    let subtitle_text = metadata
        .subtitle
        .clone()
        .unwrap_or_else(|| "Transcribed By: ______".to_string());
    let subtitle_font_size = 12.0;
    let subtitle_y = title_y + subtitle_font_size + 2.0;

    commands.push(PaintCommand::Text {
        text: subtitle_text,
        font_family: "sans-serif".to_string(),
        font_size: subtitle_font_size,
        position: kurbo::Point::new(center_x, subtitle_y),
        color: Color::from_rgb8(100, 100, 100),
        anchor: TextAnchor::Middle,
        weight: FontWeight::Normal,
        style: FontStyle::Italic,
    });

    // Calculate header height
    let left_column_bottom = if tempo.is_some() {
        tempo_y + tempo_font_size
    } else {
        version_y + small_font_size
    };
    current_y = subtitle_y.max(left_column_bottom);

    if !commands.is_empty() {
        root.add_child(SceneNode::anonymous_leaf(commands));
    }

    // Return total header height (from margin top to current_y, plus bottom padding)
    // This padding creates space between header content and the first system
    let header_bottom_padding = 35.0;
    let header_height = current_y - (page_y + margins.top) + header_bottom_padding;
    header_height.max(0.0)
}

/// Configuration for count-in snippet in header.
#[derive(Debug, Clone, Default)]
pub struct CountInHeaderConfig {
    /// Number of count-in measures (0 means no count-in).
    pub num_measures: usize,
    /// Beats per measure (numerator of time signature).
    pub beats_per_measure: u8,
    /// Beat unit (denominator of time signature).
    pub beat_unit: u8,
    /// Whether there's a pushed chord on beat 1 that spills into the count-in.
    /// When true, extra vertical space is added above the count-in for chord rendering.
    pub has_pushed_chord: bool,
}

/// Add title header section with optional count-in snippet.
///
/// This is an extended version of `add_title_header` that also renders
/// a compact count-in snippet next to the tempo indicator.
///
/// Returns `(header_height, count_in_beat_geometries)`.
#[allow(clippy::too_many_arguments)]
pub fn add_title_header_with_count_in(
    root: &mut SceneNode,
    page_x: f64,
    page_y: f64,
    page_width: f64,
    margins: &PageMargins,
    spatium: f64,
    metadata: &crate::SongMetadata,
    tempo: Option<&crate::time::Tempo>,
    count_in: Option<&CountInHeaderConfig>,
) -> (f64, Vec<CountInBeatGeometry>) {
    // If there's no title, skip the entire header (no part name, version, subtitle, etc.)
    if metadata.title.is_none() {
        return (0.0, Vec::new());
    }

    let mut commands = Vec::new();
    let center_x = page_x + page_width / 2.0;
    let right_x = page_x + page_width - margins.right;
    let left_x = page_x + margins.left;

    let frame_top_y = page_y + margins.top;

    // Header padding for better visual breathing room
    let header_top_padding = 8.0; // Extra space before title

    // Title (large, bold, centered) - at the very top of the header
    let title_font_size = 34.0;
    let title_y = frame_top_y + header_top_padding + title_font_size;

    if let Some(ref title) = metadata.title {
        let quoted_title = format!("\"{}\"", title);
        commands.push(PaintCommand::Text {
            text: quoted_title,
            font_family: "FreeSans".to_string(),
            font_size: title_font_size,
            position: kurbo::Point::new(center_x, title_y),
            color: Color::BLACK,
            anchor: TextAnchor::Middle,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });
    }

    // Part name (left) and Composer/Artist (right) - aligned with vertical middle of title
    let title_vertical_middle = title_y - (title_font_size * 0.35);
    let composer_text = metadata.composer.as_ref().or(metadata.artist.as_ref());

    // Determine part name - default to "Master Rhythm" if not specified
    let part_name = metadata.part_name.as_deref().unwrap_or("Master Rhythm");
    let is_master_rhythm = part_name.eq_ignore_ascii_case("master rhythm");

    // For Master Rhythm, split into two lines and render in bold caps
    let part_name_lines: Vec<String> = if is_master_rhythm {
        vec!["MASTER".to_string(), "RHYTHM".to_string()]
    } else {
        vec![part_name.to_uppercase()]
    };

    let small_font_size = 11.0;
    let part_start_y = title_vertical_middle + small_font_size * 0.35;

    // Part name - left aligned, bold font
    for (i, line) in part_name_lines.iter().enumerate() {
        let line_y = part_start_y + (i as f64 * (small_font_size + 2.0));
        commands.push(PaintCommand::Text {
            text: line.clone(),
            font_family: "FreeSans".to_string(),
            font_size: small_font_size,
            position: kurbo::Point::new(left_x, line_y),
            color: Color::BLACK,
            anchor: TextAnchor::Start,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });
    }

    // Version - below part name, dark gray, not bold
    let version = metadata.version.unwrap_or(1);
    let version_y = part_start_y + (part_name_lines.len() as f64 * (small_font_size + 2.0));
    commands.push(PaintCommand::Text {
        text: format!("V{}", version),
        font_family: "sans-serif".to_string(),
        font_size: small_font_size,
        position: kurbo::Point::new(left_x, version_y),
        color: Color::from_rgb8(100, 100, 100),
        anchor: TextAnchor::Start,
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    });

    // Tempo indicator - below version with padding
    // Extra space is added when there's a pushed chord that spills into the count-in
    let has_pushed_chord = count_in.map_or(false, |c| c.has_pushed_chord);
    let tempo_padding = if has_pushed_chord { 20.0 } else { 6.0 };
    let tempo_y = version_y + small_font_size + tempo_padding;
    let tempo_font_size = 12.0;
    let tempo_symbol_ratio = 5.0 / 3.0;
    let tempo_symbol_pt = tempo_font_size * tempo_symbol_ratio;
    let tempo_glyph_size = tempo_symbol_pt / spatium;

    // Track tempo text width for count-in positioning
    let mut tempo_end_x = left_x;

    if let Some(tempo_val) = tempo {
        let glyph_width = tempo_glyph_size * spatium * 0.5;
        let glyph_y = tempo_y + tempo_symbol_pt * 0.15;

        commands.push(PaintCommand::Glyph {
            codepoint: '\u{eca5}', // metNoteQuarterUp
            position: kurbo::Point::new(left_x, glyph_y),
            size: tempo_glyph_size,
            color: Color::BLACK,
        });

        // Estimate tempo text width (roughly 7 points per character)
        let tempo_text = format!("= {}", tempo_val.bpm as u32);
        let tempo_text_width = tempo_text.len() as f64 * 7.0;

        commands.push(PaintCommand::Text {
            text: tempo_text,
            font_family: "sans-serif".to_string(),
            font_size: tempo_font_size,
            position: kurbo::Point::new(left_x + glyph_width + 4.0, tempo_y),
            color: Color::BLACK,
            anchor: TextAnchor::Start,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });

        tempo_end_x = left_x + glyph_width + 4.0 + tempo_text_width;
    }

    // Add commands accumulated so far
    if !commands.is_empty() {
        root.add_child(SceneNode::anonymous_leaf(commands));
    }

    // Render count-in snippet next to tempo indicator
    let mut count_in_height = 0.0;
    let mut count_in_beat_geos = Vec::new();
    if let Some(config) = count_in {
        if config.num_measures > 0 {
            let snippet_config = CountInSnippetConfig {
                beats_per_measure: config.beats_per_measure,
                beat_unit: config.beat_unit,
                num_measures: config.num_measures,
                spatium,
                scale: 0.6, // 60% of normal size for header snippet
            };

            // Position snippet to the right of tempo indicator
            let snippet_x = tempo_end_x + 8.0; // Closer horizontal spacing

            // Vertically center the snippet's staff with the tempo text
            // Tempo text visual center is ~40% above its baseline
            // Snippet staff center is at snippet_y + staff_height/2
            let scaled_spatium = spatium * 0.6; // Match the snippet's scale
            let staff_height = scaled_spatium * 4.0;
            let tempo_text_center = tempo_y - tempo_font_size * 0.4;
            let snippet_y = tempo_text_center - staff_height / 2.0;

            let snippet_result = count_in_renderer::render_count_in_snippet(
                &snippet_config,
                kurbo::Point::new(snippet_x, snippet_y),
            );

            root.add_child(snippet_result.node);
            count_in_height = snippet_result.height;
            count_in_beat_geos = snippet_result.beat_geometries;
        }
    }

    // Composer/Artist - right aligned
    let mut commands = Vec::new();
    let mut current_y = part_start_y;
    if let Some(composer) = composer_text {
        let artists: Vec<&str> = composer.split(',').map(|s| s.trim()).collect();
        for (i, artist) in artists.iter().enumerate() {
            let line_y = part_start_y + (i as f64 * (small_font_size + 2.0));
            commands.push(PaintCommand::Text {
                text: artist.to_string(),
                font_family: "sans-serif".to_string(),
                font_size: small_font_size,
                position: kurbo::Point::new(right_x, line_y),
                color: Color::BLACK,
                anchor: TextAnchor::End,
                weight: FontWeight::Normal,
                style: FontStyle::Normal,
            });
        }

        let max_lines = part_name_lines.len().max(artists.len());
        current_y = part_start_y + ((max_lines - 1) as f64 * (small_font_size + 2.0));
    } else {
        current_y = part_start_y + ((part_name_lines.len() - 1) as f64 * (small_font_size + 2.0));
    }

    // Subtitle (medium, centered) - below the title with minimal spacing
    let subtitle_text = metadata
        .subtitle
        .clone()
        .unwrap_or_else(|| "Transcribed By: ______".to_string());
    let subtitle_font_size = 12.0;
    let subtitle_y = title_y + subtitle_font_size + 2.0;

    commands.push(PaintCommand::Text {
        text: subtitle_text,
        font_family: "sans-serif".to_string(),
        font_size: subtitle_font_size,
        position: kurbo::Point::new(center_x, subtitle_y),
        color: Color::from_rgb8(100, 100, 100),
        anchor: TextAnchor::Middle,
        weight: FontWeight::Normal,
        style: FontStyle::Italic,
    });

    if !commands.is_empty() {
        root.add_child(SceneNode::anonymous_leaf(commands));
    }

    // Calculate header height
    // Include count-in snippet height if present
    let left_column_bottom = if tempo.is_some() {
        (tempo_y + tempo_font_size).max(tempo_y + count_in_height)
    } else {
        version_y + small_font_size
    };
    current_y = subtitle_y.max(left_column_bottom);

    // Return total header height (from margin top to current_y, plus bottom padding)
    // This padding creates space between header content and the first system
    let header_bottom_padding = 35.0;
    let header_height = current_y - (page_y + margins.top) + header_bottom_padding;
    (header_height.max(0.0), count_in_beat_geos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_staff_lines_count() {
        let commands = draw_staff_lines(0.0, 0.0, 100.0, 10.0);
        assert_eq!(commands.len(), 5);
    }

    #[test]
    fn test_draw_barline_single() {
        let node = draw_barline(100.0, 0.0, 40.0, BarlineType::Single);
        // Single barline should have one command
        assert!(!node.children.is_empty() || !node.commands.is_empty());
    }

    #[test]
    fn test_draw_barline_double() {
        let node = draw_barline(100.0, 0.0, 40.0, BarlineType::Double);
        // Double barline should have two commands
        assert!(!node.children.is_empty() || !node.commands.is_empty());
    }

    #[test]
    fn test_draw_barline_end() {
        let node = draw_barline(100.0, 0.0, 40.0, BarlineType::End);
        // End barline should have two commands (thin + thick)
        assert!(!node.children.is_empty() || !node.commands.is_empty());
    }
}
