//! Rendering a chart on the page.
//!
//! `keyflow-ui` deliberately stops short of this: it hands back a
//! `ChartLayoutManager` that lays a chart out and can paint it into an
//! `anyrender::PaintScene` or serialise it to SVG, and leaves presentation
//! to the host app. That split is the whole reason the notation domain has
//! no dependency on any application — so this is the site's job, and it
//! lives here.
//!
//! # Why SVG and not the GPU canvas
//!
//! `keyflow-ui` also ships a WebGL surface (`ChartGraphics`, behind
//! `wasm-graphics`), and the first version of this module used it. It was
//! the wrong tool for this site, for a reason worth writing down: a WebGL
//! context is a scarce, stateful resource, and every chart on a page wanted
//! its own. Browsers cap live contexts at around sixteen, a guide page
//! creates several, and re-creating one on each re-render wedged the
//! renderer outright.
//!
//! Nothing on this site needs a GPU. Layout costs about 10 ms and the
//! output is *static* until the source changes — so it is serialised once
//! to SVG and handed to the browser, which then scales, prints and
//! selects it for free. The GPU path stays in `keyflow-ui` for the case it
//! was built for: a live cursor tracking playback at 120 Hz, in the desktop
//! app.
//!
//! # Fonts
//!
//! `ChartLayoutManager::export_svg_pages` embeds Bravura, MuseJazzText and
//! FreeSans into each document — about 485 KB apiece, which is right for a
//! downloaded file and badly wrong for a page with four charts on it. So
//! the site uses the linked variant and emits the `@font-face` block once,
//! via [`ChartFonts`].

use dioxus::prelude::*;
use keyflow_ui::ChartLayoutManager;

/// The chart fonts, declared once for the whole page.
///
/// Render this once, above any [`ChartSvg`]. Without it charts fall back to
/// the browser's default font and the musical glyphs render as tofu.
#[component]
pub fn ChartFonts() -> Element {
    // Static across the life of the page: the font bundle is baked into the
    // binary, so this is computed once rather than per render.
    let css = use_hook(|| {
        ChartLayoutManager::new()
            .map(|m| m.font_face_css())
            .unwrap_or_default()
    });
    rsx! { document::Style { {css} } }
}

/// An engraved chart.
///
/// Renders `source` and nothing else — no cursor, no playback, no dock. The
/// panels that wired those together used to live in `keyflow-ui` and were
/// deleted when the notation domain was extracted; anything that couples a
/// chart to *application* state belongs on this side of the boundary.
#[component]
pub fn ChartSvg(
    /// Chart source. Re-engraved whenever this changes.
    source: String,
    /// Lay out as a continuous snippet rather than paginated pages. Guide
    /// examples are snippets; the editor shows pages.
    #[props(default = true)]
    snippet: bool,
    /// Width in scene units to lay out against. Only the aspect ratio
    /// reaches the page — the SVG is scaled to its container by CSS — so
    /// this sets how much fits on a system, not how big the chart looks.
    #[props(default = 900.0)]
    layout_width: f64,
) -> Element {
    // `use_memo` and not `use_effect`: engraving is a pure function of the
    // source, so it belongs in the render path. The earlier effect-based
    // version wrote a signal the component also read, which re-entered on
    // every pass.
    //
    // `use_reactive!` is load-bearing, not decoration. A bare
    // `use_memo(move || engrave(&source, ..))` captures `source` by move on
    // the FIRST render and never re-runs, because a plain `String` prop is
    // not a reactive dependency — the memo has nothing to subscribe to. The
    // chart then freezes on whatever it was seeded with while the textarea
    // happily updates, which is exactly how this shipped the first time.
    let rendered = use_memo(use_reactive!(|(source, snippet, layout_width)| engrave(
        &source,
        snippet,
        layout_width
    )));

    match &*rendered.read() {
        // Output of our own serialiser over our own parser — not user HTML.
        Ok(svg) => rsx! { div { class: "kf-chart", dangerous_inner_html: "{svg}" } },
        Err(message) => rsx! {
            div { class: "kf-chart kf-chart-failed",
                p { class: "kf-chart-error", "{message}" }
            }
        },
    }
}

/// Lay out and serialise one chart.
///
/// Split out so it can be tested without a renderer — it is the whole of
/// what this module does that can be wrong.
fn engrave(source: &str, snippet: bool, layout_width: f64) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("Nothing to engrave yet.".to_string());
    }
    let mut manager = ChartLayoutManager::new()?;
    manager.parse_and_layout(source, layout_width, snippet)?;

    // Snippets crop to the music; pages crop to the page. A guide example
    // laid out on A4 and cropped to the page is two centimetres of chart
    // above a screenful of blank paper.
    let svg = if snippet {
        manager.export_svg_snippet()?
    } else {
        manager
            .export_svg_pages_linked()?
            .into_iter()
            .next()
            .ok_or_else(|| "The chart laid out to no pages.".to_string())?
    };

    Ok(inline_ready(&svg))
}

