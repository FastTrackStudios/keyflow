//! Native workspace adapter for the live chart renderer.
//!
//! Supply [`ChartSnapshot`] in `Extras::with_data`; increment `revision`
//! whenever the chart changes and wake the mounted pane after updates. The
//! host retains ownership of parsing, editing, playback and theme selection.
//! The framing ([`ChartView`]) is shared both ways: the pane writes the
//! wheel and a middle-button drag into it, and a host reads or sets it.

use std::hash::{Hash as _, Hasher as _};
use std::sync::Arc;

use architect_workspace::paint::peniko::kurbo::{Affine, Point, Rect, RoundedRect, Stroke};
use architect_workspace::paint::peniko::{Color, Fill};
use architect_workspace::paint::{
    Layer, LayerKey, PaintMount, PaintScene as _, PaintView, PaintWake, Scene,
};
use architect_workspace::{Button, Extras, HostData, ViewCtx, ViewInput};
use keyflow::engraver::layout::chart::select::{Selectable, in_box, pick};
use keyflow::engraver::renderer::{self, ChartTheme, LayoutWorker, points_to_px};

use crate::selection::{Gesture, Selection};

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
    /// Note input's cursor and shadow note, when a host is in note input.
    pub input: Option<InputOverlay>,
}

/// Where note input's next entry goes, as the pane draws it: MuseScore's
/// note-input cursor over the span of the bar the next note will fill, and
/// the shadow note, the note that would be entered, at its pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputOverlay {
    /// The bar (the chart's bars from 0, as `bar#N` names them).
    pub bar: usize,
    /// The span of the bar the next entry fills, as fractions of it.
    pub from: f64,
    pub to: f64,
    /// The note that would be entered, when the pane should show it.
    pub shadow: Option<ShadowNote>,
}

/// The note or rest note input would enter, drawn where it would go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowNote {
    /// Its staff position: half-spaces below the staff's top line (a line
    /// is even, a space odd; negative is above the staff).
    pub below_top: i32,
    /// A half or a whole: an open notehead.
    pub open: bool,
    /// A rest, not a note.
    pub rest: bool,
}

/// A key pressed on the chart, or a click on a bar: what the pane hands
/// its host, which turns keys into actions through its keymap (the view
/// binds none itself) and a click into a place for note input.
#[derive(Clone, Debug, PartialEq)]
pub enum ChartGesture {
    /// A key by its `KeyboardEvent.key` name, with the modifiers held.
    Key {
        key: String,
        mods: Vec<&'static str>,
    },
    /// The main button pressed on bar `bar`, `fraction` of the way across.
    Click { bar: usize, fraction: f64 },
}

/// Where the pane hands its gestures: given in its extras
/// ([`chart_mount_with`]).
#[derive(Clone)]
pub struct ChartGestures(pub Arc<dyn Fn(ChartGesture) + Send + Sync>);

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
    /// What is selected on the chart, as the pane last wrote it.
    pub selection: HostData<ChartSelection>,
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
    chart_mount_with(data, None)
}

/// [`chart_mount`], handing the pane's keys and clicks to `gestures`.
#[must_use]
pub fn chart_mount_with(
    data: HostData<ChartSnapshot>,
    gestures: Option<ChartGestures>,
) -> ChartMount {
    let status = HostData::new(ChartPaintStatus::default());
    let selection = HostData::new(ChartSelection::default());
    let mount = PaintMount::new(|ctx| {
        let result = ChartPaintView::new();
        if let Some(status) = ctx.data::<ChartPaintStatus>() {
            status.set(ChartPaintStatus {
                error: result.as_ref().err().cloned(),
            });
        }
        MountedChart(result.ok())
    })
    .extras({
        let extras = Extras::default()
            .with_data(data)
            .with_data(status.clone())
            .with_data(selection.clone());
        match gestures {
            Some(g) => extras.with(g),
            None => extras,
        }
    });
    let wake = mount.wake_handle();
    ChartMount {
        mount,
        wake,
        status,
        selection,
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

    fn layers(&mut self, ctx: &ViewCtx) -> Option<Vec<(LayerKey, Layer)>> {
        self.0.as_mut().and_then(|view| view.layers(ctx))
    }

    fn animating(&self, ctx: &ViewCtx) -> bool {
        self.0.as_ref().is_some_and(|view| view.animating(ctx))
    }
}

/// What is selected on the chart: the selectables' keys
/// (`keyflow::engraver::layout::chart::select`), in reading order, and
/// the one under the pointer. The pane writes it as it changes, when its
/// host gave it one in its extras. Both are presence: this person's view
/// of the chart, not the chart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChartSelection {
    pub keys: Vec<String>,
    /// The element under the pointer, highlighted as itself.
    pub hover: Option<String>,
}

