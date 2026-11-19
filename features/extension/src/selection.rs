//! Actions on the selection (`keyflow::edit::select`): push, pull, accent,
//! staccato, transpose, retype, on what is selected on the chart or what a
//! script selects, on the active chart or one named by `path` (open in a
//! pane or not).
//!
//! The selection is read and set by script (`KEYFLOW_SELECTION`,
//! `KEYFLOW_SELECT`); every action takes `select` to act on something
//! else without changing it, and `KEYFLOW_EDIT` runs several as one edit,
//! one undo step. The keys and the palette run these same actions.

use std::sync::{Mutex, PoisonError};

use architect_extensions::linked::Builder;
use keyflow::edit::select::{self, Action, Pick, Step};
use serde_json::{Value as Json, json};

use crate::{handle, open_charts, read_text, reload, text, write_text};

/// What a script selected, on which chart: used until the person selects
/// something on that chart themselves.
static SCRIPTED: Mutex<Option<(String, Vec<Pick>)>> = Mutex::new(None);

fn scripted() -> std::sync::MutexGuard<'static, Option<(String, Vec<Pick>)>> {
    SCRIPTED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The actions on the selection, each with its name in the manifest.
const ACTIONS: &[(&str, &str)] = &[
    ("KEYFLOW_SELECTION_PUSH", "push"),
    ("KEYFLOW_SELECTION_PULL", "pull"),
    ("KEYFLOW_SELECTION_ACCENT", "accent"),
    ("KEYFLOW_SELECTION_STACCATO", "staccato"),
    ("KEYFLOW_SELECTION_TRANSPOSE", "transpose"),
    ("KEYFLOW_SELECTION_CHORD", "chord"),
];

pub(crate) fn build(ext: &mut Builder) {
    for (id, name) in ACTIONS {
        let name = *name;
        ext.action(*id, move |args| async move {
            let action = action_of(name, &args)?;
            let path = path_of(&args)?;
            let picks = match args.get("select") {
                Some(select) => picks_of(select)?,
                None => current(&path),
            };
            run(&path, vec![(picks, action)]).await
        });
    }
    ext.action("KEYFLOW_SELECT", |args| async move {
        let path = path_of(&args)?;
        let picks = picks_of(args.get("select").unwrap_or(&args))?;
        // Bars selected show selected on the chart, where it is open.
        let keys: Vec<String> = picks
            .iter()
            .flat_map(|p| match p {
                Pick::Bars { from, to } => (*from..=*to).map(|b| format!("bar#{b}")).collect(),
                Pick::At { bar, .. } => vec![format!("bar#{bar}")],
                _ => Vec::new(),
            })
            .collect();
        for open in open_charts().values().filter(|o| o.path == path) {
            open.selection.update(|s| {
                s.keys.clone_from(&keys);
                s.items.clear();
            });
            open.wake.wake();
        }
        *scripted() = Some((path.clone(), picks));
        selection(&path).await
    });
    ext.action("KEYFLOW_SELECTION", |args| async move {
        selection(&path_of(&args)?).await
    });
    ext.action("KEYFLOW_DESCRIBE", |args| async move {
        let path = path_of(&args)?;
        let source = read_text(&handle()?, &path).await?;
        describe(&path, &source)
    });
    ext.action("KEYFLOW_EDIT", |args| async move {
        let path = path_of(&args)?;
        let steps = args
            .get("steps")
            .and_then(Json::as_array)
            .ok_or("steps: a list of {select, action, …}")?;
        let mut todo = Vec::with_capacity(steps.len());
        for step in steps {
            let name = text(step, "action").ok_or("each step names its action")?;
            let picks = picks_of(step.get("select").ok_or("each step says what it selects")?)?;
            todo.push((picks, action_of(&name, step)?));
        }
        run(&path, todo).await
    });
}

/// The chart an action is on: `path`, else the active one.
fn path_of(args: &Json) -> Result<String, String> {
    text(args, "path")
        .or_else(|| crate::active_chart().map(|(_, path)| path))
        .ok_or_else(|| "Open a chart first, or name one with `path`".to_owned())
}

