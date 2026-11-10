//! The continuous-scroll system loop: one tall page, systems stacked down
//! it, with the same bars, symbols and annotations the paginated loop draws.

use super::annotations::{BarNotations, volta_at};
use super::bar_chords::BarChordFrame;
use super::section_plan::{
    MeterRun, SectionBreaking, SectionPlan, change_highlight_color, key_change_indicator_node,
    system_prefix,
};
use super::system_widths::SystemBars;
use super::timeline::{
    BarOnTimeline, bar_beat_positions, chart_first_chord_is_pushed, count_in_measure_count,
};
use super::vertical_spacing::{SystemRow, bare_system_gap};
use super::{
    BeatPosition, ChartLayoutEngine, ChartLayoutResult, MeasureLayoutParams, chord_layout,
    chord_renderer, constants, detect_push_spillbacks, detect_section_start_spillback,
    engine_helpers, lyric_lane, measure_pass, melody_note_extent, notation_renderer,
    prefix_renderer, slash_glyph_for_ticks, symbol_spacing,
};
use crate::Chart;
use crate::engraver::layout::context::LayoutContextOwned;
use crate::engraver::layout::segment::SegmentType;
use crate::engraver::layout::text_metrics::TextFontMetrics;
use crate::engraver::layout::tlayout::BarlineType;
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::SceneNode;
use crate::sections::SectionType;
use kurbo::Affine;
use tracing::debug;

