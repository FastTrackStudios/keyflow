//! Cross-barline tie rendering for rhythm slashes and melody notes.
//!
//! Both passes mirror MuseScore's `Tie` element painted between
//! consecutive `ChordRest` segments. Living here keeps `mod.rs` lean.

use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::segment::SegmentType;
use crate::engraver::layout::tlayout::{SlurDirection, SlurEndpoint, SlurTieConfig, layout_tie};
use crate::engraver::notation::MeasureScene;
use crate::engraver::scene::node::SceneNode;

use super::ChartLayoutEngine;
use super::types::{MeasureMelodyData, melody_pitch_to_line_for_clef};

impl ChartLayoutEngine {
    /// Add tie arcs on rhythm-slash heads marked `ChordRhythm::Slashes.tied`.
    ///
    /// For each slash chord whose data carries `tied = true`, drapes a tie
    /// arc from the right side of its last segment to the left side of the
    /// next segment (or just past the measure end when the tie crosses the
    /// barline). Mirrors MuseScore's `Tie` between rhythm-slash chord-rests.
    pub(super) fn add_slash_ties(
        &self,
        measure_result: &mut MeasureScene,
        measure: &crate::chart::types::Measure,
        ctx: &LayoutContext<'_>,
    ) {
        use crate::chord::ChordRhythm;
        let spatium = ctx.spatium();
        let tie_config = SlurTieConfig::default();

        let chord_segments: Vec<_> = measure_result
            .segments
            .iter()
            .filter(|seg| seg.seg_type.contains(SegmentType::CHORD_REST))
            .collect();

        let mut seg_cursor = 0usize;
        let staff_y = 2.0 * spatium;

        for chord in &measure.chords {
            let seg_count = match &chord.rhythm {
                ChordRhythm::Slashes { count, .. } => (*count as usize).max(1),
                _ => 1,
            };
            let tied = matches!(&chord.rhythm, ChordRhythm::Slashes { tied: true, .. });
            let last_seg_idx = seg_cursor + seg_count - 1;

            if tied {
                let from = chord_segments.get(last_seg_idx);
                let to = chord_segments.get(last_seg_idx + 1);
                if let Some(from) = from {
                    // Segment positions and the measure width are in points.
                    let start_x = from.x + spatium;
                    let end_x = match to {
                        Some(seg) => seg.x,
                        None => measure_result.width + spatium * 0.5,
                    };
                    let start = SlurEndpoint {
                        x: start_x,
                        y: staff_y,
                        stem_up: false,
                    };
                    let end = SlurEndpoint {
                        x: end_x,
                        y: staff_y,
                        stem_up: false,
                    };
                    let tie_layout = layout_tie(
                        &start,
                        &end,
                        SlurDirection::Down,
                        3000 + last_seg_idx as u64,
                        spatium,
                        &tie_config,
                    );
                    measure_result
                        .scene
                        .add_child(SceneNode::anonymous_leaf(tie_layout.commands));
                }
            }

            seg_cursor += seg_count;
        }
    }
    /// Tie arcs on melody notes: a tie into the next note of the same bar
    /// ends at that note; one into the next bar (a note split at the
    /// barline, or written `~`) runs to the bar's end, and its other half
    /// comes in from the start of the next bar. The arc curves away from
    /// the stem. `across_barlines` turns the barline halves off for styles
    /// that print the second piece as a fresh attack.
    pub(super) fn add_melody_ties(
        &self,
        measure_result: &mut MeasureScene,
        melody_data: &MeasureMelodyData,
        ctx: &LayoutContext<'_>,
        clef: crate::chart::ChartClef,
        across_barlines: bool,
    ) {
        let spatium = ctx.spatium();
        let tie_config = SlurTieConfig::default();

        let chord_segments: Vec<_> = measure_result
            .segments
            .iter()
            .filter(|seg| seg.seg_type.contains(SegmentType::CHORD_REST))
            .collect();

        let line_of = |i: usize| -> Option<i32> {
            let seg = melody_data.segments.get(i)?;
            let oct = seg.octave?;
            Some(melody_pitch_to_line_for_clef(&seg.pitch, oct, clef).0)
        };

        for (i, segment) in melody_data.segments.iter().enumerate() {
            if segment.is_rest {
                continue;
            }
            let line = line_of(i);
            let note_y = line.map_or(2.0 * spatium, |l| -(l as f64) * spatium / 2.0);
            // Away from the stem: the stem the note was actually drawn with
            // (a beam can turn a high note's stem up), and for a stemless
            // note the side its stem would take.
            let stem_up = measure_result
                .entry_stem_up
                .get(i)
                .copied()
                .flatten()
                .unwrap_or_else(|| line.is_some_and(|l| l < 0));
            let direction = if stem_up { SlurDirection::Down } else { SlurDirection::Up };
            // A tie sits off the heads on its side (y grows downward), not
            // through their middles.
            let tie_y = note_y + if stem_up { 0.55 * spatium } else { -0.55 * spatium };
            let Some(seg) = chord_segments.get(i) else {
                continue;
            };
            // Segment positions and the measure width are in points.
            let note_x = seg.x;

            if segment.tie_to_next {
                let next_in_bar = melody_data
                    .segments
                    .get(i + 1)
                    .filter(|n| n.tie_from_previous && !n.is_rest)
                    .and(chord_segments.get(i + 1));
                let end_x = match next_in_bar {
                    Some(next) => Some(next.x),
                    None if across_barlines => Some(measure_result.width + spatium * 0.5),
                    None => None,
                };
                if let Some(end_x) = end_x {
                    let start = SlurEndpoint {
                        x: note_x + 1.1 * spatium,
                        y: tie_y,
                        stem_up: false,
                    };
                    let end = SlurEndpoint {
                        x: end_x - 0.1 * spatium,
                        y: tie_y,
                        stem_up: false,
                    };
                    let tie_layout =
                        layout_tie(&start, &end, direction, 1000 + i as u64, spatium, &tie_config);
                    measure_result
                        .scene
                        .add_child(SceneNode::anonymous_leaf(tie_layout.commands));
                }
            }

            // The incoming half of a tie over the barline. A tie from a note
            // earlier in this bar was drawn whole from that note.
            if segment.tie_from_previous && i == 0 && across_barlines {
                let start = SlurEndpoint {
                    x: -spatium * 0.5,
                    y: tie_y,
                    stem_up: false,
                };
                let end = SlurEndpoint {
                    x: note_x - 0.1 * spatium,
                    y: tie_y,
                    stem_up: false,
                };
                let tie_layout =
                    layout_tie(&start, &end, direction, 2000 + i as u64, spatium, &tie_config);
                measure_result
                    .scene
                    .add_child(SceneNode::anonymous_leaf(tie_layout.commands));
            }
        }
    }
}
