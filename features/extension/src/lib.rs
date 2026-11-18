//! Keyflow as an Architect extension: a `.kf` chart opened in a pane is
//! engraved there, in the app's theme.
//!
//! One value, [`Keyflow`], runs two ways (`extension.toml`): linked into
//! Architect or Studio as a built-in, or served as a process by its
//! program (`src/main.rs`) when a development copy runs.
//!
//! A chart is a document of the app's: the pane's state is its path, its
//! text is read through the host (`read_document`), and a save anywhere
//! (another person's included: the app's documents are CRDT replicas)
//! reads it again through the `doc.saved` hook. The text is the chart's
//! storage, not its editor: editing happens on the chart.
//!
//! The view is whole on its own, as MuseScore's score view: its palettes
//! are a sidebar inside it ([`palette`]) and its note-input toolbar is an
//! Architect toolbar on its own pane (`where = "view:keyflow/chart"`), so it
//! docks into another app with both. The toolbar's tools ([`tools`]) are
//! actions, lit while on; their icons are the chart's own music font
//! ([`icons`]).

pub mod icons;
pub mod palette;
pub mod tools;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use architect_extensions::contrib::{StateKind, StateWrite};
use architect_extensions::hostcall::HostRequest;
use keyflow_session::ChartSource;
use architect_extensions::linked::{Builder, Extension, Handle, PaintPane, use_pane_state};
use architect_workspace::paint::peniko::Color;
use architect_workspace::{HostData, PaintWake};
use dioxus::prelude::*;
use keyflow::engraver::renderer::ChartTheme;
use keyflow_ui::paint_view::{ChartSelection, ChartSnapshot, ChartView, chart_mount};
use serde_json::{Value as Json, json};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// The manifest, for a host that reads it without building the extension.
pub const MANIFEST: &str = include_str!("../extension.toml");

/// The Keyflow extension.
pub struct Keyflow;

impl Extension for Keyflow {
    fn manifest(&self) -> &'static str {
        MANIFEST
    }

    /// The toolbar's icons, drawn from the chart's music font.
    fn files(&self) -> &'static [(&'static str, &'static str)] {
        icons::files()
    }

    fn build(&self, ext: &mut Builder) {
        *HANDLE.lock().unwrap_or_else(PoisonError::into_inner) = Some(ext.handle());
        for tool in tools::TOOLS {
            let command = tool.command;
            ext.action(tool.action, move |_| async move {
                let on = {
                    let mut input = input();
                    input.apply(command);
                    input.is_on(command)
                };
                publish_input();
                Ok(json!({ "on": on }))
            });
        }
        ext.view("chart", ChartPane)
            .action("KEYFLOW_STATE", |_| async { Ok(state()) })
            .action("KEYFLOW_CHART_VIEW", |args| async move { set_view(&args) })
            .action(
                "KEYFLOW_SET_CHART_THEME",
                |args| async move { set_theme(&args) },
            )
            .action_in("KEYFLOW_RELOAD", |args, cx| async move {
                // A hook's path is its invocation's; a person's is an argument.
                let path = text(&args, "path").or_else(|| text(&cx, "path"));
                reload(path.as_deref());
                Ok(json!({ "ok": true }))
            });
    }
}

/// The host this extension's builder was given: how a pane reads its
/// chart. The latest build's (a reload builds it again).
static HANDLE: Mutex<Option<Handle>> = Mutex::new(None);

/// One chart open in a pane.
#[derive(Clone)]
struct Open {
    path: String,
    data: HostData<ChartSnapshot>,
    /// What is selected on it, as its pane writes it.
    selection: HostData<ChartSelection>,
    /// Its pane's frames: how long the view took to give each one.
    stats: architect_workspace::paint::FrameStats,
    wake: PaintWake,
    error: Option<String>,
    /// Tells the pane something changed (from any thread: an action's).
    tell: UnboundedSender<Msg>,
}

