//! The paginated system loop: sections into systems, systems onto pages,
//! and every bar, symbol and annotation on each line.

use super::annotations::{BarNotations, volta_at};
use super::bar_chords::BarChordFrame;
use super::section_plan::{
    MeterRun, SectionBreaking, SectionPlan, change_highlight_color, key_change_indicator_node,
    system_prefix,
};
use super::system_widths::SystemBars;
use super::timeline::{
    BarOnTimeline, CountInTiming, bar_beat_positions, chart_first_chord_is_pushed, close_beat_gaps,
    count_in_beat_positions, count_in_measure_count,
};
use super::vertical_spacing::{
    SystemInk, SystemOnPage, SystemRow, draws_above_staff, draws_below_staff,
    record_system_ink_bottom,
};
use super::{
    BeatPosition, ChartLayoutEngine, ChartLayoutResult, MeasureLayoutParams, chord_layout,
    chord_renderer, constants, detect_push_spillbacks, detect_section_start_spillback,
    engine_helpers, lyric_lane, measure_pass, melody_note_extent, notation_renderer,
    prefix_renderer, slash_glyph_for_ticks, symbol_spacing,
};
use super::{spacing, width_dist};
use crate::Chart;
use crate::chart::Measure;
use crate::engraver::layout::context::LayoutContextOwned;
use crate::engraver::layout::orchestrator::{PageLayout, SystemLayout};
use crate::engraver::layout::segment::SegmentType;
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::layout::tlayout::BarlineType;
use crate::engraver::layout::tlayout::HarmonyStyle;
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::SceneNode;
use crate::sections::SectionType;
use kurbo::Affine;
use tracing::debug;

