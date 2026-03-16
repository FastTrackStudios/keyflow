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

/// Parse a .kf document (potentially multi-block) and return the Chart from the keyflow block.
///
/// If the document has `--- keyflow ---` / `--- chordpro ---` delimiters, only the
/// keyflow block is parsed into a Chart. If no delimiters, the entire content is parsed.
pub fn parse_document(
    input: &str,
) -> Result<(Chart, keyflow_proto::document::KfDocument), String> {
    let doc = parser::parse_kf_document(input)?;

    // Find the keyflow block content to parse
    let keyflow_content = if doc.is_plain_keyflow() {
        doc.blocks[0].content.clone()
    } else {
        doc.find_block("keyflow")
            .map(|b| b.content.clone())
            .unwrap_or_default()
    };

    let chart = parse_chart(&keyflow_content)?;
    Ok((chart, doc))
}

pub mod parser;
