//! Score data model for music notation.
//!
//! This module defines the core data structures for representing musical scores,
//! from individual notes to complete multi-part compositions.

// region:    --- Modules

mod duration;
mod element;
mod header;
mod layout;
mod measure;
mod measure_layout;
mod note;
mod page_style;
mod part;
mod pitch;
mod score;

// endregion: --- Modules

// region:    --- Re-exports

pub use duration::{Duration, DurationKind};
pub use element::{ChordSymbol, Clef, ElementId, KeySignature, MusicElement, Rest, TimeSignature};
pub use header::{
    ComputedHeaderLayout, HeaderFrameConfig, HeaderStyles, HeaderTextAlign, HeaderTextStyle,
    ScoreHeader,
};
pub use layout::{
    compute_all_system_y_positions, compute_page_layout, compute_page_layout_mut,
    compute_system_layout, spread_systems_on_page, LayoutBreak, LineBreakPolicy, PageContentBounds,
    PageInfo, PageLayout, PageLayoutConfig, RehearsalMark, RehearsalMarkStyle, SystemInfo,
    SystemLayout, SystemYPosition,
};
pub use measure::Measure;
pub use measure_layout::{
    calculate_beat_positions, compute_measure_layouts, justify_measure_layouts, BeatPosition,
    MeasureInfo, MeasureLayout, MeasureLayoutConfig,
};
pub use note::{Accidental, Note, NoteHead, Stem};
pub use page_style::{LineBreakConfig, Margins, PageStyle, PaperSize, StaffConfig, SystemSpacing};
pub use part::{Part, PartId};
pub use pitch::{Octave, Pitch, PitchClass};
pub use score::{LayoutSettings, Score, ScoreMetadata};

// endregion: --- Re-exports

// region:    --- Voice

use serde::{Deserialize, Serialize};

/// Voice within a measure - a single melodic/rhythmic line.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Voice {
    /// Elements in this voice (notes, rests, chords, etc.)
    pub elements: Vec<MusicElement>,
}

impl Voice {
    /// Create a new empty voice.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an element to this voice.
    pub fn add(&mut self, element: MusicElement) {
        self.elements.push(element);
    }
}

// endregion: --- Voice
