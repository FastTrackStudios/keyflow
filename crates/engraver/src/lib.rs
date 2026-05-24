//! Engraver facade — public API for chart layout, rendering, and export.
//!
//! This crate is exported by `keyflow` as `keyflow::engraver`. Prefer that
//! facade in application code; direct use of `engraver` is kept for internal
//! workspace crates and compatibility.

// Re-export the engraver module contents at the root level
#[cfg(feature = "engraver")]
pub use engraver_proto::engraver::*;

// Re-export API surface
#[cfg(feature = "engraver")]
pub use engraver_proto::api;
#[cfg(feature = "engraver")]
pub use engraver_proto::api_prelude;
