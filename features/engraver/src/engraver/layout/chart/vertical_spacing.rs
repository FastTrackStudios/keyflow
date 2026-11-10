//! Vertical spacing between systems: what each line hangs above and below
//! its staff, how far down its ink reaches, and the pass that closes up the
//! dead space between lines once everything is drawn.

use super::{ChartLayoutConfig, ChartLayoutEngine, ChartLayoutResult, notation_renderer};
use crate::chart::Measure;
use crate::chart::notations::Placement;
use crate::engraver::layout::orchestrator::PageLayout;
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::PaintCommand;
use crate::engraver::scene::traverse::SceneNodeExt;
use kurbo::{Affine, Rect};
use std::collections::HashMap;
use tracing::debug;

use super::{MeasureMelodyData, melody_note_extent};
use crate::Chart;
use crate::chart::types::ChartSection;
use crate::engraver::layout::context::LayoutContext;

/// How tall a system's row is.
pub(super) struct SystemRow {
    /// The section label's capsule height (the staff height when unfolded).
    pub(super) label_height: f64,
    pub(super) row_height: f64,
}

impl ChartLayoutEngine {
    /// How far the system's melody notes reach above and below the staff.
    ///
    /// Compute extra space for melody notes extending beyond the staff
    pub(super) fn system_melody_extent(
        &self,
        chart: &Chart,
        chart_section: &ChartSection,
        melody_data_map: &HashMap<usize, MeasureMelodyData>,
        measure_indices: &[usize],
    ) -> (f64, f64) {
        let mut melody_extra_above = 0.0f64;
        let mut melody_extra_below = 0.0f64;
        for &m_idx in measure_indices {
            if let Some(md) = melody_data_map.get(&m_idx) {
                let (above, below) = melody_note_extent(
                    md,
                    self.config.spatium,
                    self.section_proto_clef(chart, chart_section),
                );
                melody_extra_above = melody_extra_above.max(above);
                melody_extra_below = melody_extra_below.max(below);
            }
        }
        (melody_extra_above, melody_extra_below)
    }

    /// The row a system draws on: a staff, or for a folded section the rule
    /// through its label.
    ///
    /// A folded row is as tall as its margin capsule, so the rule
    /// runs through the badge's middle and the two read as one
    /// mark. A capsule that runs to two or three lines overhangs
    /// the row instead of stretching it — exactly as it overhangs a
    /// staff today — or a section whose badge needs two lines would
    /// cost more folded than written.
    pub(super) fn system_row(
        &self,
        chart_section: &ChartSection,
        section_folded: bool,
        letter: Option<char>,
        ctx: &LayoutContext,
    ) -> SystemRow {
        let staff_height = self.config.spatium * 4.0;
        let label_height = section_folded
            .then(|| {
                self.section_label_height(
                    &chart_section.section,
                    // Neither the page's x nor the staff's y changes
                    // how tall the capsule comes out.
                    0.0,
                    self.config.margins.left,
                    0.0,
                    staff_height,
                    letter,
                    ctx,
                )
            })
            .unwrap_or(staff_height);
        let row_height = if section_folded {
            label_height
                .min(staff_height)
                .max(label_height / 2.0 + self.config.spatium * 0.5)
        } else {
            staff_height
        };
        SystemRow {
            label_height,
            row_height,
        }
    }

