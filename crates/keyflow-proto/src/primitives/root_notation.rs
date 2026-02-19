//! Root notation - unified representation for note names, scale degrees, and roman numerals
//!
//! This module provides a single RootNotation type that can represent any of the three
//! ways to specify a chord root, while preserving the original format for display.

use super::note::{MusicalNote, MusicalNoteToken, Note};
use super::roman_numeral::RomanNumeralToken;
use facet::Facet;
use std::fmt;

/// Case for Roman numeral notation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Facet)]
#[repr(u8)]
pub enum RomanCase {
    Upper, // I, II, III, IV, V, VI, VII (typically major)
    Lower, // i, ii, iii, iv, v, vi, vii (typically minor)
}

/// The original format used to specify the root
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[repr(u8)]
pub enum RootFormat {
    /// Scale degree (1-7)
    ScaleDegree(u8),
    /// Roman numeral (I-VII or i-vii)
    RomanNumeral { degree: u8, case: RomanCase },
    /// Explicit note name (C, D#, Eb, etc.)
    NoteName(String),
    /// Empty/rest - no root note (used for rhythm-only entries like rests)
    Empty,
}

/// Unified root notation that works for all three input formats
///
/// This struct stores both the resolved note and the original format,
/// making all three formats 100% interchangeable while preserving
/// how the user originally wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
pub struct RootNotation {
    /// The resolved note (may be None if key-relative and not yet resolved)
    resolved_note: Option<MusicalNote>,
    /// The original format as written by the user
    original_format: RootFormat,
}

impl RootNotation {
    /// Create an empty root notation (for rests and rhythm-only entries)
    pub fn empty() -> Self {
        Self {
            resolved_note: None,
            original_format: RootFormat::Empty,
        }
    }

    /// Create from a scale degree with optional accidental
    pub fn from_scale_degree(
        degree: u8,
        _accidental: Option<super::accidental::Accidental>,
    ) -> Self {
        assert!((1..=7).contains(&degree), "Scale degree must be 1-7");
        Self {
            resolved_note: None, // Needs key context to resolve
            original_format: RootFormat::ScaleDegree(degree),
        }
        // TODO: Store accidental information in RootFormat::ScaleDegree if needed
    }

    /// Create from a roman numeral
    pub fn from_roman_numeral(degree: u8, case: RomanCase) -> Self {
        assert!(
            (1..=7).contains(&degree),
            "Roman numeral degree must be 1-7"
        );
        Self {
            resolved_note: None, // Needs key context to resolve
            original_format: RootFormat::RomanNumeral { degree, case },
        }
    }

    /// Create from an explicit note name
    pub fn from_note_name(note: MusicalNote) -> Self {
        Self {
            resolved_note: Some(note.clone()),
            original_format: RootFormat::NoteName(note.name()),
        }
    }

    /// Create from a parsed note name token
    pub fn from_note_token(token: MusicalNoteToken) -> Self {
        let note = MusicalNote::from_letter_and_accidental(token.letter, token.accidental);
        Self::from_note_name(note)
    }

    /// Create from a parsed Roman numeral token
    pub fn from_roman_token(token: RomanNumeralToken) -> Self {
        Self::from_roman_numeral(token.degree, token.case)
    }

    /// Parse from a string (auto-detect format)
    pub fn from_string(s: &str) -> Option<Self> {
        // Try scale degree (single digit 1-7)
        if let Ok(degree) = s.parse::<u8>() {
            if (1..=7).contains(&degree) {
                return Some(Self::from_scale_degree(degree, None));
            }
        }

        // Try roman numeral
        if let Some((degree, case)) = Self::parse_roman_numeral(s) {
            return Some(Self::from_roman_numeral(degree, case));
        }

        // Try note name
        if let Some(note) = MusicalNote::from_string(s) {
            return Some(Self::from_note_name(note));
        }

        None
    }

