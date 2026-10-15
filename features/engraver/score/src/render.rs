//! SVG export for part layouts — same font embedding as the kf CLI so the
//! SMuFL glyphs (Leland) and text faces resolve identically everywhere.

use engraver_proto::engraver::export::svg::{SvgExportConfig, SvgSerializer};
use engraver_proto::engraver::fonts::ChartFontBundle;

use crate::layout::PartLayout;

/// Render each page of a [`PartLayout`] to a standalone SVG string.
pub fn svg_pages(layout: &PartLayout, fonts: &ChartFontBundle) -> Vec<String> {
    let leland = fonts.symbol_font_data().as_ref().clone();
    let bravura = fonts.bravura_font_data().as_ref().clone();
    let leland_text = fonts.leland_text_font_data().as_ref().clone();
    let chicago = fonts.chicago_font_data().as_ref().clone();
    let freesans = fonts.freesans_font_data().as_ref().clone();

    layout
        .pages
        .iter()
        .map(|page| {
            let config =
                SvgExportConfig::for_page(page.x_offset, page.y_offset, page.width, page.height)
                    .with_embedded_font("Leland", leland.clone())
                    .with_embedded_font("Bravura", bravura.clone())
                    .with_embedded_font("Leland Text", leland_text.clone())
                    .with_embedded_font("LelandText", leland_text.clone())
                    .with_embedded_font("Chicago", chicago.clone())
                    .with_embedded_font("ChicagoFLF", chicago.clone())
                    .with_embedded_font("FreeSans", freesans.clone())
                    .with_embedded_font(
                        crate::fonts::SERIF_FAMILY,
                        crate::fonts::serif_regular().to_vec(),
                    )
                    .with_embedded_font("sans-serif", freesans.clone());
            let mut serializer = SvgSerializer::new(config);
            serializer.serialize(&layout.scene)
        })
        .collect()
}

/// Render a [`PartLayout`] to a multi-page PDF. Every serif face is loaded,
/// so bold and italic text resolves to the real FreeSerif Bold/Italic
/// faces rather than synthesized ones.
#[cfg(feature = "pdf")]
pub fn pdf(layout: &PartLayout, fonts: &ChartFontBundle) -> Result<Vec<u8>, String> {
    use engraver_proto::engraver::export::pdf::PdfSerializer;
    let pages = svg_pages(layout, fonts);
    let leland = fonts.symbol_font_data();
    let bravura = fonts.bravura_font_data();
    let leland_text = fonts.leland_text_font_data();
    let freesans = fonts.freesans_font_data();
    let chicago = fonts.chicago_font_data();
    let mut faces: Vec<(&str, &[u8])> = vec![
        ("Leland", leland.as_slice()),
        ("Bravura", bravura.as_slice()),
        ("Leland Text", leland_text.as_slice()),
        ("FreeSans", freesans.as_slice()),
        ("ChicagoFLF", chicago.as_slice()),
    ];
    faces.extend(crate::fonts::serif_faces());
    PdfSerializer::serialize_from_svg(&pages, &faces).map_err(|e| e.to_string())
}
