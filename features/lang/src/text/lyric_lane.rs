//! Put an LRC's lyrics into a chart as timed lyric lanes.
//!
//! An `.lrc` says *when* each line is sung, in seconds; a chart says where
//! each bar is, in beats. This module lays the chart's bars on the clock
//! ([`Timeline`]) — from its own tempo and meters and the time of the first
//! downbeat, or from a measured start time per bar — then puts every word on
//! the nearest grid step of the bar it falls in ([`place_lrc`]) and writes
//! the result into the chart source as one `[lyrics lrc]` lane per section
//! ([`write_lanes`]):
//!
//! ```text
//! VS 8
//! D G A D
//! [lyrics lrc]
//! @1:3 Sing @2 a @2:2 new @2:3 song
//! ```
//!
//! It is a rough rhythm, on purpose: the right bar and about the right beat
//! is what a band chart needs. A plain LRC has one time per line, so a
//! line's words are spread over the time until the next line, weighted by
//! their syllables; an enhanced LRC's word times are used as they are.

use std::collections::BTreeMap;

use keyflow_proto::lrc::Lrc;

use crate::text::chart::{Chart, LyricLine, LyricSyllable};
use crate::text::sections::SectionType;

/// The name the generated lyric track carries. Re-running replaces tracks
/// with this name and leaves every other lyric track alone.
pub const LANE_NAME: &str = "lrc";

/// One bar of the chart on the clock.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    /// Index into `chart.sections`.
    pub section: usize,
    /// 0-based bar within the section.
    pub index: usize,
    /// Seconds from the start of the recording.
    pub start: f64,
    pub end: f64,
    /// Beats in the bar and the note value of a beat (`(3, 4)` for 3/4).
    pub meter: (u8, u8),
}

/// Every bar of the chart, in playing order, with its start and end time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    pub bars: Vec<Bar>,
}

/// The bars of the chart, in order: (section index, bar index, meter).
fn chart_bars(chart: &Chart) -> Vec<(usize, usize, (u8, u8))> {
    let mut out = Vec::new();
    for (s, section) in chart.sections.iter().enumerate() {
        for (i, measure) in section.measures().iter().enumerate() {
            let (n, d) = measure.time_signature;
            let meter = if n == 0 || d == 0 { (4, 4) } else { (n, d) };
            out.push((s, i, meter));
        }
    }
    out
}

/// Quarter notes in a bar of `meter`.
fn quarters(meter: (u8, u8)) -> f64 {
    meter.0 as f64 * 4.0 / meter.1 as f64
}

/// The tempo in force at the start of each section: the chart's tempo, then
/// any tempo change from the section it is written in. (A change written
/// mid-section takes effect from that section's first bar — near enough for
/// lyrics.)
fn section_tempos(chart: &Chart) -> Vec<f64> {
    let mut bpm = chart.tempo.as_ref().map(|t| t.bpm).unwrap_or(120.0);
    (0..chart.sections.len())
        .map(|s| {
            if let Some(change) = chart
                .tempo_changes
                .iter()
                .filter(|c| c.section_index == s)
                .last()
            {
                bpm = change.to_tempo.bpm;
            }
            bpm
        })
        .collect()
}

/// Bars of the count-in (a `Count N` section) before bar 1. They are bars
/// of the chart, but not numbered and never sung.
pub fn count_in_bars(chart: &Chart) -> usize {
    chart
        .sections
        .iter()
        .take_while(|s| s.section.section_type == SectionType::CountIn)
        .map(|s| s.measures().len())
        .sum()
}

impl Timeline {
    /// The chart's own clock: its tempo and meters, with bar 1 — the first
    /// bar after any count-in — starting `downbeat` seconds into the
    /// recording.
    pub fn from_chart(chart: &Chart, downbeat: f64) -> Self {
        let tempos = section_tempos(chart);
        let layout = chart_bars(chart);
        let secs: Vec<f64> = layout
            .iter()
            .map(|(section, _, meter)| quarters(*meter) * 60.0 / tempos[*section])
            .collect();
        let count_in: f64 = secs[..count_in_bars(chart).min(secs.len())].iter().sum();
        let mut t = downbeat - count_in;
        let bars = layout
            .into_iter()
            .zip(secs)
            .map(|((section, index, meter), secs)| {
                let bar = Bar {
                    section,
                    index,
                    start: t,
                    end: t + secs,
                    meter,
                };
                t += secs;
                bar
            })
            .collect();
        Self { bars }
    }