/// The layers a chart is drawn in, by key. Each page is its own (its
/// number), so an edit re-sends only the pages it changed; the
/// selection, the hover, a lasso and the playhead are each one more, over
/// the pages, so a hover moving re-sends only its own small layer.
const SELECTION_LAYER: LayerKey = 1 << 32;
const LASSO_LAYER: LayerKey = SELECTION_LAYER + 1;
const PLAYHEAD_LAYER: LayerKey = SELECTION_LAYER + 2;
const HOVER_LAYER: LayerKey = SELECTION_LAYER + 3;
const INPUT_LAYER: LayerKey = SELECTION_LAYER + 4;

/// The layers' stacking over the pages (z 0).
const HOVER_Z: i32 = 1;
const SELECTION_Z: i32 = 2;
const PLAYHEAD_Z: i32 = 3;
const LASSO_Z: i32 = 4;
const INPUT_Z: i32 = 5;

/// How far, in chart points, a click may miss what it selects.
const PICK_SLOP: f64 = 3.0;

/// The theme tokens the selection and the hover are drawn in: a theme's
/// own `chart.select` / `chart.hover` (the engraver's `chart.*` tokens,
/// `ChartTheme::from_tokens`) when it has them, else the app's brand
/// colour (`primary`) and `info`. Not `accent`: in the app's tokens that
/// is a hover's background tint, near the paper.
const SELECTION_TOKENS: [&str; 2] = ["chart.select", "primary"];
const HOVER_TOKENS: [&str; 2] = ["chart.hover", "info"];

/// A chart that follows the app's theme without `chart.*` tokens of its
/// own is drawn on the app's card, in its card ink, with muted staff lines.
const CHART_TOKEN_FALLBACKS: [(&str, &str); 3] = [
    ("chart.paper", "card"),
    ("chart.ink", "card-foreground"),
    ("chart.staff", "muted-foreground"),
];

/// `name` from the pane's theme tokens, or the app token a `chart.*` one
/// falls back to.
fn chart_token(ctx: &ViewCtx, name: &str) -> Option<Color> {
    ctx.theme.color(name).or_else(|| {
        CHART_TOKEN_FALLBACKS
            .iter()
            .find(|(chart, _)| *chart == name)
            .and_then(|(_, app)| ctx.theme.color(app))
    })
}
/// Without a theme's tokens (a host that gives none): MuseScore's.
const SELECTION_FALLBACK: Color = Color::from_rgb8(0x00, 0x65, 0xBF);
const HOVER_FALLBACK: Color = Color::from_rgb8(0x2E, 0x9C, 0xE6);

/// The two colours a highlight takes, as the theme and the chart's paper
/// give them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HighlightColours {
    pub selection: Color,
    pub hover: Color,
}

impl HighlightColours {
    /// From the theme's tokens (`token`), made to read on `paper`: a
    /// token too close to the paper (a dark theme's light accent on a
    /// chart pinned to light paper) is mixed toward `ink` until it does.
    #[must_use]
    pub fn from_tokens(
        token: impl Fn(&str) -> Option<Color>,
        paper: Color,
        ink: Color,
    ) -> Self {
        let pick = |names: [&str; 2], fallback: Color| {
            names.iter().find_map(|n| token(n)).unwrap_or(fallback)
        };
        Self {
            selection: legible(pick(SELECTION_TOKENS, SELECTION_FALLBACK), paper, ink),
            hover: legible(pick(HOVER_TOKENS, HOVER_FALLBACK), paper, ink),
        }
    }

