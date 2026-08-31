//! Engraving a chart for the page — the one place the site does it.
//!
//! `keyflow-ui` deliberately stops short of this: it hands back a
//! `ChartLayoutManager` that lays a chart out and can serialise it, and
//! leaves presentation to the host app. That split is why the notation
//! domain has no dependency on any application — so this is the site's
//! job, and it lives here.
//!
//! Everything that turns chart *source* into chart *pixels* goes through
//! [`engrave`], and every chart on the page is one [`Chart`] component
//! with a [`ChartShape`]. Before this there were two components with two
//! engrave functions at two different crops, five separate layout-manager
//! constructions across two files, and three layout widths that had each
//! been picked locally — which is how the site ended up asking for a font
//! by a name nothing declared.
//!
//! # Why SVG and not the GPU canvas
//!
//! `keyflow-ui` also ships a WebGL surface (`ChartGraphics`), and the
//! first version of this used it. It was the wrong tool, for a reason
//! worth keeping written down: a WebGL context is a scarce, stateful
//! resource, and every chart on a page wanted its own. Browsers cap live
//! contexts at around sixteen, a guide page creates several, and
//! re-creating one on each re-render wedged the renderer outright.
//!
//! Nothing here needs a GPU. Layout costs about 10 ms and the output is
//! *static* until the source changes, so it is serialised once to SVG and
//! the browser scales, prints and selects it for free. The GPU path stays
//! in `keyflow-ui` for the case it was built for: a cursor tracking
//! playback at 120 Hz in the desktop app.
//!
//! # Fonts
//!
//! The embedding exports carry ~485 KB of font data per document, which
//! is right for a file someone downloads and wrong for a page with four
//! charts on it. So everything on-screen uses the *linked* exports and
//! [`ChartFonts`] emits the `@font-face` block once per document;
//! [`export_svg`] and [`export_pdf`] — whose output leaves the page — use
//! the embedding ones.

use dioxus::prelude::*;
use keyflow::engraver::api::pipeline::Paper;
use keyflow_ui::ChartLayoutManager;

/// Points → CSS pixels. Screen DPI over the typographic 72.
pub const DPI_SCALE: f64 = 96.0 / 72.0;

/// Layout width handed to the engine, in points.
///
/// It decides how much music fits on a system in [`ChartShape::Inline`],
/// and is ignored outright in [`ChartShape::Page`], where paper size
/// decides. One value, so a chart cannot break differently depending on
/// which caller engraved it.
const LAYOUT_WIDTH_PT: f64 = 900.0;

/// How a chart is laid out and framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartShape {
    /// Continuous, cropped to the music, scaled to its container. For a
    /// chart inside prose, where the surrounding text sets the measure —
    /// a two-bar example on an uncropped A4 page is a centimetre of music
    /// above a screenful of blank paper.
    Inline,
    /// Paginated at true paper size. For a working surface, where the
    /// point is to see the page you are about to print.
    Page,
}

/// One engraved page: its real size in CSS px, and its markup.
#[derive(Clone, PartialEq)]
pub struct Page {
    pub width_px: f64,
    pub height_px: f64,
    pub svg: String,
}

/// A blank sheet.
///
/// Shown whenever the source does not yet lay out — still typing the
/// title, or a parse error. The page should never simply vanish while
/// someone is mid-thought.
pub fn blank_page() -> Page {
    Page {
        width_px: 595.0 * DPI_SCALE,
        height_px: 842.0 * DPI_SCALE,
        svg: String::new(),
    }
}

