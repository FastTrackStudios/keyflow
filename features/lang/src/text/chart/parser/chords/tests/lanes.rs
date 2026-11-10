use super::*;

#[test]
fn let_aliases_expand_for_chord_attached_and_standalone_text() {
    let input = r##"
Let Alias Section
120bpm 4/4 #C

let fb = ^"4-3 2-1"
let #fb = ^"#4-3 2-1"

Intro 2
A<#fb>
<fb> /.
"##;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();

    assert_eq!(measures[0].chords[0].full_symbol, "A");
    assert_eq!(measures[0].figured_bass.len(), 1);
    assert_eq!(measures[0].figured_bass[0].rows.len(), 2);
    assert_eq!(measures[0].figured_bass[0].rows[0].accidental, "#");

    assert_eq!(measures[1].figured_bass.len(), 0);
    assert_eq!(measures[1].staff_text.len(), 1);
    assert_eq!(measures[1].staff_text[0].text, "4-3 2-1");
    assert_eq!(measures[1].staff_text[0].placement, Placement::Above);
    assert_eq!(measures[1].staff_text[0].beat, 1);
}

#[test]
fn let_block_alias_can_invoke_parallel_container() {
    let input = r#"
Let Block Section
120bpm 6/8 #E

let openingHits = {
  <<
\ChordLength 8. F#m7 G#m7 Amaj7 B ;
m { <F# 'C#>8. <G# 'D#>8. <A 'E>8. <B 'F#>8. }
  >>
}

Opening 1
<openingHits>
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].chords.len(), 4);
    assert_eq!(measures[0].chords[0].full_symbol, "F#m7");
    assert_eq!(measures[0].chords[2].full_symbol, "Amaj7");
    assert_eq!(measures[0].melodies.len(), 1);
    assert_eq!(measures[0].melodies[0].notes.len(), 4);
}

#[test]
fn sectioned_lane_aliases_can_merge_as_whole_chart_parallel() {
    let input = r#"
Lane Chart
120bpm 6/8 #E

let chords = {
  intro 2
  C#m
  F#m7_8. G#m7 Amaj7 B
}

let melody = {
  intro 2
  \octave 3 m { C#2. }
  \octave 2 m { <F# 'C#>8. <G# 'D#> <A 'E> <B 'F#> }
}

<< <chords> ; <melody> >>
"#;

    let chart = parse_chart(input).expect("Should parse sectioned lanes");
    assert_eq!(chart.sections.len(), 1);
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords[0].full_symbol, "C#m");
    assert_eq!(measures[0].melodies.len(), 1);
    assert_eq!(measures[1].chords.len(), 4);
    assert_eq!(measures[1].chords[2].full_symbol, "Amaj7");
    assert_eq!(measures[1].melodies[0].notes.len(), 4);
}

#[test]
fn inline_dynamic_does_not_inflate_measure_count() {
    // `dyn <level>` inside a bar is a zero-duration annotation; it must not
    // be counted as a chord (which would steal beats and split the bar).
    // Regression for the MusicXML round-trip exporter, which emits dynamics
    // inline like `| dyn mp C#m //. |`.
    let input = "T\n8th=168bpm 6/8 #E\n\nvs 1\n| dyn mp C#m //. |\n";
    let chart = parse_chart(input).expect("inline dyn should parse");
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 1, "dyn must not add a measure");
    assert_eq!(measures[0].chords[0].full_symbol, "C#m");
    assert_eq!(measures[0].classical_dynamics.len(), 1);
    assert_eq!(measures[0].classical_dynamics[0].level, DynamicLevel::Mp);
}

#[test]
fn inline_hairpin_does_not_inflate_measure_count() {
    let input = "T\n8th=168bpm 6/8 #E\n\nvs 1\n| hairpin < 2..4 C#m //. |\n";
    let chart = parse_chart(input).expect("inline hairpin should parse");
    let measures = chart.sections[0].measures();
    assert_eq!(measures.len(), 1, "hairpin must not add a measure");
    assert_eq!(measures[0].hairpins.len(), 1);
}

