//! The chart as typed data in the session document.
//!
//! A session's chart has been one text entry holding Keyflow source
//! ([`crate::CHART`]). This is the chart as the session's own timeline
//! data instead: lanes Keyflow owns, an item per bar, per chord and per
//! cue, each at its time on the session's timeline with a typed payload.
//! So a chord is something in the document: it merges between people
//! field by field (one pushes it while another accents it, and both
//! land), it is in the document's history and undo, and another view
//! reads where the bars, chords and cues are from the timeline
//! (`ReadTimeline` gives every extension the lanes, kinds and times;
//! the payloads are Keyflow's).
//!
//! # The schema (version [`SCHEMA`])
//!
//! Five lanes, owner `keyflow`:
//!
//! | lane | item kind | at … end | payload |
//! |---|---|---|---|
//! | `keyflow.bars` | `keyflow.bar` | the bar's span | `index`, `numerator`, `denominator`, `section`, `section_start`, `plays` (the bar it plays again), `repeat_sign` |
//! | `keyflow.chords` | `keyflow.chord` | the beat it is written on … the next chord or the barline | `symbol`, `bar`, `beat`, `push` (`early`, `level`, `base`, …; absent when not pushed), `accent`, `staccato` |
//! | `keyflow.cues` | `keyflow.cue` | its bar and beat | `text`, `bar`, `beat`, `group` |
//! | `keyflow.rhythm` | `keyflow.rest` | the rest's span | `bar`, `beat` |
//! | `keyflow.dynamics` | `keyflow.dynamic` | its bar and beat | `text` (`mf`), `bar`, `beat` |
//!
//! Times are quarter notes on the session's timeline, from the song's
//! bar 1 ([`session::Session::bar_one`]): the chart's bar 1 is the
//! session's, by the same number, not by a second count kept in step.
//!
//! A chord's **push** is its own field, the model's
//! (`ChordInstance::push_pull`): [`set_push`] is one write to one item.
//!
//! # What is typed, and what is not yet
//!
//! Typed: every bar and its meter, the section each belongs to, every
//! chord with its beat, length, push, accent and staccato, every written
//! rest, every dynamic, and every text and instrument cue. **Not yet
//! typed** ([`Untyped`] counts what it can for a chart): written melodies
//! and notes, ties, hairpins, lyric lanes, repeats and endings, fermatas
//! and stops, key changes, and the header (title, key, tempo).
//! Until those are, the text entry stays beside this and is what the
//! chart is engraved from; this module is not wired into a running
//! session yet.
//!
//! Sections are written as this lane's bars' `section`, not yet as the
//! session's regions: a region is the session's structure (it needs the
//! structure grant, and the arrangement owns its names), and making the
//! chart's sections *be* those regions is the next step, not a copy made
//! here.

use std::collections::{BTreeMap, BTreeSet};

use keyflow_lang::edit::select::{PushPull, beats_of};
use keyflow_lang::edit::{bars, chords, replays, sections};
use keyflow_proto::Chart;
use keyflow_proto::chord::{LilySyntax, PushPullAmount, PushPullBase};
use session::doc::{DocResult, SessionDoc};
use session::{ExtensionId, Item, ItemId, Lane, LaneId, Time, Value};

/// This schema's version, kept in Keyflow's namespace as register
/// [`SCHEMA_ENTRY`]: Keyflow migrates its own data.
pub const SCHEMA: i64 = 1;
/// The register the schema version is kept in.
pub const SCHEMA_ENTRY: &str = "chart_schema";

pub const BARS: &str = "keyflow.bars";
pub const CHORDS: &str = "keyflow.chords";
pub const CUES: &str = "keyflow.cues";
pub const BAR: &str = "keyflow.bar";
pub const CHORD: &str = "keyflow.chord";
pub const CUE: &str = "keyflow.cue";
pub const RHYTHM: &str = "keyflow.rhythm";
pub const REST: &str = "keyflow.rest";
pub const DYNAMICS: &str = "keyflow.dynamics";
pub const DYNAMIC: &str = "keyflow.dynamic";

/// Keyflow's lanes, in order, with their names.
const LANES: [(&str, &str); 5] = [
    (BARS, "Bars"),
    (CHORDS, "Chords"),
    (RHYTHM, "Rhythm"),
    (CUES, "Cues"),
    (DYNAMICS, "Dynamics"),
];