/// The chart fonts, declared once for the whole page.
///
/// Render this once, above any [`Chart`]. Without it the musical glyphs
/// render as tofu.
#[component]
pub fn ChartFonts() -> Element {
    // Static across the life of the page: the bundle is baked into the
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
/// Renders `source` and nothing else — no cursor, no playback, no dock.
/// The panels that wired those together used to live in `keyflow-ui` and
/// were deleted when the notation domain was extracted; anything that
/// couples a chart to *application* state belongs on this side. For a
/// chart someone drives — pan, zoom, export — see
/// [`ChartPreview`](crate::chart_preview::ChartPreview), which is this
/// same engraving under an interactive shell.
#[component]
pub fn Chart(
    /// Chart source. Re-engraved when it changes.
    source: String,
    /// How to lay it out and frame it.
    #[props(default = ChartShape::Inline)]
    shape: ChartShape,
    /// Keep the last chart that engraved when the source stops parsing,
    /// instead of reporting the error.
    ///
    /// For a source that is *being written* — the hero's typewriter, a
    /// live editor — where most intermediate states are half a chord and
    /// do not parse. Reporting each one turns the pane into a strobe of
    /// error text; holding the last good engraving reads as the chart
    /// keeping up.
    ///
    /// Off by default, and it should stay off anywhere the source is
    /// finished: a chart that silently shows the previous chart's music
    /// is worse than one that says what is wrong with it.
    #[props(default = false)]
    hold_last_good: bool,
) -> Element {
    // `use_memo` and not `use_effect`: engraving is a pure function of the
    // source, so it belongs in the render path. The earlier effect-based
    // version wrote a signal the component also read, which re-entered on
    // every pass.
    //
    // `use_reactive!` is load-bearing, not decoration. A bare
    // `use_memo(move || engrave(&source, ..))` captures `source` by move
    // on the FIRST render and never re-runs, because a plain `String` prop
    // is not a reactive dependency — the memo has nothing to subscribe to.
    // The chart then freezes on whatever it was seeded with while the
    // editor happily updates, which is exactly how this shipped the first
    // time.

    // The last engraving that succeeded, for `hold_last_good`. Written
    // from inside the memo and read only through `peek`, so nothing
    // subscribes to it — which is what keeps a write during render from
    // re-entering the pass that made it.
    let mut last_good = use_signal(Vec::<Page>::new);

    let rendered = use_memo(use_reactive!(|(source, shape, hold_last_good)| {
        match engrave(&source, shape) {
            Ok(pages) => {
                if hold_last_good {
                    last_good.set(pages.clone());
                }
                Ok(pages)
            }
            Err(message) => {
                let held = last_good.peek();
                if hold_last_good && !held.is_empty() {
                    Ok(held.clone())
                } else {
                    Err(message)
                }
            }
        }
    }));

    match &*rendered.read() {
        // Output of our own serialiser over our own parser, not user HTML.
        Ok(pages) => rsx! {
            div { class: "kf-chart",
                for (i, page) in pages.iter().enumerate() {
                    div {
                        key: "{i}",
                        class: "kf-chart-page",
                        // The page's *proportions*, not its pixel size.
                        // A4 is 793x1123 CSS px, which is taller than
                        // most places a chart appears; pinning those
                        // numbers means every container has to be that
                        // big or clip. `aspect-ratio` says the same
                        // thing — this is paper, in paper's shape — and
                        // lets the container pick the scale, so the same
                        // markup is a full-size working page in one
                        // place and a thumbnail in another.
                        style: match shape {
                            ChartShape::Page => {
                                format!("aspect-ratio: {} / {};", page.width_px, page.height_px)
                            }
                            ChartShape::Inline => String::new(),
                        },
                        dangerous_inner_html: "{page.svg}",
                    }
                }
            }
        },
        Err(message) => rsx! {
            div { class: "kf-chart kf-chart-failed",
                p { class: "kf-chart-error", "{message}" }
            }
        },
    }
}

/// Lay a chart out and serialise it for the screen.
///
/// Split out so it can be tested without a renderer — it is the whole of
/// what this module does that can be wrong.
pub fn engrave(source: &str, shape: ChartShape) -> Result<Vec<Page>, String> {
    let manager = laid_out(source, shape)?;

    match shape {
        ChartShape::Inline => Ok(vec![Page {
            width_px: 0.0,
            height_px: 0.0,
            svg: inline_ready(&manager.export_svg_snippet()?),
        }]),
        ChartShape::Page => {
            let pages: Vec<Page> = manager
                .export_svg_pages_sized()?
                .into_iter()
                .map(|(w_pt, h_pt, svg)| Page {
                    width_px: w_pt * DPI_SCALE,
                    height_px: h_pt * DPI_SCALE,
                    svg: inline_ready(&svg),
                })
                .collect();
            Ok(if pages.is_empty() {
                vec![blank_page()]
            } else {
                pages
            })
        }
    }
}

/// A manager holding a laid-out chart — the single place the site builds
/// one, so every caller gets the same layout for the same source.
///
/// A4, explicitly. The on-screen preset defaults to Letter, and this site
/// does not: [`blank_page`] is 595x842 and [`export_svg`]/[`export_pdf`]
/// hand out A4, so a Letter layout on screen meant the page someone was
/// looking at broke its systems in different places from the page they
/// downloaded.
fn laid_out(source: &str, shape: ChartShape) -> Result<ChartLayoutManager, String> {
    if source.trim().is_empty() {
        return Err("Nothing to engrave yet.".to_string());
    }
    let mut manager = ChartLayoutManager::new()?.with_paper(Paper::A4);
    manager.parse_and_layout(source, LAYOUT_WIDTH_PT, shape == ChartShape::Inline)?;
    Ok(manager)
}

/// Export a chart as SVG: one file, or a zip when it runs to several
/// pages.
///
/// Returns the bytes, their MIME type, and the extension to save under.
pub fn export_svg(source: &str) -> Result<(Vec<u8>, &'static str, &'static str), String> {
    // The EMBEDDING variant: this file leaves the page, so it cannot rely
    // on a stylesheet that stays behind.
    let pages = laid_out(source, ChartShape::Page)?.export_svg_pages()?;
    match pages.len() {
        0 => Err("the chart laid out to no pages".to_string()),
        1 => Ok((
            pages.into_iter().next().expect("len == 1").into_bytes(),
            "image/svg+xml",
            "svg",
        )),
        _ => Ok((
            zip_pages(&pages, &filename_for(source))?,
            "application/zip",
            "zip",
        )),
    }
}

/// Export a chart as one multi-page vector PDF.
pub fn export_pdf(source: &str) -> Result<(Vec<u8>, &'static str, &'static str), String> {
    Ok((
        laid_out(source, ChartShape::Page)?.export_pdf_bytes()?,
        "application/pdf",
        "pdf",
    ))
}

/// Filename stem for an export: the chart's own title, or `chart`.
pub fn filename_for(source: &str) -> String {
    keyflow::parse(source)
        .ok()
        .and_then(|c| c.metadata.title.clone())
        .map_or_else(|| "chart".to_string(), |t| t.trim().replace(' ', "_"))
}

/// Bundle per-page SVGs into a zip.
fn zip_pages(pages: &[String], stem: &str) -> Result<Vec<u8>, String> {
    use std::io::{Cursor, Write as _};

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        for (i, svg) in pages.iter().enumerate() {
            zip.start_file(format!("{stem}_page{}.svg", i + 1), options)
                .map_err(|e| format!("could not build the zip: {e}"))?;
            zip.write_all(svg.as_bytes())
                .map_err(|e| format!("could not build the zip: {e}"))?;
        }
        zip.finish()
            .map_err(|e| format!("could not build the zip: {e}"))?;
    }
    Ok(buffer.into_inner())
}

