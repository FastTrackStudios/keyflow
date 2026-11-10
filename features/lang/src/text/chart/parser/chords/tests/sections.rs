use super::*;

#[test]
fn rhythm_slash_overrides_duration_default() {
    // Under `\Duration 2` (half notes) a trailing rhythm slash sets that
    // chord's length explicitly, ignoring the default. From "Life Giving
    // Water": `E B/D# A/C# E/B / /G# / A B E B4` must tile into four 4/4
    // bars, with `E/B` (one beat, via `/`) and the floating `/G#` → `E/G#`
    // (one beat) both landing in measure 2.
    let measures = parse_line("\\Duration 2 E B/D# A/C# E/B / /G# / A B E B4");
    assert_eq!(measures.len(), 4);
    let ts = TimeSignature::common_time();

    // m0: E, B/D# — the default half-note duration applies (2 beats each).
    assert_eq!(measures[0].chords[0].full_symbol, "E");
    assert_eq!(measures[0].chords[0].duration.to_beats(ts), 2.0);
    assert_eq!(measures[0].chords[1].full_symbol, "B/D#");
    assert_eq!(measures[0].chords[1].duration.to_beats(ts), 2.0);

    // m1: A/C# (2) + E/B (1, slash overrides default) + E/G# (1, floating
    // slash-bass + slash). E/G# is in measure 2, not spilled forward.
    assert_eq!(measures[1].chords.len(), 3);
    assert_eq!(measures[1].chords[0].full_symbol, "A/C#");
    assert_eq!(measures[1].chords[0].duration.to_beats(ts), 2.0);
    assert_eq!(measures[1].chords[1].full_symbol, "E/B");
    assert_eq!(measures[1].chords[1].duration.to_beats(ts), 1.0);
    assert_eq!(measures[1].chords[2].full_symbol, "E/G#");
    assert_eq!(
        measures[1].chords[2].display_override.as_deref(),
        Some("/G#")
    );
    assert_eq!(measures[1].chords[2].duration.to_beats(ts), 1.0);

    // m3 ends on E + B4 (B carrying an attached suspension figure "4").
    assert_eq!(measures[3].chords[1].full_symbol, "B");
    assert_eq!(measures[3].suspensions[0].figure, "4");
}

#[test]
fn section_header_trailing_key_change_is_recognized_and_applied() {
    // `BR 8 #G` (key change on the header) must still register as a Bridge
    // section — without stripping the `#G`, the header is unrecognized and
    // its content merges into the previous section. The key change also
    // applies entering the bridge.
    let input = "Song\n120 BPM #E 4/4\n\nVS 1\nE\n\nBR 1 #G\nG\n";
    let chart = parse_chart(input).expect("parse");
    assert_eq!(chart.sections.len(), 2, "bridge must be its own section");
    assert_eq!(chart.sections[1].section.section_type, SectionType::Bridge);
    assert!(
        chart
            .key_changes
            .iter()
            .any(|kc| format!("{}", kc.to_key).contains('G')),
        "expected a key change to G entering the bridge, got {:?}",
        chart.key_changes
    );
}

#[test]
fn simile_in_a_barred_line_under_chord_length_repeats_the_bar_before_it() {
    // `\ChordLength` routes a barred line through the per-bar parse. `%`
    // there must repeat the bar to its left, not the last bar of the line
    // above (it used to come back as `Db` here, and as the line above's
    // whole-bar rest in "It's My Life").
    let input = r#"
Simile
120bpm 4/4 #C

VS 8
\ChordLength /
| Db | Ab | Ebm | % |
| Db | Ebm | F# | % |
"#;
    let chart = parse_chart(input).expect("Should parse");
    let bars = chart.sections[0].measures();
    assert_eq!(bars.len(), 8);
    assert_eq!(bars[3].chords[0].full_symbol, "Ebm");
    assert!(bars[3].simile);
    assert_eq!(bars[7].chords[0].full_symbol, "F#");
}

