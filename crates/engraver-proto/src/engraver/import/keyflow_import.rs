//! Import from keyflow Chart format.
//!
//! Converts keyflow's lead sheet representation (chords, sections) into
//! a renderable Score format with:
//! - Rhythm slashes for chord durations
//! - Chord symbols above the staff
//! - Section labels as rehearsal marks
//! - Key signatures from the chart's key context
//! - Proper handling of rests and spaces from rhythm_elements

use crate::engraver::model::{
    ChordSymbol, Duration, DurationKind, KeySignature, LayoutBreak, Measure as EngraverMeasure,
    MusicElement, Note, NoteHead, Octave, Part, PartId, Pitch, PitchClass, RehearsalMark,
    RehearsalMarkStyle, Rest, Score, ScoreMetadata, Stem, TimeSignature as EngraverTimeSig, Voice,
};
use crate::engraver::quantize::{QuantizeConfig, quantize_duration};
use keyflow_proto::Note as KeyflowNote;
use keyflow_proto::chart::types::{RestInstance, RhythmElement, SpaceInstance};
use keyflow_proto::core::ChordSymbol as ChordSymbolTrait;

// region:    --- From<Chart> for Score

impl From<keyflow_proto::Chart> for Score {
    fn from(chart: keyflow_proto::Chart) -> Self {
        import_chart(&chart)
    }
}

impl From<&keyflow_proto::Chart> for Score {
    fn from(chart: &keyflow_proto::Chart) -> Self {
        import_chart(chart)
    }
}

// endregion: --- From<Chart> for Score

// region:    --- Import Functions

/// Import a keyflow Chart into a Score.
///
/// This converts the lead sheet representation (chords, sections) into
/// a renderable score format with:
/// - Rhythm slashes for chord durations
/// - Chord symbols above the staff
/// - Section labels as rehearsal marks
/// - Key signatures
#[must_use]
pub fn import_chart(chart: &keyflow_proto::Chart) -> Score {
    let metadata = ScoreMetadata {
        title: chart.metadata.title.clone(),
        composer: chart.metadata.artist.clone(),
        arranger: chart.metadata.arranger.clone(),
        lyricist: chart.metadata.lyricist.clone(),
        copyright: chart.metadata.copyright.clone(),
        ..Default::default()
    };

    // Create a single part for the lead sheet
    let mut part = Part::new(PartId(1), "Lead Sheet");

    // Convert time signature
    let time_sig = chart
        .time_signature
        .as_ref()
        .map_or(EngraverTimeSig::COMMON, |ts| {
            EngraverTimeSig::new(
                u8::try_from(ts.numerator).unwrap_or(4),
                u8::try_from(ts.denominator).unwrap_or(4),
            )
        });

    // Get initial key signature
    let initial_key_sig = chart.initial_key.as_ref().map(key_to_signature);

    // Process each section
    let mut measure_num = 1u32;
    let mut is_first_measure = true;

    for (section_idx, chart_section) in chart.sections.iter().enumerate() {
        for (measure_idx, chart_measure) in chart_section.measures().iter().enumerate() {
            let is_section_start = measure_idx == 0;

            // Create measure with signatures on first measure
            let mut measure = EngraverMeasure::with_signatures(
                measure_num,
                if is_first_measure {
                    Some(time_sig)
                } else {
                    None
                },
                if is_first_measure {
                    initial_key_sig
                } else {
                    None
                },
            );

            // Add section label as rehearsal mark at the start of each section
            if is_section_start {
                let section_name = format_section_name(&chart_section.section);
                measure = measure.with_rehearsal_mark(
                    RehearsalMark::new(&section_name)
                        .with_style(RehearsalMarkStyle::Capsule)
                        .with_break(section_idx > 0), // Force break for all but first section
                );
            }

            // Convert rhythm elements (chords, rests, spaces) to voice elements
            let mut voice = Voice::new();

            // Use rhythm_elements if available, otherwise fall back to chords
            if chart_measure.has_explicit_rhythm() {
                // Process complete rhythm pattern including rests
                for element in &chart_measure.rhythm_elements {
                    match element {
                        RhythmElement::Chord(chord_instance) => {
                            // Add chord symbol
                            let chord_symbol = chord_instance_to_symbol(chord_instance);
                            voice.add(MusicElement::ChordSymbol(chord_symbol));

                            // Add rhythm slash note
                            let duration = chord_duration_to_engraver(&chord_instance.duration);
                            let note = create_rhythm_slash(duration);
                            voice.add(MusicElement::Note(note));
                        }
                        RhythmElement::Rest(rest_instance) => {
                            // Convert rest to engraver Rest
                            let rest = rest_instance_to_engraver(rest_instance);
                            voice.add(MusicElement::Rest(rest));
                        }
                        RhythmElement::Space(space_instance) => {
                            // Spaces become rhythm slashes (auto-fill behavior)
                            let note = space_instance_to_slash(space_instance);
                            voice.add(MusicElement::Note(note));
                        }
                    }
                }
            } else {
                // Legacy path: use chords field
                for chord_instance in &chart_measure.chords {
                    // Add chord symbol
                    let chord_symbol = chord_instance_to_symbol(chord_instance);
                    voice.add(MusicElement::ChordSymbol(chord_symbol));

                    // Add rhythm slash note
                    let duration = chord_duration_to_engraver(&chord_instance.duration);
                    let note = create_rhythm_slash(duration);
                    voice.add(MusicElement::Note(note));
                }
            }

            // If measure is empty, add a whole rest worth of slashes
            if chart_measure.chords.is_empty() && chart_measure.rhythm_elements.is_empty() {
                let duration = Duration::new(DurationKind::Whole);
                let note = create_rhythm_slash(duration);
                voice.add(MusicElement::Note(note));
            }

            measure.voices = vec![voice];

            // Add section break at end of section (except last section)
            if measure_idx == chart_section.measures().len() - 1
                && section_idx < chart.sections.len() - 1
            {
                measure = measure.with_layout_break(LayoutBreak::Section);
            }

            part.add_measure(measure);
            measure_num += 1;
            is_first_measure = false;
        }
    }

    Score {
        metadata,
        parts: vec![part],
        time_signature: time_sig,
        tempo_bpm: chart.tempo.as_ref().map(|t| t.bpm),
        ..Default::default()
    }
}