    /// A measured clock: `starts[k]` is when the chart's bar `first + k`
    /// starts (`first` counts the count-in bars, so 0 is the chart's very
    /// first bar) — a Songsterr sync, say. A recording that drifts stays in
    /// step this way where a single tempo cannot. Bars outside `starts`
    /// continue at the chart's tempo.
    pub fn from_bar_starts(chart: &Chart, starts: &[f64], first: usize) -> Self {
        let tempos = section_tempos(chart);
        let layout = chart_bars(chart);
        let nominal: Vec<f64> = layout
            .iter()
            .map(|(section, _, meter)| quarters(*meter) * 60.0 / tempos[*section])
            .collect();
        let start_of =
            |k: usize| -> Option<f64> { k.checked_sub(first).and_then(|i| starts.get(i).copied()) };
        // Bars before the sync: back from its first bar at the chart's tempo.
        let mut t = starts.first().copied().unwrap_or(0.0)
            - nominal[..first.min(nominal.len())].iter().sum::<f64>();
        let mut bars = Vec::with_capacity(layout.len());
        for (k, (section, index, meter)) in layout.into_iter().enumerate() {
            let start = start_of(k).unwrap_or(t);
            let end = start_of(k + 1)
                .filter(|&e| e > start)
                .unwrap_or(start + nominal[k]);
            bars.push(Bar {
                section,
                index,
                start,
                end,
                meter,
            });
            t = end;
        }
        Self { bars }
    }

    /// Grid steps in bar `k` at `grid` (8 = eighth notes).
    fn steps(&self, k: usize, grid: u32) -> u32 {
        let (n, d) = self.bars[k].meter;
        ((n as u32 * grid) / d as u32).max(1)
    }
}

/// Where a moment falls on the timeline, as a global grid step.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Locate {
    Before,
    After,
    At(u64),
}

/// The grid: each bar's first global step, so a step number names a bar
/// and a place in it.
struct Grid<'a> {
    timeline: &'a Timeline,
    grid: u32,
    first_step: Vec<u64>,
    total: u64,
}

impl<'a> Grid<'a> {
    fn new(timeline: &'a Timeline, grid: u32) -> Self {
        let mut first_step = Vec::with_capacity(timeline.bars.len());
        let mut total = 0u64;
        for k in 0..timeline.bars.len() {
            first_step.push(total);
            total += timeline.steps(k, grid) as u64;
        }
        Self {
            timeline,
            grid,
            first_step,
            total,
        }
    }

    fn locate(&self, t: f64) -> Locate {
        let bars = &self.timeline.bars;
        let (Some(first), Some(last)) = (bars.first(), bars.last()) else {
            return Locate::After;
        };
        // Half a grid step of slack at either end: a word sung a hair
        // before the downbeat belongs on it.
        let slack = |k: usize| {
            let b = &bars[k];
            (b.end - b.start) / self.timeline.steps(k, self.grid) as f64 / 2.0
        };
        if t < first.start - slack(0) {
            return Locate::Before;
        }
        if t >= last.end {
            return Locate::After;
        }
        let k = bars.iter().rposition(|b| b.start <= t).unwrap_or(0);
        let b = &bars[k];
        let steps = self.timeline.steps(k, self.grid) as f64;
        let frac = ((t - b.start) / (b.end - b.start)).max(0.0);
        let step = (frac * steps).round() as u64;
        Locate::At(self.first_step[k] + step)
    }

    /// The bar and 0-based beat of a global step.
    fn place(&self, step: u64) -> Option<(usize, f32)> {
        if step >= self.total {
            return None;
        }
        let k = self.first_step.partition_point(|&s| s <= step) - 1;
        let (_, d) = self.timeline.bars[k].meter;
        let within = (step - self.first_step[k]) as f32;
        Some((k, within * d as f32 / self.grid as f32))
    }
}

