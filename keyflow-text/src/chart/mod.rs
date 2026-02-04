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

pub mod parser;
