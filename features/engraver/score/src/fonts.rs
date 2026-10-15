//! Text faces for engraved parts.
//!
//! Orchestral parts are set in a Times-style serif (Finale's default page
//! text is Times New Roman). We ship GNU FreeSerif — the serif sibling of
//! the FreeSans the chart bundle already carries — subset to Latin plus
//! the music-text symbols (♭ ♮ ♯), so a part's PDF does not grow by 3 MB.
//! The faces live here rather than in `ChartFontBundle` so lead-sheet
//! exports do not pay for them.
//!
//! Text widths are measured from the real glyph advances: a rehearsal box
//! sized from a character-count estimate comes out visibly wrong.

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// The family name every face below registers under.
pub const SERIF_FAMILY: &str = "FreeSerif";

static REGULAR: &[u8] = include_bytes!("../fonts/FreeSerif.ttf");
static BOLD: &[u8] = include_bytes!("../fonts/FreeSerifBold.ttf");
static ITALIC: &[u8] = include_bytes!("../fonts/FreeSerifItalic.ttf");
static BOLD_ITALIC: &[u8] = include_bytes!("../fonts/FreeSerifBoldItalic.ttf");

/// Every serif face, for loading into a font database (PDF export).
#[must_use]
pub fn serif_faces() -> [(&'static str, &'static [u8]); 4] {
    [
        ("FreeSerif", REGULAR),
        ("FreeSerif Bold", BOLD),
        ("FreeSerif Italic", ITALIC),
        ("FreeSerif Bold Italic", BOLD_ITALIC),
    ]
}

/// The regular face (what an SVG preview embeds).
#[must_use]
pub fn serif_regular() -> &'static [u8] {
    REGULAR
}

fn face(bold: bool, italic: bool) -> &'static [u8] {
    match (bold, italic) {
        (false, false) => REGULAR,
        (true, false) => BOLD,
        (false, true) => ITALIC,
        (true, true) => BOLD_ITALIC,
    }
}

/// Advance width of `text` in points, set in the serif face at `size` pt.
/// Characters the subset lacks count as half an em.
#[must_use]
pub fn text_width(text: &str, size: f64, bold: bool, italic: bool) -> f64 {
    let Ok(font) = FontRef::new(face(bold, italic)) else {
        return 0.5 * size * text.chars().count() as f64;
    };
    let charmap = font.charmap();
    let metrics = font.glyph_metrics(Size::new(size as f32), LocationRef::default());
    text.chars()
        .map(|c| {
            charmap
                .map(c)
                .and_then(|gid| metrics.advance_width(gid))
                .map_or(0.5 * size, f64::from)
        })
        .sum()
}

/// Cap height of the serif face as a fraction of the point size (for
/// centring text in a box).
#[must_use]
pub fn cap_height_ratio() -> f64 {
    0.66
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_come_from_glyph_advances() {
        let narrow = text_width("iiii", 12.0, false, false);
        let wide = text_width("MMMM", 12.0, false, false);
        assert!(wide > 2.0 * narrow, "M ({wide}) vs i ({narrow})");
        // Bold is wider than regular.
        assert!(
            text_width("Verse 1", 14.0, true, false) > text_width("Verse 1", 14.0, false, false)
        );
        // Times-like proportions: "Verse 1" at 14pt is ~45pt wide.
        let w = text_width("Verse 1", 14.0, true, false);
        assert!((35.0..60.0).contains(&w), "{w}");
    }
}
