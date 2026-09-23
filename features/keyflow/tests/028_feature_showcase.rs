//! Test 028: Feature Showcase
//!
//! A comprehensive chart that demonstrates all keyflow rendering features:
//! - Chord symbols (major, minor, 7th, extended, slash chords)
//! - Stop signs (!STOP before/after, !STOPGROOVE before/after)
//! - Text cues (@keys, @drums, @bass, @all)
//! - Explicit rhythm notation (durations, rests, triplets)
//! - Push/pull notation ('C, C')
//! - Accents and staccato (>C, .C)
//! - Dynamic markings (<Build>, <Down>)
//! - Section types (Intro, Verse, Pre-Chorus, Chorus, Bridge, Solo, Outro)
//! - Slash rhythm notation (C //, C ////)
//! - Repeat syntax (x4)
//! - Measure separators (|)
//! - Tempo changes (->140bpm)

use std::fs;
use std::path::PathBuf;

use engraver::api::pipeline::ChartPipeline;
use engraver::api::style::leak_style;
use engraver::layout::chart::{ChartLayoutConfig, LayoutMode};
use engraver::style::MStyle;

fn output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/output");
    fs::create_dir_all(&dir).expect("create output dir");
    dir
}

#[test]
fn test_feature_showcase_pdf() {
    let chart_text = r#"
Feature Showcase - Keyflow Demo

120bpm 4/4 #C

IN 4
@drums "click count"
| !STOP >C | Am7 | F | G !STOP |

VS 8
@keys "rhodes"
| C | Am | F | G |
| C 'Em | Am | .F .G .Am .Bm | >C |

PRE-CH 4
<Build>
| Dm7 | G7 | Em7 | Am7 !STOPGROOVE |

CH 8
->140bpm
@all "full band"
| !STOP F | G | Am | Em |
| F | G | C !STOP | C |

BR 4
@keys "pad"
<Down>
| Dm | Fm | !STOPGROOVE C/E | F |

SOLO 4
@guitar "lead"
| Am | F | C | G !STOPGROOVE |

OUT 4
| C | Am | !STOP F | G !STOP |
"#;

    let chart = keyflow::parse(chart_text).expect("parse chart");

    // One pipeline: it owns the font bundle, the engine and the export,
    // so the families the PDF embeds cannot drift from the ones the
    // layout engine emits. Hand-wiring the three here is what dropped
    // chord symbols onto a system face.
    let pipeline = ChartPipeline::with_style(leak_style(MStyle::new())).expect("build pipeline");

    let layout_config = ChartLayoutConfig::master_rhythm().with_page_offsets(true);
    let mode = LayoutMode::paginated_a4();
    let result = pipeline.layout_with_config(&chart, &mode, &layout_config);

    let pdf_bytes = pipeline.export_pdf(&result).expect("serialize PDF");

    let out = output_dir().join("feature_showcase.pdf");
    fs::write(&out, &pdf_bytes).expect("write feature_showcase.pdf");
    println!(
        "Wrote {} ({} pages, {} bytes)",
        out.display(),
        result.pages.len(),
        pdf_bytes.len()
    );
}