    /// The gap a paginated system leaves under itself.
    ///
    /// A rule needs less air around it than a staff does, and two
    /// lines carrying no text need less than the full band. This
    /// has to match what `compact_paginated_vertical_dead_space`
    /// will hold back afterwards: page breaks are decided here, and
    /// a break taken against spacing the compactor then removes
    /// leaves a page that did not need to start.
    /// A folded row placed against a staff keeps the ordinary bare
    /// gap; only two folded rows in a row close up, and the
    /// compactor does that once it can see what follows.
    pub(super) fn gap_after_system(&self, line: &SystemOnPage<'_>) -> f64 {
        let SystemOnPage {
            chart,
            section_idx,
            systems,
            sys_idx,
            section_folded,
            system_measures,
            system_lyrics,
        } = *line;
        let chart_section = &chart.sections[section_idx];
        // What comes next decides how much room this line leaves
        // behind it. A folded row is a rule and a word: it takes the
        // air under the line above rather than asking for its own.
        let next_section_folded = chart
            .sections
            .get(section_idx + 1)
            .is_some_and(|next| next.folded && self.config.fold_sections);
        let next_is_folded = if sys_idx + 1 < systems.len() {
            section_folded
        } else {
            next_section_folded
        };
        // Under `tight_system_spacing` the wide band belongs to
        // whichever side puts something in it: what this line hangs
        // below itself, or what the next one carries above its own
        // staff. Reserving it here is the cautious half of the job —
        // the compactor takes back what neither side turned out to
        // need, and it cannot give room back that was never left.
        let next_system_draws_above = || {
            systems
                .get(sys_idx + 1)
                .map(|next| {
                    let next_measures: Vec<Measure> = next
                        .iter()
                        .filter_map(|idx| chart_section.measures().get(*idx).cloned())
                        .collect();
                    draws_above_staff(&next_measures)
                })
                .or_else(|| {
                    chart
                        .sections
                        .get(section_idx + 1)
                        .map(|next| draws_above_staff(next.measures()))
                })
                .unwrap_or(false)
        };
        if section_folded || next_is_folded {
            bare_system_gap(&self.config)
        } else if self.config.tight_system_spacing
            && !draws_below_staff(system_measures)
            && !system_lyrics
            && !next_system_draws_above()
        {
            bare_system_gap(&self.config)
        } else {
            self.config.system_spacing
        }
    }

    /// A system's height as drawn: at least its reserved height, and as far
    /// down as its ink reaches.
    ///
    /// A folded row is measured, not discovered: its ink is the
    /// rule and the label capsule, both of which are centred on the
    /// row and overhang it by design. Letting them grow the row
    /// would give a section that draws nothing back the vertical
    /// space folding it was meant to save.
    pub(super) fn drawn_system_height(&self, ink: &SystemInk<'_>) -> f64 {
        let base_system_height = ink.base_height;
        let ink_system_height = ink.ink_bottom - ink.page_top;
        let actual_system_height = if ink.section_folded {
            base_system_height
        } else {
            ink_system_height.max(base_system_height)
        };
        let extra_system_height = (actual_system_height - base_system_height).max(0.0);
        if extra_system_height > self.config.spatium {
            let source_measures = ink
                .measure_indices
                .iter()
                .filter_map(|idx| ink.chart_section.measures().get(*idx))
                .filter_map(|measure| measure.source_measure_number)
                .collect::<Vec<_>>();
            debug!(
                target: "keyflow::layout::system_spacing",
                page = ink.page_number,
                system = ink.global_system_index,
                section = ?ink.chart_section.section.section_type,
                measures = ?source_measures,
                base_height = base_system_height,
                ink_height = ink_system_height,
                actual_height = actual_system_height,
                extra_height = extra_system_height,
                contributors = ?ink.contributors,
                "system height expanded by rendered ink"
            );
        }
        actual_system_height
    }
}

/// A paginated system and its neighbours: what decides the gap under it.
#[derive(Clone, Copy)]
pub(super) struct SystemOnPage<'a> {
    pub(super) chart: &'a Chart,
    pub(super) section_idx: usize,
    /// The section's systems, and this one's index among them.
    pub(super) systems: &'a [Vec<usize>],
    pub(super) sys_idx: usize,
    pub(super) section_folded: bool,
    pub(super) system_measures: &'a [Measure],
    pub(super) system_lyrics: bool,
}

/// What a paginated system's drawing reached, for measuring its height.
pub(super) struct SystemInk<'a> {
    pub(super) chart_section: &'a ChartSection,
    pub(super) measure_indices: &'a [usize],
    pub(super) section_folded: bool,
    pub(super) page_number: u32,
    pub(super) global_system_index: usize,
    /// The reserved height: the band above the staff plus the row.
    pub(super) base_height: f64,
    /// The system's top on the page, and its lowest ink.
    pub(super) page_top: f64,
    pub(super) ink_bottom: f64,
    /// What set `ink_bottom`, for the trace.
    pub(super) contributors: &'a [String],
}

