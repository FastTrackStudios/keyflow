//! The chart preview — true-size pages, pan, zoom, and export.
//!
//! This is a port of the studio preview from the pre-split FastTrackStudio
//! site (`apps/site/src/components/live_editor.rs` plus the `ExportButton`
//! in `chart_editor.rs`), which had solved several problems the first
//! version of this file re-invented worse:
//!
//! - **Pages at true paper size.** A chart laid out for A4 is shown at A4,
//!   not stretched to whatever the pane happens to be. It is the page you
//!   are about to print.
//! - **A debounced re-render with a generation counter.** Engraving on
//!   every keystroke blocks the main thread; a newer edit supersedes a
//!   pending render rather than queueing behind it.
//! - **Font-less preview SVGs.** The `@font-face` block is injected once
//!   and each pass serialises paths, not several MB of re-embedded font
//!   data per keystroke.
//! - **Export off the render path**, with multi-page SVG as a zip and the
//!   filename taken from the chart's own title.
//!
//! What is *not* ported is that site's own 549-line `ChartLayoutManager`.
//! It predates the consolidation into `keyflow-ui`, and reviving it would
//! reintroduce exactly the drift that left its SVG export declaring
//! `MuseJazzText` while its PDF export carried a comment explaining the
//! family is really `MuseJazz Text`. One layout manager, one font list.

use dioxus::prelude::*;
use keyflow_ui::ChartLayoutManager;

/// Points → CSS pixels. Screen DPI over the typographic 72.
const DPI_SCALE: f64 = 96.0 / 72.0;

/// Zoom bounds, matching keyflow-ui's native chart viewports.
const ZOOM_MIN: f64 = 0.1;
const ZOOM_MAX: f64 = 8.0;

/// Gap between stacked pages, in CSS px.
const PAGE_GAP_PX: f64 = 24.0;

/// Idle delay before re-engraving. Long enough that a run of keystrokes
/// costs one render, short enough to feel live.
#[cfg(target_arch = "wasm32")]
const DEBOUNCE_MS: u32 = 150;

/// One engraved page: real dimensions in CSS px, plus its markup.
#[derive(Clone, PartialEq)]
struct Page {
    width_px: f64,
    height_px: f64,
    svg: String,
}

/// A blank sheet.
///
/// Shown before the first render lands, and whenever the source does not
/// yet lay out — still typing the title, or a parse error. The page should
/// never simply vanish while someone is mid-thought.
fn blank_page() -> Page {
    Page {
        width_px: 595.0 * DPI_SCALE,
        height_px: 842.0 * DPI_SCALE,
        svg: String::new(),
    }
}