    /// The lasso's wash and edge: the selection colour, light.
    fn lasso(self) -> (Color, Color) {
        (
            self.selection.with_alpha(0.22),
            self.selection.with_alpha(0.86),
        )
    }
}

impl Default for HighlightColours {
    fn default() -> Self {
        Self {
            selection: SELECTION_FALLBACK,
            hover: HOVER_FALLBACK,
        }
    }
}

/// Relative luminance (WCAG), near enough for a contrast guard.
fn luminance(c: Color) -> f32 {
    let [r, g, b, _] = c.components;
    let lin = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio of two colours.
fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// `colour`, mixed toward `ink` in steps until it reads on `paper`
/// (3:1, WCAG's for graphics), opaque.
fn legible(colour: Color, paper: Color, ink: Color) -> Color {
    let mut c = colour.with_alpha(1.0);
    for _ in 0..6 {
        if contrast(c, paper) >= 3.0 {
            break;
        }
        c = mix(c, ink.with_alpha(1.0), 0.3);
    }
    c
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.components, b.components);
    Color::new([
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ])
}

/// A press that may become a lasso: where it started, in pane pixels, and
/// whether it adds to the selection.
#[derive(Clone, Copy)]
struct Press {
    from: (f64, f64),
    to: (f64, f64),
    add: bool,
    moved: bool,
}

/// What it last recorded, kept while what it shows is the same: a pan or
/// a zoom is a new transform on the same recordings.
#[derive(Default)]
struct Recorded {
    /// The engraver's generation the pages were recorded at.
    generation: Option<u64>,
    pages: Vec<(LayerKey, Layer)>,
    /// The selection's keys and colour it was drawn for, and its layer.
    selection: Option<((Vec<String>, Color), Layer)>,
    /// The hovered element's key and colour it was drawn for, and its layer.
    hover: Option<((String, Color), Layer)>,
    /// Note input's overlay and colour it was drawn for, and its layer.
    input: Option<((InputOverlay, Color), Layer)>,
    /// The playhead's time it was drawn at, and its layer.
    playhead: Option<(u64, Option<Layer>)>,
    /// The laid-out chart's selectables, at that generation.
    selectables: Vec<Selectable>,
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
    /// The theme last set, by its debug text's hash: set again only when
    /// it changed, so the recordings are kept.
    theme: Option<u64>,
    recorded: Recorded,
    selection: Selection<String>,
    /// The element under the pointer.
    hover: Option<String>,
    /// The theme's highlight colours, read at the last frame.
    colours: HighlightColours,
    press: Option<Press>,
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
            theme: None,
            recorded: Recorded::default(),
            selection: Selection::default(),
            hover: None,
            colours: HighlightColours::default(),
            press: None,
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

