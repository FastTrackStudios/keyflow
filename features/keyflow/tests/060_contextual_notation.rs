//! Contextual notation: a chart resolves whether it is written in letters,
//! Nashville numbers or Roman numerals, and that decision has to reach every
//! token — including the ones that look like they belong to another system.

use keyflow::{ChartView, NotationSystem};

fn chart(body: &str) -> String {
    format!("Sunday Morning - The Wandering\n4/4 120bpm #C\n\nVS 4\n{body}\n")
}

fn symbols(src: &str) -> Vec<String> {
    keyflow::parse(src)
        .unwrap()
        .sections
        .iter()
        .flat_map(|s| s.measures().to_vec())
        .flat_map(|m| {
            m.chords
                .iter()
                .map(|c| c.full_symbol.clone())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn in_letters(src: &str) -> Vec<String> {
    let c = keyflow::parse(src).unwrap();
    let out = keyflow::apply_view(
        &c,
        &ChartView {
            notation: NotationSystem::Letters,
            target_key: Some(keyflow::Key::parse("C").unwrap()),
            ..Default::default()
        },
    );
    out.sections
        .iter()
        .flat_map(|s| s.measures().to_vec())
        .flat_map(|m| {
            m.chords
                .iter()
                .map(|c| c.full_symbol.clone())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// A lowercase `b` before a digit is a flat, whatever the rest of the line is
/// written in. `C b3 F G` used to engrave as `C B F G`: the line voted
/// "letters", and the flat degree was read as the note B — for `b2`/`b3`/`b4`
/// the digit was then swallowed as a suspension figure, leaving a bare `B`.
///
/// The note is written `B`. That is what makes the lowercase form free to be
/// a flat.
#[test]
fn flat_degrees_survive_a_letters_chart() {
    for degree in ["b2", "b3", "b4", "b5", "b6", "b7"] {
        assert_eq!(
            symbols(&chart(&format!("C {degree} F G"))),
            ["C", degree, "F", "G"],
            "{degree} should stay a flat degree on a letters line"
        );
    }
}

#[test]
fn an_uppercase_b_is_still_the_note() {
    assert_eq!(symbols(&chart("C B F G")), ["C", "B", "F", "G"]);
    assert_eq!(symbols(&chart("C B7 F G")), ["C", "B7", "F", "G"]);
    assert_eq!(symbols(&chart("C Bb F G")), ["C", "Bb", "F", "G"]);
}

#[test]
fn flat_degrees_still_work_in_their_own_systems() {
    assert_eq!(symbols(&chart("1 b3 4 5")), ["1", "b3", "4", "5"]);
    assert_eq!(symbols(&chart("I bIII IV V")), ["I", "bIII", "IV", "V"]);
}

/// A Roman numeral over a Roman numeral is a secondary chord, not a slash
/// bass. `Chord::parse` already knew that; the chart parser split the slash
/// itself before reaching that logic, so `V/V` in C resolved to `G/V` — a
/// five chord with a `V` bolted on as a bass — instead of D.
#[test]
fn roman_over_roman_is_a_secondary_chord() {
    assert_eq!(symbols(&chart("I V/V IV V")), ["I", "V/V", "IV", "V"]);
    assert_eq!(in_letters(&chart("I V/V IV V")), ["C", "D", "F", "G"]);
    assert_eq!(in_letters(&chart("I V/vi IV V")), ["C", "E", "F", "G"]);
    assert_eq!(in_letters(&chart("I V/ii IV V")), ["C", "A", "F", "G"]);
    // The applied chord keeps its own quality: V7/V is a dominant on D.
    assert_eq!(in_letters(&chart("I V7/V IV V")), ["C", "D7", "F", "G"]);
}

#[test]
fn an_ordinary_slash_chord_is_still_a_slash_chord() {
    // Only Roman-over-Roman is applied notation. A degree or a letter after
    // the slash is a bass note, as before.
    assert_eq!(symbols(&chart("1 1/3 4 5")), ["1", "1/3", "4", "5"]);
    assert_eq!(symbols(&chart("C G/B F G")), ["C", "G/B", "F", "G"]);
}

/// Nashville keeps the quality marker, including when it is the degree's own
/// diatonic quality: `Em` in G is `6m`, not `6`.
#[test]
fn nashville_keeps_the_minor_marker() {
    let c = keyflow::parse("Sunday - W\n4/4 #G\n\nVS 4\nG C D Em\n").unwrap();
    let out = keyflow::apply_view(
        &c,
        &ChartView {
            notation: NotationSystem::Nashville,
            ..Default::default()
        },
    );
    let got: Vec<String> = out
        .sections
        .iter()
        .flat_map(|s| s.measures().to_vec())
        .flat_map(|m| {
            m.chords
                .iter()
                .map(|c| c.full_symbol.clone())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(got, ["1", "4", "5", "6m"]);
}
