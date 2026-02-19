//! Score header metadata and layout types.
//!
//! This module defines the metadata fields displayed in a score's title frame,
//! following the MuseScore convention of Title, Subtitle, Composer, etc.

use serde::{Deserialize, Serialize};

/// Score header metadata displayed at the top of the first page.
///
/// This follows the traditional engraving convention:
/// - Part Name in top left
/// - Composer in top right
/// - Title centered (largest text)
/// - Subtitle centered below title (smaller)
/// - Version below composer (optional)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreHeader {
    /// Main title of the piece (centered, largest text)
    pub title: Option<String>,
    /// Subtitle - often includes arrangement info, opus number, etc. (centered, below title)
    pub subtitle: Option<String>,
    /// Composer name (top right)
    pub composer: Option<String>,
    /// Part name - instrument or voice (top left)
    pub part_name: Option<String>,
    /// Version identifier (below composer, optional)
    pub version: Option<String>,
    /// Lyricist/poet name (optional, below part name)
    pub lyricist: Option<String>,
    /// Arranger name (optional)
    pub arranger: Option<String>,
    /// Copyright notice (typically in footer)
    pub copyright: Option<String>,
}

impl ScoreHeader {
    /// Create a new empty score header.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a header with just a title.
    #[must_use]
    pub fn with_title(title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            ..Default::default()
        }
    }

    /// Builder method to set the title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Builder method to set the subtitle.
    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Builder method to set the composer.
    #[must_use]
    pub fn composer(mut self, composer: impl Into<String>) -> Self {
        self.composer = Some(composer.into());
        self
    }

    /// Builder method to set the part name.
    #[must_use]
    pub fn part_name(mut self, part_name: impl Into<String>) -> Self {
        self.part_name = Some(part_name.into());
        self
    }

    /// Builder method to set the version.
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builder method to set the lyricist.
    #[must_use]
    pub fn lyricist(mut self, lyricist: impl Into<String>) -> Self {
        self.lyricist = Some(lyricist.into());
        self
    }

    /// Builder method to set the arranger.
    #[must_use]
    pub fn arranger(mut self, arranger: impl Into<String>) -> Self {
        self.arranger = Some(arranger.into());
        self
    }

    /// Builder method to set the copyright.
    #[must_use]
    pub fn copyright(mut self, copyright: impl Into<String>) -> Self {
        self.copyright = Some(copyright.into());
        self
    }

    /// Check if the header has any content to display.
    #[must_use]
    pub fn has_content(&self) -> bool {
        self.title.is_some()
            || self.subtitle.is_some()
            || self.composer.is_some()
            || self.part_name.is_some()
            || self.version.is_some()
    }
}

/// Text alignment for header elements.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum HeaderTextAlign {
    /// Align to left edge
    Left,
    /// Center horizontally
    #[default]
    Center,
    /// Align to right edge
    Right,
}

/// Style configuration for header text elements.
#[derive(Debug, Clone, Copy)]
pub struct HeaderTextStyle {
    /// Font size in points
    pub font_size: f32,
    /// Line height multiplier
    pub line_height: f32,
    /// Horizontal alignment
    pub align: HeaderTextAlign,
    /// Whether text should be bold
    pub bold: bool,
    /// Whether text should be italic
    pub italic: bool,
}

impl Default for HeaderTextStyle {
    fn default() -> Self {
        Self {
            font_size: 12.0,
            line_height: 1.2,
            align: HeaderTextAlign::Center,
            bold: false,
            italic: false,
        }
    }
}

/// Predefined header text styles following MuseScore conventions.
pub struct HeaderStyles;

impl HeaderStyles {
    /// Title style - large, centered, bold
    pub const TITLE: HeaderTextStyle = HeaderTextStyle {
        font_size: 24.0, // MuseScore uses 22pt, we use slightly larger
        line_height: 1.2,
        align: HeaderTextAlign::Center,
        bold: true,
        italic: false,
    };

    /// Subtitle style - medium, centered
    pub const SUBTITLE: HeaderTextStyle = HeaderTextStyle {
        font_size: 14.0,
        line_height: 1.2,
        align: HeaderTextAlign::Center,
        bold: false,
        italic: false,
    };

    /// Composer style - small, right-aligned
    pub const COMPOSER: HeaderTextStyle = HeaderTextStyle {
        font_size: 12.0,
        line_height: 1.2,
        align: HeaderTextAlign::Right,
        bold: false,
        italic: false,
    };

    /// Part name style - small, left-aligned
    pub const PART_NAME: HeaderTextStyle = HeaderTextStyle {
        font_size: 12.0,
        line_height: 1.2,
        align: HeaderTextAlign::Left,
        bold: false,
        italic: false,
    };

