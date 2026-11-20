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
pub mod selection;
#[cfg(not(target_family = "wasm"))]
pub mod session_host;
#[cfg(not(target_family = "wasm"))]
pub mod session_view;
pub mod tools;
#[cfg(not(target_family = "wasm"))]
pub mod typed_session;
#[cfg(target_family = "wasm")]
mod web_pane;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use architect_extensions::contrib::{StateKind, StateWrite};
use architect_extensions::hostcall::{DocEdit, HostRequest};
#[cfg(not(target_family = "wasm"))]
use architect_extensions::linked::PaintPane;
use architect_extensions::linked::{
    Builder, Extension, Handle, ViewState, use_pane_state, use_view_state,
};
use architect_workspace::paint::peniko::Color;
use architect_workspace::{HostData, PaintWake};
use dioxus::prelude::*;
use keyflow::edit::{self, Mode, Op, Pos, Score};
use keyflow::engraver::renderer::ChartTheme;
use keyflow_session::ChartSource;
use keyflow_ui::paint_view::{
    ChartGesture, ChartGestures, ChartPaintStatus, ChartSelection, ChartSnapshot, ChartView,
    InputOverlay, PointerShadow, ShadowNote, chart_mount_with,
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
        // A chart on a session: its playhead, followed while it plays.
        #[cfg(not(target_family = "wasm"))]
        session_view::start_playhead();
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
            let name = action.action;
            ext.action(name, move |_| async move {
                // Normal mode: a key acts on what is selected on the chart
                // (a chord or a note clicked), as MuseScore's; with nothing
                // selected it does nothing. Syntax mode: only Esc.
                let (mode, selected) = {
                    let e = edit_state();
                    (e.input.mode, e.input.selected.is_some())
                };
                // (The re-pitch method is chosen in any mode, for the next
                // note input, as MuseScore's input methods are.)
                let always = matches!(name, "KEYFLOW_ESCAPE" | "KEYFLOW_REPITCH");
                let idle = (mode == Mode::Normal && !selected && !always)
                    || (mode == Mode::Syntax && !always);
                if idle {
                    return Ok(input_state());
                }
                run_op(move |_| vec![op]).await
            });
        }
        // `bar`: the chart's bars from 0 as the engraver numbers them
        // (`bar#N`, what a click hands over; a count-in is not one of
        // them, `edit::bars`), `fraction` of the way across it.
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
            let voice = args
                .get("voice")
                .and_then(Json::as_u64)
                .and_then(|v| u8::try_from(v).ok())
                .filter(|v| *v < 4);
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
                let mut ops: Vec<Op> = target.map(Op::Target).into_iter().collect();
                ops.extend(voice.map(Op::Voice));
                ops.push(at);
                ops
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
        ext.action("KEYFLOW_CHORD_SET", |args| async move {
            let typed = text(&args, "text").ok_or("text: the chord symbol")?;
            edit_state().chord_draft = None;
            run_edit(|_| Vec::new(), Some(Then::Retype(typed))).await
        });
        // A palette cell on what is selected (a chord's quality, an
        // accent or a staccato on it), as MuseScore applies a palette.
        ext.action("KEYFLOW_PALETTE_APPLY", |args| async move {
            let token = text(&args, "token").ok_or("token: the palette cell's token")?;
            // A push or a pull is an action on the selection (any number
            // of chords, bars, a range), not a mark on one chord.
            let shift = match token.as_str() {
                "'" => Some(("KEYFLOW_SELECTION_PUSH", "eighth")),
                "''" => Some(("KEYFLOW_SELECTION_PUSH", "sixteenth")),
                "pull" => Some(("KEYFLOW_SELECTION_PULL", "eighth")),
                _ => None,
            };
            if let Some((action, step)) = shift {
                if let Err(why) = dispatch(action, json!({ "step": step })).await {
                    edit_state().refused = Some(why);
                    rerender_input().await;
                }
                return Ok(input_state());
            }
            let then =
                palette::cell(&token).ok_or_else(|| format!("{token}: not a palette cell"))?;
            match then {
                Ok(then) => run_edit(|_| Vec::new(), Some(then)).await,
                Err(why) => {
                    edit_state().refused = Some(why);
                    rerender_input().await;
                    Ok(input_state())
                }
            }
        });
        ext.action("KEYFLOW_CHORD_EDIT", |_| async { chord_edit().await });
        // Edit mode: the palettes beside the chart and the note-input
        // toolbar over it, together. Off, the pane is the chart alone
        // (reading it, following playback).
        ext.action("KEYFLOW_EDIT_MODE", |args| async move {
            let shown = dispatch("KEYFLOW_STATE", Json::Null)
                .await
                .ok()
                .and_then(|s| s.get("palettes").and_then(Json::as_bool))
                .unwrap_or(true);
            let on = args.get("on").and_then(Json::as_bool).unwrap_or(!shown);
            if shown != on {
                dispatch("KEYFLOW_PALETTES", Json::Null).await?;
            }
            let place = if on { "view:keyflow/chart" } else { "hidden" };
            dispatch(
                "TOOLBAR_SHOW",
                serde_json::json!({ "id": "keyflow/note-input", "where": place }),
            )
            .await?;
            Ok(serde_json::json!({ "edit": on }))
        });
        // The chart's zoom from the keyboard, MuseScore's keys: a step of
        // the wheel's, 100%, or the pane's width.
        for (name, step) in [
            ("KEYFLOW_ZOOM_IN", ZoomTo::Step(1.0)),
            ("KEYFLOW_ZOOM_OUT", ZoomTo::Step(-1.0)),
            ("KEYFLOW_ZOOM_100", ZoomTo::Exactly(1.0)),
            ("KEYFLOW_ZOOM_FIT", ZoomTo::Fit),
        ] {
            ext.action(name, move |_| async move { Ok(zoom_chart(step)) });
        }
        // MuseScore's "Input by duration mode cursor" preference: the
        // above-staff ruler or the classic cursor.
        ext.action("KEYFLOW_DURATION_RULER", |_| async {
            let on = !DURATION_RULER.load(Ordering::Relaxed);
            DURATION_RULER.store(on, Ordering::Relaxed);
            rerender_input().await;
            Ok(json!({ "ruler": on }))
        });
        // How playback is shown: the stepped cursor on the symbol
        // sounding (the default), or the old line gliding across the bar.
        ext.action("KEYFLOW_PLAYBACK_CURSOR", |args| async move {
            use keyflow::engraver::renderer::PlayheadStyle;
            let style = match args.get("style").and_then(Json::as_str) {
                Some("smooth") => PlayheadStyle::Smooth,
                Some("stepped") => PlayheadStyle::Stepped,
                Some(other) => return Err(format!("style: stepped or smooth, not {other}")),
                None => match keyflow_ui::paint_view::playhead_style() {
                    PlayheadStyle::Stepped => PlayheadStyle::Smooth,
                    PlayheadStyle::Smooth => PlayheadStyle::Stepped,
                },
            };
            keyflow_ui::paint_view::set_playhead_style(style);
            for open in open_charts().values() {
                open.wake.wake();
            }
            Ok(json!({ "style": if style == PlayheadStyle::Smooth { "smooth" } else { "stepped" } }))
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
        selection::build(ext);
        #[cfg(not(target_family = "wasm"))]
        typed_session::build(ext);
        #[cfg(target_family = "wasm")]
        browser_sessions(ext);
        ext.action("KEYFLOW_INPUT_STATE", |_| async { Ok(input_state()) });
        ext.action("KEYFLOW_UNDO", |_| async { history(true).await });
        ext.action("KEYFLOW_REDO", |_| async { history(false).await });
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
    /// What its view reports: the zoom in effect and the pane's size.
    status: HostData<ChartPaintStatus>,
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
    /// Time to show the zoom in effect, if it changed.
    Zoom,
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
    /// How the previewed bars' chords were read (`bar 21: Em7 1 beat, A7
    /// 3 beats`): Keyflow's `/` is a beat and the chord none, so what was
    /// typed is shown before Enter writes it.
    preview_read: Option<String>,
    /// A chord symbol being retyped (↩ on a selected chord): the symbol
    /// it starts from, in the field under the chart.
    chord_draft: Option<String>,
}

static EDIT: std::sync::LazyLock<Mutex<EditState>> =
    std::sync::LazyLock::new(|| Mutex::new(EditState::default()));

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
    let pane_id = use_pane_state().map(|(id, _)| id).unwrap_or_default();
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
    // The field under the chart: what is typed in it (syntax, or a chord
    // symbol retyped), and a key that gives a fresh, empty one after each
    // write (it is uncontrolled: a value written back on every key would
    // move the caret as it is typed).
    let mut draft = use_signal(String::new);
    let mut chord = use_signal(|| None::<String>);
    let mut field = use_signal(|| 0_u64);
    // The zoom in effect, as the status bar shows it.
    let mut zoom_label = use_signal(String::new);
    // The pane's place as its pane keeps it (architect#859): the latest,
    // for the keeper below to write through, and what was kept, given back
    // once (the pane's input reaches a process just after its first render,
    // so it may come a render late).
    let place = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<ViewState>)));
    if let Some(kept) = use_view_state() {
        let mut places = PLACES.lock().unwrap_or_else(PoisonError::into_inner);
        let slot = places.entry(key).or_insert((String::new(), false));
        if !slot.1 {
            slot.0.clone_from(&kept.value);
        }
        *place.borrow_mut() = Some(kept);
    }
    let tell = use_hook(|| {
        let place = place.clone();
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
                        // A chord to retype: a fresh field holding its symbol.
                        if e.chord_draft != *chord.peek() {
                            if let Some(symbol) = &e.chord_draft {
                                draft.set(symbol.clone());
                                field += 1;
                            }
                            chord.set(e.chord_draft.clone());
                        }
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
                                // Only the duration chosen comes along.
                                e.input = edit::InputState {
                                    duration: edit::Duration {
                                        triplet: false,
                                        ..e.input.duration
                                    },
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
                    // The pane's place kept, once what was kept is given back
                    // (a starting view would write its defaults over it).
                    Msg::Zoom => {
                        if let Some(o) = open_charts().get(&key).cloned() {
                            let text = zoom_text(&o);
                            if *zoom_label.peek() != text {
                                zoom_label.set(text);
                            }
                        }
                    }
                    Msg::Keep => {
                        if give_back_place(key).await
                            && let Some(now) = pane_place(key)
                        {
                            if let Some(kept) = place.borrow().clone() {
                                kept.set(now.to_string());
                            }
                            // And the chart's own, for whichever pane opens
                            // it next (this one closed, the app restarted).
                            keep_document_place(key, &now).await;
                        }
                    }
                }
            }
        });
        tx
    });
    // The view's place, kept by its pane (`use_view_state`): given back
    // once its chart is read, then kept as it changes (a zoom or a scroll
    // is the pane's, so it is looked at now and then).
    #[cfg(not(target_family = "wasm"))]
    use_hook(|| {
        let tell = tell.clone();
        std::thread::spawn(move || {
            while tell.send(Msg::Keep).is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(700));
            }
        });
    });
    // In a browser, on the page's timer.
    #[cfg(target_family = "wasm")]
    use_hook(|| {
        let tell = tell.clone();
        spawn(async move {
            while tell.send(Msg::Keep).is_ok() {
                gloo_timers::future::sleep(std::time::Duration::from_millis(700)).await;
            }
        });
    });
    // The zoom readout kept up with the wheel and the keys: looked at
    // often, drawn again only when it changes.
    #[cfg(not(target_family = "wasm"))]
    use_hook(|| {
        let tell = tell.clone();
        std::thread::spawn(move || {
            while tell.send(Msg::Zoom).is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
        });
    });
    #[cfg(target_family = "wasm")]
    use_hook(|| {
        let tell = tell.clone();
        spawn(async move {
            while tell.send(Msg::Zoom).is_ok() {
                gloo_timers::future::sleep(std::time::Duration::from_millis(120)).await;
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
                status: mount.status.clone(),
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
        let was = current.peek().clone();
        current.set(path.clone());
        if let Some(open) = open_charts().get_mut(&key) {
            open.path.clone_from(&path);
            // Another chart in this pane: it starts at its own view (its
            // kept one, if any, applied once it is read), not at the last
            // chart's zoom, scroll and selection.
            if !was.is_empty() {
                open.data.update(|s| s.view = ChartView::default());
                open.selection.update(|s| {
                    s.keys.clear();
                    s.hover = None;
                });
                open.wake.wake();
            }
        }
    }

    // Read and engrave the document whenever the path or `reads` moves.
    let mut titled = use_signal(String::new);
    let pane_for_title = pane_id.clone();
    let _ = use_resource(move || {
        let path = current();
        let _ = reads();
        let pane = pane_for_title.clone();
        async move {
            let outcome = read_chart(&path).await;
            let message = outcome.as_ref().err().cloned();
            // Its tab named for the chart, not "Chart": its title, else its
            // file's name.
            if let Ok(chart) = &outcome {
                let title = tab_title(chart, &path);
                if !pane.is_empty() && *titled.peek() != title {
                    titled.set(title.clone());
                    let _ = dispatch("WORKSPACE_TAB_SET", json!({ "pane": pane, "title": title }))
                        .await;
                }
            }
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
            give_back_place(key).await;
            give_back_document_place(key, &path).await;
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

    let shown_status = without_mode(&status(), mode());
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
                    ChartCanvas { mount: mount.mount.clone(), pane: pane_id.clone(), path: path.clone() }
                }
                if mode() == Mode::Syntax || chord().is_some() {
                    div { style: "display:flex; gap:8px; align-items:center; padding:6px 10px; border-top:1px solid var(--border, #8884);",
                        span { style: "font-size:12px; opacity:0.75; white-space:nowrap;",
                            if chord().is_some() { "Chord symbol" } else { "Keyflow syntax" }
                        }
                        // A text field: the keys typed in it are its own (the
                        // chart's keys are the keymap's, `[[keys]] when =
                        // "view:chart"`, which a focused field keeps).
                        for generation in std::iter::once(field()) {
                            input {
                                key: "{generation}",
                                style: "flex:1; min-width:0; font-family:ui-monospace, monospace; font-size:13px; padding:4px 6px; border-radius:4px; border:1px solid var(--border, #8886); background:var(--background, transparent); color:var(--foreground, inherit);",
                                placeholder: if chord().is_some() { "Gsus4" } else { "C // Am7 // | F // G //   (a / is a beat)" },
                                value: chord().unwrap_or_default(),
                                autofocus: true,
                                oninput: move |e| {
                                    let typed = e.value();
                                    draft.set(typed.clone());
                                    if chord.peek().is_none() {
                                        spawn(async move {
                                            let _ = dispatch("KEYFLOW_SYNTAX_PREVIEW", json!({ "text": typed })).await;
                                        });
                                    }
                                },
                                onkeydown: move |e| {
                                    match e.key().to_string().as_str() {
                                        "Enter" => {
                                            let typed = draft();
                                            let retyping = chord.peek().is_some();
                                            spawn(async move {
                                                if retyping {
                                                    edit_state().chord_draft = None;
                                                    if let Err(why) = dispatch("KEYFLOW_CHORD_SET", json!({ "text": typed })).await {
                                                        edit_state().refused = Some(why);
                                                        rerender_input().await;
                                                    }
                                                } else if dispatch("KEYFLOW_SYNTAX_WRITE", json!({ "text": typed })).await.is_ok()
                                                    && edit_state().refused.is_none()
                                                {
                                                    draft.set(String::new());
                                                    field += 1;
                                                }
                                            });
                                        }
                                        "Escape" => {
                                            let retyping = chord.peek().is_some();
                                            spawn(async move {
                                                if retyping {
                                                    edit_state().chord_draft = None;
                                                    rerender_input().await;
                                                } else {
                                                    let _ = dispatch("KEYFLOW_ESCAPE", json!({})).await;
                                                }
                                            });
                                        }
                                        _ => {}
                                    }
                                },
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
                    span { style: "opacity:0.8; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; flex:1; min-width:0;",
                        // The mode is the chip's: the line goes on from it.
                        "{shown_status}"
                    }
                    // The zoom in effect; a click fits the page to the pane
                    // (⌘= ⌘- ⌘0 and ⌥⌘0 from the keyboard).
                    if !zoom_label().is_empty() {
                        span {
                            style: "padding:1px 8px; border-radius:9px; border:1px solid var(--border, #8886); white-space:nowrap; opacity:0.85; cursor:pointer;",
                            title: "Zoom: ⌘= in, ⌘- out, ⌘0 100%, ⌥⌘0 fit the page to the width (click)",
                            onclick: move |_| {
                                spawn(async move {
                                    let _ = dispatch("KEYFLOW_ZOOM_FIT", json!({})).await;
                                });
                            },
                            "{zoom_label}"
                        }
                    }
                }
            }
        }
    }
}

/// Pane `key`'s place, as its pane keeps it (`use_view_state`,
/// architect#859): its chart's zoom, scroll and selection, by the chart it
/// shows, and note input when it is on this pane. Given back when the view
/// starts again: its program restarted (a development copy rebuilt), its
/// pane moved or popped out, the app opened again.
fn pane_place(key: u64) -> Option<Json> {
    let o = open_charts().get(&key).cloned()?;
    if o.path.is_empty() {
        return None;
    }
    let view = o.data.read(|s| s.view);
    let selection = o.selection.read(|s| s.keys.clone());
    let mut place = json!({
        "path": o.path,
        "chart": keep::chart_json(view.zoom, view.scroll, &selection),
    });
    let e = edit_state();
    if e.active == Some(key) {
        place["input"] = keep::input_json(&e.input);
    }
    Some(place)
}

/// The data key a chart's view is kept under in this extension's data: its
/// path's, so a pane that opens the chart again finds it.
fn document_place_key(path: &str) -> String {
    format!("view:{path}")
}

/// Each chart's view as last written to this extension's data (by path),
/// and the panes that were given theirs back (by mount key and path): a
/// view is written only once its chart's kept one has been applied, or a
/// fresh pane would write its defaults over it.
static DOCUMENT_PLACES: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
static DOCUMENT_GIVEN: Mutex<std::collections::BTreeSet<(u64, String)>> =
    Mutex::new(std::collections::BTreeSet::new());

/// Pane `key`'s chart's view (`place`'s zoom, scroll and selection) kept
/// with the chart, when it changed.
async fn keep_document_place(key: u64, place: &Json) {
    let (Some(path), Some(chart)) = (place.get("path").and_then(Json::as_str), place.get("chart"))
    else {
        return;
    };
    let given = DOCUMENT_GIVEN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&(key, path.to_owned()));
    if !given {
        return;
    }
    let text = chart.to_string();
    {
        let mut places = DOCUMENT_PLACES
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if places.get(path) == Some(&text) {
            return;
        }
        places.insert(path.to_owned(), text);
    }
    if let Ok(handle) = handle() {
        let _ = handle
            .host(HostRequest::SetData {
                key: document_place_key(path),
                value: Some(chart.clone()),
            })
            .await;
    }
}

/// Pane `key`, showing `path`, given the chart's kept view once, unless its
/// own kept place (its pane's) was for this chart and has been given back.
async fn give_back_document_place(key: u64, path: &str) {
    if path.is_empty() {
        return;
    }
    let first = DOCUMENT_GIVEN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert((key, path.to_owned()));
    if !first {
        return;
    }
    let pane_had_it = PLACES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .is_some_and(|(text, _)| {
            serde_json::from_str::<Json>(text)
                .ok()
                .and_then(|p| p.get("path").and_then(Json::as_str).map(|p| p == path))
                .unwrap_or(false)
        });
    if pane_had_it {
        return;
    }
    let Ok(handle) = handle() else { return };
    let Ok(kept) = handle
        .host(HostRequest::Data {
            key: document_place_key(path),
        })
        .await
    else {
        return;
    };
    if kept.is_null() {
        return;
    }
    let Some(o) = open_charts().get(&key).cloned() else {
        return;
    };
    if o.path != path {
        return;
    }
    let (zoom, scroll, selection) = keep::chart_from(&kept);
    o.data.update(|s| s.view = ChartView { zoom, scroll });
    o.selection.set(ChartSelection {
        keys: selection,
        ..ChartSelection::default()
    });
    o.wake.wake();
    DOCUMENT_PLACES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(path.to_owned(), kept.to_string());
}

/// What each pane's view was given back (its kept place, as text), and
/// whether it has been applied: a pane is given its place once, when its
/// chart's path is known and is the chart it was kept for.
static PLACES: Mutex<BTreeMap<u64, (String, bool)>> = Mutex::new(BTreeMap::new());

/// Pane `key`'s kept place applied, if it has one not yet applied and its
/// chart is the one it was kept for: zoom, scroll and selection, and note
/// input if it was on this pane. Whether there is nothing left to apply
/// (so the pane's place may now be written).
async fn give_back_place(key: u64) -> bool {
    let kept = {
        let places = PLACES.lock().unwrap_or_else(PoisonError::into_inner);
        match places.get(&key) {
            Some((text, false)) if !text.is_empty() => text.clone(),
            _ => return true,
        }
    };
    let Some(o) = open_charts().get(&key).cloned() else {
        return false;
    };
    if o.path.is_empty() {
        return false;
    }
    let place: Json = serde_json::from_str(&kept).unwrap_or(Json::Null);
    if let Some(slot) = PLACES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_mut(&key)
    {
        slot.1 = true;
    }
    // Kept for another chart (the pane shows another now): not applied.
    if place.get("path").and_then(Json::as_str) != Some(o.path.as_str()) {
        return true;
    }
    if let Some(chart) = place.get("chart") {
        let (zoom, scroll, selection) = keep::chart_from(chart);
        o.data.update(|s| s.view = ChartView { zoom, scroll });
        o.selection.set(ChartSelection {
            keys: selection,
            ..ChartSelection::default()
        });
        o.wake.wake();
    }
    if let Some(input) = place.get("input") {
        {
            let mut e = edit_state();
            e.input = keep::input_from(input);
            e.active = Some(key);
        }
        rerender_input().await;
    }
    true
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
        ask_session(handle, &call).await?;
        // A session whose chart has been typed into its timeline: the
        // items kept in step with the edit (what it changed, written).
        #[cfg(not(target_family = "wasm"))]
        if let Err(why) = typed_session::retype(handle, path, &text, false).await {
            // The edit is written; only the typed items are behind.
            edit_state().refused = Some(format!("The chart's typed items were not updated: {why}"));
        }
        return Ok(());
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

/// A session chart's edits, to undo and redo: a session (`session:<id>`)
/// is edited through `fts.session`, whose history no host action reaches
/// yet. A chart that is a document has its document's own (`DOC_UNDO`).
static UNDO: Mutex<Vec<Step>> = Mutex::new(Vec::new());
static REDO: Mutex<Vec<Step>> = Mutex::new(Vec::new());

/// An edit made: kept to undo, and what was undone is no longer redone,
/// for a session chart (a document's history is its own).
fn remember(
    path: &str,
    before: String,
    after: String,
    input: (edit::InputState, edit::InputState),
) {
    if !matches!(
        ChartSource::of_pane_state(path),
        ChartSource::Session { .. }
    ) {
        return;
    }
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

/// An edit made by an action on the selection: kept to undo as note
/// input's are (the cursor where it was).
pub(crate) fn remember_edit(path: &str, before: String, after: String) {
    let input = edit_state().input.clone();
    remember(path, before, after, (input.clone(), input));
}

/// ⌘Z (`undo`) or ⇧⌘Z: the chart's last edit taken back (or made again):
/// a document's through its own history; a session's, note input's last
/// edit, as a new edit. Another person's edits since, elsewhere in the chart,
/// stay; one over this edit's own text is not undone over.
async fn history(undo: bool) -> Result<Json, String> {
    // A chart that is a document: its document's own history (the host's
    // undo_document / redo_document, architect#858, as DOC_UNDO runs it),
    // every edit to it one step, other people's included, as the editor's
    // undo walks.
    if let Some((_, path)) = active_chart()
        && !matches!(
            ChartSource::of_pane_state(&path),
            ChartSource::Session { .. }
        )
    {
        let handle = handle()?;
        let request = if undo {
            HostRequest::UndoDocument { path: path.clone() }
        } else {
            HostRequest::RedoDocument { path: path.clone() }
        };
        let step = handle.host(request).await.map_err(|e| format!("{e:?}"))?;
        if step.is_null() {
            let why = if undo {
                "Nothing to undo"
            } else {
                "Nothing to redo"
            };
            edit_state().refused = Some(why.to_owned());
        }
        reload(Some(&path));
        rerender_input().await;
        return Ok(input_state());
    }
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
    let score = Score::from_text(&write, &chart);
    show_input(&chart, &score, None);
    Ok(input_state())
}

/// The text undoing (`undo`) or redoing `step` writes, when the chart is
/// `now`: that step's own change taken back (or made again) and nothing
/// else. Edits made since elsewhere in the chart (the other person's, in
/// another bar) stay: the step's splice moves past them. One that touched
/// the step's own text is not undone over.
fn step_text(step: &Step, now: &str, undo: bool) -> Result<String, String> {
    let (from, to) = if undo {
        (&step.after, &step.before)
    } else {
        (&step.before, &step.after)
    };
    if now == from {
        return Ok(to.clone());
    }
    rebase(from, to, now).ok_or_else(|| {
        "That bar changed since the edit; undo the change in its document".to_owned()
    })
}

/// `from` → `to` (one splice) applied to `now`, which is `from` with
/// someone else's change: `None` when that change touches the splice's
/// text. Positions are Unicode scalars, as `splice_between`'s.
fn rebase(from: &str, to: &str, now: &str) -> Option<String> {
    let (at, delete, insert) = keyflow_session::splice_between(from, to);
    let (theirs_at, theirs_delete, theirs_insert) = keyflow_session::splice_between(from, now);
    let (at, delete) = (at as usize, delete as usize);
    let (theirs_at, theirs_delete) = (theirs_at as usize, theirs_delete as usize);
    let theirs_end = theirs_at + theirs_delete;
    // Theirs wholly before ours: ours moves by what theirs added. Wholly
    // after: ours stays. Adjacent or overlapping: refused, since theirs
    // may continue ours.
    let moved = if theirs_end < at {
        (at + theirs_insert.chars().count()).checked_sub(theirs_delete)?
    } else if theirs_at > at + delete {
        at
    } else {
        return None;
    };
    let chars: Vec<char> = now.chars().collect();
    let ours: Vec<char> = from.chars().skip(at).take(delete).collect();
    if chars.get(moved..moved + delete)? != ours.as_slice() {
        return None;
    }
    let head: String = chars.get(..moved)?.iter().collect();
    let tail: String = chars.get(moved + delete..)?.iter().collect();
    Some(format!("{head}{insert}{tail}"))
}

/// One note-input operation on the active chart: read it, apply it, write
/// the bars it changed back, and show where input is now. `make` builds
/// the operation from the chart's bars (a click's tick needs its bar).
async fn run_op(make: impl FnOnce(&Score) -> Vec<Op>) -> Result<Json, String> {
    run_edit(make, None).await
}

/// What is done to the selected chord after an edit's operations.
pub(crate) enum Then {
    /// Made the symbol typed (`InputState::set_chord`).
    Retype(String),
    /// Its quality made this one (a palette's quality cell).
    Quality(String),
    /// This mark put on it, or off (a palette's articulation cell).
    Mark(String),
}

impl Then {
    fn apply(&self, input: &mut edit::InputState, score: &mut Score) -> edit::Outcome {
        match self {
            Self::Retype(t) => input.set_chord(score, t),
            Self::Quality(q) => input.set_quality(score, q),
            Self::Mark(m) => input.toggle_mark(score, m),
        }
    }

    /// Its undo step's name.
    fn name(&self) -> String {
        match self {
            Self::Retype(t) => format!("Chord {t}"),
            Self::Quality(q) => format!("Chord quality {}", if q.is_empty() { "major" } else { q }),
            Self::Mark(m) => format!("Chord mark {m}"),
        }
    }
}

/// `make`'s operations applied, then `then` done to the selected chord:
/// written back as one edit, one undo step.
async fn run_edit(
    make: impl FnOnce(&Score) -> Vec<Op>,
    retype: Option<Then>,
) -> Result<Json, String> {
    let _one = ONE_AT_A_TIME.lock().await;
    let Some((key, path)) = active_chart() else {
        return Err("Open a chart first".to_owned());
    };
    let handle = handle()?;
    let text = read_text(&handle, &path).await?;
    let chart = keyflow::text::chart::parse_chart(&text)?;
    let mut score = Score::from_text(&text, &chart);
    let ops = make(&score);
    let Some(&op) = ops.last().or(retype.as_ref().map(|_| &Op::Escape)) else {
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
            if o.note.is_some() {
                outcome.note = o.note;
            }
            if o.refused.is_some() {
                outcome.refused = o.refused;
            }
        }
        if let Some(then) = &retype {
            let o = then.apply(&mut e.input, &mut score);
            outcome.changed.extend(o.changed);
            outcome.refused = o.refused.or(outcome.refused.take());
        }
        (before, outcome)
    };
    // A refusal, or else what the person should know it did (an entry on
    // a replayed bar written where the bar it plays is): the status line.
    let mut refused = outcome.refused.clone().or_else(|| outcome.note.clone());
    if !outcome.changed.is_empty() {
        match keyflow::edit::commit::write_bars(&text, &chart, &score, &outcome.changed) {
            Ok(new) => {
                let name = retype.as_ref().map_or_else(|| op_name(op), Then::name);
                write_text(&handle, &path, new.clone(), &name).await?;
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
    // Leaving syntax mode drops what was previewed: the chart as it is.
    if !edit_state().input.mode.is_input() && edit_state().preview.take().is_some() {
        reload(Some(&path));
    }
    show_input(&chart, &score, refused);
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
    let mut from = edit_state().input.cursor.bar;
    // Typed over a replayed bar: written over the bar it plays.
    let mut note = None;
    if let Some(src) = Score::from_text(&text, &chart)
        .replays
        .get(from)
        .copied()
        .flatten()
    {
        note = Some(format!(
            "Bar {} plays bar {} again: written from bar {}",
            from + 1,
            src + 1,
            src + 1
        ));
        from = src;
    }
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
            note
        }
        Err(why) => Some(why),
    };
    let score = Score::from_text(&text, &chart);
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
    let score = Score::from_text(&text, &chart);
    if typed.trim().is_empty() {
        edit_state().preview = None;
        reload(Some(&path));
        show_input(&chart, &score, None);
        return Ok(input_state());
    }
    let refused = match keyflow::edit::commit::splice_syntax(&text, &chart, from, typed) {
        Ok((new, bars)) => {
            if let Ok(preview) = keyflow::text::chart::parse_chart(&new) {
                let read: Vec<String> = keyflow::edit::bars(&preview)
                    .iter()
                    .enumerate()
                    .skip(from)
                    .take(bars)
                    .filter_map(|(i, m)| {
                        keyflow::edit::chords::beats_read(m).map(|r| format!("bar {}: {r}", i + 1))
                    })
                    .collect();
                edit_state().preview_read = (!read.is_empty()).then(|| read.join(" · "));
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
            edit_state().preview_read = None;
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
    let score = Score::from_text(&text, &chart);
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
            _ if e.chord_draft.is_some() => {
                "Retyping the chord symbol · Enter sets it (its length kept), Esc keeps it as it was · Alt+↑ ↓ step its quality instead".to_owned()
            }
            Some(n) if e.input.mode == Mode::Syntax => format!(
                "Read as {} · Enter writes {}, Esc drops {} (`/` is a beat: `Em7 // A7 //` is two each)",
                e.preview_read.clone().unwrap_or_else(|| format!(
                    "{n} bar{} from bar {}",
                    if n == 1 { "" } else { "s" },
                    e.input.cursor.bar + 1
                )),
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
            ruler: false,
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
        // Input by duration's cursor: the ruler or the box, as chosen.
        ruler: input.mode == Mode::Duration && DURATION_RULER.load(Ordering::Relaxed),
    })
}

/// Whether input by duration shows MuseScore's above-staff ruler rather
/// than the box cursor (`KEYFLOW_DURATION_RULER`): the ruler, as
/// MuseScore's handbook shows it.
static DURATION_RULER: AtomicBool = AtomicBool::new(true);

/// The staff step of the top line of bar `bar`'s staff, by its clef.
fn top_line(chart: &keyflow::Chart, bar: usize) -> i32 {
    use keyflow::chart::ChartClef;
    let mut n = 0;
    // The engraver's default: a chart that declares no clef is in bass.
    let mut clef = chart.initial_clef.unwrap_or(ChartClef::Bass);
    for section in edit::sections(chart) {
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

/// ↩ on a selected chord symbol: it is retyped in a bar under the chart,
/// starting from the symbol as it is written.
async fn chord_edit() -> Result<Json, String> {
    let Some((_, path)) = active_chart() else {
        return Err("Open a chart first".to_owned());
    };
    let handle = handle()?;
    let text = read_text(&handle, &path).await?;
    let chart = keyflow::text::chart::parse_chart(&text)?;
    let score = Score::from_text(&text, &chart);
    let symbol = {
        let mut e = edit_state();
        e.input.target = edit::Target::Chords;
        e.input.selected_symbol(&score)
    };
    let refused = match symbol {
        Some(s) => {
            edit_state().chord_draft = Some(s);
            None
        }
        None => Some("Select a chord symbol to retype it".to_owned()),
    };
    show_input(&chart, &score, refused);
    Ok(input_state())
}

/// A bar clicked on a chart: noted as where note input starts, or the
/// cursor moved there.
async fn gesture(g: ChartGesture) {
    match g {
        // Keys are the keymap's (`[[keys]] when = "view:chart"`): the host
        // runs a bound key's action before the view hears it, and a key
        // bound to nothing is no command.
        ChartGesture::Key { .. } => {}
        ChartGesture::Click {
            bar,
            fraction,
            kind,
            below_top,
            voice,
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
                    // A note or rest of another voice clicked: that voice's.
                    "voice": voice,
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

/// Where a zoom key takes the chart's zoom.
#[derive(Clone, Copy)]
enum ZoomTo {
    /// Steps of the wheel's (in, positive; out, negative).
    Step(f64),
    Exactly(f64),
    /// The page fitted to the pane's width (the view's `None`).
    Fit,
}

/// The chart's paint view in its pane: a Blitz widget on a desk (and in
/// a process), a WebGL2 canvas in a browser (`web_pane`).
#[component]
fn ChartCanvas(mount: architect_workspace::PaintMount, pane: String, path: String) -> Element {
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = (pane, path);
        rsx! { PaintPane { mount } }
    }
    #[cfg(target_family = "wasm")]
    rsx! { web_pane::WebChartPane { mount, pane, path } }
}

/// The session actions the manifest declares, in a browser: a session
/// is hosted, joined and followed off the browser (`session_host`, vox
/// on a socket), so each says so. A browser's chart follows the app's
/// playhead instead (`web_pane`), and the engine in the page provides
/// `fts.session` there.
#[cfg(target_family = "wasm")]
fn browser_sessions(ext: &mut Builder) {
    const SESSIONS: [&str; 13] = [
        "KEYFLOW_READ",
        "KEYFLOW_WRITE",
        "KEYFLOW_STRUCTURE",
        "KEYFLOW_SESSION_BAR_ONE",
        "KEYFLOW_SESSION_DOCUMENT",
        "KEYFLOW_SESSION_FOLLOW",
        "KEYFLOW_SESSION_JOIN",
        "KEYFLOW_SESSION_OPEN",
        "KEYFLOW_SESSION_PLAY",
        "KEYFLOW_SESSION_SERVE",
        "KEYFLOW_SESSION_STATE",
        "KEYFLOW_SESSION_TYPE_CHART",
        "KEYFLOW_SESSION_TYPED",
    ];
    for name in SESSIONS {
        ext.action(name, move |_| async move {
            Err::<Json, String>(format!(
                "{name}: Keyflow hosts and follows sessions off the browser; \
                 in a browser its chart follows the app's playhead"
            ))
        });
    }
}

/// The song folder of the chart at `chart` (a vault path): its folder, or
/// for an arrangement's chart (`<Song>/Arrangements/<name>/x.kf`) the
/// song's. What a browser's chart matches the playing item against.
#[must_use]
pub fn song_folder(chart: &str) -> Option<std::path::PathBuf> {
    let folder = std::path::Path::new(chart).parent()?;
    let song = folder
        .ancestors()
        .find(|a| a.file_name().is_some_and(|n| n == "Arrangements"))
        .and_then(std::path::Path::parent)
        .unwrap_or(folder);
    (!song.as_os_str().is_empty()).then(|| song.to_owned())
}

/// Every open chart of `path` drawn with the playhead at `at` seconds
/// (none: no playhead), what a browser's chart follows the app's
/// playhead by (`web_pane`).
#[cfg(target_family = "wasm")]
fn set_playhead(path: &str, at: Option<f64>) {
    for open in open_charts().values().filter(|o| o.path == path) {
        open.data.update(|s| s.playhead_secs = at);
        open.wake.wake();
    }
}

/// The active chart zoomed as `to` says, about the middle of its pane:
/// the chart there stays there.
fn zoom_chart(to: ZoomTo) -> Json {
    let Some(o) = active_chart().and_then(|(k, _)| open_charts().get(&k).cloned()) else {
        return json!({ "zoomed": false });
    };
    let (now, (w, h)) = o
        .status
        .read(|s| (if s.zoom > 0.0 { s.zoom } else { 1.0 }, s.pane));
    let next = match to {
        ZoomTo::Step(n) => Some(keyflow_ui::paint_view::clamp_zoom(
            now * keyflow_ui::paint_view::wheel_zoom_factor(-20.0 * n),
        )),
        ZoomTo::Exactly(z) => Some(keyflow_ui::paint_view::clamp_zoom(z)),
        ZoomTo::Fit => None,
    };
    o.data.update(|s| {
        let after = next.unwrap_or(now);
        // The chart point at the pane's middle, kept there.
        let middle = (
            s.view.scroll.0 + w / 2.0 / now,
            s.view.scroll.1 + h / 2.0 / now,
        );
        let scroll = if next.is_none() {
            (0.0, s.view.scroll.1 + h / 2.0 / now - h / 2.0 / after)
        } else {
            (middle.0 - w / 2.0 / after, middle.1 - h / 2.0 / after)
        };
        s.view = ChartView {
            zoom: next,
            scroll: (scroll.0.max(0.0), scroll.1.max(0.0)),
        };
    });
    o.wake.wake();
    json!({ "zoom": next })
}

/// The zoom readout: `110%`, or `Fit · 64%` when the page fits the pane.
fn zoom_text(o: &Open) -> String {
    let zoom = o.status.read(|s| s.zoom);
    let fit = o.data.read(|s| s.view.zoom.is_none());
    let percent = (zoom * 100.0).round();
    if fit {
        format!("Fit · {percent}%")
    } else {
        format!("{percent}%")
    }
}

/// `status` without the mode it starts with (`Normal mode · …`), which
/// the status bar's chip shows: `KEYFLOW_INPUT_STATE` keeps it whole.
fn without_mode(status: &str, mode: Mode) -> String {
    status
        .strip_prefix(mode.name())
        .and_then(|rest| rest.strip_prefix(" · "))
        .unwrap_or(status)
        .to_owned()
}

/// What a chart's tab is called: its title (`"Drive To LA"`), else its
/// file's name without the extension, else "Chart".
fn tab_title(chart: &keyflow::Chart, path: &str) -> String {
    chart
        .metadata
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            let name = path.rsplit('/').next().unwrap_or(path);
            let stem = name.strip_suffix(".kf").unwrap_or(name);
            (!stem.is_empty()).then(|| stem.to_owned())
        })
        .unwrap_or_else(|| "Chart".to_owned())
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

    #[test]
    fn a_charts_song_folder_is_its_folder_or_its_arrangements_song() {
        let at = |c: &str| song_folder(c).map(|p| p.to_string_lossy().into_owned());
        assert_eq!(
            at("Libraries/L/Fire/Fire.kf").as_deref(),
            Some("Libraries/L/Fire")
        );
        assert_eq!(
            at("Libraries/L/Know Me/Arrangements/Extended/Know Me (Extended).kf").as_deref(),
            Some("Libraries/L/Know Me")
        );
        assert_eq!(at("Chart.kf"), None);
    }

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
        // Someone else's edit since, over the step's own text: not undone over.
        assert!(step_text(&s, "| D / / / |", true).is_err());
    }

    /// Two people typing in different bars: each one's undo takes back
    /// only its own edit, wherever the other's moved it.
    #[test]
    fn undo_takes_back_only_this_edit_past_anothers_in_another_bar() {
        let mine = Step {
            path: "session:x".into(),
            before: "| C / / / | F / / / | G / / / |".into(),
            after: "| C / / / | Dm7 / / / | G / / / |".into(),
            input: Default::default(),
        };
        // Theirs after mine (bar 3), and before it (bar 1).
        let later = "| C / / / | Dm7 / / / | G7sus4 / / / |";
        assert_eq!(
            step_text(&mine, later, true).as_deref(),
            Ok("| C / / / | F / / / | G7sus4 / / / |")
        );
        let earlier = "| Cmaj9 / / / | Dm7 / / / | G / / / |";
        assert_eq!(
            step_text(&mine, earlier, true).as_deref(),
            Ok("| Cmaj9 / / / | F / / / | G / / / |")
        );
        // Redo puts it back over theirs the same way.
        let undone = "| Cmaj9 / / / | F / / / | G / / / |";
        assert_eq!(
            step_text(&mine, undone, false).as_deref(),
            Ok("| Cmaj9 / / / | Dm7 / / / | G / / / |")
        );
        // Theirs inside my bar's chord: refused.
        assert!(step_text(&mine, "| C / / / | Dm9 / / / | G / / / |", true).is_err());
    }
}
