//! The chart preview — pan, zoom, and export.
//!
//! [`crate::chart_view::ChartSvg`] draws a chart at a fixed size, which is
//! right inline in a guide chapter. A working surface needs more: a full
//! score is wider and taller than any pane, so it has to be pannable and
//! zoomable, and the point of writing a chart is eventually to get a PDF
//! of it onto a music stand.
//!
//! Pan and zoom are done in CSS over the engraved SVG rather than by
//! re-laying out. Layout costs ~10 ms and the SVG is resolution
//! independent, so dragging and scaling is free and stays sharp — there is
//! nothing to re-render until the *source* changes.
//!
//! Export goes through `ChartLayoutManager`, which already knows how to
//! produce both formats. PDF needs the `pdf` feature; it is off by default
//! because printpdf's azul dependency does not build for iOS, but it does
//! build for wasm32, so on the web the download is a real vector PDF
//! rather than a trip through the browser's print dialog.

use dioxus::prelude::*;
use keyflow_ui::ChartLayoutManager;

/// A chart you can move around and take away.
#[component]
pub fn ChartPreview(
    /// Chart source. Re-engraved when it changes; pan and zoom survive.
    source: String,
    /// Filename stem for downloads.
    #[props(default = "chart".to_string())]
    name: String,
) -> Element {
    let rendered = use_memo(use_reactive!(|(source)| engrave_pages(&source)));

    let mut zoom = use_signal(|| 1.0_f64);
    let mut pan = use_signal(|| (0.0_f64, 0.0_f64));
    let mut dragging = use_signal(|| Option::<(f64, f64)>::None);
    let mut status = use_signal(|| Option::<String>::None);

    let src_for_export = source.clone();

    rsx! {
        div { class: "kf-preview",
            div { class: "kf-preview-bar",
                button {
                    class: "kf-button",
                    onclick: move |_| *zoom.write() = (zoom() * 1.25).min(8.0),
                    "+"
                }
                button {
                    class: "kf-button",
                    onclick: move |_| *zoom.write() = (zoom() / 1.25).max(0.15),
                    "−"
                }
                button {
                    class: "kf-button",
                    onclick: move |_| {
                        zoom.set(1.0);
                        pan.set((0.0, 0.0));
                    },
                    "Fit"
                }
                span { class: "kf-note", "{(zoom() * 100.0).round()}%" }

                span { class: "kf-preview-spacer" }

                button {
                    class: "kf-button",
                    onclick: {
                        let src = src_for_export.clone();
                        let name = name.clone();
                        move |_| {
                            let r = export_svg(&src).and_then(|svg| {
                                download(&format!("{name}.svg"), "image/svg+xml", svg.as_bytes())
                            });
                            status.set(r.err());
                        }
                    },
                    "SVG"
                }
                button {
                    class: "kf-button kf-button-primary",
                    onclick: {
                        let src = src_for_export.clone();
                        let name = name.clone();
                        move |_| {
                            let r = export_pdf(&src).and_then(|bytes| {
                                download(&format!("{name}.pdf"), "application/pdf", &bytes)
                            });
                            status.set(r.err());
                        }
                    },
                    "PDF"
                }
            }

            div {
                class: "kf-preview-stage",
                // Drag to pan. Pointer events (not mouse) so this works
                // with a trackpad, a pen and a touchscreen alike.
                onpointerdown: move |e| {
                    let c = e.client_coordinates();
                    dragging.set(Some((c.x - pan().0, c.y - pan().1)));
                },
                onpointermove: move |e| {
                    if let Some((ox, oy)) = dragging() {
                        let c = e.client_coordinates();
                        pan.set((c.x - ox, c.y - oy));
                    }
                },
                onpointerup: move |_| dragging.set(None),
                onpointerleave: move |_| dragging.set(None),

                match &*rendered.read() {
                    Ok(pages) => rsx! {
                        div {
                            class: "kf-preview-pages",
                            style: "transform: translate({pan().0}px, {pan().1}px) scale({zoom()}); ",
                            for (i, page) in pages.iter().enumerate() {
                                div { key: "{i}", class: "kf-preview-page",
                                    dangerous_inner_html: "{page}" }
                            }
                        }
                    },
                    Err(message) => rsx! {
                        p { class: "kf-chart-error", "{message}" }
                    },
                }
            }

            if let Some(message) = status() {
                p { class: "kf-chart-error", "{message}" }
            }
        }
    }
}

/// Lay the chart out as pages, ready to inline.
fn engrave_pages(source: &str) -> Result<Vec<String>, String> {
    if source.trim().is_empty() {
        return Err("Nothing to engrave yet.".to_string());
    }
    let mut manager = ChartLayoutManager::new()?;
    manager.parse_and_layout(source, 900.0, false)?;
    Ok(manager
        .export_svg_pages_linked()?
        .iter()
        .map(|svg| crate::chart_view::inline_ready(svg))
        .collect())
}

/// A standalone SVG — fonts embedded, so the file works anywhere.
fn export_svg(source: &str) -> Result<String, String> {
    let mut manager = ChartLayoutManager::new()?;
    manager.parse_and_layout(source, 900.0, false)?;
    // The EMBEDDING variant here, deliberately: this leaves the page, so
    // it cannot rely on a stylesheet it will not travel with.
    manager
        .export_svg_pages()?
        .into_iter()
        .next()
        .ok_or_else(|| "The chart laid out to no pages.".to_string())
}

/// A multi-page vector PDF.
fn export_pdf(source: &str) -> Result<Vec<u8>, String> {
    let mut manager = ChartLayoutManager::new()?;
    manager.parse_and_layout(source, 900.0, false)?;
    manager.export_pdf_bytes()
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

    // Revoking immediately is safe: the click has already started the
    // download, and leaving it would leak the blob for the tab's lifetime.
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

    #[test]
    fn engraves_a_chart_to_pages() {
        let pages = engrave_pages(keyflow_ui::examples::EXAMPLE_THRILLER).unwrap();
        assert!(!pages.is_empty(), "a real chart should lay out to pages");
        assert!(pages[0].starts_with("<svg"));
    }

    #[test]
    fn an_empty_chart_reports_rather_than_rendering_nothing() {
        assert!(engrave_pages("  \n").is_err());
    }

    #[test]
    fn the_downloadable_svg_is_self_contained() {
        // It leaves the page, so it cannot depend on a stylesheet that
        // stays behind — this is the one place the embedding export is
        // the right call.
        let svg = export_svg("VS: | 1 4 | 5 6- |\n").unwrap();
        assert!(
            svg.contains("@font-face"),
            "a downloaded SVG must carry its own fonts"
        );
    }

    #[test]
    fn the_pdf_export_produces_a_pdf() {
        let bytes = export_pdf("VS: | 1 4 | 5 6- |\n").unwrap();
        assert!(
            bytes.starts_with(b"%PDF"),
            "export_pdf did not produce a PDF document"
        );
    }
}
