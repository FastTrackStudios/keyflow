//! Clean facade API for engraving workflows.
//!
//! This module provides a small, ergonomic layer over the engraver pipeline,
//! leaving the full surface area available under `engraver::*`.

use std::error::Error;
use std::fmt;

/// Commonly used types for chart engraving.
pub mod prelude {
    pub use crate::api::style::{
        leak_default_style, leak_jazz_lead_sheet_style, leak_lead_sheet_style,
    };
    pub use crate::engraver::fonts::ChartFontBundle;
    pub use crate::engraver::layout::chart::{ChartLayoutEngine, ChartLayoutResult, LayoutMode};
    pub use crate::engraver::style::MStyle;
    pub use keyflow_proto::{Chart, ParseError};
}

/// Convenience helpers for obtaining a `'static` style.
pub mod style {
    use crate::engraver::style::MStyle;

    /// Leak a style to obtain a `'static` reference for chart layout engines.
    #[must_use]
    pub fn leak_style(style: MStyle) -> &'static MStyle {
        Box::leak(Box::new(style))
    }

    /// Default engraving style.
    #[must_use]
    pub fn leak_default_style() -> &'static MStyle {
        leak_style(MStyle::default())
    }

    /// Lead sheet style preset.
    #[must_use]
    pub fn leak_lead_sheet_style() -> &'static MStyle {
        leak_style(MStyle::lead_sheet())
    }

    /// Jazz lead sheet style preset.
    #[must_use]
    pub fn leak_jazz_lead_sheet_style() -> &'static MStyle {
        leak_style(MStyle::jazz_lead_sheet())
    }
}

/// Errors returned by the facade helpers.
#[derive(Debug)]
pub enum ChartLayoutError {
    Parse(String),
    Fonts(String),
}

impl fmt::Display for ChartLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "chart parse error: {err}"),
            Self::Fonts(err) => write!(f, "font bundle error: {err}"),
        }
    }
}

impl Error for ChartLayoutError {}

/// Chart engraving helpers (keyflow chart → layout).
pub mod chart {
    use super::{ChartLayoutError, style};
    use crate::engraver::fonts::ChartFontBundle;
    use crate::engraver::layout::chart::{ChartLayoutEngine, ChartLayoutResult, LayoutMode};
    use crate::engraver::style::MStyle;
    use keyflow_proto::Chart;

    /// Create a chart layout engine from a style and font bundle.
    #[must_use]
    pub fn engine(style: &'static MStyle, fonts: &ChartFontBundle) -> ChartLayoutEngine {
        fonts.create_layout_engine(style)
    }

    /// Layout an already-parsed chart using the provided engine and layout mode.
    #[must_use]
    pub fn layout(
        chart: &Chart,
        engine: &ChartLayoutEngine,
        mode: &LayoutMode,
    ) -> ChartLayoutResult {
        engine.layout_chart(chart, mode)
    }

    /// Parse and layout chart text with default fonts and a lead sheet style.
    pub fn layout_text(
        text: &str,
        mode: &LayoutMode,
    ) -> Result<ChartLayoutResult, ChartLayoutError> {
        let chart = keyflow_text::chart::parse_chart(text).map_err(ChartLayoutError::Parse)?;
        layout_chart(&chart, mode)
    }

    /// Layout a chart with default fonts and a lead sheet style.
    pub fn layout_chart(
        chart: &Chart,
        mode: &LayoutMode,
    ) -> Result<ChartLayoutResult, ChartLayoutError> {
        let fonts = ChartFontBundle::new().map_err(ChartLayoutError::Fonts)?;
        let style = style::leak_lead_sheet_style();
        let engine = fonts.create_layout_engine(style);
        Ok(engine.layout_chart(chart, mode))
    }
}

