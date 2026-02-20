//! Page style configuration for music notation rendering.
//!
//! This module defines configurable page layout parameters based on
//! traditional engraving conventions (particularly LilyPond/MuseScore).

use serde::{Deserialize, Serialize};

/// Standard paper sizes for music notation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum PaperSize {
    /// A4: 210mm × 297mm (common in Europe)
    A4,
    /// Letter: 8.5" × 11" (common in US, default)
    #[default]
    Letter,
    /// Legal: 8.5" × 14"
    Legal,
    /// Tabloid/Ledger: 11" × 17"
    Tabloid,
    /// B4: 250mm × 353mm
    B4,
    /// Custom size with explicit dimensions in points (72pt = 1 inch)
    Custom { width_pt: f32, height_pt: f32 },
}

impl PaperSize {
    /// Get paper dimensions in points (72 points = 1 inch).
    #[must_use]
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            Self::A4 => (595.28, 841.89),     // 210mm × 297mm
            Self::Letter => (612.0, 792.0),   // 8.5" × 11"
            Self::Legal => (612.0, 1008.0),   // 8.5" × 14"
            Self::Tabloid => (792.0, 1224.0), // 11" × 17"
            Self::B4 => (708.66, 1000.63),    // 250mm × 353mm
            Self::Custom {
                width_pt,
                height_pt,
            } => (*width_pt, *height_pt),
        }
    }

    /// Get paper dimensions in pixels at a given DPI.
    #[must_use]
    pub fn dimensions_px(&self, dpi: f32) -> (f32, f32) {
        let (w_pt, h_pt) = self.dimensions_pt();
        let scale = dpi / 72.0;
        (w_pt * scale, h_pt * scale)
    }
}

/// Margin configuration in points (72pt = 1 inch, ~2.83pt = 1mm).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Margins {
    /// Top margin in points
    pub top: f32,
    /// Bottom margin in points
    pub bottom: f32,
    /// Left margin in points
    pub left: f32,
    /// Right margin in points
    pub right: f32,
}

impl Default for Margins {
    /// Default margins based on MuseScore defaults: 15mm all around
    fn default() -> Self {
        Self {
            top: 42.5,    // 15mm
            bottom: 42.5, // 15mm
            left: 42.5,   // 15mm
            right: 42.5,  // 15mm
        }
    }
}

impl Margins {
    /// Create margins with all sides equal.
    #[must_use]
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            bottom: value,
            left: value,
            right: value,
        }
    }

    /// Create margins from millimeter values.
    #[must_use]
    pub fn from_mm(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        const MM_TO_PT: f32 = 72.0 / 25.4; // ~2.835
        Self {
            top: top * MM_TO_PT,
            bottom: bottom * MM_TO_PT,
            left: left * MM_TO_PT,
            right: right * MM_TO_PT,
        }
    }

    /// Create margins from inch values.
    #[must_use]
    pub fn from_inches(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        const INCH_TO_PT: f32 = 72.0;
        Self {
            top: top * INCH_TO_PT,
            bottom: bottom * INCH_TO_PT,
            left: left * INCH_TO_PT,
            right: right * INCH_TO_PT,
        }
    }
}

/// Staff configuration settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StaffConfig {
    /// Number of staff lines (typically 5)
    pub line_count: u32,
    /// Staff line thickness in staff-space units (0.05 for thin lines, 0.1 for standard)
    pub line_thickness: f32,
    /// Staff height in points (distance from top to bottom line)
    /// Standard is ~20pt for a readable staff
    pub staff_height: f32,
    /// Space between staff lines in points (staff_height / (line_count - 1))
    pub staff_space: f32,
}

impl Default for StaffConfig {
    /// Default staff configuration with 5 lines and thin line thickness.
    /// Uses MuseScore's default spatium of 1.75mm (~5pt per staff space).
    fn default() -> Self {
        let line_count = 5;
        // MuseScore spatium = 1.75mm = ~5pt (72pt/25.4mm * 1.75mm)
        let staff_space = 5.0; // ~1.75mm per staff space
        let staff_height = staff_space * (line_count - 1) as f32; // ~20pt
        Self {
            line_count,
            line_thickness: 0.05, // Very thin staff lines (from thin-staff-lines.ly)
            staff_height,
            staff_space,
        }
    }
}

impl StaffConfig {
    /// Create a staff config with a specific spatium (staff space) in points.
    /// This is the preferred way to set staff size as spatium is the fundamental unit.
    #[must_use]
    pub fn with_spatium(spatium: f32) -> Self {
        let line_count = 5;
        let staff_height = spatium * (line_count - 1) as f32;
        Self {
            line_count,
            line_thickness: 0.05,
            staff_height,
            staff_space: spatium,
        }
    }