    /// Version style - small, right-aligned, italic
    pub const VERSION: HeaderTextStyle = HeaderTextStyle {
        font_size: 10.0,
        line_height: 1.2,
        align: HeaderTextAlign::Right,
        bold: false,
        italic: true,
    };

    /// Lyricist style - small, left-aligned
    pub const LYRICIST: HeaderTextStyle = HeaderTextStyle {
        font_size: 10.0,
        line_height: 1.2,
        align: HeaderTextAlign::Left,
        bold: false,
        italic: false,
    };
}

/// Configuration for the header frame (title box).
#[derive(Debug, Clone, Copy)]
pub struct HeaderFrameConfig {
    /// Minimum height of the header frame in pixels
    pub min_height: f32,
    /// Top margin inside the frame
    pub margin_top: f32,
    /// Bottom margin inside the frame
    pub margin_bottom: f32,
    /// Left margin inside the frame
    pub margin_left: f32,
    /// Right margin inside the frame
    pub margin_right: f32,
    /// Spacing between title and subtitle
    pub title_subtitle_gap: f32,
    /// Whether to auto-size based on content
    pub auto_size: bool,
}

impl Default for HeaderFrameConfig {
    fn default() -> Self {
        Self {
            min_height: 100.0,
            margin_top: 20.0,
            margin_bottom: 20.0,
            margin_left: 10.0,
            margin_right: 10.0,
            title_subtitle_gap: 8.0,
            auto_size: true,
        }
    }
}

/// Computed layout information for a header.
#[derive(Debug, Clone, Default)]
pub struct ComputedHeaderLayout {
    /// Total height of the header frame
    pub frame_height: f32,
    /// Y position for the title text
    pub title_y: f32,
    /// Y position for the subtitle text
    pub subtitle_y: f32,
    /// Y position for the top row (part name, composer)
    pub top_row_y: f32,
    /// Y position for the second row (lyricist, version)
    pub second_row_y: f32,
}

impl ComputedHeaderLayout {
    /// Compute header layout based on content and configuration.
    #[must_use]
    pub fn compute(header: &ScoreHeader, config: &HeaderFrameConfig) -> Self {
        let mut height = config.margin_top;

        // Top row (part name left, composer right)
        let top_row_y = height;
        if header.part_name.is_some() || header.composer.is_some() {
            height += HeaderStyles::PART_NAME.font_size * HeaderStyles::PART_NAME.line_height;
        }

        // Second row (lyricist left, version right)
        let second_row_y = height;
        if header.lyricist.is_some() || header.version.is_some() {
            height += HeaderStyles::LYRICIST.font_size * HeaderStyles::LYRICIST.line_height;
        }

        // Title (centered, below top rows)
        height += 10.0; // Gap before title
        let title_y = height;
        if header.title.is_some() {
            height += HeaderStyles::TITLE.font_size * HeaderStyles::TITLE.line_height;
        }

        // Subtitle (centered, below title)
        let subtitle_y = height + config.title_subtitle_gap;
        if header.subtitle.is_some() {
            height += config.title_subtitle_gap;
            height += HeaderStyles::SUBTITLE.font_size * HeaderStyles::SUBTITLE.line_height;
        }

        height += config.margin_bottom;

        // Apply minimum height
        let frame_height = if config.auto_size {
            height.max(config.min_height)
        } else {
            config.min_height
        };

        Self {
            frame_height,
            title_y,
            subtitle_y,
            top_row_y,
            second_row_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_builder() {
        let header = ScoreHeader::new()
            .title("Symphony No. 5")
            .subtitle("in C minor, Op. 67")
            .composer("Ludwig van Beethoven")
            .part_name("Violin I");

        assert_eq!(header.title.as_deref(), Some("Symphony No. 5"));
        assert_eq!(header.subtitle.as_deref(), Some("in C minor, Op. 67"));
        assert_eq!(header.composer.as_deref(), Some("Ludwig van Beethoven"));
        assert_eq!(header.part_name.as_deref(), Some("Violin I"));
        assert!(header.has_content());
    }

    #[test]
    fn test_empty_header() {
        let header = ScoreHeader::new();
        assert!(!header.has_content());
    }

    #[test]
    fn test_computed_layout() {
        let header = ScoreHeader::new()
            .title("Test Title")
            .composer("Test Composer");

        let config = HeaderFrameConfig::default();
        let layout = ComputedHeaderLayout::compute(&header, &config);

        assert!(layout.frame_height >= config.min_height);
        assert!(layout.title_y > 0.0);
    }
}
