//! The engraver draws SMuFL glyphs by outlining them itself, so a
//! codepoint the primary face lacks has nowhere to go but a placeholder
//! rectangle. That is not hypothetical: the primary face carries
//! `noteheadBlack` and none of the slash noteheads, which is every bar of
//! a rhythm chart.
//!
//! SVG export never showed it — it emits the codepoint as text and the
//! browser falls back across the `@font-face` set by itself. Only a
//! renderer that outlines glyphs (the GPU surface) sees the hole.

use engraver_proto::engraver::fonts::ChartFontBundle;
use skrifa::{FontRef, MetadataProvider as _};

/// The slash noteheads, which carry the rhythm on a chart.
const SLASHES: [char; 3] = ['\u{E100}', '\u{E101}', '\u{E102}'];

fn has(bytes: &[u8], cp: char) -> bool {
    FontRef::new(bytes).is_ok_and(|f| f.charmap().map(cp).is_some())
}

#[test]
fn the_music_font_covers_what_the_primary_smufl_face_misses() {
    let fonts = ChartFontBundle::shared().expect("font bundle");

    // The gap this fallback exists for. If the primary face ever gains
    // these, this assert fails and the fallback can be reconsidered —
    // it should not be deleted quietly on the assumption it is unused.
    let primary = fonts.symbol_font_data();
    assert!(
        SLASHES.iter().any(|c| !has(primary, *c)),
        "the primary SMuFL face now has every slash notehead — the \
         `render_glyph` fallback may no longer be needed"
    );

    // `SceneRenderConfig::music_font_family` defaults to Bravura, which
    // is what the fallback reaches for. It must actually have them.
    let bravura = fonts.bravura_font_data();
    for cp in SLASHES {
        assert!(
            has(bravura, cp),
            "Bravura is the music-font fallback but has no U+{:04X} — a \
             rhythm chart would render as rectangles",
            cp as u32
        );
    }
}