    /// Create a staff config with a specific height.
    #[must_use]
    pub fn with_height(staff_height: f32) -> Self {
        let line_count = 5;
        let staff_space = staff_height / (line_count - 1) as f32;
        Self {
            line_count,
            line_thickness: 0.05,
            staff_height,
            staff_space,
        }
    }

    /// Get the actual line thickness in points.
    #[must_use]
    pub fn line_thickness_pt(&self) -> f32 {
        self.line_thickness * self.staff_space
    }
}

/// System (line of music) spacing configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemSpacing {
    /// Basic distance between systems in points
    /// LilyPond default: 15 staff-spaces
    pub system_to_system: f32,
    /// Extra spacing before a section start in points
    pub section_spacing: f32,
    /// Minimum distance between systems (won't compress below this)
    pub minimum_distance: f32,
    /// Top padding before first system on a page
    pub top_padding: f32,
    /// Bottom padding after last system on a page
    pub bottom_padding: f32,
}

impl Default for SystemSpacing {
    /// Default system spacing using MuseScore's actual default values.
    /// minSystemDistance = 8.5 spatium (gap from bottom of one staff to top of next).
    fn default() -> Self {
        // Default spatium is 5pt (1.75mm)
        let spatium = 5.0;
        Self {
            system_to_system: spatium * 8.5, // 42.5pt - MuseScore minSystemDistance
            section_spacing: spatium * 4.0,  // 20pt - Extra space before sections (optional)
            minimum_distance: spatium * 8.5, // 42.5pt - MuseScore minSystemDistance
            top_padding: spatium * 6.0,      // 30pt - Padding at top of content area
            bottom_padding: spatium * 4.0,   // 20pt - Padding at bottom
        }
    }
}

/// Line breaking configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LineBreakConfig {
    /// Default number of measures per line
    pub measures_per_line: u32,
    /// Whether to force breaks at section boundaries
    pub break_at_sections: bool,
    /// First line indent in points (typically 0 for lead sheets)
    pub first_line_indent: f32,
}

impl Default for LineBreakConfig {
    fn default() -> Self {
        Self {
            measures_per_line: 4, // 4 measures per line (from auto-four-measure-breaks.ly)
            break_at_sections: true,
            first_line_indent: 0.0, // No indent for lead sheets
        }
    }
}

/// Complete page style configuration.
///
/// This struct combines all layout parameters into a single configuration
/// that can be used to render music notation with consistent styling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageStyle {
    /// Paper size
    pub paper_size: PaperSize,
    /// Page margins
    pub margins: Margins,
    /// Staff configuration
    pub staff: StaffConfig,
    /// System spacing
    pub system_spacing: SystemSpacing,
    /// Line breaking rules
    pub line_breaks: LineBreakConfig,
    /// Header frame height (0 if no header)
    pub header_height: f32,
    /// Footer frame height (for page numbers, copyright, etc.)
    pub footer_height: f32,
    /// Whether to show page numbers
    pub show_page_numbers: bool,
    /// Display DPI for screen rendering
    pub display_dpi: f32,
}

impl Default for PageStyle {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::A4,
            margins: Margins::default(),
            staff: StaffConfig::default(),
            system_spacing: SystemSpacing::default(),
            line_breaks: LineBreakConfig::default(),
            header_height: 100.0, // Room for title, composer, etc.
            footer_height: 30.0,  // Room for page number and footer text
            show_page_numbers: true,
            display_dpi: 96.0, // Standard screen DPI
        }
    }
}

impl PageStyle {
    /// Create a new page style with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a page style for lead sheets.
    /// Uses MuseScore's default values with no first-line indent.
    #[must_use]
    pub fn lead_sheet() -> Self {
        Self {
            paper_size: PaperSize::Letter,
            margins: Margins::default(),   // 15mm - MuseScore default
            staff: StaffConfig::default(), // 5pt spatium - MuseScore default
            system_spacing: SystemSpacing::default(), // 8.5 spatium - MuseScore default
            line_breaks: LineBreakConfig {
                measures_per_line: 4,
                break_at_sections: true,
                first_line_indent: 0.0, // No indent for lead sheets
            },
            header_height: 100.0,
            footer_height: 25.0,
            show_page_numbers: true,
            display_dpi: 96.0,
        }
    }

    /// Create a page style for full scores.
    /// Uses MuseScore defaults with standard first-line indent.
    #[must_use]
    pub fn full_score() -> Self {
        Self {
            paper_size: PaperSize::Letter,
            margins: Margins::default(),   // 15mm - MuseScore default
            staff: StaffConfig::default(), // 5pt spatium - MuseScore default
            system_spacing: SystemSpacing::default(), // 8.5 spatium - MuseScore default
            line_breaks: LineBreakConfig {
                measures_per_line: 4,
                break_at_sections: true,
                first_line_indent: 14.0, // Standard first-line indent for scores
            },
            header_height: 120.0,
            footer_height: 30.0,
            show_page_numbers: true,
            display_dpi: 96.0,
        }
    }

