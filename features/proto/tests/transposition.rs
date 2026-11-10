//! Transposition and re-notation: which accidentals a key spells with,
//! enharmonics, slash basses and chromatic chords, through the public
//! `transpose_source` / `renotate_symbol` / capo helpers.

use keyflow_proto::Key;
use keyflow_proto::transpose::{
    ChartView, NotationSystem, capo_to_play, interval_semitones, renotate_symbol,
    shape_key_for_capo, transpose_source,
};

fn key(s: &str) -> Key {
    Key::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
}

/// The chord line of a one-line chart in `from`, re-spelled as letters
/// sounding in `to`.
fn transpose_line(from: &str, line: &str, to: &str) -> String {
    let source = format!("#{from} 120bpm 4/4\n{line}\n");
    let view = ChartView {
        target_key: Some(key(to)),
        notation: NotationSystem::Letters,
        capo: 0,
    };
    let out = transpose_source(&source, &view);
    out.lines()
        .nth(1)
        .unwrap_or_else(|| panic!("no chord line in {out:?}"))
        .to_string()
}

fn scale(k: &str) -> Vec<String> {
    let key = key(k);
    (1..=7)
        .map(|d| key.get_scale_degree(d).expect("degree").name)
        .collect()
}

/// A flat key spells its chords with flats and a sharp key with sharps —
/// I IV V vi in C moved to Eb, E and Db.
#[test]
fn target_key_decides_sharps_or_flats() {
    assert_eq!(transpose_line("C", "C F G Am", "Eb"), "Eb Ab Bb Cm");
    assert_eq!(transpose_line("C", "C F G Am", "E"), "E A B C#m");
    assert_eq!(transpose_line("C", "C F G Am", "Db"), "Db Gb Ab Bbm");
    assert_eq!(transpose_line("C", "C F G Am", "B"), "B E F# G#m");
}

/// Enharmonic keys spell differently: the same sounding chords read
/// `F# B C#` in F# and `Gb … Db` in Gb. Chord spelling is deliberately
/// relaxed (`KeySpelling` avoids E#, B#, Cb and Fb), so Gb's IV is `B`.
#[test]
fn enharmonic_keys_spell_their_own_way() {
    assert_eq!(transpose_line("C", "C F G", "F#"), "F# B C#");
    assert_eq!(transpose_line("C", "C F G", "Gb"), "Gb B Db");
    assert_eq!(transpose_line("C", "Am Dm Em", "Gb"), "Ebm Abm Bbm");
}

/// Seventh, extension, suspension and altered qualities survive the move.
#[test]
fn qualities_survive_transposition() {
    assert_eq!(
        transpose_line("C", "Cmaj7 Dm7 Bm7b5 G7sus4 C6 Cadd9 G7b9", "D"),
        "Dmaj7 Em7 C#m7b5 A7sus4 D6 Dadd9 A7b9"
    );
}

/// Slash basses move with the chord and are spelled for the new key.
#[test]
fn slash_basses_move_and_respell() {
    assert_eq!(
        transpose_line("C", "C/E G/B Am/G F/C", "Bb"),
        "Bb/D F/A Gm/F Eb/Bb"
    );
    assert_eq!(transpose_line("G", "G/B D/F# C/E", "A"), "A/C# E/G# D/F#");
}

/// A borrowed chord keeps its function: bVII in E (D) is bVII in F (Eb),
/// and the bVI–bVII of a minor-plagal cadence stays flat.
#[test]
fn chromatic_chords_keep_their_function() {
    assert_eq!(transpose_line("E", "E D A E", "F"), "F Eb Bb F");
    assert_eq!(transpose_line("C", "C Ab Bb C", "D"), "D Bb C D");
}

/// Minor keys transpose as minor keys.
#[test]
fn minor_keys_transpose() {
    assert_eq!(transpose_line("Am", "Am Dm E7 Am", "Cm"), "Cm Fm G7 Cm");
    assert_eq!(transpose_line("Em", "Em C D B7", "F#m"), "F#m D E C#7");
}

/// Scale spelling, which every transposition above leans on.
#[test]
fn scales_spell_one_letter_per_degree() {
    assert_eq!(scale("B"), ["B", "C#", "D#", "E", "F#", "G#", "A#"]);
    assert_eq!(scale("Db"), ["Db", "Eb", "F", "Gb", "Ab", "Bb", "C"]);
    assert_eq!(scale("Bm"), ["B", "C#", "D", "E", "F#", "G", "A"]);
    assert_eq!(scale("Cm"), ["C", "D", "Eb", "F", "G", "Ab", "Bb"]);
}

/// `Key::notes()` and `get_scale_degree` describe the same scale, so they
/// must spell it the same way.
#[test]
fn key_notes_match_scale_degrees() {
    for k in [
        "C", "G", "D", "A", "E", "B", "F#", "F", "Bb", "Eb", "Ab", "Db", "Am", "Em", "Dm",
    ] {
        let names: Vec<String> = key(k).notes().into_iter().map(|n| n.name).collect();
        assert_eq!(names, scale(k), "{k}");
    }
}

/// Numbers and letters, both ways, in a sharp key.
#[test]
fn renotate_in_a_sharp_key() {
    let e = key("E");
    let letters = |s| renotate_symbol(s, &e, NotationSystem::Letters);
    let numbers = |s| renotate_symbol(s, &e, NotationSystem::Nashville);
    let roman = |s| renotate_symbol(s, &e, NotationSystem::Roman);

    assert_eq!(letters("6m").as_deref(), Some("C#m"));
    assert_eq!(letters("3").as_deref(), Some("G#m"));
    assert_eq!(letters("5/7").as_deref(), Some("B/D#"));
    assert_eq!(letters("b7").as_deref(), Some("D"));
    assert_eq!(numbers("C#m7").as_deref(), Some("6m7"));
    assert_eq!(numbers("A/C#").as_deref(), Some("4/6"));
    assert_eq!(roman("B7").as_deref(), Some("V7"));
}

/// The shortest way round the octave, and the capo arithmetic.
#[test]
fn intervals_and_capo() {
    assert_eq!(interval_semitones(key("C"), key("G")), -5);
    assert_eq!(interval_semitones(key("G"), key("C")), 5);
    assert_eq!(interval_semitones(key("C"), key("D")), 2);
    assert_eq!(interval_semitones(key("C"), key("C")), 0);
    assert_eq!(interval_semitones(key("C"), key("F#")).abs(), 6);

    assert_eq!(capo_to_play(key("A"), key("G")), Some(2));
    assert_eq!(capo_to_play(key("Eb"), key("D")), Some(1));
    assert_eq!(capo_to_play(key("G"), key("G")), Some(0));
    assert_eq!(shape_key_for_capo(key("A"), 2), key("G"));
    assert_eq!(shape_key_for_capo(key("A"), 0), key("A"));
    // A capo at 12 is the same shapes an octave up.
    assert_eq!(shape_key_for_capo(key("A"), 12), key("A"));
    // Minor stays minor under the capo.
    assert_eq!(shape_key_for_capo(key("Bm"), 2), key("Am"));
}

/// Transposing there and back is the identity on letters.
#[test]
fn round_trip_is_identity() {
    let line = "G D/F# Em7 Cmaj7 Am7 D7sus4";
    let there = transpose_line("G", line, "Bb");
    assert_eq!(transpose_line("Bb", &there, "G"), line);
}