/// One bar.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedBar {
    /// Its place among the chart's bars, from 0.
    pub index: usize,
    pub meter: (u8, u8),
    /// Its section's name as the chart shows it (`VS 1`).
    pub section: String,
    /// Whether it is its section's first bar.
    pub section_start: bool,
    /// The bar it plays again, when it writes none of its own.
    pub plays: Option<usize>,
    /// Whether it is drawn as a repeat-bar sign.
    pub repeat_sign: bool,
    /// Where it starts and how long it is, in quarter notes from bar 1.
    pub at: f64,
    pub length: f64,
}

/// One chord.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedChord {
    pub bar: usize,
    /// Its place among its bar's chords, from 0.
    pub index: usize,
    /// Its symbol (`F#m7/A`).
    pub symbol: String,
    /// The beat it is written on, from 1.
    pub beat: f64,
    /// Where that is and how long it lasts (to the next chord or the
    /// barline), in quarter notes from bar 1.
    pub at: f64,
    pub length: f64,
    /// Its push: the model's field.
    pub push: PushPull,
    pub accent: bool,
    pub staccato: bool,
}

/// One cue.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedCue {
    pub bar: usize,
    pub index: usize,
    pub text: String,
    /// The beat it is on, from 1.
    pub beat: f64,
    /// Whom it is for (`all`, `keys`, `drums`…); empty for plain text.
    pub group: String,
    pub at: f64,
}

/// One written rest.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedRest {
    pub bar: usize,
    pub index: usize,
    /// The beat it starts on, from 1.
    pub beat: f64,
    pub at: f64,
    pub length: f64,
}

/// One dynamic.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedDynamic {
    pub bar: usize,
    pub index: usize,
    /// As written (`mf`, `ff`).
    pub text: String,
    pub beat: f64,
    pub at: f64,
}

/// What of a chart this schema does not hold yet, counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Untyped {
    /// Bars with written notes (a melody, a voice).
    pub bars_with_notes: usize,
    pub hairpins: usize,
    /// Bars that open or close a repeat, or start an ending.
    pub repeats: usize,
    /// Bars whose chords or rests could not be placed by beat.
    pub bars_unplaced: usize,
}

impl Untyped {
    /// Whether the typed data is the whole chart.
    #[must_use]
    pub const fn is_nothing(&self) -> bool {
        self.bars_with_notes == 0
            && self.hairpins == 0
            && self.repeats == 0
            && self.bars_unplaced == 0
    }
}

/// A chart as typed data.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypedChart {
    pub bars: Vec<TypedBar>,
    pub chords: Vec<TypedChord>,
    pub cues: Vec<TypedCue>,
    pub rests: Vec<TypedRest>,
    pub dynamics: Vec<TypedDynamic>,
}

/// A quarter note's worth of a meter's beat.
fn beat_quarters(meter: (u8, u8)) -> f64 {
    4.0 / f64::from(meter.1.max(1))
}

