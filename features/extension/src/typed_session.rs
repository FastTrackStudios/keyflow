//! A session's chart as typed data (`keyflow_session::typed`): the chart's
//! bars as played, chords, rests, cues and dynamics as items on the
//! session's timeline, its sections the session's regions and its meters
//! and tempo the session's tempo map, written through `fts.session`
//! (whoever provides it).
//!
//! Asked for, not automatic: `KEYFLOW_SESSION_TYPE_CHART` types a
//! session's chart into its timeline. Once a session has been typed, an
//! edit to its chart keeps the items in step (only what the edit changed
//! is written: a chord pushed is one item put). The chart is still
//! engraved from its text entry; the typed items are what other views
//! read, and what the editor moves onto.

use architect_extensions::linked::{Builder, Handle};
use keyflow_session::ChartSource;
use keyflow_session::typed::{self, Held, TypedChart};
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
async fn timeline(handle: &Handle, source: &ChartSource) -> Result<Option<Held>, String> {
    let Some([t, s, d]) = source.timeline_calls().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let (t, s, d) = (
        ask_session(handle, &t).await?,
        ask_session(handle, &s).await?,
        ask_session(handle, &d).await?,
    );
    Held::of_replies(&t, &s, &d)
        .map(Some)
        .map_err(|e| e.to_string())
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
