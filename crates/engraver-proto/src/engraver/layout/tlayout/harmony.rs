//! Chord Symbol (Harmony) Layout
//!
//! Provides layout functions for rendering chord symbols like "Cm7", "CMaj7", "C7b9/G".
//! Supports both Standard and Jazz notation styles.
//!
//! # Layout Approach
//!
//! Chord symbols are composed of multiple text segments with different sizes and positions:
//! 1. **Root** - Full size (C, F#, Bb)
//! 2. **Quality** - Same size (m, dim, aug) or Jazz symbols (°, +)
//! 3. **Extension** - Scaled, superscript position (7, Maj7, 9)
//! 4. **Alterations** - Scaled, stacked or inline (b5, #9)
//! 5. **Bass** - Scaled, after slash (/G, /Bb)

use std::sync::Arc;

use kurbo::{Point, Rect};
use vello::peniko::Color;

use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;

/// Chord notation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChordNotation {
    /// Standard notation: CMaj7, Cdim, Caug, Cm7b5
    #[default]
    Standard,
    /// Jazz notation: C△7, C°, C+, Cø7
    Jazz,
}

/// Style configuration for chord symbols.
///
/// Based on MuseScore's chord rendering parameters from chords_std.xml:
/// - Root and quality render at full size on baseline
/// - Extensions (7, 9, 11, 13) render as superscript
/// - Alterations (b5, #9) render as superscript
/// - Superscript uses 0.75 scale and -0.36 vertical offset
///
/// # Required: Font Metrics
///
/// `text_font_metrics` is **required** for layout. The layout function will panic
/// if not provided. Use [`with_text_font_metrics`](Self::with_text_font_metrics) or
/// [`with_font_data`](Self::with_font_data) to set font metrics.
///
/// `symbol_font_metrics` is optional and falls back to `text_font_metrics` if not set.
#[derive(Clone)]
pub struct HarmonyStyle {
    /// Notation style (Standard or Jazz)
    pub notation: ChordNotation,
    /// Font family for rendering text (root, quality text)
    pub font_family: String,
    /// Font family for symbols (accidentals, special characters)
    /// If None, uses font_family
    pub symbol_font_family: Option<String>,
    /// Which symbol set to use (SMuFL, MuseJazz PUA, or Unicode fallback)
    pub symbol_set: SymbolSet,
    /// Root note font size (in points)
    pub root_size: f64,
    /// Scale factor for superscript elements (extensions like 7, alterations like b5)
    /// MuseScore default: 0.75
    pub superscript_scale: f64,
    /// Scale factor for bass note
    /// MuseScore default: 1.0
    pub bass_scale: f64,
    /// Text color
    pub color: Color,
    /// Superscript vertical offset as fraction of root size (negative = up)
    /// MuseScore default: -0.36
    pub superscript_offset: f64,
    /// Optional text font metrics for accurate glyph measurement.
    /// When provided, uses actual font metrics instead of estimation.
    pub text_font_metrics: Option<TextFontMetrics>,
    /// Optional symbol font metrics for symbols.
    pub symbol_font_metrics: Option<TextFontMetrics>,
}

impl std::fmt::Debug for HarmonyStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarmonyStyle")
            .field("notation", &self.notation)
            .field("font_family", &self.font_family)
            .field("symbol_font_family", &self.symbol_font_family)
            .field("symbol_set", &self.symbol_set)
            .field("root_size", &self.root_size)
            .field("superscript_scale", &self.superscript_scale)
            .field("bass_scale", &self.bass_scale)
            .field("color", &self.color)
            .field("superscript_offset", &self.superscript_offset)
            .field("text_font_metrics", &self.text_font_metrics.is_some())
            .field("symbol_font_metrics", &self.symbol_font_metrics.is_some())
            .finish()
    }
}

impl Default for HarmonyStyle {
    fn default() -> Self {
        Self {
            notation: ChordNotation::Standard,
            // Default to sans-serif (FreeSans in demo)
            font_family: "sans-serif".to_string(),
            symbol_font_family: None,
            // Default to Unicode fallback (works with any font)
            symbol_set: SymbolSet::Unicode,
            root_size: 14.0,
            // MuseScore defaults from chords_std.xml
            superscript_scale: 0.75, // <mag>0.75</mag> in type class
            bass_scale: 1.0,         // Same size as root
            color: Color::BLACK,
            // MuseScore's superscript offset: &super; = 0.36
            superscript_offset: -0.36,
            // Font metrics (use estimation if not provided)
            text_font_metrics: None,
            symbol_font_metrics: None,
        }
    }
}

impl HarmonyStyle {
    /// Create a standard notation style.
    #[must_use]
    pub fn standard() -> Self {
        Self::default()
    }

    /// Create a jazz notation style.
    #[must_use]
    pub fn jazz() -> Self {
        Self {
            notation: ChordNotation::Jazz,
            ..Default::default()
        }
    }

    /// Create style using SMuFL symbols from Leland font.
    /// Uses default text font for letters/numbers, Leland for accidentals and quality symbols.
    #[must_use]
    pub fn leland() -> Self {
        Self {
            // Use default text font for ASCII (root notes, numbers, quality text)
            font_family: "sans-serif".to_string(),
            // Use Leland for SMuFL symbols (flat, sharp, triangle, circle, etc.)
            symbol_font_family: Some("Leland".to_string()),
            symbol_set: SymbolSet::Smufl,
            ..Default::default()
        }
    }

    /// Create jazz style using SMuFL symbols from Leland font.
    #[must_use]
    pub fn leland_jazz() -> Self {
        Self {
            notation: ChordNotation::Jazz,
            font_family: "sans-serif".to_string(),
            symbol_font_family: Some("Leland".to_string()),
            symbol_set: SymbolSet::Smufl,
            ..Default::default()
        }
    }

