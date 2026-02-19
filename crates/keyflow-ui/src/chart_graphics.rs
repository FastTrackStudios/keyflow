//! Chart graphics context using anyrender for hybrid WGPU/Dioxus rendering.
//!
//! This module provides GPU-accelerated chart rendering using anyrender's
//! VelloWindowRenderer, designed to work alongside the Dioxus WebView UI.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │  Main Window (WGPU Surface - background)        │
//! │  ┌───────────────────────────────────────────┐  │
//! │  │  Vello Chart Rendering via anyrender      │  │
//! │  │  (renders behind transparent areas)       │  │
//! │  └───────────────────────────────────────────┘  │
//! │                                                  │
//! │  ┌───────────────────────────────────────────┐  │
//! │  │  Dioxus WebView (transparent overlay)     │  │
//! │  │  - UI controls float on top               │  │
//! │  │  - Transparent areas show WGPU content    │  │
//! │  └───────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────┘
//! ```

use std::sync::Arc;

use anyrender::WindowRenderer;
use anyrender_vello::{VelloRendererOptions, VelloWindowRenderer};
use dioxus::desktop::tao::window::Window;
use peniko::Color;

/// Chart graphics context wrapping anyrender's VelloWindowRenderer.
///
/// This provides a simple API for rendering charts to the window surface,
/// with the Dioxus WebView overlaid on top as a transparent child window.
pub struct ChartGraphics {
    renderer: VelloWindowRenderer,
    width: u32,
    height: u32,
}

impl ChartGraphics {
    /// Create a new ChartGraphics context for the given window.
    ///
    /// This initializes the Vello GPU renderer via anyrender.
    /// The window should have transparency enabled for the hybrid overlay to work.
    pub fn new(window: Arc<Window>, width: u32, height: u32) -> Self {
        let mut renderer = VelloWindowRenderer::with_options(VelloRendererOptions {
            base_color: Color::TRANSPARENT,
            ..Default::default()
        });

        renderer.resume(window, width, height);

        Self {
            renderer,
            width,
            height,
        }
    }

    /// Resize the rendering surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.renderer.set_size(width, height);
    }

    /// Check if the renderer is active.
    pub fn is_active(&self) -> bool {
        self.renderer.is_active()
    }

    /// Render using a drawing function.
    pub fn render<F>(&mut self, draw_fn: F)
    where
        F: FnOnce(&mut <VelloWindowRenderer as WindowRenderer>::ScenePainter<'_>),
    {
        self.renderer.render(draw_fn);
    }

    /// Get the current surface dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Render chart content through the anyrender PaintScene abstraction.
    pub fn render_chart<F>(&mut self, draw_fn: F)
    where
        F: FnOnce(&mut <VelloWindowRenderer as WindowRenderer>::ScenePainter<'_>),
    {
        self.renderer.render(draw_fn);
    }
}

impl Drop for ChartGraphics {
    fn drop(&mut self) {
        self.renderer.suspend();
    }
}