    /// The host's theme, or the pane's tokens', set on the engraver when
    /// it changed.
    fn update_theme(&mut self, ctx: &ViewCtx, snapshot: &ChartSnapshot) {
        let theme = snapshot
            .theme
            .clone()
            .unwrap_or_else(|| ChartTheme::from_tokens(|name| chart_token(ctx, name)));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{theme:?}").hash(&mut hasher);
        let hash = hasher.finish();
        self.colours = HighlightColours::from_tokens(
            |name| ctx.theme.color(name),
            theme.render.background_color,
            theme.render.foreground_color,
        );
        if self.theme != Some(hash) {
            self.theme = Some(hash);
            self.view.set_theme(theme);
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

    /// Chart pixels at 100% to the pane's logical pixels, for `snapshot`'s
    /// framing in a pane `ctx` measures.
    fn placement(&self, ctx: &ViewCtx, view: ChartView) -> Affine {
        let zoom = self.effective_zoom(view.zoom, ctx.geometry.width);
        let (x, y) = clamp_scroll(view.scroll, self.content, pane_size(ctx), zoom);
        Affine::scale(zoom) * Affine::translate((-x, -y))
    }

    /// The pages recorded again when the engraver's layout or theme
    /// changed; the same `Arc`s otherwise.
    fn record_pages(&mut self) {
        let generation = self.view.generation();
        if self.recorded.generation == Some(generation) {
            return;
        }
        let Some(pages) = self.view.record_pages() else {
            return;
        };
        self.recorded.generation = Some(generation);
        self.recorded.pages = pages
            .into_iter()
            .map(|page| {
                (
                    LayerKey::from(page.number),
                    Layer::new(Arc::new(page.commands)),
                )
            })
            .collect();
        self.recorded.selectables = self.view.selectables();
        // A new layout renumbers what can be selected: what was selected
        // and is still there stays.
        let keys: Vec<String> = self
            .recorded
            .selectables
            .iter()
            .map(|s| s.key.clone())
            .collect();
        let kept: Vec<String> = self
            .selection
            .items()
            .iter()
            .filter(|k| keys.contains(k))
            .cloned()
            .collect();
        if kept.len() != self.selection.items().len() {
            self.selection.lasso(kept, false);
        }
        if self.hover.as_ref().is_some_and(|h| !keys.contains(h)) {
            self.hover = None;
        }
        self.recorded.selection = None;
        self.recorded.hover = None;
        self.recorded.input = None;
        self.recorded.playhead = None;
    }

    /// `keys`' elements drawn as themselves in `colour`: each element's
    /// own ink, recoloured by the engraver ([`renderer::ChartView::
    /// record_elements`]); a bar, which is a box and not a mark, outlined
    /// as MuseScore outlines a selected measure.
    fn highlight(&self, keys: &[String], colour: Color, z: i32) -> Layer {
        let items: Vec<&Selectable> = self
            .recorded
            .selectables
            .iter()
            .filter(|s| keys.contains(&s.key))
            .collect();
        let nodes: Vec<&[usize]> = items.iter().filter_map(|s| s.node.as_deref()).collect();
        let mut commands = if nodes.is_empty() {
            Vec::new()
        } else {
            self.view
                .record_elements(&nodes, colour)
                .unwrap_or_default()
        };
        let bars: Vec<&&Selectable> = items.iter().filter(|s| s.node.is_none()).collect();
        if !bars.is_empty() {
            let mut scene = Scene::new();
            let px = points_to_px(1.0);
            for bar in bars {
                let b = bar.bounds.inflate(1.0, 2.0);
                let rect = Rect::new(b.x0 * px, b.y0 * px, b.x1 * px, b.y1 * px);
                scene.stroke(
                    &Stroke::new(2.0),
                    Affine::IDENTITY,
                    colour,
                    None,
                    &RoundedRect::from_rect(rect, 1.5),
                );
            }
            commands.extend(scene.commands);
        }
        Layer::new(Arc::new(commands)).z(z)
    }

    /// The selection's highlight, in chart pixels at 100%: drawn again
    /// only when the selection or the layout changed.
    fn selection_layer(&mut self) -> Option<Layer> {
        let keys = self.selection.items().to_vec();
        if keys.is_empty() {
            self.recorded.selection = None;
            return None;
        }
        let drawn_for = (keys, self.colours.selection);
        if let Some((drawn, layer)) = &self.recorded.selection
            && *drawn == drawn_for
        {
            return Some(layer.clone());
        }
        let layer = self.highlight(&drawn_for.0, drawn_for.1, SELECTION_Z);
        self.recorded.selection = Some((drawn_for, layer.clone()));
        Some(layer)
    }

    /// The element under the pointer, drawn as itself in the hover
    /// colour: drawn again only when the pointer moves onto another. A
    /// selected element shows as selected, not hovered.
    fn hover_layer(&mut self) -> Option<Layer> {
        let key = self
            .hover
            .clone()
            .filter(|k| !self.selection.contains(k))?;
        let drawn_for = (key, self.colours.hover);
        if let Some((drawn, layer)) = &self.recorded.hover
            && *drawn == drawn_for
        {
            return Some(layer.clone());
        }
        let layer = self.highlight(std::slice::from_ref(&drawn_for.0), drawn_for.1, HOVER_Z);
        self.recorded.hover = Some((drawn_for, layer.clone()));
        Some(layer)
    }

    /// The bar's box (chart points) the chart's bar `bar` is drawn in: its
    /// first, when it wraps.
    fn bar_box(&self, bar: usize) -> Option<Rect> {
        let key = format!("bar#{bar}");
        let part = format!("bar#{bar}.");
        self.recorded
            .selectables
            .iter()
            .find(|s| s.key == key || s.key.starts_with(&part))
            .map(|s| s.bounds)
    }

    /// Note input's cursor and shadow note, in chart pixels at 100%:
    /// drawn again only when they move.
    fn input_layer(&mut self, overlay: Option<InputOverlay>) -> Option<Layer> {
        let overlay = overlay?;
        let drawn_for = (overlay, self.colours.selection);
        if let Some((drawn, layer)) = &self.recorded.input
            && *drawn == drawn_for
        {
            return Some(layer.clone());
        }
        let bounds = self.bar_box(overlay.bar)?;
        let px = points_to_px(1.0);
        let colour = self.colours.selection;
        let mut scene = Scene::new();
        let w = bounds.width();
        let x0 = bounds.x0 + overlay.from.clamp(0.0, 1.0) * w;
        let x1 = (bounds.x0 + overlay.to.clamp(0.0, 1.0) * w).max(x0 + 4.0);
        let space = bounds.height() / 4.0;
        // MuseScore's note-input cursor: a translucent box in the voice's
        // colour over where the next note goes, a little past the staff.
        let cursor = Rect::new(
            x0 * px,
            (bounds.y0 - space) * px,
            x1 * px,
            (bounds.y1 + space) * px,
        );
        let shape = RoundedRect::from_rect(cursor, 2.0);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            colour.with_alpha(0.16),
            None,
            &shape,
        );
        scene.stroke(
            &Stroke::new(1.5),
            Affine::IDENTITY,
            colour.with_alpha(0.85),
            None,
            &shape,
        );
        if let Some(shadow) = overlay.shadow {
            draw_shadow(&mut scene, shadow, x0, bounds, colour.with_alpha(0.6));
        }
        let layer = Layer::new(Arc::new(scene.commands)).z(INPUT_Z);
        self.recorded.input = Some((drawn_for, layer.clone()));
        Some(layer)
    }

    /// The bar under the pane point `at`, and how far across it.
    fn bar_under(&self, ctx: &ViewCtx, view: ChartView, at: (f64, f64)) -> Option<(usize, f64)> {
        let point = self.chart_point(ctx, view, at);
        // The bar whose column the point is in, on the nearest staff within
        // a staff's height or two (a click on its chord symbols counts).
        self.recorded
            .selectables
            .iter()
            .filter(|s| s.key.starts_with("bar#"))
            .filter(|s| point.x >= s.bounds.x0 && point.x <= s.bounds.x1)
            .map(|s| {
                let b = s.bounds;
                let dy = if point.y < b.y0 {
                    b.y0 - point.y
                } else if point.y > b.y1 {
                    point.y - b.y1
                } else {
                    0.0
                };
                (s, dy)
            })
            .filter(|(s, dy)| *dy <= s.bounds.height() * 2.5)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(s, _)| s)
            .and_then(|s| {
                let number = s.key.trim_start_matches("bar#").split('.').next()?;
                let bar = number.parse::<usize>().ok()?;
                let fraction = (point.x - s.bounds.x0) / s.bounds.width().max(1.0);
                Some((bar, fraction.clamp(0.0, 1.0)))
            })
    }

