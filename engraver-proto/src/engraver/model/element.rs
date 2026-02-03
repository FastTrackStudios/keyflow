//! Music elements that can appear in a score.

use super::{Duration, Note};
use serde::{Deserialize, Serialize};

/// Unique identifier for a music element (for layout references).
///
/// Used by the layout system to track elements during positioning
/// and collision detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(pub usize);

impl ElementId {
    /// Create a new element ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }
}

/// Rest (silence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rest {
    /// Duration of the rest
    pub duration: Duration,
}

/// A chord symbol (harmony) attached to a beat position.
///
/// Used for lead sheets and chord charts where chord symbols appear
/// above the staff independent of the notes being played.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordSymbol {
    /// Full chord symbol text (e.g., "Cmaj7", "Dm7b5", "G7/B")
    pub symbol: String,
    /// Root note name (e.g., "C", "D", "G")
    pub root: String,
    /// Root accidental (empty, "#", "b")
    pub root_accidental: String,
    /// Quality (empty for major, "m" for minor, "dim", "aug", etc.)
    pub quality: String,
    /// Extension ("7", "maj7", "9", "11", "13", etc.)
    pub extension: String,
    /// Alterations (e.g., ["b5", "#9"])
    pub alterations: Vec<String>,
    /// Bass note for slash chords (e.g., "B" in "G7/B")
    pub bass: Option<String>,
    /// Bass accidental
    pub bass_accidental: String,
}

impl ChordSymbol {
    /// Create a chord symbol from its full text representation.
    #[must_use]
    pub fn new(symbol: impl Into<String>) -> Self {
        let symbol = symbol.into();
        Self {
            symbol: symbol.clone(),
            root: String::new(),
            root_accidental: String::new(),
            quality: String::new(),
            extension: String::new(),
            alterations: Vec::new(),
            bass: None,
            bass_accidental: String::new(),
        }
    }

    /// Create a chord symbol with parsed components.
    #[must_use]
    pub fn parsed(
        symbol: impl Into<String>,
        root: impl Into<String>,
        root_accidental: impl Into<String>,
        quality: impl Into<String>,
        extension: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            root: root.into(),
            root_accidental: root_accidental.into(),
            quality: quality.into(),
            extension: extension.into(),
            alterations: Vec::new(),
            bass: None,
            bass_accidental: String::new(),
        }
    }

    /// Add a bass note (for slash chords like "C/G").
    #[must_use]
    pub fn with_bass(mut self, bass: impl Into<String>) -> Self {
        self.bass = Some(bass.into());
        self
    }

    /// Add alterations (e.g., "b5", "#9").
    #[must_use]
    pub fn with_alterations(mut self, alts: Vec<String>) -> Self {
        self.alterations = alts;
        self
    }
}

impl Rest {
    /// Create a new rest with the given duration.
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self { duration }
    }

    /// Get the SMuFL codepoint for this rest.
    #[must_use]
    pub fn smufl_codepoint(&self) -> char {
        match self.duration.kind {
            super::DurationKind::Whole => '\u{E4E3}',     // restWhole
            super::DurationKind::Half => '\u{E4E4}',      // restHalf
            super::DurationKind::Quarter => '\u{E4E5}',   // restQuarter
            super::DurationKind::Eighth => '\u{E4E6}',    // rest8th
            super::DurationKind::Sixteenth => '\u{E4E7}', // rest16th
            super::DurationKind::ThirtySecond => '\u{E4E8}', // rest32nd
            super::DurationKind::SixtyFourth => '\u{E4E9}', // rest64th
        }
    }
}

/// A chord (multiple notes sounding together).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteChord {
    /// Notes in the chord (sorted by pitch, lowest first)
    pub notes: Vec<Note>,
}

impl NoteChord {
    /// Create a new chord from notes.
    #[must_use]
    pub fn new(mut notes: Vec<Note>) -> Self {
        // Sort by pitch (lowest first)
        notes.sort_by_key(|n| n.pitch.midi_note());
        Self { notes }
    }

    /// Get the duration of the chord (uses first note's duration).
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.notes.first().map_or(Duration::QUARTER, |n| n.duration)
    }
}

/// Dynamic marking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dynamic {
    /// Pianississimo (very very soft)
    Ppp,
    /// Pianissimo (very soft)
    Pp,
    /// Piano (soft)
    P,
    /// Mezzo-piano (moderately soft)
    Mp,
    /// Mezzo-forte (moderately loud)
    Mf,
    /// Forte (loud)
    F,
    /// Fortissimo (very loud)
    Ff,
    /// Fortississimo (very very loud)
    Fff,
    /// Sforzando
    Sfz,
    /// Forte-piano
    Fp,
}

impl Dynamic {
    /// Get the SMuFL codepoint for this dynamic.
    #[must_use]
    pub const fn smufl_codepoint(self) -> char {
        match self {
            Self::Ppp => '\u{E52A}', // dynamicPPP
            Self::Pp => '\u{E52B}',  // dynamicPP
            Self::P => '\u{E520}',   // dynamicPiano
            Self::Mp => '\u{E52C}',  // dynamicMP
            Self::Mf => '\u{E52D}',  // dynamicMF
            Self::F => '\u{E522}',   // dynamicForte
            Self::Ff => '\u{E52F}',  // dynamicFF
            Self::Fff => '\u{E530}', // dynamicFFF
            Self::Sfz => '\u{E539}', // dynamicSforzando1
            Self::Fp => '\u{E534}',  // dynamicFortePiano
        }
    }
}

