//! Whole-corpus smoke test.
//!
//! Ignored: the corpus is a chordsheet.com account backup — 200-odd
//! transcriptions of commercial songs — and this repo is public, so the files
//! are not ours to ship. Drop a backup's `source/` directory at
//! `features/examples/chordsheet/source/` (and its `manifest.json` beside it)
//! and run `cargo test -p keyflow-chordsheet -- --ignored`.
//!
//! What it proves: every chart in the backup imports without panicking, and
//! the `.kf` it exports parses back into a chart with the same number of
//! sections. That round trip is the contract — an import that only *looks*
//! right until keyflow reads it back is not an import.

use std::path::PathBuf;

use keyflow_chordsheet::manifest::Manifest;

fn corpus() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/chordsheet/source");
    dir.is_dir().then_some(dir)
}

#[test]
#[ignore = "needs a chordsheet.com backup at features/examples/chordsheet/source (not redistributable)"]
fn every_chart_in_the_backup_round_trips_through_keyflow_text() {
    let Some(dir) = corpus() else {
        panic!("corpus not present — see the module docs");
    };
    let manifest = Manifest::read(dir.join("../manifest.json")).unwrap_or_default();

    let mut sources: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read corpus dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("txt"))
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "corpus dir has no .txt files");

    let mut mismatches = Vec::new();
    for path in &sources {
        let source = std::fs::read_to_string(path).expect("read source");
        let chart = keyflow_chordsheet::import_str(&source, &manifest.options_for(path));
        let text = keyflow_text::chart::exporter::chart_to_keyflow(&chart);

        match keyflow_text::chart::parse_chart(&text) {
            Ok(reparsed) => {
                let before: Vec<usize> =
                    chart.sections.iter().map(|s| s.measures().len()).collect();
                let after: Vec<usize> = reparsed
                    .sections
                    .iter()
                    .map(|s| s.measures().len())
                    .collect();
                // Sections must survive one for one, and no section may come
                // back shorter. It may come back *longer*: `chart_to_keyflow`
                // writes `|: … :|` out as the bars it stands for, so a section
                // with a repeat legitimately doubles.
                if before.len() != after.len() || before.iter().zip(&after).any(|(b, a)| a < b) {
                    mismatches.push(format!(
                        "{}: sections {before:?} in, {after:?} out",
                        path.display()
                    ));
                }
            }
            Err(e) => mismatches.push(format!("{}: reparse failed: {e}", path.display())),
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} of {} charts did not round-trip:\n{}",
        mismatches.len(),
        sources.len(),
        mismatches.join("\n")
    );
}

/// Every chart in the backup engraves, and none of them strand a bar.
///
/// The round-trip test above proves the chords survive; this one proves the
/// page does. Both faults it checks for were found by looking at rendered
/// PDFs rather than by any test: a five-bar phrase broken four-and-one, with
/// the fifth bar alone on a line of its own.
#[test]
#[ignore = "needs a chordsheet.com backup at features/examples/chordsheet/source (not redistributable)"]
fn every_chart_in_the_backup_engraves_without_stranding_a_bar() {
    use engraver::api::pipeline::ChartMode;
    use engraver::api::pipeline::{ChartPipeline, Preset, PresetOptions};

    let Some(dir) = corpus() else {
        panic!("corpus not present — see the module docs");
    };
    let manifest = Manifest::read(dir.join("../manifest.json")).unwrap_or_default();
    let pipeline = ChartPipeline::shared().expect("fonts");

    let mut sources: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read corpus dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("txt"))
        .collect();
    sources.sort();

    let mut faults = Vec::new();
    for path in &sources {
        let source = std::fs::read_to_string(path).expect("read source");
        // A backup can carry an empty file — a song started and abandoned. It
        // imports to a chart of one blank bar, which is the right answer and
        // not a line to count against the layout.
        if source.trim().is_empty() {
            continue;
        }
        let chart = keyflow_chordsheet::import_str(&source, &manifest.options_for(path));

        for mode in [ChartMode::Default, ChartMode::Folded, ChartMode::Compact] {
            let layout = pipeline.layout_preset(
                &chart,
                Preset::Page,
                PresetOptions::for_export().with_mode(mode),
            );
            if layout.pages.is_empty() {
                faults.push(format!(
                    "{} ({mode:?}): laid out to nothing",
                    path.display()
                ));
                continue;
            }
            // A line of one bar is only honest when the section itself is one
            // bar long — a `[Page]` marker, or a single hit. Any more of them
            // than that and a phrase has been broken with a bar left over.
            let one_bar_sections = chart
                .sections
                .iter()
                .filter(|section| section.measures().len() == 1)
                .count();
            let stranded = layout
                .pages
                .iter()
                .flat_map(|page| &page.systems)
                .filter(|system| system.measure_indices.len() == 1)
                .count();
            if stranded > one_bar_sections {
                faults.push(format!(
                    "{} ({mode:?}): {stranded} lines of one bar, {one_bar_sections} one-bar sections",
                    path.display(),
                ));
            }
        }
    }

    assert!(
        faults.is_empty(),
        "{} of {} charts engraved badly:\n{}",
        faults.len(),
        sources.len(),
        faults.join("\n")
    );
}
