//! What a session's chart shows: the session's own structure, with the
//! chart's typed items in it.
//!
//! The chart pane on a session draws from the session alone, never from a
//! `.kf` file or a text entry: its **road map** ([`RoadMap`]) is the
//! session's regions as sections (in order, with their bars), its tempo
//! map's tempos and meters, and its markers; the chart's typed items
//! ([`crate::typed`]: chords, rests, cues, dynamics) are drawn in those
//! bars. A session nobody has charted yet still shows its road map, every
//! bar an empty measure of slashes.
//!
//! The bars are the session's bars, read the way [`Held::chart`] reads
//! them (regions as sections, the tempo map's meter at each bar), so the
//! chart and the arrangement count the same bars.
//!
//! The engraver draws a [`keyflow::Chart`], and Keyflow's language is the
//! one constructor that fills a chart's derived parts (chord memory,
//! rhythm slashes, bar positions): [`RoadMap::source`] composes the
//! session's chart in it, in memory, and the pane parses that. The text is
//! never stored; an edit is written back as typed items
//! ([`crate::typed::plan`]).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use keyflow_proto::SectionType;
use session::{MarkId, Region, TempoMap, Time};

use crate::typed::{Held, TypedChart};

/// Two times this close are one place (quarter notes).
const NEAR: f64 = 1e-6;

/// One section of the road map: a region of the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadSection {
    /// The region's name, as the arrangement shows it (`VS 1`).
    pub name: String,
    /// Its first bar, from the song's bar 1 (1-based).
    pub first_bar: i32,
    /// How many bars it spans.
    pub bars: u32,
}

/// A tempo or a meter from a bar on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadTempo {
    /// The bar it starts at (1-based, from the song's bar 1).
    pub bar: i32,
    pub bpm: f64,
    pub meter: (u32, u32),
}

/// A marker, at its bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadMarker {
    pub name: String,
    pub bar: i32,
    pub beat: i32,
}

/// The session's structure as the chart draws it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoadMap {
    /// Bars before the first section, from the song's bar 1, drawn as a
    /// count-in (they keep no bar numbers).
    pub lead_in: u32,
    pub sections: Vec<RoadSection>,
    /// The opening tempo and meter, then each change, in order.
    pub tempo: Vec<RoadTempo>,
    pub markers: Vec<RoadMarker>,
}

impl RoadMap {
    /// Every bar the road map draws (the count-in left out).
    #[must_use]
    pub fn bars(&self) -> u32 {
        self.sections.iter().map(|s| s.bars).sum()
    }
}

/// Where `q` (quarter notes on the session's timeline) is, as a bar from
/// the song's bar 1 and a beat: `first` is the session's bar number of the
/// song's bar 1.
fn bar_at(tempo: &TempoMap, q: f64, first: i32) -> (i32, i32, f64) {
    let at = tempo.bar_beat(q + NEAR);
    (at.bar - first + 1, at.beat, at.fraction)
}

/// The bar a span ending at `q` ends with: the bar `q` falls in, or the
/// one before when `q` is on its barline.
fn last_bar(tempo: &TempoMap, q: f64, first: i32) -> i32 {
    let at = tempo.bar_beat(q + NEAR);
    let on_barline = at.beat == 1 && at.fraction < 1e-3;
    at.bar - first + 1 - i32::from(on_barline)
}

/// The ruler the song's sections are on: the one most regions are on (the
/// default ruler on a tie). Another ruler's spans (a set's songs, loop
/// ranges) are another kind of span.
pub(crate) fn sections_ruler_of(regions: &BTreeMap<MarkId, Region>) -> Option<u32> {
    let mut counts: BTreeMap<Option<u32>, usize> = BTreeMap::new();
    for r in regions.values() {
        *counts.entry(r.ruler).or_default() += 1;
    }
    counts
        .iter()
        .max_by_key(|(ruler, n)| (**n, ruler.is_none()))
        .and_then(|(ruler, _)| *ruler)
}

/// The regions that are the song's sections ([`sections_ruler_of`]), in
/// order of start.
fn section_regions(regions: &BTreeMap<MarkId, Region>, tempo: &TempoMap) -> Vec<Region> {
    let ruler = sections_ruler_of(regions);
    let mut out: Vec<Region> = regions
        .values()
        .filter(|r| r.ruler == ruler)
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        a.start
            .quarter_notes(tempo)
            .total_cmp(&b.start.quarter_notes(tempo))
    });
    out
}

