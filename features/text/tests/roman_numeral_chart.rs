//! A chart written in Roman numerals needs no section marker to be read.
//!
//! Numerals are the one chord shape that collides with English — `I` opens a
//! valid chord line and a song title equally well — and neither of the two
//! `looks_like_chord_content` heuristics (there is one in `metadata.rs` and a
//! second in `sections.rs`) had a numeral case at all. So `I ii iii IV` on its
//! own was read as the TITLE: the chart engraved with a title bar and no
//! chords in it. Adding a section marker was the only way to write one, which
//! is why the guide's numeral examples all carried a `VS` they did not need.

use keyflow_text::chart::parse_chart;

/// The chord symbols of the first section, in order.
fn symbols(src: &str) -> Vec<String> {
    let chart = parse_chart(src).expect("chart should parse");
    chart.sections[0]
        .measures()
        .iter()
        .flat_map(|m| m.chords.iter())
        .map(|c| c.full_symbol.clone())
        .collect()
}

#[test]
fn a_bare_numeral_line_is_chords_not_a_title() {
    assert_eq!(symbols("I ii iii IV"), ["I", "ii", "iii", "IV"]);
}

#[test]
fn case_is_preserved_because_the_case_is_the_quality() {
    // `ii` is the minor two. Upper- and lowercase must not collapse together.
    assert_eq!(symbols("i ii iii iv"), ["i", "ii", "iii", "iv"]);
    assert_eq!(symbols("I II III IV"), ["I", "II", "III", "IV"]);
}

#[test]
fn a_flat_degree_survives_the_metadata_guard() {
    // The `sections.rs` heuristic rejects anything starting with `b` as
    // metadata, so the numeral check has to run ahead of it.
    assert_eq!(symbols("bVII IV I V"), ["bVII", "IV", "I", "V"]);
}

#[test]
fn an_english_title_that_opens_with_i_is_still_a_title() {
    // The whole reason the numeral check reads every token on the line rather
    // than just the first one.
    let chart = parse_chart("I Will Always Love You").expect("chart should parse");
    assert_eq!(
        chart.metadata.title.as_deref(),
        Some("I Will Always Love You"),
        "an English sentence opening with `I` must not become chords"
    );
}

#[test]
fn a_numeral_line_still_works_under_a_section_marker() {
    assert_eq!(symbols("VS\nI ii iii IV"), ["I", "ii", "iii", "IV"]);
}