/// Articulation marking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Articulation {
    /// Staccato (detached)
    Staccato,
    /// Staccatissimo (very detached)
    Staccatissimo,
    /// Accent
    Accent,
    /// Strong accent (marcato)
    Marcato,
    /// Tenuto (held)
    Tenuto,
    /// Fermata (pause)
    Fermata,
}

impl Articulation {
    /// Get the SMuFL codepoint for this articulation (above staff).
    #[must_use]
    pub const fn smufl_codepoint_above(self) -> char {
        match self {
            Self::Staccato => '\u{E4A2}',      // articStaccatoAbove
            Self::Staccatissimo => '\u{E4A6}', // articStaccatissimoAbove
            Self::Accent => '\u{E4A0}',        // articAccentAbove
            Self::Marcato => '\u{E4AC}',       // articMarcatoAbove
            Self::Tenuto => '\u{E4A4}',        // articTenutoAbove
            Self::Fermata => '\u{E4C0}',       // fermataAbove
        }
    }
}

/// Clef type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Clef {
    /// Treble clef (G clef on line 2)
    #[default]
    Treble,
    /// Bass clef (F clef on line 4)
    Bass,
    /// Alto clef (C clef on line 3)
    Alto,
    /// Tenor clef (C clef on line 4)
    Tenor,
}

impl Clef {
    /// Get the SMuFL codepoint for this clef.
    #[must_use]
    pub const fn smufl_codepoint(self) -> char {
        match self {
            Self::Treble => '\u{E050}', // gClef
            Self::Bass => '\u{E062}',   // fClef
            Self::Alto => '\u{E05C}',   // cClef
            Self::Tenor => '\u{E05C}',  // cClef (same glyph, different position)
        }
    }

    /// Get the staff line this clef sits on (0 = bottom line).
    #[must_use]
    pub const fn staff_line(self) -> i32 {
        match self {
            Self::Treble => 1, // G on line 2 (0-indexed: 1)
            Self::Bass => 3,   // F on line 4 (0-indexed: 3)
            Self::Alto => 2,   // C on line 3 (0-indexed: 2)
            Self::Tenor => 3,  // C on line 4 (0-indexed: 3)
        }
    }
}

/// Key signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct KeySignature {
    /// Number of sharps (positive) or flats (negative), -7 to +7
    pub fifths: i8,
}

impl KeySignature {
    /// C major / A minor (no accidentals)
    pub const C_MAJOR: Self = Self { fifths: 0 };

    /// Create a key signature with the given number of sharps (positive) or flats (negative).
    #[must_use]
    pub const fn new(fifths: i8) -> Self {
        Self { fifths }
    }

    /// Get the number of sharps (0-7).
    #[must_use]
    pub const fn sharps(&self) -> u8 {
        if self.fifths > 0 {
            self.fifths as u8
        } else {
            0
        }
    }

    /// Get the number of flats (0-7).
    #[must_use]
    pub const fn flats(&self) -> u8 {
        if self.fifths < 0 {
            (-self.fifths) as u8
        } else {
            0
        }
    }
}

/// Time signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSignature {
    /// Numerator (beats per measure)
    pub numerator: u8,
    /// Denominator (note value that gets one beat)
    pub denominator: u8,
}

impl TimeSignature {
    /// Common time (4/4)
    pub const COMMON: Self = Self {
        numerator: 4,
        denominator: 4,
    };

    /// Cut time (2/2)
    pub const CUT: Self = Self {
        numerator: 2,
        denominator: 2,
    };

    /// Create a new time signature.
    #[must_use]
    pub const fn new(numerator: u8, denominator: u8) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Get the duration of one measure in quarter notes.
    #[must_use]
    pub fn measure_duration(&self) -> f64 {
        f64::from(self.numerator) * (4.0 / f64::from(self.denominator))
    }
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self::COMMON
    }
}

/// A music element that can appear in a voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MusicElement {
    /// A single note
    Note(Note),
    /// A rest
    Rest(Rest),
    /// A chord (multiple simultaneous notes)
    Chord(NoteChord),
    /// A chord symbol (harmony) above the staff
    ChordSymbol(ChordSymbol),
    /// Clef change
    Clef(Clef),
    /// Key signature change
    KeySignature(KeySignature),
    /// Time signature change
    TimeSignature(TimeSignature),
    /// Dynamic marking
    Dynamic(Dynamic),
    /// Barline (for special barlines like repeat signs)
    Barline(BarlineType),
}

/// Barline type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarlineType {
    /// Standard single barline
    Single,
    /// Double barline
    Double,
    /// Final (end) barline
    Final,
    /// Start repeat
    RepeatStart,
    /// End repeat
    RepeatEnd,
}

impl BarlineType {
    /// Get the SMuFL codepoint for this barline.
    #[must_use]
    pub const fn smufl_codepoint(self) -> char {
        match self {
            Self::Single => '\u{E030}',      // barlineSingle
            Self::Double => '\u{E031}',      // barlineDouble
            Self::Final => '\u{E032}',       // barlineFinal
            Self::RepeatStart => '\u{E040}', // repeatLeft
            Self::RepeatEnd => '\u{E041}',   // repeatRight
        }
    }
}
