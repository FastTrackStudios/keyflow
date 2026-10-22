//! Golden round trip: every `.kf` chart in the repo goes through
//! parse → `chart_to_keyflow` → parse, and the two charts must be equal.
//!
//! "Equal" means equal as music, not as source text. The comparison runs
//! over [`ChartShape`], a projection of the parsed chart that keeps
//! everything the chart *means* (header metadata, sections, bars, meters,
//! repeats, chords and their lengths, key changes) and drops what only
//! the source carries: spans, the original token spelling, absolute
//! positions (derived from the bars), and parser bookkeeping
//! (`chord_memory`, `templates`, `section_measure_memory`).
//!
//! Charts the exporter cannot yet reproduce are listed in [`KNOWN_GAPS`]
//! with the reason. The test fails if a listed chart starts round-tripping,
//! so the list never goes stale: delete the entry when you fix the gap.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use keyflow_lang::text::chart::exporter::chart_to_keyflow;
use keyflow_lang::text::chart::parse_document;
use keyflow_proto::Chart;
use keyflow_proto::chart::types::RhythmElement;
use keyflow_proto::time::MusicalPositionExt;

/// Charts whose round trip is a known exporter gap: (path relative to the
/// repo root, why).
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "examples/chordsheet-compat/kf/Scorpions - Sails of Charon.kf",
        "a repeat whose only bar is a cue (`|: ^\"SOLO\" :|x4`) is written out by the \
         exporter as the bars before it, so the section grows from 3 bars to 5",
    ),
    (
        "features/keyflow/tests/data/in_my_room.kf",
        "an empty count-in bar exports as `r1`, a whole-note rest, which is not a bar \
         of 6/8; the exporter needs a bar-length rest (or `s`) for empty bars in \
         non-4/4 meters",
    ),
    (
        "features/keyflow/tests/midi/Broadview - Slow Pulp.kf",
        "source does not parse with `parse_document` (Chorus: 18 bars against a \
         10-bar header); the MIDI tests read this chart through their own path",
    ),
    (
        "features/keyflow/tests/midi/Cryin' - Mateus Asato.kf",
        "source does not parse with `parse_document` (Chorus: 25 bars against a \
         4-bar header); the MIDI tests read this chart through their own path",
    ),
    (
        "features/keyflow/tests/midi/For Cryin' Out Loud - FINNEAS.kf",
        "Intro bar 7 holds a 4.5-beat `B` in 4/4 (a dotted length that overruns \
         the bar); the exporter clamps it to the bar, so the reparse has 4 beats",
    ),
    (
        "features/proto/examples/vienna.kf",
        "lowercase `vs verse 1` headers and inline `[lyrics]` lines parse as Intro \
         sections of chords; what the exporter writes back for that misparse \
         (`Ddim /`) reads with other lengths. Fix the source parse first",
    ),
];

/// Directories holding the `.kf` corpus, relative to the repo root.
const CORPUS_DIRS: &[&str] = &[
    "examples/chordsheet-compat/kf",
    "features/keyflow/tests/midi",
    "features/keyflow/tests/data",
    "features/lang/tests/fixtures",
    "features/proto/examples",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files: Vec<PathBuf> = CORPUS_DIRS
        .iter()
        .flat_map(|dir| {
            std::fs::read_dir(root.join(dir))
                .unwrap_or_else(|e| panic!("corpus dir {dir}: {e}"))
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "kf"))
                .collect::<Vec<_>>()
        })
        .collect();
    files.sort();
    files
}

fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug, PartialEq)]
struct ChartShape {
    title: Option<String>,
    artist: Option<String>,
    tempo_bpm: Option<u32>,
    initial_key: Option<String>,
    initial_time_signature: Option<(u32, u32)>,
    key_changes: Vec<String>,
    sections: Vec<SectionShape>,
}

#[derive(Debug, PartialEq)]
struct SectionShape {
    kind: String,
    number: Option<u32>,
    bars: Vec<BarShape>,
}

#[derive(Debug, PartialEq)]
struct BarShape {
    time_signature: (u8, u8),
    /// Each chord as `symbol@beats`.
    chords: Vec<String>,
    /// Rests and spaces as `kind@beats`, in order with the chords.
    elements: Vec<String>,
}

fn beats(d: &keyflow_proto::time::MusicalDuration, ts: (u8, u8)) -> String {
    let ts = keyflow_proto::time::TimeSignature::new(u32::from(ts.0), u32::from(ts.1));
    format!("{:.3}", d.to_beats(ts))
}

