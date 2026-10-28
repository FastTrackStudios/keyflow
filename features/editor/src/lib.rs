//! Keyflow in the Editor: `.kf` fences rendered to inline SVG, and the
//! language support — highlighting, diagnostics and hover — surfaced as
//! editor decorations.
//!
//! The renderer is the `render` feature (on by default): it pulls
//! engraver's CPU-only `svg` tier. `lang` needs only the parser, so a host
//! that wants decorations without fences turns defaults off.
//!
//! Opt-in, both off by default: `preview` (the chart preview's model —
//! pages and camera math, no Dioxus) and `view` (the chart editor as a
//! Dioxus component, for a browser or a Blitz window; `native` / `web`
//! pick editor-view's renderer).

#[cfg(feature = "render")]
mod fence;
#[cfg(feature = "render")]
pub use fence::*;

/// Subset `@font-face` rules for the charts a page actually draws.
///
/// Behind the `subset` feature: `allsorts` is large and nothing in a
/// browser needs it. Turned on by the site's build script and by its
/// dev server.
#[cfg(feature = "subset")]
pub mod font_subset;

pub mod lang;

/// The chart preview's model: engraving to page SVGs and the camera math,
/// without Dioxus or a DOM. Behind the `preview` feature.
#[cfg(feature = "preview")]
pub mod preview;

/// The chart editor as a Dioxus component, and the live preview's hook.
/// Behind the `view` feature; `native` / `web` pick the renderer.
#[cfg(feature = "view")]
pub mod view;