/// What a pane is told from outside its own thread.
#[derive(Clone, Copy, Debug)]
enum Msg {
    /// Read the document again.
    Reread,
    /// The input state changed (the palettes shown or hidden).
    Input,
}

/// The note-input state the toolbar sets, one for the app: as MuseScore's,
/// it is the tool chosen, whichever score it is used on.
static INPUT: std::sync::LazyLock<Mutex<tools::InputState>> =
    std::sync::LazyLock::new(|| Mutex::new(tools::InputState::default()));

fn input() -> std::sync::MutexGuard<'static, tools::InputState> {
    INPUT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Lights each tool's button as the input state says, and tells the panes.
fn publish_input() {
    let state = input().clone();
    if let Some(handle) = HANDLE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    {
        for tool in tools::TOOLS {
            handle.set_state(StateWrite {
                key: tool.action.to_owned(),
                kind: StateKind::Checked,
                text: if state.is_on(tool.command) {
                    "on"
                } else {
                    "off"
                }
                .to_owned(),
                ..StateWrite::default()
            });
        }
    }
    for open in open_charts().values() {
        let _ = open.tell.send(Msg::Input);
    }
}

/// Every chart open in a pane, by a key of the pane's mount.
static OPEN: Mutex<BTreeMap<u64, Open>> = Mutex::new(BTreeMap::new());
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Whether the charts are dark. Light paper by default, whatever the app's
/// chrome: a chart reads best as a page. `KEYFLOW_SET_CHART_THEME` pins it
/// dark or light, or (`follow`) hands it to the app's theme.
static DARK: AtomicBool = AtomicBool::new(false);
static PINNED: AtomicBool = AtomicBool::new(true);

/// The theme a pinned chart is drawn in.
fn pinned_theme() -> ChartTheme {
    if DARK.load(Ordering::Relaxed) {
        dark_theme()
    } else {
        ChartTheme::default()
    }
}

fn open_charts() -> std::sync::MutexGuard<'static, BTreeMap<u64, Open>> {
    OPEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The chart pane: the chart at the pane's path, engraved.