/// `name` (`push`, `pull`, `accent`, `staccato`, `transpose`, `chord`)
/// with its arguments: `step`, `by`, `text`.
fn action_of(name: &str, args: &Json) -> Result<Action, String> {
    let step = match args.get("step").and_then(Json::as_str) {
        Some(named) => Some(Step::named(named).ok_or_else(|| {
            format!(
                "step: {named} is no amount a push holds (eighth, sixteenth, thirty-second, triplet, quarter)"
            )
        })?),
        None => None,
    };
    Ok(match name {
        "push" => Action::Push(step),
        "pull" => Action::Pull(step),
        "accent" => Action::Accent,
        "staccato" => Action::Staccato,
        "transpose" => Action::Transpose(
            args.get("by")
                .and_then(Json::as_i64)
                .and_then(|n| i8::try_from(n).ok())
                .filter(|n| (-11..=11).contains(n) && *n != 0)
                .ok_or("by: semitones, -11 to 11")?,
        ),
        "chord" => Action::Retype(text(args, "text").ok_or("text: the chord symbol")?),
        other => {
            return Err(format!(
                "{other}: not an action on the selection (push, pull, accent, staccato, transpose, chord)"
            ));
        }
    })
}

/// A selection a script writes: `{bar}` or `{bar, beat}` (bars and beats
/// from 1, as the chart prints them), `{bars: [from, to]}`, or a list of
/// those.
fn picks_of(select: &Json) -> Result<Vec<Pick>, String> {
    if let Some(list) = select.as_array() {
        let mut out = Vec::new();
        for one in list {
            out.extend(picks_of(one)?);
        }
        return Ok(out);
    }
    let bar_of = |v: &Json| {
        v.as_u64()
            .filter(|n| *n >= 1)
            .map(|n| n as usize - 1)
            .ok_or("a bar is a number from 1")
    };
    if let Some(range) = select.get("bars").and_then(Json::as_array) {
        let [from, to] = range.as_slice() else {
            return Err("bars: [from, to]".to_owned());
        };
        let (from, to) = (bar_of(from)?, bar_of(to)?);
        if to < from {
            return Err("bars: from comes first".to_owned());
        }
        return Ok(vec![Pick::Bars { from, to }]);
    }
    if let Some(bar) = select.get("bar") {
        let bar = bar_of(bar)?;
        return Ok(vec![match select.get("beat").and_then(Json::as_f64) {
            Some(beat) => Pick::At { bar, beat },
            None => Pick::Bars { from: bar, to: bar },
        }]);
    }
    Err("select: {bar}, {bar, beat}, {bars: [from, to]} or a list of them".to_owned())
}

/// What is selected on `path`: what the person selected on its chart, else
/// what a script selected, else where note input's selection is.
fn current(path: &str) -> Vec<Pick> {
    let open = open_charts().values().find(|o| o.path == path).cloned();
    if let Some(open) = &open {
        let beats = |bar: usize| {
            open.data.read(|s| {
                keyflow::edit::bars(&s.chart)
                    .get(bar)
                    .map_or(4.0, |m| f64::from(m.time_signature.0))
            })
        };
        let picks: Vec<Pick> = open.selection.read(|s| {
            s.items
                .iter()
                .map(|item| match (item.kind, item.span, item.bar) {
                    ("bar", _, Some((bar, _))) => Pick::Bars { from: bar, to: bar },
                    ("chord", Some((start, end)), _) => Pick::Span { start, end },
                    // A slash, a note or a rest: the chord at its beat.
                    ("chord" | "note" | "rest", _, Some((bar, fraction))) => Pick::At {
                        bar,
                        beat: 1.0 + (fraction * beats(bar) * 2.0).round() / 2.0,
                    },
                    (kind, ..) => Pick::Other(kind.to_owned()),
                })
                .collect()
        });
        if !picks.is_empty() {
            return picks;
        }
    }
    if let Some((on, picks)) = scripted().clone()
        && on == path
    {
        return picks;
    }
    let e = crate::edit_state();
    match (e.input.selected, open) {
        (Some(pos), Some(open)) => {
            let length = open.data.read(|s| {
                keyflow::edit::bars(&s.chart)
                    .get(pos.bar)
                    .map_or((4, 4), |m| m.time_signature)
            });
            let beat = f64::from(keyflow::edit::TICKS_PER_WHOLE) / f64::from(length.1.max(1));
            vec![Pick::At {
                bar: pos.bar,
                beat: 1.0 + f64::from(pos.tick) / beat,
            }]
        }
        _ => Vec::new(),
    }
}

