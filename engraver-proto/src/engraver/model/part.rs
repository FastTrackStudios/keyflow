//! Part (instrument) representation.

use super::{element::Clef, Measure};
use serde::{Deserialize, Serialize};

/// Unique identifier for a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartId(pub u32);

/// Staff configuration for a part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Staff {
    /// The clef for this staff
    pub clef: Clef,
    /// Number of staff lines (typically 5)
    pub lines: u8,
}

impl Staff {
    /// Standard treble staff.
    pub const TREBLE: Self = Self {
        clef: Clef::Treble,
        lines: 5,
    };

    /// Standard bass staff.
    pub const BASS: Self = Self {
        clef: Clef::Bass,
        lines: 5,
    };

    /// Create a new staff with the given clef.
    #[must_use]
    pub const fn new(clef: Clef) -> Self {
        Self { clef, lines: 5 }
    }
}

impl Default for Staff {
    fn default() -> Self {
        Self::TREBLE
    }
}

/// A part (instrument or voice) in a score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    /// Unique identifier for this part
    pub id: PartId,
    /// Display name (e.g., "Piano", "Violin I")
    pub name: String,
    /// Short name for system labels (e.g., "Pno.", "Vln. I")
    pub abbreviation: Option<String>,
    /// Staff configuration(s) for this part
    pub staves: Vec<Staff>,
    /// Measures in this part
    pub measures: Vec<Measure>,
}

impl Part {
    /// Create a new part with a single staff.
    #[must_use]
    pub fn new(id: PartId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            abbreviation: None,
            staves: vec![Staff::default()],
            measures: Vec::new(),
        }
    }

    /// Create a piano part (grand staff).
    #[must_use]
    pub fn piano(id: PartId) -> Self {
        Self {
            id,
            name: "Piano".into(),
            abbreviation: Some("Pno.".into()),
            staves: vec![Staff::TREBLE, Staff::BASS],
            measures: Vec::new(),
        }
    }

    /// Set the abbreviation.
    #[must_use]
    pub fn with_abbreviation(mut self, abbrev: impl Into<String>) -> Self {
        self.abbreviation = Some(abbrev.into());
        self
    }

    /// Add staves to this part.
    #[must_use]
    pub fn with_staves(mut self, staves: Vec<Staff>) -> Self {
        self.staves = staves;
        self
    }

    /// Add a measure to this part.
    pub fn add_measure(&mut self, measure: Measure) {
        self.measures.push(measure);
    }

    /// Get the number of staves in this part.
    #[must_use]
    pub fn staff_count(&self) -> usize {
        self.staves.len()
    }
}
