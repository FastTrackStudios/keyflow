//! The consolidation, asserted rather than remembered.
//!
//! Engraving a chart is always the same four steps — build the font
//! bundle, build a layout engine over it, lay the chart out, serialise
//! the result — and for a while three places did all four independently:
//! `keyflow-ui`'s `ChartLayoutManager`, the CLI's `LayoutPipeline`, and
//! `editor-keyflow`. They drifted, and the drift shipped: chord symbols
//! are emitted as `MuseJazz Text`, with a space, and one of the three
//! declared the family as `MuseJazzText`, so every chart it exported
//! fell back to a system sans and the `maj7` triangles came out blank.
//!
//! Nothing about that was hard to fix once it was found. The hard part
//! was that there was no place the three answers sat side by side, so
//! nobody could see they disagreed. `ChartPipeline` is that place, and
//! these tests are what keeps it that way — each one pins an invariant
//! whose violation is silent at runtime.

use engraver_proto::api::pipeline::{ChartPipeline, Paper, Preset, PresetOptions};
use engraver_proto::engraver::fonts::ChartFontBundle;
use engraver_proto::engraver::layout::chart::LayoutMode;

const TWO_BARS: &str = "VS: | Cmaj7 F#m7b5 | Bbmaj9 G7b9 |\n";

fn chart(source: &str) -> keyflow_proto::Chart {
    keyflow_text::chart::parse_chart(source).expect("the fixture parses")
}

#[test]
fn the_font_bundle_is_built_once_for_the_process() {
    // `ChartFontBundle::new` copies seven font files out of the binary
    // and parses each one. Sharing it is most of why the pipeline exists,
    // and identity is the only way to prove sharing actually happens —
    // an equal-but-separate bundle costs the same as no sharing at all.
    let a = ChartFontBundle::shared().expect("the bundle loads");
    let b = ChartFontBundle::shared().expect("the bundle loads");
    assert!(std::ptr::eq(a, b), "shared() handed out two bundles");

    let pipeline = ChartPipeline::shared().expect("the pipeline builds");
    assert!(
        std::ptr::eq(pipeline.fonts(), a),
        "the pipeline built its own bundle instead of taking the shared one"
    );
}

#[test]
fn every_font_the_scene_asks_for_is_declared_in_the_css() {
    // The actual shipped bug, from the other side: a `font-family` in the
    // output that no `@font-face` declares does not error. The browser
    // silently substitutes, and the chart renders — wrong, and plausibly
    // enough that it survived review.
    let pipeline = ChartPipeline::shared().expect("the pipeline builds");
    let result = pipeline.layout(&chart(TWO_BARS), &LayoutMode::snippet(900.0));
    let svg = pipeline.export_svg_snippet(&result, 6.0);
    let css = pipeline.font_face_css();

    for family in svg_font_families(&svg) {
        // Generic families are the browser's to resolve, not ours.
        if matches!(family.as_str(), "sans-serif" | "serif" | "monospace") {
            continue;
        }
        assert!(
            css.contains(&format!("font-family: '{family}'")),
            "the scene asks for `{family}` but no @font-face declares it"
        );
    }
}

#[test]
fn the_chord_font_is_declared_under_the_name_the_scene_uses() {
    // Pinned by name, because `MuseJazz Text` versus `MuseJazzText` is
    // exactly the kind of difference an eye slides over.
    let css = ChartPipeline::shared().expect("builds").font_face_css();
    assert!(
        css.contains("font-family: 'MuseJazz Text'"),
        "the spaced chord-font family is missing from the font CSS"
    );
}

#[test]
fn the_embedding_and_linked_exports_differ_only_in_the_fonts() {
    // Two exports, one layout. If they ever diverge in anything but the
    // font payload, a downloaded chart stops matching the one on screen.
    let pipeline = ChartPipeline::shared().expect("builds");
    let result = pipeline.layout(&chart(TWO_BARS), &LayoutMode::paginated_a4());

    let embedded = pipeline.export_svg_pages(&result);
    let linked = pipeline.export_svg_pages_linked(&result);
    assert_eq!(embedded.len(), linked.len(), "different page counts");

    for (with, without) in embedded.iter().zip(&linked) {
        assert!(with.contains("@font-face"), "the download lost its fonts");
        assert!(
            !without.contains("@font-face"),
            "the on-screen export embedded the fonts anyway"
        );
        assert!(
            with.len() > without.len(),
            "the embedding export is not carrying the font data"
        );
    }
}

