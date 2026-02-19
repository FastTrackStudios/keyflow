//! Keyflow MIDI import helpers.
//!
//! This crate re-exports the MIDI import APIs from `engraver-proto` so
//! MIDI tooling can be pulled in without the full keyflow crate.

pub use keyflow_proto as proto;

pub mod import {
    pub use engraver_proto::engraver::import::*;
}

use std::path::Path;

use keyflow_proto::Chart;

pub fn parse_midi_bytes(bytes: &[u8]) -> Result<Chart, String> {
    let midi =
        engraver_proto::engraver::import::MidiFile::parse(bytes).map_err(|e| e.to_string())?;
    let config = engraver_proto::engraver::import::MidiChartConfig::default();
    let text = engraver_proto::engraver::import::generate_chart_text(&midi, &config);
    keyflow_text::api::parse::chart(&text)
}

pub fn parse_midi_path(path: &Path) -> Result<Chart, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    parse_midi_bytes(&bytes)
}