    /// Create style using MuseJazz font (handwritten jazz style).
    /// Uses MuseJazz Text for text and its PUA symbols for chord symbols.
    /// MuseJazz has its own Private Use Area codepoints for triangle, circle, etc.
    /// Note: Font family is "MuseJazz Text" (with space) to match the font's internal name.
    #[must_use]
    pub fn musejazz() -> Self {
        Self {
            // Font internal name is "MuseJazz Text" (with space)
            font_family: "MuseJazz Text".to_string(),
            // Same font for symbols - MuseJazz Text has PUA symbols
            symbol_font_family: None,
            // Use MuseJazz-specific PUA codepoints
            symbol_set: SymbolSet::MuseJazz,
            ..Default::default()
        }
    }

    /// Create jazz notation style using MuseJazz font.
    /// Note: Font family is "MuseJazz Text" (with space) to match the font's internal name.
    #[must_use]
    pub fn musejazz_jazz() -> Self {
        Self {
            notation: ChordNotation::Jazz,
            // Font internal name is "MuseJazz Text" (with space)
            font_family: "MuseJazz Text".to_string(),
            // Same font for symbols
            symbol_font_family: None,
            // Use MuseJazz-specific PUA codepoints
            symbol_set: SymbolSet::MuseJazz,
            ..Default::default()
        }
    }

    /// Enable SMuFL symbols with specified music font.
    #[must_use]
    pub fn with_smufl_font(mut self, font: &str) -> Self {
        self.symbol_font_family = Some(font.to_string());
        self.symbol_set = SymbolSet::Smufl;
        self
    }

    /// Set the symbol set to use.
    #[must_use]
    pub fn with_symbol_set(mut self, symbol_set: SymbolSet) -> Self {
        self.symbol_set = symbol_set;
        self
    }

    /// Set the text font family.
    #[must_use]
    pub fn with_font(mut self, font: &str) -> Self {
        self.font_family = font.to_string();
        self
    }

    /// Set text font metrics for accurate glyph measurement.
    #[must_use]
    pub fn with_text_font_metrics(mut self, metrics: TextFontMetrics) -> Self {
        self.text_font_metrics = Some(metrics);
        self
    }

    /// Set symbol font metrics for accurate SMuFL symbol measurement.
    #[must_use]
    pub fn with_symbol_font_metrics(mut self, metrics: TextFontMetrics) -> Self {
        self.symbol_font_metrics = Some(metrics);
        self
    }

    /// Set both text and symbol font metrics from font data.
    #[must_use]
    pub fn with_font_data(
        mut self,
        text_font_data: Arc<Vec<u8>>,
        symbol_font_data: Option<Arc<Vec<u8>>>,
    ) -> Self {
        self.text_font_metrics = Some(TextFontMetrics::new(text_font_data));
        if let Some(symbol_data) = symbol_font_data {
            self.symbol_font_metrics = Some(TextFontMetrics::new(symbol_data));
        }
        self
    }
}

/// Parameters for chord symbol layout.
#[derive(Debug, Clone)]
pub struct HarmonyParams {
    /// Unique identifier
    pub id: u64,
    /// Root note (C, D, E, F, G, A, B)
    pub root: String,
    /// Accidental for root (empty, "#", "b")
    pub root_accidental: String,
    /// Quality (empty for major, "m" for minor, "dim", "aug")
    pub quality: String,
    /// Extension (empty, "7", "Maj7", "9", "11", "13")
    pub extension: String,
    /// Alterations ("b5", "#5", "b9", "#9", "b13", etc.)
    pub alterations: Vec<String>,
    /// Bass note for slash chords (None if not a slash chord)
    pub bass: Option<String>,
    /// Bass note accidental
    pub bass_accidental: String,
    /// Position (baseline left of root)
    pub position: Point,
    /// Style configuration
    pub style: HarmonyStyle,
}

impl Default for HarmonyParams {
    fn default() -> Self {
        Self {
            id: 0,
            root: "C".to_string(),
            root_accidental: String::new(),
            quality: String::new(),
            extension: String::new(),
            alterations: Vec::new(),
            bass: None,
            bass_accidental: String::new(),
            position: Point::ZERO,
            style: HarmonyStyle::default(),
        }
    }
}

impl HarmonyParams {
    /// Create a simple major chord.
    #[must_use]
    pub fn major(root: &str) -> Self {
        Self {
            root: root.to_string(),
            ..Default::default()
        }
    }

    /// Create a minor chord.
    #[must_use]
    pub fn minor(root: &str) -> Self {
        Self {
            root: root.to_string(),
            quality: "m".to_string(),
            ..Default::default()
        }
    }

    /// Create a seventh chord.
    #[must_use]
    pub fn seventh(root: &str, quality: &str, extension: &str) -> Self {
        Self {
            root: root.to_string(),
            quality: quality.to_string(),
            extension: extension.to_string(),
            ..Default::default()
        }
    }

    /// Set position and return self for chaining.
    #[must_use]
    pub fn at(mut self, x: f64, y: f64) -> Self {
        self.position = Point::new(x, y);
        self
    }

    /// Set style and return self for chaining.
    #[must_use]
    pub fn with_style(mut self, style: HarmonyStyle) -> Self {
        self.style = style;
        self
    }

    /// Add a bass note (slash chord).
    #[must_use]
    pub fn with_bass(mut self, bass: &str) -> Self {
        self.bass = Some(bass.to_string());
        self
    }

    /// Add alterations.
    #[must_use]
    pub fn with_alterations(mut self, alts: &[&str]) -> Self {
        self.alterations = alts.iter().map(|s| (*s).to_string()).collect();
        self
    }
}

/// Layout data returned from harmony layout.
#[derive(Debug, Clone)]
pub struct HarmonyLayoutData {
    /// Bounding box of the entire chord symbol
    pub bounds: Rect,
    /// Total width
    pub width: f64,
    /// Height (from baseline to top of superscripts)
    pub height: f64,
    /// Baseline Y position
    pub baseline: f64,
}