#[test]
fn a_preset_resolves_the_same_way_every_time_it_is_asked() {
    // Callers cache layouts keyed on the resolved mode. A resolver that
    // answered differently on a second call — or that a caller
    // re-implemented — serves a stale layout for a chart that has
    // changed, which looks like the engine ignoring an edit.
    let options = PresetOptions::for_export();
    for preset in [Preset::Page, Preset::Snippet, Preset::Responsive] {
        let (first, _) = ChartPipeline::resolve_preset(preset, options);
        let (second, _) = ChartPipeline::resolve_preset(preset, options);
        assert_eq!(
            format!("{first:?}"),
            format!("{second:?}"),
            "{preset:?} resolved two different ways"
        );
    }
}

#[test]
fn export_options_pick_the_paper_they_name() {
    // `for_export` is A4 and `for_screen` is Letter, and the difference
    // is visible: a chart exported on the wrong sheet reflows.
    let a4 = PresetOptions::for_export();
    assert_eq!(a4.paper, Paper::A4);
    assert!(
        !a4.page_offsets,
        "an exported page carries no screen offsets"
    );

    let screen = PresetOptions::for_screen(612.0, 1.0);
    assert_eq!(screen.paper, Paper::Letter);
    assert!(screen.page_offsets, "the screen preset needs page offsets");
}

#[test]
fn a_degenerate_viewport_still_lays_out() {
    // An unmeasured pane reports zero, and a collapsed one can report
    // NaN. Either reaching the engine is a panic or an empty chart, in a
    // component that has no way to report the failure.
    let pipeline = ChartPipeline::shared().expect("builds");
    let chart = chart(TWO_BARS);
    for viewport in [0.0, -10.0, f64::NAN, 1.0] {
        let result = pipeline.layout_preset(
            &chart,
            Preset::Snippet,
            PresetOptions::for_screen(viewport, 1.0),
        );
        assert!(
            !result.pages.is_empty(),
            "viewport {viewport} laid out to nothing"
        );
    }
}

/// Every `font-family="..."` value in an SVG document.
fn svg_font_families(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("font-family=\"") {
        let after = &rest[i + 13..];
        let Some(j) = after.find('"') else { break };
        out.push(after[..j].to_string());
        rest = &after[j..];
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn a_titled_snippet_crops_to_its_music_like_a_titleless_one() {
    // `layout_snippet` lays the chart out on a 10000pt-tall scratch page
    // and then shrinks the page to whatever it measures. Anything pinned
    // to the BOTTOM of that scratch page is therefore measured as
    // content, and the crop grows to fit it.
    //
    // The page footer was pinned there, skipped only when the chart had
    // no title. So a titleless snippet cropped to ~80pt of music and a
    // titled one — the site's landing-page hero — cropped to ~10041pt: a
    // strip of blank white a hundred screens tall with four bars at the
    // top. Nothing errored; it just rendered.
    let pipeline = ChartPipeline::shared().expect("the pipeline builds");
    let mode = LayoutMode::snippet(560.0);

    let titleless = pipeline.layout(&chart("VS: | 1 4 |\n"), &mode);
    let titled = pipeline.layout(&chart("Hero - Keyflow\n\nVS: | 1 4 |\n"), &mode);

    assert!(
        titled.total_height < titleless.total_height * 2.0,
        "a title made the snippet {:.0}pt tall against {:.0}pt without one — \
         page chrome is being measured as content",
        titled.total_height,
        titleless.total_height,
    );

    let bounds = titled.content_bounds().expect("the scene is not empty");
    assert!(
        bounds.height() < 400.0,
        "the titled snippet's content spans {:.0}pt",
        bounds.height(),
    );
}

#[test]
fn paper_still_carries_the_footer() {
    // The other half of the fix: skipping the footer for snippets must
    // not skip it for the thing it exists for.
    let pipeline = ChartPipeline::shared().expect("the pipeline builds");
    let result = pipeline.layout(
        &chart("Hero - Keyflow\n\nVS: | 1 4 |\n"),
        &LayoutMode::paginated_a4(),
    );
    let svg = pipeline.export_svg_pages_linked(&result).concat();
    assert!(
        svg.contains("Created with FastTrackStudio"),
        "a printed page lost its footer"
    );
}
