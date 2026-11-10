//! When each bar of a chart is played, in seconds of its recording.
//!
//! [`bar_times`] walks the chart in playing order — a `|: … :|` span as
//! many times as it says, first and second endings on their own passes —
//! and puts every bar on the clock from the chart's tempo, its `->140bpm`
//! changes and its meters, with bar 1 (the first bar after any `Count`)
//! at [`BarClock::downbeat`]. A measured start per bar (a Songsterr sync)
//! replaces the computed one where it is given ([`BarClock::measured`]):
//! a recording that drifts stays in step that way where one tempo cannot.
//!
//! The bars are the ones the engraver draws: a `Count` section is the
//! count-in, not a bar, and an `End` section draws nothing. So
//! [`BarTime::bar`] is the engraver's own bar index
//! (`BeatPosition::measure`), and a bar that a repeat plays twice is in the
//! list twice, once per pass.

use std::collections::BTreeMap;

use keyflow_proto::chart::Measure;
use keyflow_proto::chart::notations::RepeatMark;

use crate::text::chart::Chart;
use crate::text::sections::SectionType;

/// The tempo a chart without one is laid out at — the engraver's default.
pub const DEFAULT_BPM: f64 = 120.0;

/// Where the chart's bars fall in its recording.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BarClock<'a> {
    /// Seconds into the recording where bar 1 starts.
    pub downbeat: f64,
    /// Measured start times: `measured[k]` is when the `k`-th bar played
    /// (in playing order, from bar 1) starts. Bars past its end continue at
    /// the chart's tempo.
    pub measured: Option<&'a [f64]>,
}

impl BarClock<'_> {
    /// The chart's own clock, with bar 1 at `downbeat` seconds.
    #[must_use]
    pub fn at(downbeat: f64) -> Self {
        Self {
            downbeat,
            measured: None,
        }
    }
}

/// One bar as it is played.
#[derive(Debug, Clone, PartialEq)]
pub struct BarTime {
    /// The bar as the engraver numbers it: 0-based over every drawn bar of
    /// the chart (no count-in), the index `BeatPosition::measure` carries.
    pub bar: usize,
    /// Index into `chart.sections`.
    pub section: usize,
    /// 0-based bar within its section.
    pub index: usize,
    /// Which time through a repeat this is (1 outside repeats).
    pub pass: u32,
    /// Seconds from the start of the recording.
    pub start: f64,
    pub end: f64,
}

/// One drawn bar: where it is, how long it lasts in quarter notes, and the
/// tempo changes inside it (`(quarters into the bar, bpm)`).
struct Drawn<'c> {
    section: usize,
    index: usize,
    measure: &'c Measure,
    tempo_at_start: f64,
    changes: Vec<(f64, f64)>,
}

/// Every bar of `chart` in playing order, with its start and end time.
///
/// Empty for a chart with no bars.
#[must_use]
pub fn bar_times(chart: &Chart, clock: BarClock<'_>) -> Vec<BarTime> {
    let drawn = drawn_bars(chart);
    let order = playing_order(&drawn);

    let mut out = Vec::with_capacity(order.len());
    let mut t = clock.downbeat;
    for (k, (bar, pass)) in order.into_iter().enumerate() {
        let d = &drawn[bar];
        let start = clock.measured.and_then(|m| m.get(k).copied()).unwrap_or(t);
        let nominal = seconds(d);
        let end = clock
            .measured
            .and_then(|m| m.get(k + 1).copied())
            .filter(|&e| e > start)
            .unwrap_or(start + nominal);
        out.push(BarTime {
            bar,
            section: d.section,
            index: d.index,
            pass,
            start,
            end,
        });
        t = end;
    }
    out
}

/// The bars the engraver draws, in score order, each with its tempo.
fn drawn_bars(chart: &Chart) -> Vec<Drawn<'_>> {
    // Tempo changes by (section, bar in section), in written order.
    let mut changes: BTreeMap<(usize, usize), Vec<(f64, f64)>> = BTreeMap::new();
    for change in &chart.tempo_changes {
        let pos = &change.position.total_duration;
        let bar = usize::try_from(pos.measure).unwrap_or(0);
        let beat = f64::from(pos.beat.max(0));
        changes
            .entry((change.section_index, bar))
            .or_default()
            .push((beat, change.to_tempo.bpm));
    }

    // `chart.tempo` is the tempo in force at the END of the chart (the
    // parser moves it on at each `->`), so the opening tempo is the one the
    // first change moved away from.
    let opening = match chart.tempo_changes.first() {
        Some(first) => first.from_tempo.as_ref(),
        None => chart.tempo.as_ref(),
    };
    let mut bpm = opening
        .map(|t| t.bpm)
        .filter(|b| *b > 0.0)
        .unwrap_or(DEFAULT_BPM);
    let mut out = Vec::new();
    for (s, section) in chart.sections.iter().enumerate() {
        let drawn = !section.section.section_type.is_compact()
            && !matches!(section.section.section_type, SectionType::End);
        for (i, measure) in section.measures().iter().enumerate() {
            let (n, d) = meter(measure);
            // A change on the bar's last beat or later (`G /// ->140`) is
            // written at the next bar's downbeat: carry it there.
            let mut inside = Vec::new();
            let mut at_start = bpm;
            let mut carried = None;
            for &(beat, to) in changes.get(&(s, i)).into_iter().flatten() {
                if to <= 0.0 {
                    continue;
                }
                if beat <= 0.0 {
                    at_start = to;
                } else if beat < f64::from(n) {
                    inside.push((beat * 4.0 / f64::from(d), to));
                } else {
                    carried = Some(to);
                    continue;
                }
                bpm = to;
            }
            if let Some(to) = carried {
                bpm = to;
            }
            if drawn {
                out.push(Drawn {
                    section: s,
                    index: i,
                    measure,
                    tempo_at_start: at_start,
                    changes: inside,
                });
            }
        }
    }
    out
}

