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

impl Timeline {
    /// The chart's own clock: its tempo and meters, with bar 1 starting
    /// `downbeat` seconds into the recording.
    pub fn from_chart(chart: &Chart, downbeat: f64) -> Self {
        let tempos = section_tempos(chart);
        let mut t = downbeat;
        let bars = chart_bars(chart)
            .into_iter()
            .map(|(section, index, meter)| {
                let secs = quarters(meter) * 60.0 / tempos[section];
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

    /// A measured clock: `starts[k]` is when the chart's bar `k` (0-based,
    /// across the whole chart) starts — a Songsterr sync, say. A recording
    /// that drifts stays in step this way where a single tempo cannot. Bars
    /// past the end of `starts` continue at the chart's tempo.
    pub fn from_bar_starts(chart: &Chart, starts: &[f64]) -> Self {
        let tempos = section_tempos(chart);
        let layout = chart_bars(chart);
        let mut bars = Vec::with_capacity(layout.len());
        let mut t = starts.first().copied().unwrap_or(0.0);
        for (k, (section, index, meter)) in layout.into_iter().enumerate() {
            let nominal = quarters(meter) * 60.0 / tempos[section];
            let start = starts.get(k).copied().unwrap_or(t);
            let end = starts
                .get(k + 1)
                .copied()
                .filter(|&e| e > start)
                .unwrap_or(start + nominal);
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
    pub time: f64,
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

/// Put every lyric line of `lrc` on the timeline.
///
/// Words land on the nearest grid step; a word never lands on or before
/// the step of the word before it, so two words never share a place and the
/// order always survives. A line that starts before bar 1 or after the
/// last bar is reported rather than dropped, and so are the words of a line
/// that run past the end of the chart.
pub fn place_lrc(lrc: &Lrc, timeline: &Timeline, opts: &PlaceOptions) -> Placement {
    let grid = Grid::new(timeline, opts.grid.max(1));
    let mut out = Placement::default();
    let mut last_step: Option<u64> = None;

    for i in 0..lrc.lines.len() {
        let line = &lrc.lines[i];
        if line.text.trim().is_empty() {
            continue;
        }
        let start = line.start as f64;
        match grid.locate(start) {
            Locate::Before => {
                out.unplaced.push(Unplaced {
                    time: start,
                    text: line.text.clone(),
                    reason: "before bar 1 (check --downbeat / the sync)".into(),
                });
                continue;
            }
            Locate::After => {
                out.unplaced.push(Unplaced {
                    time: start,
                    text: line.text.clone(),
                    reason: "after the last bar of the chart".into(),
                });
                continue;
            }
            Locate::At(_) => {}
        }

        let mut placed_any = false;
        let mut spilled: Vec<String> = Vec::new();
        // Section → syllables of this line in that section.
        let mut pieces: BTreeMap<usize, Vec<LyricSyllable>> = BTreeMap::new();
        for (t, word) in timed_words(lrc, i, timeline, opts) {
            let word = clean_word(&word);
            if word.is_empty() {
                continue;
            }
            let step = match grid.locate(t) {
                Locate::At(s) => s,
                Locate::Before => 0,
                Locate::After => grid.total,
            };
            let step = match last_step {
                Some(prev) if step <= prev => prev + 1,
                _ => step,
            };
            let Some((k, beat)) = grid.place(step) else {
                spilled.push(word);
                continue;
            };
            last_step = Some(step);
            let bar = &timeline.bars[k];
            let mut syl = LyricSyllable::with_timing(word, bar.index, beat);
            syl.word_initial = true;
            pieces.entry(bar.section).or_default().push(syl);
            placed_any = true;
        }
        for (section, mut syls) in pieces {
            if let Some(last) = syls.last_mut() {
                last.line_break_after = true;
            }
            let lane = out.lanes.entry(section).or_insert_with(|| {
                let mut l = LyricLine::new(Vec::new());
                l.timed = true;
                l
            });
            lane.syllables.extend(syls);
        }
        if placed_any {
            out.placed_lines += 1;
        }
        if !spilled.is_empty() {
            out.unplaced.push(Unplaced {
                time: start,
                text: spilled.join(" "),
                reason: "ran past the last bar of the chart".into(),
            });
        }
    }
    out
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
