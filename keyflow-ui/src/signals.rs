//! Global signals for Chart Editor state.
//!
//! These signals drive the chart editor's reactive data flow:
//! source text changes → parse → layout → render.

use crate::examples::DEFAULT_CHART;
use crate::prelude::*;

/// The current keyflow source text being edited.
pub static CHART_SOURCE: GlobalSignal<String> = GlobalSignal::new(|| DEFAULT_CHART.to_string());

/// The current preview mode (Snippet vs Page).
pub static CHART_PREVIEW_MODE: GlobalSignal<PreviewMode> = GlobalSignal::new(|| PreviewMode::Page);

/// The physical pixel bounds of the chart preview area.
/// Updated by the UI after layout, consumed by the WGPU rendering loop.
pub static CHART_EDITOR_BOUNDS: GlobalSignal<ChartEditorBounds> =
    GlobalSignal::new(ChartEditorBounds::zero);

/// Preview mode for the chart renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    /// Content-sized layout with no fixed page dimensions.
    Snippet,
    /// A4 paginated layout (595x842 points).
    Page,
}

/// Physical pixel bounds of the chart preview area in the window.
/// Used to tell the WGPU renderer where to draw the chart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartEditorBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Device pixel ratio (e.g. 2.0 on Retina displays).
    pub dpr: f64,
}

/// Viewport state for the chart preview (scroll offset + zoom).
/// Mouse events captured by the WebView are translated into viewport changes.
pub static CHART_VIEWPORT: GlobalSignal<ChartViewport> = GlobalSignal::new(ChartViewport::default);

/// Chart render stats (FPS, frame time). Updated by the render loop.
pub static CHART_RENDER_STATS: GlobalSignal<RenderStats> = GlobalSignal::new(RenderStats::default);

/// Lightweight render performance stats based on vello's stats.rs.
/// Uses a sliding window of frame times to compute FPS and min/max.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderStats {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub frame_time_min_ms: f64,
    pub frame_time_max_ms: f64,
}

impl RenderStats {
    pub const fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            frame_time_min_ms: 0.0,
            frame_time_max_ms: 0.0,
        }
    }
}

/// Viewport transform for the chart preview area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartViewport {
    /// Horizontal scroll offset in points.
    pub scroll_x: f64,
    /// Vertical scroll offset in points.
    pub scroll_y: f64,
    /// Zoom scale factor (1.0 = 100%).
    pub zoom: f64,
}

impl ChartViewport {
    pub const fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl ChartEditorBounds {
    pub const fn new(x: f64, y: f64, width: f64, height: f64, dpr: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            dpr,
        }
    }

    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            dpr: 1.0,
        }
    }

    /// Returns true if the bounds have a non-zero area.
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
}
