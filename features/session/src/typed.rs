//! The chart as typed data in the session document.
//!
//! A session's chart has been one text entry holding Keyflow source
//! ([`crate::CHART`]). This is the chart as the session's own data
//! instead, so that the chart and the session cannot disagree: what they
//! share is stored once, in the session.
//!
//! # One structure, the session's
//!
//! - **Bars are the session's bars.** The timeline is linear: a bar is an
//!   item for each time it is *played*, at its place from the session's
//!   bar 1 ([`session::Session::bar_one`]). A repeat's second pass is the
//!   next bars along, with the next bar numbers; each bar item says which
//!   written bar it plays and on which pass, and carries that bar's repeat
//!   marks (`|:`, `:|`, its count, its ending), so the chart can still be
//!   drawn as written. Bar numbers are the session's: bar `n` here is bar
//!   `n` there.
//! - **Sections are the session's regions.** A bar item holds no section:
//!   its section is the region it lies in. Importing a chart writes its
//!   sections *as* regions; a region renamed anywhere is the chart's
//!   section renamed.
//! - **Meters and tempo are the session's tempo map.** A bar item holds
//!   no meter: its meter is the map's at its time. Importing a chart makes
//!   the map, from bar 1 on, the chart's.
//!
//! # The chart's own data (schema version [`SCHEMA`])
//!
//! Five lanes, owner `keyflow`:
//!
//! | lane | item kind | at … end | payload |
//! |---|---|---|---|
//! | `keyflow.bars` | `keyflow.bar` | the bar's span | `index` (its bar number less one), `written` (the bar it plays), `pass`, `repeat_start`, `repeat_end`, `repeat_count`, `ending`, `repeat_sign` |
//! | `keyflow.chords` | `keyflow.chord` | the beat it is written on … the next chord or the barline | `symbol`, `bar`, `beat`, `written_bar`, `written_index`, `accent`, `staccato`, and its push (`push_early`, `push_level`, `push_base`, …; absent when not pushed) |
//! | `keyflow.rhythm` | `keyflow.rest` | the rest's span | `bar`, `beat` |
//! | `keyflow.cues` | `keyflow.cue` | its bar and beat | `text`, `bar`, `beat`, `group` |
//! | `keyflow.dynamics` | `keyflow.dynamic` | its bar and beat | `text` (`mf`), `bar`, `beat` |
//!
//! A chord's **push** is its own fields, the model's
//! (`ChordInstance::push_pull`). A chord in a repeated bar is one written
//! chord played more than once: [`push_requests`] writes the push to each
//! time it is played.
//!
//! # What is typed, and what is not yet
//!
//! Typed: every bar as played with its repeat marks, every chord with its
//! beat, length, push, accent and staccato, every written rest, every
//! dynamic, every text and instrument cue; the sections, meters and tempo
//! in the session's own structure. **Not yet typed** ([`Untyped`] counts
//! what it can for a chart): written melodies and notes, ties, hairpins,
//! lyric lanes, fermatas and stops, key changes, and the header (title,
//! key). Until those are, the text entry stays beside this and is what
//! the chart is engraved from.

use std::collections::{BTreeMap, BTreeSet};

use keyflow_lang::edit::select::{PushPull, beats_of};
use keyflow_lang::edit::{bars, chords, replays};
use keyflow_lang::text::bar_times::{BarClock, bar_times};
use keyflow_proto::Chart;
use keyflow_proto::chart::notations::RepeatMark;
use keyflow_proto::chord::{LilySyntax, PushPullAmount, PushPullBase};
use session::doc::{DocResult, SessionDoc};
use session::extension::{call_json, response_of};
use session::service::{Meter, READ, Request, Response, TempoChange, TempoMapData, action_for};
use session::{ExtensionId, Item, ItemId, Lane, LaneId, MarkId, Region, TempoMap, Time, Value};

use crate::{ChartSource, Error, ServiceCall};

/// This schema's version, kept in Keyflow's namespace as register
/// [`SCHEMA_ENTRY`]: Keyflow migrates its own data.
pub const SCHEMA: i64 = 2;
/// The register the schema version is kept in.
pub const SCHEMA_ENTRY: &str = "chart_schema";
/// The register that says the session's tempo map from bar 1 was its own
/// when the chart was typed, so the chart is read against it and never
/// writes it.
pub const TEMPO_KEPT: &str = "chart_keeps_session_tempo";

pub const BARS: &str = "keyflow.bars";
pub const CHORDS: &str = "keyflow.chords";
pub const CUES: &str = "keyflow.cues";
pub const RHYTHM: &str = "keyflow.rhythm";
pub const DYNAMICS: &str = "keyflow.dynamics";
pub const BAR: &str = "keyflow.bar";
pub const CHORD: &str = "keyflow.chord";
pub const CUE: &str = "keyflow.cue";
pub const REST: &str = "keyflow.rest";
pub const DYNAMIC: &str = "keyflow.dynamic";
/// The regions an import writes are named `keyflow.section.<n>`: a section
/// the chart brought. A region made elsewhere is a section just the same.
pub const SECTION: &str = "keyflow.section.";

/// Keyflow's lanes, in order, with their names.
const LANES: [(&str, &str); 5] = [
    (BARS, "Bars"),
    (CHORDS, "Chords"),
    (RHYTHM, "Rhythm"),
    (CUES, "Cues"),
    (DYNAMICS, "Dynamics"),
];

/// One bar as it is played.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedBar {
    /// Its place on the timeline, from 0: its bar number less one.
    pub index: usize,
    /// The written bar it plays (the chart's bars as drawn, from 0).
    pub written: usize,
    /// Which time through a repeat this is (1 outside repeats).
    pub pass: u32,
    /// Its meter: the session's, at its time.
    pub meter: (u8, u8),
    /// Its section's name: the region's it lies in (empty in none).
    pub section: String,
    /// Whether it is the first bar played of its section.
    pub section_start: bool,
    /// The written bar opens a repeat (`|:`), closes one (`:|`) and how
    /// many times that plays, and the passes its ending is for (`[1]`).
    pub repeat_start: bool,
    pub repeat_end: bool,
    pub repeat_count: u32,
    pub ending: Vec<u8>,
    /// Whether it is drawn as a repeat-bar sign.
    pub repeat_sign: bool,
    /// Where it starts and how long it is, in quarter notes from bar 1.
    pub at: f64,
    pub length: f64,
}