/// Does anything on these bars draw below the staff?
///
/// The south band under a system exists to clear dynamics, cues, figures and
/// text placed below the staff. A system of plain chords and slashes has none
/// of that, and holding the band open anyway breaks the page earlier than the
/// music needs.
pub(super) fn draws_below_staff(measures: &[Measure]) -> bool {
    let below = |placement: &Placement| matches!(placement, Placement::Below);
    measures.iter().any(|measure| {
        !measure.text_cues.is_empty()
            || !measure.hairpins.is_empty()
            || measure
                .classical_dynamics
                .iter()
                .any(|dynamic| below(&dynamic.placement))
            || measure.staff_text.iter().any(|text| below(&text.placement))
            || measure
                .figured_bass
                .iter()
                .any(|figure| below(&figure.placement))
            || measure
                .suspensions
                .iter()
                .any(|figure| below(&figure.placement))
    })
}

/// Does anything on these bars draw above the staff, beyond the chord symbols
/// every bar has?
///
/// `system_spacing` is deliberately generous — eight spatia — so that text,
/// figures and endings between two lines never crowd either of them. The band
/// belongs to whichever side actually puts something in it: a line whose text
/// sits above it needs the room above it, not below.
pub(super) fn draws_above_staff(measures: &[Measure]) -> bool {
    let above = |placement: &Placement| matches!(placement, Placement::Above);
    measures.iter().any(|measure| {
        measure.volta_start.is_some()
            || !measure.melodies.is_empty()
            || measure
                .classical_dynamics
                .iter()
                .any(|dynamic| above(&dynamic.placement))
            || measure.staff_text.iter().any(|text| above(&text.placement))
            || measure
                .figured_bass
                .iter()
                .any(|figure| above(&figure.placement))
            || measure
                .suspensions
                .iter()
                .any(|figure| above(&figure.placement))
    })
}

/// The gap between two systems that carry nothing between them.
///
/// Enough to keep one line's ink off the next, and no more. Used both when
/// systems are placed and when the vertical compactor closes up after them, so
/// the page breaks taken during layout survive the compaction.
pub(super) fn bare_system_gap(config: &ChartLayoutConfig) -> f64 {
    (config.spatium * 3.0).max(8.0)
}

/// The gap above and below a folded row.
///
/// A rule and a word: it needs to read as its own line and nothing more.
pub(super) fn folded_system_gap(config: &ChartLayoutConfig) -> f64 {
    config.spatium * 1.5
}

fn collect_system_ink_bounds(scene: &SceneNode, page: &PageLayout) -> Vec<Option<Rect>> {
    let mut bounds_by_system = vec![None; page.systems.len()];
    let content_x0 = page.x_offset + page.margins.left;
    let content_x1 = page.x_offset + page.width - page.margins.right;
    let margin_label_cutoff_x = content_x0 + 80.0;

    for (node, transform) in scene.iter_with_transforms() {
        if node.commands.is_empty() {
            continue;
        }
        if node
            .metadata
            .get("section_start_dynamic")
            .is_some_and(|value| value == "true")
        {
            continue;
        }
        let Some(bounds) = notation_renderer::scene_ink_bounds(node, transform) else {
            continue;
        };
        if is_page_background(bounds, page) {
            continue;
        }
        if node.id.is_none() && bounds.x1 <= margin_label_cutoff_x {
            continue;
        }
        if bounds.x1 <= content_x0 || bounds.x0 >= content_x1 {
            continue;
        }

        let center_x = (bounds.x0 + bounds.x1) * 0.5;
        let center_y = (bounds.y0 + bounds.y1) * 0.5;
        if center_x < page.x_offset
            || center_x > page.x_offset + page.width
            || center_y < page.y_offset
            || center_y > page.y_offset + page.height
        {
            continue;
        }

        let y_on_page = center_y - page.y_offset;
        let Some((system_idx, _)) = page.systems.iter().enumerate().min_by(|(_, a), (_, b)| {
            let a_center = a.y + a.height * 0.5;
            let b_center = b.y + b.height * 0.5;
            (y_on_page - a_center)
                .abs()
                .total_cmp(&(y_on_page - b_center).abs())
        }) else {
            continue;
        };

        let slot = &mut bounds_by_system[system_idx];
        *slot = Some(slot.map_or(bounds, |current: kurbo::Rect| current.union(bounds)));
    }

    bounds_by_system
}

