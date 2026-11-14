//! A chord named twice in one bar keeps each occurrence's own length in
//! `rhythm_elements`. The engraver places chord symbols from those lengths,
//! so `A/E_4. A/E_8 B/F#_8 C/G_4.` used to give the second A/E the first
//! one's dotted quarter and push B/F# and C/G a beat late, off their notes.

use keyflow_lang::text::chart::parse_chart;
use keyflow_lang::text::chart::types::RhythmElement;
use keyflow_proto::time::{MusicalPositionExt, TimeSignature};

fn element_beats(src: &str) -> Vec<(String, f64)> {
    let chart = parse_chart(src).expect("chart should parse");
    let measure = chart.sections[0].measures().last().cloned().expect("a bar");
    let ts = TimeSignature::new(4, 4);
    measure
        .rhythm_elements
        .iter()
        .filter_map(|e| match e {
            RhythmElement::Chord(c) => Some((c.full_symbol.clone(), c.duration.to_beats(ts))),
            _ => None,
        })
        .collect()
}

#[test]
fn repeated_chord_keeps_its_own_length() {
    let got = element_beats("T - X\n#E 116bpm 4/4\n\nIN 1\nA/E_4. A/E_8 B/F#_8 C/G_4.\n");
    assert_eq!(
        got,
        vec![
            ("A/E".to_string(), 1.5),
            ("A/E".to_string(), 0.5),
            ("B/F#".to_string(), 0.5),
            ("C/G".to_string(), 1.5),
        ]
    );
}

#[test]
fn repeated_chord_after_other_bars_keeps_its_own_length() {
    let got = element_beats("T - X\n#E 116bpm 4/4\n\nVS 4\nA E/G# F#m7 E_4. E_8 F#5_8 G5_4.\n");
    assert_eq!(
        got,
        vec![
            ("E".to_string(), 1.5),
            ("E".to_string(), 0.5),
            ("F#5".to_string(), 0.5),
            ("G5".to_string(), 1.5),
        ]
    );
}
