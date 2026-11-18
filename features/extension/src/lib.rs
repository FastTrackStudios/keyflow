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
pub mod input;
pub mod keep;
pub mod palette;
#[cfg(not(target_family = "wasm"))]
pub mod session_host;
pub mod tools;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use architect_extensions::contrib::{StateKind, StateWrite};
use architect_extensions::hostcall::{DocEdit, HostRequest};
use architect_extensions::linked::{Builder, Extension, Handle, PaintPane, use_pane_state};
use architect_workspace::paint::peniko::Color;
use architect_workspace::{HostData, PaintWake};
use dioxus::prelude::*;
use keyflow::edit::{self, Mode, Op, Pos, Score};
use keyflow::engraver::renderer::ChartTheme;
use keyflow_session::ChartSource;
use keyflow_ui::paint_view::{
    ChartGesture, ChartGestures, ChartSelection, ChartSnapshot, ChartView, InputOverlay,
    PointerShadow, ShadowNote, chart_mount_with,
};
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
        // `fts.session`: Keyflow as a session's host (native only).
        #[cfg(not(target_family = "wasm"))]
        session_host::build(ext);
        for tool in tools::TOOLS {
            let command = tool.command;
            ext.action(tool.action, move |_| async move {
                let now = edit_state().input.mode;
                if let Some(op) = command.op(now) {
                    return run_op(move |_| vec![op]).await;
                }
                let on = {
                    let mut input = input();
                    input.apply(command);
                    input.is_on(&edit_state().input, command)
                };
                publish_input();
                Ok(json!({ "on": on }))
            });
        }
        for action in input::OP_ACTIONS {
            let op = action.op;
            ext.action(action.action, move |_| async move {
                run_op(move |_| vec![op]).await
            });
        }
        ext.action("KEYFLOW_INPUT_MOVE", |args| async move {
            let bar = args
                .get("bar")
                .and_then(Json::as_u64)
                .ok_or("bar: a number")? as usize;
            let fraction = args.get("fraction").and_then(Json::as_f64).unwrap_or(0.0);
            let target = match args.get("target").and_then(Json::as_str) {
                Some("chords") => Some(edit::Target::Chords),
                Some("notes") => Some(edit::Target::Notes),
                _ => None,
            };
            let select = args.get("select").and_then(Json::as_bool).unwrap_or(true);
            let out = run_op(move |score| {
                let length = score.bars.get(bar).map_or(0, |b| b.length);
                let at = Op::MoveTo(Pos {
                    bar,
                    tick: (fraction.clamp(0.0, 1.0) * f64::from(length)) as u32,
                });
                // A note clicked in a bar with no melody is a rhythm
                // slash: it is the bar's chords.
                let target = target.map(|t| match t {
                    edit::Target::Notes
                        if !(0..4)
                            .any(|v| score.voice_bars(v).get(bar).is_some_and(|b| b.written)) =>
                    {
                        edit::Target::Chords
                    }
                    t => t,
                });
                target.map_or_else(|| vec![at], |t| vec![Op::Target(t), at])
            })
            .await;
            if !select {
                edit_state().input.selected = None;
                rerender_input().await;
            }
            out
        });
        ext.action("KEYFLOW_INPUT_ENTER_AT", |args| async move {
            let bar = args
                .get("bar")
                .and_then(Json::as_u64)
                .ok_or("bar: a number")? as usize;
            let fraction = args.get("fraction").and_then(Json::as_f64).unwrap_or(0.0);
            let below_top = args.get("below_top").and_then(Json::as_i64).unwrap_or(4) as i32;
            let top = TOP_STEP.load(Ordering::Relaxed);
            run_op(move |score| {
                let length = score.bars.get(bar).map_or(0, |b| b.length);
                vec![
                    Op::MoveTo(Pos {
                        bar,
                        tick: (fraction.clamp(0.0, 1.0) * f64::from(length)) as u32,
                    }),
                    Op::EnterStep(top - below_top),
                ]
            })
            .await
        });
        ext.action("KEYFLOW_TARGET_SWITCH", |_| async {
            let next = match edit_state().input.target {
                edit::Target::Chords => edit::Target::Notes,
                edit::Target::Notes => edit::Target::Chords,
            };
            run_op(move |_| vec![Op::Target(next)]).await
        });
        ext.action("KEYFLOW_SYNTAX_PREVIEW", |args| async move {
            let typed = text(&args, "text").unwrap_or_default();
            syntax_preview(&typed).await
        });
        ext.action("KEYFLOW_SYNTAX_WRITE", |args| async move {
            let typed = text(&args, "text").ok_or("text: the bars to write")?;
            syntax_write(&typed).await
        });
        ext.action("KEYFLOW_INPUT_STATE", |_| async { Ok(input_state()) });
        ext.action("KEYFLOW_UNDO", |_| async { history(true).await });
        ext.action("KEYFLOW_REDO", |_| async { history(false).await });
        ext.action("KEYFLOW_KEYMAP_RELOAD", |_| async {
            load_keymap().await;
            Ok(json!({ "keys": edit_state().keymap.len() }))
        });
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
#[derive(Clone, Debug)]
enum Msg {
    /// Read the document again.
    Reread,
    /// The input state changed (the palettes shown or hidden, the mode,
    /// the status line).
    Input,
    /// A key pressed or a bar clicked on its chart.
    Gesture(ChartGesture),
    /// Time to keep the view, if it changed.
    Keep,
}

