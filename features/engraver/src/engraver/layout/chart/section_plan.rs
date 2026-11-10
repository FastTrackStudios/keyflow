//! What a section brings to the system loop: its bars grouped into systems,
//! the chords pushed across its barlines, the key and meter in force where
//! each line starts, the prefix that opens each line, and the red marks for
//! mid-chart changes.

use std::collections::HashMap;

use super::{
    ChartLayoutEngine, ChartMeasurements, MeasureMelodyData, PushSpillback, detect_push_spillbacks,
    detect_section_start_spillback, prefix_renderer,
};
use crate::Chart;
use crate::engraver::layout::context::LayoutContext;
use crate::engraver::scene::node::SceneNode;
use crate::key::KeySpelling;
use crate::sections::SectionType;
use crate::time::MusicalPositionExt as _;
use keyflow_proto::chart::types::KeyChange;
use kurbo::Affine;
use tracing::debug;

pub(super) fn key_signature_fifths(chart: &Chart) -> i8 {
    chart
        .initial_key
        .as_ref()
        .map(|key| {
            let is_major = key.mode.name() == "Ionian";
            KeySpelling::new(key.root(), is_major).accidental_count()
        })
        .unwrap_or(0)
}

/// Prevailing key-signature fifths in effect at a section-local measure,
/// applying every key change at or before it (in playback order). Lets each
/// system's prefix show the key actually sounding there rather than always the
/// chart's opening key.
pub(super) fn prevailing_fifths_at(chart: &Chart, section_idx: usize, local_measure: usize) -> i8 {
    let mut fifths = chart
        .initial_key
        .as_ref()
        .map(prefix_renderer::key_to_fifths)
        .unwrap_or(0);
    for kc in &chart.key_changes {
        let kc_measure = kc.position.total_duration.measure as usize;
        let applies = kc.section_index < section_idx
            || (kc.section_index == section_idx && kc_measure <= local_measure);
        if applies {
            fifths = prefix_renderer::key_to_fifths(&kc.to_key);
        }
    }
    fifths
}

/// The clef, key signature and time signature that open a system, and the
/// room they take.
pub(super) struct SystemPrefix {
    pub(super) include_clef: bool,
    pub(super) include_key_sig: bool,
    pub(super) include_time_sig: bool,
    pub(super) key_signature: i8,
    /// The key a change on the line's first bar comes from, whose
    /// accidentals the prefix cancels.
    pub(super) key_change_from: Option<i8>,
    pub(super) starts_with_key_change: bool,
    pub(super) clef_width: f64,
    pub(super) key_sig_width: f64,
    pub(super) time_sig_width: f64,
    pub(super) prefix_width: f64,
}

/// Plan the prefix of the system holding `measure_indices` of section
/// `section_idx`. Only the chart's first system carries a time signature.
pub(super) fn system_prefix(
    chart: &Chart,
    section_idx: usize,
    measure_indices: &[usize],
    key_changes_in_section: &HashMap<usize, &KeyChange>,
    first_system: bool,
    spatium: f64,
) -> SystemPrefix {
    // Calculate system prefix width (clef + key sig + time sig) FIRST
    // Needed to determine staff line width for short systems
    let include_clef = true; // Always show clef at system start
    let include_key_sig = true; // Key sig on every system (standard notation)
    let include_time_sig = first_system; // Time sig only on first system

    // Key signature prevailing at this system's downbeat (follows
    // mid-chart key changes), and whether the change happens right
    // here — if so the prefix is drawn red in place rather than
    // stacking a separate red indicator over it.
    let system_first_measure = measure_indices.first().copied().unwrap_or(0);
    let key_signature: i8 = prevailing_fifths_at(chart, section_idx, system_first_measure);
    let system_starts_with_key_change = measure_indices
        .first()
        .is_some_and(|m| key_changes_in_section.contains_key(m));

    let (clef_width, key_sig_width, time_sig_width, prefix_width) =
        prefix_renderer::calculate_prefix_width(
            spatium,
            include_clef,
            include_key_sig,
            key_signature,
            include_time_sig,
        );
    // A key change on the line's first bar is printed in the
    // prefix, naturals first: B to C cancels five sharps rather
    // than silently dropping them.
    let key_change_from: Option<i8> = measure_indices
        .first()
        .and_then(|m| key_changes_in_section.get(m))
        .and_then(|kc| kc.from_key.as_ref())
        .map(prefix_renderer::key_to_fifths);
    let naturals_width = key_change_from.map_or(0.0, |prev| {
        prefix_renderer::naturals_width(prev, key_signature, spatium)
    });
    SystemPrefix {
        include_clef,
        include_key_sig,
        include_time_sig,
        key_signature,
        key_change_from,
        starts_with_key_change: system_starts_with_key_change,
        clef_width,
        key_sig_width: key_sig_width + naturals_width,
        time_sig_width,
        prefix_width: prefix_width + naturals_width,
    }
}

