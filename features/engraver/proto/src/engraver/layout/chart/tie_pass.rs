//! Cross-barline tie rendering for rhythm slashes and melody notes.
//!
//! Both passes mirror MuseScore's `Tie` element painted between
//! consecutive `ChordRest` segments. Living here keeps `mod.rs` lean.

use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::segment::SegmentType;
use crate::engraver::layout::tlayout::{
    ArticulationConfig, ArticulationContext, ArticulationInput, ArticulationType, SlurDirection,
    SlurEndpoint, SlurObstacle, SlurTieConfig, layout_articulation, layout_slur_with_obstacles,
    layout_tie,
};
use crate::engraver::notation::MeasureScene;
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;

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
            // Tied to a note whose stem goes the other way: the tie curves
            // upward (Gould). Otherwise away from the shared stem side.
            let next_stem = measure_result.entry_stem_up.get(i + 1).copied().flatten();
            let next_in_bar = melody_data
                .segments
                .get(i + 1)
                .is_some_and(|n| n.tie_from_previous);
            let mixed = segment.tie_to_next && next_stem.is_some_and(|n| n != stem_up);
            // A tie over the barline is drawn in two halves that cannot see
            // each other's stems; both belong to one pitch, so the pitch's own
            // stem side decides for both (middle line and up: stem down, tie
            // above).
            let over_barline =
                (segment.tie_to_next && !next_in_bar) || (segment.tie_from_previous && i == 0);
            let below = if over_barline {
                line.is_some_and(|l| l < 0)
            } else {
                stem_up && !mixed
            };
            let direction = if below {
                SlurDirection::Down
            } else {
                SlurDirection::Up
            };
            // A tie sits off the heads on its side (y grows downward), not
            // through their middles.
            let tie_y = note_y
                + if below {
                    0.55 * spatium
                } else {
                    -0.55 * spatium
                };
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
                    let tie_layout = layout_tie(
                        &start,
                        &end,
                        direction,
                        1000 + i as u64,
                        spatium,
                        &tie_config,
                    );
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
                let tie_layout = layout_tie(
                    &start,
                    &end,
                    direction,
                    2000 + i as u64,
                    spatium,
                    &tie_config,
                );
                measure_result
                    .scene
                    .add_child(SceneNode::anonymous_leaf(tie_layout.commands));
            }
        }
    }

    /// Slurs on melody notes. A slur runs from its start note to its end
    /// note in the bar; one crossing a barline is drawn to the bar's end and
    /// picks up from the next bar's start. It goes on the notehead side —
    /// below when every stem under it goes up, above otherwise (stems down,
    /// or mixed: Gould) — and arches over the notes inside it.
    pub(super) fn add_melody_slurs(
        &self,
        measure_result: &mut MeasureScene,
        melody_data: &MeasureMelodyData,
        ctx: &LayoutContext<'_>,
        clef: crate::chart::ChartClef,
    ) {
        let spatium = ctx.spatium();
        let config = SlurTieConfig::default();
        let segs = &melody_data.segments;
        let chord_segments: Vec<_> = measure_result
            .segments
            .iter()
            .filter(|seg| seg.seg_type.contains(SegmentType::CHORD_REST))
            .collect();
        // A note's heads, top and bottom: a slur on a chord meets its outer
        // note on the slur's side.
        let extent = |i: usize| -> Option<(f64, f64)> {
            let seg = segs.get(i)?;
            let oct = seg.octave?;
            let y_of = |pitch: &str, octave: u8| {
                let (line, _) = melody_pitch_to_line_for_clef(pitch, octave, clef);
                -(line as f64) * spatium / 2.0
            };
            let main = y_of(&seg.pitch, oct);
            Some(
                seg.extra_pitches
                    .iter()
                    .fold((main, main), |(top, bottom), (p, o)| {
                        let y = y_of(p, o.unwrap_or(oct));
                        (top.min(y), bottom.max(y))
                    }),
            )
        };
        let head_x = |i: usize| chord_segments.get(i).map(|s| s.x + 0.6 * spatium);

        // (first, last, opens_before_bar, runs_past_bar) for each slur here.
        let mut spans: Vec<(usize, usize, bool, bool)> = Vec::new();
        let mut open: Option<usize> = None;
        for (i, seg) in segs.iter().enumerate() {
            if seg.is_rest {
                continue;
            }
            if seg.slur_stop {
                match open.take() {
                    Some(start) => spans.push((start, i, false, false)),
                    None => spans.push((0, i, true, false)),
                }
            }
            if seg.slur_start {
                open = Some(i);
            }
        }
        if let Some(start) = open {
            let last = segs.iter().rposition(|s| !s.is_rest).unwrap_or(start);
            spans.push((start, last, false, true));
        }

        let stem_of = |i: usize| measure_result.entry_stem_up.get(i).copied().flatten();
        for (k, (a, b, from_start, to_end)) in spans.into_iter().enumerate() {
            let stems: Vec<bool> = (a..=b).filter_map(stem_of).collect();
            // Over a barline each half sees only its own bar's stems, and the
            // two halves must agree: such a slur goes above (Gould's choice
            // when the stems cannot settle it).
            let crosses = from_start || to_end;
            let below = !crosses && !stems.is_empty() && stems.iter().all(|up| *up);
            let side = if below { 1.0 } else { -1.0 };
            // Where the slur meets a note: off the head, or — when the stem
            // points into the slur's side — off the stem's tip.
            const STEM: f64 = 3.5;
            let attach = |i: usize| -> Option<(f64, f64)> {
                let (top, bottom) = extent(i)?;
                let (x, y) = (head_x(i)?, if below { bottom } else { top });
                // A flag or beam sits at the tip of a short note's stem.
                let short = segs.get(i).is_some_and(|s| s.beats < 1.0);
                let stem_into = match stem_of(i) {
                    Some(up) => up != below,
                    None => false,
                };
                // A staccato or tenuto on the slur's side sits inside it:
                // the slur clears the mark (marks go opposite the stem).
                let mark_inside = segs.get(i).is_some_and(|s| {
                    s.articulations
                        .iter()
                        .any(|a| matches!(a.as_str(), "staccato" | "staccatissimo" | "tenuto"))
                }) && stem_of(i).is_some_and(|up| up == below);
                let y = if mark_inside {
                    y + side * 1.2 * spatium
                } else {
                    y
                };
                Some(if stem_into {
                    // Stem on the slur's side: the stem stands at the head's
                    // right (up) or left (down) edge.
                    let sx = if below {
                        x - 0.6 * spatium
                    } else {
                        x + 0.6 * spatium
                    };
                    let clear = if short { 0.6 } else { 0.0 };
                    let flag_x = if short && !below { 0.4 * spatium } else { 0.0 };
                    (sx + flag_x, y + side * (STEM + 0.5 + clear) * spatium)
                } else {
                    (x, y + side * 0.9 * spatium)
                })
            };
            let (Some((xa, ya)), Some((xb, yb))) = (attach(a), attach(b)) else {
                continue;
            };
            let start = SlurEndpoint {
                x: if from_start { -0.5 * spatium } else { xa },
                y: ya,
                stem_up: !below,
            };
            let end = SlurEndpoint {
                x: if to_end {
                    measure_result.width + 0.5 * spatium
                } else {
                    xb
                },
                y: yb,
                stem_up: !below,
            };
            // The notes it passes over, with any stem reaching its side.
            let obstacles: Vec<SlurObstacle> = (a + 1..b)
                .filter_map(|i| {
                    let x = head_x(i)?;
                    let (top, bottom) = extent(i)?;
                    let reach = match stem_of(i) {
                        Some(up) if up != below => STEM * spatium,
                        _ => 0.5 * spatium,
                    };
                    let (y0, y1) = if below {
                        (top - 0.5 * spatium, bottom + reach)
                    } else {
                        (top - reach, bottom + 0.5 * spatium)
                    };
                    Some(SlurObstacle::note(kurbo::Rect::new(
                        x - 0.6 * spatium,
                        y0,
                        x + 0.6 * spatium,
                        y1,
                    )))
                })
                .collect();
            let direction = if below {
                SlurDirection::Down
            } else {
                SlurDirection::Up
            };
            let layout = layout_slur_with_obstacles(
                &start,
                &end,
                direction,
                &obstacles,
                3000 + k as u64,
                spatium,
                &config,
            );
            measure_result
                .scene
                .add_child(SceneNode::anonymous_leaf(layout.commands));
        }
    }

    /// Articulations on melody notes, opposite the stem (Gould): staccato,
    /// staccatissimo and tenuto close to the notehead — in the next space,
    /// off any staff line — accents and marcato outside the staff.
    pub(super) fn add_melody_articulations(
        &self,
        measure_result: &mut MeasureScene,
        melody_data: &MeasureMelodyData,
        ctx: &LayoutContext<'_>,
        clef: crate::chart::ChartClef,
    ) {
        let spatium = ctx.spatium();
        let config = ArticulationConfig::default();
        let chord_segments: Vec<_> = measure_result
            .segments
            .iter()
            .filter(|seg| seg.seg_type.contains(SegmentType::CHORD_REST))
            .collect();
        for (i, seg) in melody_data.segments.iter().enumerate() {
            if seg.is_rest || seg.articulations.is_empty() {
                continue;
            }
            let (Some(cs), Some(oct)) = (chord_segments.get(i), seg.octave) else {
                continue;
            };
            let y_of = |pitch: &str, octave: u8| {
                let (line, _) = melody_pitch_to_line_for_clef(pitch, octave, clef);
                -(line as f64) * spatium / 2.0
            };
            let main = y_of(&seg.pitch, oct);
            let (top, bottom) = seg
                .extra_pitches
                .iter()
                .fold((main, main), |(t, b), (p, o)| {
                    let y = y_of(p, o.unwrap_or(oct));
                    (t.min(y), b.max(y))
                });
            let x = cs.x + 0.6 * spatium;
            let stem_up = measure_result
                .entry_stem_up
                .get(i)
                .copied()
                .flatten()
                .unwrap_or(main > 0.0);
            let above = !stem_up;
            // Stacked outward from the head, close marks first.
            let mut offset = 0.0;
            let mut names: Vec<&String> = seg.articulations.iter().collect();
            names.sort_by_key(|n| !matches!(n.as_str(), "staccato" | "staccatissimo" | "tenuto"));
            for (k, name) in names.into_iter().enumerate() {
                // A fall drops away from the notehead's right; a fermata
                // stands over the note, above the staff.
                if name == "fall" || name == "fermata" {
                    let (glyph, at) = if name == "fall" {
                        // brassFallSmoothShort
                        ('\u{E5DA}', kurbo::Point::new(x + 1.0 * spatium, main))
                    } else {
                        // fermataAbove, over the stem tip when the stem is up.
                        let reach = if stem_up { top - 3.5 * spatium } else { top };
                        (
                            '\u{E4C0}',
                            kurbo::Point::new(x, reach.min(-2.0 * spatium) - 1.2 * spatium),
                        )
                    };
                    measure_result
                        .scene
                        .add_child(SceneNode::anonymous_leaf(vec![PaintCommand::glyph(
                            glyph,
                            at,
                            spatium * config.scale,
                            peniko::Color::BLACK,
                        )]));
                    continue;
                }
                let kind = match name.as_str() {
                    "staccato" => ArticulationType::Staccato,
                    "staccatissimo" => ArticulationType::Staccatissimo,
                    "tenuto" => ArticulationType::Tenuto,
                    "accent" => ArticulationType::Accent,
                    "marcato" => ArticulationType::Marcato,
                    _ => continue,
                };
                let close = matches!(
                    kind,
                    ArticulationType::Staccato
                        | ArticulationType::Staccatissimo
                        | ArticulationType::Tenuto
                );
                let id = 4000 + (i * 8 + k) as u64;
                if close && offset == 0.0 {
                    let glyph = if above {
                        kind.smufl_codepoint_above()
                    } else {
                        kind.smufl_codepoint_below()
                    };
                    let side = if above { -1.0 } else { 1.0 };
                    let mut y = (if above { top } else { bottom }) + side * spatium;
                    // Inside the staff (lines at whole spaces from the middle)
                    // a mark on a line moves into the next space out.
                    let in_staff = y.abs() <= 2.0 * spatium + 0.01;
                    if in_staff && ((y / spatium) - (y / spatium).round()).abs() < 0.1 {
                        y += side * 0.5 * spatium;
                    }
                    offset = (y - if above { top } else { bottom }).abs() + 0.5 * spatium;
                    measure_result
                        .scene
                        .add_child(SceneNode::anonymous_leaf(vec![PaintCommand::glyph(
                            glyph,
                            kurbo::Point::new(x, y),
                            spatium * config.scale,
                            peniko::Color::BLACK,
                        )]));
                } else {
                    let context = ArticulationContext::new(x, main, -2.0 * spatium, 2.0 * spatium)
                        .with_stem(
                            if stem_up {
                                x + 0.6 * spatium
                            } else {
                                x - 0.6 * spatium
                            },
                            stem_up,
                        )
                        .with_chord_bounds(
                            top - offset * f64::from(u8::from(above)),
                            bottom + offset * f64::from(u8::from(!above)),
                        );
                    let layout = layout_articulation(
                        &ArticulationInput::new(kind).with_id(id),
                        &context,
                        spatium,
                        &config,
                    );
                    measure_result
                        .scene
                        .add_child(SceneNode::anonymous_leaf(layout.commands));
                }
            }
        }
    }
}
