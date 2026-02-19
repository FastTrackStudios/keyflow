//! MIDI event integration for chord detection
//!
//! Provides utilities for converting MIDI events to chords and working with MIDI note data.

use crate::chord::quality::SuspendedType;
use crate::chord::{Chord, ChordDegree, ChordQuality, from_semitones};
use crate::primitives::note::Note;
use crate::primitives::{MusicalNote, RootNotation};
use helgoboss_midi::KeyNumber;

/// A MIDI note event with timing information
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MidiNote {
    /// MIDI pitch number (0-127)
    pub pitch: u8,
    /// Start time in PPQ (parts per quarter note)
    pub start_ppq: i64,
    /// End time in PPQ (parts per quarter note)
    pub end_ppq: i64,
    /// MIDI channel (0-15)
    pub channel: u8,
    /// Note velocity (0-127)
    pub velocity: u8,
}

impl MidiNote {
    /// Create a new MIDI note
    pub fn new(pitch: u8, start_ppq: i64, end_ppq: i64, channel: u8, velocity: u8) -> Self {
        Self {
            pitch,
            start_ppq,
            end_ppq,
            channel,
            velocity,
        }
    }

    /// Get the note name with octave (e.g., "C4", "A#3")
    pub fn note_name(&self) -> Option<MidiNoteName> {
        MidiNoteName::from_midi_pitch(self.pitch)
    }
}

/// MIDI note with note name and octave
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiNoteName {
    note: MusicalNote,
    octave: i32,
}

impl MidiNoteName {
    /// Create from MIDI pitch number (0-127)
    /// MIDI 0 = C-1, MIDI 1 = C#-1, MIDI 12 = C0, MIDI 60 = C4 (middle C), etc.
    pub fn from_midi_pitch(pitch: u8) -> Option<Self> {
        // Validate using helgoboss-midi
        KeyNumber::try_from(pitch).ok()?;

        let semitone = pitch % 12;
        let octave = (pitch / 12) as i32 - 1; // MIDI 0-11 is octave -1, 12-23 is octave 0, etc.
        let note = MusicalNote::from_semitone(semitone, true); // prefer sharp

        Some(Self { note, octave })
    }

    /// Get the note (without octave)
    pub fn note(&self) -> MusicalNote {
        self.note.clone()
    }

    /// Get the octave
    pub fn octave(&self) -> i32 {
        self.octave
    }
}

impl std::fmt::Display for MidiNoteName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.note, self.octave)
    }
}

/// Convert MIDI pitch to note name string
pub fn midi_pitch_to_note_name(pitch: u8) -> String {
    MidiNoteName::from_midi_pitch(pitch)
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("Invalid({})", pitch))
}

/// Detected chord with timing information
#[derive(Debug, Clone)]
pub struct DetectedChord {
    /// The detected chord
    pub chord: Chord,
    /// Start time in PPQ
    pub start_ppq: i64,
    /// End time in PPQ
    pub end_ppq: i64,
    /// Root pitch (MIDI note number)
    pub root_pitch: u8,
    /// Maximum velocity of notes in this chord (0-127)
    /// Used for phrase marker detection (velocity > 120 = accented)
    pub velocity: u8,
}

impl DetectedChord {
    /// Create a new detected chord
    pub fn new(chord: Chord, start_ppq: i64, end_ppq: i64, root_pitch: u8, velocity: u8) -> Self {
        Self {
            chord,
            start_ppq,
            end_ppq,
            root_pitch,
            velocity,
        }
    }

    /// Check if this chord is accented (velocity > 120)
    /// Used for phrase marker (`>`) detection in chart generation
    pub fn is_accented(&self) -> bool {
        self.velocity > 120
    }
}

/// Active note at a specific time (internal use)
#[derive(Debug, Clone)]
struct ActiveNote {
    pitch: u8,
    start_ppq: i64,
    end_ppq: i64,
    velocity: u8,
}

/// Detect chords from MIDI notes using keyflow's semitone sequence analysis
///
/// Based on Lil Chordbox.lua's GetChords function:
/// - Process notes sequentially, tracking active notes
/// - Build chords when 2+ notes overlap
/// - Filter out very short chords (< min_chord_duration_ppq) to avoid arpeggiated fragments
/// - Merge consecutive identical chords
///
/// # Arguments
///
/// * `notes` - Slice of MIDI notes to analyze
/// * `min_chord_duration_ppq` - Minimum chord duration in PPQ to filter out arpeggiated fragments (e.g., 180)
///
/// # Returns
///
/// Vector of detected chords with timing information
pub fn detect_chords_from_midi_notes(
    notes: &[MidiNote],
    min_chord_duration_ppq: i64,
) -> Vec<DetectedChord> {
    if notes.is_empty() {
        return Vec::new();
    }

    // Sort notes by start time (like Lil Chordbox)
    let mut sorted_notes = notes.to_vec();
    sorted_notes.sort_by_key(|n| n.start_ppq);

    let mut chords = Vec::new();
    let mut active_notes: Vec<ActiveNote> = Vec::new();
    let mut chord_min_eppq: Option<i64> = None;

    // Process notes sequentially (like Lil Chordbox's GetChords)
    for note_info in &sorted_notes {
        let note = ActiveNote {
            pitch: note_info.pitch,
            start_ppq: note_info.start_ppq,
            end_ppq: note_info.end_ppq,
            velocity: note_info.velocity,
        };

        // Update chord_min_eppq (earliest end time of active notes)
        chord_min_eppq = chord_min_eppq
            .map(|min| min.min(note.end_ppq))
            .or(Some(note.end_ppq));

        // If this note starts after or at the earliest end time, process existing active notes
        if note.start_ppq >= chord_min_eppq.unwrap_or(0) {
            // Build chord from current active notes if we have 2+
            if active_notes.len() >= 2 {
                if let Some(chord) = build_chord_from_notes(
                    &active_notes,
                    chord_min_eppq.unwrap_or(0),
                    note.start_ppq,
                    min_chord_duration_ppq,
                ) {
                    chords.push(chord);
                }
            }

            // Remove notes that have ended before or at this note's start time
            // Use >= instead of > to ensure notes ending exactly when new note starts are removed
            let mut new_notes = Vec::new();
            chord_min_eppq = None;
            for active_note in &active_notes {
                if active_note.end_ppq > note.start_ppq {
                    // Note is still active - keep it
                    new_notes.push(active_note.clone());
                    chord_min_eppq = chord_min_eppq
                        .map(|min| min.min(active_note.end_ppq))
                        .or(Some(active_note.end_ppq));
                }
                // Notes ending exactly at note.start_ppq are removed (not >)
            }
            active_notes = new_notes;

            // Update chord_min_eppq with new note
            chord_min_eppq = chord_min_eppq
                .map(|min| min.min(note.end_ppq))
                .or(Some(note.end_ppq));
        } else {
            // Note starts before earliest end - build chord from current active notes
            // This happens when a new note starts while previous notes are still active
            if active_notes.len() >= 2 {
                // Build chord ending at this note's start
                if let Some(chord) = build_chord_from_notes(
                    &active_notes,
                    chord_min_eppq.unwrap_or(0),
                    note.start_ppq,
                    min_chord_duration_ppq,
                ) {
                    chords.push(chord);
                }
            }
        }

        // Add this note to active notes
        active_notes.push(note);
    }

    // Process remaining active notes at the end
    if active_notes.len() >= 2 {
        if let Some(chord) = build_chord_from_notes(
            &active_notes,
            chord_min_eppq.unwrap_or(0),
            i64::MAX,
            min_chord_duration_ppq,
        ) {
            chords.push(chord);
        }
    }

    // Merge consecutive identical chords (like Lil Chordbox)
    // But don't merge if chords start exactly when the previous one ends (separate musical events)
    let mut merged_chords: Vec<DetectedChord> = Vec::new();
    for chord in chords {
        if let Some(last_chord) = merged_chords.last_mut() {
            // Check if this chord is the same as the last one and overlapping (not just adjacent)
            // Only merge if there's actual overlap, not just adjacency
            let has_overlap = last_chord.end_ppq > chord.start_ppq;
            let is_same_chord = last_chord.root_pitch == chord.root_pitch
                && last_chord.chord.quality == chord.chord.quality
                && last_chord.chord.family == chord.chord.family;

            if is_same_chord && has_overlap {
                // Merge: extend the end time
                last_chord.end_ppq = last_chord.end_ppq.max(chord.end_ppq);
                continue;
            }
        }
        merged_chords.push(chord);
    }

    merged_chords
}