fn shift_scene_systems(
    scene: &mut SceneNode,
    pages: &[PageLayout],
    page_shift_plans: &[(u32, Vec<(usize, f64)>)],
) {
    for child in &mut scene.children {
        let Some(bounds) = child_world_bounds(child) else {
            continue;
        };
        let center_y = (bounds.y0 + bounds.y1) * 0.5;
        let center_x = (bounds.x0 + bounds.x1) * 0.5;

        let Some(page) = child
            .get_page()
            .and_then(|page_number| pages.iter().find(|page| page.number == page_number))
            .or_else(|| {
                pages.iter().find(|page| {
                    center_x >= page.x_offset
                        && center_x <= page.x_offset + page.width
                        && center_y >= page.y_offset
                        && center_y <= page.y_offset + page.height
                })
            })
        else {
            continue;
        };

        if is_page_background(bounds, page) {
            continue;
        }

        let Some((_, shifts)) = page_shift_plans
            .iter()
            .find(|(page_number, _)| *page_number == page.number)
        else {
            continue;
        };

        let y_on_page = center_y - page.y_offset;
        let Some(system) = child
            .get_system()
            .and_then(|system_index| {
                page.systems
                    .iter()
                    .find(|system| system.index == system_index as usize)
            })
            .or_else(|| {
                page.systems.iter().min_by(|a, b| {
                    let a_center = a.y + a.height * 0.5;
                    let b_center = b.y + b.height * 0.5;
                    (y_on_page - a_center)
                        .abs()
                        .total_cmp(&(y_on_page - b_center).abs())
                })
            })
        else {
            continue;
        };

        if let Some((_, shift)) = shifts
            .iter()
            .rev()
            .find(|(system_index, _)| system.index >= *system_index)
        {
            child.transform = Affine::translate((0.0, -*shift)) * child.transform;
        }
    }
}

fn child_world_bounds(node: &SceneNode) -> Option<Rect> {
    let bounds = node.compute_bounds();
    if bounds.is_zero_area() {
        None
    } else {
        Some(node.transform.transform_rect_bbox(bounds))
    }
}

fn is_page_background(bounds: Rect, page: &PageLayout) -> bool {
    bounds.width() > page.width * 0.8 && bounds.height() > page.height * 0.8
}

fn node_spacing_reason(node: &SceneNode, fallback: &str) -> String {
    let element = node
        .metadata
        .get("element_type")
        .map(String::as_str)
        .or_else(|| node.id.as_ref().map(|id| id.element_type.svg_type_name()))
        .unwrap_or(fallback);
    let content = node.commands.iter().find_map(|command| {
        if let PaintCommand::Text { text, .. } = command {
            Some(text.as_str())
        } else {
            None
        }
    });
    if let Some(content) = content {
        format!("{fallback}:{element}:{content}")
    } else {
        format!("{fallback}:{element}")
    }
}

fn should_count_for_system_height(node: &SceneNode) -> bool {
    if node
        .metadata
        .get("section_start_dynamic")
        .is_some_and(|value| value == "true")
    {
        let stack_count = node
            .metadata
            .get("section_start_dynamic_stack_count")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1);
        return stack_count > 1;
    }
    true
}

pub(super) fn record_system_ink_bottom(
    system_ink_bottom: &mut f64,
    contributors: &mut Vec<String>,
    node: &SceneNode,
    reason: &str,
) {
    if !should_count_for_system_height(node) {
        return;
    }
    if let Some(bounds) = notation_renderer::scene_ink_bounds(node, Affine::IDENTITY)
        && bounds.y1 > *system_ink_bottom
    {
        *system_ink_bottom = bounds.y1;
        contributors.push(format!(
            "{} y1={:.1} bounds=({:.1},{:.1},{:.1},{:.1})",
            node_spacing_reason(node, reason),
            bounds.y1,
            bounds.x0,
            bounds.y0,
            bounds.x1,
            bounds.y1
        ));
    }
}