// endregion: --- Import Functions

// region:    --- Conversion Helpers

/// Convert a keyflow Key to an engraver KeySignature.
///
/// Maps the key root and mode to the circle of fifths position.
fn key_to_signature(key: &crate::Key) -> KeySignature {
    // Get the semitone value of the root note using the Note trait
    let root_semitone = key.root.semitone();

    // Check if the mode is minor-like (aeolian/natural minor, harmonic minor, melodic minor)
    // These modes share key signatures with their relative major (3 semitones up)
    let mode_name = key.mode.name().to_lowercase();
    let is_minor = mode_name.contains("minor")
        || mode_name.contains("aeolian")
        || mode_name.contains("dorian")
        || mode_name.contains("phrygian")
        || mode_name.contains("locrian");

    let major_root_semitone = if is_minor {
        (root_semitone + 3) % 12 // Minor to relative major
    } else {
        root_semitone
    };

    // Map major key root to fifths count
    // C=0, G=1, D=2, A=3, E=4, B=5, F#/Gb=6/-6, F=-1, Bb=-2, Eb=-3, Ab=-4, Db=-5
    let fifths = match major_root_semitone {
        0 => 0,   // C
        1 => -5,  // Db (or C#=7, but we prefer flats for ambiguous cases)
        2 => 2,   // D
        3 => -3,  // Eb
        4 => 4,   // E
        5 => -1,  // F
        6 => 6,   // F# (or Gb=-6)
        7 => 1,   // G
        8 => -4,  // Ab
        9 => 3,   // A
        10 => -2, // Bb
        11 => 5,  // B
        _ => 0,
    };

    KeySignature::new(fifths)
}

/// Format a section name for display.
fn format_section_name(section: &crate::Section) -> String {
    let base_name = section.section_type.abbreviation();

    if let Some(number) = section.number {
        if let Some(letter) = section.split_letter {
            format!("{} {}{}", base_name, number, letter)
        } else {
            format!("{} {}", base_name, number)
        }
    } else if let Some(letter) = section.split_letter {
        format!("{}{}", base_name, letter)
    } else {
        base_name
    }
}