/// Note input: one person's, for the app, as MuseScore's (the tools and
/// the cursor are theirs, whichever chart they are on).
#[derive(Default)]
struct EditState {
    input: edit::InputState,
    /// What the status line says.
    status: String,
    /// What the last operation could not do.
    refused: Option<String>,
    /// The pane note input is on (its mount's key), the last one used.
    active: Option<u64>,
    /// The last bar clicked and how far across: where note input starts.
    clicked: Option<(usize, f64)>,
    /// Syntax typed and not yet written: the bars it would write from the
    /// cursor, shown on the chart as they would be.
    preview: Option<usize>,
    /// Syntax mode's typing: keys typed on the chart (it keeps the
    /// keyboard, so Esc leaves the chart ready for the next key).
    draft: String,
    /// Shortcut → action: MuseScore's keys, as the person's settings
    /// change them (`keyflow.keys.*`).
    keymap: BTreeMap<String, String>,
}

static EDIT: std::sync::LazyLock<Mutex<EditState>> = std::sync::LazyLock::new(|| {
    Mutex::new(EditState {
        keymap: input::default_keys()
            .map(|(action, key)| (key.to_owned(), action.to_owned()))
            .collect(),
        ..EditState::default()
    })
});

fn edit_state() -> std::sync::MutexGuard<'static, EditState> {
    EDIT.lock().unwrap_or_else(PoisonError::into_inner)
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
    let edit = edit_state().input.clone();
    if let Some(handle) = HANDLE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    {
        for tool in tools::TOOLS {
            handle.set_state(StateWrite {
                key: tool.action.to_owned(),
                kind: StateKind::Checked,
                text: if state.is_on(&edit, tool.command) {
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

/// The theme a chart is given: a pinned one's, or `None` to follow the
/// app's theme tokens.
fn chart_theme() -> Option<ChartTheme> {
    PINNED.load(Ordering::Relaxed).then(pinned_theme)
}

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
    let mut status = use_signal(|| edit_state().status.clone());
    let mut mode = use_signal(|| edit_state().input.mode);
    let mut target = use_signal(|| edit_state().input.target);
    let mut draft = use_signal(String::new);
    let tell = use_hook(|| {
        let (tx, mut rx) = unbounded_channel::<Msg>();
        spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Msg::Reread => reads += 1,
                    Msg::Input => {
                        palettes.set(input().palettes);
                        let e = edit_state();
                        status.set(match &e.refused {
                            Some(why) => format!("{why} · {}", e.status),
                            None => e.status.clone(),
                        });
                        mode.set(e.input.mode);
                        target.set(e.input.target);
                        draft.set(e.draft.clone());
                    }
                    // In order, one at a time: C D E typed fast is C D E.
                    Msg::Gesture(g) => {
                        // Another chart: note input starts afresh in it (a
                        // cursor at bar 6 of the last chart means nothing
                        // here), as MuseScore leaves note input on another
                        // score.
                        let switched = {
                            let mut e = edit_state();
                            let switched = e.active.is_some_and(|a| a != key);
                            if switched {
                                e.input = edit::InputState {
                                    duration: e.input.duration,
                                    target: e.input.target,
                                    ..edit::InputState::default()
                                };
                                e.preview = None;
                                e.refused = None;
                                e.clicked = None;
                            }
                            e.active = Some(key);
                            switched
                        };
                        if switched {
                            rerender_input().await;
                        }
                        gesture(g).await;
                    }
                    Msg::Keep => keep_view().await,
                }
            }
        });
        tx
    });
    // Keys arrive by the bindings of the person's settings.
    use_hook(|| spawn(load_keymap()));
    // The view as it was before a reload: zoom, scroll, selection, and
    // note input. Then kept as it changes (a zoom or a scroll is the pane's,
    // so it is looked at now and then; an operation keeps it at once).
    use_hook(|| spawn(restore_view(key)));
    #[cfg(not(target_family = "wasm"))]
    use_hook(|| {
        let tell = tell.clone();
        std::thread::spawn(move || {
            while tell.send(Msg::Keep).is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(700));
            }
        });
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
            input: None,
        });
        let gestures = {
            let tell = tell.clone();
            ChartGestures(std::sync::Arc::new(move |g| {
                let _ = tell.send(Msg::Gesture(g));
            }))
        };
        let mount = chart_mount_with(data.clone(), Some(gestures));
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
            // The chart's kept view, now that its path is known; and note
            // input's cursor drawn on it, which a restore before the path
            // came could not.
            apply_kept_chart(key);
            restore_input(key).await;
            error.set(message);
        }
    });

    // Light paper unless pinned otherwise. Following the app's theme, the
    // chart is drawn from its pane's theme tokens (`theme: None`): the
    // tokens the host gives the view, live, linked in or as a process.
    use_hook(move || {
        if let Some(open) = open_charts().get(&key) {
            open.data.update(|s| s.theme = chart_theme());
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
                if mode() == Mode::Syntax {
                    div { style: "display:flex; gap:8px; align-items:center; padding:6px 10px; border-top:1px solid var(--border, #8884);",
                        span { style: "font-size:12px; opacity:0.75; white-space:nowrap;", "Keyflow syntax" }
                        // What is typed on the chart, with its caret: the
                        // chart keeps the keyboard (a field would take it,
                        // and leaving it would leave the keys nowhere).
                        div {
                            style: "flex:1; min-width:0; font-family:ui-monospace, monospace; font-size:13px; padding:4px 6px; border-radius:4px; border:1px solid var(--primary, #4f46e5); white-space:pre; overflow:hidden;",
                            if draft().is_empty() {
                                span { style: "opacity:0.5;", "C / / / | Am7 / F G" }
                            } else {
                                // Its end, where the typing is.
                                span { "{tail(&draft(), 110)}▏" }
                            }
                        }
                    }
                }
                // The mode, always shown, and what the next key does:
                // MuseScore's status bar.
                div { style: "display:flex; gap:8px; align-items:center; padding:4px 10px; font-size:12px; border-top:1px solid var(--border, #8884); min-height:22px;",
                    span {
                        style: if mode().is_input() {
                            "padding:1px 8px; border-radius:9px; background:var(--primary, #4f46e5); color:var(--primary-foreground, white); white-space:nowrap;"
                        } else {
                            "padding:1px 8px; border-radius:9px; border:1px solid var(--border, #8886); white-space:nowrap; opacity:0.8;"
                        },
                        "{mode().name()}"
                    }
                    // What a letter enters, while entering: Y switches.
                    if mode() == Mode::NoteName || mode() == Mode::Duration {
                        span {
                            style: "padding:1px 8px; border-radius:9px; border:1px solid var(--primary, #4f46e5); color:var(--primary, #4f46e5); white-space:nowrap;",
                            "{target().name()}"
                        }
                    }
                    span { style: "opacity:0.8; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                        "{status}"
                    }
                }
            }
        }
    }
}

