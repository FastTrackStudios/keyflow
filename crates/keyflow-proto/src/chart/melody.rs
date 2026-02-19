//! Melody notation system
//!
//! Provides a system for notating melodic lines with pitches and rhythms.
//!
//! Syntax:
//! - `m{ C_8 D_8 E_4 }` - inline melody block
//! - `mainRiff = m{ C_8 D_8 E_4 }` - variable assignment
//! - `$mainRiff` - variable reference
//!
//! Note format: `<pitch><modifiers>_<duration>`
//! - Pitch: C, D, E, F, G, A, B (with optional # or b) OR scale degrees 1-7
//! - Octave modifiers: `'` to jump up an octave, `,` to drop down an octave
//! - Octave: optional explicit number (overrides relative mode)
//! - Duration: Lilypond-style (_4 = quarter, _8 = eighth, _16 = sixteenth, etc.)
//!
//! Relative mode (default): Each note chooses the closest octave to the previous note.
//! Use `'` or `,` to force octave jumps when the closest isn't what you want.
//!
//! Examples:
//! - `C_4` - C quarter note (relative to previous)
//! - `D#'_8` - D# eighth note, one octave up from closest
//! - `Bb,_2` - Bb half note, one octave down from closest
//! - `C5_4` - C5 quarter note (explicit octave)
//! - `3_8` - Scale degree 3 eighth note
//! - `r_4` - quarter rest

use crate::time::AbsolutePosition;
use facet::Facet;
use std::collections::HashMap;
use std::fmt;

/// Pitch class values for interval calculation (C=0, D=2, E=4, F=5, G=7, A=9, B=11)
const PITCH_CLASS: [(&str, i8); 7] = [
    ("C", 0),
    ("D", 2),
    ("E", 4),
    ("F", 5),
    ("G", 7),
    ("A", 9),
    ("B", 11),
];

/// Scale degree to pitch letter mapping (1=C, 2=D, etc. in C major)
const SCALE_DEGREES: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];

/// Octave modifier for relative pitch calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet, Default)]
#[repr(u8)]
pub enum OctaveModifier {
    /// No modifier - use closest octave (relative mode)
    #[default]
    None,
    /// `'` - force one octave up from closest
    Up,
    /// `,` - force one octave down from closest
    Down,
}

/// A single note in a melody
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct MelodyNote {
    /// The pitch name (C, D, E, F, G, A, B) with optional accidental
    /// "r" for rest
    pub pitch: String,

    /// Explicit octave number (None = use relative mode)
    pub octave: Option<u8>,

    /// Octave modifier for relative mode (`'` or `,`)
    pub octave_modifier: OctaveModifier,

    /// Duration as a Lilypond-style value (4 = quarter, 8 = eighth, etc.)
    pub duration: u8,

    /// Is this note dotted?
    pub dotted: bool,

    /// Original scale degree if parsed from number (1-7)
    pub scale_degree: Option<u8>,

    /// Position in the song (set during position calculation)
    pub position: Option<AbsolutePosition>,
}

impl MelodyNote {
    /// Create a new melody note
    pub fn new(pitch: impl Into<String>, duration: u8) -> Self {
        Self {
            pitch: pitch.into(),
            octave: None,
            octave_modifier: OctaveModifier::None,
            duration,
            dotted: false,
            scale_degree: None,
            position: None,
        }
    }

    /// Create a rest
    pub fn rest(duration: u8) -> Self {
        Self {
            pitch: "r".to_string(),
            octave: None,
            octave_modifier: OctaveModifier::None,
            duration,
            dotted: false,
            scale_degree: None,
            position: None,
        }
    }

    /// Set the octave
    pub fn with_octave(mut self, octave: u8) -> Self {
        self.octave = Some(octave);
        self
    }

    /// Make this note dotted
    pub fn dotted(mut self) -> Self {
        self.dotted = true;
        self
    }

    /// Check if this is a rest
    pub fn is_rest(&self) -> bool {
        self.pitch == "r"
    }

    /// Set the octave modifier
    pub fn with_octave_modifier(mut self, modifier: OctaveModifier) -> Self {
        self.octave_modifier = modifier;
        self
    }