impl SystemPrefix {
    /// What the prefix renderer needs to draw this prefix at `x`.
    pub(super) fn render_context(
        &self,
        x: f64,
        staff_y: f64,
        spatium: f64,
        clef_type: crate::engraver::layout::tlayout::clef::ClefType,
        page_number: Option<u32>,
        time_signature: (u8, u8),
    ) -> prefix_renderer::PrefixRenderContext {
        prefix_renderer::PrefixRenderContext {
            x,
            staff_y,
            spatium,
            include_clef: self.include_clef,
            include_key_sig: self.include_key_sig,
            include_time_sig: self.include_time_sig,
            key_signature: self.key_signature,
            prev_key_signature: self.key_change_from,
            key_sig_color: self.starts_with_key_change.then(change_highlight_color),
            time_signature,
            clef_width: self.clef_width,
            clef_type,
            key_sig_width: self.key_sig_width,
            time_sig_width: self.time_sig_width,
            page_number,
        }
    }
}

/// One section, ready for the system loop: its melodies carried across
/// barlines, the chords the next bar pushes back into each bar, its key
/// changes, and the bars grouped into systems.
pub(super) struct SectionPlan<'c> {
    pub(super) melody_data_map: HashMap<usize, MeasureMelodyData>,
    pub(super) push_spillback_map: HashMap<usize, Vec<PushSpillback>>,
    /// Mid-chart key changes, keyed by section-local measure index.
    pub(super) key_changes_in_section: HashMap<usize, &'c KeyChange>,
    pub(super) section_folded: bool,
    pub(super) systems: Vec<Vec<usize>>,
}

/// Where a section's bars sit in the measuring pass's results, and the line
/// width they are broken against.
pub(super) struct SectionBreaking<'m> {
    pub(super) chart_measurements: &'m ChartMeasurements,
    /// Index of the section's first bar in `chart_measurements`.
    pub(super) measure_offset: usize,
    pub(super) content_width: f64,
}

impl ChartLayoutEngine {
    /// Plan one section for the system loop. Both layout modes walk the
    /// same plan; what differs is where they put the systems.
    pub(super) fn plan_section<'c>(
        &self,
        chart: &'c Chart,
        section_idx: usize,
        breaking: &SectionBreaking<'_>,
    ) -> SectionPlan<'c> {
        let chart_section = &chart.sections[section_idx];
        let time_signature = chart
            .time_signature
            .map(|ts| (ts.numerator as u8, ts.denominator as u8))
            .unwrap_or((4u8, 4u8));

        // Preprocess melodies to handle cross-measure spillover
        let beats_per_measure = time_signature.0 as f64;
        let melody_data_map = self.expanded_melody_data(chart, chart_section, beats_per_measure);

        // Detect push spillbacks (chords from next measure that push back)
        let mut push_spillback_map = detect_push_spillbacks(chart_section.measures());

        if !push_spillback_map.is_empty() {
            debug!(
                "[spillback-detection] Section {} detected {} spillbacks: {:?}",
                chart_section.section.section_type.full_name(),
                push_spillback_map.values().map(|v| v.len()).sum::<usize>(),
                push_spillback_map
                    .iter()
                    .map(|(k, v)| (
                        k,
                        v.iter()
                            .map(|s| (&s.chord_symbol, &s.push_base))
                            .collect::<Vec<_>>()
                    ))
                    .collect::<Vec<_>>()
            );
        }

        // Check for cross-section spillback: if the NEXT section starts with a pushed chord,
        // it spills back to THIS section's last measure
        let next_non_compact_section = chart.sections.iter().skip(section_idx + 1).find(|s| {
            !s.section.section_type.is_compact()
                && !matches!(s.section.section_type, SectionType::End)
        });
        if let Some(next_section) = next_non_compact_section
            && let Some(mut spillback) = detect_section_start_spillback(next_section.measures())
        {
            // Adjust beat position based on this section's last measure time signature
            if let Some(last_measure) = chart_section.measures().last() {
                let ts = last_measure.time_signature;
                spillback.beat_position = (ts.0 as usize).saturating_sub(1);
            }
            let last_measure_idx = chart_section.measures().len().saturating_sub(1);

            debug!(
                "[cross-section-spillback] Section {} -> {} spillback: '{}' at measure {} beat {}",
                chart_section.section.section_type.full_name(),
                next_section.section.section_type.full_name(),
                spillback.chord_symbol,
                last_measure_idx,
                spillback.beat_position
            );

            push_spillback_map
                .entry(last_measure_idx)
                .or_default()
                .push(spillback);
        }

        // Mid-chart key changes in this section, keyed by section-local
        // measure index (the parser stores the change's measure ordinal in
        // total_duration.measures).
        let key_changes_in_section: HashMap<usize, _> = chart
            .key_changes
            .iter()
            .filter(|kc| kc.section_index == section_idx)
            .map(|kc| (kc.position.total_duration.measure as usize, kc))
            .collect();

        // A folded section draws as one titled rule rather than its bars,
        // so it needs exactly one system and that system holds nothing.
        // Everything else about a system — the page break check, the page
        // background, the section label, the vertical advance — still has
        // to happen, which is why this is an empty system rather than a
        // separate path around the loop.
        let section_folded = chart_section.folded && self.config.fold_sections;
        let systems = if section_folded {
            vec![Vec::new()]
        } else {
            self.break_section_into_systems(chart, section_idx, breaking)
        };

        SectionPlan {
            melody_data_map,
            push_spillback_map,
            key_changes_in_section,
            section_folded,
            systems,
        }
    }

    fn break_section_into_systems(
        &self,
        chart: &Chart,
        section_idx: usize,
        breaking: &SectionBreaking<'_>,
    ) -> Vec<Vec<usize>> {
        let chart_section = &chart.sections[section_idx];
        // The measuring pass's minimums: what each bar's printed
        // symbols really take, which the grouping's own estimate
        // can undercount.
        let measured: Vec<f64> = (0..chart_section.measures().len())
            .map(|i| {
                breaking
                    .chart_measurements
                    .get(breaking.measure_offset + i)
                    .map_or(0.0, |m| m.min_width)
            })
            .collect();
        // The clef, key and time signature take the start of
        // every line: what is left is the room for bars.
        let (_, _, _, prefix_width) = prefix_renderer::calculate_prefix_width(
            self.config.spatium,
            true,
            true,
            prevailing_fifths_at(chart, section_idx, 0),
            true,
        );
        self.group_measures_into_systems(
            chart_section.measures(),
            breaking.content_width - prefix_width,
            &measured,
        )
    }
}

