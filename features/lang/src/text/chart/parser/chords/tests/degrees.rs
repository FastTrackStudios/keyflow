use super::*;

#[test]
fn numbers_take_diatonic_quality_in_major_key() {
    let q = chord_qualities("P\n4/4 120bpm #C\n\nVS\n1 2 3 4 5 6 7\n");
    assert_eq!(
        q,
        [
            "Major",
            "Minor",
            "Minor",
            "Major",
            "Major",
            "Minor",
            "Diminished"
        ]
    );
}

#[test]
fn numbers_take_diatonic_quality_in_minor_key() {
    let q = chord_qualities("P\n4/4 120bpm #Am\n\nVS\n1 2 4 5\n");
    assert_eq!(q, ["Minor", "Diminished", "Minor", "Minor"]);
}

#[test]
fn diatonic_quality_is_overridable() {
    // !2 = literal major; 2M / 2maj = explicit major; 2m = explicit minor;
    // 2:7 keeps the diatonic minor and stacks the seventh (Dm7).
    let q = chord_qualities("P\n4/4 120bpm #C\n\nVS\n!2 2M 2maj 2m 2:7\n");
    assert_eq!(q, ["Major", "Major", "Major", "Minor", "Minor"]);
}

#[test]
fn optional_colon_separates_root_and_quality() {
    assert_eq!(chord_symbols("C:7"), ["C7"]);
    assert_eq!(chord_symbols("1 4:maj9"), ["1", "4maj9"]);
    assert_eq!(chord_symbols("I:7"), ["I7"]);
}

#[test]
fn colon_and_quality_work_on_roman_numerals() {
    assert_eq!(chord_symbols("I:7 IV:maj7"), ["I7", "IVmaj7"]);
    // Lowercase Roman keeps its case (minor), with or without the colon.
    assert_eq!(chord_symbols("v:7"), ["v7"]);
    assert_eq!(chord_symbols("v7"), ["v7"]);
    // An explicit `m` is accepted and understood, but not echoed back:
    // the case already says minor, and `i7` is how the convention
    // spells a minor seventh on the tonic.
    assert_eq!(chord_symbols("i:m7"), ["i7"]);
}

#[test]
fn merged_degree_and_quality_parses_without_colon() {
    // `17` is degree-1 + a 7th (the terse, less-readable form of `1:7`).
    assert_eq!(chord_symbols("1 17"), ["1", "17"]);
    assert_eq!(chord_symbols("1 46"), ["1", "46"]);
}

#[test]
fn flat_roman_roots_parse_in_a_chart_line() {
    // `normalize_chord_case` must not uppercase the leading flat into note B.
    assert_eq!(chord_symbols("I bIII IV"), ["I", "bIII", "IV"]);
    assert_eq!(chord_symbols("i bvi V"), ["i", "bvi", "V"]);
}

#[test]
fn a_lowercase_flat_degree_is_a_flat_even_on_a_letter_line() {
    // This used to assert `["C", "F", "B7", "G"]` — a letters-majority
    // line resolved to `Letter`, and `b7` was read as the note B.
    // Lowercase `b` before a digit is a flat in every context; the note
    // is written `B`, and `B7` still parses as B dominant seven.
    assert_eq!(chord_symbols("C F b7 G"), ["C", "F", "b7", "G"]);
    assert_eq!(chord_symbols("C F b3 G"), ["C", "F", "b3", "G"]);
    assert_eq!(chord_symbols("C F B7 G"), ["C", "F", "B7", "G"]);
}

#[test]
fn contextual_b7_number_line_is_flat_degree() {
    assert_eq!(chord_symbols("1 4 b7 5"), ["1", "4", "b7", "5"]);
}

#[test]
fn contextual_b7_roman_line_is_flat_degree() {
    assert_eq!(chord_symbols("I IV b7 V"), ["I", "IV", "b7", "V"]);
}

#[test]
fn a_lone_flat_degree_is_a_flat_degree() {
    // Previously `["B7"]`: with no line context to vote on, the parser
    // fell back to reading the leading `b` as the note.
    assert_eq!(chord_symbols("b7"), ["b7"]);
    assert_eq!(chord_symbols("B7"), ["B7"]);
}

#[test]
fn flat_degrees_two_three_four_parse_in_number_context() {
    // These get stripped as sus/figured-bass figures on note B unless the
    // degree context bypasses the suffix extractors.
    assert_eq!(chord_symbols("1 b2 b3 b4 5"), ["1", "b2", "b3", "b4", "5"]);
}

#[test]
fn b9_is_always_note_b_even_in_number_context() {
    // Scale degrees only reach 7, so `b9` can only be the note B + a 9th.
    assert_eq!(chord_symbols("1 4 b9 5"), ["1", "4", "B9", "5"]);
}

#[test]
fn contextual_b7_falls_back_to_chart_when_line_is_ambiguous() {
    // A line that is only `b7` follows the chart's number system -> ♭7.
    let chart = parse_chart("P\n4/4 120bpm #C\n\nVS\n1 4 5 1\nb7\n").expect("parse chart");
    let syms: Vec<String> = chart
        .sections
        .iter()
        .flat_map(|s| s.measures().iter())
        .flat_map(|m| m.chords.iter().map(|c| c.full_symbol.clone()))
        .collect();
    assert!(
        syms.contains(&"b7".to_string()),
        "expected ♭7, got {syms:?}"
    );
}