    /// Get page dimensions in pixels at the configured DPI.
    #[must_use]
    pub fn page_dimensions_px(&self) -> (f32, f32) {
        self.paper_size.dimensions_px(self.display_dpi)
    }

    /// Get the content area width (page width minus margins).
    #[must_use]
    pub fn content_width_pt(&self) -> f32 {
        let (w, _) = self.paper_size.dimensions_pt();
        w - self.margins.left - self.margins.right
    }

    /// Get the content area height (page height minus margins only).
    /// Footer and header are handled separately in layout calculations.
    #[must_use]
    pub fn content_height_pt(&self) -> f32 {
        let (_, h) = self.paper_size.dimensions_pt();
        h - self.margins.top - self.margins.bottom
    }

    /// Get the content height for the first page (accounts for header).
    /// Footer is in margin area, so not subtracted here.
    #[must_use]
    pub fn first_page_content_height_pt(&self) -> f32 {
        self.content_height_pt() - self.header_height
    }

    /// Calculate system height (staff height plus room for elements above/below).
    /// System height = staff height + ~4 spatium for dynamics, lyrics, etc.
    #[must_use]
    pub fn system_height(&self) -> f32 {
        // Staff height plus room above/below for dynamics, articulations, lyrics
        // ~2 spatium above + 2 spatium below
        self.staff.staff_height + (self.staff.staff_space * 4.0)
    }

    /// Estimate maximum systems per page.
    #[must_use]
    pub fn max_systems_per_page(&self) -> usize {
        let available = self.content_height_pt() - self.system_spacing.top_padding;
        let per_system = self.staff.staff_height + self.system_spacing.system_to_system;
        (available / per_system).floor() as usize
    }

    /// Convert this PageStyle to a PageLayoutConfig for the layout engine.
    /// Uses staff_height (not system_height) to match actual rendering space.
    /// Footer is placed in margin area, so footer_height is 0 for layout calculation.
    /// Section extra spacing is 0 to match MuseScore default (no extra space for sections).
    #[must_use]
    pub fn to_layout_config(&self) -> super::PageLayoutConfig {
        super::PageLayoutConfig {
            content_height: self.content_height_pt(),
            system_height: self.staff.staff_height, // Use actual staff height, not system_height
            system_spacing: self.system_spacing.system_to_system,
            section_extra_spacing: 0.0, // No extra spacing for sections (matches renderer)
            top_padding: self.system_spacing.top_padding,
            first_page_header_height: self.header_height,
            footer_height: 0.0, // Footer is in margin area, doesn't take content space
            // Spread options (disabled by default for PageStyle)
            enable_vertical_spread: false,
            min_system_spread: self.system_spacing.minimum_distance,
            max_system_spread: self.system_spacing.system_to_system * 3.0,
            align_to_margins: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_size_dimensions() {
        let a4 = PaperSize::A4;
        let (w, h) = a4.dimensions_pt();
        assert!((w - 595.28).abs() < 0.1);
        assert!((h - 841.89).abs() < 0.1);

        let letter = PaperSize::Letter;
        let (w, h) = letter.dimensions_pt();
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);
    }

    #[test]
    fn test_margins_from_mm() {
        let margins = Margins::from_mm(12.0, 12.0, 16.0, 16.0);
        // 12mm ≈ 34pt, 16mm ≈ 45pt
        assert!((margins.top - 34.0).abs() < 1.0);
        assert!((margins.left - 45.0).abs() < 1.0);
    }

    #[test]
    fn test_page_style_content_dimensions() {
        let style = PageStyle::lead_sheet();
        let content_width = style.content_width_pt();
        let content_height = style.content_height_pt();

        // Letter is 612pt wide, margins are ~42.5pt each side = ~527pt content
        assert!(content_width > 520.0 && content_width < 540.0);

        // Letter is 792pt tall, margins ~85pt = ~707pt content
        assert!(content_height > 690.0 && content_height < 720.0);
    }

    #[test]
    fn test_max_systems_per_page() {
        let style = PageStyle::lead_sheet();
        let max_systems = style.max_systems_per_page();

        // Lead sheet should fit 7-10 systems per page
        assert!((6..=12).contains(&max_systems));
    }

    #[test]
    fn test_to_layout_config() {
        let style = PageStyle::lead_sheet();
        let config = style.to_layout_config();

        assert!(config.content_height > 0.0);
        assert!(config.system_height > 0.0);
        assert!(config.first_page_header_height > 0.0);
    }
}
