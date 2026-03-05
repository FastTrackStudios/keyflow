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
use vello::AaConfig;

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
    fn antialiasing_from_env(var: &str, default: &str) -> AaConfig {
        match std::env::var("KEYFLOW_AA")
            .or_else(|_| std::env::var(var))
            .unwrap_or_else(|_| default.to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "msaa16" => AaConfig::Msaa16,
            "msaa8" => AaConfig::Msaa8,
            _ => AaConfig::Area,
        }
    }

    /// Create a new ChartGraphics context for the given window.
    ///
    /// This initializes the Vello GPU renderer via anyrender.
    /// The window should have transparency enabled for the hybrid overlay to work.
    pub fn new(window: Arc<Window>, width: u32, height: u32) -> Self {
        let current_aa = Self::antialiasing_from_env("KEYFLOW_AA_QUALITY", "msaa8");
        tracing::info!("ChartGraphics AA mode: {:?}", current_aa);

        let mut renderer = VelloWindowRenderer::with_options(VelloRendererOptions {
            base_color: Color::TRANSPARENT,
            antialiasing_method: current_aa,
            ..Default::default()
        });

        renderer.resume(window.clone(), width, height);

        Self {
            renderer,
            width,
            height,
        }
    }

    /// Use lower AA while actively interacting (pan/zoom), higher AA when idle.
    pub fn set_interaction_active(&mut self, active: bool) {
        // Runtime renderer hot-swap can disrupt WebView layering in hybrid mode.
        // Keep fixed AA for stability; runtime switching can be reintroduced behind
        // a safer surface reconfiguration path.
        let _ = active;
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
