//! WGPU/Vello renderer for music notation.
//!
//! This module provides GPU-accelerated rendering of music notation
//! using Vello for 2D vector graphics.
//!
//! ## Components
//!
//! - [`VelloSceneRenderer`] - Renders scene graph to Vello for GPU output
//! - [`EngraverRenderer`] - High-level renderer API
//! - [`primitives`] - Low-level WGPU vertex types and shaders

// region:    --- Modules

#[cfg(feature = "wgpu")]
pub mod canvas2d;
#[cfg(feature = "engraver-example")]
pub mod context;
pub mod cursor_renderer;
#[cfg(feature = "wgpu")]
pub mod primitives;
pub mod scene_renderer;
pub mod theme;
pub mod view;

// endregion: --- Modules

// region:    --- Re-exports

#[cfg(feature = "wgpu")]
pub use canvas2d::{Canvas2D, Color as Canvas2DColor, Rect as Canvas2DRect, Vertex2D};
pub use cursor_renderer::render_cursor_commands;
#[cfg(feature = "wgpu")]
pub use primitives::{
    BLIT_SHADER_SOURCE, BlitVertex, CameraUniform, SDF_SHADER_SOURCE, SHADER_SOURCE, SdfRectVertex,
    Vertex, create_blit_pipeline, create_blit_texture_bind_group_layout,
    create_camera_bind_group_layout, create_fullscreen_quad, create_line, create_main_pipeline,
    create_rect, create_sdf_pipeline, create_sdf_rounded_rect, px_to_ndc,
};
pub use scene_renderer::{SceneRenderBuilder, SceneRenderConfig, VelloSceneRenderer};
pub use theme::{
    ArtworkLayer, ChartGeometry, ChartTheme, ChartTypography, InkMap, InkRole, MARKER_COLOR,
    PageArtwork, REPEAT_COLOR,
};
#[cfg(not(target_arch = "wasm32"))]
pub use view::LayoutWorker;
pub use view::{ChartView, LayoutJob, PreparedLayout, points_to_px};

#[cfg(feature = "engraver-example")]
pub use context::VelloRenderContext;

// endregion: --- Re-exports

// region:    --- RenderConfig

use peniko::Color;

/// A chart's colours and staff size — the paint-time half of a
/// [`ChartTheme`]. `ChartView` reads every field but `selection_color`,
/// which is the host's (it draws its own selections).
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Page (paper) colour. The layout's white knock-outs become this too.
    pub background_color: Color,
    /// Colour of strokes: staff lines, barlines, stems, boxes.
    pub staff_color: Color,
    /// Colour of glyphs, text and filled shapes (the ink).
    pub foreground_color: Color,
    /// Selection highlight colour, for the host's own selection drawing.
    pub selection_color: Color,
    /// Playback cursor colour; `None` keeps the cursor's own.
    pub cursor_color: Option<Color>,
    /// Chromatic colours the layout bakes in (rehearsal-mark palettes,
    /// highlights), each mapped to the theme's replacement. Matched on
    /// RGB; the source alpha is kept.
    pub accents: Vec<(Color, Color)>,
    /// Staff space in points; `None` keeps the layout preset's. Changing
    /// it re-lays the chart out (it is geometry, not colour).
    pub staff_space: Option<f64>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            background_color: Color::WHITE,
            staff_color: Color::BLACK,
            foreground_color: Color::BLACK,
            selection_color: Color::from_rgb8(0, 120, 215),
            cursor_color: None,
            accents: Vec::new(),
            staff_space: None,
        }
    }
}

// endregion: --- RenderConfig

// The high-level `EngraverRenderer` stub was removed. The real chart rendering
// pipeline goes `layout_chart` → `SceneNode` → `VelloSceneRenderer`. Use
// [`scene_renderer::SceneRenderBuilder`] / [`VelloSceneRenderer`] directly.