/// Strip the XML prolog so the document can be inlined into HTML.
///
/// The serialiser emits a standalone `.svg` file, which opens with
/// `<?xml …?>`. That is correct for a file and invalid inside an HTML
/// element — the browser renders it as stray text above the chart.
fn inline_ready(svg: &str) -> String {
    match svg.find("<svg") {
        Some(at) => svg[at..].to_string(),
        None => svg.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_BARS: &str = "VS: | 1 4 | 5 6- |\n";

    #[test]
    fn engraves_an_inline_snippet() {
        let pages = engrave(keyflow_ui::examples::EXAMPLE_THRILLER, ChartShape::Inline).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].svg.starts_with("<svg"));
        assert!(pages[0].svg.contains("</svg>"));
    }

    #[test]
    fn engraves_pages_at_true_paper_size() {
        let pages = engrave(keyflow_ui::examples::EXAMPLE_THRILLER, ChartShape::Page).unwrap();
        assert!(!pages.is_empty());

        // A4 portrait — 595x842pt, so 793x1122 CSS px at 96 DPI — and not
        // merely "some portrait page". The engraver's on-screen preset
        // defaults to US Letter, which is also portrait and also wider
        // than 500px, so the loose assertion this replaces passed while
        // the site laid out on Letter and exported A4. Paper decides
        // system breaks; the two are different charts.
        let (w, h) = (pages[0].width_px, pages[0].height_px);
        assert!(
            (w - 595.0 * DPI_SCALE).abs() < 1.0 && (h - 842.0 * DPI_SCALE).abs() < 1.0,
            "expected an A4 page (793x1122 px), got {w:.0}x{h:.0} \
             — Letter is 816x1056",
        );
    }

    #[test]
    fn an_inline_snippet_is_cropped_to_its_music() {
        // A two-bar example on an uncropped A4 page is mostly blank paper.
        // The viewBox height is what proves it was cropped.
        let pages = engrave("VS: | 1 4 |\n", ChartShape::Inline).unwrap();
        let view_box = pages[0]
            .svg
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
    fn nothing_on_screen_carries_the_fonts() {
        // The whole point of the linked variants. If this regresses, every
        // chart on a page costs another ~485 KB of duplicated font data —
        // and the preview re-pays it on every debounced keystroke.
        for shape in [ChartShape::Inline, ChartShape::Page] {
            let pages = engrave(TWO_BARS, shape).unwrap();
            assert!(
                !pages[0].svg.contains("@font-face"),
                "{shape:?} embedded the fonts anyway ({} bytes)",
                pages[0].svg.len()
            );
        }
    }

    #[test]
    fn a_guide_sized_example_is_small() {
        // Guide examples are a bar or two. If one ever costs hundreds of
        // kilobytes, the linked-font path has regressed — a full song
        // legitimately does not fit this budget, which is why this
        // measures a fragment and not Thriller.
        let pages = engrave(TWO_BARS, ChartShape::Inline).unwrap();
        assert!(
            pages[0].svg.len() < 60_000,
            "a two-bar example is {} bytes",
            pages[0].svg.len()
        );
    }

    #[test]
    fn an_empty_chart_reports_rather_than_rendering_nothing() {
        for shape in [ChartShape::Inline, ChartShape::Page] {
            assert!(engrave("   \n", shape).is_err());
        }
    }

    #[test]
    fn the_svg_can_be_inlined_into_html() {
        // Inlined into a <div>, so an XML prolog would render as stray
        // text above the chart.
        let pages = engrave("VS: | 1 4 |\n", ChartShape::Inline).unwrap();
        assert!(pages[0].svg.starts_with("<svg"));
        assert!(!pages[0].svg.contains("<?xml"));
    }

    #[test]
    fn the_downloadable_svg_is_self_contained() {
        // It leaves the page, so it cannot depend on a stylesheet that
        // stays behind — the one place the embedding export is right.
        let (bytes, mime, ext) = export_svg(TWO_BARS).unwrap();
        assert_eq!((mime, ext), ("image/svg+xml", "svg"));
        let svg = String::from_utf8(bytes).unwrap();
        assert!(
            svg.contains("@font-face"),
            "a downloaded SVG must carry its own fonts"
        );
    }

    #[test]
    fn the_pdf_export_produces_a_pdf() {
        let (bytes, mime, ext) = export_pdf(TWO_BARS).unwrap();
        assert_eq!((mime, ext), ("application/pdf", "pdf"));
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn a_multi_page_chart_exports_as_a_zip() {
        let (bytes, mime, ext) = export_svg(keyflow_ui::examples::EXAMPLE_THRILLER).unwrap();
        if ext == "zip" {
            assert_eq!(mime, "application/zip");
            assert!(bytes.starts_with(b"PK"), "not a zip archive");
        } else {
            // Single-page is a legitimate outcome for this fixture; the
            // zip path is covered directly below.
            assert_eq!(ext, "svg");
        }
    }

    #[test]
    fn zipping_pages_produces_an_archive_per_page() {
        let zip = zip_pages(&["<svg/>".into(), "<svg/>".into()], "song").unwrap();
        assert!(zip.starts_with(b"PK"));
        let text = String::from_utf8_lossy(&zip);
        assert!(text.contains("song_page1.svg"));
        assert!(text.contains("song_page2.svg"));
    }

    #[test]
    fn the_filename_comes_from_the_chart_title() {
        assert_eq!(
            filename_for("Build My Life - Housefires\n"),
            "Build_My_Life"
        );
        assert_eq!(filename_for(TWO_BARS), "chart");
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
        let manager = laid_out("VS: | Cmaj7 F#m7b5 | Bbmaj9 G7b9 |\n", ChartShape::Inline).unwrap();
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

    #[test]
    fn every_engraved_guide_fence_still_engraves() {
        // A ```kf+ fence is the guide asserting "this is a real chart".
        // The editor renders these now, but the claim is the same, and
        // this is what notices when a guide example stops being valid.
        for page in crate::guide::GUIDE_PAGES {
            for source in engraved_fences(page.source) {
                assert!(
                    engrave(&source, ChartShape::Inline).is_ok(),
                    "guide page `{}` has a kf+ fence that will not engrave:\n{source}",
                    page.slug
                );
            }
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
