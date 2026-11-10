//! Chord-line parser tests that go through `parse_chord_line` or a whole
//! chart, grouped by what they exercise.

use crate::text::chart::parse_chart;
use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::ChartParser as Chart;
use crate::text::chart::types::RhythmElement;
use crate::text::chord::{ChordRhythm, LilySyntax};
use crate::text::key::Key;
use crate::text::sections::SectionType;
use crate::text::time::{MusicalPositionExt, TimeSignature, TimeSignatureExt};
use keyflow_proto::chart::{DynamicLevel, HairpinKind, Placement, RepeatMark};

mod bare;
mod bars;
mod degrees;
mod directives;
mod figures;
mod inline_melody_tie;
mod lanes;
mod push_pull;
mod sections;
mod text;

/// Helper: parse a chord line in isolation and return the parsed measures.
fn parse_line(line: &str) -> Vec<crate::text::chart::types::Measure> {
    let mut chart = crate::text::chart::Chart::new();
    let mut parser = ChartParser::new(&mut chart);
    parser
        .parse_chord_line(line, &SectionType::Verse, Some(1))
        .expect("parse_chord_line failed")
}

fn collect_chord_spans(
    measures: &[crate::text::chart::types::Measure],
) -> Vec<(String, Option<crate::text::parsing::TextSpan>)> {
    let mut out = Vec::new();
    for m in measures {
        for c in &m.chords {
            out.push((c.original_token.clone(), c.source_span));
        }
    }
    out
}

/// Collect rendered chord symbols from a parsed line.
fn chord_symbols(line: &str) -> Vec<String> {
    parse_line(line)
        .iter()
        .flat_map(|m| m.chords.iter().map(|c| c.full_symbol.clone()))
        .collect()
}

/// Parse a full chart (with a key header) and collect each chord's quality.
fn chord_qualities(src: &str) -> Vec<String> {
    parse_chart(src).unwrap().sections[0]
        .measures()
        .iter()
        .flat_map(|m| m.chords.iter().map(|c| format!("{:?}", c.parsed.quality)))
        .collect()
}

/// Like [`parse_line`] but with an explicit section measure count, for
/// content that spans more than one bar.
fn parse_line_n(line: &str, count: usize) -> Vec<crate::text::chart::types::Measure> {
    let mut chart = crate::text::chart::Chart::new();
    let mut parser = ChartParser::new(&mut chart);
    parser
        .parse_chord_line(line, &SectionType::Verse, Some(count))
        .expect("parse_chord_line failed")
}
