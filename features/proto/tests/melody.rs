//! Melody lanes (`m{ … }`): pitches with accidentals, relative octaves and
//! octave marks, rhythmic values, and articulations, checked against how
//! the phrases actually sound.

use keyflow_proto::chart::{Melody, MelodyNote, OctaveModifier};

fn melody(src: &str) -> Melody {
    Melody::parse(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

/// Resolve octaves from middle C and spell each note as `<pitch><octave>`.
fn sounding(src: &str) -> Vec<String> {
    let mut m = melody(src);
    m.resolve_absolute_octaves();
    m.notes
        .iter()
        .filter(|n| !n.is_rest())
        .map(|n| format!("{}{}", n.pitch, n.octave.expect("resolved")))
        .collect()
}

#[test]
fn accidentals_give_the_right_pitch_class() {
    for (pitch, pc) in [
        ("C", 0),
        ("C#", 1),
        ("Db", 1),
        ("Eb", 3),
        ("E", 4),
        ("F#", 6),
        ("Gb", 6),
        ("Ab", 8),
        ("Bb", 10),
        ("B", 11),
        // Enharmonics across the octave line.
        ("Cb", 11),
        ("B#", 0),
        ("E#", 5),
        ("Fb", 4),
    ] {
        let note = MelodyNote::parse(&format!("{pitch}4")).unwrap();
        assert_eq!(note.pitch, pitch);
        assert_eq!(note.pitch_class(), Some(pc), "{pitch}");
    }
}

/// Relative mode moves to the nearest octave, so a scale walks up and
/// crosses into the next octave at C.
#[test]
fn relative_octaves_follow_the_line() {
    assert_eq!(
        sounding("C D E F G A B C"),
        ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"]
    );
    // Twinkle, twinkle: a fifth up is further than a fourth down, so the
    // leap needs its `'` (as in LilyPond's relative mode); without it the
    // line drops to the G below.
    assert_eq!(sounding("C G"), ["C4", "G3"]);
    assert_eq!(
        sounding("C C G' G A A G2"),
        ["C4", "C4", "G4", "G4", "A4", "A4", "G4"]
    );
    // Going down from C, B is the B just below.
    assert_eq!(sounding("C B A G"), ["C4", "B3", "A3", "G3"]);
}

/// `'` and `,` override the nearest octave: a leap of a sixth up from C
/// to A is nearer below, so it needs the mark.
#[test]
fn octave_marks_force_the_leap() {
    assert_eq!(sounding("C A"), ["C4", "A3"]);
    assert_eq!(sounding("C A'"), ["C4", "A4"]);
    assert_eq!(sounding("C E,"), ["C4", "E3"]);
    assert_eq!(sounding("C C''"), ["C4", "C6"]);
    let up = MelodyNote::parse("G''8").unwrap();
    assert_eq!(up.octave_modifier, OctaveModifier::UpBy(2));
}

/// An explicit `:` octave pins the note, and the line continues from it.
#[test]
fn explicit_octave_resets_the_reference() {
    assert_eq!(sounding("C:5 D E"), ["C5", "D5", "E5"]);
    assert_eq!(sounding("C:3 B"), ["C3", "B2"]);
}

/// Rhythmic values in quarter-note beats.
#[test]
fn rhythmic_values() {
    for (src, beats) in [
        ("C1", 4.0),
        ("C2", 2.0),
        ("C2.", 3.0),
        ("C4", 1.0),
        ("C4.", 1.5),
        ("C8", 0.5),
        ("C8.", 0.75),
        ("C16", 0.25),
        ("C32", 0.125),
    ] {
        let note = MelodyNote::parse(src).unwrap();
        assert!((note.duration_beats() - beats).abs() < 1e-9, "{src}");
    }
}

/// The figures that have to add up to a beat or a bar, do.
#[test]
fn figures_fill_their_beats() {
    // Dotted eighth + sixteenth: one beat.
    assert!((melody("C8. D16").total_beats() - 1.0).abs() < 1e-9);
    // Eighth-note triplet: one beat.
    assert!((melody("C8t D8t E8t").total_beats() - 1.0).abs() < 1e-9);
    // Quarter-note triplet: two beats.
    assert!((melody("C4t D4t E4t").total_beats() - 2.0).abs() < 1e-9);
    // A 4/4 bar of mixed values, with a rest.
    assert!((melody("C4. D8 r4 E8 F8").total_beats() - 4.0).abs() < 1e-9);
}

/// Each written articulation is kept by name, and a note may carry several.
#[test]
fn articulations_by_name() {
    for (src, name) in [
        ("C:44-.", "staccato"),
        ("C:44->", "accent"),
        ("C:44--", "tenuto"),
        ("C:44-^", "marcato"),
        ("C:44-!", "staccatissimo"),
        ("C:44-fermata", "fermata"),
    ] {
        let note = MelodyNote::parse(src).unwrap();
        assert_eq!(note.articulations, vec![name.to_string()], "{src}");
        assert_eq!(note.to_string(), src, "{src} prints back");
    }
    let both = MelodyNote::parse("C:44->-.").unwrap();
    assert_eq!(both.articulations, ["accent", "staccato"]);
}

/// Ties join two notes into one sound; a tie mark on a rest-free pair is
/// recorded on both sides.
#[test]
fn ties_mark_both_notes() {
    let m = melody("C2~ ~C8 D8");
    assert!(m.notes[0].tie_start && !m.notes[0].tie_stop);
    assert!(m.notes[1].tie_stop && !m.notes[1].tie_start);
    assert!(!m.notes[2].tie_start && !m.notes[2].tie_stop);
}

/// Scale degrees spell in C until a key is applied elsewhere.
#[test]
fn scale_degrees_carry_their_degree() {
    // The accidental follows the degree in a melody lane: `7b` is ♭7.
    let m = melody("1 3 5 7b");
    let pitches: Vec<&str> = m.notes.iter().map(|n| n.pitch.as_str()).collect();
    assert_eq!(pitches, ["C", "E", "G", "Bb"]);
    assert_eq!(m.notes[3].scale_degree, Some(7));
}

#[test]
fn malformed_notes_are_errors() {
    assert!(MelodyNote::parse("").is_err());
    assert!(MelodyNote::parse("H4").is_err());
    assert!(Melody::parse_block("C D E").is_err());
    assert!(Melody::parse_block("m{ C D E").is_err());
}
