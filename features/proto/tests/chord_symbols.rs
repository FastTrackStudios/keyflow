//! Chord symbols, read the way a musician writes them and printed the way
//! a chart shows them — through the crate's public surface only (`Lexer`
//! → `Chord::parse` → `Display`), so a refactor behind it cannot pass by
//! accident.

use keyflow_proto::chord::{ChordFamily, ChordQuality, SuspendedType};
use keyflow_proto::{Chord, Key, Lexer};

fn chord(symbol: &str) -> Chord {
    let mut lexer = Lexer::new(symbol.to_string());
    Chord::parse(&lexer.tokenize()).unwrap_or_else(|e| panic!("{symbol}: {e:?}"))
}

fn note_names(symbol: &str) -> Vec<String> {
    chord(symbol)
        .notes(None)
        .unwrap_or_else(|| panic!("{symbol} has no notes"))
        .into_iter()
        .map(|n| n.name)
        .collect()
}

/// The everyday symbols a lead sheet carries print back as written (or as
/// the one canonical spelling the house style uses).
#[test]
fn common_symbols_print_back_as_written() {
    for (written, shown) in [
        ("C", "C"),
        ("Cm", "Cm"),
        ("C7", "C7"),
        ("Cmaj7", "Cmaj7"),
        ("Cm7", "Cm7"),
        ("Cdim", "Cdim"),
        ("Cdim7", "Cdim7"),
        ("Cm7b5", "Cm7b5"),
        ("Csus4", "Csus4"),
        ("Csus2", "Csus2"),
        ("C7sus4", "C7sus4"),
        ("C9", "C9"),
        ("Cmaj9", "Cmaj9"),
        ("Cm9", "Cm9"),
        ("C13", "C13"),
        ("C7b9", "C7b9"),
        ("C7#9", "C7#9"),
        ("C6", "C6"),
        ("Cm6", "Cm6"),
        ("C6/9", "C6/9"),
        ("Cadd9", "Cadd9"),
        ("C5", "C5"),
        ("F#m7b5", "F#m7b5"),
        ("Bbmaj7", "Bbmaj7"),
        ("Ebm", "Ebm"),
    ] {
        assert_eq!(chord(written).to_string(), shown, "{written}");
    }
}

/// Augmented is printed `+`, whichever way it was typed.
#[test]
fn augmented_is_spelled_plus() {
    for written in ["Caug", "C+"] {
        let c = chord(written);
        assert_eq!(c.quality, ChordQuality::Augmented, "{written}");
        assert_eq!(c.to_string(), "C+", "{written}");
    }
}

/// `maj7` has several spellings in the wild; all of them are one family.
/// (`CM7` is not among them here: keyflow reads a capital `M` as the
/// major-triad marker, so `6M7` is a dominant seventh on 6.)
#[test]
fn major_seventh_spellings_agree() {
    for written in ["Cmaj7", "CMaj7", "C△7"] {
        let c = chord(written);
        assert_eq!(c.quality, ChordQuality::Major, "{written}");
        assert_eq!(c.family, Some(ChordFamily::Major7), "{written}");
        assert_eq!(c.to_string(), "Cmaj7", "{written}");
        assert_eq!(c.pitch_classes(), vec![0, 4, 7, 11], "{written}");
    }
    assert_eq!(chord("C△9").to_string(), "Cmaj9");
    assert_eq!(chord("Cm△7").family, Some(ChordFamily::MinorMajor7));
}

/// `m7b5` and `ø7` are the same half-diminished chord: a diminished fifth
/// under a minor seventh, and it prints `m7b5` either way.
#[test]
fn m7b5_is_half_diminished() {
    for written in ["Bm7b5", "Bø7"] {
        let c = chord(written);
        assert_eq!(c.pitch_classes(), vec![0, 3, 6, 10], "{written}");
        assert_eq!(c.to_string(), "Bm7b5", "{written}");
    }
    assert_eq!(note_names("Bm7b5"), ["B", "D", "F", "A"]);
}

/// A bare `2` on a triad is an added second and a bare `add4` keeps its
/// spelling; `sus2` / `sus4` replace the third instead of adding to it.
#[test]
fn add_and_sus_are_different_chords() {
    assert_eq!(chord("G2").to_string(), "Gadd2");
    assert_eq!(chord("Gadd4").to_string(), "Gadd4");
    assert_eq!(chord("Gadd2").pitch_classes(), vec![0, 2, 4, 7]);
    assert_eq!(chord("Gadd4").pitch_classes(), vec![0, 4, 5, 7]);

    let sus2 = chord("Gsus2");
    assert_eq!(sus2.quality, ChordQuality::Suspended(SuspendedType::Second));
    assert_eq!(sus2.pitch_classes(), vec![0, 2, 7]);
    let sus4 = chord("Gsus4");
    assert_eq!(sus4.quality, ChordQuality::Suspended(SuspendedType::Fourth));
    assert_eq!(sus4.pitch_classes(), vec![0, 5, 7]);
}