fn shape(chart: &Chart) -> ChartShape {
    ChartShape {
        title: chart.metadata.title.clone(),
        artist: chart.metadata.artist.clone(),
        tempo_bpm: chart.tempo.as_ref().map(|t| t.bpm.round() as u32),
        initial_key: chart.initial_key.as_ref().map(|k| format!("{k}")),
        initial_time_signature: chart
            .initial_time_signature
            .as_ref()
            .or(chart.time_signature.as_ref())
            .map(|ts| (ts.numerator, ts.denominator)),
        key_changes: chart
            .key_changes
            .iter()
            .map(|kc| format!("s{}:{}", kc.section_index, kc.to_key))
            .collect(),
        sections: chart
            .sections
            .iter()
            .map(|s| SectionShape {
                kind: format!("{:?}", s.section.section_type),
                number: s.section.number,
                bars: s
                    .measures()
                    .iter()
                    .map(|m| BarShape {
                        time_signature: m.time_signature,
                        // A padded bar (header count > bars written) holds a
                        // placeholder chord spelled `s`; the exporter writes it
                        // as `s`, which reads back as a space. Both are the
                        // same empty bar (the engraver draws both as slashes),
                        // so the shape counts the placeholder as a space.
                        chords: m
                            .chords
                            .iter()
                            .filter(|c| c.full_symbol != "s")
                            .map(|c| {
                                format!(
                                    "{}@{}",
                                    c.full_symbol,
                                    beats(&c.duration, m.time_signature)
                                )
                            })
                            .collect(),
                        elements: m
                            .rhythm_elements
                            .iter()
                            .map(|e| {
                                let kind = match e {
                                    RhythmElement::Chord(c) if c.full_symbol == "s" => "space",
                                    RhythmElement::Chord(_) => "chord",
                                    RhythmElement::Rest(_) => "rest",
                                    RhythmElement::Space(_) => "space",
                                };
                                format!("{kind}@{}", beats(e.duration(), m.time_signature))
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Where two shapes first differ, for a readable failure.
fn first_difference(a: &ChartShape, b: &ChartShape) -> String {
    macro_rules! field {
        ($f:ident) => {
            if a.$f != b.$f {
                return format!("{}: {:?} -> {:?}", stringify!($f), a.$f, b.$f);
            }
        };
    }
    field!(title);
    field!(artist);
    field!(tempo_bpm);
    field!(initial_key);
    field!(initial_time_signature);
    if a.sections.len() != b.sections.len() {
        return format!("section count {} -> {}", a.sections.len(), b.sections.len());
    }
    for (i, (sa, sb)) in a.sections.iter().zip(&b.sections).enumerate() {
        if sa.kind != sb.kind || sa.number != sb.number {
            return format!(
                "section {i}: {} {:?} -> {} {:?}",
                sa.kind, sa.number, sb.kind, sb.number
            );
        }
        if sa.bars.len() != sb.bars.len() {
            return format!(
                "section {i} ({}): {} bars -> {}",
                sa.kind,
                sa.bars.len(),
                sb.bars.len()
            );
        }
        for (j, (ba, bb)) in sa.bars.iter().zip(&sb.bars).enumerate() {
            if ba != bb {
                return format!("section {i} ({}) bar {j}: {ba:?} -> {bb:?}", sa.kind);
            }
        }
    }
    field!(key_changes);
    "shapes differ".to_string()
}

/// `Ok(())` if the chart survives; `Err(reason)` otherwise. `Err` starting
/// with `source:` means the corpus chart itself does not parse.
fn round_trip(path: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("source: read: {e}"))?;
    let (first, _) = parse_document(&source).map_err(|e| format!("source: {e}"))?;
    let exported = chart_to_keyflow(&first);
    if let Some(want) = std::env::var_os("KEYFLOW_ROUND_TRIP_DUMP") {
        if rel(path).contains(want.to_string_lossy().as_ref()) {
            println!(
                "=== {} source\n{source}\n=== exported\n{exported}",
                rel(path)
            );
        }
    }
    let (second, _) =
        parse_document(&exported).map_err(|e| format!("export does not reparse: {e}"))?;
    let (a, b) = (shape(&first), shape(&second));
    if a == b {
        Ok(())
    } else {
        Err(first_difference(&a, &b))
    }
}

#[test]
fn every_corpus_chart_round_trips_through_the_keyflow_exporter() {
    let files = corpus();
    assert!(
        files.len() >= 200,
        "expected the .kf corpus, found {} files",
        files.len()
    );
    let gaps: BTreeMap<&str, &str> = KNOWN_GAPS.iter().copied().collect();

    let mut failures = Vec::new();
    let mut stale = Vec::new();
    for path in &files {
        let name = rel(path);
        let result = round_trip(path);
        match (result, gaps.contains_key(name.as_str())) {
            (Ok(()), false) | (Err(_), true) => {}
            (Ok(()), true) => stale.push(name),
            (Err(why), false) => failures.push(format!("{name}: {why}")),
        }
    }
    for listed in gaps.keys() {
        assert!(
            files.iter().any(|f| rel(f) == *listed),
            "KNOWN_GAPS names {listed}, which is not in the corpus"
        );
    }
    if std::env::var_os("KEYFLOW_ROUND_TRIP_REPORT").is_some() {
        for f in &failures {
            println!("GAP\t{f}");
        }
    }

    assert!(
        stale.is_empty(),
        "these charts now round-trip; remove them from KNOWN_GAPS:\n{}",
        stale.join("\n")
    );
    assert!(
        failures.is_empty(),
        "{} of {} corpus charts did not round-trip:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}