    /// Get the pitch class (0-11) for this note, ignoring octave
    /// Returns None for rests
    pub fn pitch_class(&self) -> Option<i8> {
        if self.is_rest() {
            return None;
        }

        let base_pitch = self.pitch.chars().next()?;
        let base_class = PITCH_CLASS
            .iter()
            .find(|(p, _)| p.starts_with(base_pitch))
            .map(|(_, c)| *c)?;

        // Apply accidentals
        let accidental_offset = if self.pitch.contains('#') {
            1
        } else if self.pitch.contains('b') {
            -1
        } else {
            0
        };

        Some((base_class + accidental_offset).rem_euclid(12))
    }

    /// Resolve the absolute octave for this note given a reference pitch
    ///
    /// In relative mode (no explicit octave), chooses the closest octave to the reference.
    /// Applies octave modifiers (`'` or `,`) after finding closest.
    ///
    /// # Arguments
    /// * `ref_pitch_class` - Pitch class of the reference note (0-11)
    /// * `ref_octave` - Octave of the reference note
    ///
    /// # Returns
    /// The resolved octave for this note
    pub fn resolve_octave(&self, ref_pitch_class: i8, ref_octave: u8) -> u8 {
        // If explicit octave is set, use it (ignores modifiers)
        if let Some(oct) = self.octave {
            return oct;
        }

        let Some(this_class) = self.pitch_class() else {
            return ref_octave; // Rest, keep same octave context
        };

        // Calculate interval (how many semitones up from ref to this note in same octave)
        let interval = (this_class - ref_pitch_class).rem_euclid(12);

        // Choose closest octave:
        // If interval <= 6, stay in same octave
        // If interval > 6, go down an octave (closer to come from below)
        let base_octave = if interval <= 6 {
            ref_octave
        } else {
            ref_octave.saturating_sub(1)
        };

        // Apply octave modifier
        match self.octave_modifier {
            OctaveModifier::None => base_octave,
            OctaveModifier::Up => base_octave.saturating_add(1),
            OctaveModifier::Down => base_octave.saturating_sub(1),
        }
    }

    /// Parse a melody note from a string
    ///
    /// Format: `<pitch><modifiers>_<duration>` or `<pitch><modifiers>_<duration>.`
    ///
    /// Pitch can be:
    /// - Letter: C, D, E, F, G, A, B (case-insensitive)
    /// - With accidental: C#, Db, F#, Bb, etc.
    /// - Scale degree: 1, 2, 3, 4, 5, 6, 7
    ///
    /// Modifiers (after pitch, before underscore):
    /// - `'` - octave up
    /// - `,` - octave down
    /// - Number - explicit octave (e.g., C5, D#4)
    ///
    /// Examples:
    /// - `C_4` - C quarter note (relative mode)
    /// - `D#'_8` - D# eighth note, one octave up
    /// - `Bb,_2.` - Bb dotted half note, one octave down
    /// - `C5_4` - C5 quarter note (explicit octave)
    /// - `3_8` - Scale degree 3 eighth note
    /// - `r_4` - quarter rest
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();

        if s.is_empty() {
            return Err("Empty note string".to_string());
        }

        // Check for dotted
        let (note_str, dotted) = if s.ends_with('.') {
            (&s[..s.len() - 1], true)
        } else {
            (s, false)
        };