#[component]
fn ChartPane() -> Element {
    // Its pane's state, linked in or as a process (its host tells it).
    let path = use_pane_state().map(|(_, state)| state).unwrap_or_default();
    let key = use_hook(|| NEXT.fetch_add(1, Ordering::Relaxed));
    let mut reads = use_signal(|| 0_u64);
    let mut error = use_signal(|| None::<String>);
    let mut palettes = use_signal(|| input().palettes);
    // What is told off the UI thread lands here: a reread bumps `reads`, an
    // input change shows or hides the palettes.
    let tell = use_hook(|| {
        let (tx, mut rx) = unbounded_channel::<Msg>();
        spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Msg::Reread => reads += 1,
                    Msg::Input => palettes.set(input().palettes),
                }
            }
        });
        tx
    });
    // The toolbar's buttons lit as the input state is, once a chart shows.
    use_hook(publish_input);
    let mount = use_hook(|| {
        let data = HostData::new(ChartSnapshot {
            chart: keyflow::Chart::new(),
            revision: 0,
            playhead_secs: None,
            theme: None,
            view: ChartView::default(),
        });
        let mount = chart_mount(data.clone());
        open_charts().insert(
            key,
            Open {
                path: path.clone(),
                data,
                selection: mount.selection.clone(),
                stats: mount.mount.frame_stats(),
                wake: mount.wake.clone(),
                error: None,
                tell: tell.clone(),
            },
        );
        mount
    });
    use_drop(move || {
        open_charts().remove(&key);
    });

    // The pane's state is its path, and an opener may reuse the pane for
    // another chart: follow it, as a signal the read below depends on.
    let mut current = use_signal(|| path.clone());
    if *current.peek() != path {
        current.set(path.clone());
        if let Some(open) = open_charts().get_mut(&key) {
            open.path.clone_from(&path);
        }
    }

    // Read and engrave the document whenever the path or `reads` moves.
    let _ = use_resource(move || {
        let path = current();
        let _ = reads();
        async move {
            let outcome = read_chart(&path).await;
            let message = outcome.as_ref().err().cloned();
            if let Some(open) = open_charts().get_mut(&key) {
                if let Ok(chart) = outcome {
                    open.data.update(|s| {
                        s.chart = chart;
                        s.revision = s.revision.wrapping_add(1);
                    });
                    open.wake.wake();
                }
                open.error.clone_from(&message);
            }
            error.set(message);
        }
    });

    // Light paper unless pinned otherwise; the app's theme when following it.
    let colours = architect_theme::use_themed::<ChartColours>();
    use_effect(move || {
        let c = colours();
        let theme = if PINNED.load(Ordering::Relaxed) {
            pinned_theme()
        } else {
            DARK.store(c.dark, Ordering::Relaxed);
            c.theme()
        };
        if let Some(open) = open_charts().get(&key) {
            open.data.update(|s| s.theme = Some(theme));
            open.wake.wake();
        }
    });

    rsx! {
        div { style: "display:flex; flex-direction:row; width:100%; height:100%; min-height:0;",
            if palettes() {
                palette::Palettes {}
            }
            div { style: "display:flex; flex-direction:column; flex:1; min-width:0; height:100%; min-height:0;",
                if path.is_empty() {
                    div { style: "padding:16px; opacity:0.7;", "Open a .kf chart to see it here." }
                } else if let Some(message) = error() {
                    div { style: "padding:8px 16px; font-size:13px; color:var(--danger, #c33);",
                        "{path}: {message}"
                    }
                }
                div { style: "flex:1; min-height:0;",
                    PaintPane { mount: mount.mount.clone() }
                }
            }
        }
    }
}

/// The chart at `path`, read through the host and parsed.
async fn read_chart(path: &str) -> Result<keyflow::Chart, String> {
    if path.is_empty() {
        return Err("no chart".to_owned());
    }
    let handle = HANDLE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or("the extension is not loaded")?;
    // The pane's state names a document (the chart alone) or a session
    // (`session:<id>`, the chart as Keyflow's data there).
    let source = ChartSource::of_pane_state(path);
    let text = match source.read_call().map_err(|e| e.to_string())? {
        None => handle
            .host(HostRequest::ReadDocument {
                path: path.to_owned(),
            })
            .await
            .map_err(|e| format!("{e:?}"))?
            .as_str()
            .ok_or("the document is not text")?
            .to_owned(),
        Some(call) => {
            let reply = handle
                .host(service_request(&call)?)
                .await
                .map_err(|e| format!("{e:?}"))?;
            ChartSource::chart_of(&reply.to_string()).map_err(|e| e.to_string())?
        }
    };
    keyflow::parse(&text).map_err(|e| e.to_string())
}

/// A `fts.session` call as the host request that makes it (the host runs
/// the provider's action as this extension).
fn service_request(call: &keyflow_session::ServiceCall) -> Result<HostRequest, String> {
    Ok(HostRequest::Service {
        service: call.service.to_owned(),
        method: call.method.to_owned(),
        args: serde_json::from_str(&call.args).map_err(|e| e.to_string())?,
    })
}

/// The open charts, for `KEYFLOW_STATE`.
fn state() -> Json {
    let charts: Vec<Json> = open_charts()
        .values()
        .map(|o| {
            let view = o.data.read(|s| s.view);
            let frames = o.stats.summary();
            let ms = |d: std::time::Duration| (d.as_secs_f64() * 100_000.0).round() / 100.0;
            json!({
                "path": o.path,
                "error": o.error,
                "zoom": view.zoom,
                "scroll": [view.scroll.0, view.scroll.1],
                "selection": o.selection.read(|s| s.keys.clone()),
                "frames": {
                    "count": frames.frames,
                    "p50_ms": ms(frames.p50),
                    "p99_ms": ms(frames.p99),
                    "max_ms": ms(frames.max),
                },
            })
        })
        .collect();
    let on: Vec<&str> = {
        let state = input();
        tools::TOOLS
            .iter()
            .filter(|t| state.is_on(t.command))
            .map(|t| t.action)
            .collect()
    };
    json!({ "charts": charts, "dark": DARK.load(Ordering::Relaxed), "tools_on": on })
}

