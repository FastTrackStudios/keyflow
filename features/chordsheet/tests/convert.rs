//! chordsheet.com source → keyflow `Chart` → `.kf`.
//!
//! The fixtures are the site's own published manual examples (`manual-*.txt`),
//! so what these assert is the documented language rather than one author's
//! habits. The corpus test next door covers the habits — see `corpus.rs`.

use std::path::PathBuf;

use keyflow_chordsheet::ast::{Item, LineKind};
use keyflow_chordsheet::{import_str, parse, to_chart, ImportOptions};
use keyflow_proto::chart::notations::RepeatMark;
use keyflow_proto::chart::Chart;
use keyflow_proto::sections::SectionType;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn to_kf(chart: &Chart) -> String {
    keyflow_text::chart::exporter::chart_to_keyflow(chart)
}

/// The exported `.kf` has to survive keyflow's own parser. Everything else in
/// this file is detail; this is the contract.
fn assert_round_trips(chart: &Chart) -> Chart {
    let text = to_kf(chart);
    match keyflow_text::chart::parse_chart(&text) {
        Ok(reparsed) => {
            assert_eq!(
                reparsed.sections.len(),
                chart.sections.len(),
                "section count changed on re-parse:\n{text}"
            );
            reparsed
        }
        Err(e) => panic!("exported keyflow did not parse: {e}\n{text}"),
    }
}

#[test]
fn a_bar_a_chord_and_a_section() {
    let chart = import_str("=VERSE\nG C Em D\n", &ImportOptions::default());

    assert_eq!(chart.sections.len(), 1);
    assert_eq!(chart.sections[0].section.section_type, SectionType::Verse);
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 4);
    assert_eq!(measures[0].chords[0].full_symbol, "G");
    assert_eq!(measures[3].chords[0].full_symbol, "D");

    assert_round_trips(&chart);
}

/// A split bar divides evenly, which is what the site draws — and what the
/// exported `.kf` has to say, or the bar re-parses as two.
#[test]
fn a_split_bar_divides_the_measure() {
    let chart = import_str("=A\nDm7_G7 Cmaj7\n", &ImportOptions::default());
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords.len(), 2);
    assert_eq!(measures[1].chords.len(), 1);

    let text = to_kf(&chart);
    assert!(text.contains("Dm7_2 G7_2"), "{text}");
    assert_round_trips(&chart);
}

/// Eight chords crammed into one bar is ordinary chordsheet.com — the *Africa*
/// intro riff is written exactly this way. They have to come out as eighths,
/// not as eight quarter-note bars.
#[test]
fn a_crowded_bar_subdivides_rather_than_overflowing() {
    let chart = import_str("=INTRO\nA,A,A,A,A,G#,C#C#\n", &ImportOptions::default());
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 1, "eight chords, still one bar");
    assert_eq!(measures[0].chords.len(), 8);

    let reparsed = assert_round_trips(&chart);
    assert_eq!(
        reparsed.sections[0].measures().len(),
        1,
        "the exported bar must re-parse as one measure, not two"
    );
}

#[test]
fn a_bare_repeat_becomes_repeat_marks_and_a_counted_one_is_written_out() {
    let bare = import_str("=A\n(A B C D)\n", &ImportOptions::default());
    let measures = bare.sections[0].measures();
    assert_eq!(measures.len(), 4);
    assert_eq!(measures[0].start_repeat, RepeatMark::Forward);
    assert_eq!(measures[3].end_repeat, RepeatMark::Backward);

    let counted = import_str("=A\n(A B C D)x3\n", &ImportOptions::default());
    assert_eq!(
        counted.sections[0].measures().len(),
        12,
        "x3 plays the four-bar phrase three times"
    );
    assert_eq!(counted.sections[0].measures()[3].repeat_count, 3);
}

/// `%` and `%N` have no keyflow counterpart, so they become the bars they
/// stand for. The shorthand is lost; the music is not.
#[test]
fn simile_and_reprint_become_the_bars_they_stand_for() {
    let chart = import_str("=A\nA B % %1\n", &ImportOptions::default());
    let symbols: Vec<&str> = chart.sections[0]
        .measures()
        .iter()
        .map(|m| m.chords[0].full_symbol.as_str())
        .collect();
    assert_eq!(symbols, ["A", "B", "B", "B"]);

    let four = import_str("=A\nA B C D\n%4\n", &ImportOptions::default());
    assert_eq!(four.sections[0].measures().len(), 8);
}