/// A line the converter could not put in the chart, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Unplaced {
    /// Seconds into the recording (LRC input).
    pub time: Option<f64>,
    /// Printed bar number (bar-word input).
    pub bar: Option<usize>,
    pub text: String,
    pub reason: String,
}

/// The words placed, per section, and what could not be.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    /// Section index → its lyric lane.
    pub lanes: BTreeMap<usize, LyricLine>,
    pub unplaced: Vec<Unplaced>,
    /// Lines placed (whole or in part).
    pub placed_lines: usize,
}

/// Options for [`place_lrc`].
#[derive(Debug, Clone, Copy)]
pub struct PlaceOptions {
    /// Grid to quantise to, as a note value: 8 = eighth notes, 4 = beats.
    pub grid: u32,
    /// The longest a plain line's words are spread over, in beats per word.
    /// Past that, the singing has stopped and an instrumental has begun.
    pub max_beats_per_word: f64,
}

impl Default for PlaceOptions {
    fn default() -> Self {
        Self {
            grid: 8,
            max_beats_per_word: 2.0,
        }
    }
}

/// Characters the chart language reads as something else: a `;` starts a
/// comment, quotes and brackets start text cues and chord markers, and a
/// leading `@` would be a stamp.
fn clean_word(word: &str) -> String {
    let w: String = word
        .chars()
        .map(|c| if c == ';' { ',' } else { c })
        .filter(|c| !matches!(c, '"' | '{' | '}' | '[' | ']' | '|'))
        .collect();
    w.trim_start_matches('@').to_string()
}

/// A rough syllable count — vowel groups — so a long word gets more of the
/// line than "the".
fn syllables(word: &str) -> usize {
    let mut count = 0;
    let mut in_vowel = false;
    for c in word.chars().map(|c| c.to_ascii_lowercase()) {
        let vowel = matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if vowel && !in_vowel {
            count += 1;
        }
        in_vowel = vowel;
    }
    if count > 1 && word.to_ascii_lowercase().ends_with('e') && !word.ends_with("le") {
        count -= 1;
    }
    count.max(1)
}

/// Seconds per beat in the bar sounding at `t`.
fn beat_seconds(timeline: &Timeline, t: f64) -> f64 {
    let bar = timeline
        .bars
        .iter()
        .rfind(|b| b.start <= t)
        .or(timeline.bars.first());
    match bar {
        Some(b) => (b.end - b.start) / b.meter.0.max(1) as f64,
        None => 0.5,
    }
}

/// The words of a line with the time each starts.
fn timed_words(
    lrc: &Lrc,
    i: usize,
    timeline: &Timeline,
    opts: &PlaceOptions,
) -> Vec<(f64, String)> {
    let line = &lrc.lines[i];
    if !line.words.is_empty() {
        return line
            .words
            .iter()
            .map(|w| (w.start as f64, w.text.clone()))
            .collect();
    }
    let words: Vec<&str> = line.text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let start = line.start as f64;
    let next = lrc
        .lines
        .get(i + 1)
        .map(|l| l.start as f64)
        .unwrap_or(f64::INFINITY);
    let cap = words.len() as f64 * opts.max_beats_per_word * beat_seconds(timeline, start);
    let span = (next - start).min(cap).max(0.0);
    let weights: Vec<usize> = words.iter().map(|w| syllables(w)).collect();
    let total: usize = weights.iter().sum();
    let mut before = 0usize;
    words
        .iter()
        .zip(&weights)
        .map(|(w, &n)| {
            let t = start + span * before as f64 / total as f64;
            before += n;
            (t, (*w).to_string())
        })
        .collect()
}

/// Lays words on the grid one at a time, keeping them in order, and
/// collects them into per-section lanes a source line at a time.
struct Placer<'a> {
    grid: Grid<'a>,
    /// First step of bar 1 (after the count-in).
    first_real_step: u64,
    out: Placement,
    last_step: Option<u64>,
    /// The previous word ended in `-`: this one finishes it.
    continues: bool,
    /// Section → syllables of the current source line in that section.
    pieces: BTreeMap<usize, Vec<LyricSyllable>>,
}

