//! Keyflow - Musical Chart Parser
//!
//! A trait-based system for parsing and manipulating musical charts.
//!
//! All chart types and parsing functionality is re-exported from `keyflow-proto`.

// Re-export all types from keyflow-proto for convenience
pub use keyflow_proto::*;
#[cfg(feature = "engraver")]
pub use engraver_proto as engraver;

// Local modules
mod error;
pub use error::{Error, Result};

pub mod patterns;
