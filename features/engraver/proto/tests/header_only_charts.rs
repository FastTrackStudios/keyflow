//! A chart engraves from the first thing you type.
//!
//! Everything downstream of layout hangs off a system, and a chart with
//! no sections produces none — so a header on its own used to lay out to
//! ZERO pages, and every export answered "No chart layout available to
//! export". On screen that reads as an editor that ignores you until you
//! have typed enough to satisfy it.
//!
//! These pin the five things that must stand alone. Each asserts on the
//! text the page actually draws, not on the absence of an error: a page
//! that renders but omits the tempo is the bug that was there before.

use engraver_proto::api::pipeline::{ChartPipeline, Paper, Preset, PresetOptions};

/// SMuFL codepoints the engraved page uses.
mod glyph {
    /// `gClef`/`bassClef` — a staff was drawn at all.
    pub const CLEF: char = '\u{E062}';
    /// `timeSig4` — the meter, drawn once per numeral.
    pub const TIME_SIG_4: char = '\u{E084}';
    /// `accidentalSharp` in a key signature.
    pub const SHARP: char = '\u{E262}';
}

/// Every text run the engraved page draws, in order.
fn drawn_text(source: &str) -> Vec<String> {
    let pipeline = ChartPipeline::shared().expect("the pipeline builds");
    let chart = keyflow_text::chart::parse_chart(source).expect("parses");
    let laid_out = pipeline.layout_preset(
        &chart,
        Preset::Page,
        PresetOptions::for_export().with_paper(Paper::A4),
    );
    assert!(
        !laid_out.pages.is_empty(),
        "{source:?} laid out to no pages — nothing can be exported from it"
    );
    pipeline
        .export_svg_pages_linked(&laid_out)
        .join("")
        .split("</text>")
        .filter_map(|chunk| chunk.rsplit_once('>').map(|(_, t)| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .collect()
}

fn joined(source: &str) -> String {
    drawn_text(source).join(" | ")
}

#[test]
fn a_bare_title_engraves_as_the_title() {
    let out = joined("Song Title");
    assert!(
        out.contains("Song Title"),
        "the title never reached the page: {out}"
    );
}

#[test]
fn a_leading_dash_credits_the_writer_and_names_no_song() {
    // `- Cody Wright` credits somebody and titles nothing. The general
    // "Title - Artist" splitter wants text on both sides of the dash, so
    // it used to keep the whole line — dash included — as the title.
    let out = joined("- Cody Wright");
    assert!(out.contains("Cody Wright"), "no credit on the page: {out}");
    assert!(
        !out.contains("- Cody Wright"),
        "the dash was kept as part of a title: {out}"
    );
}

#[test]
fn a_bare_meter_opens_a_measure_with_its_time_signature() {
    let out = joined("4/4");
    assert!(out.contains(glyph::CLEF), "no staff was drawn: {out}");
    assert_eq!(
        out.matches(glyph::TIME_SIG_4).count(),
        2,
        "4/4 should draw two numerals: {out}"
    );
}

#[test]
fn a_bare_tempo_engraves_the_tempo_mark() {
    // The header used to be skipped whole unless the chart had a title,
    // which threw the tempo away with it.
    let out = joined("120bpm");
    assert!(
        out.contains("= 120"),
        "the tempo mark is missing from a chart that names only a tempo: {out}"
    );
}

#[test]
fn a_bare_key_opens_a_measure_with_its_key_signature() {
    // E major is four sharps, and the meter falls back to 4/4 so the
    // measure the writer is about to fill is already there.
    let out = joined("#E");
    assert_eq!(
        out.matches(glyph::SHARP).count(),
        4,
        "E major should draw four sharps: {out}"
    );
    assert_eq!(
        out.matches(glyph::TIME_SIG_4).count(),
        2,
        "the assumed 4/4 should still be drawn: {out}"
    );
}

#[test]
fn the_opening_measure_is_implicit_and_draws_no_section_card() {
    // The measure is synthesized, so nothing labelled it. An implicit
    // section renders no name — without that a bare `#E` would engrave a
    // "Verse 1" card nobody asked for.
    for source in ["Song Title", "4/4", "#E", "120bpm"] {
        let out = joined(source);
        for label in ["Verse", "VS", "Chorus", "CH"] {
            assert!(
                !out.contains(label),
                "{source:?} engraved a {label} card for a section nobody wrote: {out}"
            );
        }
    }
}

#[test]
fn a_header_with_no_content_at_all_still_draws_its_measure() {
    // No title, artist or tempo — the header is skipped, but the staff
    // is not: `part_name` defaults to "Master Rhythm", and counting it
    // as content would stamp MASTER RHYTHM across a chart whose author
    // typed only a meter.
    let out = joined("4/4");
    assert!(
        !out.contains("MASTER"),
        "part-name furniture leaked in: {out}"
    );
    assert!(out.contains(glyph::CLEF), "no staff was drawn: {out}");
}
