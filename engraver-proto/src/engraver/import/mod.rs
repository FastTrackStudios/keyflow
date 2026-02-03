//! Import functionality for various music formats.
//!
//! Currently supports:
//! - keyflow Chart format (always available with `engraver` feature)
//! - MIDI files (requires `midi-import` feature)
//!
//! Planned:
//! - MusicXML

mod keyflow_import;

#[cfg(feature = "midi-import")]
mod midi_import;

#[cfg(feature = "midi-import")]
mod midi_chart_builder;

pub use keyflow_import::import_chart;

#[cfg(feature = "midi-import")]
pub use midi_chart_builder::{generate_chart_text, MidiChartConfig};

#[cfg(feature = "midi-import")]
pub use midi_import::{
    format_duration_suffix, format_measure_rhythm, format_rest, generate_measure_rhythm,
    normalize_chord_name, ChordMarker, MarkerEvent, MarkerType, MidiFile, MidiImportConfig,
    MidiNote, MidiTrack, MusicalPosition, PushPull, PushPullAmount, RhythmElement, SectionMarker,
    SectionType, TempoEvent, TimeSignatureEvent,
};
