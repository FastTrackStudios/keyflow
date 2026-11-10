//! Annotation placement around the staff: endings, and the dynamics, text,
//! figures, hairpins and cues each bar hangs above or below its line.

use super::{ChartLayoutEngine, notation_renderer};
use crate::chart::Measure;
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::scene::node::SceneNode;
use kurbo::{Affine, Rect};

/// One bar's staff-attached notations (dynamics, staff text, figured bass,
/// suspensions, hairpins and cues), with what their renderers need to know
/// about the bar and the line it sits on.
pub(super) struct BarNotations<'a> {
    pub(super) measure: &'a Measure,
    /// The section's bars: a hairpin drawn here may have started earlier.
    pub(super) section_measures: &'a [Measure],
    pub(super) measure_idx: usize,
    pub(super) frame: &'a notation_renderer::MeasureFrame<'a>,
    /// The bar's rendered chord-symbol bounds, which suspension figures
    /// anchor to.
    pub(super) chord_bounds: &'a [Rect],
    /// The bar's text cues, already rendered: they stack in rows with the
    /// other below-staff texts.
    pub(super) cue_nodes: Vec<SceneNode>,
    /// The bar's right edge, and the line's.
    pub(super) bar_end: f64,
    pub(super) line_end: f64,
}

/// A notation waiting for the system's skyline: the node, whether it sits
/// above the staff, and whether it is text that may shrink to fit.
pub(super) type PendingNotation = (SceneNode, bool, bool);

impl ChartLayoutEngine {
    /// Render a bar's notations for the paginated loop, which places them
    /// only once every bar of the line has put its ink on the skyline.
    ///
    /// Dynamics are stamped with the page and system they belong to.
    pub(super) fn deferred_bar_notations(
        &self,
        bar: BarNotations<'_>,
        page_number: u32,
        global_system_index: usize,
        text_metrics: &TextFontMetrics,
        id_counter: &mut u64,
    ) -> Vec<PendingNotation> {
        let BarNotations {
            measure,
            section_measures,
            measure_idx,
            frame: notation_frame,
            chord_bounds: suspension_chord_bounds,
            cue_nodes,
            bar_end,
            line_end,
        } = bar;
        let autoplace_gap = self.config.spatium * 0.1;
        let mut pending_notation: Vec<PendingNotation> = Vec::new();
        // The bar's below-staff texts — its dynamic, text cues
        // and instrument cues — are laid out together, in
        // rows, so none lands on another.
        let mut below_stack: Vec<SceneNode> = Vec::new();
        for (item, mut node) in
            measure
                .classical_dynamics
                .iter()
                .zip(notation_renderer::render_dynamics(
                    &measure.classical_dynamics,
                    notation_frame,
                    id_counter,
                ))
        {
            node.set_page(page_number);
            node.set_system(global_system_index as u32);
            node.set_measure(measure_idx as u32);
            if matches!(item.placement, crate::chart::notations::Placement::Below) {
                below_stack.push(node);
            } else {
                pending_notation.push((node, false, false));
            }
        }
        for (item, mut node) in measure
            .staff_text
            .iter()
            .zip(notation_renderer::render_staff_text(
                &measure.staff_text,
                notation_frame,
                text_metrics,
                id_counter,
            ))
        {
            let above = matches!(item.placement, crate::chart::notations::Placement::Above);
            if !above {
                below_stack.push(node);
                continue;
            }
            notation_renderer::shrink_node_to_max_right(&mut node, bar_end - autoplace_gap, 0.45);
            pending_notation.push((node, above, true));
        }
        // Figured bass is deferred to the skyline autoplace pass
        // like the other annotations. Its natural baseline can
        // sit on the chord symbol (a single Above row anchors at
        // chord_y), so it must be pushed clear of chord ink
        // rather than painted directly on top of it.
        for (item, node) in measure
            .figured_bass
            .iter()
            .zip(notation_renderer::render_figured_bass(
                &measure.figured_bass,
                notation_frame,
                id_counter,
            ))
        {
            let above = matches!(item.placement, crate::chart::notations::Placement::Above);
            pending_notation.push((node, above, false));
        }
        for (item, node) in measure
            .suspensions
            .iter()
            .zip(notation_renderer::render_suspensions(
                &measure.suspensions,
                notation_frame,
                self.config.harmony_style.root_size,
                suspension_chord_bounds,
                id_counter,
            ))
        {
            let above = matches!(item.placement, crate::chart::notations::Placement::Above);
            pending_notation.push((node, above, false));
        }
        let hairpins = notation_renderer::hairpins_through(section_measures, measure_idx);
        for ((item, _), node) in hairpins.iter().zip(notation_renderer::render_hairpins(
            &hairpins,
            notation_frame,
            id_counter,
        )) {
            let above = matches!(item.placement, crate::chart::notations::Placement::Above);
            pending_notation.push((node, above, false));
        }
        below_stack.extend(cue_nodes);
        for node in notation_renderer::stack_below_staff_rows(
            below_stack,
            bar_end - autoplace_gap,
            line_end,
            self.config.spatium * 1.6,
            self.config.spatium * 0.6,
        ) {
            pending_notation.push((node, false, false));
        }
        pending_notation
    }