/// One owner of fonts, layout and export.
///
/// Engraving a chart is always the same four steps: build the font
/// bundle, build a layout engine from it, lay the chart out, serialise
/// the result. Before this existed, **three** places did all four
/// independently — `keyflow-ui`'s `ChartLayoutManager`, the CLI's
/// `LayoutPipeline`, and `editor-keyflow` — and they drifted.
///
/// The drift was not theoretical. Chord symbols are emitted as
/// `MuseJazz Text`, with a space, matching the font's internal name. Two
/// of the three declared that family correctly; the third declared
/// `MuseJazzText` and so every chart it exported fell back to a system
/// sans, which is why `maj7` triangles and slash-chord glyphs came out
/// blank. Nobody could see the disagreement because there was no place
/// the three answers sat side by side.
///
/// This is that place. The font list comes from
/// [`ChartFontBundle::embeddable_fonts`], and every export below goes
/// through it.
///
/// # Which export
///
/// The variants differ in one respect — whether the font bytes travel
/// with the document — and picking wrong is a silent failure either way:
///
/// - [`Self::export_svg_pages`] **embeds** the fonts. Use it for a file
///   that leaves the page (a download, an attachment); it cannot rely on
///   a stylesheet that stays behind. Costs ~485 KB per page.
/// - [`Self::export_svg_pages_linked`] and [`Self::export_svg_snippet`]
///   **reference** the fonts by name. Use them on a page that also emits
///   [`Self::font_face_css`] once — several charts then share one copy
///   instead of re-embedding megabytes per chart, which matters when a
///   live preview re-renders on every keystroke.
pub mod pipeline {
    use super::{ChartLayoutError, style};
    use crate::engraver::export::{SvgExportConfig, SvgSerializer};
    use crate::engraver::fonts::ChartFontBundle;
    use crate::engraver::layout::chart::{
        ChartLayoutConfig, ChartLayoutEngine, ChartLayoutResult, LayoutMode,
    };
    use keyflow_proto::Chart;

    /// A font bundle, a layout engine built from it, and every export.
    pub struct ChartPipeline {
        fonts: ChartFontBundle,
        engine: ChartLayoutEngine,
    }

    impl ChartPipeline {
        /// Build a pipeline with the default fonts and the lead-sheet style.
        ///
        /// # Errors
        ///
        /// Returns [`ChartLayoutError::Fonts`] if the font bundle cannot
        /// be built.
        pub fn new() -> Result<Self, ChartLayoutError> {
            Self::with_style(style::leak_lead_sheet_style())
        }

        /// Build a pipeline with an explicit style.
        ///
        /// # Errors
        ///
        /// Returns [`ChartLayoutError::Fonts`] if the font bundle cannot
        /// be built.
        pub fn with_style(
            style: &'static crate::engraver::style::MStyle,
        ) -> Result<Self, ChartLayoutError> {
            let fonts = ChartFontBundle::new().map_err(ChartLayoutError::Fonts)?;
            let engine = fonts.create_layout_engine(style);
            Ok(Self { fonts, engine })
        }

        /// The font bundle, for callers that need the raw bytes.
        #[must_use]
        pub const fn fonts(&self) -> &ChartFontBundle {
            &self.fonts
        }

        /// The layout engine.
        #[must_use]
        pub const fn engine(&self) -> &ChartLayoutEngine {
            &self.engine
        }

        /// Lay a chart out.
        #[must_use]
        pub fn layout(&self, chart: &Chart, mode: &LayoutMode) -> ChartLayoutResult {
            self.engine.layout_chart(chart, mode)
        }

        /// Lay a chart out with an explicit layout config.
        #[must_use]
        pub fn layout_with_config(
            &self,
            chart: &Chart,
            mode: &LayoutMode,
            config: &ChartLayoutConfig,
        ) -> ChartLayoutResult {
            self.engine.layout_chart_with_config(chart, mode, config)
        }

        /// Attach every embeddable font to an export config.
        ///
        /// The list lives on the font bundle so it cannot drift from the
        /// names the layout engine actually emits.
        #[must_use]
        pub fn embed_fonts(&self, config: SvgExportConfig) -> SvgExportConfig {
            self.fonts
                .embeddable_fonts()
                .into_iter()
                .fold(config, |c, (family, bytes)| {
                    c.with_embedded_font(family, bytes.as_ref().clone())
                })
        }

        /// `@font-face` rules for every chart font, as data URIs.
        ///
        /// Emit once per document, alongside the *linked* exports.
        #[must_use]
        pub fn font_face_css(&self) -> String {
            self.embed_fonts(SvgExportConfig::default()).font_face_css()
        }

        /// One self-contained SVG per page, fonts embedded.
        #[must_use]
        pub fn export_svg_pages(&self, result: &ChartLayoutResult) -> Vec<String> {
            self.pages(result, true)
                .into_iter()
                .map(|(_, _, s)| s)
                .collect()
        }

