//! Engraver protocol - implementation crate for Keyflow engraving.
//!
//! Public consumers should use `keyflow` with its default `engraver` feature
//! and access the engine through `keyflow::engraver`.

// region:    --- Keyflow Implementation Aliases

pub(crate) mod ast {
    pub(crate) use keyflow_syntax::ast::*;
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
    pub(crate) use keyflow_proto::metadata::*;
}
pub(crate) mod parsing {
    pub(crate) use keyflow_syntax::parsing::*;
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

#[cfg(feature = "engraver")]
pub mod engraver;

#[cfg(feature = "engraver")]
pub mod api;

// endregion: --- Modules

// region:    --- Re-exports

#[cfg(feature = "engraver")]
pub use api::prelude as api_prelude;

#[cfg(feature = "engraver")]
pub use engraver::error::{Error, Result};
#[cfg(feature = "engraver")]
pub use engraver::model::{Measure, MusicElement, Part, Score, Voice};
#[cfg(feature = "engraver")]
pub use engraver::style::{MStyle, Sid, StyleValue};
#[cfg(feature = "engraver")]
pub use engraver::{
    export, fonts, import, interaction, layout, model, notation, quantize, renderer, scene, style,
};

// endregion: --- Re-exports
