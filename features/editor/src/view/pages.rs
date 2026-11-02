//! The live preview's engraving, as a hook: the pages of a chart that is
//! being typed, re-engraved on a debounce.

use dioxus::prelude::*;

use crate::preview::{ChartShape, Page, blank_page, engrave};

/// Idle delay before re-engraving, in ms. Long enough that a run of
/// keystrokes costs one render, short enough to feel live.
pub const DEBOUNCE_MS: u32 = 150;

/// The engraved pages of `source`, re-engraved [`DEBOUNCE_MS`] after it
/// last changed.
///
/// Debounced rather than merely delayed: a generation counter makes a
/// render that is no longer the newest drop itself on wake instead of
/// overwriting a fresher one. A source that does not lay out yet shows a
/// [`blank_page`] — the page never vanishes mid-thought.
///
/// Pass the source as it should be DRAWN (re-notated, say); the host
/// mounts the pages ([`Page::svg`] into an element, at `width_px` ×
/// `height_px`, next to [`font_face_css`](crate::preview::font_face_css)
/// once per document) and moves them with the camera math in
/// [`crate::preview`].
pub fn use_engraved_pages(source: String) -> Signal<Vec<Page>> {
    let mut pages = use_signal(|| vec![blank_page()]);
    let mut generation = use_signal(|| 0_u64);
    use_effect(use_reactive!(|source| {
        let mine = generation.peek().wrapping_add(1);
        generation.set(mine);
        spawn(async move {
            settle(DEBOUNCE_MS).await;
            if *generation.peek() != mine {
                return;
            }
            let rendered = engrave(&source, ChartShape::Page).unwrap_or_else(|e| {
                tracing::debug!(len = source.len(), "chart preview render failed: {e}");
                vec![blank_page()]
            });
            pages.set(rendered);
        });
    }));
    pages
}

/// Wait out the debounce: a browser timer on wasm, tokio's where a
/// runtime is running (dioxus-native enters one). Anywhere else — a test,
/// a host check of the site — there is no timer, and the render simply
/// goes next.
async fn settle(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(ms).await;
    #[cfg(not(target_arch = "wasm32"))]
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::time::sleep(std::time::Duration::from_millis(u64::from(ms))).await;
    }
}