/// The meter in force as the bars go by, in playback order.
///
/// Prevailing meter, carried across sections/systems in playback order so a
/// measure whose `time_signature` differs from its predecessor renders an
/// inline meter change. Seeded with the chart's initial meter.
/// `previous` + `run_len` detect a one-measure excursion (e.g.
/// the auto-revert after `!T2/4`): that revert is drawn but NOT highlighted,
/// since the return-to-normal is adjacent and obvious. A revert that is
/// several measures after the change is highlighted like any other change.
pub(super) struct MeterRun {
    prevailing: (u8, u8),
    previous: Option<(u8, u8)>,
    run_len: usize,
}

/// What a bar's meter does to the line: whether it prints a new time
/// signature, and whether that one is red.
pub(super) struct MeterChange {
    pub(super) changed: bool,
    pub(super) highlight: bool,
}

impl MeterRun {
    pub(super) fn new(initial: (u8, u8)) -> Self {
        Self {
            prevailing: initial,
            previous: None,
            run_len: 0,
        }
    }

    /// Step into the next bar, which is in `measure_ts`.
    ///
    /// A measure whose meter differs from the prevailing one
    /// renders the new time signature inline. It is highlighted
    /// red unless it merely closes a one-measure excursion (the
    /// auto-revert after `!T2/4`): that return-to-normal renders
    /// in default black because it is adjacent and obvious.
    pub(super) fn enter(&mut self, measure_ts: (u8, u8)) -> MeterChange {
        let ts_changed = measure_ts != self.prevailing;
        let is_oneshot_revert =
            ts_changed && self.run_len == 1 && self.previous == Some(measure_ts);
        let highlight = ts_changed && !is_oneshot_revert;
        if ts_changed {
            self.previous = Some(self.prevailing);
            self.prevailing = measure_ts;
            self.run_len = 1;
        } else {
            self.run_len += 1;
        }
        MeterChange {
            changed: ts_changed,
            highlight,
        }
    }
}

/// Red used to highlight mid-chart meter and key changes so they are obvious.
pub(super) fn change_highlight_color() -> peniko::Color {
    peniko::Color::from_rgba8(0xCC, 0x00, 0x00, 0xFF)
}

/// Build a red, in-place key-change indicator (cancelling naturals + the new
/// key's accidentals, MuseScore-style) sitting in the barline gap just before
/// `measure_x`, on the staff. Returns the positioned scene node.
pub(super) fn key_change_indicator_node(
    kc: &keyflow_proto::chart::types::KeyChange,
    measure_x: f64,
    staff_y: f64,
    spatium: f64,
    ctx: &LayoutContext,
    id: u64,
) -> SceneNode {
    use crate::engraver::layout::tlayout::keysig::{
        ClefContext, KeySigParams, KeySigType, layout_keysig,
    };
    use prefix_renderer::key_to_fifths;

    let to_fifths = key_to_fifths(&kc.to_key);
    let prev_fifths = kc.from_key.as_ref().map(key_to_fifths);
    let params = KeySigParams {
        id,
        key: KeySigType::Standard(to_fifths),
        clef: ClefContext::Treble,
        show_naturals: prev_fifths.is_some(),
        prev_key: prev_fifths.map(KeySigType::Standard),
        color: Some(change_highlight_color()),
    };
    let (layout, mut node) = layout_keysig(&params, ctx);
    let width = layout.bbox.width();
    let x = (measure_x - width - spatium * 0.5).max(0.0);
    node.transform = Affine::translate((x, staff_y + 2.0 * spatium));
    node
}
