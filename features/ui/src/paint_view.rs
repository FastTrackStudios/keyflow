//! Native workspace adapter for the live chart renderer.
//!
//! Supply [`ChartSnapshot`] in `Extras::with_data`; increment `revision`
//! whenever the chart changes and wake the mounted pane after updates. The
//! host retains ownership of parsing, editing, playback and theme selection.
//! The framing ([`ChartView`]) is shared both ways: the pane writes the
//! wheel and a middle-button drag into it, and a host reads or sets it.

use architect_workspace::paint::{PaintMount, PaintView, PaintWake, Scene};
use architect_workspace::{Button, Extras, HostData, ViewCtx, ViewInput};
use keyflow::engraver::renderer::{self, points_to_px, ChartTheme, LayoutWorker};

/// The zoom a host's `Some(zoom)` is clamped to.
const MIN_ZOOM: f64 = 0.25;
const MAX_ZOOM: f64 = 4.0;
/// Ctrl + wheel zooms by this much per notch. A notch is one line as the host
/// reports it, which is 20 logical pixels.
const ZOOM_PER_NOTCH: f64 = 1.1;
const WHEEL_NOTCH_PX: f64 = 20.0;

/// How a chart is framed in its pane. It lives in [`ChartSnapshot`], so a host
/// reads what the user did and can set it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartView {
    /// The zoom, where 1.0 is 100%: the chart's own size in logical pixels.
    /// `None` fits the page to the pane's width. `Some` is clamped to
    /// [0.25, 4.0].
    pub zoom: Option<f64>,
    /// The pan from the chart's top-left corner, in logical pixels of the
    /// chart at 100% (a point is `points_to_px(1.0)` of them). It is clamped
    /// so the chart does not scroll past its edges as it is drawn.
    pub scroll: (f64, f64),
}

impl Default for ChartView {
    fn default() -> Self {
        Self {
            zoom: None,
            scroll: (0.0, 0.0),
        }
    }
}

/// The chart, playback position and framing supplied by the host.
#[derive(Clone)]
pub struct ChartSnapshot {
    pub chart: keyflow::Chart,
    /// Changes whenever the chart content changes.
    pub revision: u64,
    /// Seconds on the chart timeline, or `None` when no cursor is drawn.
    pub playhead_secs: Option<f64>,
    /// Live theme supplied by the host; `None` uses the mount-time tokens.
    pub theme: Option<ChartTheme>,
    /// Zoom and pan. The pane updates them on the wheel (Ctrl to zoom) and
    /// with a middle-button drag; a host may set them as well.
    pub view: ChartView,
}

/// Initialization status for a chart pane. Hosts can show `error` in their
/// own chrome if fonts or the layout worker could not be started.
#[derive(Clone, Debug, Default)]
pub struct ChartPaintStatus {
    pub error: Option<String>,
}

/// A mount and its per-pane wake handle and initialization status.
#[derive(Clone)]
pub struct ChartMount {
    pub mount: PaintMount,
    pub wake: PaintWake,
    pub status: HostData<ChartPaintStatus>,
}

impl PartialEq for ChartMount {
    fn eq(&self, other: &Self) -> bool {
        self.mount == other.mount
    }
}

/// Prepare a chart pane. After changing `data`, call `wake.wake()` to request
/// a frame. Register `KindSpec::paint()` and mount this inside a workspace
/// pane with `PaintPane { mount }`.
#[must_use]
pub fn chart_mount(data: HostData<ChartSnapshot>) -> ChartMount {
    let status = HostData::new(ChartPaintStatus::default());
    let mount = PaintMount::new(|ctx| {
        let result = ChartPaintView::new();
        if let Some(status) = ctx.data::<ChartPaintStatus>() {
            status.set(ChartPaintStatus {
                error: result.as_ref().err().cloned(),
            });
        }
        MountedChart(result.ok())
    })
    .extras(Extras::default().with_data(data).with_data(status.clone()));
    let wake = mount.wake_handle();
    ChartMount {
        mount,
        wake,
        status,
    }
}

struct MountedChart(Option<ChartPaintView>);

impl PaintView for MountedChart {
    fn input(&mut self, ctx: &ViewCtx, event: &ViewInput) -> bool {
        self.0.as_mut().is_some_and(|view| view.input(ctx, event))
    }

    fn paint(&mut self, ctx: &ViewCtx, scene: &mut Scene) {
        if let Some(view) = &mut self.0 {
            view.paint(ctx, scene);
        }
    }

    fn animating(&self, ctx: &ViewCtx) -> bool {
        self.0.as_ref().is_some_and(|view| view.animating(ctx))
    }
}

