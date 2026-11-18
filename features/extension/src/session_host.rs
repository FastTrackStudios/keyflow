//! Keyflow as a session's host, in a running app: `fts.session` provided to
//! every extension, and the session shared with another app instance.
//!
//! - `KEYFLOW_READ`, `KEYFLOW_WRITE`, `KEYFLOW_STRUCTURE`: `fts.session`'s
//!   three methods (the host runs `<PREFIX><METHOD>` as the caller, with
//!   its id in `cx.caller`), answered by `keyflow_session::Provider`.
//! - `KEYFLOW_SESSION_OPEN {path, chart?}`: opens a `.session` file (made
//!   with `chart` when it is not there) and serves it; answers its id and
//!   the pane state that shows it (`session:<id>`).
//! - `KEYFLOW_SESSION_SERVE {id, bind}`: serves that session to other app
//!   instances: Architect's document sync (`DocSyncHost` on this file's
//!   document), its presence, and the session clock, over vox on a
//!   WebSocket (`EngineHost`, `/vox`).
//! - `KEYFLOW_SESSION_JOIN {url, id, path}`: joins a served session: a
//!   `.session` of the same lineage at `path`, synced durably, its
//!   presence, and the host's clock estimated.
//! - `KEYFLOW_SESSION_PLAY {playing, position}`: publishes the transport,
//!   stamped in the shared clock (the host's).
//! - `KEYFLOW_SESSION_STATE`: what this instance holds and sees: each
//!   session's chart, the leader's transport, the followed playhead, the
//!   clock estimate, and this process's clock epoch in wall-clock time.
//!
//! Native only: hosting and joining are sockets.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use architect::LayerRouter;
use architect::connect::{Endpoint, pool};
use architect::host::EngineHost;
use architect_extensions::linked::Builder;
use crdt::presence::{Here, PeerState, PlayState};
use crdt::sync::{
    DocPresenceClient, DocPresenceDispatcher, DocSyncClient, DocSyncDispatcher, DocSyncHost,
    PresenceHost, PresencePeer, doc_presence_service_descriptor, doc_sync_service_descriptor,
};
use keyflow_session::{ChartSession, Provider};
use serde_json::{Value as Json, json};
use session::clock::{
    ClockHost, SessionClockClient, SharedClock, serve as serve_clock,
    session_clock_rpc_service_descriptor,
};
use session::doc::{SessionDoc, transport_of};
use session::sync::Tracker;
use session::sync::clock::now_micros_f64;
use uuid::Uuid;

/// What this instance hosts and follows.
#[derive(Default)]
struct Sessions {
    provider: Provider,
    charts: HashMap<Uuid, ChartSession>,
    presence: Option<PresencePeer>,
    here: Option<Here<PresencePeer>>,
    /// The host's clock, estimated; `None` on the host itself.
    clock: Option<SharedClock>,
    tracker: Tracker,
    /// Per session, the subscription that re-reads its chart panes when
    /// another replica's edit arrives.
    watches: HashMap<Uuid, crdt::loro::Subscription>,
    /// Sessions that are vault documents, held as the node's copy through
    /// the host (`KEYFLOW_SESSION_DOCUMENT`): no file of this extension's
    /// and no host of its own; the node syncs them like any document.
    documents: HashMap<Uuid, (architect_extensions::live::Replica, SessionDoc)>,
}

fn sessions() -> std::sync::MutexGuard<'static, Sessions> {
    static SESSIONS: OnceLock<Mutex<Sessions>> = OnceLock::new();
    SESSIONS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// This process's clock epoch in wall-clock microseconds: what a reader
/// on the same machine measures the shared clock's error against.
fn epoch_wall_micros() -> f64 {
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1e6;
    wall - now_micros_f64()
}

fn text(args: &Json, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Json::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("`{key}` is missing"))
}

fn id_of(args: &Json) -> Result<Uuid, String> {
    Uuid::parse_str(&text(args, "id")?).map_err(|e| e.to_string())
}

/// `fts.session`'s method `method`, for the extension in `cx.caller`.
fn answer(method: &'static str, args: &Json, cx: &Json) -> Result<Json, String> {
    let caller = cx.get("caller").and_then(Json::as_str);
    let reply = sessions()
        .provider
        .answer(method, &args.to_string(), caller)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&reply).map_err(|e| e.to_string())
}