    /// Render a bar's notations for the continuous loop and place each on
    /// the system's skyline as it comes. Returns the placed nodes.
    pub(super) fn place_bar_notations(
        &self,
        bar: BarNotations<'_>,
        system_skyline: &mut notation_renderer::MeasureSkyline,
        text_metrics: &TextFontMetrics,
        id_counter: &mut u64,
    ) -> Vec<SceneNode> {
        let BarNotations {
            measure,
            section_measures,
            measure_idx,
            frame: notation_frame,
            chord_bounds: suspension_chord_bounds,
            cue_nodes,
            bar_end,
            line_end,
        } = bar;
        let autoplace_gap = self.config.spatium * 0.1;
        let text_shrink_displacement = self.config.spatium * 4.0;
        // Collect (node, is_above) then drain into root after
        // skyline runs so closures don't fight over &mut root.
        let mut placed: Vec<SceneNode> = Vec::new();
        // Below-staff texts are stacked in rows together.
        let mut below_stack: Vec<SceneNode> = Vec::new();
        for (item, node) in
            measure
                .classical_dynamics
                .iter()
                .zip(notation_renderer::render_dynamics(
                    &measure.classical_dynamics,
                    notation_frame,
                    id_counter,
                ))
        {
            if matches!(item.placement, crate::chart::notations::Placement::Below) {
                below_stack.push(node);
            } else if let Some(n) =
                notation_renderer::autoplace_node(system_skyline, node, false, autoplace_gap)
            {
                placed.push(n);
            }
        }
        for (item, mut node) in measure
            .staff_text
            .iter()
            .zip(notation_renderer::render_staff_text(
                &measure.staff_text,
                notation_frame,
                text_metrics,
                id_counter,
            ))
        {
            let above = matches!(item.placement, crate::chart::notations::Placement::Above);
            if !above {
                below_stack.push(node);
                continue;
            }
            notation_renderer::shrink_node_to_max_right(&mut node, bar_end - autoplace_gap, 0.45);
            if let Some(n) = notation_renderer::autoplace_text_node(
                system_skyline,
                node,
                above,
                autoplace_gap,
                text_shrink_displacement,
            ) {
                placed.push(n);
            }
        }
        for (item, node) in measure
            .figured_bass
            .iter()
            .zip(notation_renderer::render_figured_bass(
                &measure.figured_bass,
                notation_frame,
                id_counter,
            ))
        {
            let _ = item;
            if let Some(bounds) = notation_renderer::scene_ink_bounds(&node, Affine::IDENTITY) {
                system_skyline.add_above(bounds);
                system_skyline.add_below(bounds);
            }
            placed.push(node);
        }
        for (item, node) in measure
            .suspensions
            .iter()
            .zip(notation_renderer::render_suspensions(
                &measure.suspensions,
                notation_frame,
                self.config.harmony_style.root_size,
                suspension_chord_bounds,
                id_counter,
            ))
        {
            let _ = item;
            if let Some(bounds) = notation_renderer::scene_ink_bounds(&node, Affine::IDENTITY) {
                system_skyline.add_above(bounds);
                system_skyline.add_below(bounds);
            }
            placed.push(node);
        }
        let hairpins = notation_renderer::hairpins_through(section_measures, measure_idx);
        for ((item, _), node) in hairpins.iter().zip(notation_renderer::render_hairpins(
            &hairpins,
            notation_frame,
            id_counter,
        )) {
            let above = matches!(item.placement, crate::chart::notations::Placement::Above);
            if let Some(n) =
                notation_renderer::autoplace_node(system_skyline, node, above, autoplace_gap)
            {
                placed.push(n);
            }
        }
        below_stack.extend(cue_nodes);
        for node in notation_renderer::stack_below_staff_rows(
            below_stack,
            bar_end - autoplace_gap,
            line_end,
            self.config.spatium * 1.6,
            self.config.spatium * 0.6,
        ) {
            if let Some(n) =
                notation_renderer::autoplace_node(system_skyline, node, false, autoplace_gap)
            {
                placed.push(n);
            }
        }
        placed
    }
}

/// The ending bracket drawn from a measure: one that starts there, or —
/// on the first measure of a system — one carried over a system break.
pub(super) struct VoltaSpan<'a> {
    pub(super) volta: &'a crate::chart::notations::Volta,
    /// Bars from this measure to the ending's last, inclusive.
    pub(super) remaining: usize,
    /// Carried over from the system above: no left hook, no label.
    pub(super) continued: bool,
    /// Ends with a closing hook. Gould: an ending that ends in a repeat sign
    /// is closed; the last ending, played straight through, is left open.
    pub(super) closed: bool,
}

pub(super) fn volta_at(
    measures: &[crate::chart::types::Measure],
    idx: usize,
    system_start: bool,
) -> Option<VoltaSpan<'_>> {
    let closed = |start: usize, len: usize| {
        measures
            .get(start + len.max(1) - 1)
            .is_some_and(|m| matches!(m.end_repeat, crate::chart::notations::RepeatMark::Backward))
    };
    if let Some(volta) = &measures.get(idx)?.volta_start {
        let len = usize::from(volta.length_measures.max(1));
        return Some(VoltaSpan {
            volta,
            remaining: len,
            continued: false,
            closed: closed(idx, len),
        });
    }
    if !system_start {
        return None;
    }
    // The nearest ending that starts before this bar, if it reaches it.
    let (start, volta) = measures[..idx]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, m)| m.volta_start.as_ref().map(|v| (i, v)))?;
    let len = usize::from(volta.length_measures.max(1));
    (start + len > idx).then(|| VoltaSpan {
        volta,
        remaining: start + len - idx,
        continued: true,
        closed: closed(start, len),
    })
}
