//! Keyflow UI Components
//!
//! Dioxus components for Keyflow chart editing with live WGPU preview.
//! Provides a chart editor with syntax highlighting and GPU-accelerated rendering
//! powered by the engraver pipeline.
//!
//! # Architecture
//!
//! This crate provides UI components that work with the keyflow parser and engraver:
//!
//! - **Components**: Reusable UI primitives (highlighted editor, etc.)
//! - **Layouts**: Complete view layouts (ChartEditorLayout)
//! - **Signals**: Global state management via Dioxus signals
//! - **ChartLayoutManager**: Desktop rendering engine (parse → layout → vello::Scene)
//! - **Examples**: Built-in chart examples (Thriller, Empty)
//!
//! # Setup
//!
//! The consuming app provides the WGPU rendering surface (via ChartGraphics).
//! This crate produces `vello::Scene` objects that the app renders to the surface.
//!
//! ```rust,ignore
//! use keyflow_ui::{ChartEditorLayout, ChartLayoutManager};
//! use keyflow_ui::signals::CHART_SOURCE;
//!
//! // In the app's rendering loop:
//! let mut manager = ChartLayoutManager::new().unwrap();
//! let chart = keyflow::text::chart::parse_chart(&*CHART_SOURCE.read()).ok();
//! if let Some(chart) = &chart {
//!     manager.layout_chart_with_mode(chart, width, true);
//!     let mut scene = vello::Scene::new();
//!     manager.render_to_scene(&mut scene, width, height, transform);
//!     // Hand scene to ChartGraphics for WGPU rendering
//! }
//! ```

/// Re-export dioxus prelude based on feature flags.
pub mod prelude {
    #[cfg(feature = "native")]
    pub use dioxus_native::prelude::*;

    #[cfg(not(feature = "native"))]
    pub use dioxus::prelude::*;
}

pub mod chart_renderer;
pub mod components;
pub mod examples;
pub mod layouts;
pub mod signals;

// Re-export key types for convenience
pub use chart_renderer::ChartLayoutManager;
pub use layouts::ChartEditorLayout;
pub use signals::{
    ChartEditorBounds, ChartViewport, PreviewMode, RenderStats, CHART_EDITOR_BOUNDS,
    CHART_PREVIEW_MODE, CHART_RENDER_STATS, CHART_SOURCE, CHART_VIEWPORT,
};