/// Re-reads the panes showing session `id`'s chart when another
/// replica's edit arrives: the other machine's typing appears here.
fn watch(chart: &ChartSession, id: Uuid) -> crdt::loro::Subscription {
    let pane = format!("session:{id}");
    chart.on_remote_change(move || crate::reload(Some(&pane)))
}

/// A new session's chart: the `.kf` file at `chart_path`, or `chart`
/// (Keyflow source); either must read as a chart. A path given as
/// `chart` is refused, not taken for a one-bar chart titled with it.
fn chart_source(args: &Json) -> Result<String, String> {
    let source = match (
        args.get("chart_path").and_then(Json::as_str),
        args.get("chart").and_then(Json::as_str),
    ) {
        (Some(path), _) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        (None, Some(text)) => text.to_owned(),
        (None, None) => return Ok(String::new()),
    };
    keyflow::parse(&source).map_err(|e| format!("not a chart: {e}"))?;
    if !source.contains('\n') && (source.ends_with(".kf") || source.contains('/')) {
        return Err(format!(
            "`chart` is the chart's text; {source:?} looks like a path: give it as `chart_path`"
        ));
    }
    Ok(source)
}

async fn open(args: Json) -> Result<Json, String> {
    let path = PathBuf::from(text(&args, "path")?);
    let chart = if path.exists() {
        ChartSession::open(&path).await
    } else {
        let source = chart_source(&args)?;
        ChartSession::create(&path, &source).await
    }
    .map_err(|e| e.to_string())?;
    let id = chart.id();
    let mut s = sessions();
    s.provider.open(&chart);
    s.watches.insert(id, watch(&chart, id));
    s.charts.insert(id, chart);
    Ok(json!({ "id": id.to_string(), "pane": format!("session:{id}") }))
}

