//! The MusicXML catalog's conversion of a lead sheet's harmony into a
//! keyflow chart: the one piece of keyflow-ui that is plain logic rather
//! than components.
#![cfg(not(target_arch = "wasm32"))]

use keyflow_ui::catalog::load_musicxml_catalog_chart;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Title and composer come from the score, the key from `<fifths>` (two
/// flats is Bb), the meter from the first `<time>`; harmony kinds become
/// chord symbols, four bars to a line, and an empty bar is a `.`.
#[test]
fn lead_sheet_becomes_a_chart() {
    let chart = load_musicxml_catalog_chart(&fixture("lead_sheet.musicxml")).unwrap();
    assert_eq!(
        chart,
        "Fixture Tune\n\
         Composer: Anon\n\
         120bpm 3/4 #Bb\n\
         \n\
         CHART\n\
         Bbmaj7 | Gm7 | C7/E | F#m7b5 B+\n\
         .\n"
    );
}

#[test]
fn missing_or_invalid_files_are_errors() {
    assert!(load_musicxml_catalog_chart(&fixture("no_such_file.musicxml")).is_err());
    assert!(load_musicxml_catalog_chart(&fixture("../catalog_musicxml.rs")).is_err());
}