        // Split on underscore to get pitch and duration
        let parts: Vec<&str> = note_str.split('_').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid note format '{}': expected <pitch>_<duration>",
                s
            ));
        }

        let pitch_part = parts[0];
        let duration_str = parts[1];

        // Parse duration
        let duration: u8 = duration_str
            .parse()
            .map_err(|_| format!("Invalid duration: {}", duration_str))?;

        // Check for rest
        if pitch_part == "r" || pitch_part == "R" {
            return Ok(MelodyNote {
                pitch: "r".to_string(),
                octave: None,
                octave_modifier: OctaveModifier::None,
                duration,
                dotted,
                scale_degree: None,
                position: None,
            });
        }

        let mut chars = pitch_part.chars().peekable();
        let first_char = *chars.peek().ok_or("Missing pitch")?;

        // Check if it's a scale degree (starts with 1-7)
        if first_char.is_ascii_digit() {
            let degree: u8 = chars
                .next()
                .unwrap()
                .to_digit(10)
                .ok_or("Invalid scale degree")? as u8;

            if !(1..=7).contains(&degree) {
                return Err(format!("Scale degree must be 1-7, got {}", degree));
            }

            // Convert to pitch letter
            let pitch = SCALE_DEGREES[(degree - 1) as usize].to_string();

            // Check for accidental after the number
            let mut final_pitch = pitch;
            if let Some(&c) = chars.peek()
                && (c == '#' || c == 'b') {
                    final_pitch.push(chars.next().unwrap());
                }

            // Check for octave modifier
            let octave_modifier = Self::parse_octave_modifier(&mut chars);

            // Check for explicit octave
            let octave = Self::parse_explicit_octave(&mut chars)?;

            return Ok(MelodyNote {
                pitch: final_pitch,
                octave,
                octave_modifier,
                duration,
                dotted,
                scale_degree: Some(degree),
                position: None,
            });
        }

        // Parse pitch letter
        let pitch_letter = chars
            .next()
            .ok_or("Missing pitch letter")?
            .to_uppercase()
            .to_string();

        if !matches!(
            pitch_letter.as_str(),
            "C" | "D" | "E" | "F" | "G" | "A" | "B"
        ) {
            return Err(format!("Invalid pitch letter: {}", pitch_letter));
        }

        // Check for accidental
        let mut pitch = pitch_letter;
        if let Some(&c) = chars.peek()
            && (c == '#' || c == 'b') {
                pitch.push(chars.next().unwrap());
            }

        // Check for octave modifier
        let octave_modifier = Self::parse_octave_modifier(&mut chars);

        // Check for explicit octave
        let octave = Self::parse_explicit_octave(&mut chars)?;

        Ok(MelodyNote {
            pitch,
            octave,
            octave_modifier,
            duration,
            dotted,
            scale_degree: None,
            position: None,
        })
    }

    /// Parse octave modifier (`'` or `,`) from character iterator
    fn parse_octave_modifier(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> OctaveModifier {
        match chars.peek() {
            Some('\'') => {
                chars.next();
                OctaveModifier::Up
            }
            Some(',') => {
                chars.next();
                OctaveModifier::Down
            }
            _ => OctaveModifier::None,
        }
    }

    /// Parse explicit octave number from remaining characters
    fn parse_explicit_octave(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<Option<u8>, String> {
        if chars.peek().is_some() {
            let octave_str: String = chars.collect();
            Ok(Some(
                octave_str
                    .parse::<u8>()
                    .map_err(|_| format!("Invalid octave: {}", octave_str))?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Get duration in beats (assuming quarter note = 1 beat)
    pub fn duration_beats(&self) -> f64 {
        let base = 4.0 / self.duration as f64;
        if self.dotted { base * 1.5 } else { base }
    }
}

impl fmt::Display for MelodyNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use scale degree if available, otherwise pitch letter
        if let Some(degree) = self.scale_degree {
            write!(f, "{}", degree)?;
            // Add accidental if present (after removing base letter)
            if self.pitch.len() > 1 {
                write!(f, "{}", &self.pitch[1..])?;
            }
        } else {
            write!(f, "{}", self.pitch)?;
        }

        // Add octave modifier
        match self.octave_modifier {
            OctaveModifier::Up => write!(f, "'")?,
            OctaveModifier::Down => write!(f, ",")?,
            OctaveModifier::None => {}
        }

        // Add explicit octave if set
        if let Some(oct) = self.octave {
            write!(f, "{}", oct)?;
        }

        write!(f, "_{}", self.duration)?;
        if self.dotted {
            write!(f, ".")?;
        }
        Ok(())
    }
}

/// A melody is a sequence of notes
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct Melody {
    /// The notes in this melody
    pub notes: Vec<MelodyNote>,

    /// Optional name (for variable assignment)
    pub name: Option<String>,
}

impl Melody {
    /// Create a new empty melody
    pub fn new() -> Self {
        Self {
            notes: Vec::new(),
            name: None,
        }
    }

    /// Create a melody with the given notes
    pub fn with_notes(notes: Vec<MelodyNote>) -> Self {
        Self { notes, name: None }
    }

    /// Set the name (for variable assignment)
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a note to the melody
    pub fn add_note(&mut self, note: MelodyNote) {
        self.notes.push(note);
    }

    /// Get total duration in beats
    pub fn total_beats(&self) -> f64 {
        self.notes.iter().map(|n| n.duration_beats()).sum()
    }

    /// Parse a melody from a string (the content inside m{ })
    ///
    /// Example: "C_8 D_8 E_4 F_4"
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut notes = Vec::new();

        for token in content.split_whitespace() {
            let note = MelodyNote::parse(token)?;
            notes.push(note);
        }

        Ok(Melody { notes, name: None })
    }

    /// Parse a melody block from a string including the m{ } wrapper
    ///
    /// Format: `m{ <notes> }` or `<name> = m{ <notes> }`
    pub fn parse_block(s: &str) -> Result<(Option<String>, Self), String> {
        let s = s.trim();

        // Check for variable assignment
        let (name, melody_part) = if let Some(eq_pos) = s.find('=') {
            let name = s[..eq_pos].trim().to_string();
            let rest = s[eq_pos + 1..].trim();
            (Some(name), rest)
        } else {
            (None, s)
        };

        // Parse the m{ } block
        if !melody_part.starts_with("m{") {
            return Err("Melody block must start with m{".to_string());
        }

        let close_brace = melody_part
            .rfind('}')
            .ok_or("Missing closing brace in melody block")?;

        let content = &melody_part[2..close_brace];
        let mut melody = Self::parse(content)?;
        melody.name = name.clone();

        Ok((name, melody))
    }
}

impl Default for Melody {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Melody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{} = ", name)?;
        }
        write!(f, "m{{ ")?;
        let notes: Vec<String> = self.notes.iter().map(|n| n.to_string()).collect();
        write!(f, "{}", notes.join(" "))?;
        write!(f, " }}")
    }
}

