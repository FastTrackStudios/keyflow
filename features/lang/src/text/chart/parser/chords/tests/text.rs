use super::*;

#[test]
fn parallel_container_can_merge_chords_with_long_melody_branch() {
    let input = r#"
Parallel Melody Test
120bpm 6/8 #E

opening 2
<< \ChordLength 8. F#m7 G#m7 Amaj7 B | C#m ;
   m { <F# 'C#>4. <G# 'D#>4. <A 'E>4. <B 'F#>4. } >>
"#;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();

    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords.len(), 4);
    assert_eq!(measures[1].chords.len(), 1);
    assert_eq!(measures[0].melodies.len(), 1);
    assert_eq!(measures[0].melodies[0].notes.len(), 2);
    assert_eq!(measures[0].melodies[0].notes[0].extra_pitches[0].0, "C#");
    assert_eq!(measures[1].melodies.len(), 1);
    assert_eq!(measures[1].melodies[0].notes.len(), 2);
}

#[test]
fn aliases_expand_for_chord_attached_and_standalone_text() {
    let input = r##"
Alias Test
120bpm 4/4 #C
\alias fb ^"4-3 2-1"
\alias #fb ^"#4-3 2-1"

Verse 2
A<#fb>
C#m <fb> /. _<fb> /.
"##;
    let chart = parse_chart(input).expect("Should parse");
    let measures = chart.sections[0].measures();

    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].chords[0].full_symbol, "A");
    assert_eq!(measures[0].figured_bass.len(), 1);
    assert_eq!(measures[0].figured_bass[0].rows[0].accidental, "#");
    assert_eq!(measures[0].figured_bass[0].rows[0].text, "4-3");

    assert_eq!(measures[1].figured_bass.len(), 0);
    assert_eq!(measures[1].staff_text.len(), 2);
    assert_eq!(measures[1].staff_text[0].text, "4-3 2-1");
    assert_eq!(measures[1].staff_text[0].placement, Placement::Above);
    assert_eq!(measures[1].staff_text[1].text, "4-3 2-1");
    assert_eq!(measures[1].staff_text[1].placement, Placement::Below);
}

#[test]
fn standalone_quoted_numeric_text_stays_staff_text() {
    let measures = parse_line("| A / _\"#4-3 2-1\" // |");

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].figured_bass.len(), 0);
    assert_eq!(measures[0].staff_text.len(), 1);
    assert_eq!(measures[0].staff_text[0].beat, 2);
    assert_eq!(measures[0].staff_text[0].placement, Placement::Below);
    assert_eq!(measures[0].staff_text[0].text, "#4-3 2-1");
}

#[test]
fn quoted_staff_text_attaches_to_current_measure() {
    let measures = parse_line("| \"Ac. Gtr. groove\" A /// |");

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].staff_text.len(), 1);
    assert_eq!(measures[0].staff_text[0].text, "Ac. Gtr. groove");
    assert_eq!(measures[0].staff_text[0].beat, 1);
    assert_eq!(measures[0].staff_text[0].placement, Placement::Below);
}

#[test]
fn quoted_staff_text_can_choose_placement() {
    let measures = parse_line("| _\"below staff\" ^\"above staff\" A /// |");

    assert_eq!(measures.len(), 1);
    assert_eq!(measures[0].staff_text.len(), 2);
    assert_eq!(measures[0].staff_text[0].text, "below staff");
    assert_eq!(measures[0].staff_text[0].placement, Placement::Below);
    assert_eq!(measures[0].staff_text[1].text, "above staff");
    assert_eq!(measures[0].staff_text[1].placement, Placement::Above);
}