/// One chord, a time it is played.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedChord {
    /// The bar it is played in ([`TypedBar::index`]).
    pub bar: usize,
    /// Its place among that bar's chords, from 0.
    pub index: usize,
    /// The written bar it is a chord of: the same for every pass.
    pub written_bar: usize,
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
    /// Bars whose chords or rests could not be placed by beat.
    pub bars_unplaced: usize,
}

impl Untyped {
    /// Whether the typed data is the whole chart.
    #[must_use]
    pub const fn is_nothing(&self) -> bool {
        self.bars_with_notes == 0 && self.hairpins == 0 && self.bars_unplaced == 0
    }
}

/// A chart as typed data: its bars as played, and what is in them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypedChart {
    pub bars: Vec<TypedBar>,
    pub chords: Vec<TypedChord>,
    pub cues: Vec<TypedCue>,
    pub rests: Vec<TypedRest>,
    pub dynamics: Vec<TypedDynamic>,
}

/// A tempo or a meter from a time on: what an import makes the session's
/// tempo map hold from bar 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoAt {
    /// Seconds from bar 1.
    pub seconds: f64,
    pub bpm: f64,
    pub meter: (u8, u8),
}

/// A quarter note's worth of a meter's beat.
fn beat_quarters(meter: (u8, u8)) -> f64 {
    4.0 / f64::from(meter.1.max(1))
}

/// A meter, with a missing one read as 4/4.
fn meter_of(time: (u8, u8)) -> (u8, u8) {
    match time {
        (0, _) | (_, 0) => (4, 4),
        m => m,
    }
}

/// What one written bar holds, in quarter notes from its barline.
#[derive(Default)]
struct Written {
    /// `(symbol, beat, from, length, push, accent, staccato)`.
    chords: Vec<(String, f64, f64, f64, PushPull, bool, bool)>,
    /// `(text, beat, from, group)`.
    cues: Vec<(String, f64, f64, String)>,
    /// `(beat, from, length)`.
    rests: Vec<(f64, f64, f64)>,
    /// `(text, beat, from)`.
    dynamics: Vec<(String, f64, f64)>,
}

impl TypedChart {
    /// `chart`, as typed data: its bars in the order they are played.
    #[must_use]
    pub fn of(chart: &Chart) -> Self {
        let measures = bars(chart);
        let plays = replays(chart);
        // The bar whose own music a bar plays: a recalled section's bar,
        // a line's copy of itself, a `%`.
        let source = |bar: usize| {
            let mut at = plays.get(bar).copied().flatten().unwrap_or(bar);
            for _ in 0..measures.len() {
                let Some(m) = measures.get(at) else { break };
                match m.plays_back {
                    Some(n) if n > 0 && n <= at => at -= n,
                    _ if m.simile && at > 0 => at -= 1,
                    _ => break,
                }
            }
            at
        };
        let written: Vec<Written> = measures.iter().map(|m| written_of(m)).collect();

        let mut out = Self::default();
        let mut at = 0.0;
        let mut section = None;
        for (index, played) in bar_times(chart, BarClock::at(0.0)).into_iter().enumerate() {
            let Some(measure) = measures.get(played.bar) else {
                continue;
            };
            let meter = meter_of(measure.time_signature);
            let unit = beat_quarters(meter);
            let length = f64::from(meter.0) * unit;
            let section_start = section != Some(played.section);
            section = Some(played.section);
            out.bars.push(TypedBar {
                index,
                written: played.bar,
                pass: played.pass,
                meter,
                section: chart
                    .sections
                    .get(played.section)
                    .map(|s| s.section.display_name())
                    .unwrap_or_default(),
                section_start,
                repeat_start: measure.start_repeat == RepeatMark::Forward,
                repeat_end: measure.end_repeat == RepeatMark::Backward,
                repeat_count: if measure.end_repeat == RepeatMark::Backward {
                    u32::try_from(measure.repeat_count.max(2)).unwrap_or(2)
                } else {
                    0
                },
                ending: measure
                    .volta_start
                    .as_ref()
                    .map(|v| v.numbers.clone())
                    .unwrap_or_default(),
                repeat_sign: measure.simile,
                at,
                length,
            });
            // The music it plays is its source bar's; what is marked on
            // the bar itself (a cue, a dynamic) is its own.
            let (music, own) = (&written[source(played.bar)], &written[played.bar]);
            for (k, (symbol, beat, from, len, push, accent, staccato)) in
                music.chords.iter().enumerate()
            {
                out.chords.push(TypedChord {
                    bar: index,
                    index: k,
                    written_bar: source(played.bar),
                    symbol: symbol.clone(),
                    beat: *beat,
                    at: at + from,
                    length: *len,
                    push: *push,
                    accent: *accent,
                    staccato: *staccato,
                });
            }
            for (k, (beat, from, len)) in music.rests.iter().enumerate() {
                out.rests.push(TypedRest {
                    bar: index,
                    index: k,
                    beat: *beat,
                    at: at + from,
                    length: *len,
                });
            }
            for (k, (text, beat, from, group)) in own.cues.iter().enumerate() {
                out.cues.push(TypedCue {
                    bar: index,
                    index: k,
                    text: text.clone(),
                    beat: *beat,
                    group: group.clone(),
                    at: at + from,
                });
            }
            for (k, (text, beat, from)) in own.dynamics.iter().enumerate() {
                out.dynamics.push(TypedDynamic {
                    bar: index,
                    index: k,
                    text: text.clone(),
                    beat: *beat,
                    at: at + from,
                });
            }
            at += length;
        }
        out
    }

    /// `chart`'s tempo and meters as played, from bar 1: a change wherever
    /// either moves from one bar to the next. (A tempo change inside a bar
    /// is that bar's average here.)
    #[must_use]
    pub fn tempo_of(chart: &Chart) -> Vec<TempoAt> {
        let measures = bars(chart);
        let mut out: Vec<TempoAt> = Vec::new();
        for played in bar_times(chart, BarClock::at(0.0)) {
            let Some(measure) = measures.get(played.bar) else {
                continue;
            };
            let meter = meter_of(measure.time_signature);
            let quarters = f64::from(meter.0) * beat_quarters(meter);
            let span = played.end - played.start;
            if span <= 0.0 {
                continue;
            }
            let bpm = (quarters * 60.0 / span * 1.0e6).round() / 1.0e6;
            if out
                .last()
                .is_none_or(|last| last.meter != meter || (last.bpm - bpm).abs() > 1e-6)
            {
                out.push(TempoAt {
                    seconds: played.start,
                    bpm,
                    meter,
                });
            }
        }
        out
    }