/// Storage for melody variables
#[derive(Debug, Clone, Default, PartialEq, Facet)]
pub struct MelodyVariables {
    /// Map from variable name to melody
    variables: HashMap<String, Melody>,
}

impl MelodyVariables {
    /// Create new empty variable storage
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Store a melody variable
    pub fn set(&mut self, name: impl Into<String>, melody: Melody) {
        self.variables.insert(name.into(), melody);
    }

    /// Get a melody variable
    pub fn get(&self, name: &str) -> Option<&Melody> {
        self.variables.get(name)
    }

    /// Check if a variable exists
    pub fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Get all variable names
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.variables.keys()
    }

    /// Iterate over all variables
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Melody)> {
        self.variables.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_note() {
        let note = MelodyNote::parse("C_4").unwrap();
        assert_eq!(note.pitch, "C");
        assert_eq!(note.octave, None);
        assert_eq!(note.duration, 4);
        assert!(!note.dotted);
    }

    #[test]
    fn test_parse_note_with_octave() {
        let note = MelodyNote::parse("D5_8").unwrap();
        assert_eq!(note.pitch, "D");
        assert_eq!(note.octave, Some(5));
        assert_eq!(note.duration, 8);
    }

    #[test]
    fn test_parse_note_with_accidental() {
        let note = MelodyNote::parse("F#4_4").unwrap();
        assert_eq!(note.pitch, "F#");
        assert_eq!(note.octave, Some(4));

        let note2 = MelodyNote::parse("Bb3_2").unwrap();
        assert_eq!(note2.pitch, "Bb");
        assert_eq!(note2.octave, Some(3));
    }

    #[test]
    fn test_parse_dotted_note() {
        let note = MelodyNote::parse("G_4.").unwrap();
        assert_eq!(note.pitch, "G");
        assert_eq!(note.duration, 4);
        assert!(note.dotted);
    }

    #[test]
    fn test_parse_rest() {
        let note = MelodyNote::parse("r_4").unwrap();
        assert!(note.is_rest());
        assert_eq!(note.duration, 4);
    }

    #[test]
    fn test_duration_beats() {
        let quarter = MelodyNote::new("C", 4);
        assert_eq!(quarter.duration_beats(), 1.0);

        let eighth = MelodyNote::new("C", 8);
        assert_eq!(eighth.duration_beats(), 0.5);

        let half = MelodyNote::new("C", 2);
        assert_eq!(half.duration_beats(), 2.0);

        let dotted_quarter = MelodyNote::new("C", 4).dotted();
        assert_eq!(dotted_quarter.duration_beats(), 1.5);
    }

    #[test]
    fn test_note_display() {
        let note = MelodyNote::parse("D#5_8").unwrap();
        assert_eq!(format!("{}", note), "D#5_8");

        let dotted = MelodyNote::parse("A_4.").unwrap();
        assert_eq!(format!("{}", dotted), "A_4.");
    }

    #[test]
    fn test_parse_melody() {
        let melody = Melody::parse("C_8 D_8 E_4").unwrap();
        assert_eq!(melody.notes.len(), 3);
        assert_eq!(melody.notes[0].pitch, "C");
        assert_eq!(melody.notes[1].pitch, "D");
        assert_eq!(melody.notes[2].pitch, "E");
    }

    #[test]
    fn test_parse_melody_block() {
        let (name, melody) = Melody::parse_block("m{ C_8 D_8 E_4 }").unwrap();
        assert!(name.is_none());
        assert_eq!(melody.notes.len(), 3);
    }

    #[test]
    fn test_parse_melody_block_with_name() {
        let (name, melody) = Melody::parse_block("mainRiff = m{ C_8 D_8 E_4 }").unwrap();
        assert_eq!(name, Some("mainRiff".to_string()));
        assert_eq!(melody.notes.len(), 3);
    }

    #[test]
    fn test_melody_display() {
        let melody = Melody::parse("C_8 D_8 E_4").unwrap();
        assert_eq!(format!("{}", melody), "m{ C_8 D_8 E_4 }");

        let named = melody.with_name("riff");
        assert_eq!(format!("{}", named), "riff = m{ C_8 D_8 E_4 }");
    }

    #[test]
    fn test_melody_variables() {
        let mut vars = MelodyVariables::new();

        let melody = Melody::parse("C_4 D_4 E_4").unwrap();
        vars.set("mainRiff", melody);

        assert!(vars.contains("mainRiff"));
        assert!(!vars.contains("otherRiff"));

        let retrieved = vars.get("mainRiff").unwrap();
        assert_eq!(retrieved.notes.len(), 3);
    }

    #[test]
    fn test_melody_total_beats() {
        let melody = Melody::parse("C_4 D_4 E_4 F_4").unwrap();
        assert_eq!(melody.total_beats(), 4.0);

        let melody2 = Melody::parse("C_8 D_8 E_8 F_8").unwrap();
        assert_eq!(melody2.total_beats(), 2.0);
    }

    // New tests for scale degrees, octave modifiers, and relative pitch

    #[test]
    fn test_parse_scale_degree() {
        let note = MelodyNote::parse("1_4").unwrap();
        assert_eq!(note.pitch, "C");
        assert_eq!(note.scale_degree, Some(1));
        assert_eq!(note.duration, 4);

        let note = MelodyNote::parse("3_8").unwrap();
        assert_eq!(note.pitch, "E");
        assert_eq!(note.scale_degree, Some(3));

        let note = MelodyNote::parse("5_2").unwrap();
        assert_eq!(note.pitch, "G");
        assert_eq!(note.scale_degree, Some(5));

        let note = MelodyNote::parse("7_4").unwrap();
        assert_eq!(note.pitch, "B");
        assert_eq!(note.scale_degree, Some(7));
    }

    #[test]
    fn test_parse_scale_degree_with_accidental() {
        let note = MelodyNote::parse("3#_4").unwrap();
        assert_eq!(note.pitch, "E#");
        assert_eq!(note.scale_degree, Some(3));

        let note = MelodyNote::parse("7b_8").unwrap();
        assert_eq!(note.pitch, "Bb");
        assert_eq!(note.scale_degree, Some(7));
    }

    #[test]
    fn test_parse_octave_modifier_up() {
        let note = MelodyNote::parse("C'_4").unwrap();
        assert_eq!(note.pitch, "C");
        assert_eq!(note.octave_modifier, OctaveModifier::Up);
        assert_eq!(note.octave, None);

        let note = MelodyNote::parse("D#'_8").unwrap();
        assert_eq!(note.pitch, "D#");
        assert_eq!(note.octave_modifier, OctaveModifier::Up);
    }

    #[test]
    fn test_parse_octave_modifier_down() {
        let note = MelodyNote::parse("G,_4").unwrap();
        assert_eq!(note.pitch, "G");
        assert_eq!(note.octave_modifier, OctaveModifier::Down);
        assert_eq!(note.octave, None);

        let note = MelodyNote::parse("Bb,_2.").unwrap();
        assert_eq!(note.pitch, "Bb");
        assert_eq!(note.octave_modifier, OctaveModifier::Down);
        assert!(note.dotted);
    }

    #[test]
    fn test_scale_degree_with_octave_modifier() {
        let note = MelodyNote::parse("3'_4").unwrap();
        assert_eq!(note.pitch, "E");
        assert_eq!(note.scale_degree, Some(3));
        assert_eq!(note.octave_modifier, OctaveModifier::Up);

        let note = MelodyNote::parse("5,_8").unwrap();
        assert_eq!(note.pitch, "G");
        assert_eq!(note.scale_degree, Some(5));
        assert_eq!(note.octave_modifier, OctaveModifier::Down);
    }

    #[test]
    fn test_pitch_class() {
        let c = MelodyNote::new("C", 4);
        assert_eq!(c.pitch_class(), Some(0));

        let d = MelodyNote::new("D", 4);
        assert_eq!(d.pitch_class(), Some(2));

        let fsharp = MelodyNote::new("F#", 4);
        assert_eq!(fsharp.pitch_class(), Some(6)); // F=5, +1 for sharp

        let bb = MelodyNote::new("Bb", 4);
        assert_eq!(bb.pitch_class(), Some(10)); // B=11, -1 for flat

        let rest = MelodyNote::rest(4);
        assert_eq!(rest.pitch_class(), None);
    }

    #[test]
    fn test_resolve_octave_closest() {
        // C4 -> D should stay in octave 4 (close)
        let d = MelodyNote::new("D", 4);
        assert_eq!(d.resolve_octave(0, 4), 4);

        // C4 -> B should go down to octave 3 (B is closer below)
        let b = MelodyNote::new("B", 4);
        assert_eq!(b.resolve_octave(0, 4), 3);

        // G4 -> A should stay in octave 4 (close)
        let a = MelodyNote::new("A", 4);
        assert_eq!(a.resolve_octave(7, 4), 4); // G = pitch class 7
    }

    #[test]
    fn test_resolve_octave_with_modifier() {
        // C4 -> D' should go to octave 5 (up modifier)
        let d_up = MelodyNote::new("D", 4).with_octave_modifier(OctaveModifier::Up);
        assert_eq!(d_up.resolve_octave(0, 4), 5);

        // C4 -> D, should go to octave 3 (down modifier)
        let d_down = MelodyNote::new("D", 4).with_octave_modifier(OctaveModifier::Down);
        assert_eq!(d_down.resolve_octave(0, 4), 3);
    }

    #[test]
    fn test_resolve_octave_explicit_overrides() {
        // Explicit octave ignores relative calculation
        let note = MelodyNote::new("D", 4).with_octave(6);
        assert_eq!(note.resolve_octave(0, 4), 6);
    }

    #[test]
    fn test_display_with_octave_modifier() {
        let note = MelodyNote::parse("C'_4").unwrap();
        assert_eq!(format!("{}", note), "C'_4");

        let note = MelodyNote::parse("D,_8").unwrap();
        assert_eq!(format!("{}", note), "D,_8");
    }

    #[test]
    fn test_display_scale_degree() {
        let note = MelodyNote::parse("3_4").unwrap();
        assert_eq!(format!("{}", note), "3_4");

        let note = MelodyNote::parse("5#'_8").unwrap();
        assert_eq!(format!("{}", note), "5#'_8");
    }

    #[test]
    fn test_melody_with_mixed_notation() {
        let melody = Melody::parse("C_4 3_8 G'_8 1,_4").unwrap();
        assert_eq!(melody.notes.len(), 4);
        assert_eq!(melody.notes[0].pitch, "C");
        assert_eq!(melody.notes[1].pitch, "E");
        assert_eq!(melody.notes[1].scale_degree, Some(3));
        assert_eq!(melody.notes[2].pitch, "G");
        assert_eq!(melody.notes[2].octave_modifier, OctaveModifier::Up);
        assert_eq!(melody.notes[3].pitch, "C");
        assert_eq!(melody.notes[3].octave_modifier, OctaveModifier::Down);
    }

    #[test]
    fn test_invalid_scale_degree() {
        assert!(MelodyNote::parse("0_4").is_err());
        assert!(MelodyNote::parse("8_4").is_err());
        assert!(MelodyNote::parse("9_4").is_err());
    }
}