/// A chart you can move around, zoom, and take away.
#[component]
pub fn ChartPreview(
    /// Chart source. Re-engraved on a debounce; pan and zoom survive.
    source: String,
) -> Element {
    let mut pages = use_signal(|| vec![blank_page()]);
    let mut generation = use_signal(|| 0_u64);
    let mut exporting = use_signal(|| false);
    let mut status = use_signal(|| Option::<String>::None);

    // Debounced re-render. The generation counter is what makes it
    // debounced rather than merely delayed: a render that is no longer the
    // newest drops itself on wake instead of overwriting a fresher one.
    use_effect(use_reactive!(|(source)| {
        let mine = generation.peek().wrapping_add(1);
        generation.set(mine);
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(DEBOUNCE_MS).await;
            if *generation.peek() != mine {
                return;
            }
            let rendered = engrave_pages(&source).unwrap_or_else(|e| {
                tracing::debug!("chart preview render failed: {e}");
                vec![blank_page()]
            });
            pages.set(rendered);
        });
    }));

    let mut zoom = use_signal(|| 0.75_f64);
    let mut pan = use_signal(|| (24.0_f64, 24.0_f64));
    let mut dragging = use_signal(|| false);
    let mut last = use_signal(|| (0.0_f64, 0.0_f64));

    let src_for_export = source.clone();

    rsx! {
        div { class: "kf-preview",
            div { class: "kf-preview-bar",
                button {
                    class: "kf-button",
                    onclick: move |_| *zoom.write() = (zoom() * 1.25).min(ZOOM_MAX),
                    "+"
                }
                button {
                    class: "kf-button",
                    onclick: move |_| *zoom.write() = (zoom() / 1.25).max(ZOOM_MIN),
                    "−"
                }
                button {
                    class: "kf-button",
                    onclick: move |_| {
                        zoom.set(0.75);
                        pan.set((24.0, 24.0));
                    },
                    "Reset"
                }
                span { class: "kf-note", "{(zoom() * 100.0).round()}%" }

                span { class: "kf-preview-spacer" }

                ExportButton {
                    source: src_for_export,
                    exporting,
                    status,
                }
            }

            div {
                class: "kf-preview-stage",

                // Wheel → zoom, anchored at the cursor so the chart grows
                // around what you are looking at rather than the corner.
                onwheel: move |e| {
                    e.prevent_default();
                    let old = zoom();
                    let factor = if e.delta().strip_units().y < 0.0 { 1.05 } else { 0.95 };
                    let new = (old * factor).clamp(ZOOM_MIN, ZOOM_MAX);
                    let at = e.element_coordinates();
                    let (px, py) = pan();
                    let change = new / old;
                    pan.set((
                        at.x - (at.x - px) * change,
                        at.y - (at.y - py) * change,
                    ));
                    zoom.set(new);
                },

                onmousedown: move |e| {
                    dragging.set(true);
                    let c = e.client_coordinates();
                    last.set((c.x, c.y));
                },
                onmousemove: move |e| {
                    if !dragging() {
                        return;
                    }
                    let c = e.client_coordinates();
                    let (lx, ly) = last();
                    let (px, py) = pan();
                    pan.set((px + (c.x - lx), py + (c.y - ly)));
                    last.set((c.x, c.y));
                },
                onmouseup: move |_| dragging.set(false),
                onmouseleave: move |_| dragging.set(false),

                div {
                    class: "kf-preview-pages",
                    style: "transform: translate({pan().0}px, {pan().1}px) scale({zoom()}); transform-origin: 0 0;",
                    for (i, page) in pages.read().iter().enumerate() {
                        div {
                            key: "{i}",
                            class: "kf-preview-page",
                            style: "width: {page.width_px}px; height: {page.height_px}px; margin-bottom: {PAGE_GAP_PX}px;",
                            div { dangerous_inner_html: "{page.svg}" }
                        }
                    }
                }
            }

            if let Some(message) = status() {
                p { class: "kf-chart-error", "{message}" }
            }
        }
    }
}

/// Export to PDF, or to SVG (a zip when the chart runs to several pages).
#[component]
fn ExportButton(
    source: String,
    exporting: Signal<bool>,
    status: Signal<Option<String>>,
) -> Element {
    let mut exporting = exporting;
    let mut status = status;

    // A big chart's PDF takes long enough to notice, and this runs on the
    // main thread — so it goes through `spawn` with a visible pending
    // state rather than freezing the tab mid-click.
    let mut run = move |format: &'static str, source: String| {
        if exporting() {
            return;
        }
        exporting.set(true);
        status.set(None);
        spawn(async move {
            let result = match format {
                "pdf" => export_pdf(&source),
                _ => export_svg(&source),
            };
            match result {
                Ok((bytes, mime, ext)) => {
                    let name = format!("{}.{ext}", filename_for(&source));
                    status.set(download(&name, mime, &bytes).err());
                }
                Err(e) => status.set(Some(e)),
            }
            exporting.set(false);
        });
    };

    let svg_source = source.clone();
    let pdf_source = source;

    rsx! {
        button {
            class: "kf-button",
            disabled: exporting(),
            onclick: move |_| run("svg", svg_source.clone()),
            "SVG"
        }
        button {
            class: "kf-button kf-button-primary",
            disabled: exporting(),
            onclick: move |_| run("pdf", pdf_source.clone()),
            if exporting() { "Exporting…" } else { "PDF" }
        }
    }
}

