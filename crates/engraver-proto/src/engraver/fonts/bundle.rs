//! Chart Font Bundle
//!
//! Provides a single source of truth for all fonts needed by the chart layout
//! and rendering pipeline. Both the REAPER extension and web app should use this
//! bundle to guarantee identical font configuration.

use std::sync::Arc;

use crate::engraver::layout::chart::ChartLayoutEngine;
use crate::engraver::renderer::scene_renderer::VelloSceneRenderer;
use crate::engraver::style::MStyle;

use super::SMuFLFont;

// Embedded fonts — single source of truth for the entire workspace
static BRAVURA_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/Bravura.otf");
static BRAVURA_METADATA_BYTES: &[u8] = include_bytes!("../../../fonts/bravura_metadata.json");
static FREESANS_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/FreeSans.ttf");
static MUSEJAZZ_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/MuseJazzText.otf");

/// All fonts needed for chart layout and rendering, bundled together.
///
/// This ensures both the REAPER extension and web app use exactly the same
/// fonts with the same configuration, making it impossible for them to diverge.
///
/// # Font roles
/// - **Text font** (`MuseJazzText`): Used for chord symbol measurement and rendering.
///   The `ChartLayoutEngine` measures chord widths with this font.
/// - **Symbol font** (`Bravura`): SMuFL music notation font for noteheads, rests,
///   accidentals, and other music symbols.
/// - **Aux font** (`FreeSans`): Used for titles, section labels, part names, and
///   other non-musical text.
pub struct ChartFontBundle {
    smufl_font: SMuFLFont<'static>,
    /// MuseJazz — chord symbol measurement and rendering
    text_font_data: Arc<Vec<u8>>,
    /// Bravura — SMuFL music notation symbols
    symbol_font_data: Arc<Vec<u8>>,
    /// FreeSans — titles, section notes, part names
    aux_font_data: Arc<Vec<u8>>,
}

impl ChartFontBundle {
    /// Create a new font bundle with all embedded fonts loaded.
    ///
    /// # Errors
    /// Returns an error if the embedded Bravura font or metadata cannot be parsed.
    pub fn new() -> Result<Self, String> {
        let smufl_font = SMuFLFont::from_reader(
            BRAVURA_FONT_BYTES,
            std::io::Cursor::new(BRAVURA_METADATA_BYTES),
        )
        .map_err(|e| format!("Failed to load Bravura font: {e}"))?;

        Ok(Self {
            smufl_font,
            text_font_data: Arc::new(MUSEJAZZ_FONT_BYTES.to_vec()),
            symbol_font_data: Arc::new(BRAVURA_FONT_BYTES.to_vec()),
            aux_font_data: Arc::new(FREESANS_FONT_BYTES.to_vec()),
        })
    }

    /// Get the loaded SMuFL font (Bravura).
    #[must_use]
    pub fn smufl_font(&self) -> &SMuFLFont<'static> {
        &self.smufl_font
    }

    /// Get the text/chord font data (MuseJazzText).
    #[must_use]
    pub fn text_font_data(&self) -> &Arc<Vec<u8>> {
        &self.text_font_data
    }

    /// Get the symbol font data (Bravura).
    #[must_use]
    pub fn symbol_font_data(&self) -> &Arc<Vec<u8>> {
        &self.symbol_font_data
    }

    /// Get the auxiliary font data (FreeSans) for titles, section notes, etc.
    #[must_use]
    pub fn aux_font_data(&self) -> &Arc<Vec<u8>> {
        &self.aux_font_data
    }

    /// Create a correctly-wired `ChartLayoutEngine`.
    ///
    /// Uses MuseJazz for text metrics (chord width measurement) and Bravura for
    /// symbol font, matching the web app's configuration exactly.
    #[must_use]
    pub fn create_layout_engine(&self, style: &'static MStyle) -> ChartLayoutEngine {
        ChartLayoutEngine::new(
            style,
            self.text_font_data.clone(),   // MuseJazz — chord measurement
            self.symbol_font_data.clone(), // Bravura — symbols
        )
    }

    /// Configure a `VelloSceneRenderer` with all required named fonts.
    ///
    /// Registers the SMuFL font, text font, and all named font aliases so that
    /// `PaintCommand::Text` references resolve correctly. This is the canonical
    /// font configuration — both REAPER and web should use this method.
    #[must_use]
    pub fn configure_renderer<'a>(
        &'a self,
        renderer: VelloSceneRenderer<'a>,
    ) -> VelloSceneRenderer<'a> {
        renderer
            .with_font(&self.smufl_font)
            .with_text_font_arc(self.aux_font_data.clone()) // FreeSans as default text fallback
            .with_named_font_arc("MuseJazzText", self.text_font_data.clone())
            .with_named_font_arc("MuseJazz", self.text_font_data.clone())
            .with_named_font_arc("MuseJazz Text", self.text_font_data.clone())
            .with_named_font_arc("section-note", self.aux_font_data.clone())
            .with_named_font_arc("title-bold", self.aux_font_data.clone())
            .with_named_font_arc("part-name-bold", self.aux_font_data.clone())
    }
}