/// Strip the XML prolog so the document can be inlined into HTML.
///
/// The serialiser emits a standalone `.svg` file, which opens with
/// `<?xml …?>`. That is correct for a file and invalid inside an HTML
/// element — the browser renders it as stray text above the chart.
pub(crate) fn inline_ready(svg: &str) -> String {
    let body = match svg.find("<svg") {
        Some(at) => &svg[at..],
        None => svg,
    };
    body.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engraves_a_chart_to_svg() {
        let svg = engrave(keyflow_ui::examples::EXAMPLE_THRILLER, true, 900.0).unwrap();
        assert!(svg.starts_with("<svg"), "not an SVG document: {:.40}", svg);
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn linked_output_does_not_carry_the_fonts() {
        // The whole point of the linked variant. If this regresses, every
        // chart on a page costs another ~485 KB of duplicated font data.
        for source in [
            keyflow_ui::examples::EXAMPLE_THRILLER,
            "VS: | 1 4 | 5 6- |\n",
        ] {
            let svg = engrave(source, true, 900.0).unwrap();
            assert!(
                !svg.contains("@font-face"),
                "linked SVG embedded the fonts anyway ({} bytes)",
                svg.len()
            );
        }
    }

    #[test]
    fn a_snippet_is_cropped_to_its_music() {
        // A two-bar example on an uncropped A4 page is mostly blank paper.
        // The viewBox height is the thing that proves it was cropped.
        let svg = engrave("VS: | 1 4 |\n", true, 900.0).unwrap();
        let view_box = svg
            .split_once("viewBox=\"")
            .and_then(|(_, r)| r.split_once('"'))
            .map(|(v, _)| v.to_owned())
            .expect("the SVG carries a viewBox");
        let height: f64 = view_box
            .split_whitespace()
            .nth(3)
            .and_then(|h| h.parse().ok())
            .expect("the viewBox has four numbers");
        assert!(
            height < 400.0,
            "a two-bar snippet has a viewBox {height} tall — it was cropped to the page"
        );
    }

    #[test]
    fn a_guide_sized_example_is_small() {
        // Guide examples are a bar or two. If one of these ever costs
        // hundreds of kilobytes, the linked-font path has regressed — a
        // full song legitimately does not fit this budget, which is why
        // this measures a fragment and not Thriller.
        let svg = engrave("VS: | 1 4 | 5 6- |\n", true, 900.0).unwrap();
        assert!(
            svg.len() < 60_000,
            "a two-bar example is {} bytes",
            svg.len()
        );
    }

    #[test]
    fn the_svg_can_be_inlined_into_html() {
        // Inlined into a <div>, so an XML prolog would render as stray text
        // above the chart.
        let svg = engrave("VS: | 1 4 |\n", true, 900.0).unwrap();
        assert!(svg.starts_with("<svg"), "leading junk: {:.60}", svg);
        assert!(!svg.contains("<?xml"));
    }

    #[test]
    fn every_font_the_svg_asks_for_is_declared_in_the_css() {
        // The bug this exists to catch: the chart scene emits chord
        // symbols as `MuseJazz Text` — WITH a space, matching the font's
        // internal name — while the exporters declared `MuseJazzText`.
        // The family never resolved, chord symbols fell back to a system
        // sans, and every `maj7` triangle rendered blank. Same class of
        // error had Leland's bytes filed under `Bravura`.
        //
        // A missing @font-face does not error; it silently falls back. So
        // the invariant has to be asserted, not eyeballed.
        let mut manager = ChartLayoutManager::new().unwrap();
        manager
            .parse_and_layout("VS: | Cmaj7 F#m7b5 | Bbmaj9 G7b9 |\n", 900.0, true)
            .unwrap();
        let svg = manager.export_svg_snippet().unwrap();
        let css = manager.font_face_css();

        for family in svg_font_families(&svg) {
            // Generic CSS families are the browser's job, not ours.
            if matches!(family.as_str(), "sans-serif" | "serif" | "monospace") {
                continue;
            }
            assert!(
                css.contains(&format!("font-family: '{family}'")),
                "the chart asks for `{family}` but no @font-face declares it — \
                 it will silently fall back to a system font"
            );
        }
    }

    #[test]
    fn the_chord_font_is_declared_under_the_name_the_scene_uses() {
        // Pinned separately because it is the specific regression, and
        // because "MuseJazz Text" vs "MuseJazzText" is exactly the kind of
        // difference a reviewer's eye slides over.
        let css = ChartLayoutManager::new().unwrap().font_face_css();
        assert!(
            css.contains("font-family: 'MuseJazz Text'"),
            "the spaced chord-font family is missing from the font CSS"
        );
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
    fn an_empty_chart_reports_rather_than_rendering_nothing() {
        assert!(engrave("   \n", true, 900.0).is_err());
    }

    #[test]
    fn every_engraved_guide_fence_still_engraves() {
        // A ```kf+ fence is the guide asserting "this is a real chart".
        // The editor renders these now, but the claim is the same, and
        // this is what notices when a guide example stops being valid.
        for page in crate::guide::GUIDE_PAGES {
            for source in engraved_fences(page.source) {
                assert!(
                    engrave(&source, true, 900.0).is_ok(),
                    "guide page `{}` has a kf+ fence that will not engrave:\n{source}",
                    page.slug
                );
            }
        }
    }

    /// Bodies of the ```kf+ fences in a note — the ones the guide marks
    /// as real charts rather than syntax illustrations.
    fn engraved_fences(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = source;
        while let Some(i) = rest.find("```kf+\n") {
            let after = &rest[i + 7..];
            let Some(j) = after.find("\n```") else { break };
            out.push(after[..j].to_string());
            rest = &after[j..];
        }
        out
    }
}