/// The view as kept: note input, and each open chart's zoom, scroll and
/// selection by its path.
fn view_state() -> Json {
    let input = keep::input_json(&edit_state().input);
    let pending: Vec<u64> = open_charts()
        .iter()
        .filter(|(k, o)| kept_pending(**k, &o.path))
        .map(|(k, _)| *k)
        .collect();
    // A chart not yet given its kept view back keeps what was kept.
    let mut charts: serde_json::Map<String, Json> = KEPT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|k| k.get("charts")?.as_object().cloned())
        .unwrap_or_default();
    let live: serde_json::Map<String, Json> = open_charts()
        .iter()
        .filter(|(k, o)| !o.path.is_empty() && !pending.contains(k))
        .map(|(_, o)| o)
        .map(|o| {
            let view = o.data.read(|s| s.view);
            let selection = o.selection.read(|s| s.keys.clone());
            (
                o.path.clone(),
                keep::chart_json(view.zoom, view.scroll, &selection),
            )
        })
        .collect();
    charts.extend(live);
    // Note input is in one chart: the one it is given back to.
    let input_chart = active_chart().map(|(_, path)| path);
    json!({ "input": input, "input_chart": input_chart, "charts": charts })
}

/// The view written to the extension's own data, when it changed.
async fn keep_view() {
    // Not before what was kept has been read back: a process starting up
    // would write its defaults over it.
    if !RESTORE_TRIED.load(Ordering::Relaxed) {
        return;
    }
    let state = view_state();
    if !keep::changed(&state) {
        return;
    }
    if let Ok(handle) = handle() {
        let _ = handle
            .host(HostRequest::SetData {
                key: keep::KEY.to_owned(),
                value: Some(state),
            })
            .await;
    }
}

/// Note input is taken back once a process (a reload): not again for a
/// second pane.
static INPUT_RESTORED: AtomicBool = AtomicBool::new(false);

/// Whether this process has read the kept view back (or found none).
static RESTORE_TRIED: AtomicBool = AtomicBool::new(false);

/// The pane `key`'s chart taken back to its kept zoom, scroll and
/// selection, and note input to its kept mode, tools and cursor.
async fn restore_view(key: u64) {
    let Ok(handle) = handle() else { return };
    let kept = handle
        .host(HostRequest::Data {
            key: keep::KEY.to_owned(),
        })
        .await;
    let Ok(kept) = kept else {
        RESTORE_TRIED.store(true, Ordering::Relaxed);
        return;
    };
    // The chart's path is the pane's, read once it mounted.
    restore_from(key, &kept).await;
    RESTORE_TRIED.store(true, Ordering::Relaxed);
}

async fn restore_from(key: u64, kept: &Json) {
    *KEPT.lock().unwrap_or_else(PoisonError::into_inner) = Some(kept.clone());
    apply_kept_chart(key);
    restore_input(key).await;
}

/// Note input given back, once, to pane `key` if it shows the chart it
/// was kept in (not to whichever pane mounts first), and drawn there.
/// Its path may not be known yet; the chart's read calls this again.
async fn restore_input(key: u64) {
    if INPUT_RESTORED.load(Ordering::Relaxed) {
        return;
    }
    let Some(path) = open_charts()
        .get(&key)
        .map(|o| o.path.clone())
        .filter(|p| !p.is_empty())
    else {
        return;
    };
    let kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let Some(input) = kept.as_ref().and_then(|k| k.get("input")) else {
        return;
    };
    let chart = kept
        .as_ref()
        .and_then(|k| k.get("input_chart"))
        .and_then(Json::as_str);
    if chart.is_some_and(|c| c != path) || INPUT_RESTORED.swap(true, Ordering::Relaxed) {
        return;
    }
    {
        let mut e = edit_state();
        e.input = keep::input_from(input);
        e.active = Some(key);
    }
    // Remembered as written, so the first look does not write it again.
    keep::changed(&view_state());
    rerender_input().await;
}