impl ChartLayoutEngine {
    pub(super) fn compact_paginated_vertical_dead_space(
        &self,
        result: &mut ChartLayoutResult,
        systems_with_ink_below: &std::collections::HashSet<usize>,
        systems_with_ink_above: &std::collections::HashSet<usize>,
        folded_systems: &std::collections::HashSet<usize>,
    ) {
        // Compaction should preserve ink clearance, not a second system-spacing
        // reserve. Larger north/south reserves are useful during initial system
        // construction, but after all notation is rendered we can safely pack
        // systems by visible ink bounds. We still hold a baseline gap equal to
        // the configured `system_spacing` so compacted lines never sit tighter
        // than the pre-compactor layout — only genuine dead space beyond that
        // baseline is removed.
        let annotated_gap = self
            .config
            .system_spacing
            .max(self.config.spatium * 1.5)
            .max(8.0);
        let bare_gap = bare_system_gap(&self.config).min(annotated_gap);
        let folded_gap = folded_system_gap(&self.config).min(annotated_gap);

        for _ in 0..1 {
            let mut page_shift_plans: Vec<(u32, Vec<(usize, f64)>)> = Vec::new();

            for page in &result.pages {
                let mut system_bounds = collect_system_ink_bounds(&result.scene, page);
                // A folded row's ink is a rule and a word centred on it, and
                // the word's bounding box swings with its letters. Measuring
                // that leaves four folded rows sitting at four different
                // distances. The row's own box is the deliberate one, so it is
                // what the row is spaced by.
                for (slot, system) in system_bounds.iter_mut().zip(&page.systems) {
                    if folded_systems.contains(&system.index) {
                        let top = page.y_offset + system.y;
                        *slot = Some(Rect::new(0.0, top, 0.0, top + system.height));
                    }
                }
                let mut cumulative_shift = 0.0;
                let mut shifts = Vec::new();

                for system_idx in 1..page.systems.len() {
                    let lower_system_index = page.systems[system_idx].index;
                    let upper_system_index = page.systems[system_idx - 1].index;
                    let touches_a_folded_row = folded_systems.contains(&upper_system_index)
                        || folded_systems.contains(&lower_system_index);
                    let target_gap = if folded_systems.contains(&upper_system_index)
                        && folded_systems.contains(&lower_system_index)
                    {
                        folded_gap
                    } else if touches_a_folded_row {
                        bare_gap
                    } else if !self.config.tight_system_spacing {
                        annotated_gap
                    } else if systems_with_ink_below.contains(&upper_system_index)
                        || systems_with_ink_above.contains(&lower_system_index)
                    {
                        annotated_gap
                    } else {
                        bare_gap
                    };
                    let removable = system_bounds
                        .get(system_idx - 1)
                        .and_then(|upper| upper.as_ref())
                        .zip(
                            system_bounds
                                .get(system_idx)
                                .and_then(|lower| lower.as_ref()),
                        )
                        .map(|(upper, lower)| (lower.y0 - upper.y1 - target_gap).max(0.0))
                        .unwrap_or(0.0);

                    if removable > 0.5 {
                        cumulative_shift += removable;
                    }
                    shifts.push((lower_system_index, cumulative_shift));
                }

                if cumulative_shift > 0.0 {
                    page_shift_plans.push((page.number, shifts));
                }
            }

            if page_shift_plans.is_empty() {
                return;
            }

            let original_pages = result.pages.clone();

            for page in &mut result.pages {
                let Some((_, shifts)) = page_shift_plans
                    .iter()
                    .find(|(page_number, _)| *page_number == page.number)
                else {
                    continue;
                };

                for system in &mut page.systems {
                    if let Some((_, shift)) = shifts
                        .iter()
                        .rev()
                        .find(|(system_index, _)| system.index >= *system_index)
                    {
                        system.y -= shift;
                    }
                }
            }

            for beat in &mut result.beat_positions {
                let Some((_, shifts)) = page_shift_plans
                    .iter()
                    .find(|(page_number, _)| *page_number == beat.page)
                else {
                    continue;
                };
                if let Some((_, shift)) = shifts
                    .iter()
                    .rev()
                    .find(|(system_index, _)| beat.system >= *system_index)
                {
                    beat.staff_y -= shift;
                    beat.glyph_y -= shift;
                }
            }

            shift_scene_systems(&mut result.scene, &original_pages, &page_shift_plans);
        }
    }
}