/// `KEYFLOW_SELECTION`: the selection on `path`, and the chords it names.
async fn selection(path: &str) -> Result<Json, String> {
    let picks = current(path);
    let source = read_text(&handle()?, path).await?;
    let chart = keyflow::text::chart::parse_chart(&source)?;
    let (chords, skipped) = select::resolve(&source, &chart, &picks);
    Ok(json!({
        "path": path,
        "chords": chords.iter().map(chord_json).collect::<Vec<_>>(),
        "not_chords": skipped,
    }))
}

fn chord_json(c: &select::ChordRef) -> Json {
    json!({
        "bar": c.bar + 1,
        "beat": c.beat,
        "chord": c.token,
        "push": push_json(c.push_pull),
    })
}

/// A chord's push field, as a script reads it.
fn push_json(field: select::PushPull) -> Json {
    match field {
        None => Json::Null,
        Some((push, amount)) => json!({
            "direction": if push { "push" } else { "pull" },
            "level": amount.level,
            "beats": amount.to_beats(),
        }),
    }
}

/// `steps` done to `path`'s chart, one after another, saved as one edit
/// (one undo step). All or nothing: a step refused refuses the edit.
async fn run(path: &str, steps: Vec<(Vec<Pick>, Action)>) -> Result<Json, String> {
    let _one = crate::ONE_AT_A_TIME.lock().await;
    let handle = handle()?;
    let before = read_text(&handle, path).await?;
    let mut now = before.clone();
    let mut changed = Vec::new();
    let mut skipped = Vec::new();
    let mut names = Vec::new();
    for (picks, action) in &steps {
        let done = select::apply(&now, picks, action).inspect_err(|why| {
            crate::edit_state().refused = Some(why.clone());
        })?;
        now = done.text;
        skipped.extend(done.skipped);
        changed.extend(done.changed.iter().map(|c| {
            json!({
                "bar": c.bar + 1,
                "was": c.was,
                "now": c.now,
                "push": push_json(c.push_pull.1),
            })
        }));
        names.push(action.name());
    }
    names.dedup();
    let name = match names.as_slice() {
        [one] => one.clone(),
        _ => "Edit chart".to_owned(),
    };
    write_text(&handle, path, now.clone(), &name).await?;
    crate::remember_edit(path, before, now);
    crate::edit_state().refused = skipped.first().cloned();
    reload(Some(path));
    crate::rerender_input().await;
    Ok(json!({ "path": path, "changed": changed, "skipped": skipped }))
}

