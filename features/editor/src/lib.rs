//! Keyflow in the Editor: `.kf` fences rendered to inline SVG, and the
//! language support — highlighting, diagnostics and hover — surfaced as
//! editor decorations.
//!
//! The renderer is the `render` feature (on by default): it pulls
//! engraver's CPU-only `svg` tier. `lang` needs only the parser, so a host
//! that wants decorations without fences turns defaults off.

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
