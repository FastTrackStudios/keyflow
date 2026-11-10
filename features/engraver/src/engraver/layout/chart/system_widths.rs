//! How a system's bars share its width: each bar's content weight and
//! measured minimum, distributed across the line by the spring model.

use super::{
    ChartLayoutEngine, ChartMeasurements, constants, measure_pass, spacing, symbol_spacing,
    width_dist,
};
use crate::Chart;
use crate::chart::Measure;
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::layout::tlayout::HarmonyStyle;
use crate::sections::SectionType;

/// The bars on one system, and where to find what the measuring pass
/// learned about them.
pub(super) struct SystemBars<'a> {
    pub(super) section_idx: usize,
    pub(super) sys_idx: usize,
    pub(super) measure_indices: &'a [usize],
    /// The section's bars; `measure_indices` index into them.
    pub(super) measures: &'a [Measure],
    pub(super) chart_measurements: &'a ChartMeasurements,
    /// Index of the section's first bar in `chart_measurements`.
    pub(super) measure_offset: usize,
    pub(super) text_metrics: &'a TextFontMetrics,
    /// The line's width after its clef, key and time signature.
    pub(super) measures_area_width: f64,
}

/// What a system's width planning decided.
pub(super) struct SystemWidths {
    /// What a normal measure would be.
    pub(super) base_measure_width: f64,
    pub(super) distributed_widths: Vec<f64>,
    /// Whether the line holds fewer bars than a full one (and so is not
    /// stretched to the margin).
    pub(super) is_short_system: bool,
    /// The scale the line's chord symbols draw at: below 1.0 when a full
    /// line's dense bars would otherwise squeeze its sparse ones.
    pub(super) symbol_scale: f64,
}

impl SystemWidths {
    /// The staff's width, prefix included.
    ///
    /// For short systems, use the sum of distributed_widths (not total_width_to_distribute)
    /// because min_widths may force measures to expand beyond the planned width.
    pub(super) fn actual_system_width(&self, prefix_width: f64, content_width: f64) -> f64 {
        if self.is_short_system {
            prefix_width + self.distributed_widths.iter().sum::<f64>()
        } else {
            content_width
        }
    }
}

impl ChartLayoutEngine {
    /// === PASS 1: MEASURE ===
    /// Pre-measure all chord symbols to get accurate widths for layout:
    /// every bar of every section that draws a staff, in order.
    pub(super) fn premeasure_chart(
        &self,
        chart: &Chart,
        harmony_style: &HarmonyStyle,
    ) -> (ChartMeasurements, measure_pass::MeasurementCache) {
        let mut measurement_cache = measure_pass::MeasurementCache::new();
        let chart_measurements = measure_pass::measure_chart(
            chart
                .sections
                .iter()
                .filter(|s| {
                    !s.section.section_type.is_compact()
                        && !matches!(s.section.section_type, SectionType::End)
                })
                .map(|s| s.measures()),
            harmony_style,
            &mut measurement_cache,
            symbol_spacing::SymbolSpacing::new(
                self.config.spatium,
                self.config.min_chord_symbol_gap,
            ),
        );
        (chart_measurements, measurement_cache)
    }

    /// Plan a system's widths on a page: snippet mode sets every bar at its
    /// natural width, and a full line may draw its symbols smaller.
    pub(super) fn paginated_system_widths(&self, bars: &SystemBars<'_>) -> SystemWidths {
        let measures_area_width = bars.measures_area_width;
        let num_measures = bars.measure_indices.len();
        let max_measures = self.config.max_measures_per_system;

        // Count-in is now rendered in the header, not on the first system.
        // We no longer need to account for count-in measures in width distribution.

        let (measure_weights, measure_min_widths) = self.bar_weights_and_minimums(bars);

        // Calculate base measure width based on mode
        let base_measure_width = if self.config.snippet_mode {
            // Snippet mode: use duration-proportional natural width
            // for a quarter-note measure as the base unit
            spacing::natural_width(
                constants::TICKS_PER_QUARTER,
                self.config.spatium,
                self.config.spacing_slope,
                self.config.spacing_density,
                1.0,
            ) * 4.0 // 4 quarter notes per 4/4 measure
        } else {
            // Normal mode: distribute across available space
            measures_area_width / max_measures as f64
        };

        // For width calculation, just use the regular measure count
        let effective_measure_count = num_measures as f64;
        // Snippet mode always treated as short system (no stretching)
        let is_short_system =
            self.config.snippet_mode || effective_measure_count < max_measures as f64;

        // A full line whose chord-dense bars need so much room that
        // its sparse bars cannot keep their share draws its chord
        // symbols smaller (down to `MIN_SYSTEM_SYMBOL_SCALE`) rather
        // than squeezing a whole-bar chord, or breaking the line.
        let symbol_scale = if is_short_system {
            1.0
        } else {
            width_dist::symbol_scale_for_sparse_bars(&measure_min_widths, measures_area_width)
        };
        let measure_min_widths: Vec<f64> = measure_min_widths
            .iter()
            .map(|w| w * symbol_scale)
            .collect();

        // Distribute width proportionally using spring physics
        // For full systems, distribute the entire measures_area_width
        // For short systems, distribute only the proportional amount
        let has_spacing_expansion =
            self.has_spacing_expander(&measure_weights, &measure_min_widths, base_measure_width);
        let total_width_to_distribute = if self.config.snippet_mode {
            // Snippet mode: use duration-proportional natural widths
            // No justification stretching — just natural widths
            num_measures as f64 * base_measure_width
        } else if is_short_system && has_spacing_expansion {
            measures_area_width
        } else if is_short_system {
            // Short system: only use width proportional to measure count
            num_measures as f64 * base_measure_width
        } else {
            measures_area_width
        };

        let distributed_widths = self.distribute_bars(
            bars,
            &measure_weights,
            &measure_min_widths,
            total_width_to_distribute,
            base_measure_width,
            is_short_system,
        );
        SystemWidths {
            base_measure_width,
            distributed_widths,
            is_short_system,
            symbol_scale,
        }
    }