    /// Parse a roman numeral string
    fn parse_roman_numeral(s: &str) -> Option<(u8, RomanCase)> {
        let upper = s.to_uppercase();
        let case = if s.chars().all(|c| !c.is_lowercase() || !c.is_alphabetic()) {
            RomanCase::Upper
        } else {
            RomanCase::Lower
        };

        let degree = match upper.as_str() {
            "I" => 1,
            "II" => 2,
            "III" => 3,
            "IV" => 4,
            "V" => 5,
            "VI" => 6,
            "VII" => 7,
            _ => return None,
        };

        Some((degree, case))
    }

    /// Check if this notation is key-relative (needs key context to resolve to a note)
    pub fn is_key_relative(&self) -> bool {
        matches!(
            self.original_format,
            RootFormat::ScaleDegree(_) | RootFormat::RomanNumeral { .. }
        )
    }

    /// Get the roman case if this is a roman numeral root
    pub fn roman_case(&self) -> Option<RomanCase> {
        match &self.original_format {
            RootFormat::RomanNumeral { case, .. } => Some(*case),
            _ => None,
        }
    }

    /// Get the original format
    pub fn original_format(&self) -> &RootFormat {
        &self.original_format
    }

    /// Get the resolved note (if available)
    pub fn resolved_note(&self) -> Option<&MusicalNote> {
        self.resolved_note.as_ref()
    }

    /// Resolve to a MusicalNote (cloned), optionally using a key context
    ///
    /// For note names, returns the note directly.
    /// For scale degrees and roman numerals, requires a Key to resolve.
    ///
    /// # Arguments
    /// * `key` - Optional key context for resolving scale degrees/roman numerals
    ///
    /// # Returns
    /// * `Some(MusicalNote)` if resolved successfully
    /// * `None` if key context is required but not provided
    pub fn resolve(&self, key: Option<&crate::key::Key>) -> Option<MusicalNote> {
        // If already resolved, return it
        if let Some(ref note) = self.resolved_note {
            return Some(note.clone());
        }

        // For note names without resolved_note, shouldn't happen but handle it
        if matches!(self.original_format, RootFormat::NoteName(_)) {
            return None; // This should have been set during construction
        }

        // For scale degrees and roman numerals, need a key
        let key = key?;

        let degree = match &self.original_format {
            RootFormat::ScaleDegree(d) => *d,
            RootFormat::RomanNumeral { degree, .. } => *degree,
            RootFormat::NoteName(_) => unreachable!(),
            RootFormat::Empty => return None, // Empty root has no note
        };

        let resolved = key.get_scale_degree(degree)?;
        Some(MusicalNote::new(resolved.name(), resolved.semitone()))
    }

    /// Resolve to an actual note given a key context
    /// For key-relative notations, this computes the note from the scale degree
    pub fn resolve_with_key(&mut self, key: &crate::key::Key) -> Result<&MusicalNote, String> {
        if let Some(ref note) = self.resolved_note {
            return Ok(note);
        }

        let degree = match &self.original_format {
            RootFormat::ScaleDegree(d) => *d,
            RootFormat::RomanNumeral { degree, .. } => *degree,
            RootFormat::NoteName(_) => {
                return Err("Note name should already be resolved".to_string());
            }
            RootFormat::Empty => {
                return Err("Empty root notation has no note to resolve".to_string());
            }
        };

        let resolved = key
            .get_scale_degree(degree)
            .ok_or_else(|| format!("Could not resolve scale degree {} in key", degree))?;

        // Convert Box<dyn Note> to MusicalNote
        // We need to extract the MusicalNote from the Box<dyn Note>
        let note = MusicalNote::new(resolved.name(), resolved.semitone());
        self.resolved_note = Some(note);

        Ok(self.resolved_note.as_ref().unwrap())
    }

