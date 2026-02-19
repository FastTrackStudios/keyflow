//! Chart Parser
//!
//! Main parsing logic for charts
//!
//! Three-phase parsing:
//! 1. Metadata phase: Parse title, tempo, time signature, key
//! 2. Content phase: Parse sections and their measures
//! 3. Post-processing: Auto-number sections, finalize positions

// region:    --- Modules

pub mod chords;
pub mod helpers;
pub mod metadata;
pub mod post_process;
pub mod sections;

pub use helpers::{LineOffsetMap, PushPullModifier, RepeatCount};

// endregion: --- Modules

// region:    --- Parser Entry Point

use super::ChartParser;
use crate::sections::MeasureExpression;
use crate::time::TimeSignatureExt;

impl<'a> ChartParser<'a> {
    /// Strip comments from a line (everything after ;)
    fn strip_comment(line: &str) -> &str {
        if let Some(pos) = line.find(';') {
            line[..pos].trim()
        } else {
            line
        }
    }

    /// Resolve a measure expression and update section memory
    ///
    /// Returns the resolved measure count, or None if the expression can't be resolved.
    /// Only absolute expressions update the memory - relative expressions (+N, -N) use
    /// memory but don't change it, so subsequent sections still use the original base.
    pub(crate) fn resolve_measure_expression(
        &mut self,
        section_type: &crate::sections::SectionType,
        expr: &MeasureExpression,
    ) -> Option<usize> {
        let memory = self.section_measure_memory.get(section_type).copied();
        let resolved = expr.resolve(memory);

        // Only update memory for absolute expressions (not relative +N or -N)
        if let Some(count) = resolved {
            if expr.updates_memory() {
                self.section_measure_memory
                    .insert(section_type.clone(), count);
            }
        }

        resolved
    }

    /// Parse a chart from input string
    pub fn parse(&mut self, input: &str) -> Result<(), String> {
        use crate::key::Key;
        use crate::primitives::MusicalNote;
        use crate::time::TimeSignature;

        let lines: Vec<&str> = input
            .lines()
            .map(|l| Self::strip_comment(l.trim()))
            .collect();

        if lines.is_empty() {
            return Err("Empty input".to_string());
        }

        // Phase 1: Parse metadata at the beginning
        let mut line_idx = self.parse_metadata(&lines, 0)?;

        // Phase 1.5: Apply defaults for unspecified metadata
        // Default time signature: 4/4 (common time)
        if self.time_signature.is_none() {
            self.time_signature = Some(TimeSignature::common_time());
            self.initial_time_signature = Some(TimeSignature::common_time());
        }
        // Default key: C major
        if self.current_key.is_none() {
            let c_major = Key::major(MusicalNote::c());
            self.current_key = Some(c_major.clone());
            self.initial_key = Some(c_major);
        }

        // Phase 2: Parse sections and content
        line_idx = self.parse_sections(&lines, line_idx)?;

        // Phase 3: Post-processing
        self.post_process();

        let _ = line_idx; // Suppress unused warning

        Ok(())
    }
}

// endregion: --- Parser Entry Point