/// What became of one word.
enum Put {
    Placed,
    /// A held syllable (`_`) or nothing left after cleaning: not drawn.
    Skipped,
    BeforeStart,
    PastEnd,
}

impl<'a> Placer<'a> {
    fn new(timeline: &'a Timeline, grid: u32, count_in: usize) -> Self {
        let grid = Grid::new(timeline, grid.max(1));
        let first_real_step = grid.first_step.get(count_in).copied().unwrap_or(grid.total);
        Self {
            grid,
            first_real_step,
            out: Placement::default(),
            last_step: None,
            continues: false,
            pieces: BTreeMap::new(),
        }
    }

    /// Put `word` at `step`, or just after the word before it.
    fn put(&mut self, step: u64, word: &str) -> Put {
        // `_`: the syllable before is held here — an extender, drawn as a
        // melisma line, and only after a syllable of this lane.
        let extender = !word.is_empty() && word.chars().all(|c| c == '_');
        if extender && self.last_step.is_none() {
            return Put::Skipped;
        }
        let (text, hyphen) = match word.strip_suffix('-') {
            Some(t) if !t.is_empty() && !extender => (t, true),
            _ => (word, false),
        };
        let text = if extender {
            "_".to_string()
        } else {
            clean_word(text)
        };
        if text.is_empty() || (!extender && text.chars().all(|c| c == '_')) {
            return Put::Skipped;
        }
        let step = match self.last_step {
            Some(prev) if step <= prev => prev + 1,
            _ => step,
        };
        if step < self.first_real_step {
            return Put::BeforeStart;
        }
        let Some((k, beat)) = self.grid.place(step) else {
            return Put::PastEnd;
        };
        self.last_step = Some(step);
        let bar = &self.grid.timeline.bars[k];
        let mut syl = LyricSyllable::with_timing(text, bar.index, beat);
        syl.word_initial = !self.continues && !extender;
        syl.hyphen_after = hyphen;
        if !extender {
            self.continues = hyphen;
        }
        self.pieces.entry(bar.section).or_default().push(syl);
        Put::Placed
    }

    /// End the current source line.
    fn end_line(&mut self) {
        for (section, mut syls) in std::mem::take(&mut self.pieces) {
            if let Some(last) = syls.last_mut() {
                last.line_break_after = true;
            }
            let lane = self.out.lanes.entry(section).or_insert_with(|| {
                let mut l = LyricLine::new(Vec::new());
                l.timed = true;
                l
            });
            lane.syllables.extend(syls);
        }
    }

    fn finish(mut self) -> Placement {
        self.end_line();
        self.out
    }
}

const BEFORE: &str = "before bar 1 (check --downbeat / the sync)";

/// Put every lyric line of `lrc` on the timeline.
///
/// Words land on the nearest grid step; a word never lands on or before
/// the step of the word before it, so two words never share a place and the
/// order always survives. Nothing lands in the first `count_in` bars (a
/// count-in). A line that starts before bar 1 or after the last bar is
/// reported rather than dropped, and so are the words of a line that run
/// past the end of the chart.
pub fn place_lrc(
    lrc: &Lrc,
    timeline: &Timeline,
    opts: &PlaceOptions,
    count_in: usize,
) -> Placement {
    let mut placer = Placer::new(timeline, opts.grid, count_in);

    for i in 0..lrc.lines.len() {
        let line = &lrc.lines[i];
        if line.text.trim().is_empty() {
            continue;
        }
        let start = line.start as f64;
        let unplaced = |reason: &str| Unplaced {
            time: Some(start),
            bar: None,
            text: line.text.clone(),
            reason: reason.into(),
        };
        match placer.grid.locate(start) {
            Locate::After => {
                placer
                    .out
                    .unplaced
                    .push(unplaced("after the last bar of the chart"));
                continue;
            }
            Locate::At(s) if s >= placer.first_real_step => {}
            _ => {
                placer.out.unplaced.push(unplaced(BEFORE));
                continue;
            }
        }

        let mut placed_any = false;
        let mut spilled: Vec<String> = Vec::new();
        for (t, word) in timed_words(lrc, i, timeline, opts) {
            let step = match placer.grid.locate(t) {
                Locate::At(s) => s,
                Locate::Before => 0,
                Locate::After => placer.grid.total,
            };
            match placer.put(step, &word) {
                Put::Placed => placed_any = true,
                Put::Skipped => {}
                Put::BeforeStart | Put::PastEnd => spilled.push(word),
            }
        }
        placer.end_line();
        if placed_any {
            placer.out.placed_lines += 1;
        }
        if !spilled.is_empty() {
            placer.out.unplaced.push(Unplaced {
                time: Some(start),
                bar: None,
                text: spilled.join(" "),
                reason: "ran past the last bar of the chart".into(),
            });
        }
    }
    placer.finish()
}