/// What was kept, as read back when this process started.
static KEPT: Mutex<Option<Json>> = Mutex::new(None);

/// The panes (and their charts) given their kept view back.
static APPLIED: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());

/// Whether pane `key` showing `path` is still to be given its kept view.
fn kept_pending(key: u64, path: &str) -> bool {
    let has = KEPT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|k| k.get("charts")?.get(path))
        .is_some();
    has && !APPLIED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|(k, p)| *k == key && p == path)
}

/// Pane `key`'s chart given its kept zoom, scroll and selection, once its
/// path is known (in a process, the pane's state comes after it mounts).
fn apply_kept_chart(key: u64) {
    let Some(o) = open_charts().get(&key).cloned() else {
        return;
    };
    if o.path.is_empty() || !kept_pending(key, &o.path) {
        return;
    }
    let chart = KEPT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|k| k.get("charts")?.get(&o.path).cloned());
    let Some(chart) = chart else { return };
    let (zoom, scroll, selection) = keep::chart_from(&chart);
    o.data.update(|s| s.view = ChartView { zoom, scroll });
    o.selection.set(ChartSelection {
        keys: selection,
        hover: None,
    });
    o.wake.wake();
    APPLIED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((key, o.path.clone()));
}

/// The host this extension was built with.
fn handle() -> Result<Handle, String> {
    HANDLE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| "the extension is not loaded".to_owned())
}

/// The chart note input is on: the pane last used, else the first open.
fn active_chart() -> Option<(u64, String)> {
    let active = edit_state().active;
    let charts = open_charts();
    active
        .and_then(|k| charts.get(&k).map(|o| (k, o.path.clone())))
        .or_else(|| charts.iter().next().map(|(k, o)| (*k, o.path.clone())))
        .filter(|(_, path)| !path.is_empty())
}

/// A chart's text, through the host: the document at `path`, or, when the
/// pane names a session (`session:<id>`), Keyflow's chart there through
/// `fts.session`, as this extension.
async fn read_text(handle: &Handle, path: &str) -> Result<String, String> {
    if let Some(call) = ChartSource::of_pane_state(path)
        .read_call()
        .map_err(|e| e.to_string())?
    {
        let reply = ask_session(handle, &call).await?;
        return ChartSource::chart_of(&reply).map_err(|e| e.to_string());
    }
    let answer = handle
        .host(HostRequest::ReadDocument {
            path: path.to_owned(),
        })
        .await
        .map_err(|e| format!("{e:?}"))?;
    answer
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "the document is not text".to_owned())
}

/// `text` made the chart's text at `path`: for a document, one commit, one
/// undo step named `name` (the host makes the edits that differ, so another
/// person's typing elsewhere in it stays); for a session, the one splice
/// between what the session holds now and `text`, through `fts.session`,
/// so it merges per character the same way.
async fn write_text(handle: &Handle, path: &str, text: String, name: &str) -> Result<(), String> {
    let source = ChartSource::of_pane_state(path);
    if let ChartSource::Session { .. } = source {
        let now = read_text(handle, path).await?;
        let Some(call) = source
            .replace_call(&now, &text)
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };
        return ask_session(handle, &call).await.map(|_| ());
    }
    handle
        .host(HostRequest::EditDocument {
            path: path.to_owned(),
            edits: vec![DocEdit::Replace { text }],
            name: Some(name.to_owned()),
        })
        .await
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

/// One edit note input made: the chart's text before and after it.
struct Step {
    path: String,
    before: String,
    after: String,
    /// Note input as it was before and after it: undo puts the cursor back.
    input: (edit::InputState, edit::InputState),
}

/// One edit at a time: each reads the chart, changes it and writes it
/// back, so a key pressed while the last is still being written waits for
/// it rather than writing over it.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Note input's own edits, to undo and redo (the app's document history
/// has every one as an undo step, but no action a chart view can run
/// reaches it yet).
static UNDO: Mutex<Vec<Step>> = Mutex::new(Vec::new());
static REDO: Mutex<Vec<Step>> = Mutex::new(Vec::new());

/// An edit made: kept to undo, and what was undone is no longer redone.
fn remember(
    path: &str,
    before: String,
    after: String,
    input: (edit::InputState, edit::InputState),
) {
    let mut undo = UNDO.lock().unwrap_or_else(PoisonError::into_inner);
    undo.push(Step {
        path: path.to_owned(),
        before,
        after,
        input,
    });
    if undo.len() > 200 {
        undo.remove(0);
    }
    REDO.lock().unwrap_or_else(PoisonError::into_inner).clear();
}