    /// Get the scale degree if this is a key-relative notation (1-7)
    pub fn scale_degree(&self) -> Option<u8> {
        match &self.original_format {
            RootFormat::ScaleDegree(d) => Some(*d),
            RootFormat::RomanNumeral { degree, .. } => Some(*degree),
            RootFormat::NoteName(_) | RootFormat::Empty => None,
        }
    }

    /// Clone the root notation
    pub fn clone_notation(&self) -> Self {
        self.clone()
    }

    /// Convert this root notation to LilyPond format
    ///
    /// # Arguments
    /// * `key` - Optional key context for resolving scale degrees and roman numerals
    ///
    /// # Returns
    /// LilyPond note name (e.g., "cis", "des", "f")
    pub fn to_lilypond(&self, key: Option<&crate::key::Key>) -> String {
        // Try to resolve the note
        if let Some(note) = self.resolve(key) {
            return note.to_lilypond();
        }

        // Fallback: if it's already resolved, use it
        if let Some(note) = self.resolved_note() {
            return note.to_lilypond();
        }

        // Last resort: use a placeholder
        "c".to_string()
    }
}

impl fmt::Display for RootNotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_str = match &self.original_format {
            RootFormat::ScaleDegree(d) => d.to_string(),
            RootFormat::RomanNumeral { degree, case } => {
                let base = match *degree {
                    1 => "I",
                    2 => "II",
                    3 => "III",
                    4 => "IV",
                    5 => "V",
                    6 => "VI",
                    7 => "VII",
                    _ => "?",
                };
                match case {
                    RomanCase::Upper => base.to_string(),
                    RomanCase::Lower => base.to_lowercase(),
                }
            }
            RootFormat::NoteName(name) => name.clone(),
            RootFormat::Empty => String::new(), // Empty display for rests
        };
        write!(f, "{}", display_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_scale_degree() {
        let root = RootNotation::from_scale_degree(4, None);
        assert!(root.is_key_relative());
        assert_eq!(format!("{}", root), "4");
    }

    #[test]
    fn test_from_roman_numeral() {
        let root = RootNotation::from_roman_numeral(5, RomanCase::Upper);
        assert!(root.is_key_relative());
        assert_eq!(format!("{}", root), "V");

        let root = RootNotation::from_roman_numeral(2, RomanCase::Lower);
        assert_eq!(format!("{}", root), "ii");
    }

    #[test]
    fn test_from_note_name() {
        let note = MusicalNote::from_string("C#").unwrap();
        let root = RootNotation::from_note_name(note);
        assert!(!root.is_key_relative());
        assert_eq!(format!("{}", root), "C#");
        assert!(root.resolved_note().is_some());
    }

    #[test]
    fn test_from_string_scale_degree() {
        let root = RootNotation::from_string("4").unwrap();
        assert!(root.is_key_relative());
        assert_eq!(format!("{}", root), "4");
    }

    #[test]
    fn test_from_string_roman_numeral() {
        let root = RootNotation::from_string("V").unwrap();
        assert!(root.is_key_relative());
        assert_eq!(format!("{}", root), "V");

        let root = RootNotation::from_string("vi").unwrap();
        assert_eq!(format!("{}", root), "vi");
    }

    #[test]
    fn test_from_string_note_name() {
        let root = RootNotation::from_string("Eb").unwrap();
        assert!(!root.is_key_relative());
        assert_eq!(format!("{}", root), "Eb");
    }

    #[test]
    fn test_interchangeable() {
        // All three formats should work the same way from the application's perspective
        let scale_degree = RootNotation::from_string("4").unwrap();
        let roman = RootNotation::from_string("IV").unwrap();
        let note = RootNotation::from_string("F").unwrap();

        // All are RootNotation types - completely interchangeable
        assert!(scale_degree.is_key_relative());
        assert!(roman.is_key_relative());
        assert!(!note.is_key_relative());

        // But they remember their original format
        assert_eq!(format!("{}", scale_degree), "4");
        assert_eq!(format!("{}", roman), "IV");
        assert_eq!(format!("{}", note), "F");
    }
}