impl Held {
    /// The session's structure as the chart draws it: its sections (the
    /// regions, [`section_regions`]) with their bars from the song's bar 1,
    /// its tempos and meters, and its markers.
    #[must_use]
    pub fn road_map(&self) -> RoadMap {
        let tempo = &self.tempo;
        let first = tempo.bar_beat(self.origin + NEAR).bar;
        let mut out = RoadMap::default();
        let mut next_bar = 1;
        for region in section_regions(&self.regions, tempo) {
            let start = region.start.quarter_notes(tempo);
            let end = region.end.quarter_notes(tempo);
            let from = bar_at(tempo, start, first).0;
            let to = last_bar(tempo, end, first);
            // Before the song's bar 1 (a count-in, a click): not drawn.
            if to < 1 || to < from {
                continue;
            }
            let from = from.max(next_bar);
            if out.sections.is_empty() && from > 1 {
                out.lead_in = u32::try_from(from - 1).unwrap_or(0);
            } else if from > next_bar {
                // Bars between two sections: a section of their own.
                out.sections.push(RoadSection {
                    name: String::new(),
                    first_bar: next_bar,
                    bars: u32::try_from(from - next_bar).unwrap_or(0),
                });
            }
            out.sections.push(RoadSection {
                name: region.name.trim().to_owned(),
                first_bar: from,
                bars: u32::try_from(to - from + 1).unwrap_or(0),
            });
            next_bar = to + 1;
        }
        // The tempo and meter at the song's bar 1, then each change after.
        let origin_s = Time::QuarterNotes(self.origin).seconds(tempo);
        let opening = |s: f64| {
            let m = tempo.meter_at(s);
            (tempo.bpm_at(s), (m.numerator, m.denominator))
        };
        let (bpm, meter) = opening(origin_s + 1e-6);
        out.tempo.push(RoadTempo { bar: 1, bpm, meter });
        for point in tempo.points() {
            let s = point.position_seconds();
            if s <= origin_s + 1e-6 {
                continue;
            }
            let q = Time::Seconds(s).quarter_notes(tempo);
            let (bar, _, _) = bar_at(tempo, q, first);
            let (bpm, meter) = opening(s + 1e-6);
            let last = out.tempo.last().copied();
            if last.is_some_and(|l| (l.bpm - bpm).abs() < 1e-3 && l.meter == meter) {
                continue;
            }
            match out.tempo.last_mut() {
                Some(l) if l.bar == bar => {
                    l.bpm = bpm;
                    l.meter = meter;
                }
                _ => out.tempo.push(RoadTempo { bar, bpm, meter }),
            }
        }
        let mut marks: Vec<RoadMarker> = self
            .markers
            .values()
            .filter_map(|m| {
                let (bar, beat, _) = bar_at(tempo, m.at.quarter_notes(tempo), first);
                (bar >= 1 && !m.name.trim().is_empty()).then(|| RoadMarker {
                    name: m.name.trim().to_owned(),
                    bar,
                    beat,
                })
            })
            .collect();
        marks.sort_by_key(|m| (m.bar, m.beat));
        out.markers = marks;
        out
    }
}

/// A section's header in Keyflow's language: a built-in kind by its
/// abbreviation (`VS 1` is `VS`, numbered by the chart), else the name
/// as a custom section (`[Tag]`).
fn header(name: &str, bars: u32) -> String {
    let base = name
        .rsplit_once(' ')
        .filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit()))
        .map_or(name, |(base, _)| base)
        .trim();
    let builtin = !base.is_empty()
        && !base.contains(['[', ']', '"', ';'])
        && matches!(SectionType::parse(base), Ok(kind) if !matches!(kind, SectionType::Custom(_)));
    if builtin {
        return format!("{} {bars}", base.to_uppercase());
    }
    let shown: String = name
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | '"' | ';' | '(' | ')'))
        .collect();
    let shown = shown.trim();
    if shown.is_empty() {
        format!("[Bars] {bars}")
    } else {
        format!("[{shown}] {bars}")
    }
}

/// `quarters` as a note value (`_4`, `_2.`), when it is one.
fn note_value(quarters: f64) -> Option<&'static str> {
    const VALUES: [(f64, &str); 9] = [
        (4.0, "_1"),
        (3.0, "_2."),
        (2.0, "_2"),
        (1.5, "_4."),
        (1.0, "_4"),
        (0.75, "_8."),
        (0.5, "_8"),
        (0.25, "_16"),
        (6.0, "_1."),
    ];
    VALUES
        .iter()
        .find(|(q, _)| (q - quarters).abs() < 1e-3)
        .map(|(_, v)| *v)
}

/// Text safe inside a cue's quotes (`;` starts a comment anywhere).
fn cue_text(text: &str) -> String {
    text.replace('"', "'").replace(';', ",")
}

/// A bar with nothing typed in it: a space a beat, drawn as slashes.
fn empty_bar(meter: (u32, u32)) -> String {
    let beat = format!("s{}", meter.1.max(1));
    vec![beat; meter.0.max(1) as usize].join(" ")
}