    /// Plan a system's widths for the continuous scroll: every line is laid
    /// out against the full bars-per-line grid, symbols at full size.
    pub(super) fn continuous_system_widths(&self, bars: &SystemBars<'_>) -> SystemWidths {
        let measures_area_width = bars.measures_area_width;
        let num_measures = bars.measure_indices.len();
        let max_measures = self.config.max_measures_per_system;

        // Count-in is now rendered in the header, not on the first system.
        // We no longer need to account for count-in measures in width distribution.

        // Calculate base measure width (what a normal measure would be)
        let base_measure_width = measures_area_width / max_measures as f64;

        // For width calculation, just use the regular measure count
        let effective_measure_count = num_measures as f64;
        let is_short_system = effective_measure_count < max_measures as f64;

        let (measure_weights, measure_min_widths) = self.bar_weights_and_minimums(bars);

        // Distribute width proportionally using spring physics
        let has_spacing_expansion =
            self.has_spacing_expander(&measure_weights, &measure_min_widths, base_measure_width);
        let total_width_to_distribute = if is_short_system && has_spacing_expansion {
            measures_area_width
        } else if is_short_system {
            num_measures as f64 * base_measure_width
        } else {
            measures_area_width
        };

        let distributed_widths = self.distribute_bars(
            bars,
            &measure_weights,
            &measure_min_widths,
            total_width_to_distribute,
            base_measure_width,
            is_short_system,
        );
        SystemWidths {
            base_measure_width,
            distributed_widths,
            is_short_system,
            symbol_scale: 1.0,
        }
    }

    /// Calculate weights and minimum widths for each measure
    /// Weights come from rhythm builder (handles triplets, etc.)
    /// Min widths come from pre-measured chord symbol widths (Pass 1 results)
    fn bar_weights_and_minimums(&self, bars: &SystemBars<'_>) -> (Vec<f64>, Vec<f64>) {
        // Calculate content weights for spring-based distribution
        // Also check for spillbacks from the next measure (triplet pushes that affect current measure width)
        let all_measures = bars.measures;
        bars.measure_indices
            .iter()
            .filter_map(|&idx| all_measures.get(idx).map(|m| (idx, m)))
            .map(|(idx, m)| {
                // Weight still uses rhythm builder (correct for triplets/complex rhythms)
                let weight = self.estimate_measure_content_weight(m, bars.text_metrics);

                // Min width from pre-measured chord widths (Pass 1)
                let global_idx = bars.measure_offset + idx;
                // A written rhythm or line of notes has a floor of
                // its own: the room its attacks need (the same
                // estimate the line breaking used), not just its
                // chord symbols'.
                let min_width = bars
                    .chart_measurements
                    .get(global_idx)
                    .map(|m| m.min_width)
                    .unwrap_or(0.0)
                    .max(self.melody_width_needed(m));

                (weight, min_width)
            })
            .unzip()
    }

    /// Share `total_width_to_distribute` across the bars, and log why.
    fn distribute_bars(
        &self,
        bars: &SystemBars<'_>,
        measure_weights: &[f64],
        measure_min_widths: &[f64],
        total_width_to_distribute: f64,
        base_measure_width: f64,
        is_short_system: bool,
    ) -> Vec<f64> {
        let distributed_widths = self.distribute_measure_widths(
            measure_weights,
            0, // No count-in measures in first system (now rendered in header)
            total_width_to_distribute,
            0.4, // compact_scale (not used when num_count_in=0)
            base_measure_width,
            measure_min_widths,
        );
        self.log_system_width_decisions(
            bars.section_idx,
            bars.sys_idx,
            bars.measure_indices,
            bars.measures,
            measure_weights,
            measure_min_widths,
            &distributed_widths,
            total_width_to_distribute,
            base_measure_width,
            is_short_system,
        );
        distributed_widths
    }
}
