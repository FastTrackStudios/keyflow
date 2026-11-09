//! The lyric lane prints in the face the theme asks for — FreeSans italic
//! by default — in the PDF, not only on screen.
//!
//! A PDF that cannot find a face does not fail: usvg substitutes the
//! upright one and the chart prints, wrong. That happened: the PNG showed
//! the lyrics in italic while every PDF printed them upright, because the
//! PDF's font table had no oblique face. So this checks the fonts the PDF
//! actually embeds, which is what a reader (or `pdffonts`) sees.
#![cfg(feature = "pdf")]

use engraver::api::pipeline::ChartPipeline;
use engraver::engraver::layout::chart::LayoutMode;

const WITH_LYRICS: &str = "\
Lanterns - Test
#D 120bpm 4/4

VS 4
D G A D
[lyrics]
@1 Pa- @1:2 per @1:3 lan- @2 terns @3 drift- @3:3 ing @4 _
";

/// Base font names the PDF embeds (`/BaseFont /ABCDEF+FreeSansOblique`).
fn embedded_fonts(pdf: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(pdf);
    let mut fonts: Vec<String> = text
        .split("/BaseFont")
        .skip(1)
        .filter_map(|rest| {
            let name = rest.trim_start().strip_prefix('/')?;
            let end = name
                .find(|c: char| c.is_whitespace() || matches!(c, '/' | '>' | '['))
                .unwrap_or(name.len());
            let name = &name[..end];
            // Drop the subset tag.
            Some(name.split_once('+').map_or(name, |(_, n)| n).to_string())
        })
        .collect();
    fonts.sort();
    fonts.dedup();
    fonts
}

#[test]
fn the_lyric_lane_embeds_the_oblique_face_in_the_pdf() {
    let chart = keyflow_lang::text::chart::parse_chart(WITH_LYRICS).expect("the fixture parses");
    let pipeline = ChartPipeline::shared().expect("the pipeline builds");
    let result = pipeline.layout(&chart, &LayoutMode::paginated_letter());
    let pdf = pipeline.export_pdf(&result).expect("the PDF exports");
    let fonts = embedded_fonts(&pdf);
    assert!(
        fonts.iter().any(|f| f.contains("FreeSansOblique")),
        "lyrics need FreeSans Oblique in the PDF; it embeds {fonts:?}"
    );
}
