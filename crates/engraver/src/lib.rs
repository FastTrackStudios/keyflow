//! Engraver facade — public API for chart layout, rendering, and export.
//!
//! Re-exports from `engraver-proto` so consumers use `engraver::layout::chart::X`
//! instead of `engraver_proto::engraver::layout::chart::X`.

// Re-export the engraver module contents at the root level
#[cfg(feature = "engraver")]
pub use engraver_proto::engraver::*;

// Re-export API surface
#[cfg(feature = "engraver")]
pub use engraver_proto::api;
#[cfg(feature = "engraver")]
pub use engraver_proto::api_prelude;

// Re-export legacy compat trait
pub use engraver_proto::DurationTrait;
