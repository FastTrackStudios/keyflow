//! A bar's chord symbols: the ones written on it, and the ones the next bar
//! pushes back onto its last beat.

use super::{ChartLayoutEngine, MeasureMeasurements, PushSpillback, chord_renderer};
use crate::chart::Measure;
use crate::chart::types::ChartSection;
use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::tlayout::HarmonyStyle;
use crate::engraver::scene::node::SceneNode;
use kurbo::Rect;

/// One laid-out bar, and where its chord symbols go.
pub(super) struct BarChordFrame<'a> {
    pub(super) measure: &'a Measure,
    pub(super) section: &'a ChartSection,
    pub(super) measure_idx: usize,
    pub(super) local_measure_idx: usize,
    pub(super) global_system_index: usize,
    pub(super) page_number: Option<u32>,
    pub(super) measure_x: f64,
    pub(super) measure_width: f64,
    pub(super) chord_y: f64,
    pub(super) articulation_lift: f64,
    /// What the bar's rhythm layout found: pushes inside the bar, symbols
    /// pushed back from the next one, and the note stacks symbols clear.
    pub(super) internal_push_positions: &'a [(usize, usize)],
    pub(super) spillback_positions: &'a [(usize, String)],
    pub(super) note_line_stacks: &'a [Option<(i32, i32)>],
    pub(super) segment_positions: &'a [f64],
    pub(super) segment_ticks: &'a [i32],
    /// Chords the next bar pushes back into this one.
    pub(super) spillbacks: Option<&'a [PushSpillback]>,
    pub(super) harmony_style: &'a HarmonyStyle,
    pub(super) time_signature: (u8, u8),
    /// Pre-computed measurements for this measure
    pub(super) measure_measurements: Option<&'a MeasureMeasurements>,
    /// Whether a simile bar's chords stay unprinted under its mark.
    pub(super) hide_simile_chords: bool,
}

/// A bar's rendered chord symbols.
pub(super) struct BarChords {
    pub(super) chords: Vec<SceneNode>,
    /// Symbols pushed back from the next bar.
    pub(super) spillback_chords: Vec<SceneNode>,
    /// The bar's own symbols' final bounds, for the skyline and for
    /// anchoring suspension figures.
    pub(super) chord_bounds: Vec<Rect>,
}

impl ChartLayoutEngine {
    /// Render a bar's chord symbols, then any the next bar pushes back.
    ///
    /// `previous_chord_symbol` carries the last symbol printed, so a
    /// repeated chord can be hidden; it is updated to this bar's last.
    pub(super) fn render_bar_chords(
        &self,
        bar: &BarChordFrame<'_>,
        previous_chord_symbol: &mut Option<String>,
        id_counter: &mut u64,
        ctx: &LayoutContext,
    ) -> BarChords {
        let measure = bar.measure;
        let measure_idx = bar.measure_idx;
        let section_measures = bar.section.measures();
        let section_name = bar.section.section.section_type.full_name();
        let chord_ctx = chord_renderer::ChordRenderContext {
            measure_x: bar.measure_x,
            measure_width: bar.measure_width,
            chord_y: bar.chord_y,
            page_number: bar.page_number,
            global_system_index: bar.global_system_index,
            measure_idx,
            local_measure_idx: bar.local_measure_idx,
            section_name: &section_name,
            segment_positions: bar.segment_positions,
            segment_ticks: bar.segment_ticks,
            articulation_lift: bar.articulation_lift,
            prev_bar_held_one_chord: self.config.name_cluster_beat_one
                && measure_idx
                    .checked_sub(1)
                    .and_then(|prev| section_measures.get(prev))
                    .is_some_and(chord_renderer::holds_one_chord),
            tied_in: measure_idx
                .checked_sub(1)
                .and_then(|prev| section_measures.get(prev))
                .and_then(|prev| {
                    prev.chords
                        .iter()
                        .rev()
                        .find(|c| !chord_renderer::is_placeholder_chord(&c.full_symbol))
                })
                .filter(|c| c.rhythm.is_tied())
                .map(|c| c.full_symbol.as_str()),
            internal_push_positions: bar.internal_push_positions,
            harmony_style: bar.harmony_style,
            time_signature: bar.time_signature,
            hide_repeated_chords: self.config.hide_repeated_chords,
            min_chord_symbol_gap: self.config.min_chord_symbol_gap,
            push_alters_rhythm: self.config.push_alters_rhythm,
            spatium: self.config.spatium,
            measure_measurements: bar.measure_measurements,
            spillback_positions: bar.spillback_positions,
            note_line_stacks: bar.note_line_stacks,
        };

        // A simile bar prints no chord symbol. The mark says
        // "that bar again", and the bar it repeats is right
        // there with the symbol over it — printing it twice is
        // the clutter the mark was reached for to avoid.
        // `previous_chord_symbol` is deliberately left alone,
        // so `hide_repeated_chords` still sees the run.
        let chord_result = if bar.hide_simile_chords {
            chord_renderer::ChordRenderResult::empty(*id_counter, previous_chord_symbol.clone())
        } else {
            chord_renderer::render_chord_symbols(
                &chord_ctx,
                measure,
                previous_chord_symbol.as_deref(),
                *id_counter,
                ctx,
            )
        };
        *previous_chord_symbol = chord_result.last_chord_symbol;
        *id_counter = chord_result.next_id;

        // Render spillback chord symbols (from next measure pushing back)
        let mut spillback_chords = Vec::new();
        if let Some(spillback_list) = bar.spillbacks {
            let spillback_result = chord_renderer::render_spillback_chords(
                &chord_ctx,
                spillback_list,
                previous_chord_symbol.as_deref(),
                *id_counter,
                ctx,
            );
            spillback_chords = spillback_result.nodes;
            *previous_chord_symbol = spillback_result.last_chord_symbol;
            *id_counter = spillback_result.next_id;
        }

        BarChords {
            chords: chord_result.nodes,
            spillback_chords,
            chord_bounds: chord_result.chord_bounds,
        }
    }
}