impl ChartLayoutEngine {
    /// Layout chart in continuous scroll mode.
    pub(super) fn layout_continuous(&self, chart: &Chart, width: f64) -> ChartLayoutResult {
        let ctx_owned = LayoutContextOwned::new_minimal(self.styled_for_config());
        let ctx = ctx_owned.as_context();
        let (text_metrics, harmony_style) = self.measured_fonts();

        let content_width = width - self.config.margins.left - self.config.margins.right;
        let content_x = self.config.margins.left;
        let staff_height = self.config.spatium * 4.0;

        let mut root = SceneNode::group(SemanticId::new(ElementType::Page, 0));
        let mut total_height = self.config.margins.top;
        let mut id_counter = 100u64;
        let mut global_system_index = 0usize;
        let mut global_measure_index = 0usize;

        // Time signature — computed early because the count-in snippet needs the
        // meter for its beat geometry. (Reused below for the measure passes.)
        let time_signature = chart
            .time_signature
            .map(|ts| (ts.numerator as u8, ts.denominator as u8))
            .unwrap_or((4u8, 4u8));

        // Count-in measure count: an explicit config override, else the chart's
        // own `CountIn` section length. In continuous mode the count-in is a
        // compact snippet drawn in the header ("up by the tempo markers"), a
        // VISUAL pre-roll only — it is NOT part of the scrolling measure
        // timeline (real measures still start at t=0 / tick 0), so the live
        // cursor maps transport progress straight onto the real measures.
        let count_in_measures = count_in_measure_count(chart, self.config.count_in_measures);

        // Title header (title / artist / tempo / key) above the music — the same
        // block the paginated path draws, but only when the chart actually names
        // itself. A bare chord snippet (`Cmaj7 | …`) stays header-free and tight;
        // a titled chart (`Vienna - Billy Joel`) shows its header in the inline
        // render too, now including the count-in snippet.
        if chart.metadata.title.is_some() {
            let has_pushed_first_chord = chart_first_chord_is_pushed(chart);
            // Countdown labels into the downbeat (`-1, 0` for a 2-bar count-in).
            let count_in_labels = engine_helpers::count_in_countdown_labels(count_in_measures);
            let (header_height, _) = self.add_title_header(
                &mut root,
                0.0,
                total_height,
                width,
                &chart.metadata,
                chart.tempo.as_ref(),
                count_in_measures,
                time_signature,
                has_pushed_first_chord,
                count_in_labels,
            );
            total_height += header_height;
        }

        // Track previous chord to hide duplicates (reset at each system start)
        let mut previous_chord_symbol: Option<String>;

        // Beat position collection for cursor / highlight rendering / DAW transport sync.
        let mut beat_positions: Vec<BeatPosition> = Vec::new();

        // Tempo / tick bookkeeping (same conventions as paginated path):
        // 480 ticks per quarter note. Continuous mode has no count-in synthesis,
        // so the song timeline starts at t=0, tick=0.
        let tempo_bpm = chart.tempo.map(|t| t.bpm).unwrap_or(120.0);
        let seconds_per_tick = (60.0 / tempo_bpm) / 480.0;
        let mut cumulative_time = 0.0f64;
        let mut cumulative_ticks = 0i64;

        // Pre-compute section letters for consecutive repeats
        let section_letters = self.compute_section_letters(&chart.sections);

        // === PASS 1: MEASURE ===
        // Pre-measure all chord symbols to get accurate widths for layout
        let (chart_measurements, _) = self.premeasure_chart(chart, &harmony_style);

        // Prevailing meter, carried across sections/systems in playback order.
        let mut meter_run = MeterRun::new(time_signature);

        // Track offset into `chart_measurements`. Compact and End sections are
        // filtered out of the measure pass, so this offset only advances for
        // sections that were actually pre-measured.
        let mut global_section_measure_offset: usize = 0;

        // Process each section
        for (section_idx, chart_section) in chart.sections.iter().enumerate() {
            // Skip count-in sections - we synthesize count-in from config.
            if chart_section.section.section_type.is_compact() {
                continue;
            }

            // Skip End sections entirely - they are only for progress bars, not charts
            if matches!(chart_section.section.section_type, SectionType::End) {
                continue;
            }

            // A folded section draws as one titled rule rather than its bars,
            // exactly as it does on a page. Scrolling a chart is not a reason
            // to read a chorus out twice.
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

                // Match the paginated path: reserve skyline-like north/south
                // bands so adjacent systems cannot overlap above/below staff
                // notation before a full page-level skyline pass exists.
                let SystemRow {
                    label_height,
                    row_height,
                } = self.system_row(
                    chart_section,
                    section_folded,
                    section_letters.get(&section_idx).copied(),
                    &ctx,
                );
                let system_top_reserve = if section_folded {
                    self.config.spatium * 1.0
                } else {
                    self.config.spatium * 2.0
                };
                let system_lyrics = !section_folded
                    && lyric_lane::system_has_lyrics(chart_section, measure_indices);
                let system_bottom_reserve = if section_folded {
                    self.config.spatium * 1.0
                } else if system_lyrics {
                    // Room for the lyric line under any cues.
                    self.config.spatium * 8.0
                } else {
                    self.config.spatium * 4.5
                };
                let staff_y = total_height + system_top_reserve + melody_extra_above;
                let system_height = system_top_reserve
                    + melody_extra_above
                    + row_height
                    + melody_extra_below
                    + system_bottom_reserve;

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
                let widths = self.continuous_system_widths(&SystemBars {
                    section_idx,
                    sys_idx,
                    measure_indices,
                    measures: chart_section.measures(),
                    chart_measurements: &chart_measurements,
                    measure_offset: global_section_measure_offset,
                    text_metrics: &text_metrics,
                    measures_area_width: content_width - prefix_width,
                });
                let actual_system_width = widths.actual_system_width(prefix_width, content_width);
                let base_measure_width = widths.base_measure_width;
                let distributed_widths = widths.distributed_widths;

                if section_folded {
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

                // Place chord symbols above the highest note content (MuseScore skyline approach)
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
                        0.0,
                        self.config.margins.left,
                        staff_y,
                        staff_height,
                        &ctx,
                    );
                    section_start_dynamic_stack = Some(notation_renderer::DynamicStackAnchor {
                        x: self.config.margins.left * 0.5,
                        y_slots,
                        fallback_gap: self.config.spatium * 2.2,
                    });
                    let section_label = self.create_section_label(
                        &chart_section.section,
                        0.0,
                        self.config.margins.left,
                        staff_y,
                        staff_height,
                        letter,
                        &ctx,
                        id_counter,
                    );
                    id_counter += 1;
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
                    None, // Continuous mode has no pages
                    ts,
                );

                // A folded row has no staff, so a clef and key signature on it
                // would be hanging in the air.
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
                        let melody_data = melody_data_map.get(&measure_idx);

                        // Per-measure chord-symbol Y (see paginated path for rationale).
                        let this_measure_extra_above = melody_data
                            .map(|md| {
                                melody_note_extent(
                                    md,
                                    self.config.spatium,
                                    self.section_proto_clef(chart, chart_section),
                                )
                                .0
                            })
                            .unwrap_or(0.0);
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

                        // is_boundary: first measure of section needs width for pushed chords
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

                        let segment_positions: Vec<f64> =
                            self.get_chord_rest_positions(&measure_result);
                        let segment_ticks: Vec<i32> =
                            chord_layout::get_chord_rest_ticks(&measure_result);

                        let mut measure_container =
                            SceneNode::group(SemanticId::new(ElementType::Measure, id_counter));
                        id_counter += 1;
                        measure_container.transform =
                            Affine::translate((measure_x, staff_y + 2.0 * self.config.spatium));
                        notation_renderer::add_scene_obstacles(
                            &mut system_skyline,
                            &measure_result.scene,
                            measure_container.transform,
                            true,
                            true,
                        );
                        measure_container.add_child(measure_result.scene);
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
                            root.add_child(measure_num_node);
                        }

