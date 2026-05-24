//! Import functionality for various music formats.
//!
//! Currently supports:
//! - keyflow Chart format (always available with `engraver` feature)
//! - MIDI files via `keyflow-midi` (requires `midi-import` feature)
//!
//! Planned:
//! - MusicXML

mod keyflow_import;

#[cfg(feature = "midi-import")]
pub use keyflow_midi::import::{
    ChordMarker, MarkerEvent, MarkerType, MelodyGrid, MidiChartConfig, MidiFile, MidiImportConfig,
    MidiNote, MidiTrack, MusicalPosition, PushPull, PushPullAmount, RhythmElement, SectionMarker,
    SectionType, TempoEvent, TimeSignatureEvent, format_duration_suffix, format_measure_rhythm,
    format_rest, generate_chart_text, generate_measure_rhythm, normalize_chord_name,
};

pub use keyflow_import::import_chart;