    /// What of `chart` this is not: see the module's list.
    #[must_use]
    pub fn untyped(chart: &Chart) -> Untyped {
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
        }
        out
    }

    /// The sections as played: each one's name and its span in quarter
    /// notes from bar 1.
    #[must_use]
    pub fn sections(&self) -> Vec<(String, f64, f64)> {
        let mut out: Vec<(String, f64, f64)> = Vec::new();
        for bar in &self.bars {
            match out.last_mut() {
                Some(last) if !bar.section_start => last.2 = bar.at + bar.length,
                _ => out.push((bar.section.clone(), bar.at, bar.at + bar.length)),
            }
        }
        out
    }

    /// Every item, by its id, with bar 1 at `origin` quarter notes on the
    /// session's timeline: ids minted from position ([`bar_id`],
    /// [`chord_id`]), what a first import writes.
    #[must_use]
    pub fn items(&self, origin: f64) -> BTreeMap<ItemId, Item> {
        self.items_with(origin, &Ids::positional(self))
    }

    /// Every item, by the id `ids` gives it.
    fn items_with(&self, origin: f64, ids: &Ids) -> BTreeMap<ItemId, Item> {
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
        for (b, id) in self.bars.iter().zip(&ids.bars) {
            let payload = vec![
                ("index", int(b.index)),
                ("written", int(b.written)),
                ("pass", Value::Int(i64::from(b.pass))),
                ("repeat_start", Value::Bool(b.repeat_start)),
                ("repeat_end", Value::Bool(b.repeat_end)),
                ("repeat_count", Value::Int(i64::from(b.repeat_count))),
                (
                    "ending",
                    Value::List(b.ending.iter().map(|n| Value::Int(i64::from(*n))).collect()),
                ),
                ("repeat_sign", Value::Bool(b.repeat_sign)),
            ];
            out.insert(
                id.clone(),
                item(BARS, BAR, b.at, Some(b.at + b.length), Value::map(payload)),
            );
        }
        for (c, id) in self.chords.iter().zip(&ids.chords) {
            let mut payload = vec![
                ("symbol", Value::Text(c.symbol.clone())),
                ("bar", int(c.bar)),
                ("beat", Value::Float(c.beat)),
                ("written_bar", int(c.written_bar)),
                ("written_index", int(c.index)),
                ("accent", Value::Bool(c.accent)),
                ("staccato", Value::Bool(c.staccato)),
            ];
            payload.extend(push_fields(c.push));
            out.insert(
                id.clone(),
                item(
                    CHORDS,
                    CHORD,
                    c.at,
                    Some(c.at + c.length),
                    Value::map(payload),
                ),
            );
        }
        for (c, id) in self.cues.iter().zip(&ids.cues) {
            out.insert(
                id.clone(),
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
        for (r, id) in self.rests.iter().zip(&ids.rests) {
            out.insert(
                id.clone(),
                item(
                    RHYTHM,
                    REST,
                    r.at,
                    Some(r.at + r.length),
                    Value::map([("bar", int(r.bar)), ("beat", Value::Float(r.beat))]),
                ),
            );
        }
        for (d, id) in self.dynamics.iter().zip(&ids.dynamics) {
            out.insert(
                id.clone(),
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

/// What `measure` writes of its own.
fn written_of(measure: &keyflow_proto::chart::Measure) -> Written {
    let meter = meter_of(measure.time_signature);
    let unit = beat_quarters(meter);
    let mut out = Written::default();
    let copy = measure.simile || measure.replays.is_some() || measure.plays_back.is_some();
    if !copy && let Some(beats) = beats_of(measure) {
        // The parser's silent placeholder before a pushed first chord is
        // not a chord.
        let named: Vec<(usize, &keyflow_proto::chart::types::ChordInstance)> = measure
            .chords
            .iter()
            .enumerate()
            .filter(|(_, c)| c.source_span.is_some())
            .collect();
        for (k, (i, chord)) in named.iter().enumerate() {
            let beat = beats[*i];
            let next = named
                .get(k + 1)
                .map_or(1.0 + f64::from(meter.0), |(j, _)| beats[*j]);
            let marks = chords::parts(&chord.original_token).map_or("", |p| p.0);
            out.chords.push((
                chord.full_symbol.clone(),
                beat,
                (beat - 1.0) * unit,
                (next - beat) * unit,
                chord.push_pull,
                marks.contains('>'),
                marks.contains('.'),
            ));
        }
    }
    // Its written rests, where its rhythm is read without doubt.
    let rhythm = keyflow_lang::edit::ChordBar::from_measure(measure);
    if !copy && rhythm.locked.is_none() {
        let whole = f64::from(keyflow_lang::edit::TICKS_PER_WHOLE);
        for (event, start) in rhythm.events.iter().zip(rhythm.starts()) {
            if matches!(event.slot, keyflow_lang::edit::Slot::Rest) {
                let from = f64::from(start) * 4.0 / whole;
                out.rests.push((
                    1.0 + from / unit,
                    from,
                    f64::from(event.duration.ticks()) * 4.0 / whole,
                ));
            }
        }
    }
    // A chord lasts to the next chord or the bar's end unless a rest comes
    // first: `>C#m9_8 r8 r2.` is an eighth, not the whole bar with three
    // beats of rest after it.
    for chord in &mut out.chords {
        let (from, length) = (chord.2, chord.3);
        if let Some(rest) = out
            .rests
            .iter()
            .map(|r| r.1)
            .filter(|r| *r > from + 1e-9 && *r < from + length - 1e-9)
            .min_by(f64::total_cmp)
        {
            chord.3 = rest - from;
        }
    }
    for c in &measure.text_cues {
        let beat = f64::from(c.beat.unwrap_or(1));
        out.cues.push((
            c.text.clone(),
            beat,
            (beat - 1.0) * unit,
            format!("{:?}", c.group).to_lowercase(),
        ));
    }
    for t in &measure.staff_text {
        let beat = f64::from(t.beat.max(1));
        out.cues
            .push((t.text.clone(), beat, (beat - 1.0) * unit, String::new()));
    }
    for d in &measure.dynamics {
        let beat = f64::from(d.beat.unwrap_or(1));
        out.dynamics
            .push((d.text.clone(), beat, (beat - 1.0) * unit));
    }
    for d in &measure.classical_dynamics {
        let beat = f64::from(d.beat.max(1));
        out.dynamics.push((
            format!("{:?}", d.level).to_lowercase(),
            beat,
            (beat - 1.0) * unit,
        ));
    }
    // In time order, as a session reads them back (a bar's text cues are
    // parsed before its staff text, whatever their beats).
    out.cues.sort_by(|a, b| a.2.total_cmp(&b.2));
    out.dynamics.sort_by(|a, b| a.2.total_cmp(&b.2));
    out
}

/// Bar `index`'s item id. Ids are made from what they name, so importing
/// the same chart again writes the same items.
#[must_use]
pub fn bar_id(index: usize) -> ItemId {
    ItemId(format!("keyflow.bar.{index}"))
}

/// The id of chord `index` of played bar `bar`.
#[must_use]
pub fn chord_id(bar: usize, index: usize) -> ItemId {
    ItemId(format!("keyflow.chord.{bar}.{index}"))
}

/// The id of cue, rest or dynamic `index` of played bar `bar`.
fn placed_id(kind: &str, bar: usize, index: usize) -> ItemId {
    ItemId(format!("keyflow.{kind}.{bar}.{index}"))
}

/// An id for each of a [`TypedChart`]'s bars and items, in its order.
///
/// # Identity
///
/// An item's id is minted once, when it first appears, and kept: it is
/// not its place. An edit comes to [`plan`] as a whole chart, parsed, with
/// no ids in it, so [`Held::ids_for`] finds each of its bars and items in
/// what the session holds (an alignment: the longest run of bars alike,
/// then within each bar the items alike, a changed one paired with the one
/// it replaces) and gives it that one's id. Inserting a bar mints one id
/// and moves the bars after it; a chord pushed by someone else meanwhile
/// is still that chord. A first import mints ids from position, so two
/// replicas importing one chart make the same items.
#[derive(Debug, Clone, Default, PartialEq)]
struct Ids {
    bars: Vec<ItemId>,
    chords: Vec<ItemId>,
    cues: Vec<ItemId>,
    rests: Vec<ItemId>,
    dynamics: Vec<ItemId>,
}

impl Ids {
    /// Ids from position: what a chart nobody has typed yet is given.
    fn positional(chart: &TypedChart) -> Self {
        Self {
            bars: chart.bars.iter().map(|b| bar_id(b.index)).collect(),
            chords: chart
                .chords
                .iter()
                .map(|c| chord_id(c.bar, c.index))
                .collect(),
            cues: chart
                .cues
                .iter()
                .map(|c| placed_id("cue", c.bar, c.index))
                .collect(),
            rests: chart
                .rests
                .iter()
                .map(|r| placed_id("rest", r.bar, r.index))
                .collect(),
            dynamics: chart
                .dynamics
                .iter()
                .map(|d| placed_id("dynamic", d.bar, d.index))
                .collect(),
        }
    }
}

/// For each of `new`, the one of `old` it is: the longest common
/// subsequence of keys, then each gap between two matches whose two sides
/// are as long paired in order (a bar or a chord changed in place).
/// Each of `old` is matched at most once.
fn align<K: PartialEq>(old: &[K], new: &[K]) -> Vec<Option<usize>> {
    let (n, m) = (old.len(), new.len());
    // lcs[i][j]: the longest common subsequence of old[i..] and new[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i] == new[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut out = vec![None; m];
    let (mut i, mut j) = (0, 0);
    // Where the gap being walked started, on each side.
    let (mut gap_old, mut gap_new) = (0, 0);
    let close_gap = |out: &mut Vec<Option<usize>>, i: usize, j: usize, go: usize, gn: usize| {
        if i - go == j - gn {
            for k in 0..(j - gn) {
                out[gn + k] = Some(go + k);
            }
        }
    };
    while i < n && j < m {
        if old[i] == new[j] {
            close_gap(&mut out, i, j, gap_old, gap_new);
            out[j] = Some(i);
            i += 1;
            j += 1;
            (gap_old, gap_new) = (i, j);
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    close_gap(&mut out, n, m, gap_old, gap_new);
    out
}

/// What a bar is, for matching one chart's bars with another's: what is
/// in it and its marks, not where it is.
fn bar_keys(chart: &TypedChart) -> Vec<String> {
    let mut keys: BTreeMap<usize, String> = chart
        .bars
        .iter()
        .map(|b| {
            (
                b.index,
                format!(
                    "{}/{} p{} {}{}{} {:?} {}|",
                    b.meter.0,
                    b.meter.1,
                    b.pass,
                    u8::from(b.repeat_start),
                    u8::from(b.repeat_end),
                    b.repeat_count,
                    b.ending,
                    u8::from(b.repeat_sign),
                ),
            )
        })
        .collect();
    let mut add = |bar: usize, part: String| {
        if let Some(key) = keys.get_mut(&bar) {
            key.push_str(&part);
            key.push('|');
        }
    };
    for c in &chart.chords {
        add(c.bar, chord_key(c));
    }
    for r in &chart.rests {
        add(r.bar, format!("r{}:{}", r.beat, r.length));
    }
    for c in &chart.cues {
        add(c.bar, format!("q{}:{}:{}", c.beat, c.group, c.text));
    }
    for d in &chart.dynamics {
        add(d.bar, format!("d{}:{}", d.beat, d.text));
    }
    chart.bars.iter().map(|b| keys[&b.index].clone()).collect()
}

fn chord_key(c: &TypedChord) -> String {
    format!(
        "c{}:{}:{:?}:{}{}",
        c.beat,
        c.symbol,
        c.push,
        u8::from(c.accent),
        u8::from(c.staccato)
    )
}

/// Ids not given yet: a wanted one if it is free, else the same with
/// `~2`, `~3` …, so two replicas making the same edit mint the same ids.
struct Mint {
    taken: BTreeSet<String>,
}

impl Mint {
    fn id(&mut self, wanted: &str) -> String {
        let mut id = wanted.to_owned();
        let mut n = 2;
        while self.taken.contains(&id) {
            id = format!("{wanted}~{n}");
            n += 1;
        }
        self.taken.insert(id.clone());
        id
    }
}

/// The ids of `new`'s items of one kind: each found in `old` (by
/// [`align`], within the bar its bar was matched with) keeps that one's
/// id; the rest are minted from `positional`.
fn child_ids<T>(
    old: &[T],
    old_ids: &[ItemId],
    new: &[T],
    bar_of: impl Fn(&T) -> usize,
    key: impl Fn(&T) -> String,
    positional: impl Fn(&T) -> ItemId,
    old_bar: &BTreeMap<usize, usize>,
    mint: &mut Mint,
) -> Vec<ItemId> {
    let mut by_bar: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, item) in old.iter().enumerate() {
        by_bar.entry(bar_of(item)).or_default().push(k);
    }
    let mut out: Vec<Option<ItemId>> = vec![None; new.len()];
    let mut new_by_bar: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, item) in new.iter().enumerate() {
        new_by_bar.entry(bar_of(item)).or_default().push(k);
    }
    for (bar, mine) in &new_by_bar {
        let theirs = old_bar
            .get(bar)
            .and_then(|b| by_bar.get(b))
            .cloned()
            .unwrap_or_default();
        let old_keys: Vec<String> = theirs.iter().map(|&k| key(&old[k])).collect();
        let new_keys: Vec<String> = mine.iter().map(|&k| key(&new[k])).collect();
        for (n, found) in align(&old_keys, &new_keys).into_iter().enumerate() {
            if let Some(o) = found {
                out[mine[n]] = Some(old_ids[theirs[o]].clone());
            }
        }
    }
    out.into_iter()
        .zip(new)
        .map(|(id, item)| id.unwrap_or_else(|| ItemId(mint.id(positional(item).as_str()))))
        .collect()
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

/// What a session holds that a chart is written into and read from: its
/// timeline and its structure.
#[derive(Debug, Clone)]
pub struct Held {
    pub lanes: BTreeMap<LaneId, Lane>,
    pub items: BTreeMap<ItemId, Item>,
    pub regions: BTreeMap<MarkId, Region>,
    /// The session's markers: drawn above their bars ([`Held::road_map`]).
    pub markers: BTreeMap<MarkId, session::Marker>,
    pub tempo: TempoMap,
    /// The song's bar 1, in quarter notes on the session's timeline.
    pub origin: f64,
    /// Whether the tempo map from bar 1 was the session's own when its
    /// chart was typed, and so is kept (`None` read as not typed yet).
    tempo_kept: bool,
}

impl Held {
    /// What `doc` holds.
    #[must_use]
    pub fn of_doc(doc: &SessionDoc) -> Self {
        let tempo = doc.tempo();
        Self {
            lanes: doc.lanes(),
            items: doc.items(),
            regions: doc.regions(),
            markers: doc.markers(),
            origin: doc.bar_one().map_or(0.0, |at| at.quarter_notes(&tempo)),
            tempo,
            tempo_kept: doc
                .read_ext(&ExtensionId::from(crate::EXTENSION))
                .get(TEMPO_KEPT)
                == Some(&session::Entry::Register(Value::Bool(true))),
        }
    }

    /// With what Keyflow's namespace says: whether the session's tempo
    /// map is kept.
    fn with_data(mut self, data: &session::ExtensionData) -> Self {
        self.tempo_kept =
            data.get(TEMPO_KEPT) == Some(&session::Entry::Register(Value::Bool(true)));
        self
    }

    /// From a provider's replies to [`ChartSource::timeline_calls`], in
    /// order.
    ///
    /// # Errors
    ///
    /// A reply is not what was asked for.
    pub fn of_replies(timeline: &str, structure: &str, data: &str) -> crate::Result<Self> {
        let Response::Data(data) = response_of(data).map_err(Error::Service)? else {
            return Err(Error::Service("not Keyflow's data".to_owned()));
        };
        let Response::Timeline { lanes, items } = response_of(timeline).map_err(Error::Service)?
        else {
            return Err(Error::Service("not the timeline".to_owned()));
        };
        let Response::Structure {
            tempo,
            regions,
            markers,
            bar_one,
        } = response_of(structure).map_err(Error::Service)?
        else {
            return Err(Error::Service("not the structure".to_owned()));
        };
        let tempo = tempo
            .to_tempo_map()
            .map_err(|e| Error::Service(e.to_string()))?;
        Ok(Self {
            origin: bar_one.map_or(0.0, |at| at.quarter_notes(&tempo)),
            lanes,
            items,
            regions,
            markers,
            tempo,
            tempo_kept: false,
        }
        .with_data(&data))
    }

    /// Bar 1, in seconds.
    fn origin_seconds(&self) -> f64 {
        Time::QuarterNotes(self.origin).seconds(&self.tempo)
    }

    /// Whether a chart has been typed into it.
    #[must_use]
    pub fn is_typed(&self) -> bool {
        self.lanes.contains_key(&LaneId::from(BARS))
    }

    /// The typed chart it holds: Keyflow's items, each bar's meter from
    /// the tempo map and its section from the region it lies in.
    #[must_use]
    pub fn chart(&self) -> TypedChart {
        self.chart_and_ids().0
    }

    /// [`Held::chart`], with each bar's and item's id.
    fn chart_and_ids(&self) -> (TypedChart, Ids) {
        let owner = ExtensionId::from(crate::EXTENSION);
        let mut out = TypedChart::default();
        let mut ids = Ids::default();
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
        // The regions as sections: where each starts and ends.
        let mut regions: Vec<(f64, f64, &str)> = self
            .regions
            .values()
            .map(|r| {
                (
                    r.start.quarter_notes(&self.tempo) - self.origin,
                    r.end.quarter_notes(&self.tempo) - self.origin,
                    r.name.as_str(),
                )
            })
            .collect();
        regions.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (id, item) in &self.items {
            if item.owner != owner {
                continue;
            }
            let at = item.at.quarter_notes(&self.tempo) - self.origin;
            let length = item
                .end
                .map_or(0.0, |e| e.quarter_notes(&self.tempo) - self.origin - at);
            let p = &item.payload;
            // Its place among its bar's is from its time (below), not its id.
            let place = 0;
            let placed = || Some((number(p, "bar")?, p.get("beat").and_then(Value::as_f64)?));
            match item.kind.as_str() {
                BAR => {
                    let (Some(index), Some(written)) = (number(p, "index"), number(p, "written"))
                    else {
                        continue;
                    };
                    // Its meter is the session's, a moment into the bar
                    // (a change is on the barline).
                    let seconds = Time::QuarterNotes(self.origin + at + length.min(1.0) / 64.0)
                        .seconds(&self.tempo);
                    let meter = self.tempo.meter_at(seconds);
                    let section = regions
                        .iter()
                        .find(|(from, to, _)| at >= from - 1e-6 && at < to - 1e-6);
                    ids.bars.push(id.clone());
                    out.bars.push(TypedBar {
                        index,
                        written,
                        pass: number(p, "pass").map_or(1, |n| n as u32),
                        meter: (meter.numerator as u8, meter.denominator as u8),
                        section: section.map_or(String::new(), |s| s.2.to_owned()),
                        section_start: section.is_some_and(|s| (s.0 - at).abs() < 1e-6),
                        repeat_start: flag(p, "repeat_start"),
                        repeat_end: flag(p, "repeat_end"),
                        repeat_count: number(p, "repeat_count").map_or(0, |n| n as u32),
                        ending: match p.get("ending") {
                            Some(Value::List(list)) => list
                                .iter()
                                .filter_map(|v| v.as_i64().and_then(|n| u8::try_from(n).ok()))
                                .collect(),
                            _ => Vec::new(),
                        },
                        repeat_sign: flag(p, "repeat_sign"),
                        at,
                        length,
                    });
                }
                CHORD => {
                    let Some((bar, beat)) = placed() else {
                        continue;
                    };
                    ids.chords.push(id.clone());
                    out.chords.push(TypedChord {
                        bar,
                        index: place,
                        written_bar: number(p, "written_bar").unwrap_or(bar),
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
                    let Some((bar, beat)) = placed() else {
                        continue;
                    };
                    ids.cues.push(id.clone());
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
                    let Some((bar, beat)) = placed() else {
                        continue;
                    };
                    ids.rests.push(id.clone());
                    out.rests.push(TypedRest {
                        bar,
                        index: place,
                        beat,
                        at,
                        length,
                    });
                }
                DYNAMIC => {
                    let Some((bar, beat)) = placed() else {
                        continue;
                    };
                    ids.dynamics.push(id.clone());
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
        out.bars = sorted(out.bars, &mut ids.bars, |a, b| a.index.cmp(&b.index));
        out.chords = placed_in_order(
            out.chords,
            &mut ids.chords,
            |c| (c.bar, c.at),
            |c, k| c.index = k,
        );
        out.cues = placed_in_order(
            out.cues,
            &mut ids.cues,
            |c| (c.bar, c.at),
            |c, k| c.index = k,
        );
        out.rests = placed_in_order(
            out.rests,
            &mut ids.rests,
            |r| (r.bar, r.at),
            |r, k| r.index = k,
        );
        out.dynamics = placed_in_order(
            out.dynamics,
            &mut ids.dynamics,
            |d| (d.bar, d.at),
            |d, k| d.index = k,
        );
        (out, ids)
    }

    /// The ids `chart` (an edit of [`Held::chart`]) is written with: each
    /// bar and item that is one the session holds keeps its id, the rest
    /// are minted ([`Ids`]).
    fn ids_for(&self, chart: &TypedChart) -> Ids {
        if !self.is_typed() {
            return Ids::positional(chart);
        }
        let (old, old_ids) = self.chart_and_ids();
        let mut mint = Mint {
            taken: self.items.keys().map(|id| id.as_str().to_owned()).collect(),
        };
        let matched = align(&bar_keys(&old), &bar_keys(chart));
        // A new bar's index → the index of the held bar it is.
        let old_bar: BTreeMap<usize, usize> = matched
            .iter()
            .zip(&chart.bars)
            .filter_map(|(m, b)| Some((b.index, old.bars[(*m)?].index)))
            .collect();
        let bars = matched
            .iter()
            .zip(&chart.bars)
            .map(|(m, b)| match m {
                Some(o) => old_ids.bars[*o].clone(),
                None => ItemId(mint.id(bar_id(b.index).as_str())),
            })
            .collect();
        Ids {
            bars,
            chords: child_ids(
                &old.chords,
                &old_ids.chords,
                &chart.chords,
                |c| c.bar,
                |c| format!("{}:{}", c.beat, c.symbol),
                |c| chord_id(c.bar, c.index),
                &old_bar,
                &mut mint,
            ),
            cues: child_ids(
                &old.cues,
                &old_ids.cues,
                &chart.cues,
                |c| c.bar,
                |c| format!("{}:{}:{}", c.beat, c.group, c.text),
                |c| placed_id("cue", c.bar, c.index),
                &old_bar,
                &mut mint,
            ),
            rests: child_ids(
                &old.rests,
                &old_ids.rests,
                &chart.rests,
                |r| r.bar,
                |r| format!("{}:{}", r.beat, r.length),
                |r| placed_id("rest", r.bar, r.index),
                &old_bar,
                &mut mint,
            ),
            dynamics: child_ids(
                &old.dynamics,
                &old_ids.dynamics,
                &chart.dynamics,
                |d| d.bar,
                |d| format!("{}:{}", d.beat, d.text),
                |d| placed_id("dynamic", d.bar, d.index),
                &old_bar,
                &mut mint,
            ),
        }
    }

    /// The session's tempo map with `chart`'s tempo and meters from bar 1
    /// on, what is before bar 1 (a count-in, a click intro) kept.
    fn tempo_with(&self, changes: &[TempoAt]) -> TempoMapData {
        let from = self.origin_seconds();
        let mut data = TempoMapData::of(&self.tempo);
        data.points.retain(|p| p.seconds < from - 1e-6);
        let meter = |m: (u8, u8)| Meter {
            numerator: u32::from(m.0),
            denominator: u32::from(m.1),
        };
        for (i, change) in changes.iter().enumerate() {
            // The session starts at bar 1 and nothing is before it: the
            // chart's opening tempo and meter are the map's own.
            if i == 0 && from <= 1e-6 && data.points.is_empty() {
                data.default_bpm = change.bpm;
                data.numerator = u32::from(change.meter.0);
                data.denominator = u32::from(change.meter.1);
                continue;
            }
            data.points.push(TempoChange {
                seconds: from + change.seconds,
                bpm: change.bpm,
                meter: Some(meter(change.meter)),
                shape: None,
                tension: None,
            });
        }
        data
    }
}

/// Whose structure a session has from bar 1 on, and so which way an
/// import goes: a session with a tempo map or sections of its own there
/// keeps them (they are the song's, measured or arranged, and the chart
/// is read against them); one with none takes the chart's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brings {
    /// The chart's tempo and meters are written to the tempo map.
    pub tempo: bool,
    /// The chart's sections are written as regions.
    pub sections: bool,
}

impl Held {
    /// What an import brings to this session ([`Brings`]): only what the
    /// session does not already have of its own.
    #[must_use]
    pub fn brings(&self) -> Brings {
        let from = self.origin_seconds();
        // A map Keyflow wrote before is the chart's to write again.
        let typed = self.is_typed();
        let own_tempo = TempoMapData::of(&self.tempo)
            .points
            .iter()
            .any(|p| p.seconds > from + 1e-6);
        // The session's own sections: regions it did not take from a chart,
        // on the ruler its sections are on (the DAW engine puts its
        // sections on a ruler of their own: `Some(lane)`, not the default).
        let own_sections = self.regions.iter().any(|(id, r)| {
            !id.as_str().starts_with(SECTION)
                && r.ruler == self.sections_ruler()
                && r.end.quarter_notes(&self.tempo) > self.origin + 1e-6
        });
        Brings {
            tempo: typed && !self.keeps_tempo() || !typed && !own_tempo,
            sections: !own_sections,
        }
    }

    /// Whether the tempo map from bar 1 is the session's own (Keyflow
    /// marks the one it wrote in its namespace; see [`import`]).
    fn keeps_tempo(&self) -> bool {
        self.tempo_kept
    }

    /// Where `chart` and the session's own structure disagree, in words:
    /// a bar whose meter is not the session's there, or that does not
    /// start on a session barline; a section that does not start where a
    /// region does. Empty when they are one structure.
    #[must_use]
    pub fn disagreements(&self, chart: &Chart) -> Vec<String> {
        let typed = TypedChart::of(chart);
        let mut out = Vec::new();
        let first = self.tempo.bar_beat(self.origin).bar;
        for bar in &typed.bars {
            let q = self.origin + bar.at;
            let at = self.tempo.bar_beat(q);
            let seconds = Time::QuarterNotes(q + bar.length.min(1.0) / 64.0).seconds(&self.tempo);
            let meter = self.tempo.meter_at(seconds);
            let theirs = (meter.numerator as u8, meter.denominator as u8);
            if theirs != bar.meter {
                out.push(format!(
                    "Bar {}: the chart is in {}/{}, the session in {}/{}",
                    bar.index + 1,
                    bar.meter.0,
                    bar.meter.1,
                    theirs.0,
                    theirs.1
                ));
            } else if at.beat != 1 || at.fraction > 1e-6 || at.bar - first != bar.index as i32 {
                out.push(format!(
                    "Bar {} of the chart starts at the session's bar {} beat {}",
                    bar.index + 1,
                    at.bar - first + 1,
                    at.beat
                ));
            }
            if out.len() >= 12 {
                out.push("… and more".to_owned());
                return out;
            }
        }
        let starts: Vec<f64> = self
            .regions
            .values()
            .filter(|r| r.ruler == self.sections_ruler())
            .map(|r| r.start.quarter_notes(&self.tempo) - self.origin)
            .collect();
        if !starts.is_empty() {
            for (name, from, _) in typed.sections() {
                if !starts.iter().any(|s| (s - from).abs() < 1e-6) {
                    out.push(format!(
                        "The chart's {name} starts where the session has no section"
                    ));
                }
            }
        }
        out
    }
}

/// The writes that make a session hold `chart`. Where the session has no
/// structure of its own from bar 1 ([`Held::brings`]), its tempo map and
/// its regions are made the chart's meters, tempo and sections; where it
/// has, they are left and the chart is read against them. Keyflow's lanes
/// are put and its items made exactly the chart's (one of Keyflow's on
/// its lanes that the chart has no more removed; another extension's
/// left). Empty when it already holds it.
#[must_use]
pub fn plan(chart: &Chart, held: &Held) -> Vec<Request> {
    plan_bringing(chart, held, held.brings())
}

/// [`plan`] for an edit of the chart a session shows
/// ([`Held::shown_source`]): its tempo, meters and sections are the
/// session's already, so only Keyflow's own items are written, never the
/// session's tempo map or regions.
#[must_use]
pub fn plan_items(chart: &Chart, held: &Held) -> Vec<Request> {
    plan_bringing(
        chart,
        held,
        Brings {
            tempo: false,
            sections: false,
        },
    )
}

fn plan_bringing(chart: &Chart, held: &Held, brings: Brings) -> Vec<Request> {
    let typed = TypedChart::of(chart);
    let owner = ExtensionId::from(crate::EXTENSION);
    let mut out = Vec::new();
    if !held.is_typed() {
        // Decided once, when the chart is first typed: whose tempo it is.
        out.push(Request::Set {
            name: TEMPO_KEPT.to_owned(),
            value: Value::Bool(!brings.tempo),
        });
        out.push(Request::Set {
            name: SCHEMA_ENTRY.to_owned(),
            value: Value::Int(SCHEMA),
        });
    }

    // The structure first: the items' bars are read against it.
    if brings.tempo {
        let tempo = held.tempo_with(&TypedChart::tempo_of(chart));
        if tempo != TempoMapData::of(&held.tempo) {
            out.push(Request::SetTempo { tempo });
        }
    }
    let sections = if brings.sections {
        typed.sections()
    } else {
        Vec::new()
    };
    // A section the chart brought keeps its region's id when one before
    // it is added or removed, as an item does ([`Ids`]).
    let mut brought: Vec<(&MarkId, &Region)> = held
        .regions
        .iter()
        .filter(|(id, _)| id.as_str().starts_with(SECTION))
        .collect();
    brought.sort_by(|a, b| {
        a.1.start
            .quarter_notes(&held.tempo)
            .total_cmp(&b.1.start.quarter_notes(&held.tempo))
    });
    let names: Vec<&str> = brought.iter().map(|(_, r)| r.name.as_str()).collect();
    let wanted_names: Vec<&str> = sections.iter().map(|s| s.0.as_str()).collect();
    let mut mint = Mint {
        taken: held
            .regions
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect(),
    };
    let region_ids: Vec<MarkId> = align(&names, &wanted_names)
        .into_iter()
        .enumerate()
        .map(|(n, found)| match found {
            Some(o) => brought[o].0.clone(),
            None => MarkId(mint.id(&format!("{SECTION}{n}"))),
        })
        .collect();
    let mut wanted_regions = BTreeMap::new();
    for ((name, from, to), id) in sections.into_iter().zip(region_ids) {
        wanted_regions.insert(
            id,
            Region {
                start: Time::QuarterNotes(held.origin + from),
                end: Time::QuarterNotes(held.origin + to),
                name,
                color: None,
                ruler: None,
            },
        );
    }
    for id in held.regions.keys() {
        if id.as_str().starts_with(SECTION) && !wanted_regions.contains_key(id) {
            out.push(Request::RemoveMark { id: id.clone() });
        }
    }
    for (id, region) in wanted_regions {
        // A section renamed in the session keeps its name: the region is
        // the section, and the chart does not carry a second name for it.
        let same_span = held
            .regions
            .get(&id)
            .is_some_and(|have| have.start == region.start && have.end == region.end);
        if !same_span {
            out.push(Request::PutRegion { id, region });
        }
    }

    for (order, (id, name)) in LANES.into_iter().enumerate() {
        let lane = Lane {
            owner: owner.clone(),
            kind: id.to_owned(),
            name: name.to_owned(),
            order: order as f64,
            parent: None,
        };
        let id = LaneId::from(id);
        if held.lanes.get(&id) != Some(&lane) {
            out.push(Request::PutLane { id, lane });
        }
    }
    let wanted = typed.items_with(held.origin, &held.ids_for(&typed));
    let mine: BTreeSet<&str> = LANES.iter().map(|l| l.0).collect();
    for (id, item) in &held.items {
        if item.owner == owner && mine.contains(item.lane.as_str()) && !wanted.contains_key(id) {
            out.push(Request::RemoveItem { id: id.clone() });
        }
    }
    for (id, item) in wanted {
        if held.items.get(&id) != Some(&item) {
            out.push(Request::PutItem { id, item });
        }
    }
    out
}

/// `chart` written into `doc` ([`plan`]). Importing the same chart again
/// changes nothing.
///
/// # Errors
///
/// The document refused a write, or the chart's tempo is not one a tempo
/// map holds.
pub fn import(doc: &SessionDoc, chart: &Chart) -> crate::Result<TypedChart> {
    for request in plan(chart, &Held::of_doc(doc)) {
        apply(doc, request)?;
    }
    Ok(read(doc))
}

/// One of [`plan`]'s requests done to `doc` directly.
fn apply(doc: &SessionDoc, request: Request) -> crate::Result<()> {
    let done: DocResult<()> = match request {
        Request::SetTempo { tempo } => {
            let map = tempo
                .to_tempo_map()
                .map_err(|e| Error::Service(e.to_string()))?;
            doc.set_tempo(&map)
        }
        Request::Set { name, value } => doc
            .ext(&ExtensionId::from(crate::EXTENSION))
            .and_then(|ext| ext.set(&name, &value)),
        Request::PutRegion { id, region } => doc.put_region(&id, &region),
        Request::RemoveMark { id } => doc.remove_region(&id),
        Request::PutLane { id, lane } => doc.put_lane(&id, &lane),
        Request::PutItem { id, item } => doc.put_item(&id, &item),
        Request::RemoveItem { id } => doc.remove_item(&id),
        _ => Ok(()),
    };
    done.map_err(|e| Error::Doc(e.to_string()))
}

/// The typed chart `doc` holds. Empty when it holds none.
#[must_use]
pub fn read(doc: &SessionDoc) -> TypedChart {
    Held::of_doc(doc).chart()
}

/// The writes that give written chord `index` of written bar `bar` the
/// push `push`: one put per time it is played (a chord in a repeated bar
/// is played on each pass), each to that item's own push fields, the rest
/// of it left as it is. Empty when there is no such chord.
#[must_use]
pub fn push_requests(
    items: &BTreeMap<ItemId, Item>,
    bar: usize,
    index: usize,
    push: PushPull,
) -> Vec<Request> {
    let is = |payload: &Value, key: &str, n: usize| {
        payload.get(key).and_then(Value::as_i64) == i64::try_from(n).ok()
    };
    items
        .iter()
        .filter(|(_, item)| {
            item.kind == CHORD
                && is(&item.payload, "written_bar", bar)
                && is(&item.payload, "written_index", index)
        })
        .filter_map(|(id, item)| {
            let mut item = item.clone();
            let Value::Map(payload) = &mut item.payload else {
                return None;
            };
            payload.retain(|key, _| !key.starts_with("push_"));
            for (key, value) in push_fields(push) {
                payload.insert(key.to_owned(), value);
            }
            Some(Request::PutItem {
                id: id.clone(),
                item,
            })
        })
        .collect()
}

/// [`push_requests`] done to `doc`: how many items it wrote.
///
/// # Errors
///
/// The document refused a write.
pub fn set_push(
    doc: &SessionDoc,
    bar: usize,
    index: usize,
    push: PushPull,
) -> crate::Result<usize> {
    let requests = push_requests(&doc.items(), bar, index, push);
    let count = requests.len();
    for request in requests {
        apply(doc, request)?;
    }
    Ok(count)
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

/// `items` and their `ids` sorted together by `order`.
fn sorted<T>(
    items: Vec<T>,
    ids: &mut Vec<ItemId>,
    order: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Vec<T> {
    let mut both: Vec<(T, ItemId)> = items.into_iter().zip(ids.drain(..)).collect();
    both.sort_by(|a, b| order(&a.0, &b.0).then_with(|| a.1.cmp(&b.1)));
    let (items, sorted_ids): (Vec<T>, Vec<ItemId>) = both.into_iter().unzip();
    *ids = sorted_ids;
    items
}

/// `items` in their bars in time order, each given its place in its bar.
fn placed_in_order<T>(
    items: Vec<T>,
    ids: &mut Vec<ItemId>,
    at: impl Fn(&T) -> (usize, f64),
    set_place: impl Fn(&mut T, usize),
) -> Vec<T> {
    let mut items = sorted(items, ids, |a, b| {
        let (a, b) = (at(a), at(b));
        a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
    });
    let mut bar = None;
    let mut place = 0;
    for item in &mut items {
        let this = at(item).0;
        if bar != Some(this) {
            (bar, place) = (Some(this), 0);
        }
        set_place(item, place);
        place += 1;
    }
    items
}

impl ChartSource {
    /// The calls that read a session's timeline, its structure and
    /// Keyflow's own data (for [`Held::of_replies`]); `None` for a
    /// document, which has none of them.
    ///
    /// # Errors
    ///
    /// A request could not be encoded.
    pub fn timeline_calls(&self) -> crate::Result<Option<[ServiceCall; 3]>> {
        let Self::Session { doc } = self else {
            return Ok(None);
        };
        let call = |request| -> crate::Result<ServiceCall> {
            Ok(ServiceCall {
                service: session::extension::SERVICE,
                method: READ,
                args: call_json(doc, request).map_err(Error::Service)?,
            })
        };
        Ok(Some([
            call(Request::ReadTimeline)?,
            call(Request::ReadStructure)?,
            call(Request::ReadData)?,
        ]))
    }

    /// `requests` (from [`plan`] or [`push_requests`]) as calls to the
    /// session's provider, in order, each by the action it needs (the
    /// tempo map and regions are the session's structure); empty for a
    /// document.
    ///
    /// # Errors
    ///
    /// A request could not be encoded.
    pub fn typed_calls(&self, requests: Vec<Request>) -> crate::Result<Vec<ServiceCall>> {
        let Self::Session { doc } = self else {
            return Ok(Vec::new());
        };
        requests
            .into_iter()
            .map(|request| {
                Ok(ServiceCall {
                    service: session::extension::SERVICE,
                    method: action_for(&request),
                    args: call_json(doc, request).map_err(Error::Service)?,
                })
            })
            .collect()
    }
}
