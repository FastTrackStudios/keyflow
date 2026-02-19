//! Section Color Palette
//!
//! Provides semantic colors for sections based on their type.
//! Uses the shared color-palette crate for consistency across the app.
//!
//! ## Color Philosophy
//!
//! - **Bright variants** (400/500 shades): Used for progress bars, boundaries, active elements
//! - **Muted variants** (200 shades): Used for backgrounds, inactive states, pre/post sections
//! - **Text colors** (800 shades): Used for text on muted backgrounds
//!
//! ## Section Color Mapping
//!
//! | Section Type | Color Family | Reasoning |
//! |---|---|---|
//! | Intro | Orange | Warm, welcoming start |
//! | Verse | Emerald | Fresh, natural progression |
//! | Chorus | Blue | Strong, memorable hook |
//! | Bridge | Violet | Contrast, transitional |
//! | Outro | Amber | Warm conclusion |
//! | Instrumental | Orange (light) | Related to intro |
//! | Interlude | Yellow | Bright pause |
//! | Vamp | Lime | Repeated/improvisation section |
//! | Pre-*/Post-* | Lighter shade of parent | Visual hierarchy |
//! | Hits/Breakdown | Slate | Neutral |
//! | Custom | Slate | Neutral with border |

use super::SectionType;
use color_palette::Color;

/// Section color configuration using the unified Color type
#[derive(Debug, Clone, Copy)]
pub struct SectionColors {
    /// Bright color (for progress bars, active elements)
    pub bright: Color,
    /// Muted color (for backgrounds, inactive states)
    pub muted: Color,
    /// Text color (for text on muted backgrounds)
    pub text: Color,
}

impl SectionColors {
    /// Create a new section color set
    pub const fn new(bright: Color, muted: Color, text: Color) -> Self {
        Self {
            bright,
            muted,
            text,
        }
    }

    /// Get bright color as CSS rgb() string
    #[must_use]
    pub fn bright_css(&self) -> String {
        self.bright.to_css_rgb()
    }

    /// Get muted color as CSS rgb() string
    #[must_use]
    pub fn muted_css(&self) -> String {
        self.muted.to_css_rgb()
    }

    /// Get text color as CSS rgb() string
    #[must_use]
    pub fn text_css(&self) -> String {
        self.text.to_css_rgb()
    }

    /// Get bright color as hex string (#RRGGBB)
    #[must_use]
    pub fn bright_hex(&self) -> String {
        self.bright.to_hex_string()
    }

    /// Get muted color as hex string (#RRGGBB)
    #[must_use]
    pub fn muted_hex(&self) -> String {
        self.muted.to_hex_string()
    }

    /// Get text color as hex string (#RRGGBB)
    #[must_use]
    pub fn text_hex(&self) -> String {
        self.text.to_hex_string()
    }
}

/// Predefined section color palettes using color-palette constants
pub mod palettes {
    use super::SectionColors;
    use color_palette::palette;

    /// Orange palette for Intro/Instrumental sections
    pub const ORANGE: SectionColors = SectionColors::new(
        palette::orange::S400,
        palette::orange::S200,
        palette::orange::S800,
    );

    /// Emerald palette for Verse sections
    pub const EMERALD: SectionColors = SectionColors::new(
        palette::emerald::S400,
        palette::emerald::S200,
        palette::emerald::S800,
    );

    /// Blue palette for Chorus sections
    pub const BLUE: SectionColors = SectionColors::new(
        palette::blue::S500,
        palette::blue::S200,
        palette::blue::S800,
    );

    /// Violet palette for Bridge sections
    pub const VIOLET: SectionColors = SectionColors::new(
        palette::violet::S400,
        palette::violet::S200,
        palette::violet::S800,
    );

    /// Amber palette for Outro sections
    pub const AMBER: SectionColors = SectionColors::new(
        palette::amber::S400,
        palette::amber::S200,
        palette::amber::S800,
    );

    /// Yellow palette for Interlude sections
    pub const YELLOW: SectionColors = SectionColors::new(
        palette::yellow::S400,
        palette::yellow::S200,
        palette::yellow::S800,
    );

    /// Rose palette for Solo sections
    pub const ROSE: SectionColors = SectionColors::new(
        palette::rose::S400,
        palette::rose::S200,
        palette::rose::S800,
    );

    /// Slate palette for neutral/utility sections
    pub const SLATE: SectionColors = SectionColors::new(
        palette::slate::S400,
        palette::slate::S200,
        palette::slate::S800,
    );

    /// Lime palette for Vamp sections
    pub const LIME: SectionColors = SectionColors::new(
        palette::lime::S400,
        palette::lime::S200,
        palette::lime::S800,
    );
}

/// Get semantic colors for a section type
#[must_use]
pub fn colors_for_section_type(section_type: &SectionType) -> SectionColors {
    use palettes::*;

    match section_type {
        // Main sections - distinct colors for each
        SectionType::Intro => ORANGE,
        SectionType::Verse => EMERALD,
        SectionType::Chorus => BLUE,
        SectionType::Bridge => VIOLET,
        SectionType::Outro => AMBER,
        SectionType::Instrumental => ORANGE,
        SectionType::Solo => ROSE,
        SectionType::Interlude => YELLOW,

        // Pre/Post sections - use parent section's muted as bright
        SectionType::Pre(inner) | SectionType::Post(inner) => {
            let parent = colors_for_section_type(inner);
            SectionColors {
                bright: parent.muted,
                muted: parent.muted,
                text: parent.text,
            }
        }

        // Vamp sections - lime (repeated/improvisation)
        SectionType::Vamp => LIME,

        // Utility sections - neutral colors
        SectionType::CountIn | SectionType::End => SLATE,
        SectionType::Hits | SectionType::Breakdown => SLATE,

        // Custom sections - slate
        SectionType::Custom(_) => SLATE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verse_colors() {
        let colors = colors_for_section_type(&SectionType::Verse);
        assert_eq!(colors.bright_css(), "rgb(52, 211, 153)");
        assert_eq!(colors.bright_hex(), "#34d399");
    }

    #[test]
    fn test_chorus_colors() {
        let colors = colors_for_section_type(&SectionType::Chorus);
        assert_eq!(colors.bright_css(), "rgb(59, 130, 246)");
    }

    #[test]
    fn test_pre_verse_uses_muted_verse() {
        let pre_verse = colors_for_section_type(&SectionType::Pre(Box::new(SectionType::Verse)));
        let verse = colors_for_section_type(&SectionType::Verse);
        // Pre-verse bright should be verse muted
        assert_eq!(pre_verse.bright, verse.muted);
    }

    #[test]
    fn test_color_conversions() {
        let colors = colors_for_section_type(&SectionType::Intro);
        // Orange 400 = 0xFB923C
        assert_eq!(colors.bright_hex(), "#fb923c");
        assert_eq!(colors.bright_css(), "rgb(251, 146, 60)");
    }
}
