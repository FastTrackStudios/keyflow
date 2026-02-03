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

pub mod canvas2d;
#[cfg(feature = "engraver-example")]
pub mod context;
pub mod primitives;
pub mod scene_renderer;

// endregion: --- Modules

// region:    --- Re-exports

pub use canvas2d::{Canvas2D, Color as Canvas2DColor, Rect as Canvas2DRect, Vertex2D};
pub use primitives::{
    create_blit_pipeline, create_blit_texture_bind_group_layout, create_camera_bind_group_layout,
    create_fullscreen_quad, create_line, create_main_pipeline, create_rect, create_sdf_pipeline,
    create_sdf_rounded_rect, px_to_ndc, BlitVertex, CameraUniform, SdfRectVertex, Vertex,
    BLIT_SHADER_SOURCE, SDF_SHADER_SOURCE, SHADER_SOURCE,
};
pub use scene_renderer::{SceneRenderBuilder, SceneRenderConfig, VelloSceneRenderer};

#[cfg(feature = "engraver-example")]
pub use context::VelloRenderContext;

// endregion: --- Re-exports

// region:    --- RenderConfig

use kurbo::{Affine, Point, Rect};
use vello::peniko::Color;
use vello::Scene;

/// Renderer configuration.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Background color
    pub background_color: Color,
    /// Staff line color
    pub staff_color: Color,
    /// Note and symbol color
    pub foreground_color: Color,
    /// Selection highlight color
    pub selection_color: Color,
    /// Staff space size in pixels
    pub staff_space: f64,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            background_color: Color::WHITE,
            staff_color: Color::BLACK,
            foreground_color: Color::BLACK,
            selection_color: Color::from_rgb8(0, 120, 215),
            staff_space: 10.0,
        }
    }
}

// endregion: --- RenderConfig

// region:    --- EngraverRenderer

/// The main renderer for music notation.
pub struct EngraverRenderer {
    config: RenderConfig,
}

impl EngraverRenderer {
    /// Create a new renderer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RenderConfig::default(),
        }
    }

    /// Create a new renderer with custom configuration.
    #[must_use]
    pub fn with_config(config: RenderConfig) -> Self {
        Self { config }
    }

    /// Build a Vello scene from a score.
    ///
    /// # Arguments
    /// * `scene` - The Vello scene to build into
    /// * `score` - The score to render
    /// * `viewport` - The visible viewport rectangle
    /// * `transform` - The view transform (for zoom/pan)
    pub fn render(
        &self,
        scene: &mut Scene,
        _score: &crate::engraver::model::Score,
        _viewport: Rect,
        transform: Affine,
    ) {
        // Clear with background color
        scene.fill(
            vello::peniko::Fill::NonZero,
            transform,
            self.config.background_color,
            None,
            &Rect::new(0.0, 0.0, 1000.0, 1000.0),
        );

        // TODO: Render score elements
        // - Staff lines
        // - Clefs
        // - Time signatures
        // - Notes
        // - etc.

        // Example: Draw a staff line for testing
        self.draw_staff_lines(scene, transform, Point::new(50.0, 100.0), 800.0);
    }

    /// Draw 5 staff lines.
    fn draw_staff_lines(&self, scene: &mut Scene, transform: Affine, origin: Point, width: f64) {
        let space = self.config.staff_space;
        let stroke = vello::kurbo::Stroke::new(1.0);

        for i in 0..5 {
            let y = origin.y + f64::from(i) * space;
            let line = kurbo::Line::new(Point::new(origin.x, y), Point::new(origin.x + width, y));

            scene.stroke(&stroke, transform, self.config.staff_color, None, &line);
        }
    }
}

impl Default for EngraverRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// endregion: --- EngraverRenderer