/// ⌘Z (`undo`) or ⇧⌘Z: note input's last edit taken back (or made again),
/// as a new edit, when the chart is still as that edit left it; another
/// edit since (another person's, the text's) is not undone over.
async fn history(undo: bool) -> Result<Json, String> {
    let _one = ONE_AT_A_TIME.lock().await;
    let (from, to) = if undo { (&UNDO, &REDO) } else { (&REDO, &UNDO) };
    let Some(step) = from.lock().unwrap_or_else(PoisonError::into_inner).pop() else {
        return Err(if undo {
            "Nothing to undo"
        } else {
            "Nothing to redo"
        }
        .to_owned());
    };
    let handle = handle()?;
    let now = read_text(&handle, &step.path).await?;
    let write = match step_text(&step, &now, undo) {
        Ok(text) => text,
        Err(why) => {
            edit_state().refused = Some(why.clone());
            publish_input();
            return Err(why);
        }
    };
    write_text(
        &handle,
        &step.path,
        write.clone(),
        if undo { "Undo" } else { "Redo" },
    )
    .await?;
    reload(Some(&step.path));
    let chart = keyflow::text::chart::parse_chart(&write)?;
    edit_state().input = if undo {
        step.input.0.clone()
    } else {
        step.input.1.clone()
    };
    to.lock().unwrap_or_else(PoisonError::into_inner).push(step);
    let score = Score::from_chart(&chart);
    show_input(&chart, &score, None);
    Ok(input_state())
}

/// The text undoing (`undo`) or redoing `step` writes, when the chart is
/// `now`: only over the text that step left, never over an edit since.
fn step_text(step: &Step, now: &str, undo: bool) -> Result<String, String> {
    let (expect, write) = if undo {
        (&step.after, &step.before)
    } else {
        (&step.before, &step.after)
    };
    if now == expect {
        Ok(write.clone())
    } else {
        Err("The chart changed since that edit; undo the change in its document".to_owned())
    }
}

/// One note-input operation on the active chart: read it, apply it, write
/// the bars it changed back, and show where input is now. `make` builds
/// the operation from the chart's bars (a click's tick needs its bar).
async fn run_op(make: impl FnOnce(&Score) -> Vec<Op>) -> Result<Json, String> {
    let _one = ONE_AT_A_TIME.lock().await;
    let Some((key, path)) = active_chart() else {
        return Err("Open a chart first".to_owned());
    };
    let handle = handle()?;
    let text = read_text(&handle, &path).await?;
    let chart = keyflow::text::chart::parse_chart(&text)?;
    let mut score = Score::from_chart(&chart);
    let ops = make(&score);
    let Some(&op) = ops.last() else {
        return Ok(input_state());
    };
    let (before, outcome) = {
        let mut e = edit_state();
        e.active = Some(key);
        // Into note input from normal mode: where the person last clicked.
        if e.input.mode == Mode::Normal
            && let Op::Mode(m) = op
            && m.is_input()
            && let Some((bar, fraction)) = e.clicked.take()
        {
            let length = score.bars.get(bar).map_or(0, |b| b.length);
            let tick = (fraction * f64::from(length)) as u32;
            e.input.apply(&mut score, Op::MoveTo(Pos { bar, tick }));
        }
        // Into note input with no note before the cursor: the first
        // letter's octave is chosen near the staff's middle line, as its
        // clef puts it (MuseScore's pitch cursor starts there).
        if e.input.mode == Mode::Normal
            && matches!(op, Op::Mode(_))
            && score.pitch_before(e.input.cursor).is_none()
        {
            e.input.pitch = edit::Pitch::from_step(top_line(&chart, e.input.cursor.bar) - 4);
        }
        let before = e.input.clone();
        let mut outcome = edit::Outcome::default();
        for op in &ops {
            let o = e.input.apply(&mut score, *op);
            outcome.changed.extend(o.changed);
            if o.refused.is_some() {
                outcome.refused = o.refused;
            }
        }
        (before, outcome)
    };
    let mut refused = outcome.refused.clone();
    if !outcome.changed.is_empty() {
        match keyflow::edit::commit::write_bars(&text, &chart, &score, &outcome.changed) {
            Ok(new) => {
                write_text(&handle, &path, new.clone(), &op_name(op)).await?;
                let after = edit_state().input.clone();
                remember(&path, text.clone(), new, (before, after));
                reload(Some(&path));
            }
            Err(why) => {
                // Not written: the cursor stays where it was.
                edit_state().input = before;
                refused = Some(why);
            }
        }
    }
    if matches!(op, Op::Mode(_)) {
        load_keymap().await;
    }
    // Leaving syntax mode drops what was previewed: the chart as it is.
    if !edit_state().input.mode.is_input() && edit_state().preview.take().is_some() {
        reload(Some(&path));
    }
    show_input(&chart, &score, refused);
    keep_view().await;
    Ok(input_state())
}

/// Keyflow syntax typed at the cursor, written over the bars from there.
async fn syntax_write(typed: &str) -> Result<Json, String> {
    let _one = ONE_AT_A_TIME.lock().await;
    let Some((_, path)) = active_chart() else {
        return Err("Open a chart first".to_owned());
    };
    let handle = handle()?;
    let text = read_text(&handle, &path).await?;
    let chart = keyflow::text::chart::parse_chart(&text)?;
    let from = edit_state().input.cursor.bar;
    let refused = match keyflow::edit::commit::splice_syntax(&text, &chart, from, typed) {
        Ok((new, bars)) => {
            write_text(&handle, &path, new.clone(), "Type bars").await?;
            edit_state().preview = None;
            reload(Some(&path));
            let before = edit_state().input.clone();
            let mut after = before.clone();
            after.cursor = Pos {
                bar: from + bars,
                tick: 0,
            };
            edit_state().input = after.clone();
            remember(&path, text.clone(), new, (before, after));
            None
        }
        Err(why) => Some(why),
    };
    let score = Score::from_chart(&chart);
    show_input(&chart, &score, refused);
    Ok(input_state())
}