                        // Collect beat positions from actual segment data (mirrors paginated path).
                        // Required for DAW transport sync and cursor highlighting in scroll mode.
                        beat_positions.extend(bar_beat_positions(
                            &measure_result.segments,
                            &BarOnTimeline {
                                page: 1, // continuous mode has no real pages
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

                        // Advance song-time cursor by the measure's full duration,
                        // at its own meter (as the paginated path does).
                        let measure_duration_ticks =
                            measure_ts.0 as i32 * (1920 / measure_ts.1 as i32);
                        cumulative_time += measure_duration_ticks as f64 * seconds_per_tick;
                        cumulative_ticks += measure_duration_ticks as i64;

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
                                page_number: None, // Continuous mode has no pages,
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
                                hide_simile_chords: false,
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
                        for n in self.place_bar_notations(
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
                            &mut system_skyline,
                            &text_metrics,
                            &mut id_counter,
                        ) {
                            root.add_child(n);
                        }
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
                                root.add_child(volta_node);
                            }
                        }

                        // Start-of-measure forward repeat (left edge).
                        if matches!(
                            measure.start_repeat,
                            crate::chart::notations::RepeatMark::Forward
                        ) {
                            root.add_child(self.draw_barline(
                                measure_x,
                                staff_y,
                                staff_height,
                                BarlineType::StartRepeat,
                            ));
                        }

                        measure_x += this_measure_width;

                        root.add_child(self.draw_barline(
                            measure_x,
                            staff_y,
                            staff_height,
                            Self::end_barline_type(measure),
                        ));

                        // How many times the repeat plays, over its closing
                        // barline — the same mark the paginated path draws,
                        // because the same chart scrolled is still the chart.
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
                            if let Some(label) = notation_renderer::autoplace_node(
                                &mut system_skyline,
                                label,
                                true,
                                self.config.spatium * 0.1,
                            ) {
                                root.add_child(label);
                            }
                        }

                        global_measure_index += 1;
                    }
                }

                if system_lyrics {
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
                        root.add_child(node);
                    }
                }

                let spacing = if section_folded {
                    bare_system_gap(&self.config)
                } else {
                    self.config.system_spacing
                };
                total_height += system_height + spacing;
                global_system_index += 1;
            }

            // A folded section drew no measures, so the bar numbers have to be
            // advanced by hand — the bars are played whether or not they are
            // drawn.
            if section_folded {
                global_measure_index += chart_section.measures().len();
            }

            // Update global measure offset for next section (for chart_measurements lookup)
            global_section_measure_offset += chart_section.measures().len();
        }

        total_height += self.config.margins.bottom;

        ChartLayoutResult {
            scene: root,
            pages: Vec::new(),
            total_height,
            total_width: width,
            beat_positions,
        }
    }
}