/// A word placed by bar rather than by time: from a chord-over-lyrics
/// sheet or a hand-made bar/beat map.
#[derive(Debug, Clone, PartialEq)]
pub struct BarWord {
    /// Printed bar number: 1 is the first bar after any count-in.
    pub bar: usize,
    /// 1-based beat, if known. Words without one share their bar evenly.
    pub beat: Option<f32>,
    /// The word; a trailing `-` joins it to the next one, `_` is a held
    /// syllable (not drawn).
    pub text: String,
    /// The last word of a source line.
    pub line_end: bool,
}

/// Put words given by bar (and maybe beat) into the chart's lanes.
///
/// A word with a beat goes on the nearest grid step to it; the words of a
/// bar that have none are spread evenly across it. Order is kept: a word
/// never lands on or before the word before it.
pub fn place_bar_words(chart: &Chart, words: &[BarWord], opts: &PlaceOptions) -> Placement {
    let timeline = Timeline::from_chart(chart, 0.0);
    let count_in = count_in_bars(chart);
    let mut placer = Placer::new(&timeline, opts.grid, count_in);
    let real_bars = timeline.bars.len().saturating_sub(count_in);

    let mut lines = 0usize;
    let mut line_has_word = false;
    let mut i = 0;
    while i < words.len() {
        let bar = words[i].bar;
        let j = i + words[i..].iter().take_while(|w| w.bar == bar).count();
        let run = &words[i..j];
        i = j;

        if bar == 0 || bar > real_bars {
            for w in run {
                placer.out.unplaced.push(Unplaced {
                    time: None,
                    bar: Some(bar),
                    text: w.text.clone(),
                    reason: if bar == 0 {
                        "bar 0: bars are numbered from 1".into()
                    } else {
                        format!("past the last bar ({real_bars})")
                    },
                });
                if w.line_end {
                    placer.end_line();
                }
            }
            continue;
        }

        let k = count_in + bar - 1;
        let steps = timeline.steps(k, placer.grid.grid) as f32;
        let (_, d) = timeline.bars[k].meter;
        let per_beat = placer.grid.grid as f32 / d as f32;
        let first = placer.grid.first_step[k];
        let unbeated = run.iter().filter(|w| w.beat.is_none()).count().max(1) as f32;
        let mut nth = 0f32;
        for w in run {
            let within = match w.beat {
                Some(b) => ((b - 1.0).max(0.0) * per_beat).round(),
                None => {
                    let s = (nth * steps / unbeated).round();
                    nth += 1.0;
                    s
                }
            };
            let step = first + within.clamp(0.0, steps - 1.0) as u64;
            match placer.put(step, &w.text) {
                Put::Placed => line_has_word = true,
                Put::Skipped => {}
                Put::BeforeStart | Put::PastEnd => placer.out.unplaced.push(Unplaced {
                    time: None,
                    bar: Some(bar),
                    text: w.text.clone(),
                    reason: "pushed past the last bar by the words before it".into(),
                }),
            }
            if w.line_end {
                placer.end_line();
                lines += usize::from(line_has_word);
                line_has_word = false;
            }
        }
    }
    lines += usize::from(line_has_word);
    let mut out = placer.finish();
    out.placed_lines = lines;
    out
}