#[test]
fn life_giving_water_parses_into_its_twelve_sections() {
    // Full-chart regression: exercises section-scoped `/Duration`, the
    // rhythm-slash override of the duration default, floating slash-bass,
    // attached + standalone suspension figures, a trailing key change on a
    // section header (`BR 8 #G`), a quoted section comment (`CH 6 "New?"`),
    // and inline time changes (`!T2/4`). parse_chart only succeeds if every
    // section's parsed measure count matches its header.
    //
    // The fixture was dropped in July as "Chorus parses 6 measures against
    // a 5-measure header" and restored with three corrections to the chart
    // text, none to the parser. Each one was a fixture that disagreed
    // with rules the parser already documents and tests:
    // - `/Duration` -> `\Duration`: directives moved to a backslash in
    //   edda1e2, after this test was removed, so it was never migrated
    //   (read as slashes, `/Duration 4` put a fifth bar into the Intro).
    // - first `CH 8`, line 2: `E/B ////` -> `E/B //`. `C#m` already holds
    //   two beats of that bar, so four slashes tie `E/B` over the barline
    //   (see the tie-across-barline branch in `parse_line`). The line came
    //   to 18 beats and the chorus to 9 bars. Two slashes fill the bar.
    // - `CH "New?" 5` -> `6`: `!T2/4` closes the open bar before it, so
    //   `G` is a bar of its own. The bars are 3 + `G` + `Am7` (2/4) +
    //   `Esus ////`, which is six. The header's own `"New?"` marked the
    //   count as unsure.
    use crate::text::sections::SectionType;
    let chart = parse_chart(LIFE_GIVING_WATER).expect("Life Giving Water should parse");
    let kinds: Vec<_> = chart
        .sections
        .iter()
        .map(|s| s.section.section_type.clone())
        .collect();
    assert_eq!(chart.sections.len(), 12, "section kinds: {kinds:?}");
    assert_eq!(chart.sections[0].section.section_type, SectionType::Intro);
    assert_eq!(chart.sections[4].section.section_type, SectionType::Bridge);
    assert_eq!(
        chart.sections[6].section.section_type,
        SectionType::Instrumental
    );
    // `BR 8 #G` applied a key change to G entering the bridge.
    assert!(
        chart
            .key_changes
            .iter()
            .any(|kc| format!("{}", kc.to_key).contains('G')),
        "expected key change to G, got {:?}",
        chart.key_changes
    );
}

const LIFE_GIVING_WATER: &str = r##"Life Giving Water
64 BPM #E 4/4

\Duration 2

Intro 4
\Duration 4
C#m /// /B / A // E/G# // D //// B4-3 ////

VS 8
E B/D# A/C# E/B / /G# / A B E B4
E B/D# A/C# E/B / /G# / A B E B/A

VS 8
E B/D# A/C# E / /G# / A B E B/A
E //// A E A B E B/A

CH 8
E B/D# A/C# E/B / /G# / A C#m B4-3 ////
E E/G# C#m E/B // A B E3-4-3 ////

BR 8 #G
G D/F# C/E G/D C D G D/C
G D/F# C/E G/B C D/C G D/C

CH 8
G D/F# C/E G C //// Em D
G G/B Em G/D C D G D/C

INST 3
G D/F# C/E G/D / /B / C D

CH "New?" 6
G D/F# Em G/D C G/B G !T2/4 Am7 #A Esus ////

CH 8
A E/G# D/F# A/E / A/C# / D //// F#m E
A E/G# D/F# A/E / A/C# / D // E // F#m E/G#

CH 8
A E/G# D/F# A/E / A/C# / D //// F#m E
A A/C# F#m A/E / A/C# / D E F#m ////

Tag 5
D E F#m ////
!T2/4 D Esus // E // E5 ////

Out 4
A E/G# D/F# A/E D // Esus // F#m ////
"##;

#[test]
fn percent_repeats_previous_measure() {
    let measures = parse_line("C#\"Major Triad\" %");

    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords.len(), 1);
    assert_eq!(measures[1].chords.len(), 1);
    assert_eq!(measures[0].chords[0].full_symbol, "C#");
    assert_eq!(measures[1].chords[0].full_symbol, "C#");
}