/// Layout a chord symbol.
///
/// Returns layout data and a scene node containing all text segments.
///
/// Layout approach follows MuseScore's chords_std.xml conventions:
/// - Spacing is measured in cap-height units (approximately 0.7 × font size)
/// - Root note renders at baseline, accidental follows with padding
/// - Quality (m, dim, etc.) renders at baseline after root
/// - Extensions (7, Maj7) render as superscript (0.75 scale, -0.36 offset)
/// - Slash chords have minimal spacing around the slash
///
/// # Arguments
///
/// * `params` - Harmony parameters
/// * `_ctx` - Layout context
///
/// # Returns
///
/// A tuple of (layout data, scene node).
#[must_use]
pub fn layout_harmony(
    params: &HarmonyParams,
    _ctx: &LayoutContext<'_>,
) -> (HarmonyLayoutData, SceneNode) {
    let style = &params.style;
    let mut commands = Vec::new();
    // Layout at local origin - transform will position in world space
    let mut cursor_x = 0.0;
    let baseline_y = 0.0;

    // Font metrics are required for accurate glyph measurement
    let text_metrics = style.text_font_metrics.as_ref().expect(
        "HarmonyStyle requires text_font_metrics for layout. \
         Use HarmonyStyle::with_text_font_metrics() to provide font metrics.",
    );
    let symbol_metrics = style.symbol_font_metrics.as_ref().unwrap_or(text_metrics);

    // Measure text width using actual font metrics (no estimation fallback)
    let measure_text_width = |text: &str, font_size: f64, use_symbol_font: bool| -> f64 {
        let metrics = if use_symbol_font {
            symbol_metrics
        } else {
            text_metrics
        };
        metrics.horizontal_advance(text, font_size)
    };

    // Get ACTUAL cap-height from font metrics (like MuseScore does)
    // MuseScore: harmonyCtx.pos += moveValue * FontMetrics::capHeight(font) * scale
    let cap_height = text_metrics.cap_height(style.root_size);

    // MuseScore spacing from chords_jazz.xml (exact values in cap-height units):
    // - renderRoot: `:n :a m:0.036:0` (root, accidental, then move right)
    // - accidental token: `ms:0.1:0 b ms:0.1:0` (scaled padding each side)
    // - renderBass: `m:-0.014:0 / m:0.014:0 :n :a` (NEGATIVE tightens before slash)
    let space_after_root_acc = 0.036 * cap_height;
    let accidental_padding = 0.1 * cap_height;
    let space_before_slash = -0.014 * cap_height; // NEGATIVE moves left (tightens)
    let space_after_slash = 0.014 * cap_height;

    // Font to use for symbol glyphs (accidentals, special chars)
    let symbol_font = style
        .symbol_font_family
        .as_ref()
        .unwrap_or(&style.font_family);

    // Track previous character for kerning
    let mut prev_char: Option<char> = None;

    // Helper for kerning adjustments (values from MuseScore harmonylayout.cpp KERNED_CHARACTERS)
    // Returns kerning in cap-height units, caller multiplies by cap_height
    let get_kerning = |prev: Option<char>, next_text: &str, notation: ChordNotation| -> f64 {
        let next = next_text.chars().next();
        match (prev, next, notation) {
            // A followed by dim/half-dim symbols needs tightening
            (Some('A'), Some('\u{E870}'), _) => -0.4, // A + dim (SMuFL)
            (Some('A'), Some('\u{E871}'), _) => -0.3, // A + half-dim (SMuFL)
            (Some('A'), Some('\u{E18E}'), _) => -0.15, // A + dim (MuseJazz)
            (Some('A'), Some('\u{E18F}'), _) => -0.15, // A + half-dim (MuseJazz)
            // Triangle followed by dim/half-dim
            (Some('\u{E873}'), Some('\u{E870}'), _) => -0.4,
            (Some('\u{E873}'), Some('\u{E871}'), _) => -0.3,
            (Some('\u{E18A}'), Some('\u{E18E}'), _) => -0.15,
            (Some('\u{E18A}'), Some('\u{E18F}'), _) => -0.15,
            // A followed by slash needs slight expansion
            (Some('A'), Some('/'), _) => 0.1,
            // Jazz accidentals need tightening with following characters
            (Some('\u{266D}'), _, ChordNotation::Jazz) => -0.15, // ♭
            (Some('\u{266F}'), _, ChordNotation::Jazz) => -0.15, // ♯
            (Some('\u{266E}'), _, ChordNotation::Jazz) => -0.15, // ♮
            _ => 0.0,
        }
    };

    // 1. Root note (letter) - always use text font
    let root_letter = &params.root;
    let root_width = measure_text_width(root_letter, style.root_size, false);
    commands.push(PaintCommand::text(
        root_letter.clone(),
        &style.font_family,
        style.root_size,
        Point::new(cursor_x, baseline_y),
        style.color,
    ));
    cursor_x += root_width;
    prev_char = root_letter.chars().last();

    // Root accidental (if any) - use symbol font with padding
    if !params.root_accidental.is_empty() {
        let acc_text = format_accidental(&params.root_accidental, style.symbol_set);

        // Padding before accidental (already in absolute units from cap_height)
        cursor_x += accidental_padding;

        let acc_width = measure_text_width(&acc_text, style.root_size, true);
        commands.push(PaintCommand::text(
            acc_text.clone(),
            symbol_font,
            style.root_size,
            Point::new(cursor_x, baseline_y),
            style.color,
        ));
        cursor_x += acc_width;

        // Padding after accidental
        cursor_x += accidental_padding;
        prev_char = acc_text.chars().last();
    }

    // Add minimal spacing after root+accidental (m:0.036:0 in MuseScore)
    cursor_x += space_after_root_acc;

    // 2. Quality (m, dim, aug, or jazz symbols)
    let quality_text = format_quality(&params.quality, style.notation, style.symbol_set);
    let has_quality = !quality_text.is_empty();
    if has_quality {
        // Apply kerning adjustment (kern value * cap_height, matching MuseScore)
        let kern = get_kerning(prev_char, &quality_text, style.notation);
        cursor_x += kern * cap_height;

        // Use symbol font for special characters (°, +, etc.), text font for letters
        let is_symbol = quality_text.chars().all(|c| !c.is_ascii_alphabetic());
        let quality_font = if is_symbol {
            symbol_font
        } else {
            &style.font_family
        };
        let quality_width = measure_text_width(&quality_text, style.root_size, is_symbol);
        commands.push(PaintCommand::text(
            quality_text.clone(),
            quality_font,
            style.root_size,
            Point::new(cursor_x, baseline_y),
            style.color,
        ));
        cursor_x += quality_width;
        prev_char = quality_text.chars().last();
    }

    // 3. Extension (7, Maj7, 9, etc.) - superscript position with 0.75 scale
    let has_extension = !params.extension.is_empty();
    let has_alterations = !params.alterations.is_empty();

    if has_extension {
        let ext_text = format_extension(&params.extension, style.notation, style.symbol_set);
        let ext_size = style.root_size * style.superscript_scale;
        // Superscript offset: -0.36 cap-height (negative = up)
        let ext_y = baseline_y + style.superscript_offset * cap_height;

        // Apply kerning adjustment
        let kern = get_kerning(prev_char, &ext_text, style.notation);
        cursor_x += kern * cap_height;

        // Use symbol font if text contains special symbols (triangle, oslash)
        let has_symbols = ext_text.chars().any(|c| !c.is_ascii_alphanumeric());
        let ext_font = if has_symbols {
            symbol_font
        } else {
            &style.font_family
        };
        let ext_width = measure_text_width(&ext_text, ext_size, has_symbols);
        commands.push(PaintCommand::text(
            ext_text.clone(),
            ext_font,
            ext_size,
            Point::new(cursor_x, ext_y),
            style.color,
        ));
        cursor_x += ext_width;
        prev_char = ext_text.chars().last();
    }

    // 4. Alterations (b5, #9, etc.) - superscript position with 0.75 scale
    if has_alterations {
        let alt_size = style.root_size * style.superscript_scale;
        let alt_y = baseline_y + style.superscript_offset * cap_height;

        for alt in &params.alterations {
            let alt_text = format_alteration(alt, style.symbol_set);

            // Apply kerning adjustment (scaled for smaller text)
            let kern = get_kerning(prev_char, &alt_text, style.notation);
            cursor_x += kern * cap_height * style.superscript_scale;

            let alt_width = measure_text_width(&alt_text, alt_size, true);
            commands.push(PaintCommand::text(
                alt_text.clone(),
                symbol_font,
                alt_size,
                Point::new(cursor_x, alt_y),
                style.color,
            ));
            cursor_x += alt_width;
            prev_char = alt_text.chars().last();
        }
    }

    // 5. Bass note (slash chord)
    if let Some(bass) = &params.bass {
        let bass_size = style.root_size * style.bass_scale;

        // Move slightly left before slash (m:-0.014:0 in MuseScore - unscaled)
        cursor_x += space_before_slash;

        // Slash - use text font
        let slash_width = measure_text_width("/", bass_size, false);
        commands.push(PaintCommand::text(
            "/",
            &style.font_family,
            bass_size,
            Point::new(cursor_x, baseline_y),
            style.color,
        ));
        cursor_x += slash_width;

        // Small space after slash (m:0.014:0 in MuseScore - unscaled)
        cursor_x += space_after_slash;

        // Bass note letter
        let bass_width = measure_text_width(bass, bass_size, false);
        commands.push(PaintCommand::text(
            bass.clone(),
            &style.font_family,
            bass_size,
            Point::new(cursor_x, baseline_y),
            style.color,
        ));
        cursor_x += bass_width;

        // Bass accidental (if any)
        if !params.bass_accidental.is_empty() {
            let bass_acc_text = format_accidental(&params.bass_accidental, style.symbol_set);

            // Padding before bass accidental (scaled for bass size)
            cursor_x += accidental_padding * style.bass_scale;

            let bass_acc_width = measure_text_width(&bass_acc_text, bass_size, true);
            commands.push(PaintCommand::text(
                bass_acc_text,
                symbol_font,
                bass_size,
                Point::new(cursor_x, baseline_y),
                style.color,
            ));
            cursor_x += bass_acc_width;
        }
    }

    // Calculate bounds in local coordinates
    let total_width = cursor_x;
    let total_height = style.root_size * 1.2; // Include superscript height
    let local_bounds = Rect::new(
        0.0,
        baseline_y - style.root_size,
        cursor_x,
        baseline_y + style.root_size * 0.2,
    );

    // World bounds for layout data (offset by position)
    let world_bounds = Rect::new(
        params.position.x,
        params.position.y - style.root_size,
        params.position.x + cursor_x,
        params.position.y + style.root_size * 0.2,
    );

    let layout_data = HarmonyLayoutData {
        bounds: world_bounds,
        width: total_width,
        height: total_height,
        baseline: params.position.y,
    };

    // Create scene node with semantic ID and transform
    let mut node = SceneNode::leaf(
        SemanticId::new(ElementType::ChordSymbol, params.id),
        commands,
    )
    .with_bounds(local_bounds);
    node.transform = kurbo::Affine::translate((params.position.x, params.position.y));

    (layout_data, node)
}

