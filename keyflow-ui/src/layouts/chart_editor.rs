//! Chart Editor Layout
//!
//! Split-view editor with a keyflow text editor on the left and a live
//! WGPU chart preview on the right. The preview area is transparent so
//! the WGPU surface (rendered by the app's ChartGraphics) shows through.

use crate::components::HighlightedEditor;
use crate::examples::{self, EXAMPLES};
use crate::prelude::*;
use crate::signals::*;

/// Chart editor split-view layout.
///
/// Left panel: syntax-highlighted text editor with examples dropdown.
/// Right panel: transparent area for WGPU chart rendering + mode toggle.
///
/// The consuming app is responsible for:
/// 1. Watching `CHART_SOURCE` signal changes
/// 2. Parsing, laying out, and rendering the chart via `ChartLayoutManager`
/// 3. Rendering the resulting `vello::Scene` to `ChartGraphics` at `CHART_EDITOR_BOUNDS`
#[component]
pub fn ChartEditorLayout() -> Element {
    let source = CHART_SOURCE.read().clone();
    let preview_mode = *CHART_PREVIEW_MODE.read();

    // Dropdown open state
    let mut examples_open = use_signal(|| false);

    // Currently selected example index
    let mut selected_example = use_signal(|| 1usize); // Default: Thriller

    // Parse error state
    let parse_error = use_memo(move || {
        let source = CHART_SOURCE.read().clone();
        match keyflow::text::chart::parse_chart(&source) {
            Ok(_) => None,
            Err(e) => Some(e),
        }
    });

    rsx! {
        div {
            class: "flex h-full w-full text-foreground",
            style: "background: transparent !important;",

            // Click anywhere to close dropdown
            onclick: move |_| {
                if *examples_open.read() {
                    examples_open.set(false);
                }
            },

            // Left panel: Editor
            div {
                class: "flex flex-col w-1/2 border-r border-border bg-card",

                // Header bar
                div {
                    class: "flex items-center justify-between px-3 py-2 border-b border-border bg-card",

                    div {
                        class: "flex items-center gap-2",
                        span { class: "text-sm font-medium text-foreground", "Keyflow Source" }
                    }

                    div {
                        class: "flex items-center gap-2",

                        // Custom examples dropdown
                        div {
                            class: "relative",

                            // Dropdown trigger button
                            button {
                                class: "flex items-center gap-1.5 text-xs px-2.5 py-1.5 rounded-md border border-border bg-secondary text-secondary-foreground hover:bg-accent hover:text-accent-foreground transition-colors",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    let current = *examples_open.read();
                                    examples_open.set(!current);
                                },
                                span { "{EXAMPLES[*selected_example.read()].name}" }
                                // Chevron icon
                                svg {
                                    width: "12",
                                    height: "12",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "m6 9 6 6 6-6" }
                                }
                            }

                            // Dropdown menu
                            if *examples_open.read() {
                                div {
                                    class: "absolute left-0 top-full mt-1 w-48 rounded-lg shadow-lg border border-border bg-popover text-popover-foreground z-50 overflow-hidden",

                                    div {
                                        class: "py-1",
                                        for (i, example) in EXAMPLES.iter().enumerate() {
                                            button {
                                                class: if i == *selected_example.read() {
                                                    "w-full text-left text-xs px-3 py-2 bg-accent text-accent-foreground"
                                                } else {
                                                    "w-full text-left text-xs px-3 py-2 text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
                                                },
                                                onclick: move |evt| {
                                                    evt.stop_propagation();
                                                    selected_example.set(i);
                                                    if let Some(example) = EXAMPLES.get(i) {
                                                        *CHART_SOURCE.write() = example.source.to_string();
                                                    }
                                                    examples_open.set(false);
                                                },
                                                "{example.name}"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Reset button
                        button {
                            class: "text-xs text-muted-foreground hover:text-foreground px-2 py-1.5 rounded-md hover:bg-accent transition-colors",
                            onclick: move |_| {
                                *CHART_SOURCE.write() = examples::DEFAULT_CHART.to_string();
                                selected_example.set(1);
                            },
                            "Reset"
                        }
                    }
                }

                // Editor area
                HighlightedEditor {
                    source: source.clone(),
                    on_change: move |new_source: String| {
                        *CHART_SOURCE.write() = new_source;
                    }
                }
            }

            // Right panel: Preview (transparent for WGPU to show through)
            div {
                class: "flex flex-col w-1/2 chart-transparent-area",
                style: "background: transparent !important; background-color: transparent !important;",

                // Header bar
                div {
                    class: "flex items-center justify-between px-3 py-2 border-b border-border bg-card",

                    div {
                        class: "flex items-center gap-2",
                        span { class: "text-sm font-medium text-foreground", "Live Preview" }

                        // FPS counter
                        {
                            let stats = *CHART_RENDER_STATS.read();
                            let fps_color = if stats.fps >= 55.0 {
                                "text-green-400"
                            } else if stats.fps >= 25.0 {
                                "text-yellow-400"
                            } else {
                                "text-red-400"
                            };
                            rsx! {
                                span {
                                    class: "text-xs font-mono {fps_color}",
                                    "{stats.fps:.0} fps  {stats.frame_time_ms:.1}ms"
                                }
                            }
                        }

                        // Parse error indicator
                        if let Some(error) = &*parse_error.read() {
                            span {
                                class: "flex items-center gap-1 text-xs text-red-400 truncate max-w-xs",
                                title: "{error}",
                                // Warning icon
                                svg {
                                    width: "14",
                                    height: "14",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" }
                                    path { d: "M12 9v4" }
                                    path { d: "M12 17h.01" }
                                }
                                "Parse error"
                            }
                        }
                    }

                    // Preview mode toggle (segmented control)
                    div {
                        class: "flex items-center gap-0.5 rounded-md border border-border bg-secondary p-0.5",

                        button {
                            class: if preview_mode == PreviewMode::Snippet {
                                "text-xs px-2.5 py-1 rounded bg-primary text-primary-foreground font-medium transition-colors"
                            } else {
                                "text-xs px-2.5 py-1 rounded text-muted-foreground hover:text-foreground transition-colors"
                            },
                            onclick: move |_| {
                                *CHART_PREVIEW_MODE.write() = PreviewMode::Snippet;
                            },
                            "Snippet"
                        }

                        button {
                            class: if preview_mode == PreviewMode::Page {
                                "text-xs px-2.5 py-1 rounded bg-primary text-primary-foreground font-medium transition-colors"
                            } else {
                                "text-xs px-2.5 py-1 rounded text-muted-foreground hover:text-foreground transition-colors"
                            },
                            onclick: move |_| {
                                *CHART_PREVIEW_MODE.write() = PreviewMode::Page;
                            },
                            "Page (A4)"
                        }
                    }
                }

                // Transparent preview area — WGPU renders behind this
                // Captures mouse events and forwards them as viewport transforms
                {
                    let mut dragging = use_signal(|| false);
                    let mut last_mouse = use_signal(|| (0.0f64, 0.0f64));

                    rsx! {
                        div {
                            id: "chart-editor-preview",
                            class: "flex-1 cursor-grab",
                            style: "background: transparent !important;",

                            // Wheel → zoom (scroll wheel and trackpad pinch both zoom)
                            onwheel: move |evt| {
                                let delta_y = evt.delta().strip_units().y;
                                let mut vp = CHART_VIEWPORT.write();
                                let zoom_factor = if delta_y < 0.0 { 1.05 } else { 0.95 };
                                vp.zoom = (vp.zoom * zoom_factor).clamp(0.1, 8.0);
                            },

                            // Mouse drag → pan
                            onmousedown: move |evt| {
                                dragging.set(true);
                                let coords = evt.client_coordinates();
                                last_mouse.set((coords.x, coords.y));
                            },

                            onmousemove: move |evt| {
                                if *dragging.read() {
                                    let coords = evt.client_coordinates();
                                    let (lx, ly) = *last_mouse.read();
                                    let dx = coords.x - lx;
                                    let dy = coords.y - ly;

                                    let mut vp = CHART_VIEWPORT.write();
                                    vp.scroll_x -= dx;
                                    vp.scroll_y -= dy;

                                    last_mouse.set((coords.x, coords.y));
                                }
                            },

                            onmouseup: move |_| {
                                dragging.set(false);
                            },

                            onmouseleave: move |_| {
                                dragging.set(false);
                            },
                        }
                    }
                }
            }
        }
    }
}