    /// The element under the pane point `at`, if any.
    fn under(&self, ctx: &ViewCtx, view: ChartView, at: (f64, f64)) -> Option<String> {
        let point = self.chart_point(ctx, view, at);
        pick(&self.recorded.selectables, point, PICK_SLOP)
            .and_then(|i| self.recorded.selectables.get(i))
            .map(|s| s.key.clone())
    }

    /// The pointer moved to `at`: what it is over now. Whether that
    /// changed (a frame is needed).
    fn hover_at(&mut self, ctx: &ViewCtx, view: ChartView, at: (f64, f64)) -> bool {
        let under = self.under(ctx, view, at);
        if under == self.hover {
            return false;
        }
        self.hover = under;
        true
    }

    /// The playhead at `secs`, in chart pixels at 100%: drawn again only
    /// when the time moved.
    fn playhead_layer(&mut self, secs: Option<f64>) -> Option<Layer> {
        let secs = secs?;
        let bits = secs.to_bits();
        if let Some((at, layer)) = &self.recorded.playhead
            && *at == bits
        {
            return layer.clone();
        }
        let mut scene = Scene::new();
        let layer = self
            .view
            .paint_playhead(&mut scene, secs)
            .then(|| Layer::new(Arc::new(scene.commands)).z(PLAYHEAD_Z));
        self.recorded.playhead = Some((bits, layer.clone()));
        layer
    }