/// Syntax being typed, shown on the chart as it would be written (the
/// chart engraved from the text it would make, nothing written), or why
/// it would not be; nothing typed shows the chart as it is.
async fn syntax_preview(typed: &str) -> Result<Json, String> {
    let _one = ONE_AT_A_TIME.lock().await;
    let Some((_, path)) = active_chart() else {
        return Err("Open a chart first".to_owned());
    };
    let handle = handle()?;
    let text = read_text(&handle, &path).await?;
    let chart = keyflow::text::chart::parse_chart(&text)?;
    let from = edit_state().input.cursor.bar;
    let score = Score::from_chart(&chart);
    if typed.trim().is_empty() {
        edit_state().preview = None;
        reload(Some(&path));
        show_input(&chart, &score, None);
        return Ok(input_state());
    }
    let refused = match keyflow::edit::commit::splice_syntax(&text, &chart, from, typed) {
        Ok((new, bars)) => {
            if let Ok(preview) = keyflow::text::chart::parse_chart(&new) {
                for open in open_charts().values().filter(|o| o.path == path) {
                    open.data.update(|s| {
                        s.chart = preview.clone();
                        s.revision = s.revision.wrapping_add(1);
                    });
                    open.wake.wake();
                }
            }
            edit_state().preview = Some(bars);
            None
        }
        Err(why) => {
            edit_state().preview = None;
            Some(why)
        }
    };
    show_input(&chart, &score, refused);
    Ok(input_state())
}

/// The overlay drawn again from the active chart as it is.
async fn rerender_input() {
    let Some((_, path)) = active_chart() else {
        return;
    };
    let Ok(handle) = handle() else { return };
    let Ok(text) = read_text(&handle, &path).await else {
        return;
    };
    let Ok(chart) = keyflow::text::chart::parse_chart(&text) else {
        return;
    };
    let score = Score::from_chart(&chart);
    let refused = edit_state().refused.clone();
    show_input(&chart, &score, refused);
}

/// The staff step of the active chart's top line: what a click on the
/// staff reads its pitch from.
static TOP_STEP: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(38);

/// The status line, the cursor and the shadow note shown on every pane of
/// the active chart, and the toolbar lit.
fn show_input(chart: &keyflow::Chart, score: &Score, refused: Option<String>) {
    let (overlay, path) = {
        let mut e = edit_state();
        e.status = match e.preview {
            Some(n) if e.input.mode == Mode::Syntax => format!(
                "Preview: {n} bar{} from bar {}, shown on the chart · Enter writes {}, Esc drops {}",
                if n == 1 { "" } else { "s" },
                e.input.cursor.bar + 1,
                if n == 1 { "it" } else { "them" },
                if n == 1 { "it" } else { "them" },
            ),
            _ => e.input.status(score),
        };
        e.refused = refused;
        let path = e
            .active
            .and_then(|k| open_charts().get(&k).map(|o| o.path.clone()));
        TOP_STEP.store(top_line(chart, e.input.cursor.bar), Ordering::Relaxed);
        (overlay(&e.input, e.preview, chart, score), path)
    };
    for open in open_charts().values() {
        let on = path.as_deref() == Some(open.path.as_str());
        open.data
            .update(|s| s.input = if on { overlay } else { None });
        open.wake.wake();
    }
    publish_input();
}

/// Note input's cursor and shadow note, as the pane draws them.
fn overlay(
    input: &edit::InputState,
    preview: Option<usize>,
    chart: &keyflow::Chart,
    score: &Score,
) -> Option<InputOverlay> {
    // The voice being entered's lines (every voice's bars are as long).
    let bars = score.voice_bars(input.voice);
    let last = bars.len().checked_sub(1)?;
    // The entry selected: highlighted as itself, in normal mode too.
    let selected = input.selected.and_then(|pos| {
        let bar = pos.bar.min(last);
        let (start, len) = match input.target {
            edit::Target::Notes => {
                let b = &bars[bar];
                let i = b.index_at(pos.tick)?;
                (b.starts()[i], b.events[i].duration.ticks())
            }
            edit::Target::Chords => {
                let b = &score.chords[bar];
                let i = b.index_at(pos.tick)?;
                (b.starts()[i], b.events[i].duration.ticks())
            }
        };
        let length = f64::from(bars[bar].length.max(1));
        Some((
            bar,
            f64::from(start) / length,
            f64::from(start + len) / length,
            input.target == edit::Target::Chords,
        ))
    });
    if !input.mode.is_input() {
        return selected.map(|selected| InputOverlay {
            bar: selected.0,
            from: 0.0,
            to: 0.0,
            shadow: None,
            bars: 0,
            selected: Some(selected),
            pointer: None,
            voice: input.voice,
        });
    }
    let bar = input.cursor.bar.min(last);
    let length = f64::from(bars[bar].length.max(1));
    // Syntax writes whole bars: the cursor is the bar (or the bars typed).
    let (from, to) = if input.mode == Mode::Syntax {
        (0.0, 1.0)
    } else {
        (
            f64::from(input.cursor.tick) / length,
            (f64::from(input.cursor.tick + input.duration.ticks()) / length).min(1.0),
        )
    };
    let notes = input.target == edit::Target::Notes;
    let shadow = match input.mode {
        _ if input.rest => Some(ShadowNote {
            below_top: 4,
            open: false,
            rest: true,
        }),
        // Input by duration: the pitch cursor, where it would go.
        Mode::Duration if notes => Some(ShadowNote {
            below_top: top_line(chart, bar) - input.pitch.step(),
            open: input.duration.value >= edit::Value::Half,
            rest: false,
        }),
        _ => None,
    };
    // Input by note name, notes: the note a click would enter, under the
    // pointer (MuseScore's mouse preview).
    let pointer = (input.mode == Mode::NoteName && notes).then(|| PointerShadow {
        top_step: top_line(chart, bar),
        open: input.duration.value >= edit::Value::Half,
        rest: input.rest,
    });
    Some(InputOverlay {
        bar,
        from,
        to,
        shadow,
        bars: if input.mode == Mode::Syntax {
            preview.unwrap_or(1)
        } else {
            1
        },
        selected: if input.mode == Mode::Syntax {
            None
        } else {
            selected
        },
        pointer,
        voice: input.voice,
    })
}

