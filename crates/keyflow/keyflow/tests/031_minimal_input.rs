//! The smallest thing that is still a chart.
//!
//! Keyflow's promise is that a chart is playable text you can type
//! anywhere. That only holds if the *minimum* input works: four chord
//! names and nothing else. No title, no tempo, no meter, no section
//! header — 4/4 and the key of C are assumed and you get a chart.
//!
//! These tests exist because that path has no ceremony to protect it. A
//! header line is what every fixture, every guide example and every song
//! in the corpus happens to have, so a regression here hides behind all
//! of them.

use keyflow::parse;

/// Bars of a parsed chart, flattened, as `[chords]` per bar.
fn bars(src: &str) -> Vec<String> {
    parse(src)
        .unwrap_or_else(|e| panic!("should parse:\n{src}\n\n{e}"))
        .sections
        .iter()
        .flat_map(|s| s.measures().iter())
        .map(|m| {
            m.chords
                .iter()
                .map(|c| c.full_symbol.clone())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

#[test]
fn four_chord_names_and_nothing_else_is_a_chart() {
    assert_eq!(bars("G C Em D\n"), ["G", "C", "Em", "D"]);
}

#[test]
fn a_missing_trailing_newline_is_still_a_chart() {
    // What you get from a paste, or from an editor buffer mid-keystroke.
    assert_eq!(bars("G C Em D"), ["G", "C", "Em", "D"]);
}

#[test]
fn a_single_chord_is_a_chart() {
    // The first thing anyone types. It must render, not wait for a
    // second chord or a section.
    assert_eq!(bars("G"), ["G"]);
}

#[test]
fn bare_input_assumes_four_four_and_the_key_of_c() {
    let chart = parse("G C Em D\n").expect("parse");
    let ts = chart.time_signature.as_ref().expect("an assumed meter");
    assert_eq!((ts.numerator, ts.denominator), (4, 4));

    let key = chart.initial_key.as_ref().expect("an assumed key");
    assert_eq!(key.root.name, "C");
}

#[test]
fn bare_input_needs_no_title_and_invents_none() {
    let chart = parse("G C Em D\n").expect("parse");
    assert!(chart.metadata.title.is_none(), "a bare chart has no title");
    assert!(chart.metadata.subtitle.is_none());
    assert!(chart.metadata.artist.is_none());
}

#[test]
fn bare_numbers_work_the_same_as_bare_letters() {
    // Nashville numbers resolve against the assumed key of C.
    assert_eq!(bars("1 4 5 1\n"), ["1", "4", "5", "1"]);
}

#[test]
fn multiple_bare_lines_keep_flowing() {
    assert_eq!(
        bars("G C Em D\nAm F C G\n"),
        ["G", "C", "Em", "D", "Am", "F", "C", "G"]
    );
}

// ── A section header, with no header block above it ────────────────

/// The section types a chart parsed to, with their bar counts.
fn sections(src: &str) -> Vec<(String, usize)> {
    parse(src)
        .unwrap_or_else(|e| panic!("should parse:\n{src}\n\n{e}"))
        .sections
        .iter()
        .map(|s| (format!("{:?}", s.section.section_type), s.measures().len()))
        .collect()
}

#[test]
fn a_section_header_can_be_the_very_first_line() {
    // No title, no metadata — the chart starts at the music. The header
    // must be read as a section, not as the title it happens to sit in
    // the position of.
    let chart = parse("VS 8\nG C Em D\n").expect("parse");
    assert!(chart.metadata.title.is_none(), "`VS 8` became a title");
    assert!(chart.metadata.subtitle.is_none());
    assert_eq!(sections("VS 8\nG C Em D\n"), [("Verse".to_string(), 8)]);
}

#[test]
fn a_leading_section_header_works_for_every_form() {
    // Bare name, name + length, custom bracketed section, and a section
    // that sets its own key.
    assert_eq!(sections("VS\nG C Em D\n"), [("Verse".to_string(), 4)]);
    assert_eq!(sections("CH 8\nG C Em D\n"), [("Chorus".to_string(), 8)]);
    assert_eq!(sections("Intro 2\nG C\n"), [("Intro".to_string(), 2)]);
    assert_eq!(
        sections("BR 8 #Ab\nG C Em D\n"),
        [("Bridge".to_string(), 8)]
    );
    assert_eq!(
        sections("[SOLO] 8\nG C Em D\n"),
        [("Custom(\"SOLO\")".to_string(), 8)]
    );
}

#[test]
fn a_declared_length_pads_the_section_out() {
    // `VS 8` with four chords is eight bars: four written, four carried.
    let chart = parse("VS 8\nG C Em D\n").expect("parse");
    let verse = &chart.sections[0];
    assert_eq!(verse.measures().len(), 8);
    let written = verse
        .measures()
        .iter()
        .filter(|m| m.chords.iter().any(|c| c.full_symbol != "s"))
        .count();
    assert_eq!(written, 4, "the four written bars should carry the chords");
}

#[test]
fn two_sections_need_no_header_block_either() {
    assert_eq!(
        sections("VS 8\nG C Em D\n\nCH 8\nAm F C G\n"),
        [("Verse".to_string(), 8), ("Chorus".to_string(), 8)]
    );
}

#[test]
fn typing_a_chart_never_loses_what_is_already_there() {
    // What the live editor sees: one more character on every keystroke.
    // From the moment the section length lands, every prefix must keep
    // parsing — a state that fails here is a chart that blinks out while
    // someone is mid-word.
    let target = "VS 8\nG C Em D";
    for end in target.char_indices().map(|(i, c)| i + c.len_utf8()) {
        let partial = &target[..end];
        let chart =
            parse(partial).unwrap_or_else(|e| panic!("prefix {partial:?} stopped parsing: {e}"));
        if partial.starts_with("VS 8") {
            let bars: usize = chart.sections.iter().map(|s| s.measures().len()).sum();
            assert_eq!(bars, 8, "prefix {partial:?} lost the section length");
        }
    }
}

// ── A title without a metadata line ────────────────────────────────

#[test]
fn a_title_without_a_metadata_line_does_not_eat_the_first_section() {
    // The regression this file was written for. `looks_like_*` guards
    // rejected metadata and chord content but not a SECTION HEADER, so
    // `VS` was consumed as the subtitle — and the music then landed in an
    // implicit Intro rather than the Verse the author wrote.
    //
    // Any metadata line at all masked it, which is why it survived: every
    // fixture and guide example has one.
    let chart = parse("My Song\n\nVS\nG C Em D\n").expect("parse");

    assert_eq!(chart.metadata.title.as_deref(), Some("My Song"));
    assert_eq!(
        chart.metadata.subtitle, None,
        "the section header was swallowed as a subtitle"
    );

    let section = chart
        .sections
        .iter()
        .find(|s| !s.measures().is_empty())
        .expect("a section with music");
    assert_eq!(
        format!("{:?}", section.section.section_type),
        "Verse",
        "the music landed in the wrong section"
    );
}

#[test]
fn the_same_holds_for_every_section_name() {
    for (header, want) in [("VS", "Verse"), ("CH", "Chorus"), ("BR", "Bridge")] {
        let src = format!("My Song\n\n{header}\nG C Em D\n");
        let chart = parse(&src).expect("parse");
        assert_eq!(
            chart.metadata.subtitle, None,
            "`{header}` became a subtitle"
        );
        let section = chart
            .sections
            .iter()
            .find(|s| !s.measures().is_empty())
            .expect("a section");
        assert_eq!(format!("{:?}", section.section.section_type), want);
    }
}

#[test]
fn a_real_subtitle_line_is_still_a_subtitle() {
    // The guard must not overshoot: a genuine second line that is not a
    // section header, chord content or metadata is still the subtitle.
    let chart = parse("My Song\nTranscribed By Someone\n\nVS\nG C Em D\n").expect("parse");
    assert_eq!(
        chart.metadata.subtitle.as_deref(),
        Some("Transcribed By Someone")
    );
}

#[test]
fn parenthesised_subtitles_still_work() {
    let chart = parse("Vienna (Live) - Billy Joel\n4/4\n\nVS\nG C\n").expect("parse");
    assert_eq!(chart.metadata.title.as_deref(), Some("Vienna"));
    assert_eq!(chart.metadata.subtitle.as_deref(), Some("Live"));
    assert_eq!(chart.metadata.artist.as_deref(), Some("Billy Joel"));
}
