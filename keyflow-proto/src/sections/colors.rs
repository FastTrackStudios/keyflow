//! Section Color Palette
//!
//! Provides semantic colors for sections based on their type.
//! Uses Tailwind CSS color palette for consistency across the app.
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
//! | Pre-*/Post-* | Lighter shade of parent | Visual hierarchy |
//! | Hits/Breakdown | Slate | Neutral |
//! | Custom | Slate | Neutral with border |

use super::SectionType;

/// RGB color values as (r, g, b) tuples
pub mod rgb {
    // Slate (neutral cool gray)
    pub const SLATE_200: (u8, u8, u8) = (226, 232, 240);
    pub const SLATE_400: (u8, u8, u8) = (148, 163, 184);
    pub const SLATE_600: (u8, u8, u8) = (71, 85, 105);
    pub const SLATE_800: (u8, u8, u8) = (30, 41, 59);

    // Orange (Intro/Instrumental)
    pub const ORANGE_200: (u8, u8, u8) = (254, 215, 170);
    pub const ORANGE_400: (u8, u8, u8) = (251, 146, 60);
    pub const ORANGE_600: (u8, u8, u8) = (234, 88, 12);
    pub const ORANGE_800: (u8, u8, u8) = (154, 52, 18);

    // Amber (Outro)
    pub const AMBER_200: (u8, u8, u8) = (253, 230, 138);
    pub const AMBER_400: (u8, u8, u8) = (251, 191, 36);
    pub const AMBER_600: (u8, u8, u8) = (217, 119, 6);
    pub const AMBER_800: (u8, u8, u8) = (146, 64, 14);

    // Yellow (Interlude)
    pub const YELLOW_200: (u8, u8, u8) = (254, 240, 138);
    pub const YELLOW_400: (u8, u8, u8) = (250, 204, 21);
    pub const YELLOW_600: (u8, u8, u8) = (202, 138, 4);
    pub const YELLOW_800: (u8, u8, u8) = (133, 77, 14);

    // Emerald (Verse)
    pub const EMERALD_200: (u8, u8, u8) = (167, 243, 208);
    pub const EMERALD_400: (u8, u8, u8) = (52, 211, 153);
    pub const EMERALD_600: (u8, u8, u8) = (5, 150, 105);
    pub const EMERALD_800: (u8, u8, u8) = (6, 95, 70);

    // Blue (Chorus)
    pub const BLUE_200: (u8, u8, u8) = (191, 219, 254);
    pub const BLUE_400: (u8, u8, u8) = (96, 165, 250);
    pub const BLUE_500: (u8, u8, u8) = (59, 130, 246);
    pub const BLUE_600: (u8, u8, u8) = (37, 99, 235);
    pub const BLUE_800: (u8, u8, u8) = (30, 64, 175);

    // Violet (Bridge)
    pub const VIOLET_200: (u8, u8, u8) = (221, 214, 254);
    pub const VIOLET_400: (u8, u8, u8) = (167, 139, 250);
    pub const VIOLET_600: (u8, u8, u8) = (124, 58, 237);
    pub const VIOLET_800: (u8, u8, u8) = (91, 33, 182);

    // Rose (Solo)
    pub const ROSE_200: (u8, u8, u8) = (254, 205, 211);
    pub const ROSE_400: (u8, u8, u8) = (251, 113, 133);
    pub const ROSE_600: (u8, u8, u8) = (225, 29, 72);
    pub const ROSE_800: (u8, u8, u8) = (159, 18, 57);
}

/// Section color configuration
#[derive(Debug, Clone, Copy)]
pub struct SectionColors {
    /// Bright color (for progress bars, active elements)
    pub bright: (u8, u8, u8),
    /// Muted color (for backgrounds, inactive states)
    pub muted: (u8, u8, u8),
    /// Text color (for text on muted backgrounds)
    pub text: (u8, u8, u8),
}

impl SectionColors {
    /// Get bright color as CSS rgb() string
    #[must_use]
    pub fn bright_css(&self) -> String {
        let (r, g, b) = self.bright;
        format!("rgb({r}, {g}, {b})")
    }

    /// Get muted color as CSS rgb() string
    #[must_use]
    pub fn muted_css(&self) -> String {
        let (r, g, b) = self.muted;
        format!("rgb({r}, {g}, {b})")
    }

    /// Get text color as CSS rgb() string
    #[must_use]
    pub fn text_css(&self) -> String {
        let (r, g, b) = self.text;
        format!("rgb({r}, {g}, {b})")
    }

    /// Get bright color as hex string (#RRGGBB)
    #[must_use]
    pub fn bright_hex(&self) -> String {
        let (r, g, b) = self.bright;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Get muted color as hex string (#RRGGBB)
    #[must_use]
    pub fn muted_hex(&self) -> String {
        let (r, g, b) = self.muted;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

/// Get semantic colors for a section type
#[must_use]
pub fn colors_for_section_type(section_type: &SectionType) -> SectionColors {
    use rgb::*;

    match section_type {
        // Main sections - distinct colors for each
        SectionType::Intro => SectionColors {
            bright: ORANGE_400,
            muted: ORANGE_200,
            text: ORANGE_800,
        },
        SectionType::Verse => SectionColors {
            bright: EMERALD_400,
            muted: EMERALD_200,
            text: EMERALD_800,
        },
        SectionType::Chorus => SectionColors {
            bright: BLUE_500,
            muted: BLUE_200,
            text: BLUE_800,
        },
        SectionType::Bridge => SectionColors {
            bright: VIOLET_400,
            muted: VIOLET_200,
            text: VIOLET_800,
        },
        SectionType::Outro => SectionColors {
            bright: AMBER_400,
            muted: AMBER_200,
            text: AMBER_800,
        },
        SectionType::Instrumental => SectionColors {
            bright: ORANGE_400,
            muted: ORANGE_200,
            text: ORANGE_800,
        },
        SectionType::Solo => SectionColors {
            bright: ROSE_400,
            muted: ROSE_200,
            text: ROSE_800,
        },
        SectionType::Interlude => SectionColors {
            bright: YELLOW_400,
            muted: YELLOW_200,
            text: YELLOW_800,
        },

        // Pre/Post sections - use parent section's muted as bright
        SectionType::Pre(inner) | SectionType::Post(inner) => {
            let parent = colors_for_section_type(inner);
            SectionColors {
                bright: parent.muted,
                muted: parent.muted,
                text: parent.text,
            }
        }

        // Utility sections - neutral colors
        SectionType::CountIn | SectionType::End => SectionColors {
            bright: SLATE_400,
            muted: SLATE_200,
            text: SLATE_800,
        },
        SectionType::Hits | SectionType::Breakdown => SectionColors {
            bright: SLATE_400,
            muted: SLATE_200,
            text: SLATE_800,
        },

        // Custom sections - slate
        SectionType::Custom(_) => SectionColors {
            bright: SLATE_400,
            muted: SLATE_200,
            text: SLATE_800,
        },
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
}