/// Read a bar/beat/word map: one word per line, `bar beat word` (or
/// `bar word` to share the bar evenly), bars numbered as printed. `#`
/// starts a comment; a blank line or a `##` heading ends a lyric line.
///
/// ```text
/// ## VS 1
///   1   1   Si-
///   1   2.5 lent
///   2   1   ____
/// ```
pub fn read_word_map(text: &str) -> Result<Vec<BarWord>, String> {
    let mut words: Vec<BarWord> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let ends_line = line.is_empty() || line.starts_with("##");
        let line = match line.find('#') {
            Some(i) => line[..i].trim(),
            None => line,
        };
        if line.is_empty() {
            if ends_line && let Some(last) = words.last_mut() {
                last.line_end = true;
            }
            continue;
        }
        let mut toks = line.split_whitespace();
        let bar: usize = toks
            .next()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| format!("line {}: expected `bar beat word`, got {raw:?}", n + 1))?;
        let rest: Vec<&str> = toks.collect();
        let (beat, word) = match rest.as_slice() {
            [beat, word @ ..] if !word.is_empty() && beat.parse::<f32>().is_ok() => {
                (beat.parse::<f32>().ok(), word.join(" "))
            }
            [] => return Err(format!("line {}: no word", n + 1)),
            word => (None, word.join(" ")),
        };
        words.push(BarWord {
            bar,
            beat,
            text: word,
            line_end: false,
        });
    }
    if let Some(last) = words.last_mut() {
        last.line_end = true;
    }
    Ok(words)
}

/// Could this token be a chord symbol on a chord-over-lyrics sheet?
fn is_chord_token(tok: &str) -> bool {
    let t = tok.trim_matches(|c| c == '(' || c == ')');
    let mut chars = t.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !('A'..='G').contains(&first) {
        return false;
    }
    // Letters a chord spelling uses (maj, min, m, sus, add, dim, aug, M)
    // and the slash bass; a lyric word almost always has another.
    chars.all(|c| {
        c.is_ascii_digit() || "majinsudgM#b+-/()°øΔ".contains(c) || ('A'..='G').contains(&c)
    })
}

/// Tokens a chord line may carry that are not chords.
fn is_chord_line_filler(tok: &str) -> bool {
    matches!(tok, "|" | "||" | "-" | "/" | "%" | "N.C." | "NC" | "N.C")
        || tok
            .trim_matches(|c| c == '(' || c == ')')
            .starts_with(['x', 'X'])
            && tok
                .trim_matches(|c| c == '(' || c == ')')
                .chars()
                .skip(1)
                .all(|c| c.is_ascii_digit())
}

/// The chords of a chord line, each with the column it starts at, or
/// `None` when the line is not one.
fn chord_columns(line: &str) -> Option<Vec<usize>> {
    let mut cols = Vec::new();
    let mut any = false;
    for (col, tok) in tokens_with_columns(line) {
        if is_chord_token(tok) {
            cols.push(col);
            any = true;
        } else if !is_chord_line_filler(tok) {
            return None;
        }
    }
    any.then_some(cols)
}

/// Whitespace-separated tokens with their (character) column.
fn tokens_with_columns(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<(usize, usize)> = None; // (col, byte)
    let mut col = 0;
    for (byte, c) in line.char_indices() {
        if c.is_whitespace() {
            if let Some((sc, sb)) = start.take() {
                out.push((sc, &line[sb..byte]));
            }
        } else if start.is_none() {
            start = Some((col, byte));
        }
        col += 1;
    }
    if let Some((sc, sb)) = start {
        out.push((sc, &line[sb..]));
    }
    out
}

/// A sheet heading (`[Verse 1]`, `Chorus:`) and the name in it.
fn sheet_heading(line: &str) -> Option<String> {
    let t = line.trim();
    let name = if let Some(inner) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        inner
    } else if let Some(inner) = t.strip_suffix(':') {
        if inner.split_whitespace().count() > 3 {
            return None;
        }
        inner
    } else {
        return None;
    };
    Some(name.trim().to_string())
}