/// An in-process chart view for `PaintMount`.
pub struct ChartPaintView {
    view: renderer::ChartView,
    worker: LayoutWorker,
    requested: Option<u64>,
    /// The laid-out chart's size in logical pixels at 100%, from the last paint.
    content: (f64, f64),
    /// Whether Ctrl was held at the last key event.
    ctrl: bool,
    /// The pointer's last position in the pane, from its pointer events.
    pointer: (f64, f64),
    /// Whether a middle-button drag is in progress.
    panning: bool,
}

impl ChartPaintView {
    /// Construct a view and start its layout worker.
    ///
    /// # Errors
    /// Returns an error if the font bundle or layout worker cannot start.
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            view: renderer::ChartView::new()?,
            worker: LayoutWorker::spawn().map_err(|e| e.to_string())?,
            requested: None,
            content: (0.0, 0.0),
            ctrl: false,
            pointer: (0.0, 0.0),
            panning: false,
        })
    }

    fn update_layout(&mut self, snapshot: &ChartSnapshot) {
        if let Some(layout) = self.worker.poll() {
            if layout.key() == snapshot.revision {
                self.view.apply_precomputed_layout(layout);
            }
            self.requested = None;
        }
        if self.view.needs_layout(snapshot.revision) && self.requested != Some(snapshot.revision) {
            self.worker.submit(
                self.view
                    .layout_job(snapshot.chart.clone(), snapshot.revision),
            );
            self.requested = Some(snapshot.revision);
        }
    }

    /// The zoom in effect: the host's, clamped, or fit-to-width for `None`.
    fn effective_zoom(&self, zoom: Option<f64>, viewport_w: f32) -> f64 {
        match zoom {
            Some(zoom) => clamp_zoom(zoom),
            None => fit_zoom(
                f64::from(viewport_w),
                self.view.page(1).map(|(_, _, width, _)| width),
            ),
        }
    }

    /// Edit the host's view in place. `edit` gets the view and the zoom in effect.
    fn edit_view(
        &self,
        data: &HostData<ChartSnapshot>,
        ctx: &ViewCtx,
        edit: impl FnOnce(ChartView, f64) -> ChartView,
    ) {
        data.update(|snapshot| {
            let zoom = self.effective_zoom(snapshot.view.zoom, ctx.geometry.width);
            snapshot.view = edit(snapshot.view, zoom);
        });
    }
}

impl PaintView for ChartPaintView {
    fn input(&mut self, ctx: &ViewCtx, event: &ViewInput) -> bool {
        let Some(data) = ctx.data::<ChartSnapshot>() else {
            return false;
        };
        let pane = pane_size(ctx);
        match event {
            ViewInput::Key { modifiers, .. } => {
                self.ctrl = modifiers.contains(&"Ctrl");
                false
            }
            ViewInput::Focus(false) => {
                self.ctrl = false;
                self.panning = false;
                false
            }
            ViewInput::PointerDown { x, y, button } => {
                self.pointer = point(*x, *y);
                if *button == Button::Middle {
                    self.panning = true;
                }
                false
            }
            ViewInput::PointerUp { x, y, button } => {
                self.pointer = point(*x, *y);
                if *button == Button::Middle {
                    self.panning = false;
                }
                false
            }
            ViewInput::PointerMove { x, y } => {
                let from = std::mem::replace(&mut self.pointer, point(*x, *y));
                if !self.panning {
                    return false;
                }
                let (to, content) = (self.pointer, self.content);
                self.edit_view(&data, ctx, |view, zoom| {
                    scroll_by(view, (from.0 - to.0, from.1 - to.1), zoom, content, pane)
                });
                true
            }
            ViewInput::Wheel { dx, dy, x, y, mods } => {
                let (dx, dy) = (f64::from(*dx), f64::from(*dy));
                // Ctrl (Cmd on macOS) and the pointer come with the wheel itself.
                let ctrl = mods.iter().any(|m| *m == "Mod");
                let (pointer, content) = (point(*x, *y), self.content);
                self.edit_view(&data, ctx, |view, zoom| {
                    if ctrl && dy != 0.0 {
                        let factor = wheel_zoom_factor(dy);
                        zoom_about(view, zoom, factor, pointer, content, pane)
                    } else {
                        scroll_by(view, (dx, dy), zoom, content, pane)
                    }
                });
                true
            }
            _ => false,
        }
    }

