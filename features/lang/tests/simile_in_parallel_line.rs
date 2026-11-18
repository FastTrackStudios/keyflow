//! `%` repeats the bar before it, also on a line holding a `<< … >>` block,
//! where the line is read a bar at a time: a `%` there repeated the bar
//! before the whole line (or nothing), which is wrong music on the page.

use keyflow_lang::text::chart::parse_chart;

fn chords(src: &str) -> Vec<Vec<String>> {
    let chart = parse_chart(src).unwrap();
    chart.sections[0]
        .measures()
        .iter()
        .map(|m| m.chords.iter().map(|c| c.full_symbol.clone()).collect())
        .collect()
}

#[test]
fn a_simile_after_a_parallel_block_repeats_the_bar_before_it() {
    let src = "T - A\n4/4\n\nVS 6\n| << D ; m{ C#:54 r2. } >> | D | Bm7 | % | Em7 | % |\n";
    let bars = chords(src);
    assert_eq!(bars[3], ["Bm7"], "{bars:?}");
    assert_eq!(bars[5], ["Em7"], "{bars:?}");
}

#[test]
fn a_simile_right_after_the_block_repeats_its_chords() {
    let src = "T - A\n4/4\n\nVS 3\n| G | << C ; m{ C:54 r2. } >> | % |\n";
    let bars = chords(src);
    assert_eq!(bars[2], ["C"], "{bars:?}");
}

#[test]
fn a_dot_after_a_parallel_block_repeats_the_chord_before_it() {
    let src = "T - A\n3/4\n\nVS 4\n| << Eadd4 ; m{ C#:54 r2 } >> | . | F#m | . |\n";
    let bars = chords(src);
    assert_eq!(bars[1], ["Eadd4"], "{bars:?}");
    assert_eq!(bars[3], ["F#m"], "{bars:?}");
}