/// Does chart section `s` answer to the sheet heading `name`?
fn heading_matches(name: &str, s: &SectionType) -> bool {
    let first: String = name
        .split(|c: char| c.is_whitespace() || c.is_ascii_digit())
        .find(|w| !w.is_empty())
        .unwrap_or("")
        .to_lowercase();
    let first = first.trim_end_matches(['-', ':']);
    let want = match first {
        "verse" | "vs" | "v" => "verse",
        "chorus" | "ch" | "c" => "chorus",
        "bridge" | "br" => "bridge",
        "intro" | "in" => "intro",
        "outro" | "out" | "ending" => "outro",
        "instrumental" | "inst" | "interlude" | "solo" => "instrumental",
        "pre-chorus" | "prechorus" | "pre" => "pre_chorus",
        other => other,
    };
    let key = s.key();
    key == want
        || key.starts_with(want)
        || (want == "instrumental" && matches!(key.as_str(), "interlude" | "solo"))
        || s.full_name().to_lowercase().starts_with(want)
}

/// Read a chord-over-lyrics sheet: a line of chords with the lyric line
/// under it, each chord one bar of the chart, in order. A word belongs to
/// the bar of the chord at or before its column; words before the line's
/// first chord are a pickup, sung in the bar before. A chord line with no
/// lyrics under it is bars of playing. A heading (`[Chorus]`) moves on to
/// the chart's next section of that kind.
///
/// Returns the words and notes on what it could not match.
pub fn read_chord_sheet(text: &str, chart: &Chart) -> (Vec<BarWord>, Vec<String>) {
    let count_in = count_in_bars(chart);
    // Printed first bar of each section.
    let mut first_bar = Vec::with_capacity(chart.sections.len());
    let mut bar = 1usize;
    let mut global = 0usize;
    for s in &chart.sections {
        first_bar.push(bar);
        let n = s.measures().len();
        global += n;
        bar = global.saturating_sub(count_in) + 1;
    }

    let mut words = Vec::new();
    let mut notes = Vec::new();
    let mut cursor = 1usize; // the next chord's bar
    let mut last_bar = 1usize;
    let mut next_section = 0usize;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(name) = sheet_heading(line) {
            match (next_section..chart.sections.len())
                .find(|&s| heading_matches(&name, &chart.sections[s].section.section_type))
            {
                Some(s) => {
                    cursor = first_bar[s];
                    next_section = s + 1;
                }
                None => notes.push(format!(
                    "heading [{name}]: no matching section left in the chart; carrying on from bar {cursor}"
                )),
            }
            continue;
        }
        if let Some(cols) = chord_columns(line) {
            // Lyrics under it?
            let lyric = lines
                .get(i)
                .filter(|l| !l.trim().is_empty())
                .filter(|l| chord_columns(l).is_none() && sheet_heading(l).is_none());
            if let Some(lyric) = lyric {
                i += 1;
                let toks = tokens_with_columns(lyric);
                let n = toks.len();
                for (k, (col, word)) in toks.into_iter().enumerate() {
                    let bar = match cols.iter().rposition(|&c| c <= col) {
                        Some(idx) => cursor + idx,
                        None => cursor.saturating_sub(1).max(1),
                    };
                    words.push(BarWord {
                        bar,
                        beat: None,
                        text: word.to_string(),
                        line_end: k + 1 == n,
                    });
                }
            }
            last_bar = cursor + cols.len().saturating_sub(1);
            cursor += cols.len();
            continue;
        }
        // A lyric line with no chords over it: it goes on in the last
        // chord's bar.
        let toks = tokens_with_columns(line);
        let n = toks.len();
        for (k, (_, word)) in toks.into_iter().enumerate() {
            words.push(BarWord {
                bar: last_bar,
                beat: None,
                text: word.to_string(),
                line_end: k + 1 == n,
            });
        }
    }
    (words, notes)
}

/// Byte ranges of each section's body in `source`: from the line after its
/// header to the next header. Sections with no written header (an implicit
/// intro) have none.
fn section_bodies(source: &str, chart: &Chart) -> Vec<Option<(usize, usize)>> {
    let headers: Vec<Option<usize>> = chart
        .sections
        .iter()
        .map(|s| s.source_span.as_ref().map(|span| span.start))
        .collect();
    let mut starts: Vec<usize> = headers.iter().flatten().copied().collect();
    starts.sort_unstable();
    headers
        .iter()
        .map(|h| {
            let h = (*h)?;
            let body = source[h..]
                .find('\n')
                .map(|n| h + n + 1)
                .unwrap_or(source.len());
            let end = starts
                .iter()
                .copied()
                .find(|&s| s > h)
                .unwrap_or(source.len());
            Some((body, end.max(body)))
        })
        .collect()
}