/// `= CHORUS` with nothing under it is how the site says "the chorus again".
#[test]
fn an_empty_section_recalls_the_last_one_of_its_kind() {
    let chart = import_str(
        "=CHORUS\nC G Am F\n=VERSE\nC C G G\n=CHORUS\n",
        &ImportOptions::default(),
    );
    assert_eq!(chart.sections.len(), 3);
    assert!(chart.sections[2].from_template);
    let recalled: Vec<&str> = chart.sections[2]
        .measures()
        .iter()
        .map(|m| m.chords[0].full_symbol.as_str())
        .collect();
    assert_eq!(recalled, ["C", "G", "Am", "F"]);
}

/// A form-only chart — headers and nothing else — is a real way people use the
/// site, and the form is the whole content.
#[test]
fn a_form_only_chart_keeps_its_sections() {
    let chart = import_str(
        "= Intro\n= Verse\n= Chorus\n= Verse\n= Chorus\n= Outro\n",
        &ImportOptions::from_stem("Green Day - American Idiot"),
    );
    assert_eq!(chart.sections.len(), 6);
    assert_eq!(chart.metadata.artist.as_deref(), Some("Green Day"));
    assert_round_trips(&chart);
}

/// The source carries no meter, so a chart that isn't in 4/4 opens with a
/// `6:8` on its first bar line. That has to reach the chart's header, or the
/// exported `.kf` says 4/4 and every bar in it is read wrong.
#[test]
fn the_opening_meter_becomes_the_charts_meter() {
    let chart = import_str("=INTRO\n6:8 Ab Bbm Eb\n", &ImportOptions::default());
    let ts = chart.initial_time_signature.expect("meter");
    assert_eq!((ts.numerator, ts.denominator), (6, 8));
    assert_eq!(chart.sections[0].measures()[0].time_signature, (6, 8));
    assert_round_trips(&chart);
}

#[test]
fn annotations_and_text_lines_become_staff_text() {
    let chart = import_str(
        "=INTRO\n-\"The song starts on the & of 3\"\nA D\"Ring out\"\n",
        &ImportOptions::default(),
    );
    let measures = chart.sections[0].measures();
    assert_eq!(
        measures[0].staff_text[0].text, "The song starts on the & of 3",
        "the author's own quotes are not part of the cue"
    );
    assert_eq!(measures[1].staff_text[0].text, "Ring out");
}

#[test]
fn endings_become_voltas() {
    let chart = import_str("=A\n(A B 1. C D 2. E F)\n", &ImportOptions::default());
    let measures = chart.sections[0].measures();
    let voltas: Vec<(usize, Vec<u8>)> = measures
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.volta_start.as_ref().map(|v| (i, v.numbers.clone())))
        .collect();
    assert_eq!(voltas, vec![(2, vec![1]), (4, vec![2])]);
}

#[test]
fn a_per_bar_meter_change_applies_from_that_bar_on() {
    let chart = import_str(
        &fixture("manual-time-signatures.txt"),
        &ImportOptions::default(),
    );
    let measures = chart.sections[0].measures();
    let meters: Vec<(u8, u8)> = measures.iter().map(|m| m.time_signature).collect();
    assert_eq!(
        meters,
        vec![
            (2, 4),
            (3, 4),
            (4, 4),
            (5, 4),
            (6, 4),
            (7, 4),
            (8, 4),
            (6, 8),
            (7, 8),
            (9, 8),
            (10, 8),
            (11, 8),
        ]
    );
}

/// The site's own DaCapo example (*Comfortably Numb*). Navigation marks have
/// no home in keyflow's model, so they ride along as boxed staff text rather
/// than being dropped.
#[test]
fn the_manual_dacapo_example_keeps_its_navigation_marks() {
    let source = fixture("manual-dacapo.txt");
    let doc = parse(&source);

    let markers: Vec<String> = doc
        .lines
        .iter()
        .flat_map(|line| match &line.kind {
            LineKind::Bars(items) => items.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .filter_map(|item| match item {
            Item::Navigation { marker, .. } => Some(marker.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        markers,
        ["Segno", "Coda", "D.S. al coda con rep.", "Coda"],
        "segno, coda from, dal segno, coda to"
    );

    let chart = to_chart(&doc, &ImportOptions::default());
    let all_text: Vec<&str> = chart
        .sections
        .iter()
        .flat_map(|s| s.measures())
        .flat_map(|m| m.staff_text.iter())
        .map(|t| t.text.as_str())
        .collect();
    assert!(all_text.contains(&"D.S. al coda con rep."), "{all_text:?}");
    assert_round_trips(&chart);
}

/// The site's "All Meta Characters" example. If this converts and round-trips,
/// the documented language is covered end to end.
#[test]
fn the_manual_meta_example_converts_and_round_trips() {
    let chart = import_str(&fixture("manual-meta.txt"), &ImportOptions::default());
    // `= A`, `= B`, and the `+ A second Page` shorthand, which is a page
    // break *and* a section title.
    assert_eq!(chart.sections.len(), 3);
    assert_round_trips(&chart);
}
