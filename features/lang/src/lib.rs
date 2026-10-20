//! The Keyflow language: the chart parser and model, and the formats it
//! reads and writes.
//!
//! The text parser and ChordPro are always here; MIDI, live performance,
//! MusicXML, Finale (musx), orchestral parts, chordsheet.com import and
//! synced lyrics are each behind a feature of the same name.

pub mod chordpro;
pub mod text;

#[cfg(feature = "chordsheet")]
pub mod chordsheet;
#[cfg(feature = "live")]
pub mod live;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(feature = "musicxml")]
pub mod musicxml;
#[cfg(feature = "musx")]
pub mod musx;
#[cfg(feature = "orchestra")]
pub mod orchestra;
#[cfg(feature = "sync")]
pub mod sync;