/// Is this line the start of a lyric track we generated?
fn is_lane_marker(line: &str) -> bool {
    let t = line.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|r| r.split_once(']')) else {
        return false;
    };
    let mut words = inner.0.split_whitespace();
    matches!(
        words.next().map(str::to_ascii_lowercase).as_deref(),
        Some("lyrics" | "lyric" | "words")
    ) && words.next() == Some(LANE_NAME)
        && words.next().is_none()
}

/// A section body without its generated lyric lane: the marker line and
/// the lane lines after it, up to a blank line or the next track marker.
fn strip_lane(body: &str) -> String {
    let mut out = String::new();
    let mut in_lane = false;
    for line in body.split_inclusive('\n') {
        if is_lane_marker(line) {
            in_lane = true;
            continue;
        }
        if in_lane {
            let t = line.trim();
            if t.is_empty() || t.starts_with('[') {
                in_lane = false;
            } else {
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

/// Write `placement`'s lanes into `source`, the text `chart` was parsed
/// from. Each section's previous `[lyrics lrc]` lane is removed and the new
/// one appended after the section's last line, so running this twice gives
/// the same file as running it once. Chords and everything else are left
/// as they were.
pub fn write_lanes(source: &str, chart: &Chart, placement: &Placement) -> Result<String, String> {
    let bodies = section_bodies(source, chart);
    for section in placement.lanes.keys() {
        if bodies.get(*section).copied().flatten().is_none() {
            return Err(format!(
                "section {} has lyrics but no header line to write them under",
                section + 1
            ));
        }
    }

    let mut out = String::with_capacity(source.len() + 4096);
    let mut at = 0usize;
    let mut ordered: Vec<(usize, (usize, usize))> = bodies
        .iter()
        .enumerate()
        .filter_map(|(s, b)| b.map(|b| (s, b)))
        .collect();
    ordered.sort_by_key(|(_, (start, _))| *start);
    // Two sections never share a body; a recalled header is its own section.
    ordered.dedup_by_key(|(_, (start, _))| *start);

    for (section, (start, end)) in ordered {
        out.push_str(&source[at..start]);
        let body = strip_lane(&source[start..end]);
        match placement.lanes.get(&section) {
            Some(lane) if !lane.syllables.is_empty() => {
                // The lane goes after the section's last non-blank line.
                let content_len = body.trim_end().len();
                let (content, tail) = body.split_at(content_len);
                out.push_str(content);
                if !content.is_empty() {
                    out.push('\n');
                }
                out.push('[');
                out.push_str("lyrics ");
                out.push_str(LANE_NAME);
                out.push_str("]\n");
                out.push_str(&lane.to_timed_text());
                // Keep the blank line(s) that separated this section from
                // the next; a body that ended the file gets a final newline.
                let tail = tail.trim_start_matches(|c| c == ' ' || c == '\t');
                if tail.is_empty() {
                    out.push('\n');
                } else if tail.starts_with('\n') {
                    out.push_str(tail);
                } else {
                    out.push('\n');
                    out.push_str(tail);
                }
            }
            _ => out.push_str(&body),
        }
        at = end;
    }
    out.push_str(&source[at..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllable_estimate_is_rough_but_ordered() {
        assert_eq!(syllables("the"), 1);
        assert_eq!(syllables("singing"), 2);
        assert!(syllables("everlasting") >= 3);
    }

    #[test]
    fn words_lose_what_the_chart_language_would_misread() {
        assert_eq!(clean_word("\"hello;\""), "hello,");
        assert_eq!(clean_word("@home"), "home");
        assert_eq!(clean_word("[x]"), "x");
    }

    #[test]
    fn only_our_lane_marker_is_ours() {
        assert!(is_lane_marker("[lyrics lrc]"));
        assert!(is_lane_marker("  [Lyrics lrc]  "));
        assert!(!is_lane_marker("[lyrics]"));
        assert!(!is_lane_marker("[lyrics lead]"));
        assert!(!is_lane_marker("[Build] 4"));
    }
}
