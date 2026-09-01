//! The chart preview — a chart you can move around, zoom, and take away.
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
//! - **Export off the render path**, with multi-page SVG as a zip and the
//!   filename taken from the chart's own title.
//!
//! What is left here is only that shell. The engraving itself — layout,
//! paper sizing, SVG and PDF serialisation, the filename — is
//! [`crate::chart`], shared with the static [`Chart`](crate::chart::Chart)
//! the guide and landing page use. This file had its own copy of all of
//! it, and that is the drift worth naming: the old site's parallel layout
//! manager is what left its SVG export declaring `MuseJazzText` while its
//! PDF export carried a comment explaining the family is really
//! `MuseJazz Text`. One layout manager, one font list, one engrave.

use dioxus::prelude::*;

use crate::chart::{ChartShape, Page, blank_page, engrave, export_pdf, export_svg, filename_for};
use crate::chart_gpu;
use crate::chart_url;

/// Zoom bounds, matching keyflow-ui's native chart viewports.
const ZOOM_MIN: f64 = 0.1;
const ZOOM_MAX: f64 = 8.0;

/// Gap between stacked pages, in CSS px.
const PAGE_GAP_PX: f64 = 24.0;

/// Idle delay before re-engraving. Long enough that a run of keystrokes
/// costs one render, short enough to feel live.
#[cfg(target_arch = "wasm32")]
const DEBOUNCE_MS: u32 = 150;

