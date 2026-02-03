//! Engraver protocol - shared types for Keyflow engraving.

// region:    --- Keyflow Re-exports (compat)

pub mod ast {
    pub use keyflow_proto::ast::*;
}
pub mod chart {
    pub use keyflow_proto::chart::*;
}
pub mod chord {
    pub use keyflow_proto::chord::*;
}
pub mod core {
    pub use keyflow_proto::core::*;
}
pub mod key {
    pub use keyflow_proto::key::*;
}
pub mod metadata {
    pub use keyflow_proto::metadata::*;
}
pub mod parsing {
    pub use keyflow_proto::parsing::*;
}
pub mod primitives {
    pub use keyflow_proto::primitives::*;
}
pub mod sections {
    pub use keyflow_proto::sections::*;
}
pub mod time {
    pub use keyflow_proto::time::*;
}

pub use keyflow_proto::*;

// endregion: --- Keyflow Re-exports (compat)

// region:    --- Legacy DurationTrait (compat)

use std::fmt;

/// Trait for musical durations (legacy - use MusicalDuration directly)
pub trait DurationTrait: fmt::Debug + fmt::Display {
    fn measures(&self) -> u32;
    fn beats(&self) -> u32;
    fn subdivisions(&self) -> u32;
    fn to_beats(&self, time_sig: time::TimeSignature) -> f64;
    fn add(&self, other: &dyn DurationTrait, time_sig: time::TimeSignature) -> Box<dyn DurationTrait>;
    fn clone_box(&self) -> Box<dyn DurationTrait>;
}

impl DurationTrait for time::MusicalDuration {
    fn measures(&self) -> u32 {
        self.measure.max(0) as u32
    }

    fn beats(&self) -> u32 {
        self.beat.max(0) as u32
    }

    fn subdivisions(&self) -> u32 {
        self.subdivision.clamp(0, 999) as u32
    }

    fn to_beats(&self, time_sig: time::TimeSignature) -> f64 {
        let beats_per_measure = time_sig.numerator() as f64;
        self.measure.max(0) as f64 * beats_per_measure
            + self.beat.max(0) as f64
            + self.subdivision.clamp(0, 999) as f64 / 1000.0
    }

    fn add(&self, other: &dyn DurationTrait, time_sig: time::TimeSignature) -> Box<dyn DurationTrait> {
        let total_beats = self.to_beats(time_sig) + other.to_beats(time_sig);
        let beats_per_measure = time_sig.numerator() as f64;
        let measures = (total_beats / beats_per_measure).floor() as i32;
        let remaining_beats = total_beats - (measures as f64 * beats_per_measure);
        let beats = remaining_beats.floor() as i32;
        let subdivisions = ((remaining_beats - beats as f64) * 1000.0).round() as i32;
        Box::new(time::MusicalDuration::new(measures, beats, subdivisions.clamp(0, 999)))
    }

    fn clone_box(&self) -> Box<dyn DurationTrait> {
        Box::new(self.clone())
    }
}

// endregion: --- Legacy DurationTrait (compat)

// region:    --- Modules

#[cfg(feature = "engraver")]
pub mod engraver;

// endregion: --- Modules

// region:    --- Re-exports

#[cfg(feature = "engraver")]
pub use engraver::error::{Error, Result};
#[cfg(feature = "engraver")]
pub use engraver::model::{Measure, MusicElement, Part, Score, Voice};
#[cfg(feature = "engraver")]
pub use engraver::style::{MStyle, Sid, StyleValue};
#[cfg(feature = "engraver")]
pub use engraver::{
    export, fonts, import, interaction, layout, model, notation, quantize, renderer, scene, style,
};

// endregion: --- Re-exports
