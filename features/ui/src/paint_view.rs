//! Native workspace adapter for the live chart renderer.
//!
//! Supply [`ChartSnapshot`] in `Extras::with_data`; increment `revision`
//! whenever the chart changes and wake the mounted pane after updates. The
//! host retains ownership of parsing, editing, playback and theme selection.

use architect_workspace::paint::{PaintMount, PaintView, PaintWake, Scene};
use architect_workspace::{Extras, HostData, ViewCtx, ViewInput};
use keyflow::engraver::renderer::{points_to_px, ChartTheme, ChartView, LayoutWorker};

/// The chart and playback position supplied by the host.
#[derive(Clone)]
pub struct ChartSnapshot {
    pub chart: keyflow::Chart,
    /// Changes whenever the chart content changes.
    pub revision: u64,
    /// Seconds on the chart timeline, or `None` when no cursor is drawn.
    pub playhead_secs: Option<f64>,
    /// Live theme supplied by the host; `None` uses the mount-time tokens.
    pub theme: Option<ChartTheme>,
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
    view: ChartView,
    worker: LayoutWorker,
    requested: Option<u64>,
    zoom: f64,
    scroll: (f64, f64),
}

impl ChartPaintView {
    /// Construct a view and start its layout worker.
    ///
    /// # Errors
    /// Returns an error if the font bundle or layout worker cannot start.
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            view: ChartView::new()?,
            worker: LayoutWorker::spawn().map_err(|e| e.to_string())?,
            requested: None,
            zoom: 1.0,
            scroll: (0.0, 0.0),
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
}

impl PaintView for ChartPaintView {
    fn input(&mut self, _ctx: &ViewCtx, event: &ViewInput) -> bool {
        if let ViewInput::Wheel { dx, dy } = event {
            let factor = points_to_px(1.0) * self.zoom;
            self.scroll.0 = (self.scroll.0 + f64::from(*dx) / factor).max(0.0);
            self.scroll.1 = (self.scroll.1 + f64::from(*dy) / factor).max(0.0);
            return true;
        }
        false
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
        if let Some((_, _, width, _)) = self.view.page(1) {
            if width > 0.0 && ctx.geometry.width > 0.0 {
                self.zoom = (f64::from(ctx.geometry.width) / points_to_px(1.0) / width).min(1.0);
            }
        }
        // The host scales the scene to physical pixels; paint in logical pixels.
        self.view
            .paint_cached(scene, 1.0, self.zoom, self.scroll, snapshot.playhead_secs);
    }

    fn animating(&self, _ctx: &ViewCtx) -> bool {
        self.requested.is_some()
    }
}