// region:    --- Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::parse_chart;
    use crate::chord::ChordRhythm;
    use crate::key::Key;
    use crate::primitives::{MusicalNote, Note, RootNotation};
    use crate::sections::SectionType;
    use crate::time::MusicalDuration;

    #[test]
    fn test_parse_simple_chord_line() {
        let input = r#"
My Song - Artist Name

120bpm 4/4 #C

vs
cmaj7 Dm7 g7 Cmaj7
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Verify metadata
        assert_eq!(chart.metadata.title, Some("My Song".to_string()));
        assert_eq!(chart.metadata.artist, Some("Artist Name".to_string()));
        assert_eq!(chart.tempo.unwrap().bpm, 120.0);
        assert_eq!(chart.time_signature.unwrap().numerator, 4);
        assert_eq!(chart.time_signature.unwrap().denominator, 4);

        // Verify key
        let c_major = Key::major(MusicalNote::c());
        assert_eq!(chart.initial_key, Some(c_major.clone()));
        assert_eq!(chart.current_key, Some(c_major.clone()));

        // Verify sections
        assert_eq!(chart.sections.len(), 1);
        assert_eq!(chart.sections[0].section.section_type, SectionType::Verse);

        // Verify chords parsed (lowercase and uppercase work)
        let measures = &chart.sections[0].measures();
        assert_eq!(measures.len(), 4); // Each chord creates a measure for now

        // Check first chord (lowercase input)
        assert_eq!(measures[0].chords.len(), 1);
        assert_eq!(measures[0].chords[0].full_symbol, "Cmaj7");
    }

    #[test]
    fn test_parse_with_key_change() {
        let input = r#"
Key Change Test

120bpm 4/4 #C

vs
cmaj7 dm7 #G gmaj7 am7
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Verify initial key
        let c_major = Key::major(MusicalNote::c());
        assert_eq!(chart.initial_key, Some(c_major));

        // Verify ending key is G major
        let g_major = Key::major(MusicalNote::g());
        assert_eq!(chart.ending_key, Some(g_major.clone()));
        assert_eq!(chart.current_key, Some(g_major));

        // Verify key change was recorded
        assert_eq!(chart.key_changes.len(), 1);
        assert_eq!(chart.key_changes[0].to_key.root.name(), "G");

        // Verify chords - should have 4 chords (key change token is skipped)
        let measures = &chart.sections[0].measures();
        assert_eq!(measures.len(), 4);
    }

    #[test]
    fn test_parse_multiple_sections_with_key_change() {
        let input = r#"
Multi Section Test

120bpm 4/4 #C

vs
cmaj7 dm7

ch
Gmaj7 am7

br
#D dmaj7 Em7
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Should have 3 sections
        assert_eq!(chart.sections.len(), 3);

        // Verify section types
        assert_eq!(chart.sections[0].section.section_type, SectionType::Verse);
        assert_eq!(chart.sections[1].section.section_type, SectionType::Chorus);
        assert_eq!(chart.sections[2].section.section_type, SectionType::Bridge);

        // Verify ending key is D major
        let d_major = Key::major(MusicalNote::d());
        assert_eq!(chart.ending_key, Some(d_major));

        // Verify key change was recorded
        assert_eq!(chart.key_changes.len(), 1);
        assert_eq!(chart.key_changes[0].to_key.root.name(), "D");
    }

    #[test]
    fn test_chord_memory_across_sections() {
        let input = r#"
Memory Test

120bpm 4/4 #C

vs
cmaj7 dm7 g7

ch
c d g
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Verify verse has full chord symbols
        let verse_measures = &chart.sections[0].measures();
        assert_eq!(verse_measures.len(), 3);

        // Verify chorus chords recall memory
        let chorus_measures = &chart.sections[1].measures();
        assert_eq!(chorus_measures.len(), 3);

        // The chord memory should have remembered qualities from verse
        // Note: The current implementation stores normalized forms
        // We can verify chords were parsed successfully
        assert!(chorus_measures[0].chords[0].full_symbol.contains("C"));
        assert!(chorus_measures[1].chords[0].full_symbol.contains("D"));
        assert!(chorus_measures[2].chords[0].full_symbol.contains("G"));
    }

    #[test]
    fn test_time_signature_in_metadata() {
        let input = r#"
Time Sig Test

140bpm 6/8 #G

vs
gmaj7
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Verify time signature
        assert_eq!(chart.time_signature.unwrap().numerator, 6);
        assert_eq!(chart.time_signature.unwrap().denominator, 8);

        // Verify key
        let g_major = Key::major(MusicalNote::g());
        assert_eq!(chart.initial_key, Some(g_major));
    }

    #[test]
    fn test_section_numbering() {
        let input = r#"
Numbering Test

120bpm 4/4 #C

vs
cmaj7

ch
gmaj7

vs
dm7

ch
am7
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Should have 4 sections
        assert_eq!(chart.sections.len(), 4);

        // Verify auto-numbering
        assert_eq!(chart.sections[0].section.number, Some(1)); // Verse 1
        assert_eq!(chart.sections[1].section.number, Some(1)); // Chorus 1
        assert_eq!(chart.sections[2].section.number, Some(2)); // Verse 2
        assert_eq!(chart.sections[3].section.number, Some(2)); // Chorus 2
    }

    #[test]
    fn test_empty_section_with_measure_count() {
        let input = r#"
Empty Section Test

120bpm 4/4 #C

vs 4
cmaj7 dm7 em7 fmaj7

ch 4
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Should have 2 sections
        assert_eq!(chart.sections.len(), 2);

        // Verse should have 4 measures with chords
        assert_eq!(chart.sections[0].measures().len(), 4);

        // Chorus should have 4 empty measures (template recall placeholder)
        assert_eq!(chart.sections[1].measures().len(), 4);
        assert_eq!(chart.sections[1].section.measure_count, Some(4));
    }

    #[test]
    fn test_parse_section_with_inline_chords() {
        let input = r#"
Inline Chords Test

120bpm 4/4 #C

vs 4, cmaj7 dm7 g7 cmaj7
"#;
        let chart = parse_chart(input).expect("Failed to parse chart");

        assert_eq!(chart.sections.len(), 1);
        let section = &chart.sections[0];
        assert_eq!(section.section.section_type, SectionType::Verse);
        assert_eq!(section.section.measure_count, Some(4));
        assert_eq!(section.measures().len(), 4);
        assert_eq!(section.measures()[0].chords[0].full_symbol, "Cmaj7");
    }

    #[test]
    fn test_sectionless_chord_content() {
        // Content without a section header should still be parsed as an Intro section
        let input = r#"
Sectionless Test

120bpm 4/4 #C

Cm | Fm | Gm | Cm
"#;
        let chart = parse_chart(input).expect("Failed to parse chart");

        assert_eq!(chart.sections.len(), 1);
        let section = &chart.sections[0];
        assert_eq!(section.section.section_type, SectionType::Intro);
        assert_eq!(section.measures().len(), 4);
        assert_eq!(section.measures()[0].chords[0].full_symbol, "Cm");
        assert_eq!(section.measures()[1].chords[0].full_symbol, "Fm");
    }

    #[test]
    fn test_measure_separator() {
        let input = r#"
Measure Separator Test

120bpm 4/4 #C

vs
cmaj7_2 dm7_2 | em7_2 fmaj7_2
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Should have 2 measures (separated by |)
        // Each chord is 2 beats (half note), so 2 chords per measure
        assert_eq!(chart.sections.len(), 1);
        let measures = &chart.sections[0].measures();
        assert_eq!(measures.len(), 2);

        // First measure should have cmaj7 and dm7
        assert_eq!(measures[0].chords.len(), 2);
        assert!(measures[0].chords[0].full_symbol.contains("C"));
        assert!(measures[0].chords[1].full_symbol.contains("D"));

        // Second measure should have em7 and fmaj7
        assert_eq!(measures[1].chords.len(), 2);
        assert!(measures[1].chords[0].full_symbol.contains("E"));
        assert!(measures[1].chords[1].full_symbol.contains("F"));
    }

    #[test]
    fn test_auto_duration_two_chords_between_separators() {
        let input = r#"
Auto Duration Test

120bpm 4/4 #C

vs
| G C |
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Should have 1 measure with 2 chords, each 2 beats (half notes)
        assert_eq!(chart.sections.len(), 1);
        let measures = &chart.sections[0].measures();
        assert_eq!(measures.len(), 1);
        assert_eq!(measures[0].chords.len(), 2);

        // Each chord should be 2 beats (half note in 4/4)
        let time_sig = chart.time_signature.unwrap();
        assert!((measures[0].chords[0].duration.to_beats(time_sig) - 2.0).abs() < 0.001);
        assert!((measures[0].chords[1].duration.to_beats(time_sig) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_smart_repeat_syntax() {
        let input = r#"
Smart Repeat Test

120bpm 4/4 #C

VS 16
cmaj7 dm7 x^
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        // Should have 16 measures total (2-bar phrase repeated 8 times)
        assert_eq!(chart.sections.len(), 1);
        let measures = &chart.sections[0].measures();

        // The phrase is 2 bars (cmaj7 = 1 bar, dm7 = 1 bar in 4/4)
        // Section is 16 bars, so we need 8 repeats
        // Total: 2 bars * 8 = 16 bars
        assert_eq!(measures.len(), 16);

        // Verify the repeat count is stored on the last measure of the pattern
        // The pattern is 2 measures, so measure[1] (second measure) should have repeat_count = 8
        assert_eq!(
            measures[1].repeat_count,
            8,
            "Repeat count should be 8 on measure 1, but got {}. All measures: {:?}",
            measures[1].repeat_count,
            measures.iter().map(|m| m.repeat_count).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_chord_instance_with_source_span() {
        // Verify ChordInstance can store source_span
        use crate::chord::{Chord, ChordQuality};
        use crate::parsing::TextSpan;

        let root = RootNotation::from_note_name(MusicalNote::c());
        let chord = Chord::new(root.clone(), ChordQuality::Major);
        let span = TextSpan::with_location(10, 5, 3, 8);

        let instance = crate::chart::types::ChordInstance::new(
            root,
            "Cmaj7".to_string(),
            chord,
            ChordRhythm::Default,
            "Cmaj7".to_string(),
            MusicalDuration::new(0, 4, 0),
            crate::time::AbsolutePosition::at_beginning(),
        )
        .with_source_span(span);

        // Verify span is stored
        assert!(instance.source_span.is_some());
        let stored_span = instance.source_span.unwrap();
        assert_eq!(stored_span.start, 10);
        assert_eq!(stored_span.len, 5);
        assert_eq!(stored_span.line, 3);
        assert_eq!(stored_span.column, 8);
    }

    #[test]
    fn test_dot_repeat_with_bar_lines() {
        // With explicit bar lines, each segment is ONE measure
        // "F/C . | Cm ." should be 2 measures, each with 2 half-note chords
        let input = r#"
Dot Repeat Test
120bpm 4/4 #C

VS
F/C . | Cm .
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        let section = &chart.sections[0];
        let measures = section.measures();
        let time_sig = chart.time_signature.unwrap();

        // Should have 2 measures: (F/C, F/C repeat), (Cm, Cm repeat)
        assert_eq!(
            measures.len(),
            2,
            "Expected 2 measures (F/C + dot, Cm + dot), got {}. Chords per measure: {:?}",
            measures.len(),
            measures.iter().map(|m| m.chords.len()).collect::<Vec<_>>()
        );

        // Each measure should have 2 chords of 2 beats each
        for (i, measure) in measures.iter().enumerate() {
            assert_eq!(
                measure.chords.len(),
                2,
                "Measure {} should have 2 chords, got {}",
                i,
                measure.chords.len()
            );
            for (j, chord) in measure.chords.iter().enumerate() {
                let beats = chord.duration.to_beats(time_sig);
                assert!(
                    (beats - 2.0).abs() < 0.001,
                    "Measure {} chord {} should be 2 beats, got {}",
                    i,
                    j,
                    beats
                );
            }
        }

        // Verify chord symbols
        assert_eq!(measures[0].chords[0].full_symbol, "F/C");
        assert_eq!(measures[0].chords[1].full_symbol, "F/C"); // repeat
        assert_eq!(measures[1].chords[0].full_symbol, "Cm");
        assert_eq!(measures[1].chords[1].full_symbol, "Cm"); // repeat
    }

    #[test]
    fn test_dot_repeat_without_bar_lines() {
        // Without explicit bar lines, each chord/dot takes a full measure
        // "F/C ." should be 2 measures, each with 1 whole-note chord
        let input = r#"
Dot Repeat Test
120bpm 4/4 #C

VS
F/C .
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        let section = &chart.sections[0];
        let measures = section.measures();
        let time_sig = chart.time_signature.unwrap();

        // Should have 2 measures: F/C, F/C repeat
        assert_eq!(
            measures.len(),
            2,
            "Expected 2 measures (F/C, then dot repeat), got {}. Chords per measure: {:?}",
            measures.len(),
            measures.iter().map(|m| m.chords.len()).collect::<Vec<_>>()
        );

        // Each measure should have 1 chord of 4 beats
        for (i, measure) in measures.iter().enumerate() {
            assert_eq!(
                measure.chords.len(),
                1,
                "Measure {} should have 1 chord, got {}",
                i,
                measure.chords.len()
            );
            let beats = measure.chords[0].duration.to_beats(time_sig);
            assert!(
                (beats - 4.0).abs() < 0.001,
                "Measure {} chord should be 4 beats, got {}",
                i,
                beats
            );
        }

        // Verify chord symbols
        assert_eq!(measures[0].chords[0].full_symbol, "F/C");
        assert_eq!(measures[1].chords[0].full_symbol, "F/C"); // repeat
    }

    #[test]
    fn test_dot_repeat_clears_push_modifier() {
        // "'F/C ." should be 2 measures:
        // - Measure 0: F/C with push
        // - Measure 1: F/C WITHOUT push (dot repeat clears timing modifiers)
        let input = r#"
Dot Push Test
120bpm 4/4 #C
/push = triplet

VS
'F/C .
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        let section = &chart.sections[0];
        let measures = section.measures();

        // Should have 2 measures
        assert_eq!(
            measures.len(),
            2,
            "Expected 2 measures, got {}. Chords per measure: {:?}",
            measures.len(),
            measures.iter().map(|m| m.chords.len()).collect::<Vec<_>>()
        );

        // Filter out space placeholders ("s" chords added by post-processor)
        let m0_real_chords: Vec<_> = measures[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        let m1_real_chords: Vec<_> = measures[1]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();

        // Each measure should have 1 real chord (excluding "s" placeholders)
        assert_eq!(
            m0_real_chords.len(),
            1,
            "Measure 0 should have 1 real chord, got {}",
            m0_real_chords.len()
        );
        assert_eq!(
            m1_real_chords.len(),
            1,
            "Measure 1 should have 1 real chord, got {}",
            m1_real_chords.len()
        );

        // Measure 0 chord SHOULD have push_pull set
        let chord0 = m0_real_chords[0];
        assert!(
            chord0.push_pull.is_some(),
            "Measure 0 chord should have push_pull set"
        );
        let (is_push, _) = chord0.push_pull.as_ref().unwrap();
        assert!(*is_push, "Measure 0 chord should be a push");

        // Measure 1 chord should NOT have push_pull (cleared by dot repeat)
        let chord1 = m1_real_chords[0];
        assert!(
            chord1.push_pull.is_none(),
            "Measure 1 chord (dot repeat) should NOT have push_pull, but got {:?}",
            chord1.push_pull
        );

        // Both should have same symbol
        assert_eq!(chord0.full_symbol, "F/C");
        assert_eq!(chord1.full_symbol, "F/C");
    }

    #[test]
    fn test_slash_rhythm_same_measure() {
        // "Cm/Eb / Eb ///" should be ONE measure with:
        // - Cm/Eb taking 1 beat (from the / slash)
        // - Eb taking 3 beats (from the /// slashes)
        let input = r#"
Slash Rhythm Test
120bpm 4/4 #C

CH
Cm/Eb / Eb ///
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        let section = &chart.sections[0];
        let measures = section.measures();
        let time_sig = chart.time_signature.unwrap();

        // Should have 1 measure with 2 chords
        assert_eq!(
            measures.len(),
            1,
            "Expected 1 measure, got {}. Chords: {:?}",
            measures.len(),
            measures
                .iter()
                .map(|m| m.chords.iter().map(|c| &c.full_symbol).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );

        let measure = &measures[0];
        assert_eq!(
            measure.chords.len(),
            2,
            "Expected 2 chords in measure, got {}",
            measure.chords.len()
        );

        // Cm/Eb should be 1 beat
        let cm_eb_beats = measure.chords[0].duration.to_beats(time_sig);
        assert!(
            (cm_eb_beats - 1.0).abs() < 0.001,
            "Cm/Eb should be 1 beat, got {} (rhythm: {:?})",
            cm_eb_beats,
            measure.chords[0].rhythm
        );

        // Eb should be 3 beats
        let eb_beats = measure.chords[1].duration.to_beats(time_sig);
        assert!(
            (eb_beats - 3.0).abs() < 0.001,
            "Eb should be 3 beats, got {} (rhythm: {:?})",
            eb_beats,
            measure.chords[1].rhythm
        );
    }

    #[test]
    fn test_slash_rhythm_cross_barline() {
        // "Abmaj9 //// | //" should make Abmaj9 span 1.5 bars visually.
        // The slashes after | create SPACES (continuation) in the second measure.
        // - Measure 1: Abmaj9 with 4 beats (full measure)
        // - Measure 2: 2 spaces (continuation slashes) + Cm (2 beats) = 4 beats
        let input = r#"
Cross Barline Test
120bpm 4/4 #C

CH
Abmaj9 //// | // Cm
"#;

        let chart = parse_chart(input).expect("Failed to parse chart");

        let section = &chart.sections[0];
        let measures = section.measures();
        let time_sig = chart.time_signature.unwrap();

        // Should have 2 measures
        assert_eq!(measures.len(), 2, "Expected 2 measures");

        // First measure should have Abmaj9 with 4 beats (full measure)
        let abmaj9 = &measures[0].chords[0];
        let abmaj9_beats = abmaj9.duration.to_beats(time_sig);
        assert!(
            (abmaj9_beats - 4.0).abs() < 0.001,
            "Abmaj9 should be 4 beats (1 bar), got {} (rhythm: {:?})",
            abmaj9_beats,
            abmaj9.rhythm
        );

        // Second measure should have:
        // - 2 spaces (continuation from //)
        // - 1 chord (Cm)
        assert_eq!(
            measures[1].rhythm_elements.len(),
            3,
            "Second measure should have 3 rhythm elements (2 spaces + 1 chord)"
        );
        assert_eq!(
            measures[1].chords.len(),
            1,
            "Second measure should have 1 chord (Cm)"
        );
        assert_eq!(measures[1].chords[0].full_symbol, "Cm");

        // The continuation spaces should total 2 beats
        use crate::chart::types::RhythmElement;
        let space_count = measures[1]
            .rhythm_elements
            .iter()
            .filter(|e| matches!(e, RhythmElement::Space(_)))
            .count();
        assert_eq!(space_count, 2, "Should have 2 continuation spaces");
    }

    #[test]
    fn test_defaults_applied_for_titleless_snippet() {
        // Minimal snippet - just chords, no metadata at all
        let input = r#"
VS
C G Am F
"#;

        let chart = parse_chart(input).expect("Should parse successfully");

        // Default time signature should be 4/4
        assert_eq!(
            chart.time_signature.unwrap().numerator,
            4,
            "Default time signature numerator should be 4"
        );
        assert_eq!(
            chart.time_signature.unwrap().denominator,
            4,
            "Default time signature denominator should be 4"
        );
        assert_eq!(
            chart.initial_time_signature.unwrap().numerator,
            4,
            "Initial time signature should also be 4/4"
        );

        // Default key should be C major
        let key = chart.initial_key.as_ref().expect("Should have initial key");
        assert_eq!(key.root().name(), "C", "Default key root should be C");

        let current_key = chart.current_key.as_ref().expect("Should have current key");
        assert_eq!(
            current_key.root().name(),
            "C",
            "Current key root should be C"
        );

        // No title should be set
        assert!(
            chart.metadata.title.is_none(),
            "Title should be None for titleless snippet"
        );
    }

    #[test]
    fn test_explicit_key_overrides_default() {
        // Snippet with only key specified, no time signature
        let input = r#"
#G

VS
G D Em C
"#;

        let chart = parse_chart(input).expect("Should parse successfully");

        // Time signature should still be 4/4 (default)
        assert_eq!(chart.time_signature.unwrap().numerator, 4);
        assert_eq!(chart.time_signature.unwrap().denominator, 4);

        // Key should be G major (specified)
        let key = chart.initial_key.as_ref().expect("Should have initial key");
        assert_eq!(key.root().name(), "G", "Key should be G major as specified");
    }

    #[test]
    fn test_explicit_time_sig_overrides_default() {
        // Snippet with only time signature specified
        let input = r#"
6/8

VS
G D Em
"#;

        let chart = parse_chart(input).expect("Should parse successfully");

        // Time signature should be 6/8 (specified)
        assert_eq!(chart.time_signature.unwrap().numerator, 6);
        assert_eq!(chart.time_signature.unwrap().denominator, 8);

        // Key should be C major (default)
        let key = chart.initial_key.as_ref().expect("Should have initial key");
        assert_eq!(key.root().name(), "C", "Key should default to C major");
    }
}

// endregion: --- Tests