    /// The lasso being drawn, in the pane's pixels.
    fn lasso_layer(&self) -> Option<Layer> {
        let press = self.press.filter(|p| p.moved)?;
        let rect = Rect::from_points(press.from, press.to);
        let (wash, edge) = self.colours.lasso();
        let mut scene = Scene::new();
        scene.fill(Fill::NonZero, Affine::IDENTITY, wash, None, &rect);
        scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, edge, None, &rect);
        Some(Layer::new(Arc::new(scene.commands)).z(LASSO_Z))
    }

    /// A pane point in chart points, under `snapshot`'s framing.
    fn chart_point(&self, ctx: &ViewCtx, view: ChartView, at: (f64, f64)) -> Point {
        let px = self.placement(ctx, view).inverse() * Point::new(at.0, at.1);
        let per = points_to_px(1.0);
        Point::new(px.x / per, px.y / per)
    }

    /// A press of the main button at `at` with `mods` held: what it
    /// selects, as MuseScore's view does (Shift a range, Cmd/Ctrl one
    /// more). A press on nothing starts a lasso.
    fn press(&mut self, ctx: &ViewCtx, view: ChartView, at: (f64, f64), mods: &[&str]) -> bool {
        let shift = mods.contains(&"Shift");
        let command = mods.contains(&"Mod");
        let Some(key) = self.under(ctx, view, at) else {
            self.press = Some(Press {
                from: at,
                to: at,
                add: shift || command,
                moved: false,
            });
            return false;
        };
        let order: Vec<String> = self
            .recorded
            .selectables
            .iter()
            .map(|s| s.key.clone())
            .collect();
        match Gesture::of(shift, command) {
            Gesture::Single => self.selection.click(key),
            Gesture::Range => self.selection.extend(key, &order),
            Gesture::Toggle => self.selection.toggle(key),
        }
        true
    }

    /// The press let go: a lasso selects what it covers; a click on
    /// nothing clears the selection.
    fn release(&mut self, ctx: &ViewCtx, view: ChartView) -> bool {
        let Some(press) = self.press.take() else {
            return false;
        };
        if !press.moved {
            if press.add || self.selection.is_empty() {
                return false;
            }
            self.selection.clear();
            return true;
        }
        let a = self.chart_point(ctx, view, press.from);
        let b = self.chart_point(ctx, view, press.to);
        let covered: Vec<String> = in_box(&self.recorded.selectables, Rect::from_points(a, b))
            .into_iter()
            .filter_map(|i| self.recorded.selectables.get(i))
            .map(|s| s.key.clone())
            .collect();
        self.selection.lasso(covered, press.add);
        true
    }

    /// The selection out to the host, when it asked for it.
    fn publish(&self, ctx: &ViewCtx) {
        if let Some(out) = ctx.data::<ChartSelection>() {
            let now = ChartSelection {
                keys: self.selection.items().to_vec(),
                hover: self.hover.clone(),
            };
            if out.read(|s| *s != now) {
                out.set(now);
            }
        }
    }
}

