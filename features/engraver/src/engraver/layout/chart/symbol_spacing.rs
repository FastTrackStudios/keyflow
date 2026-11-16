//! Chord symbols ride on the slash they are played on.
//!
//! A chord symbol's left edge sits over the rhythm slash (or notehead) that
//! carries its onset. When two symbols would collide, the *beats* between them
//! widen — the symbol never drifts off its beat to make room. That needs the
//! same answer in three places, so it lives here:
//!
//! - the measuring pass asks how wide a bar must be for its symbols
//!   ([`symbol_body_min_width`]),
//! - the measure layout asks how wide each rhythm segment must be
//!   ([`symbol_segment_minimums`]),
//! - the symbol renderer asks which segment a chord's onset is
//!   ([`chord_onset_tick`] against the segments' ticks).
//!
//! Widths are the symbol's *rendered* extent — the same
//! [`harmony_layout`] the renderer draws — not a plain-text estimate of the
//! symbol string. `Bbmaj7` prints as `B♭△7` with a superscript, about half the
//! width of the six full-size letters, and estimating with the letters is
//! what made a bar of eight chord changes ask for a whole line.

use crate::chart::types::{ChordInstance, Measure, RhythmElement};
use crate::engraver::layout::tlayout::{HarmonyStyle, harmony_layout};
use crate::time::{MusicalPositionExt as _, TimeSignature};

use super::{chord_layout, chord_renderer, constants, spacing};

/// How far a chord symbol's ink reaches either side of its anchor (the left
/// edge of the slash it sits on), in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolExtent {
    /// Ink to the left of the anchor (a vertical-bass rule, say). Usually 0.
    pub left: f64,
    /// Ink to the right of the anchor: the symbol's printed width.
    pub right: f64,
}

/// A chord symbol that prints in its bar, and where.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrintedSymbol {
    /// Index into `measure.chords`.
    pub chord_idx: usize,
    /// Onset within the bar, in ticks (quarter = 480).
    pub onset: i32,
    /// Printed extent.
    pub extent: SymbolExtent,
    /// 1.0 when a slash chord prints as a vertical fraction, else 0.0 — see
    /// [`slash_stack_for_span`].
    pub stack: f64,
}

/// Whether a slash chord prints as a vertical fraction (1.0) or inline
/// (0.0), from the time it has before the next symbol (or the barline), in
/// ticks (quarter = 480). Less than a beat and it stacks: `A` over a rule
/// over `E`, as a hand-written chart would squeeze it.
#[must_use]
pub fn slash_stack_for_span(span_ticks: i32) -> f64 {
    if f64::from(span_ticks) < constants::TICKS_PER_QUARTER {
        1.0
    } else {
        0.0
    }
}

/// How much room symbols ask for, beyond their own ink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolSpacing {
    /// Staff space, in points.
    pub spatium: f64,
    /// Horizontal clearance between one symbol's ink and the next's, in
    /// points. Must be at least what the renderer's collision pass keeps, or
    /// that pass will push a symbol off its beat.
    pub clearance: f64,
}

/// Padding, in staff spaces, the renderer's collision pass adds to each side
/// of a chord symbol's layout box before asking for the minimum gap between
/// two. Small: the layout box already carries the glyphs' side bearings, and
/// every point of it costs room a four-bar line of chord clusters needs.
pub const COLLISION_PADDING_SPATIUMS: f64 = 0.2;

impl SymbolSpacing {
    /// The clearance the renderer keeps: its collision pass pads every
    /// symbol by [`COLLISION_PADDING_SPATIUMS`] each side and then asks for
    /// `min_gap` between the padded boxes.
    #[must_use]
    pub fn new(spatium: f64, min_gap: f64) -> Self {
        Self {
            spatium,
            clearance: 2.0 * COLLISION_PADDING_SPATIUMS * spatium + min_gap.max(0.0),
        }
    }
}

/// Ticks in one beat of `time_signature` (a quarter in 4/4, an eighth in 6/8).
#[must_use]
pub fn ticks_per_beat(time_signature: (u8, u8)) -> f64 {
    constants::TICKS_PER_QUARTER * 4.0 / f64::from(time_signature.1.max(1))
}