    fn paint(&mut self, ctx: &ViewCtx, scene: &mut Scene) {
        let Some(data) = ctx.data::<ChartSnapshot>() else {
            return;
        };
        let snapshot = data.get();
        self.view.set_theme(
            snapshot
                .theme
                .clone()
                .unwrap_or_else(|| ChartTheme::from_tokens(|name| ctx.theme.color(name))),
        );
        self.update_layout(&snapshot);
        let zoom = self.effective_zoom(snapshot.view.zoom, ctx.geometry.width);
        let (scroll_x, scroll_y) =
            clamp_scroll(snapshot.view.scroll, self.content, pane_size(ctx), zoom);
        // The renderer pans in points, so convert the view's logical pixels.
        let scroll_pt = (scroll_x / points_to_px(1.0), scroll_y / points_to_px(1.0));
        // The host scales the scene to physical pixels; paint in logical pixels.
        if let Some((width, height)) =
            self.view
                .paint_cached(scene, 1.0, zoom, scroll_pt, snapshot.playhead_secs)
        {
            self.content = (width * points_to_px(1.0), height * points_to_px(1.0));
        }
    }

    fn animating(&self, _ctx: &ViewCtx) -> bool {
        self.requested.is_some()
    }
}

/// The pane's logical size.
fn pane_size(ctx: &ViewCtx) -> (f64, f64) {
    (
        f64::from(ctx.geometry.width),
        f64::from(ctx.geometry.height),
    )
}

fn point(x: f32, y: f32) -> (f64, f64) {
    (f64::from(x), f64::from(y))
}