impl TypedChart {
    /// `chart`, as typed data: what [`import`] writes.
    #[must_use]
    pub fn of(chart: &Chart) -> Self {
        let plays = replays(chart);
        let mut out = Self::default();
        let mut at = 0.0;
        let mut index = 0;
        for section in sections(chart) {
            let name = section.section.display_name();
            for (i, measure) in section.measures().iter().enumerate() {
                let meter = match measure.time_signature {
                    (0, _) | (_, 0) => (4, 4),
                    m => m,
                };
                let unit = beat_quarters(meter);
                let length = f64::from(meter.0) * unit;
                out.bars.push(TypedBar {
                    index,
                    meter,
                    section: name.clone(),
                    section_start: i == 0,
                    plays: plays.get(index).copied().flatten(),
                    repeat_sign: measure.simile,
                    at,
                    length,
                });
                // A bar that plays another again writes no chord of its own.
                let own = plays.get(index).copied().flatten().is_none()
                    && !measure.simile
                    && measure.plays_back.is_none();
                if own && let Some(beats) = beats_of(measure) {
                    let named: Vec<(usize, &keyflow_proto::chart::types::ChordInstance)> = measure
                        .chords
                        .iter()
                        .enumerate()
                        // The parser's silent placeholder before a pushed
                        // first chord is not a chord.
                        .filter(|(_, c)| c.source_span.is_some())
                        .collect();
                    for (k, (i, chord)) in named.iter().enumerate() {
                        let beat = beats[*i];
                        let next = named
                            .get(k + 1)
                            .map_or(1.0 + f64::from(meter.0), |(j, _)| beats[*j]);
                        let marks = chords::parts(&chord.original_token).map_or("", |p| p.0);
                        out.chords.push(TypedChord {
                            bar: index,
                            index: k,
                            symbol: chord.full_symbol.clone(),
                            beat,
                            at: at + (beat - 1.0) * unit,
                            length: (next - beat) * unit,
                            push: chord.push_pull,
                            accent: marks.contains('>'),
                            staccato: marks.contains('.'),
                        });
                    }
                }
                let cues = measure
                    .text_cues
                    .iter()
                    .map(|c| {
                        (
                            c.text.clone(),
                            f64::from(c.beat.unwrap_or(1)),
                            format!("{:?}", c.group).to_lowercase(),
                        )
                    })
                    .chain(
                        measure
                            .staff_text
                            .iter()
                            .map(|t| (t.text.clone(), f64::from(t.beat.max(1)), String::new())),
                    );
                for (k, (text, beat, group)) in cues.enumerate() {
                    out.cues.push(TypedCue {
                        bar: index,
                        index: k,
                        text,
                        beat,
                        group,
                        at: at + (beat - 1.0) * unit,
                    });
                }
                // Its written rests, where its rhythm is read without doubt.
                let rhythm = keyflow_lang::edit::ChordBar::from_measure(measure);
                if own && rhythm.locked.is_none() {
                    let whole = f64::from(keyflow_lang::edit::TICKS_PER_WHOLE);
                    let rests = rhythm
                        .events
                        .iter()
                        .zip(rhythm.starts())
                        .filter(|(e, _)| matches!(e.slot, keyflow_lang::edit::Slot::Rest));
                    for (k, (event, start)) in rests.enumerate() {
                        let from = f64::from(start) * 4.0 / whole;
                        out.rests.push(TypedRest {
                            bar: index,
                            index: k,
                            beat: 1.0 + from / unit,
                            at: at + from,
                            length: f64::from(event.duration.ticks()) * 4.0 / whole,
                        });
                    }
                }
                let dynamics = measure
                    .dynamics
                    .iter()
                    .map(|d| (d.text.clone(), f64::from(d.beat.unwrap_or(1))))
                    .chain(measure.classical_dynamics.iter().map(|d| {
                        (
                            format!("{:?}", d.level).to_lowercase(),
                            f64::from(d.beat.max(1)),
                        )
                    }));
                for (k, (text, beat)) in dynamics.enumerate() {
                    out.dynamics.push(TypedDynamic {
                        bar: index,
                        index: k,
                        text,
                        beat,
                        at: at + (beat - 1.0) * unit,
                    });
                }
                at += length;
                index += 1;
            }
        }
        out
    }

    /// What of `chart` this is not: see the module's list.
    #[must_use]
    pub fn untyped(chart: &Chart) -> Untyped {
        use keyflow_proto::chart::notations::RepeatMark;
        use keyflow_proto::chart::types::RhythmElement;
        let mut out = Untyped::default();
        let plays = replays(chart);
        for (i, measure) in bars(chart).into_iter().enumerate() {
            let own = plays.get(i).copied().flatten().is_none() && !measure.simile;
            if (0..4).any(|v| keyflow_lang::edit::Bar::from_voice(measure, v).written) {
                out.bars_with_notes += 1;
            }
            if own {
                let rests = measure
                    .rhythm_elements
                    .iter()
                    .any(|r| matches!(r, RhythmElement::Rest(_)));
                let unread = keyflow_lang::edit::ChordBar::from_measure(measure)
                    .locked
                    .is_some();
                if (beats_of(measure).is_none() && !measure.chords.is_empty()) || (rests && unread)
                {
                    out.bars_unplaced += 1;
                }
            }
            out.hairpins += measure.hairpins.len();
            if measure.start_repeat != RepeatMark::None
                || measure.end_repeat != RepeatMark::None
                || measure.volta_start.is_some()
            {
                out.repeats += 1;
            }
        }
        out
    }

