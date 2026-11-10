use super::*;

#[test]
fn parses_repeat_barlines_and_alternate_endings() {
    let measures = parse_line("|: A | [1] D :| | [2] E |");
    let measures = measures
        .iter()
        .filter(|measure| !measure.chords.is_empty())
        .collect::<Vec<_>>();

    assert_eq!(measures.len(), 3);
    assert!(matches!(measures[0].start_repeat, RepeatMark::Forward));
    assert!(matches!(measures[1].end_repeat, RepeatMark::Backward));

    let first_ending = measures[1].volta_start.as_ref().expect("first ending");
    assert_eq!(first_ending.numbers, vec![1]);
    assert_eq!(first_ending.label, "1.");

    let second_ending = measures[2].volta_start.as_ref().expect("second ending");
    assert_eq!(second_ending.numbers, vec![2]);
    assert_eq!(second_ending.label, "2.");
}

#[test]
fn numbered_measure_check_behaves_like_barline() {
    let measures = parse_line("|{1} A |{2} D |");

    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords[0].full_symbol, "A");
    assert_eq!(measures[1].chords[0].full_symbol, "D");
}

#[test]
fn chord_length_directive_applies_to_following_chords() {
    let measures = parse_line("| \\ChordLength /. C#m B/C# |");
    let time_sig = TimeSignature::new(4, 4);

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 2);
    for chord in &measures[0].chords {
        assert!(matches!(
            chord.rhythm,
            ChordRhythm::Slashes {
                count: 1,
                dotted: true,
                tied: false
            }
        ));
        assert!((chord.duration.to_beats(time_sig) - 1.5).abs() < 0.001);
    }
}

#[test]
fn section_chord_length_directive_applies_to_content_lines() {
    let input = r#"
Chord Length Section
120bpm 4/4 #C

Intro 1
\ChordLength 4.
| C#m B/C# |
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();
    let time_sig = TimeSignature::new(4, 4);

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 2);
    for chord in &measures[0].chords {
        assert!((chord.duration.to_beats(time_sig) - 1.5).abs() < 0.001);
    }
}

#[test]
fn duration_directive_applies_to_chords_and_melody() {
    let input = r#"
Duration Section
120bpm 6/8 #C

Intro 1
\Duration 8.
<< C#m B/C# A/C# G#m7/C# ;
   m { C# D# E F# } >>
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();
    let time_sig = TimeSignature::new(6, 8);

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 4);
    for chord in &measures[0].chords {
        assert!((chord.duration.to_beats(time_sig) - 1.5).abs() < 0.001);
    }
    assert_eq!(measures[0].melodies.len(), 1);
    assert_eq!(measures[0].melodies[0].notes.len(), 4);
    for note in &measures[0].melodies[0].notes {
        assert_eq!(note.duration, 8);
        assert!(note.dotted);
    }
}

#[test]
fn global_duration_applies_to_sections_until_overridden() {
    // A top-level `\Duration` (before any section) sets a chart-wide default
    // that each section inherits; a section's own `\Duration` overrides it.
    let input = r#"
Global Duration
120bpm 4/4 #C

\Duration 2

VS 2
C G Am F

CH 2
\Duration 4
C E G Am Bm Dm F G
"#;
    let chart = parse_chart(input).expect("Should parse");

    // Verse inherits the global half-note default: 2 chords per 4/4 measure.
    let verse = chart.sections[0].measures();
    assert_eq!(verse.len(), 2);
    assert_eq!(verse[0].chords.len(), 2);

    // Chorus overrides to quarter notes: 4 chords per measure.
    let chorus = chart.sections[1].measures();
    assert_eq!(chorus.len(), 2);
    assert_eq!(chorus[0].chords.len(), 4);
}

#[test]
fn inline_t_prefix_time_signature_change() {
    // `T2/4` changes meter mid-line; the leading `T` distinguishes it from a
    // number/slash chord like `4/6`.
    let input = r#"
Meter Change
120bpm 4/4 #C

VS 3
C T2/4 Am T4/4 G
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 3);
    assert_eq!(measures[0].time_signature, (4, 4));
    assert_eq!(measures[1].time_signature, (2, 4));
    assert_eq!(measures[2].time_signature, (4, 4));
}

#[test]
fn a_oneshot_meter_alone_on_its_line_is_a_bar_of_that_meter() {
    // A structure-only chart's odd bar: `!T2/4` with no chord after it.
    let chart = parse_chart("Song\n72bpm 4/4 #D\n\nCH 2\nBreakdown 1\n!T2/4\nVS 2\n")
        .expect("Should parse");
    let meters = |i: usize| -> Vec<(u8, u8)> {
        chart.sections[i]
            .measures()
            .iter()
            .map(|m| m.time_signature)
            .collect()
    };
    assert_eq!(meters(0), vec![(4, 4), (4, 4)]);
    assert_eq!(meters(1), vec![(2, 4)], "one bar of 2/4");
    assert_eq!(meters(2), vec![(4, 4), (4, 4)], "and back to 4/4 after it");
}

#[test]
fn a_persistent_meter_alone_on_its_line_adds_no_bar() {
    // `T3/4` switches the meter for what follows; it is not a bar itself.
    let chart = parse_chart("Song\n120bpm 4/4 #C\n\nVS 2\nT3/4\nC G\n").expect("Should parse");
    let m = chart.sections[0].measures();
    assert_eq!(m.len(), 2);
    assert!(
        m.iter().all(|m| m.time_signature == (3, 4)),
        "{:?}",
        m.iter().map(|m| m.time_signature).collect::<Vec<_>>()
    );
}

#[test]
fn oneshot_t_prefix_reverts_after_one_measure() {
    // `!T2/4` applies 2/4 to exactly the next measure, then reverts to 4/4.
    // Contrast with persistent `T2/4`, which would keep 2/4 for the rest.
    let input = r#"
One Shot
120bpm 4/4 #C

VS 4
C !T2/4 Am C G
"#;
    let chart = parse_chart(input).expect("Should parse");
    let m = chart.sections[0].measures();
    assert_eq!(m.len(), 4);
    assert_eq!(m[0].time_signature, (4, 4));
    assert_eq!(m[1].time_signature, (2, 4)); // the one-shot measure
    assert_eq!(m[2].time_signature, (4, 4)); // reverted
    assert_eq!(m[3].time_signature, (4, 4));

    // Persistent form keeps 2/4 from the change onward.
    let persistent =
        parse_chart("Persist\n120bpm 4/4 #C\n\nVS 4\nC T2/4 Am C G\n").expect("Should parse");
    let p = persistent.sections[0].measures();
    assert_eq!(p[1].time_signature, (2, 4));
    assert_eq!(p[2].time_signature, (2, 4));
    assert_eq!(p[3].time_signature, (2, 4));
}

#[test]
fn explicit_chord_duration_sticks_to_following_chords() {
    let input = r#"
Sticky Duration Section
120bpm 6/8 #C

Intro 1
F#m7_8. G#m7 Amaj7 B
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();
    let time_sig = TimeSignature::new(6, 8);

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 4);
    for chord in &measures[0].chords {
        assert!((chord.duration.to_beats(time_sig) - 1.5).abs() < 0.001);
    }
}