#[test]
fn melody_octave_memory_carries_across_sections() {
    let input = r#"
Octave Carry
120bpm 6/8 #E

intro 1
\octave 4
m { C#8. D# E F# }

vs 1
m { <,,F# 'C#>8. <G# 'D#> <A 'E> <B 'F#> }
"#;

    let chart = parse_chart(input).expect("Should parse");
    let intro = chart.sections[0].measures();
    assert_eq!(intro[0].melodies[0].notes[3].pitch, "F#");
    assert_eq!(intro[0].melodies[0].notes[3].octave, Some(4));

    let verse = chart.sections[1].measures();
    let first_hit = &verse[0].melodies[0].notes[0];
    assert_eq!(first_hit.pitch, "F#");
    assert_eq!(first_hit.octave, Some(2));
    assert_eq!(first_hit.extra_pitches[0], ("C#".to_string(), Some(4)));
}

#[test]
fn reserved_melody_lane_accepts_bare_melody_lines() {
    let input = r#"
Melody Lane
120bpm 6/8 #E

intro 2

let chords = {
  intro
  C#m
  F#m7_8. G#m7 Amaj7 B
}

let melody = {
  intro
  \octave 4
  C#2.
  <,,F# 'C#>8. <G# 'D#> <A 'E> <B 'F#>
}

<< <chords> ; <melody> >>
"#;

    let chart = parse_chart(input).expect("Should parse reserved melody lane");
    let measures = chart.sections[0].measures();
    assert_eq!(measures[0].chords[0].full_symbol, "C#m");
    assert_eq!(measures[0].melodies[0].notes[0].pitch, "C#");
    assert_eq!(measures[0].melodies[0].notes[0].octave, Some(4));
    assert_eq!(measures[1].chords.len(), 4);
    assert_eq!(measures[1].melodies[0].notes[0].pitch, "F#");
    assert_eq!(measures[1].melodies[0].notes[0].octave, Some(2));
}

#[test]
fn parses_classical_dynamics_and_hairpins() {
    let input = r#"
Dynamics
120bpm 6/8 #E

intro 1
hairpin < 2..4
dyn fff@4
C#m
"#;

    let chart = parse_chart(input).expect("Should parse dynamics and hairpins");
    let measure = &chart.sections[0].measures()[0];
    assert_eq!(measure.classical_dynamics.len(), 1);
    assert_eq!(measure.classical_dynamics[0].level, DynamicLevel::Fff);
    assert_eq!(measure.classical_dynamics[0].beat, 4);
    assert_eq!(measure.hairpins.len(), 1);
    assert_eq!(measure.hairpins[0].kind, HairpinKind::Crescendo);
    assert_eq!(measure.hairpins[0].start_beat, 2);
    assert_eq!(measure.hairpins[0].end_beat, 4);
}

#[test]
fn a_hairpin_can_end_bars_later() {
    let chart =
        parse_chart("H\n120bpm 4/4 #G\n\nvs 4\nhairpin < 3..+2:2\nG\nC\nD\nhairpin > 1..+1\nG\n")
            .expect("parses");
    let measures = chart.sections[0].measures();
    let pin = &measures[0].hairpins[0];
    assert_eq!(
        (pin.start_beat, pin.end_measure_offset, pin.end_beat),
        (3, 2, 2)
    );
    let pin = &measures[3].hairpins[0];
    assert_eq!(pin.kind, HairpinKind::Decrescendo);
    assert_eq!(pin.end_measure_offset, 1);
    assert_eq!(pin.end_beat, u8::MAX, "to the end of the bar");
}

#[test]
fn melody_variable_dollar_recall_attaches_stored_riff() {
    // Pre-load a melody variable directly, then call parse_chord_line so we
    // exercise the recall path independently of section / inline-content
    // dispatch quirks.
    use crate::text::chart::Melody;
    let mut chart = crate::text::chart::Chart::new();
    let melody = Melody::parse("C_8 D_8 E_4").expect("parse melody");
    chart.melody_variables.set("mainRiff".to_string(), melody);

    let mut parser = ChartParser::new(&mut chart);
    let measures = parser
        .parse_chord_line("| 1 $mainRiff |", &SectionType::Verse, Some(1))
        .expect("parse_chord_line failed");

    let total_melodies: usize = measures.iter().map(|m| m.melodies.len()).sum();
    assert!(
        total_melodies > 0,
        "$mainRiff should attach a melody (got 0 across {} measures)",
        measures.len()
    );
}

#[test]
fn section_template_recall_replays_prior_chorus() {
    // CH 4 defines a 4-measure progression. A later bare `CH` recalls it.
    let input = "\
CH 4: 1 4 5 1
VS 4: 6 5 4 1
CH 4:
";
    let chart = parse_chart(input).expect("parse_chart should succeed");
    let chs: Vec<_> = chart
        .sections
        .iter()
        .filter(|s| matches!(s.section.section_type, SectionType::Chorus))
        .collect();
    assert_eq!(chs.len(), 2, "expected two CH sections");
    let recalled = chs[1];
    assert!(
        !recalled.measures().is_empty(),
        "second CH should be filled by template recall, got {} measures",
        recalled.measures().len()
    );
}

#[test]
fn parallel_container_token_spans_resolve_to_original_line() {
    // Two branches in a single parallel measure; verify each chord's span
    // points at the right slice of the original input.
    let line = "<< C7 ; F7 >>";
    let measures = parse_line(line);
    let spans = collect_chord_spans(&measures);
    // Find C7 and F7 spans.
    let c_span = spans
        .iter()
        .find(|(t, _)| t == "C7")
        .and_then(|(_, s)| *s)
        .expect("C7 span");
    let f_span = spans
        .iter()
        .find(|(t, _)| t == "F7")
        .and_then(|(_, s)| *s)
        .expect("F7 span");
    assert_eq!(&line[c_span.as_range()], "C7");
    assert_eq!(&line[f_span.as_range()], "F7");
}

#[test]
fn back_to_back_blocks_and_leading_text_parse() {
    // Each of these used to recurse until the stack overflowed.
    let two = parse_line("<< A ; m{ a8 b8 r4 r2 } >> << A ; m{ a8 b8 r4 r2 } >>");
    assert_eq!(two.len(), 2);
    let noted = parse_line("^\"Band In\" << A ; m{ a8 b8 r4 r2 } >>");
    assert_eq!(noted.len(), 1);
    assert_eq!(
        Chart::separate_parallel_blocks("^\"Keys\" << A ; m{ a4 } >> << B ; m{ b4 } >>"),
        "<< ^\"Keys\" A ; m{ a4 } >> | << B ; m{ b4 } >>"
    );
}

#[test]
fn parallel_container_token_spans_across_multiple_measures() {
    // Plain measure followed by a parallel-container measure.
    let line = "Am | << Dm7 ; G7 >> | Cmaj7";
    let measures = parse_line(line);
    let spans = collect_chord_spans(&measures);
    for (token, span) in &spans {
        let span = span.expect("every chord token should have a span");
        assert_eq!(
            &line[span.as_range()],
            token,
            "span for {} resolved to {}",
            token,
            &line[span.as_range()]
        );
    }
}
