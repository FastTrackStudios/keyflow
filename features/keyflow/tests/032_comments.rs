//! `;` comments, including the mid-line form.
//!
//! `;` carries two meanings. It starts a comment, and it is also the
//! **track separator** inside a `<< … >>` parallel container. Comment
//! stripping happens per line, before container lines are joined, so it
//! cannot ask the parser which one it is looking at — it has to decide
//! from the line alone.
//!
//! The old rule bailed on any spaced `" ; "`, which is far too broad: a
//! comment on an ordinary chord line was never stripped, so
//! `C F G Am ; four bars` tried to parse "four" and "bars" as chords and
//! the whole line was lost. structure.md has always documented the
//! general form ("A semicolon starts a comment. Everything after it on
//! the line is ignored"), and it only ever worked on the metadata line.
//!
//! That is also why every annotated example in the guide had to be a
//! source-only fence: the annotation could not live inside a block that
//! was going to be engraved.

use keyflow::parse;

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
fn a_comment_can_follow_chords_on_the_same_line() {
    assert_eq!(bars("C  F  G  Am   ; four bars\n"), ["C", "F", "G", "Am"]);
}

#[test]
fn a_comment_can_contain_anything() {
    // Including punctuation that would otherwise look like syntax.
    assert_eq!(
        bars("C  F  G  Am   ; four bars, one chord each (in 4/4)\n"),
        ["C", "F", "G", "Am"]
    );
}

#[test]
fn a_whole_line_comment_still_works() {
    assert_eq!(
        bars("; a leading comment\nC F G Am\n"),
        ["C", "F", "G", "Am"]
    );
}

#[test]
fn a_comment_on_the_metadata_line_still_works() {
    let chart = parse("T\n4/4 120bpm #C  ; mid-tempo, straight feel\n\nVS\nC F\n").expect("parse");
    let ts = chart.time_signature.as_ref().expect("meter");
    assert_eq!((ts.numerator, ts.denominator), (4, 4));
    assert_eq!(chart.tempo.map(|t| t.bpm), Some(120.0));
}

#[test]
fn comments_work_on_rhythm_and_bar_line_forms() {
    let head = "T\n4/4 #C\n\nVS\n";
    assert_eq!(
        bars(&format!("{head}C // G //   ; two chords, two beats each\n")),
        ["C G"]
    );
    assert_eq!(
        bars(&format!(
            "{head}|G |C |Em |D   ; four bars, one chord each\n"
        )),
        ["G", "C", "Em", "D"]
    );
}

// ── The separator must survive ─────────────────────────────────────

/// Chord bars and track count — enough to tell a parsed container from a
/// mangled one.
fn shape(src: &str) -> (usize, usize) {
    let chart = parse(src).unwrap_or_else(|e| panic!("should parse:\n{src}\n\n{e}"));
    (
        chart.sections.iter().map(|s| s.measures().len()).sum(),
        chart.sections.iter().map(|s| s.tracks.len()).sum(),
    )
}

#[test]
fn an_inline_parallel_container_keeps_its_separator() {
    // `<< C ; m{ … } >>` — the `;` here divides two tracks. Strip it as a
    // comment and the melody disappears.
    let src = "T\n4/4 #C\n\nVS\n<< C ;  m{ C8 D8 E8 F8 G8 A8 B8 C8 } >>\n";
    assert_eq!(shape(src), (1, 1));
}

#[test]
fn a_multiline_parallel_container_keeps_its_separator() {
    // Here the separator is a TRAILING `;` on the chord line.
    let src = "T\n4/4 #C\n\nVS\n<<\n  Am  F ;\n  m{ C8 D E F E4 C  E8 F G A G4 E }\n>>\n";
    assert_eq!(shape(src), (2, 1));
}

#[test]
fn a_melody_line_still_parses() {
    let src = "T\n4/4 #C\n\nVS\nC F\nm{ C D E F }\n";
    assert_eq!(shape(src), (3, 1));
}