/// `KEYFLOW_CHART_VIEW`.
fn set_view(args: &Json) -> Result<Json, String> {
    let zoom = match args.get("zoom") {
        None | Some(Json::Null) => None,
        Some(z) => Some(z.as_f64().ok_or("zoom: a number or null")?.clamp(0.25, 4.0)),
    };
    let scroll = (
        args.get("scrollX").and_then(Json::as_f64).unwrap_or(0.0),
        args.get("scrollY").and_then(Json::as_f64).unwrap_or(0.0),
    );
    let path = text(args, "path");
    for open in open_charts().values() {
        if path.as_deref().is_none_or(|p| p == open.path) {
            open.data.update(|s| s.view = ChartView { zoom, scroll });
            open.wake.wake();
        }
    }
    Ok(json!({ "ok": true }))
}

/// `KEYFLOW_SET_CHART_THEME`.
fn set_theme(args: &Json) -> Result<Json, String> {
    if args.get("follow").and_then(Json::as_bool) == Some(true) {
        // The next theme change repaints them in it.
        PINNED.store(false, Ordering::Relaxed);
        return Ok(json!({ "ok": true }));
    }
    let dark = args
        .get("dark")
        .and_then(Json::as_bool)
        .ok_or("dark: a boolean (or follow: true)")?;
    PINNED.store(true, Ordering::Relaxed);
    DARK.store(dark, Ordering::Relaxed);
    let theme = pinned_theme();
    for open in open_charts().values() {
        open.data.update(|s| s.theme = Some(theme.clone()));
        open.wake.wake();
    }
    Ok(json!({ "ok": true }))
}

/// Has the open charts (`path`: only that one) read their documents again.
fn reload(path: Option<&str>) {
    for open in open_charts().values() {
        if path.is_none_or(|p| p == open.path) {
            // A pane gone since is a closed channel: nothing to tell.
            let _ = open.tell.send(Msg::Reread);
        }
    }
}

fn text(args: &Json, key: &str) -> Option<String> {
    args.get(key).and_then(Json::as_str).map(str::to_owned)
}

fn dark_theme() -> ChartTheme {
    let mut theme = ChartTheme::default();
    theme.render.background_color = Color::from_rgb8(25, 30, 45);
    theme.render.foreground_color = Color::from_rgb8(245, 240, 220);
    theme.render.staff_color = Color::from_rgb8(210, 210, 205);
    theme
}

/// The chart's slice of the app's theme: its paper, its ink, its staff.
#[derive(Clone, PartialEq)]
struct ChartColours {
    dark: bool,
    paper: architect_theme::Rgba,
    ink: architect_theme::Rgba,
    staff: architect_theme::Rgba,
}

impl architect_theme::FromResolved for ChartColours {
    fn from_resolved(r: &architect_theme::Resolved) -> Self {
        Self {
            dark: r.scheme() == architect_theme::Scheme::Dark,
            paper: r.color("card"),
            ink: r.color("card-foreground"),
            staff: r.color("muted-foreground"),
        }
    }
}

impl ChartColours {
    fn theme(&self) -> ChartTheme {
        let c = |x: architect_theme::Rgba| Color::new([x.r, x.g, x.b, x.a]);
        let mut theme = ChartTheme::default();
        theme.render.background_color = c(self.paper);
        theme.render.foreground_color = c(self.ink);
        theme.render.staff_color = c(self.staff);
        theme
    }
}