/// Which symbol set to use for chord symbols.
/// Different fonts have different codepoints for the same symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolSet {
    /// SMuFL standard codepoints (Leland, Bravura, Petaluma)
    /// Uses codepoints like U+E873 (csymMajorSeventh), U+E870 (csymDiminished)
    #[default]
    Smufl,
    /// MuseJazz-specific Private Use Area codepoints
    /// Uses codepoints like U+E18A (triangle), U+E18E (circle)
    MuseJazz,
    /// Standard Unicode fallback (works with any font)
    /// Uses codepoints like U+25B3 (△), U+00B0 (°)
    Unicode,
}

/// SMuFL chord symbols (for music fonts like Leland, Bravura, Petaluma)
/// These codepoints are part of the SMuFL standard and work with any SMuFL-compliant font.
pub mod smufl {
    /// Major seventh triangle (△) - SMuFL csymMajorSeventh
    pub const MAJOR_SEVENTH: char = '\u{E873}';
    /// Diminished circle (°) - SMuFL csymDiminished
    pub const DIMINISHED: char = '\u{E870}';
    /// Half-diminished circle with slash (ø) - SMuFL csymHalfDiminished
    pub const HALF_DIMINISHED: char = '\u{E871}';
    /// Augmented (+) - SMuFL csymAugmented
    pub const AUGMENTED: char = '\u{E872}';
    /// Flat - SMuFL csymAccidentalFlat
    pub const FLAT: char = '\u{ED60}';
    /// Sharp - SMuFL csymAccidentalSharp
    pub const SHARP: char = '\u{ED62}';
    /// Natural - SMuFL csymAccidentalNatural
    pub const NATURAL: char = '\u{ED61}';
    /// Double flat - SMuFL csymAccidentalDoubleFlat
    pub const DOUBLE_FLAT: char = '\u{ED64}';
    /// Double sharp - SMuFL csymAccidentalDoubleSharp
    pub const DOUBLE_SHARP: char = '\u{ED63}';
    /// Minus for alterations
    pub const MINUS: char = '-';
}