/// Onset beat of the chord at `chord_idx` within its OWN measure, from the
/// cumulative duration of the elements before it. Drift-free, unlike
/// `position.beats()` (the absolute-timeline beat), which a short earlier
/// measure can shift across a barline.
#[must_use]
pub fn chord_onset_beat(measure: &Measure, chord_idx: usize, time_signature: (u8, u8)) -> f64 {
    let ts = TimeSignature::new(time_signature.0.into(), time_signature.1.into());
    // Prefer rhythm_elements: continuation slashes ("A //// // D //") and
    // rests occupy beats before the chord but never appear in `chords`.
    if !measure.rhythm_elements.is_empty() {
        let mut beats = 0.0;
        let mut seen_chords = 0usize;
        for elem in &measure.rhythm_elements {
            match elem {
                RhythmElement::Chord(c) => {
                    if seen_chords == chord_idx {
                        return beats;
                    }
                    beats += c.duration.to_beats(ts);
                    seen_chords += 1;
                }
                RhythmElement::Space(s) => beats += s.duration.to_beats(ts),
                RhythmElement::Rest(r) => beats += r.duration.to_beats(ts),
            }
        }
    }
    measure
        .chords
        .iter()
        .take(chord_idx)
        .map(|c| c.duration.to_beats(ts))
        .sum()
}

/// [`chord_onset_beat`] in ticks, the unit rhythm segments carry.
#[must_use]
pub fn chord_onset_tick(measure: &Measure, chord_idx: usize, time_signature: (u8, u8)) -> i32 {
    (chord_onset_beat(measure, chord_idx, time_signature) * ticks_per_beat(time_signature)).round()
        as i32
}

/// The printed extent of `chord`'s symbol in `style`.
#[must_use]
pub fn symbol_extent(chord: &ChordInstance, style: &HarmonyStyle) -> SymbolExtent {
    stacked_symbol_extent(chord, style, 0.0)
}

/// [`symbol_extent`] with a slash chord folded `stack` of the way toward a
/// vertical fraction. Symbols without a bass ignore `stack`.
#[must_use]
pub fn stacked_symbol_extent(
    chord: &ChordInstance,
    style: &HarmonyStyle,
    stack: f64,
) -> SymbolExtent {
    let params = chord_layout::chord_to_harmony_params(chord, style).with_bass_stack(stack);
    let (layout, _) = harmony_layout(&params);
    SymbolExtent {
        left: (params.position.x - layout.bounds.x0).max(0.0),
        right: (layout.bounds.x1 - params.position.x).max(0.0),
    }
}

/// The chord symbols that print in `measure`, in onset order.
///
/// Mirrors what the renderer skips: placeholders (`s`, `r`), a pushed first
/// chord that spills back into the previous bar (unless the bar starts a
/// line or section, where it prints here too, on beat 1), a `.` repeat, and
/// the bar's previous symbol restated.
#[must_use]
pub fn printed_symbols(
    measure: &Measure,
    style: &HarmonyStyle,
    time_signature: (u8, u8),
    is_boundary: bool,
) -> Vec<PrintedSymbol> {
    let mut printed = Vec::new();
    let mut last_in_bar: Option<&str> = None;
    for (chord_idx, chord) in measure.chords.iter().enumerate() {
        if chord_renderer::is_placeholder_chord(&chord.full_symbol) {
            continue;
        }
        let is_pushed = chord
            .push_pull
            .as_ref()
            .is_some_and(|(is_push, _)| *is_push);
        let is_first_real = chord_renderer::is_first_real_chord(&measure.chords, chord_idx);
        if is_pushed && is_first_real && !is_boundary {
            continue;
        }
        if chord.original_token == "." || last_in_bar == Some(chord.full_symbol.as_str()) {
            continue;
        }
        last_in_bar = Some(chord.full_symbol.as_str());

        let onset = if is_pushed && is_first_real {
            0
        } else {
            chord_onset_tick(measure, chord_idx, time_signature)
        };
        printed.push(PrintedSymbol {
            chord_idx,
            onset,
            extent: symbol_extent(chord, style),
            stack: 0.0,
        });
    }
    printed.sort_by_key(|s| s.onset);

    // A slash chord crowded by the next symbol folds toward a fraction, and
    // takes the narrower width it will print at.
    let bar_ticks = (f64::from(time_signature.0) * ticks_per_beat(time_signature)).round() as i32;
    let onsets: Vec<i32> = printed.iter().map(|p| p.onset).collect();
    for (i, symbol) in printed.iter_mut().enumerate() {
        let chord = &measure.chords[symbol.chord_idx];
        if !chord.full_symbol.contains('/') {
            continue;
        }
        let next = onsets.get(i + 1).copied().unwrap_or(bar_ticks);
        symbol.stack = slash_stack_for_span(next - symbol.onset);
    }
    let is_slash: Vec<bool> = printed
        .iter()
        .map(|p| measure.chords[p.chord_idx].full_symbol.contains('/'))
        .collect();
    let mut stacks: Vec<f64> = printed.iter().map(|p| p.stack).collect();
    spread_stacking(&onsets, &is_slash, &mut stacks);
    for (symbol, stack) in printed.iter_mut().zip(stacks) {
        symbol.stack = stack;
        if stack > 0.0 {
            let chord = &measure.chords[symbol.chord_idx];
            symbol.extent = stacked_symbol_extent(chord, style, stack);
        }
    }
    printed
}