/// The staff step of the top line of bar `bar`'s staff, by its clef.
fn top_line(chart: &keyflow::Chart, bar: usize) -> i32 {
    use keyflow::chart::ChartClef;
    let mut n = 0;
    // The engraver's default: a chart that declares no clef is in bass.
    let mut clef = chart.initial_clef.unwrap_or(ChartClef::Bass);
    for section in &chart.sections {
        let count = section.measures().len();
        if bar < n + count {
            clef = section.clef.unwrap_or(clef);
            break;
        }
        n += count;
    }
    let top = match clef {
        ChartClef::Bass => edit::Pitch::new('A', 0, 3),
        ChartClef::Alto => edit::Pitch::new('G', 0, 4),
        ChartClef::Tenor => edit::Pitch::new('E', 0, 4),
        ChartClef::Treble | ChartClef::Percussion => edit::Pitch::new('F', 0, 5),
    };
    top.step()
}

/// An operation's name, as the undo history shows it.
fn op_name(op: Op) -> String {
    match op {
        Op::Letter(l) => format!("Enter {l}"),
        Op::Rest => "Enter rest".to_owned(),
        Op::Tie => "Add tied note".to_owned(),
        Op::Duration(v) => format!("Enter {}", v.name().to_lowercase()),
        Op::Semitone(true) => "Pitch up".to_owned(),
        Op::Semitone(false) => "Pitch down".to_owned(),
        Op::Octave(true) => "Octave up".to_owned(),
        Op::Octave(false) => "Octave down".to_owned(),
        Op::Delete => "Delete".to_owned(),
        Op::Accidental(_) => "Change accidental".to_owned(),
        Op::Dot => "Toggle dot".to_owned(),
        _ => "Edit".to_owned(),
    }
}

/// Note input as `KEYFLOW_INPUT_STATE` reports it.
fn input_state() -> Json {
    let e = edit_state();
    json!({
        "mode": e.input.mode.name(),
        "duration": e.input.duration.name(),
        "accidental": e.input.accidental,
        "rest": e.input.rest,
        "cursor": { "bar": e.input.cursor.bar + 1, "tick": e.input.cursor.tick },
        "pitch": e.input.pitch.name(),
        "status": e.status,
        "refused": e.refused,
    })
}

/// The last `n` characters of `s`, an ellipsis before them when cut.
fn tail(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_owned();
    }
    format!("…{}", s.chars().skip(count - n).collect::<String>())
}

/// A key typed in syntax mode, taken as typing: a character added to the
/// bars being typed (and previewed), Backspace one taken off, Enter
/// writing them, Esc dropping them and leaving. Whether it was.
async fn syntax_key(key: &str, mods: &[&str]) -> bool {
    if edit_state().input.mode != Mode::Syntax
        || mods.iter().any(|m| matches!(*m, "Mod" | "Ctrl" | "Alt"))
    {
        return false;
    }
    let typed = {
        let mut e = edit_state();
        match key {
            "Enter" => None,
            "Escape" => {
                e.draft.clear();
                return false; // ⎋ is note input's Escape: it leaves.
            }
            "Backspace" => {
                e.draft.pop();
                Some(e.draft.clone())
            }
            k if k.chars().count() == 1 => {
                e.draft.push_str(k);
                Some(e.draft.clone())
            }
            _ => return false,
        }
    };
    match typed {
        Some(text) => {
            let _ = dispatch("KEYFLOW_SYNTAX_PREVIEW", json!({ "text": text })).await;
        }
        None => {
            let text = edit_state().draft.clone();
            if dispatch("KEYFLOW_SYNTAX_WRITE", json!({ "text": text }))
                .await
                .is_ok()
                && edit_state().refused.is_none()
            {
                edit_state().draft.clear();
                rerender_input().await;
            }
        }
    }
    true
}

