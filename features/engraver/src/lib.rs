// Lint debt: workspace flipped dead_code/unused to warn (task cleanup);
// this crate predates that — burn down separately.
#![allow(dead_code, unused)]

//! Engraver: chart layout, rendering and export for Keyflow.
//!
//! Layout and SVG export are the `svg` tier; `paint` draws into any
//! `anyrender::PaintScene`; `wgpu` is the live GPU renderer; `pdf` the PDF
//! export. Score import/export (MusicXML, Finale, multi-page parts) is the
//! `score` feature, and reading engraved music out of vector PDFs is
//! `pdfscore`. Application code reaches it through `keyflow::engraver`.

// region:    --- Keyflow Implementation Aliases

pub(crate) mod ast {
    // Facade glob re-export; consumed transitively as `crate::ast::*`.
    // `allow` keeps `cargo fix` from pruning it (rustc can't see the
    // glob's downstream uses).
    #[allow(unused_imports)]
    pub(crate) use keyflow_proto::syntax::ast::*;
}
pub(crate) mod chart {
    pub(crate) use keyflow_proto::chart::*;
}
pub(crate) mod chord {
    pub(crate) use keyflow_proto::chord::*;
}
pub(crate) mod core {
    pub(crate) use keyflow_proto::core::*;
}
pub(crate) mod key {
    pub(crate) use keyflow_proto::key::*;
}
pub(crate) mod metadata {
    #[allow(unused_imports)]
    pub(crate) use keyflow_proto::metadata::*;
}
pub(crate) mod parsing {
    #[allow(unused_imports)]
    pub(crate) use keyflow_proto::syntax::parsing::*;
}
pub(crate) mod primitives {
    pub(crate) use keyflow_proto::primitives::*;
}
pub(crate) mod sections {
    pub(crate) use keyflow_proto::sections::*;
}
pub(crate) mod time {
    pub(crate) use keyflow_proto::time::*;
}

pub(crate) use keyflow_proto::*;

// endregion: --- Keyflow Implementation Aliases

// region:    --- Modules

#[cfg(feature = "svg")]
pub mod engraver;

#[cfg(feature = "svg")]
pub mod api;

#[cfg(feature = "score")]
pub mod score;

#[cfg(feature = "pdfscore")]
pub mod pdfscore;

// endregion: --- Modules

// region:    --- Re-exports

// The engine's modules at the root: `engraver::layout`, `engraver::export`, …
#[cfg(feature = "svg")]
pub use engraver::*;

#[cfg(feature = "svg")]
pub use api::prelude as api_prelude;

#[cfg(feature = "svg")]
pub use engraver::error::{Error, Result};
#[cfg(feature = "svg")]
pub use engraver::style::{MStyle, Sid, StyleValue};
#[cfg(feature = "svg")]
pub use engraver::{
    export, fonts, import, interaction, layout, model, notation, quantize, scene, style,
};
// The GPU renderer (wgpu/vello) is only available with the `wgpu` feature;
// the `svg` feature provides layout + SVG export without it. `paint` is the
// middle tier: the scene/cursor painters and `ChartView`, on any
// `anyrender::PaintScene` — no wgpu pipeline.
#[cfg(feature = "paint")]
pub use engraver::renderer;

// endregion: --- Re-exports