async fn serve(args: Json) -> Result<Json, String> {
    let id = id_of(&args)?;
    let bind = text(&args, "bind").unwrap_or_else(|_| "0.0.0.0:4142".into());
    // A session file this copy holds is served whole (its content too); a
    // vault document's content travels through the node and the org, so
    // only its transport is served here: presence and the host's clock.
    let (file, document) = {
        let s = sessions();
        (
            s.charts.get(&id).map(ChartSession::crdt),
            s.documents.contains_key(&id),
        )
    };
    if file.is_none() && !document {
        return Err("open the session first (KEYFLOW_SESSION_OPEN or _DOCUMENT)".to_owned());
    }
    let router = LayerRouter::new();
    let router = match file {
        Some(doc) => router.with(
            doc_sync_service_descriptor(),
            DocSyncDispatcher::new(DocSyncHost::new(id, doc)),
        ),
        None => router,
    };
    let router = router
        .with(
            doc_presence_service_descriptor(),
            DocPresenceDispatcher::new(PresenceHost::new(id, 30_000)),
        )
        .with(
            session_clock_rpc_service_descriptor(),
            serve_clock(ClockHost),
        );
    let port = bind.rsplit(':').next().unwrap_or("4142").to_owned();
    tokio::spawn(async move {
        let _ = EngineHost::ungated(router, bind).serve().await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // The host takes part in its own presence, as the leader: its clock
    // is the shared clock.
    let url = format!("ws://127.0.0.1:{port}/vox");
    join_presence(&url, id, "lead").await?;
    Ok(json!({ "serving": url, "id": id.to_string() }))
}

async fn join_presence(url: &str, id: Uuid, me: &str) -> Result<(), String> {
    let client: DocPresenceClient = pool()
        .client(&Endpoint::url(url))
        .await
        .map_err(|e| e.to_string())?;
    let (peer, mut driver) = PresencePeer::new(id, 30_000);
    tokio::spawn(async move {
        let _ = driver.run(&client).await;
    });
    peer.set(&format!("{me}/epoch"), epoch_wall_micros());
    let here = Here::new(peer.clone(), me, PeerState::new(me, me));
    let mut s = sessions();
    s.presence = Some(peer);
    s.here = Some(here);
    Ok(())
}

/// Follows a session hosted elsewhere (the DAW engine's, another app's)
/// without holding a copy or providing it: its presence and its host's
/// clock at `url`, so the chart panes on `session:<doc>` draw where its
/// leader is. The chart itself is read and written through the host's
/// `fts.session`, from whichever extension provides it.
async fn follow(args: Json) -> Result<Json, String> {
    let url = text(&args, "url")?;
    let id = id_of(&args)?;
    let host: SessionClockClient = pool()
        .client(&Endpoint::url(&url))
        .await
        .map_err(|e| e.to_string())?;
    let clock = SharedClock::default();
    let pinger = clock.clone();
    tokio::spawn(async move {
        let _ = pinger.run(&host).await;
    });
    join_presence(&url, id, "chart").await?;
    sessions().clock = Some(clock);
    // The chart into the session it follows, when it holds none yet: the
    // `.kf` at `chart_path`, written through the host's fts.session as
    // Keyflow's own data.
    let seeded = match args.get("chart_path").and_then(Json::as_str) {
        Some(path) => seed_chart(&id.to_string(), path).await?,
        None => false,
    };
    Ok(json!({
        "following": url,
        "id": id.to_string(),
        "pane": format!("session:{id}"),
        "seeded": seeded,
    }))
}

/// Writes the chart at `chart_path` into session `doc` if it holds no
/// chart: `true` when written.
async fn seed_chart(doc: &str, chart_path: &str) -> Result<bool, String> {
    let source = chart_source(&json!({ "chart_path": chart_path }))?;
    let handle = crate::handle()?;
    let pane = format!("session:{doc}");
    if !crate::read_text(&handle, &pane).await?.trim().is_empty() {
        return Ok(false);
    }
    crate::write_text(&handle, &pane, source, "Chart").await?;
    crate::reload(Some(&pane));
    Ok(true)
}

/// Makes bar `bar` of session `id` (held here) the song's bar 1: where
/// the chart's bar 1 is after a count-in. `bar` 1 or none: the session's
/// start. (A session the engine holds is the engine's to set.)
fn set_bar_one(args: &Json) -> Result<Json, String> {
    let id = id_of(args)?;
    let bar = args
        .get("bar")
        .and_then(Json::as_i64)
        .and_then(|b| i32::try_from(b).ok())
        .filter(|b| *b > 1);
    let s = sessions();
    // A session file held here, or a vault document held as the node's copy.
    let session = s
        .charts
        .get(&id)
        .map(|chart| chart.session().clone())
        .or_else(|| s.documents.get(&id).map(|(_, session)| session.clone()))
        .ok_or("this copy does not hold that session (KEYFLOW_SESSION_OPEN, _JOIN or _DOCUMENT)")?;
    drop(s);
    let at = bar.map(|b| session.tempo().at_bar(b, 1, 0.0));
    session.set_bar_one(at).map_err(|e| e.to_string())?;
    let start = session
        .bar_one()
        .map_or(0.0, |at| at.seconds(&session.tempo()));
    Ok(json!({ "bar_one": bar.unwrap_or(1), "chart_start_seconds": start }))
}

/// Where the followed session's leader is now, session seconds, and
/// whether it plays: `None` before the clock is estimated or the leader
/// has said anything. On the host itself the offset is zero.
#[must_use]
pub fn shown_now() -> Option<(f64, bool)> {
    let mut s = sessions();
    let offset = s
        .clock
        .as_ref()
        .map_or(Some(0.0), SharedClock::offset_micros)?;
    let leader = s
        .presence
        .as_ref()?
        .states()
        .get("lead/play")
        .and_then(transport_of)?;
    s.tracker.observe(leader);
    let at = s.tracker.position(now_micros_f64(), offset)?;
    Some((at, leader.is_playing))
}

/// Where the chart's bar 1 is in session `doc`, seconds, if this copy
/// holds it; `None` for a session held elsewhere (ask its provider).
#[must_use]
pub fn held_chart_start(doc: &str) -> Option<f64> {
    let id = Uuid::parse_str(doc).ok()?;
    let s = sessions();
    if let Some(chart) = s.charts.get(&id) {
        return Some(chart.chart_start());
    }
    s.documents.get(&id).map(|(_, session)| {
        session
            .bar_one()
            .map_or(0.0, |at| at.seconds(&session.tempo()))
    })
}

/// Opens the session that is the vault document at `path` (a `.session`
/// of the vault, synced through the org like any document): the node's
/// copy, edited through the host (`Handle::document`), and serves it as
/// `fts.session`. Keyflow holds no file and runs no host for it.
async fn document(args: Json) -> Result<Json, String> {
    let path = text(&args, "path")?;
    let handle = crate::handle()?;
    let lane = handle.document(&path).await.map_err(|e| format!("{e:?}"))?;
    let id = lane.doc().doc_id;
    let writable = lane.doc().writable;
    let replica = lane.replica();
    replica.attached().await?;
    let session = SessionDoc::from_doc(replica.doc().loro().clone());
    let pane = format!("session:{id}");
    let watch = {
        let pane = pane.clone();
        replica
            .doc()
            .loro()
            .subscribe_root(std::sync::Arc::new(move |event| {
                if matches!(event.triggered_by, crdt::loro::EventTriggerKind::Import) {
                    crate::reload(Some(&pane));
                }
            }))
    };
    let mut s = sessions();
    s.provider.open_doc(id, session.clone());
    s.watches.insert(id, watch);
    s.documents.insert(id, (replica, session));
    Ok(json!({ "id": id.to_string(), "pane": pane, "path": path, "writable": writable }))
}

async fn join(args: Json) -> Result<Json, String> {
    let url = text(&args, "url")?;
    let id = id_of(&args)?;
    let path = PathBuf::from(text(&args, "path")?);
    let chart = if path.exists() {
        ChartSession::open(&path).await
    } else {
        ChartSession::adopt(&path, id).await
    }
    .map_err(|e| e.to_string())?;
    let client: DocSyncClient = pool()
        .client(&Endpoint::url(&url))
        .await
        .map_err(|e| e.to_string())?;
    let mut synced = chart.synced();
    tokio::spawn(async move {
        let _ = synced.run(&client).await;
    });
    let host: SessionClockClient = pool()
        .client(&Endpoint::url(&url))
        .await
        .map_err(|e| e.to_string())?;
    let clock = SharedClock::default();
    let pinger = clock.clone();
    tokio::spawn(async move {
        let _ = pinger.run(&host).await;
    });
    join_presence(&url, id, "follow").await?;
    let mut s = sessions();
    s.provider.open(&chart);
    s.watches.insert(id, watch(&chart, id));
    s.charts.insert(id, chart);
    s.clock = Some(clock);
    Ok(json!({ "joined": url, "id": id.to_string(), "pane": format!("session:{id}") }))
}

fn play(args: &Json) -> Result<Json, String> {
    let playing = args.get("playing").and_then(Json::as_bool).unwrap_or(true);
    let position = args.get("position").and_then(Json::as_f64).unwrap_or(0.0);
    let s = sessions();
    let here = s.here.as_ref().ok_or("serve or join a session first")?;
    // On the host its own clock is the shared clock; a peer uses its
    // estimate of the host's.
    let at_micros = match &s.clock {
        None => now_micros_f64(),
        Some(clock) => clock
            .now_micros()
            .ok_or("the host's clock is not known yet")?,
    };
    here.play(&PlayState {
        playing,
        position,
        at_ms: at_micros / 1000.0,
        rate: 1.0,
    });
    Ok(json!({ "playing": playing, "position": position, "at_ms": at_micros / 1000.0 }))
}

fn state() -> Json {
    let mut s = sessions();
    let charts: Vec<Json> = s
        .charts
        .values()
        .map(|c| json!({ "id": c.id().to_string(), "chart": c.chart() }))
        .collect();
    let offset = s
        .clock
        .as_ref()
        .map_or(Some(0.0), SharedClock::offset_micros);
    let round_trip = s.clock.as_ref().and_then(SharedClock::round_trip_micros);
    let (leader, lead_epoch) = s.presence.as_ref().map_or((None, None), |peer| {
        let states = peer.states();
        (
            states.get("lead/play").and_then(transport_of),
            states
                .get("lead/epoch")
                .and_then(|v| v.clone().into_double().ok()),
        )
    });
    let now = now_micros_f64();
    let shown = match (leader, offset) {
        (Some(t), Some(offset)) => {
            s.tracker.observe(t);
            s.tracker.position(now, offset)
        }
        _ => None,
    };
    json!({
        "charts": charts,
        "leader": leader.map(|t| json!({
            "playing": t.is_playing, "position": t.playhead_seconds, "at_us": t.host_micros,
        })),
        "shown_seconds": shown,
        "offset_us": offset,
        "round_trip_us": round_trip,
        "epoch_wall_us": epoch_wall_micros(),
        "leader_epoch_wall_us": lead_epoch,
        "local_now_us": now,
    })
}

/// `call` answered by this copy's provider, as Keyflow, if this copy holds
/// the session it names: what the chart pane and note input do instead of
/// asking the host for this extension's own service. `None` when the
/// session is elsewhere.
#[must_use]
pub fn local_answer(call: &keyflow_session::ServiceCall) -> Option<Result<String, String>> {
    let args: Json = serde_json::from_str(&call.args).ok()?;
    let id = Uuid::parse_str(args.get("doc")?.as_str()?).ok()?;
    let sessions = sessions();
    if !sessions.charts.contains_key(&id) && !sessions.documents.contains_key(&id) {
        return None;
    }
    Some(
        sessions
            .provider
            .answer(call.method, &call.args, Some(keyflow_session::EXTENSION))
            .map_err(|e| e.to_string()),
    )
}

/// Registers the session host's actions.
pub fn build(ext: &mut Builder) {
    ext.action_in("KEYFLOW_READ", |args, cx| async move {
        answer(session::service::READ, &args, &cx)
    })
    .action_in("KEYFLOW_WRITE", |args, cx| async move {
        answer(session::service::WRITE, &args, &cx)
    })
    .action_in("KEYFLOW_STRUCTURE", |args, cx| async move {
        answer(session::service::STRUCTURE, &args, &cx)
    })
    .action("KEYFLOW_SESSION_OPEN", open)
    .action("KEYFLOW_SESSION_SERVE", serve)
    .action("KEYFLOW_SESSION_JOIN", join)
    .action("KEYFLOW_SESSION_FOLLOW", follow)
    .action("KEYFLOW_SESSION_DOCUMENT", document)
    .action("KEYFLOW_SESSION_BAR_ONE", |args| async move {
        set_bar_one(&args)
    })
    .action("KEYFLOW_SESSION_PLAY", |args| async move { play(&args) })
    .action("KEYFLOW_SESSION_STATE", |_| async { Ok(state()) });
}

#[cfg(test)]
mod tests {
    use super::{chart_source, open, set_bar_one};
    use serde_json::json;

    /// A session this copy holds gets its bar 1 here (no engine to set it):
    /// bar 5 of a 4/4 session at its default 120 BPM is 8 s in.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_sessions_bar_one_is_set_here() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.session");
        let opened = open(json!({
            "path": path.display().to_string(),
            "chart": "Verse: C G Am F\n",
        }))
        .await
        .unwrap();
        let id = opened["id"].as_str().unwrap().to_owned();
        let set = set_bar_one(&json!({ "id": id, "bar": 5 })).unwrap();
        assert!(
            (set["chart_start_seconds"].as_f64().unwrap() - 8.0).abs() < 1e-9,
            "{set}"
        );
        let cleared = set_bar_one(&json!({ "id": id })).unwrap();
        assert!(cleared["chart_start_seconds"].as_f64().unwrap().abs() < 1e-9);
        assert!(set_bar_one(&json!({ "id": uuid::Uuid::new_v4().to_string(), "bar": 5 })).is_err());
    }

    #[test]
    fn a_new_sessions_chart_is_text_or_a_file_never_a_path_taken_for_text() {
        let chart = chart_source(&json!({ "chart": "Verse: C G Am F\n" })).unwrap();
        assert_eq!(chart, "Verse: C G Am F\n");
        assert!(
            chart_source(&json!({ "chart": "Charts/02 Drive To LA.kf" })).is_err(),
            "a vault path given as the chart's text"
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.kf");
        std::fs::write(&file, "Verse: C G\n").unwrap();
        let chart = chart_source(&json!({ "chart_path": file.display().to_string() })).unwrap();
        assert_eq!(chart, "Verse: C G\n");
        assert!(chart_source(&json!({ "chart_path": "/nowhere/a.kf" })).is_err());
        assert_eq!(chart_source(&json!({})).unwrap(), "");
    }
}