/// The sixth chord is a triad with an added sixth and no seventh; 6/9 adds
/// the ninth on top of it.
#[test]
fn sixth_chords_carry_the_sixth_not_a_seventh() {
    assert_eq!(chord("C6").pitch_classes(), vec![0, 4, 7, 9]);
    assert!(chord("C6").family.is_none());
    assert_eq!(chord("Cm6").pitch_classes(), vec![0, 3, 7, 9]);
    assert_eq!(chord("C6/9").pitch_classes(), vec![0, 2, 4, 7, 9]);
    // 6/9 is a chord symbol, not a slash chord over a bass note 9.
    assert!(chord("C6/9").bass.is_none());
}

/// The quality is what decides the intervals: each one checked as the set
/// of semitones above the root.
#[test]
fn qualities_have_the_right_intervals() {
    for (symbol, pcs) in [
        ("C", vec![0, 4, 7]),
        ("Cm", vec![0, 3, 7]),
        ("Cdim", vec![0, 3, 6]),
        ("C+", vec![0, 4, 8]),
        ("C5", vec![0, 7]),
        ("C7", vec![0, 4, 7, 10]),
        ("Cmaj7", vec![0, 4, 7, 11]),
        ("Cm7", vec![0, 3, 7, 10]),
        ("Cdim7", vec![0, 3, 6, 9]),
        ("C9", vec![0, 2, 4, 7, 10]),
        ("C7b9", vec![0, 1, 4, 7, 10]),
        ("C7#9", vec![0, 3, 4, 7, 10]),
    ] {
        assert_eq!(chord(symbol).pitch_classes(), pcs, "{symbol}");
    }
}

/// Spelled notes follow the chord's degrees, not the nearest black key:
/// the third of Eb is G, the seventh of Db is Cb, the fifth of C+ is G#.
#[test]
fn chord_tones_are_spelled_by_degree() {
    assert_eq!(note_names("Ebmaj7"), ["Eb", "G", "Bb", "D"]);
    assert_eq!(note_names("F#m"), ["F#", "A", "C#"]);
    assert_eq!(note_names("C+"), ["C", "E", "G#"]);
    assert_eq!(note_names("Bdim7"), ["B", "D", "F", "Ab"]);
    assert_eq!(note_names("Db7"), ["Db", "F", "Ab", "Cb"]);
}

/// A slash chord keeps its upper structure and records the bass apart from
/// it; the bass is not a chord tone the quality has to explain.
#[test]
fn slash_chords_keep_quality_and_bass() {
    let c = chord("Am7/G");
    assert_eq!(c.quality, ChordQuality::Minor);
    assert_eq!(c.family, Some(ChordFamily::Minor7));
    assert_eq!(
        c.bass.as_ref().map(ToString::to_string).as_deref(),
        Some("G")
    );
    assert_eq!(c.to_string(), "Am7/G");

    let inversion = chord("C/E");
    assert_eq!(inversion.to_string(), "C/E");
    assert_eq!(inversion.pitch_classes(), vec![0, 4, 7]);

    assert_eq!(chord("D/F#").to_string(), "D/F#");
    assert_eq!(chord("Bb/D").to_string(), "Bb/D");
}

/// Number and Roman roots resolve against the key they are read in.
#[test]
fn degree_roots_resolve_in_the_key() {
    let eb = Key::parse("Eb").unwrap();
    let root = |s: &str| chord(s).root_note(Some(&eb)).unwrap().name;
    assert_eq!(root("1"), "Eb");
    assert_eq!(root("4"), "Ab");
    assert_eq!(root("5"), "Bb");
    assert_eq!(root("6m"), "C");
    assert_eq!(root("IV"), "Ab");
    assert_eq!(root("vi"), "C");
    assert_eq!(chord("vi").quality, ChordQuality::Minor);
}

/// Parsing is total over garbage: an empty or non-chord input is an error,
/// never a panic.
#[test]
fn non_chords_are_errors() {
    let mut empty = Lexer::new(String::new());
    assert!(Chord::parse(&empty.tokenize()).is_err());
    let mut word = Lexer::new("hello".to_string());
    assert!(Chord::parse(&word.tokenize()).is_err());
}