impl PaintView for ChartPaintView {
    fn input(&mut self, ctx: &ViewCtx, event: &ViewInput) -> bool {
        let Some(data) = ctx.data::<ChartSnapshot>() else {
            return false;
        };
        let pane = pane_size(ctx);
        match event {
            ViewInput::Key {
                key,
                down,
                modifiers,
            } => {
                self.ctrl = modifiers.contains(&"Ctrl");
                // The host's keymap reads it: the view binds no keys.
                if *down && let Some(gestures) = ctx.extras.get::<ChartGestures>() {
                    (gestures.0)(ChartGesture::Key {
                        key: key.clone(),
                        mods: modifiers.clone(),
                    });
                }
                false
            }
            ViewInput::Focus(false) => {
                self.ctrl = false;
                self.panning = false;
                self.press = None;
                true
            }
            ViewInput::PointerDown {
                x, y, button, mods, ..
            } => {
                self.pointer = point(*x, *y);
                match button {
                    Button::Middle => {
                        self.panning = true;
                        false
                    }
                    Button::Primary => {
                        let view = data.read(|s| s.view);
                        if let Some(gestures) = ctx.extras.get::<ChartGestures>()
                            && let Some((bar, fraction)) = self.bar_under(ctx, view, self.pointer)
                        {
                            (gestures.0)(ChartGesture::Click { bar, fraction });
                        }
                        let changed = self.press(ctx, view, self.pointer, mods);
                        if changed {
                            self.publish(ctx);
                        }
                        changed
                    }
                    Button::Secondary => false,
                }
            }
            ViewInput::PointerUp { x, y, button, .. } => {
                self.pointer = point(*x, *y);
                match button {
                    Button::Middle => {
                        self.panning = false;
                        false
                    }
                    _ => {
                        let view = data.read(|s| s.view);
                        let changed = self.release(ctx, view);
                        if changed {
                            self.publish(ctx);
                        }
                        changed
                    }
                }
            }
            ViewInput::PointerMove { x, y, .. } => {
                let from = std::mem::replace(&mut self.pointer, point(*x, *y));
                if let Some(press) = &mut self.press {
                    press.to = self.pointer;
                    let (dx, dy) = (press.to.0 - press.from.0, press.to.1 - press.from.1);
                    press.moved |= dx.hypot(dy) > 3.0;
                    return press.moved;
                }
                if !self.panning {
                    let view = data.read(|s| s.view);
                    let changed = self.hover_at(ctx, view, self.pointer);
                    if changed {
                        self.publish(ctx);
                    }
                    return changed;
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
                // What is under the pointer moved under it.
                let view = data.read(|s| s.view);
                if self.hover_at(ctx, view, pointer) {
                    self.publish(ctx);
                }
                true
            }
            _ => false,
        }
    }

    fn paint(&mut self, ctx: &ViewCtx, scene: &mut Scene) {
        // A host that composes no layers: the same layers, composed here.
        if let Some(layers) = self.layers(ctx) {
            let mut sorted = layers;
            sorted.sort_by_key(|(key, layer)| (layer.z, *key));
            for (_, layer) in sorted {
                let mut content = Scene::new();
                content.commands.clone_from(&layer.commands);
                scene.append_scene(content, layer.transform);
            }
        }
    }

    fn layers(&mut self, ctx: &ViewCtx) -> Option<Vec<(LayerKey, Layer)>> {
        let data = ctx.data::<ChartSnapshot>()?;
        let snapshot = data.get();
        self.update_theme(ctx, &snapshot);
        self.update_layout(&snapshot);
        self.record_pages();
        if let Some(size) = self.view.content_size() {
            self.content = (size.0 * points_to_px(1.0), size.1 * points_to_px(1.0));
        }
        let at = self.placement(ctx, snapshot.view);
        let mut layers: Vec<(LayerKey, Layer)> = self
            .recorded
            .pages
            .iter()
            .map(|(key, page)| (*key, page.clone().at(at)))
            .collect();
        if let Some(hover) = self.hover_layer() {
            layers.push((HOVER_LAYER, hover.at(at)));
        }
        if let Some(selection) = self.selection_layer() {
            layers.push((SELECTION_LAYER, selection.at(at)));
        }
        if let Some(input) = self.input_layer(snapshot.input) {
            layers.push((INPUT_LAYER, input.at(at)));
        }
        if let Some(playhead) = self.playhead_layer(snapshot.playhead_secs) {
            layers.push((PLAYHEAD_LAYER, playhead.at(at)));
        }
        if let Some(lasso) = self.lasso_layer() {
            layers.push((LASSO_LAYER, lasso));
        }
        Some(layers)
    }

    fn animating(&self, _ctx: &ViewCtx) -> bool {
        self.requested.is_some()
    }
}

/// The shadow note at `x` (chart points) in the staff `bar` (its box):
/// a notehead at its staff position, open for a half or a whole, with
/// ledger lines off the staff; a rest as a block on the middle line.
fn draw_shadow(scene: &mut Scene, shadow: ShadowNote, x: f64, bar: Rect, colour: Color) {
    use architect_workspace::paint::peniko::kurbo::{Ellipse, Line};
    let px = points_to_px(1.0);
    let space = bar.height() / 4.0;
    let head_w = space * 1.3;
    let cx = x + space * 0.5 + head_w / 2.0;
    if shadow.rest {
        let r = Rect::new(cx - space * 0.4, bar.y0 + space * 1.5, cx + space * 0.4, bar.y0 + space * 2.5);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            colour,
            None,
            &Rect::new(r.x0 * px, r.y0 * px, r.x1 * px, r.y1 * px),
        );
        return;
    }
    let y = bar.y0 + f64::from(shadow.below_top) * space / 2.0;
    // Ledger lines from the staff out to the note.
    let ledger = Stroke::new(1.0);
    let mut lines = Vec::new();
    let mut step = -2;
    while step >= shadow.below_top {
        lines.push(step);
        step -= 2;
    }
    let mut step = 10;
    while step <= shadow.below_top {
        lines.push(step);
        step += 2;
    }
    for step in lines {
        let ly = bar.y0 + f64::from(step) * space / 2.0;
        let line = Line::new(
            ((cx - head_w * 0.9) * px, ly * px),
            ((cx + head_w * 0.9) * px, ly * px),
        );
        scene.stroke(&ledger, Affine::IDENTITY, colour, None, &line);
    }
    let head = Ellipse::new((cx * px, y * px), (head_w / 2.0 * px, space / 2.0 * px), -0.35);
    if shadow.open {
        scene.stroke(&Stroke::new(1.6), Affine::IDENTITY, colour, None, &head);
    } else {
        scene.fill(Fill::NonZero, Affine::IDENTITY, colour, None, &head);
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
    fn highlight_colours_come_from_the_theme_and_read_on_the_paper() {
        let white = Color::WHITE;
        let black = Color::BLACK;
        // A light theme's brand colour and info, on light paper: as they are.
        let teal = Color::from_rgb8(0x0f, 0x76, 0x6e);
        let blue = Color::from_rgb8(0x1d, 0x4e, 0xd8);
        let tokens = |name: &str| match name {
            "primary" => Some(teal),
            "info" => Some(blue),
            _ => None,
        };
        let c = HighlightColours::from_tokens(tokens, white, black);
        assert_eq!((c.selection, c.hover), (teal, blue));
        assert_ne!(c.selection, c.hover, "two states, two colours");

        // A theme's own chart tokens win over the app's.
        let red = Color::from_rgb8(0xb9, 0x1c, 0x1c);
        let c = HighlightColours::from_tokens(
            |name| (name == "chart.select").then_some(red).or_else(|| tokens(name)),
            white,
            black,
        );
        assert_eq!(c.selection, red);

        // A dark theme's light brand colour on a chart pinned to light paper is
        // darkened until it reads; on dark paper it is left alone.
        let light_teal = Color::from_rgb8(0x2d, 0xd4, 0xbf);
        let dark_tokens = |name: &str| (name == "primary").then_some(light_teal);
        let on_white = HighlightColours::from_tokens(dark_tokens, white, black);
        assert!(contrast(on_white.selection, white) >= 3.0);
        let paper = Color::from_rgb8(25, 30, 45);
        let ink = Color::from_rgb8(245, 240, 220);
        let on_dark = HighlightColours::from_tokens(dark_tokens, paper, ink);
        assert_eq!(on_dark.selection, light_teal);
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
