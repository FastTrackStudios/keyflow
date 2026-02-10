//! Text font metrics for accurate text measurement.
//!
//! Provides actual glyph advance widths from font files using skrifa,
//! similar to MuseScore's FontMetrics class.

use std::sync::Arc;

use skrifa::charmap::Charmap;
use skrifa::metrics::GlyphMetrics;
use skrifa::prelude::Size;
use skrifa::raw::FileRef;
use skrifa::MetadataProvider;

/// Text font metrics provider.
///
/// Wraps a font file and provides methods to measure actual glyph widths.
/// This is similar to MuseScore's FontMetrics class which uses an IFontProvider.
#[derive(Clone)]
pub struct TextFontMetrics {
    /// Font data (Arc for cheap cloning)
    font_data: Arc<Vec<u8>>,
}

impl TextFontMetrics {
    /// Create new font metrics from font data.
    #[must_use]
    pub fn new(font_data: Arc<Vec<u8>>) -> Self {
        Self { font_data }
    }

    /// Get the horizontal advance width for a string at the given font size.
    ///
    /// This uses actual glyph metrics from the font file, not estimation.
    #[must_use]
    pub fn horizontal_advance(&self, text: &str, font_size: f64) -> f64 {
        let font_ref = match FileRef::new(&self.font_data) {
            Ok(FileRef::Font(font)) => font,
            _ => return self.estimate_width(text, font_size),
        };

        let charmap = font_ref.charmap();
        let size = Size::new(font_size as f32);
        let glyph_metrics = font_ref.glyph_metrics(size, skrifa::instance::LocationRef::default());

        text.chars()
            .map(|c| self.char_advance(c, &charmap, &glyph_metrics, font_size))
            .sum()
    }

    /// Get advance width for a single character.
    fn char_advance(
        &self,
        c: char,
        charmap: &Charmap,
        glyph_metrics: &GlyphMetrics,
        font_size: f64,
    ) -> f64 {
        if let Some(gid) = charmap.map(c) {
            if let Some(advance) = glyph_metrics.advance_width(gid) {
                return advance as f64;
            }
        }
        // Fallback to estimation if glyph not found
        self.estimate_char_width(c, font_size)
    }

    /// Estimate character width as fallback.
    fn estimate_char_width(&self, c: char, font_size: f64) -> f64 {
        let ratio = match c {
            'm' | 'w' | 'M' | 'W' => 0.9,
            'A'..='Z' => 0.7,
            'a'..='z' => 0.5,
            '0'..='9' => 0.55,
            '/' => 0.35,
            ' ' => 0.28,
            _ => 0.55,
        };
        font_size * ratio
    }

    /// Estimate text width as fallback when font can't be loaded.
    fn estimate_width(&self, text: &str, font_size: f64) -> f64 {
        text.chars()
            .map(|c| self.estimate_char_width(c, font_size))
            .sum()
    }

    /// Get the cap-height of the font at the given size.
    ///
    /// Cap-height is the height of capital letters, used as the basis
    /// for MuseScore's spacing calculations.
    #[must_use]
    pub fn cap_height(&self, font_size: f64) -> f64 {
        let font_ref = match FileRef::new(&self.font_data) {
            Ok(FileRef::Font(font)) => font,
            _ => return font_size * 0.7, // Typical ratio
        };

        let size = Size::new(font_size as f32);
        let metrics = font_ref.metrics(size, skrifa::instance::LocationRef::default());

        // Cap height from font metrics, or estimate from ascent
        metrics.cap_height.map_or(font_size * 0.7, |h| h as f64)
    }

    /// Get the x-height (height of lowercase letters) at the given size.
    #[must_use]
    pub fn x_height(&self, font_size: f64) -> f64 {
        let font_ref = match FileRef::new(&self.font_data) {
            Ok(FileRef::Font(font)) => font,
            _ => return font_size * 0.5, // Typical ratio
        };

        let size = Size::new(font_size as f32);
        let metrics = font_ref.metrics(size, skrifa::instance::LocationRef::default());

        metrics.x_height.map_or(font_size * 0.5, |h| h as f64)
    }
}

impl std::fmt::Debug for TextFontMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextFontMetrics")
            .field("font_data_len", &self.font_data.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_width_fallback() {
        // Empty font data will trigger estimation fallback
        let metrics = TextFontMetrics::new(Arc::new(vec![]));

        let width = metrics.horizontal_advance("Test", 12.0);
        assert!(width > 0.0);
    }
}