/// MuseJazz-specific Private Use Area codepoints.
/// From chords_jazz.xml in MuseScore - these work with MuseJazz Text font.
pub mod musejazz {
    /// Major seventh triangle (△) - MuseJazz PUA
    pub const TRIANGLE: char = '\u{E18A}';
    /// Diminished circle (°) - MuseJazz PUA
    pub const CIRCLE: char = '\u{E18E}';
    /// Half-diminished circle with slash (ø) - MuseJazz PUA
    pub const OSLASH: char = '\u{E18F}';
    /// Augmented (+) - MuseJazz PUA
    pub const PLUS: char = '\u{E186}';
    /// Degree symbol - MuseJazz PUA
    pub const DEGREE: char = '\u{E187}';
    /// Flat (♭) - Standard Unicode (works in MuseJazz)
    pub const FLAT: char = '\u{266D}';
    /// Sharp (♯) - Standard Unicode (works in MuseJazz)
    pub const SHARP: char = '\u{266F}';
    /// Natural (♮) - Standard Unicode
    pub const NATURAL: char = '\u{266E}';
    /// Double sharp - Unicode
    pub const DOUBLE_SHARP: char = '\u{1D12A}';
    /// Double flat - Unicode
    pub const DOUBLE_FLAT: char = '\u{1D12B}';
    /// Flat modifier (smaller, for alterations) - MuseJazz PUA
    pub const MODIFIER_FLAT: char = '\u{E10D}';
    /// Sharp modifier (smaller, for alterations) - MuseJazz PUA
    pub const MODIFIER_SHARP: char = '\u{E10C}';
}

/// Standard Unicode chord symbols (fallback for non-SMuFL fonts)
mod unicode_fallback {
    /// Triangle for major (△) - Standard Unicode
    pub const TRIANGLE: char = '\u{25B3}';
    /// Circle for diminished (°) - Degree symbol
    pub const CIRCLE: char = '\u{00B0}';
    /// Half-diminished circle with slash (ø) - Latin small letter O with stroke
    pub const OSLASH: char = '\u{00F8}';
    /// Plus for augmented
    pub const PLUS: char = '+';
    /// Flat (♭) - Music flat sign
    pub const FLAT: char = '\u{266D}';
    /// Sharp (♯) - Music sharp sign
    pub const SHARP: char = '\u{266F}';
    /// Natural (♮) - Music natural sign
    pub const NATURAL: char = '\u{266E}';
    /// Double sharp - use superscript x
    pub const DOUBLE_SHARP: &str = "x";
    /// Double flat
    pub const DOUBLE_FLAT: &str = "bb";
}

/// Format accidental for display.
fn format_accidental(acc: &str, symbol_set: SymbolSet) -> String {
    match symbol_set {
        SymbolSet::Smufl => match acc {
            "#" => smufl::SHARP.to_string(),
            "b" => smufl::FLAT.to_string(),
            "##" => smufl::DOUBLE_SHARP.to_string(),
            "bb" => smufl::DOUBLE_FLAT.to_string(),
            _ => acc.to_string(),
        },
        SymbolSet::MuseJazz => match acc {
            "#" => musejazz::SHARP.to_string(),
            "b" => musejazz::FLAT.to_string(),
            "##" => musejazz::DOUBLE_SHARP.to_string(),
            "bb" => musejazz::DOUBLE_FLAT.to_string(),
            _ => acc.to_string(),
        },
        SymbolSet::Unicode => match acc {
            "#" => unicode_fallback::SHARP.to_string(),
            "b" => unicode_fallback::FLAT.to_string(),
            "##" => unicode_fallback::DOUBLE_SHARP.to_string(),
            "bb" => unicode_fallback::DOUBLE_FLAT.to_string(),
            _ => acc.to_string(),
        },
    }
}

