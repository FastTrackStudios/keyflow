//! Chart editor panel — split editor with live WGPU chart preview.
//!
//! Uses `ChartEditorLayout` for the Dioxus UI, and renders the chart
//! via `ChartLayoutManager` → `ChartGraphics` for WGPU.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use kurbo::Affine;

use crate::chart_graphics::ChartGraphics;
use crate::signals::{ChartEditorBounds, PreviewMode};
use crate::{
    ChartEditorLayout, ChartLayoutManager, SemanticZoomLevel, CHART_BASE_SCALE,
    CHART_CURSOR_POSITION, CHART_CURSOR_SCENE_CLICK, CHART_CURSOR_TICK, CHART_CURSOR_VISIBLE,
    CHART_EDITOR_BOUNDS, CHART_HOVER_SCENE_POINT, CHART_PAGE_INFO, CHART_PREVIEW_MODE,
    CHART_RENDER_STATS, CHART_SOURCE, CHART_VIEWPORT,
};

use dock_dioxus::DOCK_LAYOUT;
use dock_proto::PanelId;
use session_ui::{Session, ACTIVE_INDICES, ACTIVE_PLAYBACK_IS_PLAYING, ACTIVE_PLAYBACK_MUSICAL};

use super::render_stats::FpsTracker;

/// Chart Editor view — split editor with live WGPU chart preview.
///
/// Uses `ChartEditorLayout` from keyflow-ui for the Dioxus UI, and
/// renders the chart via `ChartLayoutManager` → `ChartGraphics` for WGPU.
#[component]
pub fn ChartView() -> Element {
    let graphics = consume_context::<Arc<Mutex<ChartGraphics>>>();

    // Enable transparent mode on mount
    use_effect(move || {
        document::eval(r#"document.documentElement.classList.add('transparent-mode');"#);
    });

    // Cleanup: remove transparent mode when component unmounts (if no other chart visible)
    use_drop(move || {
        let layout = DOCK_LAYOUT.peek();
        if !layout.panel_is_visible(PanelId::ChartEditor)
            && !layout.panel_is_visible(PanelId::ChartPreview)
        {
            document::eval(r#"document.documentElement.classList.remove('transparent-mode');"#);
        }
    });

    // Layout manager — created once, persists across renders.
    let layout_manager: Signal<Option<std::rc::Rc<std::cell::RefCell<ChartLayoutManager>>>> =
        use_signal(|| {
            ChartLayoutManager::new()
                .ok()
                .map(|m| std::rc::Rc::new(std::cell::RefCell::new(m)))
        });

    // Generation counter: bumped each time layout changes, so render effect re-fires.
    let mut layout_generation = use_signal(|| 0u64);

    // Layout effect: re-layout when source or preview mode changes.
    {
        use_effect(move || {
            let source = CHART_SOURCE.read().clone();
            let preview_mode = *CHART_PREVIEW_MODE.read();
            let bounds = *CHART_EDITOR_BOUNDS.read();

            if !bounds.is_valid() {
                return;
            }

            let snippet_mode = preview_mode == PreviewMode::Snippet;
            if let Some(ref manager_rc) = *layout_manager.read() {
                let mut manager = manager_rc.borrow_mut();
                match manager.parse_and_layout(&source, bounds.width, snippet_mode) {
                    Ok(true) => {
                        layout_generation.set(layout_generation() + 1);
                        tracing::info!("Chart layout updated (gen {})", layout_generation());

                        let base_scale = manager.fit_to_width_scale(bounds.width, bounds.dpr);
                        *CHART_BASE_SCALE.write() = base_scale;

                        if let Some(metadata) = manager.page_metadata() {
                            let total = metadata.len() as u32;
                            let mut info = CHART_PAGE_INFO.write();
                            info.total_pages = total.max(1);
                            info.page_metadata = metadata;
                            if info.current_page > total {
                                info.current_page = total.max(1);
                            }
                        }

                        // Apply FullPage zoom on initial layout
                        if !snippet_mode {
                            let vp = *CHART_VIEWPORT.peek();
                            if vp.zoom_level == SemanticZoomLevel::FullPage
                                && (vp.zoom - 1.0).abs() < 0.01
                                && vp.scroll_y.abs() < 0.01
                            {
                                if let Some(page) =
                                    manager.layout_result().and_then(|r| r.pages.first())
                                {
                                    if page.height > 0.0 && base_scale > 0.0 {
                                        let zoom = bounds.height / (page.height * base_scale);
                                        let mut vp = CHART_VIEWPORT.write();
                                        vp.zoom = zoom.clamp(0.1, 8.0);
                                        vp.zoom_level = SemanticZoomLevel::FullPage;
                                    }
                                }
                            }
                        }
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::info!("Chart parse error: {}", e);
                    }
                }
            }
        });
    }

    // FPS tracking: sliding window of frame times.
    let fps_state = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(FpsTracker::new())));

    // Render effect: re-renders when layout, viewport, or bounds change.
    {
        let graphics_clone = graphics.clone();
        let fps_tracker = fps_state.clone();
        use_effect(move || {
            let viewport = *CHART_VIEWPORT.read();
            let _gen = layout_generation();
            let active_song_index = ACTIVE_INDICES.peek().song_index;
            let playback_musical = *ACTIVE_PLAYBACK_MUSICAL.read();
            let playback_is_playing = *ACTIVE_PLAYBACK_IS_PLAYING.read();
            let current_cursor_tick = *CHART_CURSOR_TICK.read();

            let bounds = *CHART_EDITOR_BOUNDS.peek();

            if !bounds.is_valid() {
                return;
            }

            if let Some(ref manager_rc) = *layout_manager.read() {
                let mut manager = manager_rc.borrow_mut();
                if manager.layout_result().is_none() {
                    return;
                }

                let frame_start = std::time::Instant::now();

                let base_scale = manager.fit_to_width_scale(bounds.width, bounds.dpr);
                let pad = 20.0 * bounds.dpr;
                let transform = Affine::translate((
                    pad - viewport.scroll_x * bounds.dpr,
                    pad - viewport.scroll_y * bounds.dpr,
                )) * Affine::scale(base_scale * viewport.zoom);

                if (*CHART_BASE_SCALE.peek() - base_scale).abs() > 0.001 {
                    *CHART_BASE_SCALE.write() = base_scale;
                }

                let current_page = manager.current_page_for_scroll(
                    viewport.scroll_x,
                    base_scale,
                    viewport.zoom,
                    bounds.dpr,
                );
                {
                    let info = CHART_PAGE_INFO.peek();
                    if info.current_page != current_page || info.zoom_level != viewport.zoom_level {
                        drop(info);
                        let mut info = CHART_PAGE_INFO.write();
                        info.current_page = current_page;
                        info.zoom_level = viewport.zoom_level;
                    }
                }

                let mut cursor_tick = current_cursor_tick;

                // Follow DAW transport while playback is running.
                if playback_is_playing {
                    if let Some(musical) = playback_musical {
                        if let Some(playback_tick) = manager.tick_for_musical_position(
                            musical.measure - 1,
                            musical.beat - 1,
                            musical.subdivision,
                        ) {
                            cursor_tick = playback_tick;
                            if playback_tick != current_cursor_tick {
                                *CHART_CURSOR_TICK.write() = playback_tick;
                            }
                        }
                    }
                }

                // Process click-to-position
                let pending_click = *CHART_CURSOR_SCENE_CLICK.peek();
                if let Some((scene_x, scene_y)) = pending_click {
                    *CHART_CURSOR_SCENE_CLICK.write() = None;
                    if let Some(tick) = manager.tick_at_scene_point(scene_x, scene_y) {
                        tracing::info!(
                            "Click-to-position: scene=({:.1},{:.1}) → tick={}",
                            scene_x,
                            scene_y,
                            tick
                        );
                        cursor_tick = tick;
                        if tick != current_cursor_tick {
                            *CHART_CURSOR_TICK.write() = tick;
                        }

                        if let Some(song_index) = active_song_index {
                            if let Some((measure, beat, subdivision)) =
                                manager.musical_position_at_tick(tick)
                            {
                                spawn(async move {
                                    let _ = Session::get()
                                        .setlist()
                                        .seek_to_musical_position(
                                            song_index,
                                            daw_proto::MusicalPosition::new(
                                                measure + 1,
                                                beat + 1,
                                                subdivision,
                                            ),
                                        )
                                        .await;
                                });
                            }
                        }
                    }
                }

                let cursor_tick = if *CHART_CURSOR_VISIBLE.peek() {
                    Some(cursor_tick)
                } else {
                    let _ = *CHART_CURSOR_TICK.read();
                    None
                };

                let hover_point = *CHART_HOVER_SCENE_POINT.read();

                if let Some(tick) = cursor_tick {
                    let pos = manager.musical_position_for_tick(tick);
                    if *CHART_CURSOR_POSITION.peek() != pos {
                        *CHART_CURSOR_POSITION.write() = pos;
                    }
                }

                if let Ok(mut gfx) = graphics_clone.lock() {
                    let win_size = dioxus::desktop::window().window.inner_size();
                    let (sw, sh) = gfx.size();
                    if sw != win_size.width || sh != win_size.height {
                        tracing::debug!(
                            "Surface resize: {}x{} -> {}x{}",
                            sw,
                            sh,
                            win_size.width,
                            win_size.height
                        );
                        gfx.resize(win_size.width, win_size.height);
                    }

                    let dock_offset = Affine::translate((bounds.x, bounds.y));
                    gfx.render_chart(|painter| {
                        manager.render_to_scene(
                            painter,
                            bounds.width,
                            bounds.height,
                            dock_offset,
                            transform,
                            cursor_tick,
                            hover_point,
                        );
                    });
                }
                dioxus::desktop::window().window.request_redraw();

                let frame_time_us = frame_start.elapsed().as_micros() as u64;
                fps_tracker.borrow_mut().add_sample(frame_time_us);
            }
        });
    }

    // FPS display update: periodic, decoupled from render.
    {
        let fps_tracker_for_display = fps_state.clone();
        use_future(move || {
            let tracker = fps_tracker_for_display.clone();
            async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let stats = tracker.borrow().snapshot();
                    *CHART_RENDER_STATS.write() = stats;
                }
            }
        });
    }

    // Bounds polling: continuously query chart-editor-preview position
    {
        use_future(move || async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                let result = document::eval(
                    r#"
                        const el = document.getElementById('chart-editor-preview');
                        if (el) {
                            const rect = el.getBoundingClientRect();
                            const dpr = window.devicePixelRatio || 1;
                            return JSON.stringify({
                                x: rect.x * dpr,
                                y: rect.y * dpr,
                                width: rect.width * dpr,
                                height: rect.height * dpr,
                                dpr: dpr
                            });
                        }
                        return "null";
                    "#,
                );

                match result.await {
                    Ok(value) => {
                        let json_str = value
                            .as_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| value.to_string());

                        if json_str != "null" && json_str != "\"null\"" {
                            match serde_json::from_str::<serde_json::Value>(&json_str) {
                                Ok(parsed) => {
                                    let x = parsed["x"].as_f64().unwrap_or(0.0);
                                    let y = parsed["y"].as_f64().unwrap_or(0.0);
                                    let width = parsed["width"].as_f64().unwrap_or(0.0);
                                    let height = parsed["height"].as_f64().unwrap_or(0.0);
                                    let dpr = parsed["dpr"].as_f64().unwrap_or(1.0);

                                    if width > 0.0 && height > 0.0 {
                                        let current = *CHART_EDITOR_BOUNDS.read();
                                        if (current.x - x).abs() > 1.0
                                            || (current.y - y).abs() > 1.0
                                            || (current.width - width).abs() > 1.0
                                            || (current.height - height).abs() > 1.0
                                        {
                                            tracing::info!(
                                                    "Chart editor bounds updated: ({:.0}, {:.0}, {:.0}x{:.0}), dpr={:.2}",
                                                    x, y, width, height, dpr
                                                );
                                            *CHART_EDITOR_BOUNDS.write() =
                                                ChartEditorBounds::new(x, y, width, height, dpr);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to parse bounds JSON: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Chart editor bounds eval: {:?}", e);
                    }
                }
            }
        });
    }

    rsx! {
        div {
            class: "w-full h-full",
            style: "background: transparent !important;",
            ChartEditorLayout {}
        }
    }
}