    /// Every item, by its id, with bar 1 at `origin` quarter notes on the
    /// session's timeline.
    #[must_use]
    pub fn items(&self, origin: f64) -> BTreeMap<ItemId, Item> {
        let owner = ExtensionId::from(crate::EXTENSION);
        let item = |lane: &str, kind: &str, at: f64, end: Option<f64>, payload: Value| Item {
            owner: owner.clone(),
            lane: LaneId::from(lane),
            kind: kind.to_owned(),
            at: Time::QuarterNotes(origin + at),
            end: end.map(|e| Time::QuarterNotes(origin + e)),
            payload,
        };
        let int = |n: usize| Value::Int(i64::try_from(n).unwrap_or(i64::MAX));
        let mut out = BTreeMap::new();
        for b in &self.bars {
            let mut payload = vec![
                ("index", int(b.index)),
                ("numerator", Value::Int(i64::from(b.meter.0))),
                ("denominator", Value::Int(i64::from(b.meter.1))),
                ("section", Value::Text(b.section.clone())),
                ("section_start", Value::Bool(b.section_start)),
                ("repeat_sign", Value::Bool(b.repeat_sign)),
            ];
            if let Some(plays) = b.plays {
                payload.push(("plays", int(plays)));
            }
            out.insert(
                bar_id(b.index),
                item(BARS, BAR, b.at, Some(b.at + b.length), Value::map(payload)),
            );
        }
        for c in &self.chords {
            let mut payload = vec![
                ("symbol", Value::Text(c.symbol.clone())),
                ("bar", int(c.bar)),
                ("beat", Value::Float(c.beat)),
                ("accent", Value::Bool(c.accent)),
                ("staccato", Value::Bool(c.staccato)),
            ];
            payload.extend(push_fields(c.push));
            out.insert(
                chord_id(c.bar, c.index),
                item(
                    CHORDS,
                    CHORD,
                    c.at,
                    Some(c.at + c.length),
                    Value::map(payload),
                ),
            );
        }
        for c in &self.cues {
            out.insert(
                cue_id(c.bar, c.index),
                item(
                    CUES,
                    CUE,
                    c.at,
                    None,
                    Value::map([
                        ("text", Value::Text(c.text.clone())),
                        ("bar", int(c.bar)),
                        ("beat", Value::Float(c.beat)),
                        ("group", Value::Text(c.group.clone())),
                    ]),
                ),
            );
        }
        for r in &self.rests {
            out.insert(
                ItemId(format!("keyflow.rest.{}.{}", r.bar, r.index)),
                item(
                    RHYTHM,
                    REST,
                    r.at,
                    Some(r.at + r.length),
                    Value::map([("bar", int(r.bar)), ("beat", Value::Float(r.beat))]),
                ),
            );
        }
        for d in &self.dynamics {
            out.insert(
                ItemId(format!("keyflow.dynamic.{}.{}", d.bar, d.index)),
                item(
                    DYNAMICS,
                    DYNAMIC,
                    d.at,
                    None,
                    Value::map([
                        ("text", Value::Text(d.text.clone())),
                        ("bar", int(d.bar)),
                        ("beat", Value::Float(d.beat)),
                    ]),
                ),
            );
        }
        out
    }
}

/// Bar `index`'s item id. Ids are made from what they name, so importing
/// the same chart again writes the same items.
#[must_use]
pub fn bar_id(index: usize) -> ItemId {
    ItemId(format!("keyflow.bar.{index}"))
}

/// The id of chord `index` of bar `bar`.
#[must_use]
pub fn chord_id(bar: usize, index: usize) -> ItemId {
    ItemId(format!("keyflow.chord.{bar}.{index}"))
}

/// The id of cue `index` of bar `bar`.
#[must_use]
pub fn cue_id(bar: usize, index: usize) -> ItemId {
    ItemId(format!("keyflow.cue.{bar}.{index}"))
}