/// How near (ticks) another slash chord must be to a stacked one to stack
/// with it: two beats.
pub const STACK_NEIGHBOUR_TICKS: i32 = (constants::TICKS_PER_QUARTER as i32) * 2;

/// Once one slash chord in a bar stacks, the bar's other slash chords within
/// [`STACK_NEIGHBOUR_TICKS`] of it stack too, so neighbours read alike
/// (`A/E … B/F# C/G` all as fractions) rather than mixing the two forms.
/// `onsets`, `is_slash` and `stacks` run in print order.
pub fn spread_stacking(onsets: &[i32], is_slash: &[bool], stacks: &mut [f64]) {
    let seeds: Vec<i32> = onsets
        .iter()
        .zip(stacks.iter())
        .filter(|(_, s)| **s > 0.0)
        .map(|(o, _)| *o)
        .collect();
    for (i, stack) in stacks.iter_mut().enumerate() {
        if *stack == 0.0
            && is_slash.get(i).copied().unwrap_or(false)
            && seeds
                .iter()
                .any(|seed| (seed - onsets[i]).abs() <= STACK_NEIGHBOUR_TICKS)
        {
            *stack = 1.0;
        }
    }
}

/// Rhythm segments as `(start tick, length in ticks)`, from their lengths.
#[must_use]
pub fn segments_from_lengths(lengths: impl IntoIterator<Item = i32>) -> Vec<(i32, i32)> {
    let mut tick = 0;
    lengths
        .into_iter()
        .map(|len| {
            let segment = (tick, len);
            tick += len;
            segment
        })
        .collect()
}

/// The segment a tick falls in: the last one starting at or before it.
fn segment_at(segments: &[(i32, i32)], tick: i32) -> usize {
    segments
        .iter()
        .rposition(|(start, _)| *start <= tick)
        .unwrap_or(0)
}

/// Per-segment minimum widths (points) that keep every printed symbol clear
/// of the next one, and the last one inside the bar.
///
/// The room two symbols need is spread over the segments between them in
/// proportion to their rhythmic stretch, so a bar that has to widen for its
/// symbols still spaces its slashes by time: a quarter keeps more room than
/// the eighth beside it, and two quarters keep the same.
#[must_use]
pub fn symbol_segment_minimums(
    segments: &[(i32, i32)],
    symbols: &[PrintedSymbol],
    spacing: SymbolSpacing,
) -> Vec<f64> {
    let mut minimums = vec![0.0; segments.len()];
    if segments.is_empty() {
        return minimums;
    }

    let mut spread = |from: usize, to: usize, need: f64| {
        let stretch = |len: i32| {
            spacing::duration_stretch(
                f64::from(len.max(1)),
                constants::TICKS_PER_QUARTER,
                spacing::DEFAULT_SPACING_SLOPE,
            )
        };
        let total: f64 = segments[from..to]
            .iter()
            .map(|(_, len)| stretch(*len))
            .sum();
        if total <= 0.0 {
            return;
        }
        for (k, (_, len)) in segments.iter().enumerate().take(to).skip(from) {
            minimums[k] = f64::max(minimums[k], need * stretch(*len) / total);
        }
    };

    for pair in symbols.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let from = segment_at(segments, a.onset);
        let to = segment_at(segments, b.onset);
        if to <= from {
            continue;
        }
        spread(from, to, a.extent.right + spacing.clearance + b.extent.left);
    }

    if let Some(last) = symbols.last() {
        let from = segment_at(segments, last.onset);
        // The next bar's first symbol starts a barline-gap past the
        // barline, so the last one only has to stop a gap short of it.
        spread(
            from,
            segments.len(),
            last.extent.right + spacing.clearance - spacing.spatium,
        );
    }

    minimums
}

/// Equal beats get equal space: a bar of four one-beat chords keeps its
/// four beats even, however much wider `G#m7` is than `F#7`. Every segment
/// of a length asks for what the widest of that length needs.
///
/// Costs room, so the measure layout applies it only when the bar it was
/// given can afford it; the line-breaking minimum does not ask for it, and
/// a line keeps its four bars.
#[must_use]
pub fn equalize_equal_beats(segments: &[(i32, i32)], minimums: &[f64]) -> Vec<f64> {
    let mut minimums = minimums.to_vec();
    let widest_by_len: Vec<(i32, f64)> = segments.iter().zip(&minimums).fold(
        Vec::new(),
        |mut acc: Vec<(i32, f64)>, ((_, len), w)| {
            match acc.iter_mut().find(|(l, _)| l == len) {
                Some((_, widest)) => *widest = widest.max(*w),
                None => acc.push((*len, *w)),
            }
            acc
        },
    );
    for ((_, len), w) in segments.iter().zip(minimums.iter_mut()) {
        if let Some((_, widest)) = widest_by_len.iter().find(|(l, _)| l == len) {
            *w = *widest;
        }
    }

    minimums
}

