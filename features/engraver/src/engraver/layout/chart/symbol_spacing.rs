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

impl SymbolSpacing {
    /// The clearance the renderer keeps: its collision pass pads every
    /// symbol by half a space each side and then asks for `min_gap` between
    /// the padded boxes.
    #[must_use]
    pub fn new(spatium: f64, min_gap: f64) -> Self {
        Self {
            spatium,
            clearance: spatium + min_gap.max(0.0),
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
    let params = chord_layout::chord_to_harmony_params(chord, style);
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
        });
    }
    printed.sort_by_key(|s| s.onset);
    printed
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
