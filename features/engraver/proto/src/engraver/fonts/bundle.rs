//! Chart Font Bundle
//!
//! Provides a single source of truth for all fonts needed by the chart layout
//! and rendering pipeline. Both the REAPER extension and web app should use this
//! bundle to guarantee identical font configuration.

use std::sync::Arc;

use crate::engraver::layout::chart::ChartLayoutEngine;
#[cfg(feature = "wgpu")]
use crate::engraver::renderer::scene_renderer::VelloSceneRenderer;
use crate::engraver::style::MStyle;

use super::SMuFLFont;

// Embedded fonts — single source of truth for the entire workspace
static LELAND_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/Leland.otf");
static LELAND_METADATA_BYTES: &[u8] = include_bytes!("../../../fonts/leland_metadata.json");
static LELAND_TEXT_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/LelandText.otf");
static BRAVURA_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/Bravura.otf");
static FREESANS_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/FreeSans.ttf");
static MUSEJAZZ_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/MuseJazz.otf");
static MUSEJAZZ_TEXT_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/MuseJazzText.otf");
static CHICAGO_FLF_FONT_BYTES: &[u8] = include_bytes!("../../../fonts/ChicagoFLF.ttf");

/// All fonts needed for chart layout and rendering, bundled together.
///
/// This ensures both the REAPER extension and web app use exactly the same
/// fonts with the same configuration, making it impossible for them to diverge.
///
/// # Font roles
/// - **Text font** (`MuseJazz Text`): chord-symbol letters + numerals
///   (the jazz-style text font that pairs with MuseJazz the music font).
/// - **Symbol font** (`Leland`): SMuFL music notation font for noteheads,
///   rests, accidentals, and other music symbols. (MuseScore 4 default.)
/// - **Aux font** (`ChicagoFLF`): titles, headings, and other "regular
///   document text". A free Chicago revival by Robin Casady (public
///   domain) — matches the look of Tom Brooks-era Mac/Finale charts.
/// - **MuseJazz** (the music font): kept available for places that want
///   SMuFL-style jazz glyphs (cued in via PUA codepoints).
/// - **Leland Text**: kept available as a chord-symbol alternate and as a
///   fallback for SMuFL-text in case a chart references it directly.
pub struct ChartFontBundle {
    smufl_font: SMuFLFont<'static>,
    /// MuseJazz Text — chord symbol measurement + letter rendering.
    text_font_data: Arc<Vec<u8>>,
    /// MuseJazz — companion music font; chord-symbol PUA glyphs.
    musejazz_font_data: Arc<Vec<u8>>,
    /// Leland — SMuFL music notation symbols
    symbol_font_data: Arc<Vec<u8>>,
    /// ChicagoFLF — default document text font (titles / headings).
    aux_font_data: Arc<Vec<u8>>,
    /// Leland Text — alternate text font kept available by name.
    leland_text_font_data: Arc<Vec<u8>>,
    /// Bravura — kept available as a fallback / alternate SMuFL font
    bravura_font_data: Arc<Vec<u8>>,
    /// FreeSans — kept as a generic sans-serif fallback
    freesans_font_data: Arc<Vec<u8>>,
}