/// A push field as payload keys: none when not pushed.
fn push_fields(push: PushPull) -> Vec<(&'static str, Value)> {
    let Some((early, amount)) = push else {
        return Vec::new();
    };
    let mut fields = vec![
        ("push_early", Value::Bool(early)),
        ("push_level", Value::Int(i64::from(amount.level))),
    ];
    match amount.base {
        PushPullBase::Standard => fields.push(("push_base", Value::Text("standard".into()))),
        PushPullBase::Triplet => fields.push(("push_base", Value::Text("triplet".into()))),
        PushPullBase::Tuplet(n) => {
            fields.push(("push_base", Value::Text("tuplet".into())));
            fields.push(("push_tuplet", Value::Int(i64::from(n))));
        }
        PushPullBase::Duration {
            duration,
            dotted,
            triplet,
        } => {
            fields.push(("push_base", Value::Text("value".into())));
            fields.push(("push_value", Value::Int(i64::from(duration.value()))));
            fields.push(("push_dotted", Value::Bool(dotted)));
            fields.push(("push_triplet", Value::Bool(triplet)));
        }
    }
    fields
}

/// The push field a chord's payload holds.
fn push_of(payload: &Value) -> PushPull {
    let early = match payload.get("push_early")? {
        Value::Bool(b) => *b,
        _ => return None,
    };
    let level = u8::try_from(payload.get("push_level")?.as_i64()?).ok()?;
    let flag = |key: &str| matches!(payload.get(key), Some(Value::Bool(true)));
    let base = match payload.get("push_base")?.as_str()? {
        "standard" => PushPullBase::Standard,
        "triplet" => PushPullBase::Triplet,
        "tuplet" => PushPullBase::Tuplet(u8::try_from(payload.get("push_tuplet")?.as_i64()?).ok()?),
        "value" => PushPullBase::Duration {
            duration: LilySyntax::from_number(&payload.get("push_value")?.as_i64()?.to_string())?,
            dotted: flag("push_dotted"),
            triplet: flag("push_triplet"),
        },
        _ => return None,
    };
    Some((early, PushPullAmount { level, base }))
}

/// Where the song's bar 1 is on `doc`'s timeline, in quarter notes.
#[must_use]
pub fn origin(doc: &SessionDoc) -> f64 {
    doc.bar_one()
        .map_or(0.0, |at| at.quarter_notes(&doc.tempo()))
}

/// `chart` written into `doc` as typed data: Keyflow's three lanes, and
/// its items made exactly `chart`'s (an item of Keyflow's on them that
/// `chart` has no more is removed; another extension's is left). Importing
/// the same chart again changes nothing.
///
/// # Errors
///
/// The document refused a write.
pub fn import(doc: &SessionDoc, chart: &Chart) -> DocResult<TypedChart> {
    let typed = TypedChart::of(chart);
    let owner = ExtensionId::from(crate::EXTENSION);
    for (order, (id, name)) in LANES.into_iter().enumerate() {
        let lane = Lane {
            owner: owner.clone(),
            kind: id.to_owned(),
            name: name.to_owned(),
            order: order as f64,
            parent: None,
        };
        if doc.lane(&LaneId::from(id)).as_ref() != Some(&lane) {
            doc.put_lane(&LaneId::from(id), &lane)?;
        }
    }
    let wanted = typed.items(origin(doc));
    let have = doc.items();
    for (id, item) in &wanted {
        if have.get(id) != Some(item) {
            doc.put_item(id, item)?;
        }
    }
    let lanes: BTreeSet<&str> = LANES.iter().map(|l| l.0).collect();
    for (id, item) in &have {
        if item.owner == owner && lanes.contains(item.lane.as_str()) && !wanted.contains_key(id) {
            doc.remove_item(id)?;
        }
    }
    doc.ext(&owner)?.set(SCHEMA_ENTRY, &Value::Int(SCHEMA))?;
    Ok(typed)
}