/// Build a chord from active notes (helper function)
///
/// Uses the unified from_semitones() function for chord detection,
/// which handles inversion detection automatically.
fn build_chord_from_notes(
    active_notes: &[ActiveNote],
    chord_start_ppq: i64,
    chord_end_limit: i64,
    min_chord_duration_ppq: i64,
) -> Option<DetectedChord> {
    if active_notes.len() < 2 {
        return None;
    }

    // Get pitches and sort them
    let mut pitches: Vec<u8> = active_notes.iter().map(|n| n.pitch).collect();
    pitches.sort();

    // Calculate max velocity from all notes in this chord
    let max_velocity = active_notes.iter().map(|n| n.velocity).max().unwrap_or(0);

    // Find the earliest start and earliest end of active notes (clamped to limit)
    let chord_start = active_notes
        .iter()
        .map(|n| n.start_ppq)
        .min()
        .unwrap_or(chord_start_ppq);
    let chord_end = active_notes
        .iter()
        .map(|n| n.end_ppq)
        .min()
        .unwrap_or(chord_end_limit)
        .min(chord_end_limit);

    // Filter out very short chords (arpeggiated fragments)
    let chord_duration = chord_end - chord_start;
    if chord_duration < min_chord_duration_ppq {
        return None;
    }

    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let lowest_pitch = pitches[0];
    let lowest_pitch_class = lowest_pitch % 12;

    // Convert pitches to semitone sequence relative to lowest pitch
    let semitones = midi_pitches_to_semitone_sequence(&pitches, lowest_pitch);

    // Get root note name from lowest pitch
    let root_name = note_names[lowest_pitch_class as usize];

    if let Some(root_note) = MusicalNote::from_string(root_name) {
        let root = RootNotation::from_note_name(root_note);

        // Use from_semitones which handles inversion detection
        if let Ok(mut chord) = from_semitones(&semitones, root) {
            // Determine actual root pitch after potential inversion detection
            let actual_root_pitch = if chord.bass.is_some() {
                // Inversion was detected - find the MIDI pitch for the actual root
                if let Some(actual_root) = chord.root.resolved_note() {
                    let actual_root_class = actual_root.semitone();
                    // Find a MIDI pitch in our list that matches this pitch class
                    pitches
                        .iter()
                        .find(|&&p| p % 12 == actual_root_class)
                        .copied()
                        .unwrap_or(lowest_pitch)
                } else {
                    lowest_pitch
                }
            } else {
                lowest_pitch
            };

            // Apply MIDI-specific adjustments for octave context
            apply_midi_octave_adjustments(&mut chord, &pitches, lowest_pitch, &semitones);

            return Some(DetectedChord {
                chord,
                start_ppq: chord_start,
                end_ppq: chord_end,
                root_pitch: actual_root_pitch,
                velocity: max_velocity,
            });
        }
    }

    None
}

