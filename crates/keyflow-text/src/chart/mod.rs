//! Chart module with text parsing support.

pub use keyflow_proto::chart::*;

pub struct ChartParser<'a> {
    chart: &'a mut Chart,
}

impl<'a> ChartParser<'a> {
    pub fn new(chart: &'a mut Chart) -> Self {
        Self { chart }
    }
}

impl<'a> std::ops::Deref for ChartParser<'a> {
    type Target = Chart;

    fn deref(&self) -> &Self::Target {
        self.chart
    }
}

impl<'a> std::ops::DerefMut for ChartParser<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.chart
    }
}

pub fn parse_chart(input: &str) -> Result<Chart, String> {
    let mut chart = Chart::new();
    {
        let mut parser = ChartParser::new(&mut chart);
        parser.parse(input)?;
    }
    Ok(chart)
}

/// Parse a `.kf` document (potentially multi-block) and return the resulting
/// `Chart` plus the raw `KfDocument`.
///
/// Block layout:
/// - **`--- keyflow ---`** holds rhythm + section structure (the timing source
///   of truth). Parsed via [`parse_chart`].
/// - **`--- chordpro ---`** (optional, can appear multiple times) holds
///   ChordPro 6.07 lyrics + chord-over-lyric placement. Parsed via
///   [`keyflow_chordpro::parse`] and merged into the chart by
///   [`parser::merge_chordpro_into_chart`]: top-level metadata flows
///   into `Chart::metadata` (without overwriting fields the keyflow block
///   already supplied), and lyric blocks under environments like
///   `{start_of_verse}` / `{soc}` attach as `Track::lyrics` on matching
///   keyflow sections by `SectionType` in source order.
///
/// If the document has no delimiters, the entire content is parsed as a
/// single keyflow block.
pub fn parse_document(input: &str) -> Result<(Chart, keyflow_proto::document::KfDocument), String> {
    let doc = parser::parse_kf_document(input)?;

    // Find the keyflow block content to parse.
    let keyflow_content = if doc.is_plain_keyflow() {
        doc.blocks[0].content.clone()
    } else {
        doc.find_block("keyflow")
            .map(|b| b.content.clone())
            .unwrap_or_default()
    };

    let mut chart = parse_chart(&keyflow_content)?;

    // Merge every `--- chordpro ---` block into the chart. Multiple
    // chordpro blocks are supported as parallel lyric layers (e.g.
    // translations, singers, or parts) over the same keyflow rhythm chart.
    for block in &doc.blocks {
        if matches!(block.kind, keyflow_proto::document::KfBlockKind::ChordPro) {
            // Best-effort: parse failures are surfaced but don't abort
            // the keyflow parse. Live-editor consumers see a chart with
            // diagnostics instead of an empty document.
            if let Ok(kc) = keyflow_chordpro::parse(&block.content) {
                let _ = parser::merge_chordpro_into_chart(&mut chart, &kc);
            }
        }
    }

    Ok((chart, doc))
}

pub mod parser;