/// Convert a ChordInstance to a ChordSymbol.
///
/// Uses the `ChordSymbol` trait from `crate::core` to extract
/// parsed chord components directly from the underlying `Chord`.
fn chord_instance_to_symbol(chord_instance: &crate::ChordInstance) -> ChordSymbol {
    let chord = &chord_instance.parsed;

    // Get full symbol from trait
    let symbol = chord.to_symbol_string();

    // Split root into note name and accidental
    let root_full = chord.root_str();
    let (root, root_accidental) = split_root_accidental(&root_full);

    // Get quality from trait (e.g., "m", "dim", "aug", "")
    let quality = chord.quality_str().to_string();

    // Get seventh/family as extension (e.g., "7", "maj7")
    // and append any other extensions (e.g., "9", "11", "13")
    let seventh = chord.seventh_str().unwrap_or("");
    let extensions = chord.extensions_str();
    let extension = if extensions.is_empty() {
        seventh.to_string()
    } else if seventh.is_empty() {
        extensions
    } else {
        format!("{}{}", seventh, extensions)
    };

    // Get alterations from trait (e.g., "b5", "#9")
    let alterations_str = chord.alterations_str();
    let alterations = if alterations_str.is_empty() {
        Vec::new()
    } else {
        // Parse individual alterations (format is like "b5#9" or "b5 #9")
        parse_alterations(&alterations_str)
    };

    // Get bass note for slash chords
    let (bass, bass_accidental) = if let Some(bass_full) = chord.bass_str() {
        let (b, ba) = split_root_accidental(&bass_full);
        (Some(b), ba)
    } else {
        (None, String::new())
    };

    ChordSymbol {
        symbol,
        root,
        root_accidental,
        quality,
        extension,
        alterations,
        bass,
        bass_accidental,
    }
}

/// Split a root note string into note name and accidental.
///
/// Examples: "C" -> ("C", ""), "F#" -> ("F", "#"), "Bb" -> ("B", "b")
fn split_root_accidental(root: &str) -> (String, String) {
    let mut chars = root.chars();
    let note = chars.next().map(|c| c.to_string()).unwrap_or_default();
    let accidental = chars.collect::<String>();
    (note, accidental)
}

/// Parse alterations string into individual alterations.
///
/// Examples: "b5" -> ["b5"], "b5#9" -> ["b5", "#9"]
fn parse_alterations(alterations: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();

    for c in alterations.chars() {
        if (c == 'b' || c == '#') && !current.is_empty() {
            result.push(current);
            current = String::new();
        }
        current.push(c);
    }

    if !current.is_empty() {
        result.push(current);
    }

    result
}

/// Create a rhythm slash note.
fn create_rhythm_slash(duration: Duration) -> Note {
    // Use B4 (middle of treble staff) for rhythm slashes
    let pitch = Pitch::new(PitchClass::B, Octave::new(4));
    let head = NoteHead::slash_for_duration(duration.kind);

    Note {
        pitch,
        duration,
        accidental: crate::engraver::model::Accidental::None,
        head,
        stem: if duration.kind == DurationKind::Whole {
            Stem::None
        } else {
            Stem::Up
        },
        tie_forward: false,
        tie_back: false,
    }
}

/// Convert a RestInstance to an engraver Rest.
///
/// Uses the same quantization system as chords to properly detect
/// triplet and tuplet durations.
fn rest_instance_to_engraver(rest: &RestInstance) -> Rest {
    let duration = chord_duration_to_engraver(&rest.duration);
    Rest { duration }
}

/// Convert a SpaceInstance to a rhythm slash note.
///
/// Spaces are "invisible placeholders" that trigger auto-fill with slashes.
/// We convert them to rhythm slashes since that's the expected visual output.
fn space_instance_to_slash(space: &SpaceInstance) -> Note {
    let duration = chord_duration_to_engraver(&space.duration);
    create_rhythm_slash(duration)
}