/// The narrowest a bar's body (barline to barline) can be and still print
/// its symbols on their beats: the symbol minimums, a slash's width for every
/// segment they leave alone, and the gaps at either barline.
#[must_use]
pub fn symbol_body_min_width(
    segments: &[(i32, i32)],
    symbols: &[PrintedSymbol],
    spacing: SymbolSpacing,
) -> f64 {
    // A slash head and the air either side of it.
    let slash = spacing.spatium * 2.0;
    // Bar-to-note and note-to-bar distances.
    let edges = spacing.spatium * 2.5;
    symbol_segment_minimums(segments, symbols, spacing)
        .iter()
        .map(|w| w.max(slash))
        .sum::<f64>()
        + edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(onset: i32, right: f64) -> PrintedSymbol {
        PrintedSymbol {
            chord_idx: 0,
            onset,
            extent: SymbolExtent { left: 0.0, right },
            stack: 0.0,
        }
    }

    const SPACING: SymbolSpacing = SymbolSpacing {
        spatium: 5.0,
        clearance: 8.0,
    };

    #[test]
    fn two_symbols_a_beat_apart_widen_that_beat() {
        // Four quarters, symbols on beats 1 and 2.
        let segments = segments_from_lengths([480, 480, 480, 480]);
        let mins =
            symbol_segment_minimums(&segments, &[symbol(0, 30.0), symbol(480, 20.0)], SPACING);
        assert!((mins[0] - 38.0).abs() < 1e-9, "{mins:?}");
        // The last symbol's room is spread over the beats it covers, evenly.
        let tail = 20.0 + 8.0 - 5.0;
        for w in &mins[1..] {
            assert!((w - tail / 3.0).abs() < 1e-9, "{mins:?}");
        }
        // Equalized, every quarter asks for what the widest one needs.
        for w in equalize_equal_beats(&segments, &mins) {
            assert!((w - 38.0).abs() < 1e-9);
        }
    }

    #[test]
    fn room_between_symbols_is_spread_by_rhythmic_stretch() {
        // A half then two quarters, symbols on beat 1 and beat 4.
        let segments = segments_from_lengths([960, 480, 480]);
        let mins =
            symbol_segment_minimums(&segments, &[symbol(0, 100.0), symbol(1440, 10.0)], SPACING);
        assert!(
            mins[0] > mins[1],
            "a half keeps more room than a quarter: {mins:?}"
        );
        let ratio = mins[0] / mins[1];
        assert!(
            (ratio - 1.2).abs() < 1e-6,
            "the ratio is the default spacing slope, not the 2:1 of the durations: {ratio}"
        );
    }

    #[test]
    fn symbols_on_the_same_segment_ask_for_nothing_between_them() {
        let segments = segments_from_lengths([1920]);
        let mins = symbol_segment_minimums(&segments, &[symbol(0, 10.0), symbol(0, 10.0)], SPACING);
        assert_eq!(mins.len(), 1);
        assert!((mins[0] - (10.0 + 8.0 - 5.0)).abs() < 1e-9);
    }

    #[test]
    fn a_bar_without_symbols_is_slashes_and_edges() {
        let segments = segments_from_lengths([480; 4]);
        let width = symbol_body_min_width(&segments, &[], SPACING);
        assert!((width - (4.0 * 10.0 + 12.5)).abs() < 1e-9, "{width}");
    }

    #[test]
    fn onset_ticks_follow_written_durations() {
        let chart =
            keyflow_lang::text::chart::parse_chart("T\n#C 120bpm 4/4\n\nVS 1\nC/F_8 F_4. B7_2")
                .expect("chart parses");
        let measure = &chart.sections[0].measures()[0];
        let onsets: Vec<i32> = (0..measure.chords.len())
            .map(|i| chord_onset_tick(measure, i, (4, 4)))
            .collect();
        assert_eq!(onsets, vec![0, 240, 960]);
    }
}

#[cfg(test)]
mod spread_stacking_tests {
    use super::spread_stacking;

    #[test]
    fn near_slash_chords_stack_with_a_crowded_one() {
        // A/E @1, B/F# @3 (stacked), C/G @3.5, D @4 — beats in ticks.
        let onsets = [0, 960, 1200, 1440];
        let is_slash = [true, true, true, false];
        let mut stacks = [0.0, 1.0, 0.0, 0.0];
        spread_stacking(&onsets, &is_slash, &mut stacks);
        assert_eq!(stacks, [1.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn a_far_slash_chord_stays_inline() {
        let onsets = [0, 1680];
        let is_slash = [true, true];
        let mut stacks = [0.0, 1.0];
        spread_stacking(&onsets, &is_slash, &mut stacks);
        assert_eq!(stacks, [0.0, 1.0]);
    }
}
