//! Measure (bar) representation.

use super::element::{KeySignature, TimeSignature};
use super::layout::{LayoutBreak, RehearsalMark};
use super::Voice;
use serde::{Deserialize, Serialize};

/// A measure (bar) of music.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measure {
    /// Measure number (1-indexed)
    pub number: u32,
    /// Time signature (if changed at this measure)
    pub time_signature: Option<TimeSignature>,
    /// Key signature (if changed at this measure)
    pub key_signature: Option<KeySignature>,
    /// Voices in this measure (typically 1-4)
    pub voices: Vec<Voice>,
    /// Rehearsal mark at the start of this measure (section marker)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rehearsal_mark: Option<RehearsalMark>,
    /// Layout break after this measure (line/page/section break)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_break: Option<LayoutBreak>,
}

impl Measure {
    /// Create a new empty measure with the given number.
    #[must_use]
    pub fn new(number: u32) -> Self {
        Self {
            number,
            time_signature: None,
            key_signature: None,
            voices: vec![Voice::new()],
            rehearsal_mark: None,
            layout_break: None,
        }
    }

    /// Create a measure with time and key signature changes.
    #[must_use]
    pub fn with_signatures(
        number: u32,
        time_signature: Option<TimeSignature>,
        key_signature: Option<KeySignature>,
    ) -> Self {
        Self {
            number,
            time_signature,
            key_signature,
            voices: vec![Voice::new()],
            rehearsal_mark: None,
            layout_break: None,
        }
    }

    /// Set a rehearsal mark at the start of this measure.
    #[must_use]
    pub fn with_rehearsal_mark(mut self, mark: RehearsalMark) -> Self {
        self.rehearsal_mark = Some(mark);
        self
    }

    /// Set a layout break after this measure.
    #[must_use]
    pub fn with_layout_break(mut self, break_type: LayoutBreak) -> Self {
        self.layout_break = Some(break_type);
        self
    }

    /// Check if this measure has a rehearsal mark.
    #[must_use]
    pub fn has_rehearsal_mark(&self) -> bool {
        self.rehearsal_mark.is_some()
    }

    /// Check if this measure has an explicit line break.
    #[must_use]
    pub fn has_line_break(&self) -> bool {
        matches!(
            self.layout_break,
            Some(LayoutBreak::Line) | Some(LayoutBreak::Page) | Some(LayoutBreak::Section)
        )
    }

    /// Check if breaks are prevented at this measure.
    #[must_use]
    pub fn is_no_break(&self) -> bool {
        matches!(self.layout_break, Some(LayoutBreak::NoBreak))
    }

    /// Get the primary voice (voice 0).
    #[must_use]
    pub fn primary_voice(&self) -> Option<&Voice> {
        self.voices.first()
    }

    /// Get a mutable reference to the primary voice.
    pub fn primary_voice_mut(&mut self) -> &mut Voice {
        if self.voices.is_empty() {
            self.voices.push(Voice::new());
        }
        &mut self.voices[0]
    }

    /// Add a voice to this measure.
    pub fn add_voice(&mut self, voice: Voice) {
        self.voices.push(voice);
    }

    /// Check if this measure is empty (no elements in any voice).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.voices.iter().all(|v| v.elements.is_empty())
    }
}

impl Default for Measure {
    fn default() -> Self {
        Self::new(1)
    }
}