/// Convert a keyflow MusicalDuration to MIDI ticks.
///
/// Uses 480 PPQ standard. The keyflow MusicalDuration is in
/// measures.beats.subdivisions format where subdivisions are
/// in 1/1000 of a beat.
fn duration_to_ticks(duration: &keyflow_proto::MusicalDuration, ppq: i32) -> i32 {
    // Convert to total beats
    // subdivisions are 1/1000 of a beat (so 500 = half beat)
    let total_beats = duration.beat as f64 + (duration.subdivision as f64 / 1000.0);

    // Convert beats to ticks
    (total_beats * ppq as f64).round() as i32
}

/// Convert a keyflow duration to an engraver Duration.
///
/// Uses the quantization system to properly detect triplets and
/// other tuplet durations from MIDI tick values.
fn chord_duration_to_engraver(duration: &keyflow_proto::MusicalDuration) -> Duration {
    let config = QuantizeConfig::default();
    let ticks = duration_to_ticks(duration, config.target_ppq);

    // Use quantization to find the best matching duration
    let quantized = quantize_duration(ticks, &config);

    // Convert from notation::Duration to model::Duration
    Duration::from(quantized.to_duration())
}

// endregion: --- Conversion Helpers

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_section_name() {
        use crate::SectionType;

        let mut section = crate::Section::new(SectionType::Verse);
        section.number = Some(1);
        assert_eq!(format_section_name(&section), "VS 1");

        let mut section = crate::Section::new(SectionType::Chorus);
        section.number = Some(2);
        section.split_letter = Some('a');
        assert_eq!(format_section_name(&section), "CH 2a");
    }

    #[test]
    fn test_split_root_accidental() {
        let (root, acc) = split_root_accidental("C");
        assert_eq!(root, "C");
        assert_eq!(acc, "");

        let (root, acc) = split_root_accidental("F#");
        assert_eq!(root, "F");
        assert_eq!(acc, "#");

        let (root, acc) = split_root_accidental("Bb");
        assert_eq!(root, "B");
        assert_eq!(acc, "b");
    }

    #[test]
    fn test_parse_alterations() {
        assert_eq!(parse_alterations("b5"), vec!["b5"]);
        assert_eq!(parse_alterations("b5#9"), vec!["b5", "#9"]);
        assert_eq!(parse_alterations("#11b13"), vec!["#11", "b13"]);
        assert!(parse_alterations("").is_empty());
    }

    #[test]
    fn test_chord_duration_to_engraver() {
        // Whole note (4 beats)
        let dur = keyflow_proto::MusicalDuration::new(0, 4, 0);
        let result = chord_duration_to_engraver(&dur);
        assert_eq!(result.kind, DurationKind::Whole);

        // Half note (2 beats)
        let dur = keyflow_proto::MusicalDuration::new(0, 2, 0);
        let result = chord_duration_to_engraver(&dur);
        assert_eq!(result.kind, DurationKind::Half);

        // Quarter note (1 beat)
        let dur = keyflow_proto::MusicalDuration::new(0, 1, 0);
        let result = chord_duration_to_engraver(&dur);
        assert_eq!(result.kind, DurationKind::Quarter);
    }

    #[test]
    fn test_rest_instance_to_engraver() {
        use keyflow_proto::chord::ChordRhythm;
        use keyflow_proto::time::AbsolutePosition;

        let rest = RestInstance::new(
            ChordRhythm::Default,
            keyflow_proto::MusicalDuration::new(0, 1, 0), // quarter note
            AbsolutePosition::at_beginning(),
            "r4".to_string(),
        );

        let result = rest_instance_to_engraver(&rest);
        assert_eq!(result.duration.kind, DurationKind::Quarter);
    }

    #[test]
    fn test_space_instance_to_slash() {
        use keyflow_proto::chord::ChordRhythm;
        use keyflow_proto::time::AbsolutePosition;

        let space = SpaceInstance::new(
            ChordRhythm::Default,
            keyflow_proto::MusicalDuration::new(0, 2, 0), // half note
            AbsolutePosition::at_beginning(),
            "s2".to_string(),
        );

        let result = space_instance_to_slash(&space);
        assert_eq!(result.duration.kind, DurationKind::Half);
        // Spaces become rhythm slashes, which use B4 and slash noteheads
        assert_eq!(result.pitch.class, PitchClass::B);
    }
}
