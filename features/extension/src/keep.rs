//! The view as its pane keeps it (`use_view_state`, architect#859): each
//! chart's zoom, scroll and selection, and note input's mode, tools and
//! cursor, as JSON, given back when the view starts again (its program
//! restarted, its pane moved, the app reopened). It is the person's view
//! of the chart, not the chart: nothing here is shared with another peer.

use keyflow::edit::{Duration, InputState, Mode, Pitch, Pos, Target, Value};
use serde_json::{Value as Json, json};

fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Normal => "normal",
        Mode::NoteName => "note_name",
        Mode::Duration => "duration",
        Mode::Syntax => "syntax",
    }
}

fn mode_of(s: &str) -> Mode {
    match s {
        "note_name" => Mode::NoteName,
        "duration" => Mode::Duration,
        "syntax" => Mode::Syntax,
        _ => Mode::Normal,
    }
}

fn pos(p: Pos) -> Json {
    json!({ "bar": p.bar, "tick": p.tick })
}

fn pos_of(v: &Json) -> Option<Pos> {
    Some(Pos {
        bar: usize::try_from(v.get("bar")?.as_u64()?).ok()?,
        tick: u32::try_from(v.get("tick")?.as_u64()?).ok()?,
    })
}

/// Note input as kept.
#[must_use]
pub fn input_json(i: &InputState) -> Json {
    json!({
        "mode": mode_name(i.mode),
        "duration": i.duration.value.key(),
        "dots": i.duration.dots,
        "accidental": i.accidental,
        "rest": i.rest,
        "cursor": pos(i.cursor),
        "pitch": [i.pitch.letter.to_string(), i.pitch.alter, i.pitch.octave],
        "selected": i.selected.map(pos),
        "target": match i.target { Target::Chords => "chords", Target::Notes => "notes" },
        "voice": i.voice,
        "triplet": i.duration.triplet,
    })
}

/// Note input from what was kept; what is missing or not understood keeps
/// its default.
#[must_use]
pub fn input_from(v: &Json) -> InputState {
    let mut i = InputState::default();
    if let Some(m) = v.get("mode").and_then(Json::as_str) {
        i.mode = mode_of(m);
    }
    if let Some(value) = v
        .get("duration")
        .and_then(Json::as_u64)
        .and_then(|k| u8::try_from(k).ok())
        .and_then(Value::from_key)
    {
        i.duration = Duration {
            value,
            dots: v
                .get("dots")
                .and_then(Json::as_u64)
                .map_or(0, |d| d.min(2) as u8),
            triplet: v.get("triplet").and_then(Json::as_bool).unwrap_or(false),
        };
    }
    i.accidental = v
        .get("accidental")
        .and_then(Json::as_i64)
        .and_then(|a| i8::try_from(a).ok());
    i.rest = v.get("rest").and_then(Json::as_bool).unwrap_or(false);
    if let Some(p) = v.get("cursor").and_then(pos_of) {
        i.cursor = p;
    }
    i.selected = v.get("selected").and_then(pos_of);
    if let Some([l, a, o]) = v.get("pitch").and_then(Json::as_array).map(Vec::as_slice) {
        if let (Some(l), Some(a), Some(o)) = (
            l.as_str().and_then(|s| s.chars().next()),
            a.as_i64(),
            o.as_i64(),
        ) {
            i.pitch = Pitch::new(l, a as i8, o as i8);
        }
    }
    i.voice = v
        .get("voice")
        .and_then(Json::as_u64)
        .map_or(0, |n| n.min(3) as u8);
    i.target = match v.get("target").and_then(Json::as_str) {
        Some("notes") => Target::Notes,
        _ => Target::Chords,
    };
    i
}

/// One chart's view as kept.
#[must_use]
pub fn chart_json(zoom: Option<f64>, scroll: (f64, f64), selection: &[String]) -> Json {
    json!({ "zoom": zoom, "scroll": [scroll.0, scroll.1], "selection": selection })
}

/// One chart's view from what was kept: `(zoom, scroll, selection)`.
#[must_use]
pub fn chart_from(v: &Json) -> (Option<f64>, (f64, f64), Vec<String>) {
    let zoom = v.get("zoom").and_then(Json::as_f64);
    let scroll = v
        .get("scroll")
        .and_then(Json::as_array)
        .map_or((0.0, 0.0), |s| {
            (
                s.first().and_then(Json::as_f64).unwrap_or(0.0),
                s.get(1).and_then(Json::as_f64).unwrap_or(0.0),
            )
        });
    let selection = v
        .get("selection")
        .and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    (zoom, scroll, selection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_input_comes_back_as_it_was_kept() {
        let mut i = InputState {
            mode: Mode::Duration,
            duration: Duration::dotted(Value::Eighth),
            accidental: Some(-1),
            rest: true,
            cursor: Pos { bar: 11, tick: 96 },
            pitch: Pitch::new('F', 1, 3),
            selected: Some(Pos { bar: 11, tick: 0 }),
            target: Target::Notes,
            voice: 2,
            repitch: false,
        };
        let back = input_from(&input_json(&i));
        assert_eq!(back, i);
        i.mode = Mode::Syntax;
        assert_eq!(input_from(&input_json(&i)).mode, Mode::Syntax);
        // Nothing kept: the defaults.
        assert_eq!(input_from(&json!({})), InputState::default());
    }

    #[test]
    fn a_chart_s_view_comes_back_as_it_was_kept() {
        let v = chart_json(Some(1.5), (10.0, 200.0), &["chord#3".to_owned()]);
        assert_eq!(
            chart_from(&v),
            (Some(1.5), (10.0, 200.0), vec!["chord#3".to_owned()])
        );
        assert_eq!(chart_from(&chart_json(None, (0.0, 0.0), &[])).0, None);
    }
}