        /// One SVG per page, fonts referenced rather than embedded.
        #[must_use]
        pub fn export_svg_pages_linked(&self, result: &ChartLayoutResult) -> Vec<String> {
            self.pages(result, false)
                .into_iter()
                .map(|(_, _, s)| s)
                .collect()
        }

        /// One SVG per page with its true paper size, fonts referenced.
        ///
        /// Returns `(width_pt, height_pt, svg)`, so a preview can lay
        /// pages out at real A4/Letter dimensions rather than stretching
        /// them to a container.
        #[must_use]
        pub fn export_svg_pages_sized(
            &self,
            result: &ChartLayoutResult,
        ) -> Vec<(f64, f64, String)> {
            self.pages(result, false)
        }

        /// One SVG cropped to the chart's own bounds, fonts referenced.
        ///
        /// The engraver's `total_width`/`total_height` describe a print
        /// page — content plus A4 margins, inter-system spacing and
        /// below-staff reserve — so a one-system chart is ~20pt of music
        /// in a ~180pt box. Inline, that leaves the music marooned in
        /// mostly-empty space.
        ///
        /// Cropped with [`ChartLayoutResult::content_bounds`], not the
        /// scene's path bounds: chord symbols, lyrics, section labels and
        /// noteheads are text and glyphs that sit OUTSIDE the staff and
        /// barline paths, so a path-only box wraps the staff and clips
        /// the chord numbers above it.
        #[must_use]
        pub fn export_svg_snippet(&self, result: &ChartLayoutResult, padding: f64) -> String {
            let (x, y, w, h) = match result.content_bounds() {
                Some(b) => (
                    b.x0 - padding,
                    b.y0 - padding,
                    b.width() + 2.0 * padding,
                    b.height() + 2.0 * padding,
                ),
                None => (0.0, 0.0, result.total_width, result.total_height),
            };
            let config = SvgExportConfig::for_page(x, y, w, h);
            SvgSerializer::new(config).serialize(&result.scene)
        }

        /// The same crop, with the fonts embedded — for a snippet that
        /// leaves the page.
        #[must_use]
        pub fn export_svg_snippet_embedded(
            &self,
            result: &ChartLayoutResult,
            padding: f64,
        ) -> String {
            let (x, y, w, h) = match result.content_bounds() {
                Some(b) => (
                    b.x0 - padding,
                    b.y0 - padding,
                    b.width() + 2.0 * padding,
                    b.height() + 2.0 * padding,
                ),
                None => (0.0, 0.0, result.total_width, result.total_height),
            };
            let config = self.embed_fonts(SvgExportConfig::for_page(x, y, w, h));
            SvgSerializer::new(config).serialize(&result.scene)
        }

        /// Every page as `(width_pt, height_pt, svg)`.
        fn pages(&self, result: &ChartLayoutResult, embed: bool) -> Vec<(f64, f64, String)> {
            result
                .pages
                .iter()
                .map(|page| {
                    let config = SvgExportConfig::for_page(
                        page.x_offset,
                        page.y_offset,
                        page.width,
                        page.height,
                    );
                    let config = if embed {
                        self.embed_fonts(config)
                    } else {
                        config
                    };
                    (
                        page.width,
                        page.height,
                        SvgSerializer::new(config).serialize(&result.scene),
                    )
                })
                .collect()
        }

        /// The whole chart as one multi-page vector PDF.
        ///
        /// # Errors
        ///
        /// Returns the serialiser's error if the PDF cannot be written.
        #[cfg(feature = "pdf")]
        pub fn export_pdf(&self, result: &ChartLayoutResult) -> Result<Vec<u8>, String> {
            use crate::engraver::export::PdfSerializer;

            // The SVGs handed to the PDF writer embed their fonts, and
            // the writer is given the same list again by name: usvg
            // resolves the families out of that table, so a family the
            // table omits silently becomes a system face.
            let svg_pages = self.export_svg_pages(result);
            // The raster list: usvg has no default for the generic
            // `sans-serif` the scene emits, so it must be given a face.
            let fonts = self.fonts.embeddable_fonts_for_raster();
            let refs: Vec<(&str, &[u8])> = fonts.iter().map(|(n, b)| (*n, b.as_slice())).collect();

            PdfSerializer::serialize_from_svg(&svg_pages, &refs)
                .map_err(|e| format!("Failed to export PDF: {e}"))
        }
    }
}
