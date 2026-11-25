//! A session's chart as typed data (`keyflow_session::typed`): the chart's
//! bars as played, chords, rests, cues and dynamics as items on the
//! session's timeline, its sections the session's regions and its meters
//! and tempo the session's tempo map, written through `fts.session`
//! (whoever provides it).
//!
//! The typed items are the chart's storage. The pane draws the session's
//! road map with them in it (`read_text`, `keyflow_session::shown`), and
//! an edit is written back as items ([`write_items`]): only the items it
//! changed, never the session's tempo map or regions. Keyflow's language
//! is an input method here, not storage: `KEYFLOW_SESSION_TYPE_CHART`
//! imports a chart written in it (`text`, a `.kf` at `chart_path`, or the
//! text entry an older Keyflow kept in the session) as items.
use architect_extensions::linked::{Builder, Handle};
use keyflow_session::ChartSource;
use keyflow_session::typed::{self, TypedChart};
use serde_json::{Value as Json, json};

use crate::{ask_session, handle, held_of, read_text, text};

pub(crate) fn build(ext: &mut Builder) {
    ext.action("KEYFLOW_SESSION_TYPE_CHART", |args| async move {
        let path = session_path(&args)?;
        let handle = handle()?;
        let source = import_source(&handle, &path, &args).await?;
        let wrote = retype(&handle, &path, &source, true).await?;
        Ok(json!({ "path": path, "written": wrote, "chart": summary(&handle, &path).await? }))
    });
    ext.action("KEYFLOW_SESSION_TYPED", |args| async move {
        let path = session_path(&args)?;
        summary(&handle()?, &path).await
    });
    ext.action("KEYFLOW_SESSION_SHOWN", |args| async move {
        let path = session_path(&args)?;
        let text = read_text(&handle()?, &path).await?;
        let error = keyflow::parse(&text).err().map(|e| e.to_string());
        Ok(json!({ "path": path, "text": text, "error": error }))
    });
}

/// The session chart an action is on: `doc` (a session's id), else the
/// active chart when it is a session's.
fn session_path(args: &Json) -> Result<String, String> {
    if let Some(doc) = text(args, "doc") {
        return Ok(ChartSource::Session { doc }.pane_state());
    }
    crate::active_chart()
        .map(|(_, path)| path)
        .filter(|path| {
            matches!(
                ChartSource::of_pane_state(path),
                ChartSource::Session { .. }
            )
        })
        .ok_or_else(|| "Name a session with `doc`, or open a session's chart".to_owned())
}

/// The chart a `KEYFLOW_SESSION_TYPE_CHART` imports, in Keyflow's
/// language: `text`, else the `.kf` at `chart_path`, else the text entry
/// an older Keyflow kept in the session (`keyflow/chart`).
async fn import_source(handle: &Handle, path: &str, args: &Json) -> Result<String, String> {
    if let Some(source) = text(args, "text") {
        return Ok(source);
    }
    if let Some(file) = text(args, "chart_path") {
        return std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"));
    }
    let call = ChartSource::of_pane_state(path)
        .read_call()
        .map_err(|e| e.to_string())?
        .ok_or("a session's chart only")?;
    let reply = ask_session(handle, &call).await?;
    let source = ChartSource::chart_of(&reply).map_err(|e| e.to_string())?;
    if source.trim().is_empty() {
        return Err("Nothing to import: give `text` or `chart_path`".to_owned());
    }
    Ok(source)
}

/// An edit of the chart a session shows (`text`: what `read_text`
/// composed, edited) written as Keyflow's items: only those it changed.
/// The session's tempo map and regions are not written: they are the
/// chart's structure already.
pub(crate) async fn write_items(handle: &Handle, path: &str, text: &str) -> Result<usize, String> {
    let at = ChartSource::of_pane_state(path);
    let Some(now) = held_of(handle, &at).await? else {
        return Ok(0);
    };
    let chart = keyflow::text::chart::parse_chart(text)?;
    let calls = at
        .typed_calls(typed::plan_items(&chart, &now))
        .map_err(|e| e.to_string())?;
    for call in &calls {
        ask_session(handle, call).await?;
    }
    Ok(calls.len())
}

/// The session at `path` made to hold `source`'s chart as typed items:
/// how many writes that took. With `force` false, only a session that has
/// been typed already is kept in step (0 for one that has not).
pub(crate) async fn retype(
    handle: &Handle,
    path: &str,
    source: &str,
    force: bool,
) -> Result<usize, String> {
    let at = ChartSource::of_pane_state(path);
    let Some(now) = held_of(handle, &at).await? else {
        return Ok(0);
    };
    if !force && !now.is_typed() {
        return Ok(0);
    }
    let chart = keyflow::text::chart::parse_chart(source)?;
    // A session with a tempo map or sections of its own keeps them, and
    // the chart must already agree with them: typed against a structure
    // it does not fit, its bars would not be the session's bars.
    let disagreements = now.disagreements(&chart);
    let brings = now.brings();
    if !(brings.tempo && brings.sections) && !disagreements.is_empty() {
        return Err(format!(
            "The chart does not fit the session's own structure, so it was not typed into it: {}",
            disagreements.join("; ")
        ));
    }
    let requests = typed::plan(&chart, &now);
    let calls = at.typed_calls(requests).map_err(|e| e.to_string())?;
    for call in &calls {
        ask_session(handle, call).await?;
    }
    Ok(calls.len())
}

/// `KEYFLOW_SESSION_TYPED`: what of the chart the session's timeline
/// holds as typed items, and what of the chart's text is not typed yet.
async fn summary(handle: &Handle, path: &str) -> Result<Json, String> {
    let at = ChartSource::of_pane_state(path);
    let Some(now) = held_of(handle, &at).await? else {
        return Err("A document has no timeline: this is for a session's chart".to_owned());
    };
    let chart = now.chart();
    let source = read_text(handle, path).await?;
    let untyped = keyflow::text::chart::parse_chart(&source)
        .map(|parsed| TypedChart::untyped(&parsed))
        .ok();
    Ok(json!({
        "path": path,
        "bar_one_quarter_notes": now.origin,
        "bars": chart.bars.iter().map(|b| json!({
            "bar": b.index + 1,
            "plays_written_bar": b.written + 1,
            "pass": b.pass,
            "meter": [b.meter.0, b.meter.1],
            "section": b.section,
        })).collect::<Vec<_>>(),
        "chords": chart.chords.iter().map(|c| json!({
            "bar": c.bar + 1,
            "beat": c.beat,
            "symbol": c.symbol,
            "at_quarter_notes": c.at,
            "pushed": c.push.map(|(early, amount)| json!({
                "direction": if early { "push" } else { "pull" },
                "level": amount.level,
                "beats": amount.to_beats(),
            })),
            "accent": c.accent,
            "staccato": c.staccato,
        })).collect::<Vec<_>>(),
        "rests": chart.rests.len(),
        "cues": chart.cues.iter().map(|c| json!({ "bar": c.bar + 1, "beat": c.beat, "text": c.text, "group": c.group })).collect::<Vec<_>>(),
        "dynamics": chart.dynamics.iter().map(|d| json!({ "bar": d.bar + 1, "beat": d.beat, "text": d.text })).collect::<Vec<_>>(),
        "not_typed_yet": untyped.map(|u| json!({
            "bars_with_written_notes": u.bars_with_notes,
            "hairpins": u.hairpins,
            "bars_not_placed_by_beat": u.bars_unplaced,
        })),
    }))
}