/// The typed chart `doc` holds. Empty when it holds none.
#[must_use]
pub fn read(doc: &SessionDoc) -> TypedChart {
    let from = origin(doc);
    let owner = ExtensionId::from(crate::EXTENSION);
    let tempo = doc.tempo();
    let mut out = TypedChart::default();
    let number = |payload: &Value, key: &str| {
        payload
            .get(key)
            .and_then(Value::as_i64)
            .and_then(|n| usize::try_from(n).ok())
    };
    let text = |payload: &Value, key: &str| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let flag = |payload: &Value, key: &str| matches!(payload.get(key), Some(Value::Bool(true)));
    for (id, item) in doc.items() {
        if item.owner != owner {
            continue;
        }
        let at = item.at.quarter_notes(&tempo) - from;
        let length = item
            .end
            .map_or(0.0, |e| e.quarter_notes(&tempo) - from - at);
        let p = &item.payload;
        // An item's place among its bar's, from its id's last part.
        let place = id
            .as_str()
            .rsplit('.')
            .next()
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0);
        match item.kind.as_str() {
            BAR => {
                let (Some(index), Some(n), Some(d)) = (
                    number(p, "index"),
                    number(p, "numerator"),
                    number(p, "denominator"),
                ) else {
                    continue;
                };
                out.bars.push(TypedBar {
                    index,
                    meter: (n as u8, d as u8),
                    section: text(p, "section"),
                    section_start: flag(p, "section_start"),
                    plays: number(p, "plays"),
                    repeat_sign: flag(p, "repeat_sign"),
                    at,
                    length,
                });
            }
            CHORD => {
                let (Some(bar), Some(beat)) =
                    (number(p, "bar"), p.get("beat").and_then(Value::as_f64))
                else {
                    continue;
                };
                out.chords.push(TypedChord {
                    bar,
                    index: place,
                    symbol: text(p, "symbol"),
                    beat,
                    at,
                    length,
                    push: push_of(p),
                    accent: flag(p, "accent"),
                    staccato: flag(p, "staccato"),
                });
            }
            CUE => {
                let (Some(bar), Some(beat)) =
                    (number(p, "bar"), p.get("beat").and_then(Value::as_f64))
                else {
                    continue;
                };
                out.cues.push(TypedCue {
                    bar,
                    index: place,
                    text: text(p, "text"),
                    beat,
                    group: text(p, "group"),
                    at,
                });
            }
            REST => {
                let (Some(bar), Some(beat)) =
                    (number(p, "bar"), p.get("beat").and_then(Value::as_f64))
                else {
                    continue;
                };
                out.rests.push(TypedRest {
                    bar,
                    index: place,
                    beat,
                    at,
                    length,
                });
            }
            DYNAMIC => {
                let (Some(bar), Some(beat)) =
                    (number(p, "bar"), p.get("beat").and_then(Value::as_f64))
                else {
                    continue;
                };
                out.dynamics.push(TypedDynamic {
                    bar,
                    index: place,
                    text: text(p, "text"),
                    beat,
                    at,
                });
            }
            _ => {}
        }
    }
    out.rests.sort_by_key(|r| (r.bar, r.index));
    out.dynamics.sort_by_key(|d| (d.bar, d.index));
    out.bars.sort_by_key(|b| b.index);
    out.chords.sort_by_key(|c| (c.bar, c.index));
    out.cues.sort_by_key(|c| (c.bar, c.index));
    out
}

/// The push of chord `id` made `push`: one write to its item's own push
/// fields, the rest of it (another person's accent, made meanwhile) left
/// as it is. `Ok(false)` when there is no such chord.
///
/// # Errors
///
/// The document refused the write.
pub fn set_push(doc: &SessionDoc, id: &ItemId, push: PushPull) -> DocResult<bool> {
    let Some(mut item) = doc.item(id).filter(|i| i.kind == CHORD) else {
        return Ok(false);
    };
    let Value::Map(mut payload) = item.payload else {
        return Ok(false);
    };
    payload.retain(|key, _| !key.starts_with("push_"));
    for (key, value) in push_fields(push) {
        payload.insert(key.to_owned(), value);
    }
    item.payload = Value::Map(payload);
    doc.put_item(id, &item)?;
    Ok(true)
}

/// A flag of chord `id` (`accent`, `staccato`) set: one write to that
/// field. `Ok(false)` when there is no such chord.
///
/// # Errors
///
/// The document refused the write.
pub fn set_flag(doc: &SessionDoc, id: &ItemId, flag: &str, on: bool) -> DocResult<bool> {
    let Some(mut item) = doc.item(id).filter(|i| i.kind == CHORD) else {
        return Ok(false);
    };
    let Value::Map(payload) = &mut item.payload else {
        return Ok(false);
    };
    payload.insert(flag.to_owned(), Value::Bool(on));
    doc.put_item(id, &item)?;
    Ok(true)
}