/// A key pressed or a bar clicked on a chart: a key run as the action the
/// keymap binds it to, through the host (as ⌘K and MCP run it); a click
/// noted as where note input starts, or the cursor moved there.
async fn gesture(g: ChartGesture) {
    match g {
        ChartGesture::Key { key, mods } => {
            if syntax_key(&key, &mods).await {
                return;
            }
            let chord = input::chord(&key, &mods);
            let (action, mode, selected) = {
                let e = edit_state();
                (
                    e.keymap.get(&chord).cloned(),
                    e.input.mode,
                    e.input.selected.is_some(),
                )
            };
            let Some(action) = action else { return };
            let always = input::HISTORY_KEYS.iter().any(|(a, _)| *a == action)
                || input::mode_of(&action).is_some()
                || action == input::TARGET_KEY.0;
            // Normal mode: a key acts on what is selected on the chart (a
            // chord or a note clicked), as MuseScore's; with nothing
            // selected, only the keys into note input and undo.
            if mode == Mode::Normal && !always && !selected {
                return;
            }
            if mode == Mode::Syntax && action != "KEYFLOW_ESCAPE" && !always {
                return;
            }
            let _ = dispatch(&action, json!({})).await;
        }
        ChartGesture::Click {
            bar,
            fraction,
            kind,
            below_top,
        } => {
            let (mode, target) = {
                let mut e = edit_state();
                e.clicked = Some((bar, fraction));
                (e.input.mode, e.input.target)
            };
            let on = kind.as_deref();
            let target_of = match on {
                Some("chord") => Some("chords"),
                Some("note" | "rest") => Some("notes"),
                _ => None,
            };
            let args = match mode {
                // Normal mode: a chord or a note clicked is what keys act on.
                Mode::Normal => json!({
                    "bar": bar, "fraction": fraction,
                    "target": target_of, "select": target_of.is_some(),
                }),
                // Entering notes: a click on the staff enters the note there.
                Mode::NoteName if target == edit::Target::Notes => {
                    let _ = dispatch(
                        "KEYFLOW_INPUT_ENTER_AT",
                        json!({ "bar": bar, "fraction": fraction, "below_top": below_top }),
                    )
                    .await;
                    return;
                }
                _ => json!({ "bar": bar, "fraction": fraction }),
            };
            let _ = dispatch("KEYFLOW_INPUT_MOVE", args).await;
        }
    }
}

/// `action` run through the host, so a key runs what ⌘K and MCP run.
async fn dispatch(action: &str, args: Json) -> Result<Json, String> {
    handle()?
        .host(HostRequest::Run {
            action: action.to_owned(),
            args,
            ticket: None,
        })
        .await
        .map_err(|e| format!("{e:?}"))
}

/// The keymap read again from the person's settings: each action's
/// `keyflow.keys.*`, MuseScore's key when unset.
async fn load_keymap() {
    let Ok(handle) = handle() else { return };
    let mut map = BTreeMap::new();
    for (action, default) in input::default_keys() {
        let set = handle
            .host(HostRequest::Setting {
                key: input::setting_key(action),
            })
            .await
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|k| !k.trim().is_empty());
        map.insert(set.unwrap_or_else(|| default.to_owned()), action.to_owned());
    }
    edit_state().keymap = map;
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
    let text = read_text(&handle, path).await?;
    keyflow::parse(&text).map_err(|e| e.to_string())
}

/// A `fts.session` call, as this extension: answered here when this copy
/// hosts the session (an extension cannot `[uses]` the service it
/// provides), else through the host. The reply as JSON text.
async fn ask_session(
    handle: &Handle,
    call: &keyflow_session::ServiceCall,
) -> Result<String, String> {
    #[cfg(not(target_family = "wasm"))]
    if let Some(reply) = session_host::local_answer(call) {
        return reply;
    }
    handle
        .host(service_request(call)?)
        .await
        .map(|reply| reply.to_string())
        .map_err(|e| format!("{e:?}"))
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
                "hover": o.selection.read(|s| s.hover.clone()),
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
        let edit = edit_state().input.clone();
        tools::TOOLS
            .iter()
            .filter(|t| state.is_on(&edit, t.command))
            .map(|t| t.action)
            .collect()
    };
    json!({
        "charts": charts,
        "dark": DARK.load(Ordering::Relaxed),
        "follows_app_theme": !PINNED.load(Ordering::Relaxed),
        "tools_on": on,
    })
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
        PINNED.store(false, Ordering::Relaxed);
        for open in open_charts().values() {
            open.data.update(|s| s.theme = None);
            open.wake.wake();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn step() -> Step {
        Step {
            path: "Charts/A.kf".into(),
            before: "| C / / / |".into(),
            after: "| << C / / / ; m{ E:44 r2. } >> |".into(),
            input: Default::default(),
        }
    }

    #[test]
    fn undo_takes_an_edit_back_only_over_what_it_left() {
        let s = step();
        assert_eq!(
            step_text(&s, &s.after, true).as_deref(),
            Ok(s.before.as_str())
        );
        assert_eq!(
            step_text(&s, &s.before, false).as_deref(),
            Ok(s.after.as_str())
        );
        // Someone else's edit since: not undone over.
        assert!(step_text(&s, "| D / / / |", true).is_err());
    }
}