impl ChartFontBundle {
    /// Create a new font bundle with all embedded fonts loaded.
    ///
    /// # Errors
    /// Returns an error if the embedded Bravura font or metadata cannot be parsed.
    pub fn new() -> Result<Self, String> {
        let smufl_font = SMuFLFont::from_reader(
            LELAND_FONT_BYTES,
            std::io::Cursor::new(LELAND_METADATA_BYTES),
        )
        .map_err(|e| format!("Failed to load Leland font: {e}"))?;

        Ok(Self {
            smufl_font,
            text_font_data: Arc::new(MUSEJAZZ_TEXT_FONT_BYTES.to_vec()),
            musejazz_font_data: Arc::new(MUSEJAZZ_FONT_BYTES.to_vec()),
            symbol_font_data: Arc::new(LELAND_FONT_BYTES.to_vec()),
            aux_font_data: Arc::new(CHICAGO_FLF_FONT_BYTES.to_vec()),
            leland_text_font_data: Arc::new(LELAND_TEXT_FONT_BYTES.to_vec()),
            bravura_font_data: Arc::new(BRAVURA_FONT_BYTES.to_vec()),
            freesans_font_data: Arc::new(FREESANS_FONT_BYTES.to_vec()),
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

    /// Get the auxiliary font data (Leland Text) for titles, section notes, etc.
    #[must_use]
    pub fn aux_font_data(&self) -> &Arc<Vec<u8>> {
        &self.aux_font_data
    }

    /// Get MuseJazz font data (the music-symbol companion to MuseJazz Text).
    #[must_use]
    pub fn musejazz_font_data(&self) -> &Arc<Vec<u8>> {
        &self.musejazz_font_data
    }

    /// Get Leland Text font data.
    #[must_use]
    pub fn leland_text_font_data(&self) -> &Arc<Vec<u8>> {
        &self.leland_text_font_data
    }

    /// Get ChicagoFLF (the default document text font) — same bytes as
    /// `aux_font_data` but named after its purpose.
    #[must_use]
    pub fn chicago_font_data(&self) -> &Arc<Vec<u8>> {
        &self.aux_font_data
    }

    /// Get Bravura font data (alternate SMuFL font kept for compatibility/fallback).
    #[must_use]
    pub fn bravura_font_data(&self) -> &Arc<Vec<u8>> {
        &self.bravura_font_data
    }

    /// Get FreeSans font data (generic sans-serif fallback).
    #[must_use]
    pub fn freesans_font_data(&self) -> &Arc<Vec<u8>> {
        &self.freesans_font_data
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
    ///
    /// Only available with the `wgpu` (GPU renderer) feature.
    /// Every `(font-family, bytes)` pair a rendered scene can reference.
    ///
    /// The GPU renderer registers these as named fonts; SVG and PDF export
    /// need the same list to emit `@font-face` rules, and the two MUST
    /// agree — a scene node asks for a family by name, and a name the
    /// output does not declare silently falls back to a system font.
    ///
    /// That drift is not hypothetical. Export used to declare `Bravura`,
    /// `MuseJazzText` and `FreeSans` by hand, which was wrong three ways:
    /// the chord font is emitted as `MuseJazz Text` *with a space* (it
    /// matches the font's internal name — see `tlayout::harmony`), so
    /// chord symbols never resolved and every `maj7` triangle rendered
    /// blank; and the bytes filed under `Bravura` and `FreeSans` were
    /// actually Leland and ChicagoFLF. The GPU path was unaffected only
    /// because it registers both spellings as aliases.
    ///
    /// Hence one list, here, used by both.
    #[must_use]
    pub fn embeddable_fonts(&self) -> Vec<(&'static str, Arc<Vec<u8>>)> {
        vec![
            // Chord symbols. BOTH spellings: the scene emits the spaced
            // form, older style defaults the unspaced one.
            ("MuseJazz Text", self.text_font_data.clone()),
            ("MuseJazzText", self.text_font_data.clone()),
            ("MuseJazz", self.musejazz_font_data.clone()),
            // SMuFL.
            ("Leland", self.symbol_font_data.clone()),
            ("Leland Text", self.leland_text_font_data.clone()),
            ("LelandText", self.leland_text_font_data.clone()),
            ("Edwin", self.leland_text_font_data.clone()),
            ("Bravura", self.bravura_font_data.clone()),
            // Document text.
            ("Chicago", self.aux_font_data.clone()),
            ("ChicagoFLF", self.aux_font_data.clone()),
            ("FreeSans", self.freesans_font_data.clone()),
            ("section-note", self.aux_font_data.clone()),
            // The note printed under a section card — `SectionComment`
            // in `scene::paint` emits this name, and nothing declared it,
            // so every one of them fell back to a system serif.
            ("section-comment", self.aux_font_data.clone()),
            ("title-bold", self.aux_font_data.clone()),
            ("part-name-bold", self.aux_font_data.clone()),
        ]
    }

    /// The shared font bundle. Built once per process.
    ///
    /// [`Self::new`] copies seven baked-in font blobs — FreeSans alone is
    /// 1.5 MB — and parses the SMuFL metadata. Nothing cached it, so
    /// every consumer that wanted a layout engine paid for all of that
    /// again: a guide page with four charts, four times, on the main
    /// thread, on every render.
    ///
    /// Sharing it is safe and cheap. The bundle is `Send + Sync`, the
    /// font bytes are already behind `Arc`, and
    /// [`Self::create_layout_engine`] only clones two of those `Arc`s —
    /// so a caller that needs its own *style* still gets its own engine
    /// for almost nothing, over these same bytes.
    ///
    /// # Errors
    ///
    /// Returns the load error if the bundle cannot be built. The failure
    /// is cached too: the bytes are baked into the binary, so a failure
    /// here is deterministic.
    pub fn shared() -> Result<&'static Self, String> {
        static SHARED: std::sync::OnceLock<Result<ChartFontBundle, String>> =
            std::sync::OnceLock::new();
        SHARED.get_or_init(Self::new).as_ref().map_err(Clone::clone)
    }

    /// [`Self::embeddable_fonts`] plus a concrete face for the generic
    /// `sans-serif` family.
    ///
    /// Use this for **rasterising and PDF**, not for browser CSS.
    ///
    /// The scene emits `font-family="sans-serif"` for a few incidental
    /// runs. A browser resolves that itself, and declaring an
    /// `@font-face` for it would override the reader's own default — so
    /// the web path must NOT include it. resvg and usvg have no such
    /// default: an unresolved generic there renders as nothing, or as
    /// whatever the host system happens to have, which is how a PNG and
    /// its PDF came out with different text.
    ///
    /// Two lists, then, because the two targets genuinely differ — not
    /// because anyone forgot to sync them.
    #[must_use]
    pub fn embeddable_fonts_for_raster(&self) -> Vec<(&'static str, Arc<Vec<u8>>)> {
        let mut fonts = self.embeddable_fonts();
        // The generic families the scene emits. A browser resolves these
        // itself; a rasteriser has no idea what they mean and will draw
        // nothing, so both need a concrete face here.
        fonts.push(("sans-serif", self.aux_font_data.clone()));
        fonts.push(("serif", self.aux_font_data.clone()));
        fonts
    }

    #[cfg(feature = "wgpu")]
    #[must_use]
    pub fn configure_renderer<'a>(
        &'a self,
        renderer: VelloSceneRenderer<'a>,
    ) -> VelloSceneRenderer<'a> {
        renderer
            .with_font(&self.smufl_font)
            // ChicagoFLF as the default text fallback — matches the Mac/Finale
            // chart aesthetic that titles/headings expect.
            .with_text_font_arc(self.aux_font_data.clone())
            // Chord-symbol jazz text font.
            .with_named_font_arc("MuseJazz Text", self.text_font_data.clone())
            .with_named_font_arc("MuseJazzText", self.text_font_data.clone())
            // MuseJazz (the music font) — distinct from MuseJazz Text.
            .with_named_font_arc("MuseJazz", self.musejazz_font_data.clone())
            // Leland (SMuFL) + Leland Text (text companion).
            .with_named_font_arc("Leland", self.symbol_font_data.clone())
            .with_named_font_arc("Leland Text", self.leland_text_font_data.clone())
            .with_named_font_arc("LelandText", self.leland_text_font_data.clone())
            // Style defaults reference "Edwin"; alias to Leland Text until/unless Edwin ships.
            .with_named_font_arc("Edwin", self.leland_text_font_data.clone())
            // Chicago — document text.
            .with_named_font_arc("Chicago", self.aux_font_data.clone())
            .with_named_font_arc("ChicagoFLF", self.aux_font_data.clone())
            .with_named_font_arc("Bravura", self.bravura_font_data.clone())
            .with_named_font_arc("FreeSans", self.freesans_font_data.clone())
            .with_named_font_arc("section-note", self.aux_font_data.clone())
            .with_named_font_arc("section-comment", self.aux_font_data.clone())
            .with_named_font_arc("title-bold", self.aux_font_data.clone())
            .with_named_font_arc("part-name-bold", self.aux_font_data.clone())
    }
}