/// A bar's meter, with a missing one read as 4/4.
fn meter(measure: &Measure) -> (u8, u8) {
    match measure.time_signature {
        (0, _) | (_, 0) => (4, 4),
        m => m,
    }
}

/// How long a drawn bar lasts at its tempo, changes inside it included.
fn seconds(bar: &Drawn<'_>) -> f64 {
    let (n, d) = meter(bar.measure);
    let quarters = f64::from(n) * 4.0 / f64::from(d);
    let mut total = 0.0;
    let mut from = 0.0;
    let mut bpm = bar.tempo_at_start;
    for &(at, to) in &bar.changes {
        let at = at.min(quarters);
        total += (at - from) * 60.0 / bpm;
        from = at;
        bpm = to;
    }
    total + (quarters - from) * 60.0 / bpm
}

/// The drawn bars in the order they are played, as `(bar, pass)`.
///
/// Repeats stay inside a section, as the engraver draws them: a `:|`
/// goes back to the last `|:` in its section, or the section's first bar.
/// A bar under an ending (`[1]`, `[2:2]`) plays only on the passes it
/// names; after the last pass the bars that follow play once.
fn playing_order(drawn: &[Drawn<'_>]) -> Vec<(usize, u32)> {
    let mut order = Vec::with_capacity(drawn.len());
    let mut first = 0;
    while first < drawn.len() {
        let section = drawn[first].section;
        let mut last = first;
        while last + 1 < drawn.len() && drawn[last + 1].section == section {
            last += 1;
        }
        section_order(&drawn[first..=last], first, &mut order);
        first = last + 1;
    }
    order
}

/// Play one section's bars (`bars`, the first of which is drawn bar
/// `base`) into `order`.
fn section_order(bars: &[Drawn<'_>], base: usize, order: &mut Vec<(usize, u32)>) {
    // No repeat sign anywhere: every bar once, the common case.
    let marked = bars
        .iter()
        .any(|b| b.measure.end_repeat == RepeatMark::Backward || b.measure.volta_start.is_some());
    if !marked {
        order.extend((0..bars.len()).map(|i| (base + i, 1)));
        return;
    }

    // Which bars an ending covers, and on which passes.
    let mut ending: Vec<Option<&[u8]>> = vec![None; bars.len()];
    for (i, b) in bars.iter().enumerate() {
        if let Some(volta) = &b.measure.volta_start {
            let len = usize::from(volta.length_measures.max(1));
            for slot in ending.iter_mut().skip(i).take(len) {
                *slot = Some(volta.numbers.as_slice());
            }
        }
    }

    let mut span_start = 0;
    let mut pass: u32 = 1;
    // Set once a repeat has played all its passes: the endings right after
    // it still read the last pass; the first bar outside them starts afresh.
    let mut after_repeat = false;
    let mut i = 0;
    // Each repeat jumps back at most `repeat_count - 1` times; bound the walk
    // anyway so a malformed chart cannot spin.
    let mut steps = 0usize;
    let limit = bars.len().saturating_mul(64).max(64);
    while i < bars.len() && steps < limit {
        steps += 1;
        let m = bars[i].measure;
        if after_repeat && ending[i].is_none() {
            after_repeat = false;
            pass = 1;
            span_start = i;
        }
        if m.start_repeat == RepeatMark::Forward && pass == 1 && !after_repeat {
            span_start = i;
        }
        if let Some(numbers) = ending[i] {
            let p = u8::try_from(pass).unwrap_or(u8::MAX);
            if !numbers.contains(&p) {
                // Skipping the ending that closes the repeat (a first
                // ending on the last pass) is the repeat done.
                if m.end_repeat == RepeatMark::Backward {
                    after_repeat = true;
                }
                i += 1;
                continue;
            }
        }
        order.push((base + i, pass));
        if m.end_repeat == RepeatMark::Backward {
            let passes = u32::try_from(m.repeat_count.max(2)).unwrap_or(2);
            if pass < passes {
                pass += 1;
                i = span_start;
                continue;
            }
            after_repeat = true;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::chart::parse_chart;

    fn times(src: &str, downbeat: f64) -> Vec<(usize, u32, f64)> {
        let chart = parse_chart(src).expect("chart parses");
        bar_times(&chart, BarClock::at(downbeat))
            .into_iter()
            .map(|b| (b.bar, b.pass, (b.start * 1000.0).round() / 1000.0))
            .collect()
    }

    fn starts(src: &str, downbeat: f64) -> Vec<f64> {
        times(src, downbeat)
            .into_iter()
            .map(|(_, _, t)| t)
            .collect()
    }

    #[test]
    fn constant_tempo_from_the_downbeat() {
        // 120 bpm in 4/4: two seconds a bar, bar 1 at 1.5 s.
        let src = "Song - Artist\n120bpm 4/4 #C\n\nVS 4\nC F G C\n";
        assert_eq!(starts(src, 1.5), vec![1.5, 3.5, 5.5, 7.5]);
    }

    #[test]
    fn a_count_in_is_not_a_bar() {
        let src = "Song - Artist\n120bpm 4/4 #C\n\nCount 2\n\nVS 2\nC G\n";
        let t = times(src, 0.0);
        assert_eq!(t, vec![(0, 1, 0.0), (1, 1, 2.0)]);
    }

    #[test]
    fn a_meter_change_shortens_its_bars() {
        // 60 bpm: a 4/4 bar is 4 s, a 3/4 bar 3 s, a 6/8 bar 3 s.
        let src = "Song - Artist\n60bpm 4/4 #C\n\nVS 2\nC G\n\nCH 2\nT3/4 F C\n\nBR 2\nT6/8 D A\n";
        assert_eq!(starts(src, 0.0), vec![0.0, 4.0, 8.0, 11.0, 14.0, 17.0]);
    }

    #[test]
    fn a_tempo_change_mid_section_takes_effect_at_its_bar() {
        // 120 bpm (2 s bars) for two bars, then 60 bpm (4 s bars), written
        // on a second line of the section so the bar is counted from the
        // section, not the line.
        let src = "Song - Artist\n120bpm 4/4 #C\n\nVS 4\nC G\n->60bpm F C\n\nCH 1\nG\n";
        assert_eq!(starts(src, 0.0), vec![0.0, 2.0, 4.0, 8.0, 12.0]);
    }

    #[test]
    fn a_tempo_change_inside_a_bar_splits_it() {
        // Two beats at 120 (1 s), two at 60 (2 s): the bar is 3 s.
        let src = "Song - Artist\n120bpm 4/4 #C\n\nVS 2\nC // ->60bpm G // F\n";
        assert_eq!(starts(src, 0.0), vec![0.0, 3.0]);
    }

    #[test]
    fn a_repeat_plays_its_bars_twice() {
        // `|: C G :|` marks the wrong bar (a parser bug the keyflow skill lists);
        // a `|` between the bars puts the signs where they are written.
        let src = "Song - Artist\n120bpm 4/4 #C\n\nVS 4\n|: C | G :| F C\n";
        let t = times(src, 0.0);
        assert_eq!(
            t,
            vec![
                (0, 1, 0.0),
                (1, 1, 2.0),
                (0, 2, 4.0),
                (1, 2, 6.0),
                (2, 1, 8.0),
                (3, 1, 10.0),
            ]
        );
    }

    #[test]
    fn endings_play_on_their_own_pass() {
        // |: C [1] G :| [2] F |  — pass 1: C G, pass 2: C F.
        let mut chart = parse_chart("Song - Artist\n120bpm 4/4 #C\n\nVS 4\nC G F D\n").unwrap();
        {
            let bars = chart.sections[0].measures_mut();
            bars[0].start_repeat = RepeatMark::Forward;
            bars[1].volta_start = Some(volta(&[1]));
            bars[1].end_repeat = RepeatMark::Backward;
            bars[1].repeat_count = 2;
            bars[2].volta_start = Some(volta(&[2]));
        }
        let order: Vec<(usize, u32)> = bar_times(&chart, BarClock::at(0.0))
            .into_iter()
            .map(|b| (b.bar, b.pass))
            .collect();
        assert_eq!(order, vec![(0, 1), (1, 1), (0, 2), (2, 2), (3, 1)]);
    }

    #[test]
    fn measured_starts_win_and_the_tempo_carries_on_past_them() {
        let chart = parse_chart("Song - Artist\n120bpm 4/4 #C\n\nVS 4\nC F G C\n").unwrap();
        let measured = [10.0, 12.5];
        let t: Vec<f64> = bar_times(
            &chart,
            BarClock {
                downbeat: 0.0,
                measured: Some(&measured),
            },
        )
        .into_iter()
        .map(|b| b.start)
        .collect();
        assert_eq!(t, vec![10.0, 12.5, 14.5, 16.5]);
    }

    fn volta(numbers: &[u8]) -> keyflow_proto::chart::notations::Volta {
        keyflow_proto::chart::notations::Volta {
            numbers: numbers.to_vec(),
            label: String::new(),
            length_measures: 1,
        }
    }
}
