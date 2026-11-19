//! A session's chart as typed data (`keyflow_session::typed`): the chart's
//! bars, chords, rests, cues and dynamics as items on the session's
//! timeline, written through `fts.session` (whoever provides it).
//!
//! Asked for, not automatic: `KEYFLOW_SESSION_TYPE_CHART` types a
//! session's chart into its timeline. Once a session has been typed, an
//! edit to its chart keeps the items in step (only what the edit changed
//! is written: a chord pushed is one item put). The chart is still
//! engraved from its text entry; the typed items are what other views
//! read, and what the editor moves onto.

use architect_extensions::linked::{Builder, Handle};
use keyflow_session::ChartSource;
use keyflow_session::typed::{self, Timeline, TypedChart};
use serde_json::{Value as Json, json};

use crate::{ask_session, handle, read_text, text};

pub(crate) fn build(ext: &mut Builder) {
    ext.action("KEYFLOW_SESSION_TYPE_CHART", |args| async move {
        let path = session_path(&args)?;
        let handle = handle()?;
        let source = read_text(&handle, &path).await?;
        let wrote = retype(&handle, &path, &source, true).await?;
        Ok(json!({ "path": path, "written": wrote, "chart": summary(&handle, &path).await? }))
    });
    ext.action("KEYFLOW_SESSION_TYPED", |args| async move {
        let path = session_path(&args)?;
        summary(&handle()?, &path).await
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

/// The session's timeline as its provider answers.
async fn timeline(handle: &Handle, source: &ChartSource) -> Result<Option<Timeline>, String> {
    let Some([t, s]) = source.timeline_calls().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let (t, s) = (
        ask_session(handle, &t).await?,
        ask_session(handle, &s).await?,
    );
    Timeline::of(&t, &s).map(Some).map_err(|e| e.to_string())
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
    let Some(now) = timeline(handle, &at).await? else {
        return Ok(0);
    };
    let typed_already = now.lanes.keys().any(|lane| lane.as_str() == typed::BARS);
    if !force && !typed_already {
        return Ok(0);
    }
    let chart = keyflow::text::chart::parse_chart(source)?;
    let requests = typed::plan(&TypedChart::of(&chart), now.origin, &now.lanes, &now.items);
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
    let Some(now) = timeline(handle, &at).await? else {
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
        "bars": chart.bars.len(),
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
            "bars_with_repeats_or_endings": u.repeats,
            "bars_not_placed_by_beat": u.bars_unplaced,
        })),
    }))
}