/// Apply MIDI-specific adjustments based on octave context
///
/// These adjustments handle cases where the octave position of notes
/// affects the chord interpretation (e.g., add4 vs add11).
fn apply_midi_octave_adjustments(
    chord: &mut Chord,
    pitches: &[u8],
    root_pitch: u8,
    semitones: &[u8],
) {
    // Get pitch classes to check for specific intervals
    let pitch_classes: std::collections::HashSet<u8> = semitones.iter().map(|&s| s % 12).collect();
    let has_major_third = pitch_classes.contains(&4);
    let has_fourth = pitch_classes.contains(&5);

    // Check if 3rd and 4th are in the same octave by examining actual MIDI pitches
    let third_pitches: Vec<u8> = pitches
        .iter()
        .filter(|&&p| (p % 12) == ((root_pitch % 12) + 4) % 12)
        .copied()
        .collect();
    let fourth_pitches: Vec<u8> = pitches
        .iter()
        .filter(|&&p| (p % 12) == ((root_pitch % 12) + 5) % 12)
        .copied()
        .collect();

    let third_and_fourth_same_octave = if !third_pitches.is_empty() && !fourth_pitches.is_empty() {
        let third_octave = third_pitches[0] / 12;
        let fourth_octave = fourth_pitches[0] / 12;
        third_octave == fourth_octave
    } else {
        let has_third_simple = semitones.contains(&4);
        let has_fourth_simple = semitones.contains(&5);
        has_third_simple && has_fourth_simple
    };

    // Fix: If we have both a 3rd AND a 4th, it's "add4" not "sus4"
    if has_major_third && has_fourth && chord.quality.is_suspended() {
        chord.quality = ChordQuality::Major;
    }

    // Handle add4 vs add11 based on octave
    if chord.family.is_none() {
        if chord.extensions.eleventh.is_some() {
            if third_and_fourth_same_octave {
                // Both 3rd and 4th in same octave - convert to add4
                chord.additions.push(ChordDegree::Fourth);
                chord.extensions.eleventh = None;
            } else {
                // Different octaves - convert to add11
                chord.additions.push(ChordDegree::Eleventh);
                chord.extensions.eleventh = None;
            }
        }

        // Also handle case where we have 3rd and 4th in same octave but no extension detected
        if third_and_fourth_same_octave && chord.extensions.eleventh.is_none() {
            if !chord.additions.contains(&ChordDegree::Fourth)
                && !chord.additions.contains(&ChordDegree::Eleventh)
                && has_fourth
                && has_major_third
            {
                chord.additions.push(ChordDegree::Fourth);
            }
        }
    }

    // Handle "D2" style chords: power chord with 2nd
    // When we have root, 5th, and 2nd/9th but no 3rd, use "2" naming
    let has_root = semitones.contains(&0);
    let has_fifth = pitch_classes.contains(&7);
    let has_second = pitch_classes.contains(&2);
    let has_third = pitch_classes.contains(&3) || pitch_classes.contains(&4);

    // Handle power chord with second (sus2-like voicing)
    // But NOT if there's a ninth extension - then it's a 9th chord, not add2
    let has_ninth_extension = chord.extensions.ninth.is_some();

    if has_root && has_fifth && has_second && !has_third && !has_ninth_extension {
        // Only convert to Power+add2 when there's no 7th family.
        // If a 7th is present and quality is sus2, this is a 7sus2 chord (e.g., C7sus2),
        // NOT a power chord with add2. Converting to Power would lose the sus2 quality
        // since the Power→sus2 reconversion below only fires when family.is_none().
        let is_sus2_with_seventh = chord.quality.is_suspended()
            && matches!(
                chord.quality,
                ChordQuality::Suspended(SuspendedType::Second)
            )
            && chord.family.is_some();

        if !is_sus2_with_seventh
            && (chord.quality.is_suspended() || chord.quality == ChordQuality::Power)
        {
            chord.quality = ChordQuality::Power;
            chord.additions.retain(|&d| d != ChordDegree::Ninth);
            if !chord.additions.contains(&ChordDegree::Second) {
                chord.additions.push(ChordDegree::Second);
            }
        }
    }

    // Consolidate Second and Ninth - keep only Second (if both are in additions)
    // Also remove Second addition if there's a ninth extension
    if has_ninth_extension {
        chord.additions.retain(|&d| d != ChordDegree::Second);
    } else if chord.additions.contains(&ChordDegree::Second)
        && chord.additions.contains(&ChordDegree::Ninth)
    {
        chord.additions.retain(|&d| d != ChordDegree::Ninth);
    }

    // Convert Power + 2nd/9th (no 3rd, no 7th) → sus2
    // A chord with only root, 2nd/9th, and 5th is a sus2 chord
    // The 2nd/9th can be in extensions.ninth, additions as Ninth, or additions as Second
    // (the D2 code above may have already converted Ninth→Second)
    let has_second_or_ninth = chord.extensions.ninth.is_some()
        || chord.additions.contains(&ChordDegree::Ninth)
        || chord.additions.contains(&ChordDegree::Second);
    if has_root
        && has_fifth
        && !has_third
        && chord.family.is_none()
        && chord.quality == ChordQuality::Power
        && has_second_or_ninth
    {
        chord.quality = ChordQuality::Suspended(SuspendedType::Second);
        chord.extensions.ninth = None;
        chord
            .additions
            .retain(|&d| d != ChordDegree::Ninth && d != ChordDegree::Second);
    }

    // For sus4 chords with a natural 11th extension, the 11th is redundant (same as the 4th)
    // D11sus4 → D7sus4, G11sus4 → G7sus4
    if matches!(
        chord.quality,
        ChordQuality::Suspended(SuspendedType::Fourth)
    ) && chord.extensions.eleventh.is_some()
    {
        chord.extensions.eleventh = None;
    }

    // For sus2 chords with a natural 9th extension, the 9th is redundant (same as the 2nd)
    if matches!(
        chord.quality,
        ChordQuality::Suspended(SuspendedType::Second)
    ) && chord.extensions.ninth.is_some()
    {
        chord.extensions.ninth = None;
    }
}

