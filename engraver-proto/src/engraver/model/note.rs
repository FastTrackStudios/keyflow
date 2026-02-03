//! Note representation for music notation.

use super::{Duration, Pitch};
use serde::{Deserialize, Serialize};

/// Accidental display type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Accidental {
    /// No accidental displayed (uses key signature)
    #[default]
    None,
    /// Natural sign
    Natural,
    /// Sharp sign
    Sharp,
    /// Flat sign
    Flat,
    /// Double sharp sign
    DoubleSharp,
    /// Double flat sign
    DoubleFlat,
}

impl Accidental {
    /// Get the SMuFL codepoint for this accidental.
    #[must_use]
    pub const fn smufl_codepoint(self) -> Option<char> {
        match self {
            Self::None => None,
            Self::Natural => Some('\u{E261}'), // accidentalNatural
            Self::Sharp => Some('\u{E262}'),   // accidentalSharp
            Self::Flat => Some('\u{E260}'),    // accidentalFlat
            Self::DoubleSharp => Some('\u{E263}'), // accidentalDoubleSharp
            Self::DoubleFlat => Some('\u{E264}'), // accidentalDoubleFlat
        }
    }
}

/// Notehead type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NoteHead {
    /// Standard filled notehead (for quarter and shorter)
    #[default]
    Black,
    /// Open notehead (for half notes)
    Half,
    /// Open notehead (for whole notes)
    Whole,
    /// X-shaped notehead (for percussion/ghost notes)
    Cross,
    /// Diamond notehead (for harmonics)
    Diamond,
    /// Slash notehead for quarter/eighth notes (rhythmic notation)
    Slash,
    /// Slash notehead for half notes (rhythmic notation)
    SlashHalf,
    /// Slash notehead for whole notes (rhythmic notation)
    SlashWhole,
    /// Slash notehead for double whole notes (rhythmic notation)
    SlashDoubleWhole,
}

impl NoteHead {
    /// Get the SMuFL codepoint for this notehead.
    #[must_use]
    pub const fn smufl_codepoint(self) -> char {
        match self {
            Self::Black => '\u{E0A4}',            // noteheadBlack
            Self::Half => '\u{E0A3}',             // noteheadHalf
            Self::Whole => '\u{E0A2}',            // noteheadWhole
            Self::Cross => '\u{E0A9}',            // noteheadXBlack
            Self::Diamond => '\u{E0DB}',          // noteheadDiamondBlack
            Self::Slash => '\u{E101}',            // noteheadSlashHorizontalEnds
            Self::SlashHalf => '\u{E103}',        // noteheadSlashWhiteHalf
            Self::SlashWhole => '\u{E102}',       // noteheadSlashWhiteWhole
            Self::SlashDoubleWhole => '\u{E10A}', // noteheadSlashWhiteDoubleWhole
        }
    }

    /// Get the appropriate slash notehead for a given duration.
    #[must_use]
    pub const fn slash_for_duration(kind: super::DurationKind) -> Self {
        match kind {
            super::DurationKind::Whole => Self::SlashWhole,
            super::DurationKind::Half => Self::SlashHalf,
            // Quarter, eighth, sixteenth, etc. all use filled slash
            _ => Self::Slash,
        }
    }

    /// Returns true if this is any kind of slash notehead.
    #[must_use]
    pub const fn is_slash(self) -> bool {
        matches!(
            self,
            Self::Slash | Self::SlashHalf | Self::SlashWhole | Self::SlashDoubleWhole
        )
    }
}

/// Stem direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Stem {
    /// Stem pointing up
    #[default]
    Up,
    /// Stem pointing down
    Down,
    /// No stem (whole notes, etc.)
    None,
}

/// A single note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// The pitch of the note
    pub pitch: Pitch,
    /// The duration of the note
    pub duration: Duration,
    /// Accidental to display (if any)
    pub accidental: Accidental,
    /// Notehead type
    pub head: NoteHead,
    /// Stem direction (can be auto-calculated)
    pub stem: Stem,
    /// Whether the note is tied to the next note
    pub tie_forward: bool,
    /// Whether the note is tied from the previous note
    pub tie_back: bool,
}

impl Note {
    /// Create a new note with the given pitch and duration.
    #[must_use]
    pub fn new(pitch: Pitch, duration: Duration) -> Self {
        let head = match duration.kind {
            super::DurationKind::Whole => NoteHead::Whole,
            super::DurationKind::Half => NoteHead::Half,
            _ => NoteHead::Black,
        };

        let stem = match duration.kind {
            super::DurationKind::Whole => Stem::None,
            _ => Stem::Up, // Will be auto-calculated based on position
        };

        Self {
            pitch,
            duration,
            accidental: Accidental::None,
            head,
            stem,
            tie_forward: false,
            tie_back: false,
        }
    }

    /// Set the accidental.
    #[must_use]
    pub fn with_accidental(mut self, accidental: Accidental) -> Self {
        self.accidental = accidental;
        self
    }

    /// Auto-calculate stem direction based on pitch and staff.
    pub fn auto_stem(&mut self) {
        // Notes on or above the middle line get stems down
        if self.pitch.staff_position() >= 0 {
            self.stem = Stem::Down;
        } else {
            self.stem = Stem::Up;
        }
    }
}