/// Format quality for display based on notation style and symbol set.
fn format_quality(quality: &str, notation: ChordNotation, symbol_set: SymbolSet) -> String {
    match (quality, notation, symbol_set) {
        // Jazz uses special symbols for diminished and augmented
        ("dim", ChordNotation::Jazz, SymbolSet::Smufl) => smufl::DIMINISHED.to_string(),
        ("aug", ChordNotation::Jazz, SymbolSet::Smufl) => smufl::AUGMENTED.to_string(),
        ("dim", ChordNotation::Jazz, SymbolSet::MuseJazz) => musejazz::CIRCLE.to_string(),
        ("aug", ChordNotation::Jazz, SymbolSet::MuseJazz) => musejazz::PLUS.to_string(),
        ("dim", ChordNotation::Jazz, SymbolSet::Unicode) => unicode_fallback::CIRCLE.to_string(),
        ("aug", ChordNotation::Jazz, SymbolSet::Unicode) => unicode_fallback::PLUS.to_string(),
        // Standard uses text for diminished, but "+" symbol for augmented (like MuseScore)
        ("dim", ChordNotation::Standard, _) => "dim".to_string(),
        // Augmented uses "+" symbol in both Standard and Jazz notation
        ("aug", ChordNotation::Standard, SymbolSet::Smufl) => smufl::AUGMENTED.to_string(),
        ("aug", ChordNotation::Standard, SymbolSet::MuseJazz) => musejazz::PLUS.to_string(),
        ("aug", ChordNotation::Standard, SymbolSet::Unicode) => unicode_fallback::PLUS.to_string(),
        // Minor, power chord, suspended are the same in all styles
        ("m", _, _) => "m".to_string(),
        ("5", _, _) => "5".to_string(),
        ("sus2", _, _) => "sus2".to_string(),
        ("sus4", _, _) => "sus4".to_string(),
        _ => quality.to_string(),
    }
}

/// Format extension for display based on notation style and symbol set.
fn format_extension(ext: &str, notation: ChordNotation, symbol_set: SymbolSet) -> String {
    match (ext, notation, symbol_set) {
        // Jazz uses triangle for major 7/9/13
        ("Maj7", ChordNotation::Jazz, SymbolSet::Smufl) => format!("{}7", smufl::MAJOR_SEVENTH),
        ("Maj9", ChordNotation::Jazz, SymbolSet::Smufl) => format!("{}9", smufl::MAJOR_SEVENTH),
        ("Maj13", ChordNotation::Jazz, SymbolSet::Smufl) => format!("{}13", smufl::MAJOR_SEVENTH),
        ("Maj7", ChordNotation::Jazz, SymbolSet::MuseJazz) => format!("{}7", musejazz::TRIANGLE),
        ("Maj9", ChordNotation::Jazz, SymbolSet::MuseJazz) => format!("{}9", musejazz::TRIANGLE),
        ("Maj13", ChordNotation::Jazz, SymbolSet::MuseJazz) => format!("{}13", musejazz::TRIANGLE),
        ("Maj7", ChordNotation::Jazz, SymbolSet::Unicode) => {
            format!("{}7", unicode_fallback::TRIANGLE)
        }
        ("Maj9", ChordNotation::Jazz, SymbolSet::Unicode) => {
            format!("{}9", unicode_fallback::TRIANGLE)
        }
        ("Maj13", ChordNotation::Jazz, SymbolSet::Unicode) => {
            format!("{}13", unicode_fallback::TRIANGLE)
        }
        // Half-diminished (m7b5) in jazz uses oslash
        ("7b5", ChordNotation::Jazz, SymbolSet::Smufl) => format!("{}7", smufl::HALF_DIMINISHED),
        ("7b5", ChordNotation::Jazz, SymbolSet::MuseJazz) => format!("{}7", musejazz::OSLASH),
        ("7b5", ChordNotation::Jazz, SymbolSet::Unicode) => {
            format!("{}7", unicode_fallback::OSLASH)
        }
        // Standard notation - no symbol replacement
        _ => ext.to_string(),
    }
}

/// Format alteration for display using proper accidental symbols.
fn format_alteration(alt: &str, symbol_set: SymbolSet) -> String {
    match symbol_set {
        SymbolSet::Smufl => alt
            .replace('b', &smufl::FLAT.to_string())
            .replace('#', &smufl::SHARP.to_string()),
        SymbolSet::MuseJazz => alt
            .replace('b', &musejazz::MODIFIER_FLAT.to_string())
            .replace('#', &musejazz::MODIFIER_SHARP.to_string()),
        SymbolSet::Unicode => alt
            .replace('b', &unicode_fallback::FLAT.to_string())
            .replace('#', &unicode_fallback::SHARP.to_string()),
    }
}

/// Convenience function to create a chord symbol from a string like "Cm7b5".
///
/// Parses common chord symbol formats and returns HarmonyParams.
#[must_use]
pub fn parse_chord(chord_str: &str) -> HarmonyParams {
    let mut params = HarmonyParams::default();
    let chars: Vec<char> = chord_str.chars().collect();
    let mut i = 0;

    // Parse root note (A-G)
    if i < chars.len() && chars[i].is_ascii_uppercase() {
        params.root = chars[i].to_string();
        i += 1;
    }

    // Parse root accidental (# or b)
    if i < chars.len() && (chars[i] == '#' || chars[i] == 'b') {
        params.root_accidental = chars[i].to_string();
        i += 1;
    }

    let remaining: String = chars[i..].iter().collect();

    // Check for slash chord
    let (main_part, bass_part) = if let Some(slash_pos) = remaining.find('/') {
        let (main, bass) = remaining.split_at(slash_pos);
        (main.to_string(), Some(bass[1..].to_string()))
    } else {
        (remaining, None)
    };

    // Parse quality and extensions from main part
    parse_quality_and_extensions(&main_part, &mut params);

    // Parse bass note
    if let Some(bass) = bass_part {
        let bass_chars: Vec<char> = bass.chars().collect();
        if !bass_chars.is_empty() {
            params.bass = Some(bass_chars[0].to_string());
            if bass_chars.len() > 1 && (bass_chars[1] == '#' || bass_chars[1] == 'b') {
                params.bass_accidental = bass_chars[1].to_string();
            }
        }
    }

    params
}