fn clamp_zoom(zoom: f64) -> f64 {
    zoom.clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Fit-to-width: the pane's width against the page's width at 100%, and never
/// past 100%. Without a laid-out page, 100%.
fn fit_zoom(viewport_w: f64, page_width_pt: Option<f64>) -> f64 {
    match page_width_pt {
        Some(width) if width > 0.0 && viewport_w > 0.0 => {
            (viewport_w / (width * points_to_px(1.0))).min(1.0)
        }
        _ => 1.0,
    }
}

/// Keep the chart within its edges. `content` is the chart's size and
/// `viewport` the pane's, both in logical pixels, so the pane shows
/// `viewport / zoom` chart pixels at `zoom`.
fn clamp_scroll(
    scroll: (f64, f64),
    content: (f64, f64),
    viewport: (f64, f64),
    zoom: f64,
) -> (f64, f64) {
    let limit = |content: f64, viewport: f64| (content - viewport / zoom).max(0.0);
    (
        scroll.0.clamp(0.0, limit(content.0, viewport.0)),
        scroll.1.clamp(0.0, limit(content.1, viewport.1)),
    )
}

/// Move the chart by `delta` logical pixels of the pane, at `zoom`, then keep
/// it within its edges. The zoom stays as it was.
fn scroll_by(
    view: ChartView,
    delta: (f64, f64),
    zoom: f64,
    content: (f64, f64),
    viewport: (f64, f64),
) -> ChartView {
    let scroll = (
        view.scroll.0 + delta.0 / zoom,
        view.scroll.1 + delta.1 / zoom,
    );
    ChartView {
        scroll: clamp_scroll(scroll, content, viewport, zoom),
        ..view
    }
}

/// The factor for a Ctrl + wheel of `dy`: a wheel up (`dy` < 0) zooms in.
/// Clamped to a factor of half or twice per event.
fn wheel_zoom_factor(dy: f64) -> f64 {
    ZOOM_PER_NOTCH.powf(-dy / WHEEL_NOTCH_PX).clamp(0.5, 2.0)
}

/// Zoom `zoom` by `factor` about `pointer` (logical pixels in the pane), so
/// the chart point under the pointer stays under it.
fn zoom_about(
    view: ChartView,
    zoom: f64,
    factor: f64,
    pointer: (f64, f64),
    content: (f64, f64),
    viewport: (f64, f64),
) -> ChartView {
    let scroll = clamp_scroll(view.scroll, content, viewport, zoom);
    let next = clamp_zoom(zoom * factor);
    // The chart pixel under the pointer now, and the scroll that keeps it
    // there at the new zoom.
    let under = (pointer.0 / zoom + scroll.0, pointer.1 / zoom + scroll.1);
    let scroll = (under.0 - pointer.0 / next, under.1 - pointer.1 / next);
    ChartView {
        zoom: Some(next),
        scroll: clamp_scroll(scroll, content, viewport, next),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    /// A chart far larger than the pane, so no test hits an edge by accident.
    const HUGE: (f64, f64) = (1.0e6, 1.0e6);
    const PANE: (f64, f64) = (400.0, 300.0);

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < EPS,
            "{actual} is not {expected}"
        );
    }

    #[test]
    fn chart_view_zoom_is_clamped_to_a_quarter_and_four() {
        assert_close(clamp_zoom(0.01), 0.25);
        assert_close(clamp_zoom(9.0), 4.0);
        assert_close(clamp_zoom(1.5), 1.5);

        let mut view = ChartView::default();
        let mut zoom = 1.0;
        for _ in 0..40 {
            view = zoom_about(
                view,
                zoom,
                wheel_zoom_factor(-WHEEL_NOTCH_PX),
                (0.0, 0.0),
                HUGE,
                PANE,
            );
            zoom = view.zoom.expect("ctrl-wheel sets a zoom");
        }
        assert_eq!(view.zoom, Some(MAX_ZOOM));

        for _ in 0..40 {
            view = zoom_about(
                view,
                zoom,
                wheel_zoom_factor(WHEEL_NOTCH_PX),
                (0.0, 0.0),
                HUGE,
                PANE,
            );
            zoom = view.zoom.expect("ctrl-wheel sets a zoom");
        }
        assert_eq!(view.zoom, Some(MIN_ZOOM));
    }

    #[test]
    fn chart_view_fit_is_today_s_fit_to_width_and_never_past_one() {
        // A 1000-point page is 1333.3 logical pixels at 100%.
        assert_close(
            fit_zoom(400.0, Some(1000.0)),
            400.0 / (1000.0 * points_to_px(1.0)),
        );
        assert_close(fit_zoom(4000.0, Some(100.0)), 1.0);
        assert_close(fit_zoom(400.0, None), 1.0);
    }

    #[test]
    fn chart_view_scroll_is_clamped_to_the_chart_edges() {
        let content = (1000.0, 500.0);

        // At 100% the pane shows 400 x 300 of a 1000 x 500 chart.
        assert_eq!(
            clamp_scroll((-5.0, 9999.0), content, PANE, 1.0),
            (0.0, 200.0)
        );
        assert_eq!(
            clamp_scroll((700.0, 150.0), content, PANE, 1.0),
            (600.0, 150.0)
        );

        // At 50% the pane shows 800 x 600 chart pixels, so only 200 across fits.
        assert_eq!(
            clamp_scroll((700.0, 150.0), content, PANE, 0.5),
            (200.0, 0.0)
        );

        // A chart smaller than the pane cannot scroll at all.
        assert_eq!(
            clamp_scroll((50.0, 50.0), (100.0, 100.0), PANE, 1.0),
            (0.0, 0.0)
        );
    }

    #[test]
    fn chart_view_ctrl_wheel_keeps_the_point_under_the_pointer() {
        // (view, zoom in effect, wheel dy, pointer, expected zoom)
        let cases = [
            (
                ChartView {
                    zoom: None,
                    scroll: (100.0, 80.0),
                },
                0.5,
                -20.0,
                (120.0, 90.0),
                0.55,
            ),
            (
                ChartView {
                    zoom: Some(1.0),
                    scroll: (300.0, 200.0),
                },
                1.0,
                -40.0,
                (400.0, 250.0),
                1.21,
            ),
            (
                ChartView {
                    zoom: Some(2.0),
                    scroll: (50.0, 400.0),
                },
                2.0,
                20.0,
                (10.0, 300.0),
                2.0 / 1.1,
            ),
            (
                ChartView {
                    zoom: Some(3.0),
                    scroll: (1000.0, 1000.0),
                },
                3.0,
                -100.0,
                (200.0, 150.0),
                4.0,
            ),
        ];
        for (view, zoom, dy, pointer, expected_zoom) in cases {
            let next = zoom_about(view, zoom, wheel_zoom_factor(dy), pointer, HUGE, PANE);
            let next_zoom = next.zoom.expect("ctrl-wheel sets a zoom");
            assert_close(next_zoom, expected_zoom);

            // The chart pixel under the pointer, before and after.
            let before = (
                pointer.0 / zoom + view.scroll.0,
                pointer.1 / zoom + view.scroll.1,
            );
            let after = (
                pointer.0 / next_zoom + next.scroll.0,
                pointer.1 / next_zoom + next.scroll.1,
            );
            assert_close(after.0, before.0);
            assert_close(after.1, before.1);
        }
    }

    #[test]
    fn chart_view_drag_and_wheel_scroll_by_the_delta_over_the_zoom() {
        // A middle-button drag of (+30, -20) is a pane delta of (-30, +20) from
        // where the chart was grabbed, so the scroll moves by it over the zoom.
        let view = ChartView {
            zoom: Some(2.0),
            scroll: (500.0, 500.0),
        };
        let dragged = scroll_by(view, (-30.0, 20.0), 2.0, HUGE, PANE);
        assert_eq!(dragged.zoom, Some(2.0));
        assert_close(dragged.scroll.0, 485.0);
        assert_close(dragged.scroll.1, 510.0);

        // A pan past the left and top edges stops there.
        let edge = scroll_by(
            ChartView::default(),
            (-30.0, 20.0),
            1.0,
            (1000.0, 500.0),
            PANE,
        );
        assert_eq!(edge.scroll, (0.0, 20.0));
    }
}
