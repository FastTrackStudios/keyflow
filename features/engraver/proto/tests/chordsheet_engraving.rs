//! The chordsheet.com backup, engraved: every chart lays out on the page
//! without stranding a bar. The backup is not in a clean clone — put a local
//! copy at `features/examples/chordsheet/source/` (and its `manifest.json`
//! beside it) and run with `--ignored`. The import side is tested in
//! keyflow-lang's `chordsheet_corpus`.

use std::path::PathBuf;

use keyflow_lang::chordsheet::manifest::Manifest;

fn corpus() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chordsheet/source");
    dir.is_dir().then_some(dir)
}

/// Every chart in the backup engraves, and none of them strand a bar.
///
/// keyflow-lang's round-trip test proves the chords survive; this one proves
/// the page does. Both faults it checks for were found by looking at rendered
/// PDFs rather than by any test: a five-bar phrase broken four-and-one, with
/// the fifth bar alone on a line of its own.
#[test]
#[ignore = "needs a chordsheet.com backup at features/examples/chordsheet/source (copy it from examples/chordsheet-compat)"]
fn every_chart_in_the_backup_engraves_without_stranding_a_bar() {
    use engraver_proto::api::pipeline::ChartMode;
    use engraver_proto::api::pipeline::{ChartPipeline, Preset, PresetOptions};

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
        let chart = keyflow_lang::chordsheet::import_str(&source, &manifest.options_for(path));

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