/// Convert a vector of MIDI pitches to a semitone sequence relative to a root pitch
///
/// This function takes MIDI note pitches and converts them to semitone intervals
/// relative to a specified root pitch. It preserves octave information for extensions
/// (9th, 11th, 13th) while normalizing pitch classes for the base chord.
///
/// # Arguments
///
/// * `pitches` - Vector of MIDI pitch values (0-127)
/// * `root_pitch` - The MIDI pitch to use as the root (0)
///
/// # Returns
///
/// A sorted, deduplicated vector of semitone intervals relative to the root.
/// The root (0) is always included. Extensions are represented with their
/// compound interval values (14 for major 9th, 17 for 11th, 21 for 13th, etc.)
///
/// # Examples
///
/// ```
/// use keyflow_proto::chord::midi::midi_pitches_to_semitone_sequence;
///
/// // C major triad: C4 (60), E4 (64), G4 (67)
/// let pitches = vec![60, 64, 67];
/// let semitones = midi_pitches_to_semitone_sequence(&pitches, 60);
/// assert_eq!(semitones, vec![0, 4, 7]);
///
/// // E major with added 4th: E2 (40), G#3 (56), A3 (57), B3 (59)
/// let pitches = vec![40, 56, 57, 59];
/// let semitones = midi_pitches_to_semitone_sequence(&pitches, 40);
/// // Should have 0 (E), 4 (G#), 5 (A), 7 (B)
/// assert!(semitones.contains(&0));
/// assert!(semitones.contains(&4));
/// assert!(semitones.contains(&5));
/// assert!(semitones.contains(&7));
/// ```
pub fn midi_pitches_to_semitone_sequence(pitches: &[u8], root_pitch: u8) -> Vec<u8> {
    let mut semitones: Vec<u8> = pitches
        .iter()
        .map(|&pitch| {
            // Calculate semitone difference from root (handling negative differences)
            let total_diff = pitch as i16 - root_pitch as i16;
            let diff = total_diff % 12;
            let diff = if diff < 0 { diff + 12 } else { diff };

            // For notes in higher octaves, preserve octave info ONLY for extensions (9th, 11th, 13th)
            // Basic chord tones (3rd, 4th, 5th, 7th) should use simple intervals, even in higher octaves
            // This prevents:
            // - A3 (minor 3rd) from being interpreted as #9 when F#2 is the root
            // - A3 (4th) from being interpreted as 11th when E2 is the root and G#3 (3rd) is in the same octave
            let octave_diff = total_diff / 12;
            if octave_diff > 0 && diff > 0 {
                // Only convert to compound intervals for actual extensions (9th, 11th, 13th)
                // Basic chord tones (3rd=3/4, 4th=5, 5th=7, 7th=10/11) should use simple intervals
                // We'll check later if 3rd and 4th are in same octave to determine add4 vs 11th
                match diff {
                    1 => 13,          // minor 9th (extension)
                    2 => 14,          // major 9th (extension)
                    3 => diff as u8,  // minor 3rd - keep as simple interval, not sharp 9th
                    4 => diff as u8,  // major 3rd - keep as simple interval, not minor 10th
                    5 => diff as u8, // perfect 4th - keep as simple interval, we'll check later if it's 11th
                    6 => 18, // sharp 11th (extension - this is always an extension, not a basic chord tone)
                    7 => diff as u8, // perfect 5th - keep as simple interval, not minor 12th
                    8 => diff as u8, // augmented 5th - keep as simple interval, not perfect 12th
                    9 => 21, // minor 13th (extension)
                    10 => diff as u8, // minor 7th - keep as simple interval, not major 13th
                    11 => diff as u8, // major 7th - keep as simple interval, not minor 14th
                    _ => diff as u8,
                }
            } else {
                // Same octave or lower - use simple interval
                diff as u8
            }
        })
        .collect();

    // Sort and deduplicate
    semitones.sort();
    semitones.dedup();

    // Ensure 0 (root) is included - from_semitones requires it
    if !semitones.contains(&0) {
        semitones.insert(0, 0);
        semitones.sort();
    }

    semitones
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::{ChordDegree, ChordQuality};

    fn create_midi_note(pitch: u8, start_ppq: i64, end_ppq: i64) -> MidiNote {
        MidiNote::new(pitch, start_ppq, end_ppq, 0, 100)
    }

    #[test]
    fn test_midi_pitches_to_semitone_sequence() {
        // C major triad: C4 (60), E4 (64), G4 (67)
        let pitches = vec![60, 64, 67];
        let semitones = midi_pitches_to_semitone_sequence(&pitches, 60);
        assert_eq!(semitones, vec![0, 4, 7]);

        // E major with added 4th: E2 (40), G#3 (56), A3 (57), B3 (59)
        // Note: G#3 is in a higher octave, so it becomes compound interval 16 (minor 10th)
        // But we still need the pitch class 4 (major 3rd) for chord detection
        let pitches = vec![40, 56, 57, 59];
        let semitones = midi_pitches_to_semitone_sequence(&pitches, 40);
        // Should have 0 (E), 5 (A), 7 (B)
        // G# might be 4 or 16 depending on octave handling
        assert!(semitones.contains(&0));
        assert!(semitones.contains(&5) || semitones.iter().any(|&s| s % 12 == 5)); // A (4th)
        assert!(semitones.contains(&7) || semitones.iter().any(|&s| s % 12 == 7)); // B (5th)
        // G# should be present as either 4 (major 3rd) or 16 (minor 10th)
        assert!(
            semitones.iter().any(|&s| s % 12 == 4),
            "Should have G# (major 3rd), got: {:?}",
            semitones
        );
    }

    #[test]
    fn test_e_add4_chord() {
        // E2, E3, G#3, A3, B3 should be EAdd4, not E11
        // E = 40, G# = 56, A = 57, B = 59
        let notes = vec![
            create_midi_note(40, 0, 5760), // E2
            create_midi_note(52, 0, 5760), // E3
            create_midi_note(56, 0, 5760), // G#3
            create_midi_note(57, 0, 5760), // A3
            create_midi_note(59, 0, 5760), // B3
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Detected chord: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Family: {:?}", chord.family);
        println!("Extensions: {:?}", chord.extensions);
        println!("Additions: {:?}", chord.additions);

        // Check what semitones were generated for the selected root
        let root_pitch = chords[0].root_pitch;
        let semitones = midi_pitches_to_semitone_sequence(&[40, 52, 56, 57, 59], root_pitch);
        println!("Root pitch: {}, Semitones: {:?}", root_pitch, semitones);
        println!("Has 16 (3rd compound): {}", semitones.contains(&16));
        println!("Has 17 (11th compound): {}", semitones.contains(&17));
        println!("Has 4 (3rd simple): {}", semitones.contains(&4));
        println!("Has 5 (4th simple): {}", semitones.contains(&5));

        // Should be exactly Eadd4 (E major with added 4th)
        assert_eq!(
            chord_name, "Eadd4",
            "Should be exactly Eadd4, not Eadd11. Additions: {:?}, Extensions: {:?}, Name: {}",
            chord.additions, chord.extensions, chord_name
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Major,
            "Should be exactly Major quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family, None,
            "Should have exactly no 7th family, got {:?}",
            chord.family
        );
        assert_eq!(
            chords[0].root_pitch, 40,
            "Root should be exactly E2 (40), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.additions,
            vec![ChordDegree::Fourth],
            "Should have exactly [Fourth] in additions, got {:?}",
            chord.additions
        );
        assert_eq!(
            chord.extensions.eleventh, None,
            "Should have exactly no 11th extension, got {:?}",
            chord.extensions.eleventh
        );
    }

    #[test]
    fn test_d2_power_chord_with_added_second() {
        // D3, A3, D4, E4, A4 should be Dsus2 (root, 2nd, 5th), not Asus4/D
        // Pattern: root (D), 5th (A), root (D), 2nd (E), 5th (A)
        // D3 = 50, A3 = 57, D4 = 62, E4 = 64, A4 = 69
        let notes = vec![
            create_midi_note(50, 0, 5760), // D3
            create_midi_note(57, 0, 5760), // A3
            create_midi_note(62, 0, 5760), // D4
            create_midi_note(64, 0, 5760), // E4
            create_midi_note(69, 0, 5760), // A4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Detected chord: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Additions: {:?}", chord.additions);

        // Should be exactly Dsus2 (root, 2nd, 5th with no 3rd)
        assert_eq!(
            chord_name, "Dsus2",
            "Should be exactly Dsus2, not Asus4/D. Root pitch: {}, Additions: {:?}, Name: {}",
            chords[0].root_pitch, chord.additions, chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 50,
            "Root should be exactly D3 (50), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Suspended(SuspendedType::Second),
            "Should be exactly Suspended(Second) quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family, None,
            "Should have exactly no 7th family, got {:?}",
            chord.family
        );
        assert!(
            chord.additions.is_empty(),
            "Should have no additions (sus2 is quality, not addition), got {:?}",
            chord.additions
        );
    }

    #[test]
    fn test_b_minor_7th_not_a_sus2() {
        // B2, F#3, B3, D4, F#4, A4 should be Bm7, not Asus2/B
        // B2 = 47, F#3 = 54, B3 = 59, D4 = 62, F#4 = 66, A4 = 69
        // Pattern: root (B), 5th (F#), root (B), minor 3rd (D), 5th (F#), minor 7th (A)
        let notes = vec![
            create_midi_note(47, 0, 2880), // B2
            create_midi_note(54, 0, 2880), // F#3
            create_midi_note(59, 0, 2880), // B3
            create_midi_note(62, 0, 2880), // D4
            create_midi_note(66, 0, 2880), // F#4
            create_midi_note(69, 0, 2880), // A4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Detected chord: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Family: {:?}", chord.family);
        println!("Additions: {:?}", chord.additions);

        // Should be exactly Bm7, not Bm7#9, Asus2/B or D7/B
        // The root should be B2 (47), the lowest pitch
        assert_eq!(
            chord_name, "Bm7",
            "Should be exactly Bm7, not Bm7#9, Asus2/B or D7/B. Root pitch: {}, Quality: {:?}, Family: {:?}, Extensions: {:?}, Name: {}",
            chords[0].root_pitch, chord.quality, chord.family, chord.extensions, chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 47,
            "Root should be exactly B2 (47), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Should have exactly Minor7 family, got {:?}",
            chord.family
        );
        assert_eq!(
            chord.extensions.ninth, None,
            "Should not have 9th extension, got {:?}",
            chord.extensions.ninth
        );
    }

    #[test]
    fn test_f_sharp_minor_no_false_sharp_9() {
        // F#2, F#3, A3, C#4 should be F#m, not F#m#9
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61
        // Pattern: root (F#), root (F#), minor 3rd (A), 5th (C#)
        let notes = vec![
            create_midi_note(42, 0, 2880), // F#2
            create_midi_note(54, 0, 2880), // F#3
            create_midi_note(57, 0, 2880), // A3 (minor 3rd)
            create_midi_note(61, 0, 2880), // C#4 (5th)
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Detected chord: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Extensions: {:?}", chord.extensions);

        // Should be exactly F#m, not F#m#9
        assert_eq!(
            chord_name, "F#m",
            "Should be exactly F#m, not F#m#9. Root pitch: {}, Quality: {:?}, Extensions: {:?}, Name: {}",
            chords[0].root_pitch, chord.quality, chord.extensions, chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family, None,
            "Should have exactly no 7th family, got {:?}",
            chord.family
        );
        assert_eq!(
            chord.extensions.ninth, None,
            "Should have exactly no 9th extension (including #9), got {:?}",
            chord.extensions.ninth
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_no_false_sharp_9() {
        // F#2, F#3, A3, C#4, E4 should be F#m7, not F#m7#9
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        // Pattern: root (F#), root (F#), minor 3rd (A), 5th (C#), minor 7th (E)
        let notes = vec![
            create_midi_note(42, 0, 2880), // F#2
            create_midi_note(54, 0, 2880), // F#3
            create_midi_note(57, 0, 2880), // A3 (minor 3rd)
            create_midi_note(61, 0, 2880), // C#4 (5th)
            create_midi_note(64, 0, 2880), // E4 (minor 7th)
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Detected chord: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Family: {:?}", chord.family);
        println!("Extensions: {:?}", chord.extensions);

        // Should be F#m7, not F#m7#9 - EXACT match required
        assert_eq!(
            chord_name, "F#m7",
            "Should be exactly F#m7, not F#m7#9. Root pitch: {}, Quality: {:?}, Family: {:?}, Extensions: {:?}, Name: {}",
            chords[0].root_pitch, chord.quality, chord.family, chord.extensions, chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Should have exactly Minor7 family, got {:?}",
            chord.family
        );
        assert_eq!(
            chord.extensions.ninth, None,
            "Should not have a 9th extension (including #9), got {:?}",
            chord.extensions.ninth
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_short_duration() {
        // F#2, F#3, A3, C#4, E4 should be F#m7 even with short duration
        // This tests the case where chord might be filtered out due to duration
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        let notes = vec![
            create_midi_note(42, 0, 180), // F#2 - very short duration
            create_midi_note(54, 0, 180), // F#3
            create_midi_note(57, 0, 180), // A3
            create_midi_note(61, 0, 180), // C#4
            create_midi_note(64, 0, 180), // E4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        // Should still detect the chord even with minimum duration
        assert!(
            !chords.is_empty(),
            "Should detect F#m7 even with short duration"
        );

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Should be exactly F#m7
        assert_eq!(
            chord_name, "F#m7",
            "Should be exactly F#m7. Root pitch: {}, Quality: {:?}, Family: {:?}, Name: {}",
            chords[0].root_pitch, chord.quality, chord.family, chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Should have exactly Minor7 family, got {:?}",
            chord.family
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_different_end_times() {
        // F#2, F#3, A3, C#4, E4 should be F#m7 even when notes have slightly different end times
        // This tests the case where one note ends earlier, reducing the chord duration
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        let notes = vec![
            create_midi_note(42, 0, 180),  // F#2 - ends early
            create_midi_note(54, 0, 2880), // F#3 - longer duration
            create_midi_note(57, 0, 2880), // A3 - longer duration
            create_midi_note(61, 0, 2880), // C#4 - longer duration
            create_midi_note(64, 0, 2880), // E4 - longer duration
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        // Should still detect the chord even when one note ends early
        // The chord duration will be 180 (minimum end time), which is exactly the minimum
        assert!(
            !chords.is_empty(),
            "Should detect F#m7 even when one note ends early. Chord duration might be shorter but should still be >= 180"
        );

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Should be exactly F#m7
        assert_eq!(
            chord_name, "F#m7",
            "Should be exactly F#m7. Root pitch: {}, Quality: {:?}, Family: {:?}, Name: {}",
            chords[0].root_pitch, chord.quality, chord.family, chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Should have exactly Minor7 family, got {:?}",
            chord.family
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_staggered_start_times() {
        // F#2, F#3, A3, C#4, E4 should be F#m7 even when notes have slightly staggered start times
        // This simulates the real-world scenario where notes might not start at exactly the same time
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        // Start times: 72000, 72000, 72001, 72002, 72003 (slightly staggered)
        // End times: all 74880 (same end time)
        let notes = vec![
            create_midi_note(42, 72000, 74880), // F#2 - starts first
            create_midi_note(54, 72000, 74880), // F#3 - starts at same time
            create_midi_note(57, 72001, 74880), // A3 - starts 1 tick later
            create_midi_note(61, 72002, 74880), // C#4 - starts 2 ticks later
            create_midi_note(64, 72003, 74880), // E4 - starts 3 ticks later
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        // Should detect F#m7 even with staggered start times
        assert!(
            !chords.is_empty(),
            "Should detect F#m7 with staggered start times. Got {} chords",
            chords.len()
        );

        // Find the chord that contains F#m7
        let f_sharp_m7_chord = chords.iter().find(|c| {
            c.root_pitch == 42
                && c.chord.quality == ChordQuality::Minor
                && c.chord.family == Some(crate::chord::ChordFamily::Minor7)
        });

        assert!(
            f_sharp_m7_chord.is_some(),
            "Should detect F#m7 chord. Found chords: {:?}",
            chords
                .iter()
                .map(|c| (c.root_pitch, c.chord.to_string()))
                .collect::<Vec<_>>()
        );

        let chord = f_sharp_m7_chord.unwrap();
        let chord_name = chord.chord.to_string();

        // Should be exactly F#m7
        assert_eq!(
            chord_name, "F#m7",
            "Should be exactly F#m7. Root pitch: {}, Quality: {:?}, Family: {:?}, Name: {}",
            chord.root_pitch, chord.chord.quality, chord.chord.family, chord_name
        );
        assert_eq!(
            chord.root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chord.root_pitch
        );
        assert_eq!(
            chord.chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.chord.quality
        );
        assert_eq!(
            chord.chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Should have exactly Minor7 family, got {:?}",
            chord.chord.family
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_staggered_start_and_end_times() {
        // F#2, F#3, A3, C#4, E4 should be F#m7 even when notes have both staggered start and end times
        // This is the most realistic scenario - notes don't start or end at exactly the same time
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        // Start times: 72000, 72000, 72001, 72002, 72003 (slightly staggered)
        // End times: 74880, 74879, 74878, 74877, 74876 (slightly staggered, but all >= 180 ticks duration)
        let notes = vec![
            create_midi_note(42, 72000, 74880), // F#2 - starts first, ends last
            create_midi_note(54, 72000, 74879), // F#3 - starts same, ends 1 tick earlier
            create_midi_note(57, 72001, 74878), // A3 - starts 1 tick later, ends 2 ticks earlier
            create_midi_note(61, 72002, 74877), // C#4 - starts 2 ticks later, ends 3 ticks earlier
            create_midi_note(64, 72003, 74876), // E4 - starts 3 ticks later, ends 4 ticks earlier
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        // Should detect F#m7 even with staggered start and end times
        assert!(
            !chords.is_empty(),
            "Should detect F#m7 with staggered start and end times. Got {} chords",
            chords.len()
        );

        // Find the chord that contains F#m7
        let f_sharp_m7_chord = chords.iter().find(|c| {
            c.root_pitch == 42
                && c.chord.quality == ChordQuality::Minor
                && c.chord.family == Some(crate::chord::ChordFamily::Minor7)
        });

        assert!(
            f_sharp_m7_chord.is_some(),
            "Should detect F#m7 chord. Found chords: {:?}",
            chords
                .iter()
                .map(|c| (c.root_pitch, c.chord.to_string()))
                .collect::<Vec<_>>()
        );

        let chord = f_sharp_m7_chord.unwrap();
        let chord_name = chord.chord.to_string();

        // Should be exactly F#m7
        assert_eq!(
            chord_name, "F#m7",
            "Should be exactly F#m7. Root pitch: {}, Quality: {:?}, Family: {:?}, Name: {}",
            chord.root_pitch, chord.chord.quality, chord.chord.family, chord_name
        );
        assert_eq!(
            chord.root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chord.root_pitch
        );
        assert_eq!(
            chord.chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.chord.quality
        );
        assert_eq!(
            chord.chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Should have exactly Minor7 family, got {:?}",
            chord.chord.family
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_exact_reaper_scenario() {
        // This test simulates the exact scenario from REAPER where F#m7 is not detected
        // Based on the debug output: [F#2, F#3, A3, C#4, E4] at PPQ: 72000 should be F#m7
        // The issue might be that notes are being processed in a way that splits them
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        // Using PPQ positions similar to the REAPER output
        let notes = vec![
            create_midi_note(42, 72000, 74880), // F#2
            create_midi_note(54, 72000, 74880), // F#3
            create_midi_note(57, 72000, 74880), // A3
            create_midi_note(61, 72000, 74880), // C#4
            create_midi_note(64, 72000, 74880), // E4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        // Must detect exactly one F#m7 chord - this is a critical test
        assert!(
            !chords.is_empty(),
            "MUST detect F#m7. Got {} chords: {:?}",
            chords.len(),
            chords
                .iter()
                .map(|c| format!("{} (root: {})", c.chord, c.root_pitch))
                .collect::<Vec<_>>()
        );

        // Should have exactly one chord
        assert_eq!(
            chords.len(),
            1,
            "Should detect exactly one chord, got {}: {:?}",
            chords.len(),
            chords
                .iter()
                .map(|c| format!("{} (root: {})", c.chord, c.root_pitch))
                .collect::<Vec<_>>()
        );

        let chord = &chords[0];
        let chord_name = chord.chord.to_string();

        // Must be exactly F#m7 - no exceptions
        assert_eq!(
            chord_name,
            "F#m7",
            "MUST be exactly F#m7, not '{}'. Root pitch: {}, Quality: {:?}, Family: {:?}, Extensions: {:?}",
            chord_name,
            chord.root_pitch,
            chord.chord.quality,
            chord.chord.family,
            chord.chord.extensions
        );
        assert_eq!(
            chord.root_pitch, 42,
            "Root MUST be exactly F#2 (42), got {}",
            chord.root_pitch
        );
        assert_eq!(
            chord.chord.quality,
            ChordQuality::Minor,
            "Quality MUST be exactly Minor, got {:?}",
            chord.chord.quality
        );
        assert_eq!(
            chord.chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Family MUST be exactly Minor7, got {:?}",
            chord.chord.family
        );
        assert_eq!(
            chord.chord.extensions.ninth, None,
            "MUST NOT have 9th extension, got {:?}",
            chord.chord.extensions.ninth
        );
        assert_eq!(
            chord.start_ppq, 72000,
            "Start PPQ MUST be exactly 72000, got {}",
            chord.start_ppq
        );
        assert!(
            chord.end_ppq >= 72000 + 180,
            "End PPQ MUST be at least 180 ticks after start (>= 72180), got {}",
            chord.end_ppq
        );
    }

    #[test]
    fn test_f_sharp_minor_7th_after_previous_chord() {
        // This test simulates the exact scenario where F#m7 appears after another chord
        // Based on the debug output: F#m7 at 69120 works, but at 72000 it doesn't
        // The issue might be that notes are being split when they shouldn't be
        // First chord: F#m7 at 69120, then another F#m7 at 72000
        // F#2 = 42, F#3 = 54, A3 = 57, C#4 = 61, E4 = 64
        let notes = vec![
            // First chord group (should be detected as F#m7)
            create_midi_note(42, 69120, 72000), // F#2
            create_midi_note(54, 69120, 72000), // F#3
            create_midi_note(57, 69120, 72000), // A3
            create_midi_note(61, 69120, 72000), // C#4
            create_midi_note(64, 69120, 72000), // E4
            // Second chord group (should also be detected as F#m7)
            create_midi_note(42, 72000, 74880), // F#2
            create_midi_note(54, 72000, 74880), // F#3
            create_midi_note(57, 72000, 74880), // A3
            create_midi_note(61, 72000, 74880), // C#4
            create_midi_note(64, 72000, 74880), // E4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        // Must detect exactly two F#m7 chords
        assert!(
            !chords.is_empty(),
            "MUST detect F#m7 chords. Got {} chords",
            chords.len()
        );

        // Find all F#m7 chords
        let f_sharp_m7_chords: Vec<_> = chords
            .iter()
            .filter(|c| {
                c.root_pitch == 42
                    && c.chord.quality == ChordQuality::Minor
                    && c.chord.family == Some(crate::chord::ChordFamily::Minor7)
            })
            .collect();

        assert!(
            !f_sharp_m7_chords.is_empty(),
            "MUST detect at least one F#m7 chord. Found chords: {:?}",
            chords
                .iter()
                .map(|c| format!(
                    "{} (root: {}, start: {})",
                    c.chord, c.root_pitch, c.start_ppq
                ))
                .collect::<Vec<_>>()
        );

        // Check the second chord (at 72000) - this is the one that's failing in REAPER
        let second_chord = chords.iter().find(|c| c.start_ppq == 72000);

        assert!(
            second_chord.is_some(),
            "MUST detect F#m7 chord starting at 72000. Found chords: {:?}",
            chords
                .iter()
                .map(|c| format!(
                    "{} (root: {}, start: {})",
                    c.chord, c.root_pitch, c.start_ppq
                ))
                .collect::<Vec<_>>()
        );

        let chord = second_chord.unwrap();
        let chord_name = chord.chord.to_string();

        // Must be exactly F#m7
        assert_eq!(
            chord_name, "F#m7",
            "MUST be exactly F#m7 at 72000, not '{}'. Root pitch: {}, Quality: {:?}, Family: {:?}",
            chord_name, chord.root_pitch, chord.chord.quality, chord.chord.family
        );
        assert_eq!(
            chord.root_pitch, 42,
            "Root MUST be exactly F#2 (42) at 72000, got {}",
            chord.root_pitch
        );
        assert_eq!(
            chord.chord.quality,
            ChordQuality::Minor,
            "Quality MUST be exactly Minor at 72000, got {:?}",
            chord.chord.quality
        );
        assert_eq!(
            chord.chord.family,
            Some(crate::chord::ChordFamily::Minor7),
            "Family MUST be exactly Minor7 at 72000, got {:?}",
            chord.chord.family
        );
    }

    #[test]
    fn test_dsus2_chord_after_previous_chord() {
        // This test simulates the scenario where Dsus2 appears after another chord
        // D3 = 50, A3 = 57, D4 = 62, E4 = 64, A4 = 69
        let notes = vec![
            // First chord group (should be detected as Dsus2)
            create_midi_note(50, 80640, 83520), // D3
            create_midi_note(57, 80640, 83520), // A3
            create_midi_note(62, 80640, 83520), // D4
            create_midi_note(64, 80640, 83520), // E4
            create_midi_note(69, 80640, 83520), // A4
            // Second chord group (should also be detected as Dsus2)
            create_midi_note(50, 83520, 86400), // D3
            create_midi_note(57, 83520, 86400), // A3
            create_midi_note(62, 83520, 86400), // D4
            create_midi_note(64, 83520, 86400), // E4
            create_midi_note(69, 83520, 86400), // A4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(
            !chords.is_empty(),
            "MUST detect Dsus2 chords. Got {} chords",
            chords.len()
        );

        // Find all Dsus2 chords
        let dsus2_chords: Vec<_> = chords
            .iter()
            .filter(|c| {
                c.root_pitch == 50
                    && c.chord.quality == ChordQuality::Suspended(SuspendedType::Second)
            })
            .collect();

        assert!(
            !dsus2_chords.is_empty(),
            "MUST detect at least one Dsus2 chord. Found chords: {:?}",
            chords
                .iter()
                .map(|c| format!(
                    "{} (root: {}, start: {})",
                    c.chord, c.root_pitch, c.start_ppq
                ))
                .collect::<Vec<_>>()
        );

        // Check the second chord (at 83520)
        let second_chord = chords.iter().find(|c| c.start_ppq == 83520);

        assert!(
            second_chord.is_some(),
            "MUST detect Dsus2 chord starting at 83520. Found chords: {:?}",
            chords
                .iter()
                .map(|c| format!(
                    "{} (root: {}, start: {})",
                    c.chord, c.root_pitch, c.start_ppq
                ))
                .collect::<Vec<_>>()
        );

        let chord = second_chord.unwrap();
        let chord_name = chord.chord.to_string();

        assert_eq!(
            chord_name, "Dsus2",
            "MUST be exactly Dsus2 at 83520, not '{}'. Root pitch: {}, Quality: {:?}",
            chord_name, chord.root_pitch, chord.chord.quality
        );
        assert_eq!(chord.root_pitch, 50);
        assert_eq!(
            chord.chord.quality,
            ChordQuality::Suspended(SuspendedType::Second),
        );
        assert!(chord.chord.additions.is_empty());
    }

    #[test]
    fn test_e_add4_with_root_in_lower_octave() {
        // E2, G#3, A3, B3 should be EAdd4, not E11
        // Even though root is E2 (pitch 40) and G#3/A3 are in octave 3,
        // they're in the same octave relative to each other, so it's add4
        // E2 = 40, G#3 = 56, A3 = 57, B3 = 59
        let notes = vec![
            create_midi_note(40, 0, 5760), // E2 (root)
            create_midi_note(56, 0, 5760), // G#3 (major 3rd, compound interval 16)
            create_midi_note(57, 0, 5760), // A3 (4th, compound interval 17)
            create_midi_note(59, 0, 5760), // B3 (5th, compound interval 19)
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        // Should be E major with added 4th
        assert_eq!(chord.quality, ChordQuality::Major, "Expected Major quality");
        assert_eq!(chord.family, None, "Expected no 7th family");

        // The chord should have the 4th (A) as an addition, not an extension
        let chord_name = chord.to_string();
        let has_add4 = chord.additions.contains(&ChordDegree::Eleventh)
            || chord_name.to_lowercase().contains("add");

        assert!(
            has_add4,
            "Should have add4/add11. Additions: {:?}, Extensions: {:?}, Name: {}",
            chord.additions, chord.extensions, chord_name
        );
        assert!(
            chord.extensions.eleventh.is_none(),
            "Should not have 11th extension"
        );
    }

    #[test]
    fn test_a_slash_csharp_inversion() {
        // C#2, C#3, E3, A3 should be A/C# (A major in first inversion)
        // C# = 37, E = 52, A = 57
        let notes = vec![
            create_midi_note(37, 0, 2880), // C#2
            create_midi_note(49, 0, 2880), // C#3
            create_midi_note(52, 0, 2880), // E3
            create_midi_note(57, 0, 2880), // A3
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Detected chord: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Family: {:?}", chord.family);
        println!("Bass: {:?}", chord.bass);

        // Should be A major (not C#m#5#9b13)
        assert_eq!(
            chord.quality,
            ChordQuality::Major,
            "Should be major quality. Got: {}",
            chord_name
        );
        assert_eq!(chord.family, None, "Should not have a 7th");
        // Should have C# as bass (inversion)
        assert!(chord.bass.is_some(), "Should have bass note for inversion");

        // Should be exactly A/C# (A major in first inversion)
        let chord_name = chord.to_string();
        assert_eq!(
            chord_name, "A/C#",
            "Should be exactly A/C#, got: {}",
            chord_name
        );
        assert_eq!(
            chords[0].root_pitch, 57,
            "Root should be exactly A3 (57), got {}",
            chords[0].root_pitch
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Major,
            "Should be exactly Major quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family, None,
            "Should have exactly no 7th family, got {:?}",
            chord.family
        );
        assert!(chord.bass.is_some(), "Should have bass note for inversion");
    }

    #[test]
    fn test_no_false_slash_chords_same_pitch_class() {
        // E2, E3, G#3, A3, B3 should be Eadd11, NOT E/E
        // E = 40, G# = 56, A = 57, B = 59
        let notes = vec![
            create_midi_note(40, 0, 5760), // E2
            create_midi_note(52, 0, 5760), // E3
            create_midi_note(56, 0, 5760), // G#3
            create_midi_note(57, 0, 5760), // A3
            create_midi_note(59, 0, 5760), // B3
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        // Should be exactly Eadd4, no slash chord
        let chord_name = chord.to_string();
        assert_eq!(
            chord_name, "Eadd4",
            "Should be exactly Eadd4, got: {}",
            chord_name
        );
        assert_eq!(
            chord.bass, None,
            "Should have exactly no bass note, got {:?}",
            chord.bass
        );
        assert_eq!(
            chords[0].root_pitch, 40,
            "Root should be exactly E2 (40), got {}",
            chords[0].root_pitch
        );
    }

    #[test]
    fn test_f_sharp_minor_no_false_slash() {
        // F#2, F#3, A3, C#4 should be F#m, NOT F#m/F#
        // F# = 42, A = 57, C# = 61
        let notes = vec![
            create_midi_note(42, 0, 2880), // F#2
            create_midi_note(54, 0, 2880), // F#3
            create_midi_note(57, 0, 2880), // A3
            create_midi_note(61, 0, 2880), // C#4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();
        // Should be exactly F#m, no slash chord
        assert_eq!(
            chord_name, "F#m",
            "Should be exactly F#m, got: {}",
            chord_name
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Minor,
            "Should be exactly Minor quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.bass, None,
            "Should have exactly no bass note, got {:?}",
            chord.bass
        );
        assert_eq!(
            chords[0].root_pitch, 42,
            "Root should be exactly F#2 (42), got {}",
            chords[0].root_pitch
        );
    }

    #[test]
    fn test_d_sus2_no_false_slash() {
        // D2, D3, E3, A3 should be Dsus2, NOT Dsus2/D
        // D = 38, E = 52, A = 57
        let notes = vec![
            create_midi_note(38, 0, 5760), // D2
            create_midi_note(50, 0, 5760), // D3
            create_midi_note(52, 0, 5760), // E3
            create_midi_note(57, 0, 5760), // A3
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();
        // Root + 2nd + 5th with no 3rd = Dsus2
        assert_eq!(
            chord_name, "Dsus2",
            "Should be exactly Dsus2, got: {}",
            chord_name
        );
        assert_eq!(
            chord.quality,
            ChordQuality::Suspended(SuspendedType::Second),
            "Should be exactly Suspended(Second) quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.bass, None,
            "Should have exactly no bass note, got {:?}",
            chord.bass
        );
        assert_eq!(
            chords[0].root_pitch, 38,
            "Root should be exactly D2 (38), got {}",
            chords[0].root_pitch
        );
        assert!(
            chord.additions.is_empty(),
            "Should have no additions, got {:?}",
            chord.additions
        );
    }

    #[test]
    fn test_major_triad_root_position() {
        // C4, E4, G4 should be C major
        let notes = vec![
            create_midi_note(60, 0, 4800), // C4
            create_midi_note(64, 0, 4800), // E4
            create_midi_note(67, 0, 4800), // G4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();
        assert_eq!(chord_name, "C", "Should be exactly C, got: {}", chord_name);
        assert_eq!(
            chord.quality,
            ChordQuality::Major,
            "Should be exactly Major quality, got {:?}",
            chord.quality
        );
        assert_eq!(
            chord.family, None,
            "Should have exactly no 7th family, got {:?}",
            chord.family
        );
        assert_eq!(
            chords[0].root_pitch, 60,
            "Root should be exactly C4 (60), got {}",
            chords[0].root_pitch
        );
        assert_eq!(chords[0].root_pitch, 60); // C should be root
    }

    #[test]
    fn test_minor_triad_inversion() {
        // E3, G3, C4 should be Cm/E (C minor in first inversion)
        // But with simplicity scoring, C might be preferred as root even if E is lowest
        let notes = vec![
            create_midi_note(52, 0, 4800), // E3
            create_midi_note(55, 0, 4800), // G3
            create_midi_note(60, 0, 4800), // C4
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty());

        let chord = &chords[0].chord;
        // Should be minor (either Cm or Em)
        assert!(
            chord.quality == ChordQuality::Minor || chord.quality == ChordQuality::Major,
            "Should be minor or major, got: {:?}",
            chord.quality
        );
        // If C is root, E should be bass (inversion)
        // If E is root, it's root position
        if chords[0].root_pitch == 60 {
            // C is root - should have E as bass
            assert!(chord.bass.is_some(), "Should have bass note when C is root");
        } else {
            // E is root - no bass needed
            assert_eq!(chords[0].root_pitch, 52, "If E is root, should be 52");
        }
    }

    #[test]
    fn test_tritone_sub_db7_sharp11_over_g() {
        // Tritone substitution: Db7#11/G (or C#7#11/G)
        // G is the bass (which is also the #11 of Db)
        // G is a tritone (6 semitones) away from Db
        // G2 = 43, Db3 = 49, F3 = 53, Ab3 = 56, B3 = 59
        let notes = vec![
            create_midi_note(43, 0, 4800), // G2 (bass, also #11)
            create_midi_note(49, 0, 4800), // Db3/C#3 (root)
            create_midi_note(53, 0, 4800), // F3 (major 3rd)
            create_midi_note(56, 0, 4800), // Ab3/G#3 (perfect 5th)
            create_midi_note(59, 0, 4800), // B3/Cb4 (minor 7th)
        ];

        let chords = detect_chords_from_midi_notes(&notes, 180);
        assert!(!chords.is_empty(), "Should detect a chord");

        let chord = &chords[0].chord;
        let chord_name = chord.to_string();

        // Debug output
        println!("Tritone sub detected: {}", chord_name);
        println!("Root pitch: {}", chords[0].root_pitch);
        println!("Quality: {:?}", chord.quality);
        println!("Family: {:?}", chord.family);
        println!("Extensions: {:?}", chord.extensions);
        println!("Bass: {:?}", chord.bass);

        // The chord should be detected as either:
        // 1. Db7#11/G (preferred tritone sub notation)
        // 2. C#7#11/G (enharmonic equivalent)
        // 3. Or possibly just detected as the slash chord

        // We expect the chord to have:
        // - Dominant 7th family
        // - Major quality
        // - #11 extension OR the G in bass
        // - Bass note of G

        // Accept either Db7 or C#7 as the root (they're enharmonic)
        let root_str = chord.root.to_string();
        assert!(
            root_str == "Db" || root_str == "C#",
            "Root should be Db or C#, got: {}. Full chord: {}",
            root_str,
            chord_name
        );

        assert_eq!(
            chord.quality,
            ChordQuality::Major,
            "Should have Major quality for dominant 7. Got: {:?}",
            chord.quality
        );

        assert_eq!(
            chord.family,
            Some(crate::chord::ChordFamily::Dominant7),
            "Should have Dominant7 family. Got: {:?}",
            chord.family
        );

        // Should have bass note of G
        assert!(chord.bass.is_some(), "Should have G as bass note");

        let bass_str = chord
            .bass
            .as_ref()
            .map(|b| b.to_string())
            .unwrap_or_default();
        assert_eq!(bass_str, "G", "Bass should be G, got: {}", bass_str);
    }
}