impl ChartLayoutEngine {
    /// Layout chart in paginated mode with page breaks.
    pub(super) fn layout_paginated(
        &self,
        chart: &Chart,
        page_width: f64,
        page_height: f64,
    ) -> ChartLayoutResult {
        let ctx_owned = LayoutContextOwned::new_minimal(self.styled_for_config());
        let ctx = ctx_owned.as_context();
        let (text_metrics, harmony_style) = self.measured_fonts();

        let content_width = page_width - self.config.margins.left - self.config.margins.right;
        let content_height = page_height - self.config.margins.top - self.config.margins.bottom;
        let staff_height = self.config.spatium * 4.0;

        let mut root = SceneNode::group(SemanticId::new(ElementType::Page, 0));
        let mut pages: Vec<PageLayout> = Vec::new();
        let mut current_page_systems: Vec<SystemLayout> = Vec::new();
        let mut page_number = 1u32;
        let mut page_y = self.config.margins.top;
        let mut global_system_index = 0usize;
        // Systems whose text, figures or endings need the full gap held open.
        let mut systems_with_ink_below: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        let mut systems_with_ink_above: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        // Folded rows are measured, not discovered — see below.
        let mut folded_systems: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut id_counter = 100u64;

        // Track previous chord to hide duplicates (reset at each system start)
        let mut previous_chord_symbol: Option<String>;

        // Beat position collection for cursor/highlight rendering
        let mut beat_positions: Vec<BeatPosition> = Vec::new();
        let mut global_measure_index: usize = 0;

        // Calculate seconds per tick from tempo (480 ticks per quarter note)
        let tempo_bpm = chart.tempo.map(|t| t.bpm).unwrap_or(120.0);
        let seconds_per_quarter = 60.0 / tempo_bpm;
        let seconds_per_tick = seconds_per_quarter / 480.0;

        // Get time signature for count-in duration calculation
        let time_signature = chart
            .time_signature
            .map(|ts| (ts.numerator as u8, ts.denominator as u8))
            .unwrap_or((4u8, 4u8));

        // Prevailing meter, carried across sections/systems in playback order.
        let mut meter_run = MeterRun::new(time_signature);

        // Detect count-in from parsed chart sections if not configured explicitly.
        let count_in_measures = count_in_measure_count(chart, self.config.count_in_measures);

        // Calculate count-in duration in ticks and seconds
        // Count-in should have NEGATIVE time values so that:
        // - Count-in measures have negative time (before SONGSTART)
        // - Real measures start at time 0 (at SONGSTART)
        let ticks_per_measure = time_signature.0 as i64 * (1920i64 / time_signature.1 as i64);
        let count_in_ticks = count_in_measures as i64 * ticks_per_measure;
        let count_in_seconds = count_in_ticks as f64 * seconds_per_tick;

        // Track cumulative time and ticks through the song
        // Start at negative offset for count-in so real measures begin at 0
        let mut cumulative_time: f64 = -count_in_seconds;
        let mut cumulative_ticks: i64 = -count_in_ticks;

        // Calculate page offset for multi-page rendering
        // When use_page_offsets is false, render at origin for single-page export
        let page_offset_x = if self.config.use_page_offsets {
            20.0
        } else {
            0.0
        };
        let page_offset_y = if self.config.use_page_offsets {
            20.0
        } else {
            0.0
        };
        let page_gap = if self.config.use_page_offsets {
            40.0
        } else {
            0.0
        };

        // Track the X offset for the current page (updated when a new page starts)
        let mut current_page_x = page_offset_x;

        // Pre-compute section letters for consecutive repeats
        let section_letters = self.compute_section_letters(&chart.sections);

        // === PASS 1: MEASURE ===
        // Pre-measure all chord symbols to get accurate widths for layout
        // This replaces the estimate_measure_content_weight/compute_minimum_measure_width calls
        let (chart_measurements, measurement_cache) = self.premeasure_chart(chart, &harmony_style);

        debug!(
            "[measure-pass] Pre-measured {} measures, cached {} chord widths",
            chart_measurements.len(),
            measurement_cache.len()
        );

        // Track if we've added the title header (only on first page)
        let mut title_header_added = false;

        // Track global measure offset for looking up pre-measured values
        let mut global_section_measure_offset: usize = 0;

        // The chart's last bar closes with a final barline. A snippet is a
        // fragment, so it does not.
        let final_bar = (!self.config.snippet_mode)
            .then(|| last_drawn_bar(chart))
            .flatten();

        // Process each section
        for (section_idx, chart_section) in chart.sections.iter().enumerate() {
            // Skip count-in sections - we synthesize count-in from config.count_in_measures.
            // Advance cumulative counters by the skipped section's duration so the
            // next section starts at the correct tick (tick 0 = SONGSTART).
            if chart_section.section.section_type.is_compact() {
                let skipped_measures = chart_section.measures().len() as i64;
                cumulative_ticks += skipped_measures * ticks_per_measure;
                cumulative_time +=
                    skipped_measures as f64 * ticks_per_measure as f64 * seconds_per_tick;
                continue;
            }

            // Skip End sections entirely - they are only for progress bars, not charts
            if matches!(chart_section.section.section_type, SectionType::End) {
                continue;
            }

            let SectionPlan {
                melody_data_map,
                push_spillback_map,
                key_changes_in_section,
                section_folded,
                systems,
            } = self.plan_section(
                chart,
                section_idx,
                &SectionBreaking {
                    chart_measurements: &chart_measurements,
                    measure_offset: global_section_measure_offset,
                    content_width,
                },
            );

            for (sys_idx, measure_indices) in systems.iter().enumerate() {
                // Reset chord tracking at line breaks (new systems)
                // so repeated chords are always visible at the start of each line
                previous_chord_symbol = None;

                let (melody_extra_above, melody_extra_below) = self.system_melody_extent(
                    chart,
                    chart_section,
                    &melody_data_map,
                    measure_indices,
                );
                // MuseScore spaces systems from accumulated skyline extents
                // rather than from staff lines alone. Until this chart path has
                // a full page-level skyline pass, reserve explicit north/south
                // notation bands: chord symbols live above the staff, while
                // dynamics/text/figured bass live below.
                // A folded row is a rule and a word. It has no staff to draw,
                // no chord symbols above it and no dynamics below it, so the
                // bands that clear those have nothing to clear: charging it a
                // full system is what pushed Mr. Brightside's folded chart onto
                // a second page.
                let system_top_reserve = if section_folded {
                    self.config.spatium * 1.0
                } else {
                    self.config.spatium * 2.0
                };
                let system_measures: Vec<Measure> = measure_indices
                    .iter()
                    .filter_map(|idx| chart_section.measures().get(*idx).cloned())
                    .collect();
                let system_lyrics = lyric_lane::system_has_lyrics(chart_section, measure_indices);
                let system_bottom_reserve = if section_folded {
                    self.config.spatium * 1.0
                } else if self.config.tight_system_spacing
                    && !draws_below_staff(&system_measures)
                    && !system_lyrics
                {
                    // Nothing hangs below the staff but the odd repeat arm.
                    self.config.spatium * 1.5
                } else {
                    self.config.spatium * 4.5
                };
                let SystemRow {
                    label_height,
                    row_height,
                } = self.system_row(
                    chart_section,
                    section_folded,
                    section_letters.get(&section_idx).copied(),
                    &ctx,
                );
                let system_height = system_top_reserve
                    + melody_extra_above
                    + row_height
                    + melody_extra_below
                    + system_bottom_reserve;

                // Check for page overflow (don't include spacing - last system doesn't need it)
                if page_y + system_height > self.config.margins.top + content_height {
                    // Finalize current page (background was already added when page started)
                    if !current_page_systems.is_empty() {
                        pages.push(PageLayout {
                            number: page_number,
                            x_offset: current_page_x,
                            y_offset: page_offset_y,
                            width: page_width,
                            height: page_height,
                            systems: std::mem::take(&mut current_page_systems),
                            margins: self.config.margins,
                        });
                        page_number += 1;
                        page_y = self.config.margins.top;
                    }
                }

                // Calculate page position
                let page_x = page_offset_x + (page_number as f64 - 1.0) * (page_width + page_gap);
                let content_x = page_x + self.config.margins.left;

                // Add page background if this is first system on page
                if current_page_systems.is_empty() {
                    // Track the page X offset for this page
                    current_page_x = page_x;
                    self.add_page_background(
                        &mut root,
                        page_x,
                        page_offset_y,
                        page_width,
                        page_height,
                    );
                    self.add_page_footer(
                        &mut root,
                        page_x,
                        page_offset_y,
                        page_width,
                        page_height,
                        &chart.metadata,
                    );

                    // Add title header on first page only
                    // Include count-in snippet in header instead of on first system
                    if !title_header_added {
                        let has_pushed_first_chord = chart_first_chord_is_pushed(chart);
                        // Per-measure labels above the count-in snippet: a
                        // countdown into the downbeat, so 2 count-in measures are
                        // `-1, 0` and the first real measure is `1`.
                        let count_in_labels =
                            engine_helpers::count_in_countdown_labels(count_in_measures);
                        let (header_height, count_in_geos) = self.add_title_header(
                            &mut root,
                            page_x,
                            page_offset_y,
                            page_width,
                            &chart.metadata,
                            chart.tempo.as_ref(),
                            count_in_measures,
                            time_signature,
                            has_pushed_first_chord,
                            count_in_labels,
                        );
                        page_y += header_height;
                        title_header_added = true;

                        beat_positions.extend(count_in_beat_positions(
                            &count_in_geos,
                            page_number,
                            &CountInTiming {
                                count_in_ticks,
                                ticks_per_measure,
                                seconds_per_tick,
                                time_signature,
                            },
                        ));
                    }
                }

                // Calculate staff_y (after potential header adjustment)
                // Shift down by melody_extra_above and the above-staff reserve
                // so chord symbols on this system cannot intrude into the
                // previous system's south skyline.
                let staff_y = page_offset_y + page_y + system_top_reserve + melody_extra_above;
                let system_page_top = page_offset_y + page_y;
                let mut system_ink_bottom = staff_y + staff_height;
                let mut system_height_contributors = Vec::new();

                let prefix = system_prefix(
                    chart,
                    section_idx,
                    measure_indices,
                    &key_changes_in_section,
                    global_system_index == 0,
                    self.config.spatium,
                );
                let (clef_width, prefix_width) = (prefix.clef_width, prefix.prefix_width);

                // Calculate measure width using spring-based distribution
                let widths = self.paginated_system_widths(&SystemBars {
                    section_idx,
                    sys_idx,
                    measure_indices,
                    measures: chart_section.measures(),
                    chart_measurements: &chart_measurements,
                    measure_offset: global_section_measure_offset,
                    text_metrics: &text_metrics,
                    measures_area_width: content_width - prefix_width,
                });
                let scaled_harmony_style;
                let harmony_style: &HarmonyStyle = if widths.symbol_scale < 1.0 {
                    let size = harmony_style.root_size * widths.symbol_scale;
                    scaled_harmony_style = harmony_style.clone().with_root_size(size);
                    &scaled_harmony_style
                } else {
                    &harmony_style
                };
                let actual_system_width = widths.actual_system_width(prefix_width, content_width);
                let base_measure_width = widths.base_measure_width;
                let distributed_widths = widths.distributed_widths;

                if section_folded {
                    // The rule stands in for the staff: a line the width of
                    // the page, saying the section is here and is what it was
                    // last time.
                    root.add_child(self.draw_section_rule(
                        &chart_section.section,
                        content_x,
                        staff_y,
                        content_width,
                        label_height,
                        &ctx,
                        id_counter,
                    ));
                    id_counter += 1;
                } else {
                    // Draw staff lines (shortened for short systems)
                    root.add_child(SceneNode::anonymous_leaf(self.draw_staff_lines(
                        content_x,
                        staff_y,
                        actual_system_width,
                    )));
                }

                // System-wide chord-symbol Y baseline (used as a fallback for the
                // section label and for measures without melodies). Per-measure
                // chord_y is computed inside the measure loop below using each
                // measure's local melody extent, so a single high note in one
                // measure no longer pushes chord symbols up across the whole
                // system.
                let _chord_y = staff_y + constants::CHORD_Y_OFFSET - melody_extra_above;

                // Add section label for first system of section (skip for count-in)
                let mut section_start_dynamic_stack = None;
                if sys_idx == 0
                    && !chart_section.implicit
                    && chart_section.section.section_type.should_show_header()
                {
                    let letter = section_letters.get(&section_idx).copied();
                    let y_slots = self.repeat_pass_dynamic_slots(
                        &chart_section.section,
                        page_x,
                        self.config.margins.left,
                        staff_y,
                        staff_height,
                        &ctx,
                    );
                    section_start_dynamic_stack = Some(notation_renderer::DynamicStackAnchor {
                        x: page_x + self.config.margins.left * 0.5,
                        y_slots,
                        fallback_gap: self.config.spatium * 2.2,
                    });
                    let mut section_label = self.create_section_label(
                        &chart_section.section,
                        page_x,
                        self.config.margins.left,
                        staff_y,
                        staff_height,
                        letter,
                        &ctx,
                        id_counter,
                    );
                    id_counter += 1;
                    section_label.set_system(global_system_index as u32);
                    section_label
                        .metadata
                        .insert("page".to_string(), page_number.to_string());
                    root.add_child(section_label);
                }

                // Render system prefix (clef, key signature, and time signature)
                let ts = chart
                    .time_signature
                    .map(|ts| (ts.numerator as u8, ts.denominator as u8))
                    .unwrap_or((4u8, 4u8));

                let prefix_ctx = prefix.render_context(
                    content_x,
                    staff_y,
                    self.config.spatium,
                    self.section_clef_for(chart, chart_section),
                    Some(page_number),
                    ts,
                );

                // A folded row has no staff, so a clef and key signature on it
                // would be hanging off the front of a rule.
                if !section_folded {
                    let prefix_result =
                        prefix_renderer::render_system_prefix(&prefix_ctx, id_counter, &ctx);

                    for node in prefix_result.nodes {
                        root.add_child(node);
                    }
                    id_counter = prefix_result.next_id;
                }

                // Start measures after prefix
                let mut measure_x = content_x + prefix_width;
                let time_signature = chart
                    .time_signature
                    .map(|ts| (ts.numerator as u8, ts.denominator as u8))
                    .unwrap_or((4u8, 4u8));

                // Count-in is now rendered in the header (via add_title_header_with_count_in),
                // so we no longer render count-in measures inline on the first system.
                // Measures now start directly at measure 1.

                let mut system_skyline = notation_renderer::MeasureSkyline::new();
                let mut pending_notation: Vec<(SceneNode, bool, bool)> = Vec::new();
                // One chord-symbol baseline for the line, raised for any
                // articulations on it.
                let system_artic_lift = chord_renderer::system_articulation_lift(
                    measure_indices
                        .iter()
                        .filter_map(|&i| chart_section.measures().get(i)),
                    self.config.spatium,
                );
                for (local_measure_idx, &measure_idx) in measure_indices.iter().enumerate() {
                    if let Some(measure) = chart_section.measures().get(measure_idx) {
                        // Get preprocessed melody data for this measure (handles spillover)
                        let melody_data = melody_data_map.get(&measure_idx);

                        // Per-measure chord-symbol Y: use this measure's own melody
                        // extent rather than the system-wide max, so adjacent low
                        // measures don't get pushed up by one high measure.
                        let melody_extent = melody_data
                            .map(|md| {
                                melody_note_extent(
                                    md,
                                    self.config.spatium,
                                    self.section_proto_clef(chart, chart_section),
                                )
                                .0
                            })
                            .unwrap_or(0.0);
                        // Floor for measures that have melody notes at all: even
                        // notes inside the staff need clearance from the chord
                        // symbol above. Without this, chord text overlaps high
                        // note heads/stems when the clef-aware extent reports 0.
                        let has_melody_notes = measure
                            .melodies
                            .iter()
                            .any(|m| m.notes.iter().any(|n| !n.is_rest()));
                        let melody_floor = if has_melody_notes {
                            self.config.spatium * 1.8
                        } else {
                            0.0
                        };
                        let this_measure_extra_above = melody_extent.max(melody_floor);
                        let this_chord_y = staff_y + constants::CHORD_Y_OFFSET
                            - this_measure_extra_above
                            - system_artic_lift;

                        // Get distributed width for this measure
                        let this_measure_width = distributed_widths
                            .get(local_measure_idx)
                            .copied()
                            .unwrap_or(base_measure_width);

                        // Get spillback chords for this measure (from next measure pushing back)
                        let spillbacks = push_spillback_map.get(&measure_idx).map(|v| v.as_slice());

                        // Layout measure content (rhythm slashes) - no clef/time sig inside measure
                        // is_boundary: first measure of section needs width for pushed chords
                        // that render both here AND spill back to previous section
                        let is_section_boundary = measure_idx == 0;
                        let measure_ts = measure.time_signature;
                        let meter = meter_run.enter(measure_ts);
                        let measure_result = self.layout_measure(MeasureLayoutParams {
                            harmony_style: &harmony_style,
                            measure,
                            melody_data,
                            spillbacks,
                            measure_width: this_measure_width,
                            include_clef: false,
                            include_time_sig: meter.changed,
                            time_signature: measure_ts,
                            time_sig_color: meter.highlight.then(change_highlight_color),
                            clef: self.section_proto_clef(chart, chart_section),
                            ctx: &ctx,
                            id_base: id_counter,
                            is_boundary: is_section_boundary,
                        });
                        id_counter += 10;

                        // Red key-change indicator when a key change lands on a
                        // measure mid-system. A change on the system's first
                        // measure is shown by the red prefix key signature
                        // instead (see `system_starts_with_key_change`), so skip
                        // it here to avoid stacking two key sigs.
                        if let Some(kc) = key_changes_in_section
                            .get(&measure_idx)
                            .filter(|_| local_measure_idx != 0)
                        {
                            let kc_node = key_change_indicator_node(
                                kc,
                                measure_x,
                                staff_y,
                                self.config.spatium,
                                &ctx,
                                id_counter,
                            );
                            id_counter += 1;
                            root.add_child(kc_node);
                        }

                        // Get ChordRest segment x-positions (already in points after spacing)
                        let segment_positions: Vec<f64> =
                            self.get_chord_rest_positions(&measure_result);
                        let segment_ticks: Vec<i32> =
                            chord_layout::get_chord_rest_ticks(&measure_result);

                        let mut measure_container =
                            SceneNode::group(SemanticId::new(ElementType::Measure, id_counter));
                        id_counter += 1;
                        measure_container.transform =
                            Affine::translate((measure_x, staff_y + 2.0 * self.config.spatium));
                        measure_container
                            .metadata
                            .insert("page".to_string(), page_number.to_string());
                        notation_renderer::add_scene_obstacles(
                            &mut system_skyline,
                            &measure_result.scene,
                            measure_container.transform,
                            true,
                            true,
                        );
                        measure_container.add_child(measure_result.scene);
                        record_system_ink_bottom(
                            &mut system_ink_bottom,
                            &mut system_height_contributors,
                            &measure_container,
                            "measure",
                        );
                        root.add_child(measure_container);

                        // Add measure number on first measure of each system.
                        // Prefer the import-time source number (e.g. MusicXML
                        // `<measure number="…">`) so count-in numbering carries
                        // through — LotF's first musical bar is xml measure 3.
                        let display_measure_num = measure
                            .source_measure_number
                            .map(|n| n as i32)
                            .unwrap_or((global_measure_index as i32) + 1);
                        let is_first_of_system = local_measure_idx == 0;
                        if self.config.show_measure_numbers && is_first_of_system {
                            let measure_num_node = self.create_measure_number(
                                display_measure_num,
                                content_x,
                                staff_y,
                                id_counter,
                            );
                            id_counter += 1;
                            record_system_ink_bottom(
                                &mut system_ink_bottom,
                                &mut system_height_contributors,
                                &measure_num_node,
                                "measure_number",
                            );
                            root.add_child(measure_num_node);
                        }

                        // Collect beat positions from actual segment data
                        beat_positions.extend(bar_beat_positions(
                            &measure_result.segments,
                            &BarOnTimeline {
                                page: page_number,
                                system: global_system_index,
                                measure: global_measure_index,
                                measure_x,
                                measure_width: this_measure_width,
                                staff_y,
                                staff_height,
                                spatium: self.config.spatium,
                                time_start: cumulative_time,
                                tick_start: cumulative_ticks,
                                seconds_per_tick,
                                time_signature: measure_ts,
                            },
                        ));

                        // Advance by THIS measure's length — a bar of 2/4 in a
                        // 4/4 song is two beats, not four. The chart's opening
                        // meter here put every beat after the first meter
                        // change late by the difference, so the cursor sat on
                        // the previous bar until the old grid caught up.
                        let measure_duration_ticks =
                            measure_ts.0 as i32 * (1920 / measure_ts.1 as i32);
                        cumulative_time += measure_duration_ticks as f64 * seconds_per_tick;
                        cumulative_ticks += measure_duration_ticks as i64;
                        global_measure_index += 1;

                        // Render chord symbols using chord_renderer module
                        // Look up pre-computed measurements for this measure
                        let global_idx = global_section_measure_offset + measure_idx;
                        let bar_chords = self.render_bar_chords(
                            &BarChordFrame {
                                measure,
                                section: chart_section,
                                measure_idx,
                                local_measure_idx,
                                global_system_index,
                                page_number: Some(page_number),
                                measure_x,
                                measure_width: this_measure_width,
                                chord_y: this_chord_y,
                                articulation_lift: system_artic_lift,
                                internal_push_positions: &measure_result.internal_push_positions,
                                spillback_positions: &measure_result.spillback_positions,
                                note_line_stacks: &measure_result.note_line_stacks,
                                segment_positions: &segment_positions,
                                segment_ticks: &segment_ticks,
                                spillbacks,
                                harmony_style: &harmony_style,
                                time_signature,
                                measure_measurements: chart_measurements.get(global_idx),
                                hide_simile_chords: measure.simile && self.config.draw_similes,
                            },
                            &mut previous_chord_symbol,
                            &mut id_counter,
                            &ctx,
                        );
                        let chord_obstacles = bar_chords.chord_bounds;
                        // Kept for anchoring suspension figures to their chords;
                        // `chord_obstacles` itself is consumed as obstacles below.
                        let suspension_chord_bounds = chord_obstacles.clone();

                        for node in &bar_chords.chords {
                            notation_renderer::add_scene_obstacles(
                                &mut system_skyline,
                                node,
                                Affine::IDENTITY,
                                true,
                                true,
                            );
                        }
                        for node in bar_chords.chords {
                            record_system_ink_bottom(
                                &mut system_ink_bottom,
                                &mut system_height_contributors,
                                &node,
                                "chord",
                            );
                            root.add_child(node);
                        }
                        for node in &bar_chords.spillback_chords {
                            notation_renderer::add_scene_obstacles(
                                &mut system_skyline,
                                node,
                                Affine::IDENTITY,
                                true,
                                true,
                            );
                        }
                        for node in bar_chords.spillback_chords {
                            record_system_ink_bottom(
                                &mut system_ink_bottom,
                                &mut system_height_contributors,
                                &node,
                                "spillback_chord",
                            );
                            root.add_child(node);
                        }

                        // Render text cues below the staff. They are routed
                        // through the notation skyline below so they avoid
                        // dynamics, figured bass, hairpins, and staff text.
                        let cue_nodes = if !measure.text_cues.is_empty() {
                            chord_renderer::render_text_cues(
                                &measure.text_cues,
                                measure_x,
                                staff_y,
                                staff_height,
                                self.config.spatium,
                                &mut id_counter,
                            )
                        } else {
                            Vec::new()
                        };

                        // Staff-attached notations (dynamics, staff text, figured bass, hairpins).
                        let notation_frame = notation_renderer::MeasureFrame {
                            measure_x,
                            measure_width: this_measure_width,
                            staff_y,
                            staff_height,
                            chord_y: this_chord_y,
                            spatium: self.config.spatium,
                            beats_per_measure: time_signature.0,
                            source_measure_width: measure.source_measure_width,
                            segment_positions: Some(&segment_positions),
                            system_start_dynamic_x: (local_measure_idx == 0)
                                .then_some(content_x + clef_width * 0.5),
                            section_start_dynamic_stack: (sys_idx == 0 && local_measure_idx == 0)
                                .then_some(section_start_dynamic_stack.clone())
                                .flatten(),
                        };
                        // Per-system skyline (mirrors MuseScore Skyline + Autoplace).
                        // Seed with the actual rendered chord bounds so
                        // notations avoid chord text without treating the
                        // whole measure as occupied.
                        for bounds in chord_obstacles {
                            system_skyline.add_above(bounds);
                            system_skyline.add_below(bounds);
                        }
                        // Seed below skyline with the staff itself so below
                        // elements don't intrude into the staff lines.
                        let staff_band = kurbo::Rect::new(
                            if local_measure_idx == 0 {
                                content_x
                            } else {
                                measure_x
                            },
                            staff_y - self.config.spatium * 0.2,
                            measure_x + this_measure_width,
                            staff_y + staff_height + self.config.spatium * 0.4,
                        );
                        system_skyline.add_above(staff_band);
                        system_skyline.add_below(staff_band);
                        let autoplace_gap = self.config.spatium * 0.1;
                        pending_notation.extend(self.deferred_bar_notations(
                            BarNotations {
                                measure,
                                section_measures: chart_section.measures(),
                                measure_idx,
                                frame: &notation_frame,
                                chord_bounds: &suspension_chord_bounds,
                                cue_nodes,
                                bar_end: measure_x + this_measure_width,
                                line_end: content_x + content_width,
                            },
                            page_number,
                            global_system_index,
                            &text_metrics,
                            &mut id_counter,
                        ));
                        if let Some(span) = volta_at(
                            chart_section.measures(),
                            measure_idx,
                            local_measure_idx == 0,
                        ) {
                            let volta_end_local =
                                (local_measure_idx + span.remaining).min(distributed_widths.len());
                            let volta_x_end = measure_x
                                + distributed_widths[local_measure_idx..volta_end_local]
                                    .iter()
                                    .sum::<f64>();
                            let closes_in_system =
                                local_measure_idx + span.remaining <= distributed_widths.len();
                            let volta_node = notation_renderer::render_volta_span(
                                span.volta,
                                &notation_frame,
                                volta_x_end,
                                closes_in_system && span.closed,
                                span.continued,
                                &mut id_counter,
                            );
                            if let Some(volta_node) = notation_renderer::autoplace_node(
                                &mut system_skyline,
                                volta_node,
                                true,
                                autoplace_gap,
                            ) {
                                record_system_ink_bottom(
                                    &mut system_ink_bottom,
                                    &mut system_height_contributors,
                                    &volta_node,
                                    "volta",
                                );
                                root.add_child(volta_node);
                            }
                        }

                        // Start-of-measure forward repeat (draw at left edge of this measure).
                        if matches!(
                            measure.start_repeat,
                            crate::chart::notations::RepeatMark::Forward
                        ) {
                            let barline = self.draw_barline(
                                measure_x,
                                staff_y,
                                staff_height,
                                BarlineType::StartRepeat,
                            );
                            record_system_ink_bottom(
                                &mut system_ink_bottom,
                                &mut system_height_contributors,
                                &barline,
                                "start_repeat",
                            );
                            root.add_child(barline);
                        }

                        measure_x += this_measure_width;

                        // Closing barline — style + end-repeat aware, and
                        // thin-thick on the chart's last bar.
                        let barline_type = match Self::end_barline_type(measure) {
                            BarlineType::Single
                                if final_bar == Some((section_idx, measure_idx)) =>
                            {
                                BarlineType::End
                            }
                            other => other,
                        };
                        let barline =
                            self.draw_barline(measure_x, staff_y, staff_height, barline_type);
                        record_system_ink_bottom(
                            &mut system_ink_bottom,
                            &mut system_height_contributors,
                            &barline,
                            "barline",
                        );
                        root.add_child(barline);

                        // How many times the repeat plays, over its closing
                        // barline — including the `x2` a bare repeat implies.
                        // Strictly, engraving leaves that one off. But the
                        // count is the thing a player is scanning for, and it
                        // is easier to find in the same place every time than
                        // to notice an absence and know what it means.
                        if matches!(
                            measure.end_repeat,
                            crate::chart::notations::RepeatMark::Backward
                        ) {
                            let label = self.create_repeat_count_label(
                                measure.repeat_count.max(2),
                                measure_x,
                                staff_y,
                                id_counter,
                            );
                            id_counter += 1;
                            // The count starts just above the staff, where the
                            // bracket ends — and rides the skyline up from
                            // there, because the last bar of a repeat usually
                            // has a chord symbol sitting in exactly that spot.
                            if let Some(label) = notation_renderer::autoplace_node(
                                &mut system_skyline,
                                label,
                                true,
                                self.config.spatium * 0.1,
                            ) {
                                record_system_ink_bottom(
                                    &mut system_ink_bottom,
                                    &mut system_height_contributors,
                                    &label,
                                    "repeat_count",
                                );
                                root.add_child(label);
                            }
                        }
                    }
                }

                let autoplace_gap = self.config.spatium * 0.1;
                let text_shrink_displacement = self.config.spatium * 4.0;
                for (node, above, shrinkable) in pending_notation {
                    let placed = if shrinkable {
                        notation_renderer::autoplace_text_node(
                            &mut system_skyline,
                            node,
                            above,
                            autoplace_gap,
                            text_shrink_displacement,
                        )
                    } else {
                        notation_renderer::autoplace_node(
                            &mut system_skyline,
                            node,
                            above,
                            autoplace_gap,
                        )
                    };
                    if let Some(node) = placed {
                        record_system_ink_bottom(
                            &mut system_ink_bottom,
                            &mut system_height_contributors,
                            &node,
                            "notation",
                        );
                        root.add_child(node);
                    }
                }

                // The lyric lane goes under everything else below the staff.
                if system_lyrics && !section_folded {
                    let lyric_system = lyric_lane::LyricSystem {
                        section: chart_section,
                        measure_indices,
                        first_measure_x: content_x + prefix_width,
                        widths: &distributed_widths,
                        staff_y,
                        staff_height,
                        spatium: self.config.spatium,
                        style: self.style(),
                    };
                    for node in lyric_lane::place_system_lyrics(
                        &mut system_skyline,
                        &lyric_system,
                        &text_metrics,
                        &mut id_counter,
                    ) {
                        record_system_ink_bottom(
                            &mut system_ink_bottom,
                            &mut system_height_contributors,
                            &node,
                            "lyrics",
                        );
                        root.add_child(node);
                    }
                }

                // Track system layout
                let actual_system_height = self.drawn_system_height(&SystemInk {
                    chart_section,
                    measure_indices,
                    section_folded,
                    page_number,
                    global_system_index,
                    base_height: system_top_reserve + row_height,
                    page_top: system_page_top,
                    ink_bottom: system_ink_bottom,
                    contributors: &system_height_contributors,
                });

                if section_folded {
                    folded_systems.insert(global_system_index);
                } else {
                    if draws_below_staff(&system_measures) || system_lyrics {
                        systems_with_ink_below.insert(global_system_index);
                    }
                    if draws_above_staff(&system_measures) {
                        systems_with_ink_above.insert(global_system_index);
                    }
                }
                current_page_systems.push(SystemLayout {
                    index: global_system_index,
                    y: page_y,
                    width: content_width,
                    height: actual_system_height,
                    measure_indices: measure_indices.clone(),
                });

                let spacing = self.gap_after_system(&SystemOnPage {
                    chart,
                    section_idx,
                    systems: &systems,
                    sys_idx,
                    section_folded,
                    system_measures: &system_measures,
                    system_lyrics,
                });
                page_y += actual_system_height + spacing;
                global_system_index += 1;
            }

            // A folded section drew no measures, so nothing inside the loop
            // advanced the counters those measures are still worth. The bars
            // are played whether or not they are drawn: skipping them here
            // would restart the bar numbers after the fold and slide every
            // later beat position earlier than the music.
            if section_folded {
                let bars = chart_section.measures().len();
                for measure in chart_section.measures() {
                    let ticks = i64::from(measure.time_signature.0)
                        * (1920 / i64::from(measure.time_signature.1));
                    cumulative_ticks += ticks;
                    cumulative_time += ticks as f64 * seconds_per_tick;
                }
                global_measure_index += bars;
            }

            // Update global measure offset for next section (for chart_measurements lookup)
            global_section_measure_offset += chart_section.measures().len();
        }

        // Finalize last page
        if !current_page_systems.is_empty() {
            pages.push(PageLayout {
                number: page_number,
                x_offset: current_page_x,
                y_offset: page_offset_y,
                width: page_width,
                height: page_height,
                systems: current_page_systems,
                margins: self.config.margins,
            });
        }

        let total_width = page_offset_x * 2.0 + page_number as f64 * (page_width + page_gap);
        let total_height = page_height + page_offset_y * 2.0;

        close_beat_gaps(&mut beat_positions);

        let mut result = ChartLayoutResult {
            scene: root,
            pages,
            total_height,
            total_width,
            beat_positions,
        };
        self.compact_paginated_vertical_dead_space(
            &mut result,
            &systems_with_ink_below,
            &systems_with_ink_above,
            &folded_systems,
        );
        result
    }
}

/// The (section, bar) of the chart's last drawn bar: the last bar of the
/// last section that draws a staff.
fn last_drawn_bar(chart: &Chart) -> Option<(usize, usize)> {
    chart
        .sections
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, section)| {
            let drawn = !section.section.section_type.is_compact()
                && !matches!(section.section.section_type, SectionType::End);
            let bars = section.measures().len();
            (drawn && bars > 0).then(|| (idx, bars - 1))
        })
}
