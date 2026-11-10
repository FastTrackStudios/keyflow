use super::*;

/// Each bar's chords as `symbol@beat`, the way a chart reads.
fn bars_of(section: &crate::text::chart::ChartSection) -> Vec<String> {
    section
        .measures()
        .iter()
        .map(|m| {
            m.chords
                .iter()
                .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
                .map(|c| format!("{}@{}", c.full_symbol, c.position.total_duration.beat))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// A parenthesised addition is one chord like any other: the bare chord
/// after `5(add4) //` holds its own bar, not the two beats before it.
#[test]
fn a_parenthesised_addition_does_not_lend_its_length() {
    let chart = parse_chart("Paren\n72bpm 4/4 #Bb\n\nVS 8\n1 4 // 1 // 6m // 5(add4) // 42 x2\n")
        .expect("parses");
    assert_eq!(
        bars_of(&chart.sections[0]),
        vec![
            "1@0",
            "4@0 1@2",
            "6m@0 5add4@2",
            "4add2@0",
            "1@0",
            "4@0 1@2",
            "6m@0 5add4@2",
            "4add2@0"
        ]
    );
}

/// Rests are where they are written: six bars of N.C., then the riff.
#[test]
fn rests_at_the_start_of_a_line_stay_there() {
    let chart = parse_chart("Rests\n128bpm 4/4 #E\n\nCH 4\nr r 4 1\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[0]), vec!["", "", "4@0", "1@0"]);
}

/// Each bar's rests, as their length in beats.
fn rests_of(section: &crate::text::chart::ChartSection) -> Vec<Vec<f64>> {
    section
        .measures()
        .iter()
        .map(|m| {
            let (num, den) = m.time_signature;
            let ts = TimeSignature::new(u32::from(num), u32::from(den));
            m.rhythm_elements
                .iter()
                .filter_map(|e| match e {
                    RhythmElement::Rest(rest) => Some(rest.duration.to_beats(ts)),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

/// A bare `r` is a bar of rest, the way a bare chord is a bar of chord,
/// in whatever the bar's meter is.
#[test]
fn a_bare_rest_fills_its_bar() {
    let chart = parse_chart("Rests\n128bpm 4/4 #E\n\nCH 4\nr r 4 1\n").expect("parses");
    assert_eq!(rests_of(&chart.sections[0])[..2], [vec![4.0], vec![4.0]]);

    let chart = parse_chart("Waltz\n90bpm 3/4 #E\n\nVS 2\nr 1\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[0]), vec!["", "1@0"]);
    assert_eq!(rests_of(&chart.sections[0])[0], vec![3.0]);
}

/// Slashes after a bare `r` give it its length, as they do a chord; a
/// carried chord length does too.
#[test]
fn a_bare_rest_takes_slashes_or_the_carried_length() {
    let chart = parse_chart("Rests\n128bpm 4/4 #E\n\nVS 2\nr // 4 // 1\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[0]), vec!["4@2", "1@0"]);
    assert_eq!(rests_of(&chart.sections[0])[0], vec![2.0]);

    let chart = parse_chart("Rests\n128bpm 4/4 #E\n\nVS 2\n1_2 r 4_2 5\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[0]), vec!["1@0", "4@0 5@2"]);
    assert_eq!(rests_of(&chart.sections[0])[0], vec![2.0]);
}

/// A bare `s` is a bar of space in its written place.
#[test]
fn a_bare_space_holds_its_bar() {
    let chart = parse_chart("Spaces\n128bpm 4/4 #E\n\nCH 4\n4 s 1 5\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[0]), vec!["4@0", "", "1@0", "5@0"]);
}

/// A chart may open on a pre- or post-chorus with a bar count; the
/// header is a header there as much as anywhere else.
#[test]
fn a_chart_may_open_on_a_counted_pre_or_post() {
    for (header, want) in [
        ("PRE 2", SectionType::Pre(Box::new(SectionType::Chorus))),
        ("Post 2", SectionType::Post(Box::new(SectionType::Chorus))),
    ] {
        let chart = parse_chart(&format!("Top\n128bpm 4/4 #E\n\n{header}\n4 5\n")).expect("parses");
        assert_eq!(chart.sections[0].section.section_type, want, "{header}");
        assert_eq!(chart.sections[0].section.measure_count, Some(2), "{header}");
        assert_eq!(bars_of(&chart.sections[0]), vec!["4@0", "5@0"], "{header}");
    }
}

/// A bare header replays the section, whatever the section: a
/// pre-chorus and a post-chorus are sections like any other.
#[test]
fn a_bare_post_chorus_replays_it() {
    let chart =
        parse_chart("Song\n128bpm 4/4 #E\n\nPost 2\n1 4\nVS 2\n1 5\nPost\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[2]), vec!["1@0", "4@0"]);
}

#[test]
fn a_bare_pre_chorus_replays_it() {
    let chart =
        parse_chart("Song\n128bpm 4/4 #E\n\nPRE 2\n4 5\nCH 2\n1 1\nPRE\nCH\n").expect("parses");
    assert_eq!(bars_of(&chart.sections[2]), vec!["4@0", "5@0"]);
    assert_eq!(bars_of(&chart.sections[3]), vec!["1@0", "1@0"]);
}

/// A section written twice with different music replays the most recent
/// one: each time a section is written out, it becomes what its bare
/// header plays from then on.
#[test]
fn a_bare_header_replays_the_most_recent_writing() {
    let chart = parse_chart("Twice\n128bpm 4/4 #E\n\nCH 2\n1 4\nVS 2\n6m 5\nCH 2\n1 5\nCH\nVS\n")
        .expect("parses");
    assert_eq!(bars_of(&chart.sections[3]), vec!["1@0", "5@0"]);
    assert_eq!(bars_of(&chart.sections[4]), vec!["6m@0", "5@0"]);
}

/// Chord memory is off unless the chart turns it on: a bare `5` after
/// a `5sus` is a 5, and a `4` after a `4:6` is a 4.
#[test]
fn chord_memory_is_off_unless_the_chart_turns_it_on() {
    let symbols = |memory: &str| {
        let input = format!("Memory\n72bpm 4/4 #D\n{memory}\nIN 2\n4:6 5sus\nVS 2\n4 5\n");
        let chart = parse_chart(&input).expect("Should parse");
        chart.sections[1]
            .measures()
            .iter()
            .flat_map(|m| m.chords.iter().map(|c| c.full_symbol.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(symbols(""), vec!["4", "5"]);
    // A suspension is never remembered, memory or not.
    assert_eq!(symbols("\\chord_memory = true"), vec!["46", "5"]);
}

/// `42` is the 4 chord add2, as worship charts write it.
#[test]
fn a_number_chord_with_a_two_is_add2() {
    let chart = parse_chart("Two\n72bpm 4/4 #D\n\nVS 2\n6m7 42\n").expect("Should parse");
    let chords: Vec<_> = chart.sections[0]
        .measures()
        .iter()
        .flat_map(|m| m.chords.iter().map(|c| c.full_symbol.clone()))
        .collect();
    assert_eq!(chords, vec!["6m7", "4add2"]);
}