/// `KEYFLOW_DESCRIBE`: the chart's structure, for a script to read, decide
/// and write: its sections, and every bar with its meter and chords (each
/// with its beat and its push).
fn describe(path: &str, source: &str) -> Result<Json, String> {
    let chart = keyflow::text::chart::parse_chart(source)?;
    let chords = select::chords_of(source, &chart);
    let plays = keyflow::edit::replays(&chart);
    let mut bars = Vec::new();
    let mut sections = Vec::new();
    let mut bar = 0;
    for section in keyflow::edit::sections(&chart) {
        let count = section.measures().len();
        sections.push(json!({
            "name": section.section.display_name(),
            "bars": [bar + 1, bar + count],
        }));
        for (i, measure) in section.measures().iter().enumerate() {
            let n = bar + i;
            bars.push(json!({
                "bar": n + 1,
                "section": sections.len(),
                "meter": [measure.time_signature.0, measure.time_signature.1],
                "plays_bar": plays.get(n).copied().flatten().map(|b| b + 1),
                "repeat_sign": measure.simile,
                "chords": chords.iter().filter(|c| c.bar == n).map(chord_json).collect::<Vec<_>>(),
                "sounding": measure.chords.iter().map(|c| c.full_symbol.clone()).collect::<Vec<_>>(),
                "cues": measure
                    .text_cues
                    .iter()
                    .map(|c| json!({ "text": c.text, "beat": c.beat }))
                    .chain(measure.staff_text.iter().map(|t| json!({ "text": t.text, "beat": t.beat })))
                    .collect::<Vec<_>>(),
            }));
        }
        bar += count;
    }
    Ok(json!({
        "path": path,
        "title": chart.metadata.title,
        "tempo": chart.tempo.as_ref().map(|t| t.bpm),
        "sections": sections,
        "bars": bars,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_selects_by_bar_and_beat_from_one() {
        assert_eq!(
            picks_of(&json!({ "bar": 12, "beat": 3 })).unwrap(),
            [Pick::At { bar: 11, beat: 3.0 }]
        );
        assert_eq!(
            picks_of(&json!({ "bars": [9, 16] })).unwrap(),
            [Pick::Bars { from: 8, to: 15 }]
        );
        assert_eq!(
            picks_of(&json!([{ "bar": 2 }, { "bar": 4, "beat": 1.5 }])).unwrap(),
            [
                Pick::Bars { from: 1, to: 1 },
                Pick::At { bar: 3, beat: 1.5 }
            ]
        );
        assert!(picks_of(&json!({ "bar": 0 })).is_err(), "bars count from 1");
        assert!(picks_of(&json!({ "bars": [5, 2] })).is_err());
    }

    #[test]
    fn an_actions_arguments_are_checked_by_name() {
        assert_eq!(
            action_of("push", &json!({ "step": "sixteenth" })).unwrap(),
            Action::Push(Some(Step::Sixteenth))
        );
        assert_eq!(action_of("pull", &json!({})).unwrap(), Action::Pull(None));
        let why = action_of("push", &json!({ "step": "half" })).unwrap_err();
        assert!(why.contains("no amount a push holds"), "{why}");
        assert!(action_of("transpose", &json!({ "by": 0 })).is_err());
        assert_eq!(
            action_of("transpose", &json!({ "by": -2 })).unwrap(),
            Action::Transpose(-2)
        );
        assert!(action_of("wobble", &json!({})).is_err());
    }

    #[test]
    fn the_structure_lists_every_bar_with_its_chords() {
        let src = "Song - Test\n#C 120bpm 4/4\n\nVS 2\nC // 'G //\n%\n\nCH 1\n\"Hit\" F\n";
        let out = describe("a.kf", src).unwrap();
        assert_eq!(out["title"], "Song");
        assert_eq!(out["sections"].as_array().unwrap().len(), 2);
        let bars = out["bars"].as_array().unwrap();
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0]["chords"][1]["chord"], "'G");
        assert_eq!(bars[0]["chords"][1]["push"]["direction"], "push");
        assert_eq!(bars[0]["chords"][1]["push"]["beats"], 0.5);
        assert_eq!(bars[1]["repeat_sign"], true);
        assert_eq!(
            bars[1]["chords"].as_array().unwrap().len(),
            0,
            "it writes none"
        );
        assert_eq!(bars[2]["cues"][0]["text"], "Hit");
    }

    #[test]
    fn every_action_is_in_the_manifest() {
        for (id, _) in ACTIONS {
            assert!(crate::MANIFEST.contains(&format!("id = \"{id}\"")), "{id}");
        }
        for id in [
            "KEYFLOW_SELECT",
            "KEYFLOW_SELECTION",
            "KEYFLOW_DESCRIBE",
            "KEYFLOW_EDIT",
        ] {
            assert!(crate::MANIFEST.contains(&format!("id = \"{id}\"")), "{id}");
        }
    }
}