/// Lay the chart out and serialise each page at its true size.
fn engrave_pages(source: &str) -> Result<Vec<Page>, String> {
    if source.trim().is_empty() {
        return Err("nothing to engrave".to_string());
    }
    let mut manager = ChartLayoutManager::new()?;
    // Paginated (`snippet = false`) — the viewport-width argument is
    // ignored in that mode, since page layout is always paper-sized.
    manager.parse_and_layout(source, 800.0, false)?;

    let pages: Vec<Page> = manager
        .export_svg_pages_sized()?
        .into_iter()
        .map(|(w_pt, h_pt, svg)| Page {
            width_px: w_pt * DPI_SCALE,
            height_px: h_pt * DPI_SCALE,
            svg: crate::chart_view::inline_ready(&svg),
        })
        .collect();

    Ok(if pages.is_empty() {
        vec![blank_page()]
    } else {
        pages
    })
}

/// Filename stem: the chart's own title, or `chart`.
fn filename_for(source: &str) -> String {
    keyflow::text::chart::parse_chart(source)
        .ok()
        .and_then(|c| c.metadata.title.clone())
        .map_or_else(|| "chart".to_string(), |t| t.trim().replace(' ', "_"))
}

/// A laid-out manager ready to export from.
fn laid_out(source: &str) -> Result<ChartLayoutManager, String> {
    let mut manager = ChartLayoutManager::new()?;
    manager.parse_and_layout(source, 595.0, false)?;
    Ok(manager)
}

/// SVG export: one file, or a zip of pages.
fn export_svg(source: &str) -> Result<(Vec<u8>, &'static str, &'static str), String> {
    // The EMBEDDING variant: this file leaves the page, so it cannot rely
    // on a stylesheet that stays behind.
    let pages = laid_out(source)?.export_svg_pages()?;
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

/// PDF export: one multi-page vector document.
fn export_pdf(source: &str) -> Result<(Vec<u8>, &'static str, &'static str), String> {
    Ok((
        laid_out(source)?.export_pdf_bytes()?,
        "application/pdf",
        "pdf",
    ))
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

/// Hand `bytes` to the browser as a download.
#[cfg(target_arch = "wasm32")]
fn download(filename: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast as _;

    let win = web_sys::window().ok_or("no window")?;
    let doc = win.document().ok_or("no document")?;

    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&array);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|_| "could not build the download".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "could not build the download".to_string())?;

    let anchor: web_sys::HtmlAnchorElement = doc
        .create_element("a")
        .map_err(|_| "could not build the download".to_string())?
        .dyn_into()
        .map_err(|_| "could not build the download".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    // The click has already started the download; leaving the object URL
    // would leak the blob for the tab's lifetime.
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Host builds have no browser to download into.
#[cfg(not(target_arch = "wasm32"))]
fn download(_filename: &str, _mime: &str, _bytes: &[u8]) -> Result<(), String> {
    Err("Downloads are only available in the browser.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_BARS: &str = "VS: | 1 4 | 5 6- |\n";

    #[test]
    fn engraves_pages_at_true_paper_size() {
        let pages = engrave_pages(keyflow_ui::examples::EXAMPLE_THRILLER).unwrap();
        assert!(!pages.is_empty());
        // A4 portrait at 96 DPI is ~793x1122 CSS px. The point of the sized
        // export is that this is paper, not a stretched thumbnail.
        assert!(
            pages[0].width_px > 500.0 && pages[0].height_px > pages[0].width_px,
            "expected a portrait page, got {}x{}",
            pages[0].width_px,
            pages[0].height_px
        );
    }

    #[test]
    fn an_empty_chart_reports_rather_than_rendering_nothing() {
        assert!(engrave_pages("  \n").is_err());
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
    fn the_preview_svg_does_not_carry_the_fonts() {
        // The other half of the trade: re-embedding ~4 MB of font data on
        // every debounced keystroke is what this avoids.
        let pages = engrave_pages(TWO_BARS).unwrap();
        assert!(!pages[0].svg.contains("@font-face"));
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
            // Single-page is a legitimate outcome for this fixture; the zip
            // path is covered directly below.
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
}