/// Parse quality and extensions from the remaining chord string.
fn parse_quality_and_extensions(s: &str, params: &mut HarmonyParams) {
    // Common patterns to match (longer patterns MUST come before shorter ones that are prefixes)
    // NOTE: "maj" variants must come before "m" to avoid "maj7" being parsed as minor!
    let patterns = [
        // Major 7th variants (MUST come before "m" patterns!)
        ("maj13", "", "Maj13"),
        ("maj9", "", "Maj9"),
        ("maj7", "", "Maj7"),
        ("Maj13", "", "Maj13"),
        ("Maj9", "", "Maj9"),
        ("Maj7", "", "Maj7"),
        ("M7", "", "Maj7"),
        ("M9", "", "Maj9"),
        ("M13", "", "Maj13"),
        // Minor-major patterns
        ("mMaj9", "m", "Maj9"),
        ("mMaj7", "m", "Maj7"),
        ("mMaj", "m", "Maj"),
        ("minMaj9", "m", "Maj9"),
        ("minMaj7", "m", "Maj7"),
        // Half-diminished (minor 7 flat 5)
        ("m7b5", "m", "7b5"),
        ("min7b5", "m", "7b5"),
        ("ø7", "m", "7b5"),
        ("ø", "m", "7b5"),
        // Minor patterns
        ("min7", "m", "7"),
        ("min9", "m", "9"),
        ("min11", "m", "11"),
        ("min13", "m", "13"),
        ("min6", "m", "6"),
        ("min", "m", ""),
        ("m7", "m", "7"),
        ("m9", "m", "9"),
        ("m11", "m", "11"),
        ("m13", "m", "13"),
        ("m6", "m", "6"),
        ("m", "m", ""),
        ("-7", "m", "7"),
        ("-9", "m", "9"),
        ("-", "m", ""),
        // Diminished
        ("dim7", "dim", "7"),
        ("dim9", "dim", "9"),
        ("dim", "dim", ""),
        ("o7", "dim", "7"),
        ("°7", "dim", "7"),
        ("o", "dim", ""),
        ("°", "dim", ""),
        // Augmented
        ("aug7", "aug", "7"),
        ("aug9", "aug", "9"),
        ("aug", "aug", ""),
        ("+7", "aug", "7"),
        ("+", "aug", ""),
        // Suspended (basic)
        ("sus2", "sus2", ""),
        ("sus4", "sus4", ""),
        ("sus", "sus4", ""),
        // Suspended dominant (must come before basic 7, 9, 11, 13)
        // Note: For these, we put the full text in extension to get correct ordering (e.g., "7sus4" not "sus47")
        ("13sus4", "", "13sus4"),
        ("13sus2", "", "13sus2"),
        ("11sus4", "", "11sus4"),
        ("11sus2", "", "11sus2"),
        ("9sus4", "", "9sus4"),
        ("9sus2", "", "9sus2"),
        ("7sus4", "", "7sus4"),
        ("7sus2", "", "7sus2"),
        // Add chords
        ("add9", "", "add9"),
        ("add2", "", "add2"),
        ("add11", "", "add11"),
        // Dominant extensions with alterations
        ("7b9", "", "7b9"),
        ("7#9", "", "7#9"),
        ("7b5", "", "7b5"),
        ("7#5", "", "7#5"),
        ("7alt", "", "7alt"),
        ("9b5", "", "9b5"),
        ("9#5", "", "9#5"),
        // Basic extensions
        ("13", "", "13"),
        ("11", "", "11"),
        ("9", "", "9"),
        ("7", "", "7"),
        ("69", "", "69"),
        ("6", "", "6"),
        // Power chord
        ("5", "5", ""),
    ];

    for (pattern, quality, extension) in patterns {
        if let Some(remaining) = s.strip_prefix(pattern) {
            params.quality = quality.to_string();
            params.extension = extension.to_string();

            // Check for remaining alterations
            if !remaining.is_empty() {
                // Parse alterations like b5, #9, etc.
                let mut alt_str = remaining.to_string();
                while !alt_str.is_empty() {
                    if alt_str.starts_with("b5")
                        || alt_str.starts_with("#5")
                        || alt_str.starts_with("b9")
                        || alt_str.starts_with("#9")
                        || alt_str.starts_with("b13")
                        || alt_str.starts_with("#13")
                        || alt_str.starts_with("b11")
                        || alt_str.starts_with("#11")
                    {
                        let alt = if alt_str.len() >= 3
                            && alt_str.chars().nth(2).is_some_and(|c| c.is_ascii_digit())
                        {
                            alt_str[..3].to_string()
                        } else {
                            alt_str[..2].to_string()
                        };
                        params.alterations.push(alt.clone());
                        alt_str = alt_str[alt.len()..].to_string();
                    } else {
                        break;
                    }
                }
            }
            return;
        }
    }

    // No match - treat as major chord
    params.quality = String::new();
    params.extension = s.to_string();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engraver::layout::context::LayoutConfiguration;
    use crate::engraver::layout::text_metrics::TextFontMetrics;
    use crate::engraver::style::MStyle;

    /// Embedded minimal TrueType font for tests (reuse from fonts module).
    /// Note: For accurate text measurement tests, use a real font like FreeSans.
    static TEST_FONT_DATA: &[u8] = crate::engraver::fonts::EMPTY_FONT_DATA_FOR_TESTS;

    fn make_ctx() -> LayoutContext<'static> {
        let style = Box::leak(Box::new(MStyle::default()));
        LayoutContext::new_for_test(LayoutConfiguration::default(), style)
    }

    /// Create a test HarmonyStyle with actual font metrics from FreeSans.
    fn make_test_style() -> HarmonyStyle {
        let font_data = Arc::new(TEST_FONT_DATA.to_vec());
        HarmonyStyle::default().with_text_font_metrics(TextFontMetrics::new(font_data))
    }

    #[test]
    fn test_parse_chord_major() {
        let params = parse_chord("C");
        assert_eq!(params.root, "C");
        assert_eq!(params.quality, "");
        assert_eq!(params.extension, "");
    }

    #[test]
    fn test_parse_chord_minor() {
        let params = parse_chord("Am");
        assert_eq!(params.root, "A");
        assert_eq!(params.quality, "m");
    }

    #[test]
    fn test_parse_chord_seventh() {
        let params = parse_chord("G7");
        assert_eq!(params.root, "G");
        assert_eq!(params.quality, "");
        assert_eq!(params.extension, "7");
    }

    #[test]
    fn test_parse_chord_minor_seventh() {
        let params = parse_chord("Dm7");
        assert_eq!(params.root, "D");
        assert_eq!(params.quality, "m");
        assert_eq!(params.extension, "7");
    }

    #[test]
    fn test_parse_chord_major_seventh() {
        let params = parse_chord("FMaj7");
        assert_eq!(params.root, "F");
        assert_eq!(params.quality, "");
        assert_eq!(params.extension, "Maj7");
    }

    #[test]
    fn test_parse_chord_with_accidental() {
        let params = parse_chord("F#m7");
        assert_eq!(params.root, "F");
        assert_eq!(params.root_accidental, "#");
        assert_eq!(params.quality, "m");
        assert_eq!(params.extension, "7");
    }

    #[test]
    fn test_parse_chord_slash() {
        let params = parse_chord("C/E");
        assert_eq!(params.root, "C");
        assert_eq!(params.bass, Some("E".to_string()));
    }

    #[test]
    fn test_parse_chord_diminished() {
        let params = parse_chord("Bdim7");
        assert_eq!(params.root, "B");
        assert_eq!(params.quality, "dim");
        assert_eq!(params.extension, "7");
    }

    #[test]
    fn test_parse_chord_half_diminished() {
        let params = parse_chord("Bm7b5");
        assert_eq!(params.root, "B");
        assert_eq!(params.quality, "m");
        assert_eq!(params.extension, "7b5");
    }

    #[test]
    fn test_layout_harmony_basic() {
        let ctx = make_ctx();
        let params = HarmonyParams::major("C")
            .at(100.0, 50.0)
            .with_style(make_test_style());
        let (layout, node) = layout_harmony(&params, &ctx);

        assert!(layout.width > 0.0);
        assert!(!node.commands.is_empty());
    }

    #[test]
    fn test_format_quality_jazz_unicode() {
        // Test Unicode fallback
        assert_eq!(
            format_quality("dim", ChordNotation::Jazz, SymbolSet::Unicode),
            "\u{00B0}"
        );
        assert_eq!(
            format_quality("aug", ChordNotation::Jazz, SymbolSet::Unicode),
            "+"
        );
    }

    #[test]
    fn test_format_quality_jazz_smufl() {
        // Test SMuFL symbols
        assert_eq!(
            format_quality("dim", ChordNotation::Jazz, SymbolSet::Smufl),
            "\u{E870}"
        );
        assert_eq!(
            format_quality("aug", ChordNotation::Jazz, SymbolSet::Smufl),
            "\u{E872}"
        );
    }

    #[test]
    fn test_format_quality_jazz_musejazz() {
        // Test MuseJazz PUA symbols
        assert_eq!(
            format_quality("dim", ChordNotation::Jazz, SymbolSet::MuseJazz),
            "\u{E18E}"
        );
        assert_eq!(
            format_quality("aug", ChordNotation::Jazz, SymbolSet::MuseJazz),
            "\u{E186}"
        );
    }

    #[test]
    fn test_format_extension_jazz_unicode() {
        // Test Unicode fallback
        assert_eq!(
            format_extension("Maj7", ChordNotation::Jazz, SymbolSet::Unicode),
            "\u{25B3}7"
        );
    }

    #[test]
    fn test_format_extension_jazz_smufl() {
        // Test SMuFL symbols
        assert_eq!(
            format_extension("Maj7", ChordNotation::Jazz, SymbolSet::Smufl),
            "\u{E873}7"
        );
    }

    #[test]
    fn test_format_extension_jazz_musejazz() {
        // Test MuseJazz PUA symbols
        assert_eq!(
            format_extension("Maj7", ChordNotation::Jazz, SymbolSet::MuseJazz),
            "\u{E18A}7"
        );
    }

    #[test]
    fn test_format_quality_standard_augmented_uses_plus_symbol() {
        // Standard notation should render "aug" as "+" symbol, not text
        // This matches MuseScore's behavior for better readability
        assert_eq!(
            format_quality("aug", ChordNotation::Standard, SymbolSet::Unicode),
            "+"
        );
        assert_eq!(
            format_quality("aug", ChordNotation::Standard, SymbolSet::Smufl),
            "\u{E872}" // SMuFL csymAugmented
        );
        assert_eq!(
            format_quality("aug", ChordNotation::Standard, SymbolSet::MuseJazz),
            "\u{E186}" // MuseJazz PLUS
        );
    }

    #[test]
    fn test_format_quality_standard_diminished_uses_text() {
        // Standard notation should still use "dim" text for diminished
        assert_eq!(
            format_quality("dim", ChordNotation::Standard, SymbolSet::Unicode),
            "dim"
        );
        assert_eq!(
            format_quality("dim", ChordNotation::Standard, SymbolSet::Smufl),
            "dim"
        );
        assert_eq!(
            format_quality("dim", ChordNotation::Standard, SymbolSet::MuseJazz),
            "dim"
        );
    }
}
