use super::*;

#[test]
fn group_splits_a_bar_equally() {
    let ts = TimeSignature::common_time();
    let measures = parse_line("(C G)");
    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 2);
    assert_eq!(measures[0].chords[0].full_symbol, "C");
    assert_eq!(measures[0].chords[0].duration.to_beats(ts), 2.0);
    assert_eq!(measures[0].chords[1].full_symbol, "G");
    assert_eq!(measures[0].chords[1].duration.to_beats(ts), 2.0);
}

#[test]
fn group_of_three_is_a_whole_measure_triplet() {
    let ts = TimeSignature::common_time();
    let measures = parse_line("(D Em G)");
    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 3);
    for chord in &measures[0].chords {
        // Duration is stored at milli-beat resolution, so 4/3 lands on 1.333.
        assert!((chord.duration.to_beats(ts) - 4.0 / 3.0).abs() < 1e-3);
        // The notation carries a triplet so the engraver brackets it.
        match &chord.rhythm {
            ChordRhythm::Explicit(nd) => assert!(
                nd.tuplet.is_some(),
                "expected a tuplet on a triplet group member"
            ),
            other => panic!("expected Explicit rhythm, got {other:?}"),
        }
    }
}

#[test]
fn group_is_one_element_among_bare_chords() {
    let ts = TimeSignature::common_time();
    let measures = parse_line_n("G C (Em D) G", 4);
    assert_eq!(measures.len(), 4);
    // The three bare chords each own a whole bar.
    assert_eq!(measures[0].chords[0].full_symbol, "G");
    assert_eq!(measures[0].chords[0].duration.to_beats(ts), 4.0);
    assert_eq!(measures[1].chords[0].full_symbol, "C");
    // The grouped bar holds both chords, half each.
    assert_eq!(measures[2].chords.len(), 2);
    assert_eq!(measures[2].chords[0].full_symbol, "Em");
    assert_eq!(measures[2].chords[0].duration.to_beats(ts), 2.0);
    assert_eq!(measures[2].chords[1].full_symbol, "D");
    assert_eq!(measures[2].chords[1].duration.to_beats(ts), 2.0);
    assert_eq!(measures[3].chords[0].full_symbol, "G");
}

#[test]
fn group_slash_target_sets_total_beats() {
    let ts = TimeSignature::common_time();
    // `(D Em)//` → two beats split one each.
    let measures = parse_line("(D Em)//");
    assert_eq!(measures[0].chords.len(), 2);
    assert_eq!(measures[0].chords[0].duration.to_beats(ts), 1.0);
    assert_eq!(measures[0].chords[1].duration.to_beats(ts), 1.0);
}

// --- `|` bar lines ----------------------------------------------------

#[test]
fn chords_inside_one_bar_split_it_evenly() {
    let ts = TimeSignature::common_time();
    // `| G C Em D |` → one bar, four chords, one beat each in 4/4.
    let measures = parse_line("| G C Em D |");
    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 4);
    for (chord, name) in measures[0].chords.iter().zip(["G", "C", "Em", "D"]) {
        assert_eq!(chord.full_symbol, name);
        assert_eq!(chord.duration.to_beats(ts), 1.0);
    }
}

#[test]
fn bar_line_per_chord_is_one_chord_per_bar() {
    let ts = TimeSignature::common_time();
    // A leading `|` before each chord opens a fresh bar; each chord fills it.
    // Works with or without spaces around the bars.
    for line in ["|G |C |Em |D", "|G|C|Em|D"] {
        let measures = parse_line_n(line, 4);
        assert_eq!(measures.len(), 4, "line: {line}");
        for (m, name) in measures.iter().zip(["G", "C", "Em", "D"]) {
            assert_eq!(m.chords.len(), 1, "line: {line}");
            assert_eq!(m.chords[0].full_symbol, name);
            assert_eq!(m.chords[0].duration.to_beats(ts), 4.0);
        }
    }
}

#[test]
fn bars_split_independently() {
    let ts = TimeSignature::common_time();
    // `| G C | Em D | F |` → two half-bar pairs, then a whole-bar F.
    let measures = parse_line_n("| G C | Em D | F |", 3);
    assert_eq!(measures.len(), 3);
    assert_eq!(measures[0].chords[0].duration.to_beats(ts), 2.0);
    assert_eq!(measures[0].chords[1].duration.to_beats(ts), 2.0);
    assert_eq!(measures[1].chords[0].full_symbol, "Em");
    assert_eq!(measures[1].chords[1].duration.to_beats(ts), 2.0);
    assert_eq!(measures[2].chords[0].full_symbol, "F");
    assert_eq!(measures[2].chords[0].duration.to_beats(ts), 4.0);
}