/// One bar's music: the typed chords and rests in it, else slashes.
fn bar_body(typed: &TypedChart, index: usize, meter: (u32, u32)) -> String {
    let beat_q = 4.0 / f64::from(meter.1.max(1));
    let bar_q = beat_q * f64::from(meter.0.max(1));
    let mut events: Vec<(f64, String)> = Vec::new();
    for c in typed.chords.iter().filter(|c| c.bar == index) {
        let mut symbol = String::new();
        if c.accent {
            symbol.push('>');
        }
        if c.staccato {
            symbol.push('.');
        }
        let level = c
            .push
            .map_or(0, |(_, amount)| usize::from(amount.level.clamp(1, 3)));
        let early = c.push.is_some_and(|(early, _)| early);
        if early {
            symbol.push_str(&"'".repeat(level));
        }
        symbol.push_str(&c.symbol);
        if !early && level > 0 {
            symbol.push_str(&"'".repeat(level));
        }
        let length = c.length.min(bar_q).max(0.0);
        let written = if (length - bar_q).abs() < 1e-3 {
            String::new()
        } else if let Some(v) = note_value(length).filter(|_| level == 0 || early) {
            v.to_owned()
        } else {
            format!(
                " {}",
                "/".repeat(((length / beat_q).round() as usize).max(1))
            )
        };
        events.push(((c.beat - 1.0) * beat_q, format!("{symbol}{written}")));
    }
    for r in typed.rests.iter().filter(|r| r.bar == index) {
        let rest =
            note_value(r.length).map_or_else(|| "r4".to_owned(), |v| format!("r{}", &v[1..]));
        events.push(((r.beat - 1.0) * beat_q, rest));
    }
    if events.is_empty() {
        return empty_bar(meter);
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    events
        .into_iter()
        .map(|(_, e)| e)
        .collect::<Vec<_>>()
        .join(" ")
}

impl RoadMap {
    /// The chart this road map and `typed` make, in Keyflow's language:
    /// `title`, the opening tempo and meter, each section's header with
    /// its bars, a meter change where the tempo map has one (`T3/4`), a
    /// tempo change as a cue and a tempo mark, the markers as cues above
    /// their bars, and in each bar its typed chords, rests, cues and
    /// dynamics (an empty bar: a bar of slashes).
    #[must_use]
    pub fn source(&self, title: &str, typed: &TypedChart) -> String {
        let mut out = String::new();
        let title = title.trim();
        let _ = writeln!(
            out,
            "\\title \"{}\"",
            cue_text(if title.is_empty() { "Chart" } else { title })
        );
        let opening = self.tempo.first().copied().unwrap_or(RoadTempo {
            bar: 1,
            bpm: 120.0,
            meter: (4, 4),
        });
        let _ = writeln!(
            out,
            "{}bpm {}/{}",
            opening.bpm.round() as i64,
            opening.meter.0,
            opening.meter.1
        );
        out.push('\n');
        if self.lead_in > 0 {
            let _ = writeln!(out, "Count {}", self.lead_in);
        }
        let mut meter = opening.meter;
        let mut bpm = opening.bpm;
        for section in &self.sections {
            let _ = writeln!(out, "{}", header(&section.name, section.bars));
            for n in 0..section.bars {
                let bar = section.first_bar + i32::try_from(n).unwrap_or(0);
                let index = usize::try_from(bar - 1).unwrap_or(0);
                // A meter change is the bar's first token (a line of its
                // own would be read as the chart's opening meter).
                let mut line = String::new();
                for change in self.tempo.iter().skip(1).filter(|t| t.bar == bar) {
                    if change.meter != meter {
                        let _ = write!(line, "T{}/{} ", change.meter.0, change.meter.1);
                        meter = change.meter;
                    }
                }
                for d in typed.dynamics.iter().filter(|d| d.bar == index) {
                    let _ = writeln!(out, "dyn {}", d.text.trim());
                }
                for change in self.tempo.iter().skip(1).filter(|t| t.bar == bar) {
                    // A tempo that changes (not a meter alone), and a cue
                    // saying it: the engraver does not draw a `->` yet.
                    if (change.bpm - bpm).abs() < 1e-3 {
                        continue;
                    }
                    bpm = change.bpm;
                    let shown = change.bpm.round() as i64;
                    let _ = write!(line, "->{shown}bpm ^\"♩ = {shown}\" ");
                }
                for m in self.markers.iter().filter(|m| m.bar == bar) {
                    let _ = write!(line, "^\"{}\" ", cue_text(&m.name));
                }
                for c in typed.cues.iter().filter(|c| c.bar == index) {
                    if c.group.is_empty() {
                        let _ = write!(line, "\"{}\" ", cue_text(&c.text));
                    } else {
                        let _ = writeln!(out, "@{} \"{}\"", c.group, cue_text(&c.text));
                    }
                }
                line.push_str(&bar_body(typed, index, meter));
                let _ = writeln!(out, "{line}");
            }
        }
        out
    }
}

impl Held {
    /// The ruler the session's sections are on ([`sections_ruler_of`]).
    #[must_use]
    pub fn sections_ruler(&self) -> Option<u32> {
        sections_ruler_of(&self.regions)
    }

    /// The chart this session shows, in Keyflow's language, titled
    /// `title`: its road map with its typed items in it.
    #[must_use]
    pub fn shown_source(&self, title: &str) -> String {
        self.road_map().source(title, &self.chart())
    }
}