/// A chart you can move around, zoom, and take away.
#[component]
pub fn ChartPreview(
    /// Chart source. Re-engraved on a debounce; pan and zoom survive.
    source: String,
) -> Element {
    let mut pages = use_signal(|| vec![blank_page()]);
    let mut generation = use_signal(|| 0_u64);
    let exporting = use_signal(|| false);
    let status = use_signal(|| Option::<String>::None);

    // Debounced re-render. The generation counter is what makes it
    // debounced rather than merely delayed: a render that is no longer the
    // newest drops itself on wake instead of overwriting a fresher one.
    // (This is why the preview does not use `use_memo` the way `Chart`
    // does — the point is *not* to engrave on every pass.)
    use_effect(use_reactive!(|(source)| {
        let mine = generation.peek().wrapping_add(1);
        generation.set(mine);
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(DEBOUNCE_MS).await;
            if *generation.peek() != mine {
                return;
            }
            let rendered = engrave(&source, ChartShape::Page).unwrap_or_else(|e| {
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

    // One live surface, or none. Asked once: if WebGL2 is refused the
    // preview renders the same SVG pages the rest of the site does, and
    // nothing below ever touches a canvas.
    // OFF until hover is affordable.
    //
    // The picture is right — pages, staves, barlines, clefs, key
    // signatures, chord symbols, section cards, and (since the glyph
    // fallback in `scene_renderer`) the rhythm slashes. Camera, resize,
    // WebGL2 probing, context release and the SVG fallback all work.
    //
    // What is not ready is the cost of a pointer move. Two things happen
    // per mousemove and both are O(whole chart):
    //
    //   - `hit_test_at_point` walks the entire scene graph recursively
    //     across every page to find the beat under the cursor;
    //   - a redraw rebuilds the whole scene, because vello_hybrid is
    //     immediate-mode — there is no "repaint just the highlight".
    //
    // Skipping the redraw when the hovered beat is unchanged (see
    // `chart_gpu::draw`) was not enough on its own: the hit-test alone
    // still runs per event, and a fast sweep across the stage locks the
    // tab. The fix is to throttle hover to an animation frame and hold a
    // spatial index for hit-testing, and neither is a five-minute job.
    //
    // Flip to `true` to work on it. Everything else is in place.
    const USE_GPU_PREVIEW: bool = false;

    let gpu = USE_GPU_PREVIEW && use_hook(chart_gpu::webgl2_available);
    let surface = use_hook(chart_gpu::surface_cell);
    let mut hover = use_signal(|| None::<(f64, f64)>);
    let mut readout = use_signal(|| None::<String>);
    // Bumped whenever the canvas is laid out or resized. The first draw
    // would otherwise happen before the element has a size — a canvas
    // with no width attribute is 300x150, and the browser stretches that
    // buffer across the pane, which is a 14x magnification and looks
    // exactly like a bug in the renderer.
    let mut surface_size = use_signal(|| (0_i32, 0_i32));

    // Redraw whenever the source, the camera or the pointer moves. The
    // manager only re-lays-out when the source or the width changed, so a
    // pointer move costs a re-render of the scene and nothing more.
    {
        let surface = surface.clone();
        let src = source.clone();
        use_effect(move || {
            if !gpu {
                return;
            }
            // Read so the effect re-runs when the canvas is measured.
            let _ = surface_size();
            let text = readout.peek().clone();
            let next = chart_gpu::draw(&surface, &src, pan(), zoom(), hover());
            if next != text {
                readout.set(next);
            }
        });
    }

    // A WebGL context is not reclaimed just because its canvas left the
    // document, and they are capped per browser — so the surface goes
    // when the preview does.
    {
        let surface = surface.clone();
        use_drop(move || chart_gpu::release(&surface));
    }

    rsx! {
        div { class: "kf-preview",
            div { class: "kf-preview-bar kf-pane-head",
                span { class: "kf-pane-name", "Chart" }
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

                ShareButton {
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
                    // Stops the browser starting its own drag-select on
                    // the SVG text under the pointer. `user-select: none`
                    // on the stage covers the visual half; this stops the
                    // gesture being claimed as a selection at all.
                    e.prevent_default();
                    dragging.set(true);
                    let c = e.client_coordinates();
                    last.set((c.x, c.y));
                },
                onmousemove: move |e| {
                    let c = e.client_coordinates();
                    if dragging() {
                        let (lx, ly) = last();
                        let (px, py) = pan();
                        pan.set((px + (c.x - lx), py + (c.y - ly)));
                        last.set((c.x, c.y));
                        return;
                    }
                    // Not dragging: track the beat under the pointer. Only
                    // meaningful on the canvas — the SVG pages have no
                    // scene to hit-test against.
                    if gpu {
                        hover.set(chart_gpu::scene_point((c.x, c.y), pan(), zoom()));
                    }
                },
                onmouseup: move |_| dragging.set(false),
                onmouseleave: move |_| {
                    dragging.set(false);
                    hover.set(None);
                },

                if gpu {
                    // The camera is passed to the renderer, not applied
                    // here: on a canvas there is no element to transform,
                    // and re-rasterising at the new scale is the point —
                    // a CSS scale would just magnify pixels.
                    canvas {
                        id: chart_gpu::CANVAS_ID,
                        class: "kf-preview-canvas",
                        // The first draw happens before the stylesheet
                        // has given the canvas a size, so it measures the
                        // intrinsic 300x150 and renders into that. This
                        // fires once the node is in the document, and one
                        // more draw is all it takes — `draw` re-measures
                        // every time and resizes the surface when the
                        // answer changed.
                        onmounted: move |_| {
                            let n = surface_size.peek().0;
                            surface_size.set((n + 1, 0));
                        },
                        // And again on every later change, so a pane
                        // resize is not a stretched buffer.
                        onresize: move |e| {
                            if let Ok(size) = e.get_content_box_size() {
                                let next = (size.width as i32, size.height as i32);
                                if surface_size.peek().0 != next.0
                                    || surface_size.peek().1 != next.1
                                {
                                    surface_size.set(next);
                                }
                            }
                        },
                    }
                } else {
                    div {
                        class: "kf-preview-pages",
                        style: "transform: translate({pan().0}px, {pan().1}px) scale({zoom()}); transform-origin: 0 0;",
                        for (i, page) in pages.read().iter().enumerate() {
                            PreviewPage { key: "{i}", page: page.clone() }
                        }
                    }
                }

                // Where the pointer is, in the chart's own terms.
                if let Some(where_am_i) = readout() {
                    div { class: "kf-preview-readout", "{where_am_i}" }
                }
            }

            if let Some(message) = status() {
                p { class: "kf-chart-error", "{message}" }
            }
        }
    }
}

/// One sheet in the spread, at its true size.
#[component]
fn PreviewPage(page: Page) -> Element {
    rsx! {
        div {
            class: "kf-preview-page",
            style: "width: {page.width_px}px; height: {page.height_px}px; margin-right: {PAGE_GAP_PX}px;",
            div { dangerous_inner_html: "{page.svg}" }
        }
    }
}

/// Everything you can do with a finished chart, behind one button.
///
/// Four ways out — a link, an SVG, a PDF, the source — used rarely and
/// never in the middle of writing. As four controls in the header they
/// cost permanent space to be mostly ignored; behind one they cost a
/// click when you actually want them.
#[component]
fn ShareButton(source: String, exporting: Signal<bool>, status: Signal<Option<String>>) -> Element {
    let mut open = use_signal(|| false);

    rsx! {
        button {
            class: "kf-button kf-button-primary",
            onclick: move |_| open.set(true),
            "Share"
        }
        if open() {
            ShareDialog { source: source.clone(), exporting, status, open }
        }
    }
}

/// The share sheet itself.
#[component]
fn ShareDialog(
    source: String,
    exporting: Signal<bool>,
    status: Signal<Option<String>>,
    open: Signal<bool>,
) -> Element {
    let mut exporting = exporting;
    let mut status = status;
    let mut open = open;
    let mut copied = use_signal(|| false);

    let encoded = use_memo(use_reactive!(|source| chart_url::encode(&source)));
    let shareable = use_memo(move || chart_url::fits_in_url(&encoded()));
    let url = use_memo(move || share_url(&encoded()));

    // Exporting runs on the main thread and a big chart's PDF takes long
    // enough to notice, so it goes through `spawn` with a visible pending
    // state rather than freezing the tab mid-click.
    // Captures only the two signals, which are `Copy`, so the closure is
    // `Copy` too and each button can take its own. The source arrives per
    // call instead of being captured, which is what stops the first
    // button from moving it out from under the other two.
    let mut run = move |format: &'static str, source: String| {
        if exporting() {
            return;
        }
        exporting.set(true);
        status.set(None);
        spawn(async move {
            let result = match format {
                "pdf" => export_pdf(&source),
                "svg" => export_svg(&source),
                // The chart's own text. Nothing to render — it is what
                // the editor already holds.
                _ => Ok((
                    source.clone().into_bytes(),
                    "text/plain;charset=utf-8",
                    "kf",
                )),
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

    let (svg_src, pdf_src, kf_src) = (source.clone(), source.clone(), source.clone());

    rsx! {
        // The backdrop closes on click; the sheet stops the click from
        // reaching it, so a click inside does not dismiss.
        div {
            class: "kf-share-backdrop",
            onclick: move |_| open.set(false),
            div {
                class: "kf-share-sheet",
                onclick: move |e| e.stop_propagation(),
                div { class: "kf-share-head",
                    h2 { "Share this chart" }
                    button {
                        class: "kf-share-close",
                        "aria-label": "Close",
                        onclick: move |_| open.set(false),
                        "\u{00d7}"
                    }
                }

                if shareable() {
                    p { class: "kf-share-label", "Link" }
                    div { class: "kf-share-link",
                        code { class: "kf-share-url", "{url()}" }
                        button {
                            class: "kf-button",
                            onclick: move |_| match copy_to_clipboard(&url()) {
                                Ok(()) => {
                                    copied.set(true);
                                    status.set(None);
                                }
                                Err(e) => {
                                    copied.set(false);
                                    status.set(Some(e));
                                }
                            },
                            if copied() { "Copied" } else { "Copy" }
                        }
                    }
                    p { class: "kf-note",
                        "The chart travels in the link itself — no account, nothing stored."
                    }
                } else {
                    p { class: "kf-share-label", "Link" }
                    p { class: "kf-note",
                        "This chart is too long to fit in a link ({encoded().len()} characters). "
                        "Download it instead — saving charts needs an account, which is coming."
                    }
                }

                p { class: "kf-share-label", "Download" }
                div { class: "kf-share-actions",
                    button {
                        class: "kf-button",
                        disabled: exporting(),
                        onclick: move |_| run("svg", svg_src.clone()),
                        "SVG"
                    }
                    button {
                        class: "kf-button",
                        disabled: exporting(),
                        onclick: move |_| run("pdf", pdf_src.clone()),
                        "PDF"
                    }
                    button {
                        class: "kf-button",
                        disabled: exporting(),
                        onclick: move |_| run("kf", kf_src.clone()),
                        "Keyflow text"
                    }
                }

                if exporting() {
                    p { class: "kf-note", "Exporting\u{2026}" }
                }
                if let Some(e) = status() {
                    p { class: "kf-note kf-share-error", "{e}" }
                }
            }
        }
    }
}

/// The absolute URL a `/c/:data` link resolves to.
#[cfg(target_arch = "wasm32")]
fn share_url(encoded: &str) -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .map_or_else(|| format!("/c/{encoded}"), |o| format!("{o}/c/{encoded}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn share_url(encoded: &str) -> String {
    format!("/c/{encoded}")
}

/// Put `text` on the system clipboard.
#[cfg(target_arch = "wasm32")]
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let navigator = web_sys::window().ok_or("no window")?.navigator();

    // `navigator.clipboard` is undefined outside a secure context, and
    // web-sys types the getter as infallible — so reaching straight for
    // `.clipboard()` and calling it would throw rather than fail. Ask
    // first, and report a miss as an ordinary error the button can show.
    let has_clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into())
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false);
    if !has_clipboard {
        return Err("the clipboard needs a secure (https) page".into());
    }

    // Fire and forget: the promise settles after this handler returns,
    // and there is nothing useful to do with the result that the
    // button's own state does not already say.
    let _ = navigator.clipboard().write_text(text);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_to_clipboard(_text: &str) -> Result<(), String> {
    Err("clipboard is only available in the browser".into())
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
