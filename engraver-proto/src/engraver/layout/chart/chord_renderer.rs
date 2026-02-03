//! Chord symbol rendering for chart layout.
//!
//! This module extracts the duplicated chord rendering logic from
//! `layout_paginated` and `layout_continuous` into reusable functions.
//!
//! # Multi-Pass Layout Architecture
//!
//! With the multi-pass layout system (see [`super::measure_pass`]), chord symbol
//! widths are pre-measured during Pass 1 (Measure). This provides accurate minimum
//! measure widths to the layout pass, which should prevent most collisions.
//!
//! # Collision Detection (Safety Net)
//!
//! Despite accurate pre-measurement, collision detection is retained as a safety net.
//! When adjacent chord symbols would overlap or be closer than the minimum gap:
//! - The first chord shifts slightly left
//! - The second chord stays at its notehead-aligned position
//!
//! With proper pre-measurement, collisions should be rare. The debug logging
//! (via `tracing::debug!`) reports when collisions occur, which
//! can indicate issues with the measure pass or layout distribution.
//!
//! To disable collision detection, set `min_chord_symbol_gap = 0.0` in config.

use kurbo::{Affine, Rect, Vec2};
use vello::peniko::Color;

use crate::chart::commands::Command;
use crate::chart::types::{ChordInstance, Measure, RhythmElement};
use crate::chord::ChordRhythm;
use crate::engraver::layout::context::LayoutContext;
use crate::engraver::layout::tlayout::{layout_harmony, parse_chord, HarmonyParams, HarmonyStyle};
use crate::engraver::scene::id::{ElementType, SemanticId};
use crate::engraver::scene::node::{metadata_keys, SceneNode};
use crate::engraver::scene::paint::{PaintCommand, TextAnchor};
use crate::time::{MusicalPositionExt, TimeSignature};
use crate::{ChartPosition, SourceLink};

use super::measure_pass::MeasureMeasurements;
use super::PushSpillback;

/// Create push marker color (red for visibility)
fn push_marker_color() -> Color {
    Color::from_rgba8(0xCC, 0x00, 0x00, 0xFF)
}

/// Create pull marker color (blue to distinguish from push)
fn pull_marker_color() -> Color {
    Color::from_rgba8(0x00, 0x00, 0xCC, 0xFF)
}

/// Create accent marker color (red for visibility)
fn accent_marker_color() -> Color {
    Color::from_rgba8(0xCC, 0x00, 0x00, 0xFF)
}

/// Context for rendering chord symbols in a measure.
#[derive(Debug, Clone)]
pub struct ChordRenderContext<'a> {
    /// Measure x position (start of measure content).
    pub measure_x: f64,
    /// Measure width (for collision boundary clamping).
    pub measure_width: f64,
    /// Chord y position (above staff).
    pub chord_y: f64,
    /// Page number (1-indexed, for paginated mode).
    pub page_number: Option<u32>,
    /// Global system index (0-indexed).
    pub global_system_index: usize,
    /// Section measure index (within this section).
    pub measure_idx: usize,
    /// Local measure index (within the current system).
    pub local_measure_idx: usize,
    /// Section type name for metadata.
    pub section_name: &'a str,
    /// Segment positions from measure layout.
    pub segment_positions: &'a [f64],
    /// Internal push positions from rhythm builder.
    pub internal_push_positions: &'a [(usize, usize)],
    /// Harmony style for chord symbols.
    pub harmony_style: &'a HarmonyStyle,
    /// Time signature (numerator, denominator).
    pub time_signature: (u8, u8),
    /// Whether to hide repeated consecutive chords.
    pub hide_repeated_chords: bool,
    /// Minimum horizontal gap between adjacent chord symbols (in points).
    /// Set to 0.0 to disable collision detection.
    pub min_chord_symbol_gap: f64,
    /// Whether push/pull notation alters the rhythm display.
    /// When false, shows apostrophe markers on chord symbols instead.
    pub push_alters_rhythm: bool,
    /// Spatium (staff space) for sizing apostrophe markers.
    pub spatium: f64,
    /// Pre-computed measurements for this measure (from measure pass).
    /// When provided, collision detection uses cached chord layouts instead
    /// of re-measuring during render.
    pub measure_measurements: Option<&'a MeasureMeasurements>,
    /// Spillback positions computed by rhythm builder.
    /// Maps (rhythm_index, chord_symbol) for chords from next measure pushing back.
    /// Used to place spillback chords at correct triplet positions.
    pub spillback_positions: &'a [(usize, String)],
}

/// Result of chord symbol rendering.
#[derive(Debug)]
pub struct ChordRenderResult {
    /// Rendered chord nodes.
    pub nodes: Vec<SceneNode>,
    /// Updated previous chord symbol (for duplicate detection).
    pub last_chord_symbol: Option<String>,
    /// Next ID counter value.
    pub next_id: u64,
}

// ============================================================================
// Collision Detection
// ============================================================================

/// Information about a rendered chord for collision detection.
#[derive(Debug)]
struct ChordBoundsInfo {
    /// Index in the nodes vector.
    node_idx: usize,
    /// Original X position (in world coordinates).
    original_x: f64,
    /// Bounding box in world coordinates.
    world_bounds: Rect,
}

/// Result of chord collision resolution.
#[derive(Debug)]
pub struct ChordCollisionResult {
    /// X-position adjustments for each chord (indexed same as input).
    /// Positive = shift right, negative = shift left.
    pub adjustments: Vec<f64>,
    /// Whether any collisions were detected and resolved.
    pub had_collisions: bool,
}

/// Detect and resolve overlapping chord symbols.
///
/// This function takes rendered chord bounds and computes position adjustments
/// needed to achieve the minimum gap between adjacent chords.
///
/// # Algorithm
///
/// For each adjacent pair of chords:
/// 1. Compute the gap (left edge of B - right edge of A)
/// 2. If gap < min_gap, compute the overlap amount
/// 3. **Only shift the first chord left** - the second chord's notehead position
///    is already correct because `compute_chord_min_widths()` set segment minimums
///
/// The segment minimum widths ensure noteheads are positioned correctly.
/// This function is a safety net that only moves the FIRST chord symbol left
/// when there's still a collision (e.g., spring system didn't allocate full space).
///
/// # Arguments
///
/// * `chord_bounds` - List of chord bounds info, sorted by X position
/// * `min_gap` - Minimum required horizontal gap between chords (in points)
/// * `_measure_start_x` - Left boundary (unused - we allow moving past clef)
/// * `_measure_end_x` - Right boundary (unused - we only shift first chord left)
///
/// # Returns
///
/// `ChordCollisionResult` with adjustments for each chord.
fn resolve_chord_collisions(
    chord_bounds: &[ChordBoundsInfo],
    min_gap: f64,
    _measure_start_x: f64,
    _measure_end_x: f64,
) -> ChordCollisionResult {
    if chord_bounds.len() < 2 {
        return ChordCollisionResult {
            adjustments: vec![0.0; chord_bounds.len()],
            had_collisions: false,
        };
    }

    let mut adjustments = vec![0.0; chord_bounds.len()];
    let mut had_collisions = false;

    // Process adjacent pairs left-to-right
    for i in 0..chord_bounds.len() - 1 {
        let bounds_a = &chord_bounds[i];
        let bounds_b = &chord_bounds[i + 1];

        // Apply accumulated adjustments to get current positions
        let adjusted_right_a = bounds_a.world_bounds.x1 + adjustments[i];
        let adjusted_left_b = bounds_b.world_bounds.x0 + adjustments[i + 1];

        // Compute current gap
        let gap = adjusted_left_b - adjusted_right_a;

        if gap < min_gap {
            // Collision detected - compute overlap
            let overlap = min_gap - gap;
            had_collisions = true;

            // Only shift the first chord left - the second chord's notehead
            // is already correctly positioned via segment minimum widths.
            // Moving only the first chord left preserves notehead-chord alignment.
            adjustments[i] -= overlap;

            tracing::debug!(
                "[chord-collision] Detected overlap of {:.1}pt between chords {} and {}. \
                 Moving first chord left by {:.1}pt (preserves notehead alignment)",
                overlap,
                i,
                i + 1,
                overlap
            );
        }
    }

    ChordCollisionResult {
        adjustments,
        had_collisions,
    }
}

/// Apply position adjustments to chord nodes by modifying their transforms.
fn apply_collision_adjustments(nodes: &mut [SceneNode], adjustments: &[f64]) {
    for (node, &adjustment) in nodes.iter_mut().zip(adjustments.iter()) {
        if adjustment.abs() > 0.001 {
            // Apply horizontal translation to the existing transform
            node.transform = node.transform.then_translate(Vec2::new(adjustment, 0.0));
        }
    }
}

/// Create an apostrophe marker node for pushed/pulled chords.
///
/// When `push_alters_rhythm=false`, this creates a visual indicator showing
/// the chord is pushed (') or pulled (') without altering the rhythm notation.
///
/// # Arguments
/// * `is_push` - true for push (before chord), false for pull (after chord)
/// * `chord_bounds` - The bounding box of the chord symbol for positioning
/// * `chord_y` - Y position (same as chord symbol baseline)
/// * `spatium` - Staff space for sizing
/// * `id` - Unique ID for the node
fn create_push_marker(
    is_push: bool,
    chord_bounds: Rect,
    chord_y: f64,
    spatium: f64,
    id: u64,
) -> SceneNode {
    let marker_text = "'";
    let color = if is_push {
        push_marker_color()
    } else {
        pull_marker_color()
    };

    // Size the apostrophe relative to spatium (similar to chord symbol text)
    let font_size = spatium * 2.5;

    // Position: push markers go before (left of) the chord, pull markers after
    // The chord_y is the baseline of the chord symbol
    let marker_x = if is_push {
        chord_bounds.x0 - spatium * 0.6 // Slightly left of chord
    } else {
        chord_bounds.x1 + spatium * 0.1 // Slightly right of chord
    };

    let paint = PaintCommand::text(
        marker_text,
        "MuseJazz Text",
        font_size,
        kurbo::Point::new(marker_x, chord_y),
        color,
    );

    let mut node = SceneNode::leaf(SemanticId::new(ElementType::Articulation, id), vec![paint]);
    node.set_element_type("push_marker");
    node
}

/// Create an accent marker node for accented chords.
///
/// Renders the SMuFL accent articulation glyph (>) above the rhythm slash notehead.
/// Following MuseScore's articulation positioning:
/// - Accent placed just above the top staff line
/// - Minimum distance of 0.4 spatiums from the notehead
/// - Horizontally centered on the rhythm slash (segment position)
///
/// # Arguments
/// * `segment_x` - X position of the rhythm segment (slash position) for horizontal centering
/// * `chord_y` - Y position of chord symbol baseline (used to derive staff position)
/// * `spatium` - Staff space for sizing and positioning
/// * `id` - Unique ID for the node
fn create_accent_marker(segment_x: f64, chord_y: f64, spatium: f64, id: u64) -> SceneNode {
    use super::constants::{ARTICULATION_MAG, ARTICULATION_MIN_DISTANCE_SPATIUMS, CHORD_Y_OFFSET};

    // SMuFL articAccentAbove: U+E4A0
    let accent_glyph = '\u{E4A0}';
    let color = accent_marker_color();

    // Size relative to spatium (with articulation magnification factor)
    // Use 1.2x spatium for a more compact accent
    let font_size = spatium * 1.2 * ARTICULATION_MAG;

    // Calculate staff Y position from chord_y
    // chord_y = staff_y + CHORD_Y_OFFSET (where CHORD_Y_OFFSET is -8.0)
    // So staff_y = chord_y - CHORD_Y_OFFSET = chord_y + 8.0
    let staff_y = chord_y - CHORD_Y_OFFSET;

    // Position accent just above the top staff line
    // Staff top line is at staff_y, we want the accent above that
    // Use minimal distance for a tighter appearance
    let accent_y = staff_y - ARTICULATION_MIN_DISTANCE_SPATIUMS * spatium;

    // Horizontally center on the rhythm slash (segment position)
    // The segment_x is the left edge of the segment, add half a spatium to center on the slash
    let accent_x = segment_x + spatium * 0.5;

    let paint = PaintCommand::glyph(
        accent_glyph,
        kurbo::Point::new(accent_x, accent_y),
        font_size,
        color,
    );

    let mut node = SceneNode::leaf(SemanticId::new(ElementType::Articulation, id), vec![paint]);
    node.set_element_type("accent");
    node
}

/// Determine if a chord should be skipped (is a space/rest placeholder).
#[must_use]
pub fn is_placeholder_chord(symbol: &str) -> bool {
    symbol.is_empty() || symbol == "s" || symbol == "r"
}

/// Check if this is the first real (non-placeholder) chord in the measure.
#[must_use]
pub fn is_first_real_chord(chords: &[ChordInstance], chord_idx: usize) -> bool {
    chords
        .iter()
        .take(chord_idx)
        .all(|c| is_placeholder_chord(&c.full_symbol))
}

/// Check if a chord should be rendered at a section/system boundary.
///
/// Pushed chords at boundaries should show even if they would normally spill back.
#[must_use]
pub fn is_at_boundary(measure_idx: usize, local_measure_idx: usize) -> bool {
    let is_first_measure_of_section = measure_idx == 0;
    let is_first_measure_of_system = local_measure_idx == 0;
    is_first_measure_of_section || is_first_measure_of_system
}

/// Calculate the segment index for a chord symbol.
///
/// This complex logic handles multiple cases:
/// - Pushed chords at boundaries (force to segment 0)
/// - Internal pushed chords (use precomputed positions)
/// - Explicit rhythm notation
/// - Slash notation
/// - Simple measures
#[must_use]
pub fn calculate_segment_index(
    measure: &Measure,
    chord_idx: usize,
    chord: &ChordInstance,
    segment_positions: &[f64],
    internal_push_positions: &[(usize, usize)],
    is_first_real: bool,
    is_boundary: bool,
) -> usize {
    // Check if this is a pushed chord at a boundary
    let is_pushed_at_boundary = chord
        .push_pull
        .as_ref()
        .map_or(false, |(is_push, _)| *is_push)
        && is_first_real
        && is_boundary;

    if is_pushed_at_boundary {
        // Force pushed chord to segment 0 (beat 1) at section/line start
        return 0;
    }

    // Check if this is an internal pushed chord (pushed but not spillback)
    let is_internal_push = chord
        .push_pull
        .as_ref()
        .map_or(false, |(is_push, _)| *is_push)
        && !is_first_real
        && !internal_push_positions.is_empty();

    if is_internal_push {
        // Internal pushed chord - look up precomputed segment
        if let Some((_, seg_idx)) = internal_push_positions
            .iter()
            .find(|(c_idx, _)| *c_idx == chord_idx)
        {
            return *seg_idx;
        }
        // Fallback
        return chord_idx.min(segment_positions.len().saturating_sub(1));
    }

    // Check for explicit rhythm elements
    if !measure.rhythm_elements.is_empty() {
        let has_explicit_rhythm = measure_has_explicit_chord_rhythm(measure);

        if has_explicit_rhythm {
            // Explicit rhythm: find chord's index in rhythm_elements
            // Each RhythmElement::Chord maps to a segment in order
            let mut seen_chord_count = 0;
            let mut found_idx = None;
            for (idx, elem) in measure.rhythm_elements.iter().enumerate() {
                if let RhythmElement::Chord(_) = elem {
                    if seen_chord_count == chord_idx {
                        found_idx = Some(idx);
                        break;
                    }
                    seen_chord_count += 1;
                }
            }

            // Debug: log segment count mismatch
            if segment_positions.len() < measure.rhythm_elements.len() {
                tracing::debug!(
                    "[chord-position] WARNING: segment_positions.len()={} < rhythm_elements.len()={} for chord_idx={}",
                    segment_positions.len(),
                    measure.rhythm_elements.len(),
                    chord_idx
                );
            }

            return found_idx
                .unwrap_or(chord_idx)
                .min(segment_positions.len().saturating_sub(1));
        }

        // Slash notation: calculate segment from cumulative beat durations
        let mut cumulative_beats = 0usize;
        let mut found_beat_pos = None;
        let mut seen_chord_count = 0;

        for elem in measure.rhythm_elements.iter() {
            if let RhythmElement::Chord(c) = elem {
                if seen_chord_count == chord_idx {
                    found_beat_pos = Some(cumulative_beats);
                    break;
                }
                let chord_beats = match &c.rhythm {
                    ChordRhythm::Slashes { count, .. } => *count as usize,
                    ChordRhythm::Default => 1,
                    _ => 1,
                };
                cumulative_beats += chord_beats;
                seen_chord_count += 1;
            }
        }
        return found_beat_pos
            .unwrap_or(chord_idx)
            .min(segment_positions.len().saturating_sub(1));
    }

    // Simple measure - calculate segment from cumulative chord beats
    let mut cumulative_beats = 0usize;
    for (idx, c) in measure.chords.iter().enumerate() {
        if idx == chord_idx {
            break;
        }
        let chord_beats = match &c.rhythm {
            ChordRhythm::Slashes { count, .. } => *count as usize,
            ChordRhythm::Default => 1,
            _ => 1,
        };
        cumulative_beats += chord_beats;
    }
    cumulative_beats.min(segment_positions.len().saturating_sub(1))
}

/// Check if a measure has explicit chord rhythms (Lily or Rest notation).
fn measure_has_explicit_chord_rhythm(measure: &Measure) -> bool {
    super::rhythm_builder::measure_has_explicit_chord_rhythm(measure)
}

/// Check if a chord should be hidden due to being a duplicate.
#[must_use]
pub fn should_hide_chord(
    chord: &ChordInstance,
    current_symbol: &str,
    previous_symbol: Option<&str>,
    is_pushed_at_boundary: bool,
    time_signature: (u8, u8),
    hide_repeated_chords: bool,
) -> bool {
    if !hide_repeated_chords {
        return false;
    }

    // Short duration chords should always be shown (hits/stabs)
    let ts = TimeSignature::new(time_signature.0.into(), time_signature.1.into());
    let chord_beats = chord.duration.to_beats(ts);
    let is_short_duration = chord_beats <= 0.5;

    if is_short_duration {
        return false;
    }

    // Pushed chords at boundaries should show
    if is_pushed_at_boundary {
        return false;
    }

    // Check for duplicate
    previous_symbol.map_or(false, |prev| prev == current_symbol)
}

/// Render chord symbols for a measure with automatic collision detection.
///
/// This handles all the complex logic for determining which chords to render,
/// where to position them, and what metadata to attach. After rendering,
/// it detects collisions between adjacent chord symbols and adjusts their
/// positions to maintain the minimum gap.
///
/// Note: `internal_push_positions` should already be included in `ctx`.
pub fn render_chord_symbols(
    ctx: &ChordRenderContext<'_>,
    measure: &Measure,
    previous_chord_symbol: Option<&str>,
    mut id_counter: u64,
    layout_ctx: &LayoutContext<'_>,
) -> ChordRenderResult {
    let mut nodes = Vec::new();
    let mut chord_bounds_info: Vec<ChordBoundsInfo> = Vec::new();
    let mut last_chord_symbol = previous_chord_symbol.map(String::from);

    let is_boundary = is_at_boundary(ctx.measure_idx, ctx.local_measure_idx);

    if ctx.measure_idx == 0 {
        tracing::debug!(
            "[chord-render-start] section={} measure={} is_boundary={} chord_count={} chords={:?}",
            ctx.section_name,
            ctx.measure_idx,
            is_boundary,
            measure.chords.len(),
            measure
                .chords
                .iter()
                .map(|c| (&c.full_symbol, c.push_pull.as_ref().map(|(p, _)| *p)))
                .collect::<Vec<_>>()
        );
    }

    for (chord_idx, chord) in measure.chords.iter().enumerate() {
        let current_symbol = &chord.full_symbol;

        // Skip placeholders
        if is_placeholder_chord(current_symbol) {
            continue;
        }

        let is_first_real = is_first_real_chord(&measure.chords, chord_idx);

        // Skip pushed chords that spill back (except at boundaries)
        if let Some((is_push, _)) = &chord.push_pull {
            if *is_push && is_first_real && !is_boundary {
                continue;
            }
        }

        // Check for pushed chord at boundary
        let is_pushed_at_boundary = chord
            .push_pull
            .as_ref()
            .map_or(false, |(is_push, _)| *is_push)
            && is_first_real
            && is_boundary;

        // Check if duplicate
        if should_hide_chord(
            chord,
            current_symbol,
            last_chord_symbol.as_deref(),
            is_pushed_at_boundary,
            ctx.time_signature,
            ctx.hide_repeated_chords,
        ) {
            last_chord_symbol = Some(current_symbol.clone());
            continue;
        }

        // Update tracker
        last_chord_symbol = Some(current_symbol.clone());

        // Calculate segment index
        let segment_idx = calculate_segment_index(
            measure,
            chord_idx,
            chord,
            ctx.segment_positions,
            ctx.internal_push_positions,
            is_first_real,
            is_boundary,
        );

        {
            let is_pushed = chord
                .push_pull
                .as_ref()
                .map_or(false, |(is_push, _)| *is_push);
            tracing::debug!(
                "[chord-render] section={} measure={} chord_idx={} '{}' is_pushed={} is_first_real={} is_boundary={} is_pushed_at_boundary={} segment_idx={} internal_push_positions={:?}",
                ctx.section_name,
                ctx.measure_idx,
                chord_idx,
                current_symbol,
                is_pushed,
                is_first_real,
                is_boundary,
                is_pushed_at_boundary,
                segment_idx,
                ctx.internal_push_positions
            );
        }

        // Get segment x position
        let segment_x = ctx
            .segment_positions
            .get(segment_idx)
            .copied()
            .unwrap_or_else(|| ctx.segment_positions.first().copied().unwrap_or(0.0));

        let chord_x = ctx.measure_x + segment_x;

        // Check if chord has regular accent (not AccentOnPush - that renders on spillback)
        // Only Command::Accent renders here; AccentOnPush renders on the spillback chord
        let has_regular_accent = chord.commands.iter().any(|c| matches!(c, Command::Accent));
        let chord_y_offset = if has_regular_accent {
            // Move chord up by 0.5 spatium to make room for accent below
            -ctx.spatium * 0.5
        } else {
            0.0
        };

        // Create harmony params
        let mut params = super::chord_layout::chord_to_harmony_params(chord, ctx.harmony_style);
        params.position = kurbo::Point::new(chord_x, ctx.chord_y + chord_y_offset);
        params.id = id_counter;
        id_counter += 1;

        let (layout_data, mut chord_node) = layout_harmony(&params, layout_ctx);

        // Store bounds info for collision detection
        // layout_harmony returns bounds already in world coordinates (includes params.position)
        chord_bounds_info.push(ChordBoundsInfo {
            node_idx: nodes.len(),
            original_x: chord_x,
            world_bounds: layout_data.bounds,
        });

        // Add metadata
        if let Some(page) = ctx.page_number {
            chord_node.set_page(page);
        }
        chord_node.set_system(ctx.global_system_index as u32);
        chord_node.set_measure(ctx.measure_idx as u32);
        chord_node.set_beat(segment_idx as u32);
        chord_node.set_element_type("chord");
        chord_node.set_section_type(ctx.section_name);

        // Chart position for musical coordinates
        let chart_pos = ChartPosition::new(
            ctx.global_system_index as u32,
            ctx.measure_idx as u32,
            chord_idx as u32,
        );
        chord_node.set_json_metadata(metadata_keys::CHART_POSITION, &chart_pos);

        // Source span for click-to-highlight
        if let Some(ref span) = chord.source_span {
            chord_node.set_json_metadata(metadata_keys::SOURCE_SPAN, span);
            let source_link = SourceLink::new(*span, chart_pos.clone());
            chord_node.set_json_metadata(metadata_keys::SOURCE_LINK, &source_link);
        }

        nodes.push(chord_node);

        // Add apostrophe marker for pushed/pulled chords when push_alters_rhythm=false
        if !ctx.push_alters_rhythm {
            if let Some((is_push, _amount)) = &chord.push_pull {
                let marker_node = create_push_marker(
                    *is_push,
                    layout_data.bounds,
                    ctx.chord_y,
                    ctx.spatium,
                    id_counter,
                );
                id_counter += 1;
                nodes.push(marker_node);
            }
        }

        // Add accent marker for regular accents (rendered in red above the chord)
        // AccentOnPush accents are rendered on the spillback chord instead
        if has_regular_accent {
            let accent_node = create_accent_marker(chord_x, ctx.chord_y, ctx.spatium, id_counter);
            id_counter += 1;
            nodes.push(accent_node);
        }
    }

    // Perform collision detection and resolution if enabled
    if !chord_bounds_info.is_empty() {
        tracing::debug!(
            "[chord-collision] measure={} chords={} min_gap={:.1} bounds: {:?}",
            ctx.measure_idx,
            chord_bounds_info.len(),
            ctx.min_chord_symbol_gap,
            chord_bounds_info
                .iter()
                .map(|b| (b.original_x, b.world_bounds.x0, b.world_bounds.x1))
                .collect::<Vec<_>>()
        );
    }

    if ctx.min_chord_symbol_gap > 0.0 && chord_bounds_info.len() >= 2 {
        let collision_result = resolve_chord_collisions(
            &chord_bounds_info,
            ctx.min_chord_symbol_gap,
            ctx.measure_x,
            ctx.measure_x + ctx.measure_width,
        );

        tracing::debug!(
            "[chord-collision] had_collisions={} adjustments={:?}",
            collision_result.had_collisions,
            collision_result.adjustments
        );

        if collision_result.had_collisions {
            // Apply adjustments to nodes
            for (bounds_info, &adjustment) in chord_bounds_info
                .iter()
                .zip(collision_result.adjustments.iter())
            {
                if adjustment.abs() > 0.001 {
                    let node = &mut nodes[bounds_info.node_idx];
                    node.transform = node.transform.then_translate(Vec2::new(adjustment, 0.0));
                }
            }
        }
    }

    ChordRenderResult {
        nodes,
        last_chord_symbol,
        next_id: id_counter,
    }
}

/// Render spillback chord symbols (from next measure pushing back).
pub fn render_spillback_chords(
    ctx: &ChordRenderContext<'_>,
    spillbacks: &[PushSpillback],
    previous_chord_symbol: Option<&str>,
    mut id_counter: u64,
    layout_ctx: &LayoutContext<'_>,
) -> ChordRenderResult {
    let mut nodes = Vec::new();
    let mut last_chord_symbol = previous_chord_symbol.map(String::from);

    for spillback in spillbacks {
        // Look up the correct segment index from spillback_positions computed by rhythm builder.
        // When push_alters_rhythm is enabled and we have a triplet push, the spillback chord
        // goes on the triplet eighth (e.g., segment 4 in [Q,Q,Q,TQ,TE]) not just the last segment.
        let segment_idx = ctx
            .spillback_positions
            .iter()
            .find(|(_, symbol)| symbol == &spillback.chord_symbol)
            .map(|(idx, _)| *idx)
            .unwrap_or_else(|| ctx.segment_positions.len().saturating_sub(1));

        tracing::debug!(
            "[spillback-render] section={} measure={} '{}' beat_pos={} segment_idx={} positions_len={} segment_x={:.2} spillback_positions={:?}",
            ctx.section_name,
            ctx.measure_idx,
            spillback.chord_symbol,
            spillback.beat_position,
            segment_idx,
            ctx.segment_positions.len(),
            ctx.segment_positions.get(segment_idx).copied().unwrap_or(0.0),
            ctx.spillback_positions
        );

        let segment_x = ctx
            .segment_positions
            .get(segment_idx)
            .copied()
            .unwrap_or_else(|| ctx.segment_positions.last().copied().unwrap_or(0.0));

        let chord_x = ctx.measure_x + segment_x;

        // Offset chord Y if this spillback has an accent (AccentOnPush)
        let chord_y_offset = if spillback.has_accent {
            -ctx.spatium * 0.5
        } else {
            0.0
        };

        let mut params = parse_chord(&spillback.chord_symbol);
        params.style = ctx.harmony_style.clone();
        params.position = kurbo::Point::new(chord_x, ctx.chord_y + chord_y_offset);
        params.id = id_counter;
        id_counter += 1;

        let (_, mut spillback_node) = layout_harmony(&params, layout_ctx);

        // Add metadata
        if let Some(page) = ctx.page_number {
            spillback_node.set_page(page);
        }
        spillback_node.set_system(ctx.global_system_index as u32);
        spillback_node.set_measure(ctx.measure_idx as u32);
        spillback_node.set_element_type("chord");
        spillback_node
            .metadata
            .insert("spillback".to_string(), "true".to_string());
        spillback_node.set_section_type(ctx.section_name);

        // Update tracker for duplicate detection
        last_chord_symbol = Some(spillback.chord_symbol.clone());

        nodes.push(spillback_node);

        // Render accent if this spillback chord has AccentOnPush (>' syntax)
        // The accent appears on the pushed beat (previous measure)
        if spillback.has_accent {
            let accent_node = create_accent_marker(chord_x, ctx.chord_y, ctx.spatium, id_counter);
            id_counter += 1;
            nodes.push(accent_node);
        }
    }

    ChordRenderResult {
        nodes,
        last_chord_symbol,
        next_id: id_counter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_placeholder_chord() {
        assert!(is_placeholder_chord(""));
        assert!(is_placeholder_chord("s"));
        assert!(is_placeholder_chord("r"));
        assert!(!is_placeholder_chord("C"));
        assert!(!is_placeholder_chord("Am7"));
    }

    #[test]
    fn test_is_at_boundary() {
        // First measure of section
        assert!(is_at_boundary(0, 0));
        assert!(is_at_boundary(0, 1));

        // First measure of system (not section)
        assert!(is_at_boundary(5, 0));

        // Neither
        assert!(!is_at_boundary(5, 2));
    }

    // ==========================================================================
    // Collision Detection Tests
    // ==========================================================================

    fn make_bounds(x0: f64, width: f64) -> ChordBoundsInfo {
        ChordBoundsInfo {
            node_idx: 0,
            original_x: x0,
            world_bounds: Rect::new(x0, 0.0, x0 + width, 20.0),
        }
    }

    #[test]
    fn test_collision_no_chords() {
        let bounds: Vec<ChordBoundsInfo> = vec![];
        let result = resolve_chord_collisions(&bounds, 4.0, 0.0, 200.0);
        assert!(!result.had_collisions);
        assert!(result.adjustments.is_empty());
    }

    #[test]
    fn test_collision_single_chord() {
        let bounds = vec![make_bounds(10.0, 30.0)];
        let result = resolve_chord_collisions(&bounds, 4.0, 0.0, 200.0);
        assert!(!result.had_collisions);
        assert_eq!(result.adjustments.len(), 1);
        assert!((result.adjustments[0]).abs() < 0.001);
    }

    #[test]
    fn test_collision_no_overlap() {
        // Two chords with enough space between them
        let bounds = vec![
            make_bounds(10.0, 30.0), // ends at 40
            make_bounds(50.0, 30.0), // starts at 50, gap of 10
        ];
        let result = resolve_chord_collisions(&bounds, 4.0, 0.0, 200.0);
        assert!(!result.had_collisions);
        assert!((result.adjustments[0]).abs() < 0.001);
        assert!((result.adjustments[1]).abs() < 0.001);
    }

    #[test]
    fn test_collision_detected_and_resolved() {
        // Two chords that overlap (gap of 2, need 4)
        let bounds = vec![
            make_bounds(10.0, 30.0), // ends at 40
            make_bounds(42.0, 30.0), // starts at 42, gap of only 2
        ];
        let result = resolve_chord_collisions(&bounds, 4.0, 0.0, 200.0);
        assert!(result.had_collisions);
        // First chord should shift left by the full overlap amount
        assert!(result.adjustments[0] < 0.0);
        // Second chord should NOT move (notehead position is correct via segment widths)
        assert!((result.adjustments[1]).abs() < 0.001);
        // First chord's adjustment should be approximately the overlap amount (2)
        assert!((result.adjustments[0] + 2.0).abs() < 0.1);
    }

    #[test]
    fn test_collision_allows_past_clef() {
        // Chord at very start of measure CAN shift left past the clef
        // This is intentional - the first chord can move past the clef position
        let bounds = vec![
            make_bounds(0.0, 30.0),  // ends at 30, at measure start
            make_bounds(32.0, 30.0), // starts at 32, gap of only 2
        ];
        let result = resolve_chord_collisions(&bounds, 4.0, 0.0, 200.0);
        assert!(result.had_collisions);
        // First chord moves left by full overlap (even past measure start)
        assert!(result.adjustments[0] < 0.0);
        // Second chord does NOT move (notehead position is correct)
        assert!((result.adjustments[1]).abs() < 0.001);
        // First chord moves left by full overlap amount (2.0)
        assert!((result.adjustments[0] + 2.0).abs() < 0.1);
    }

    #[test]
    fn test_collision_three_chords_cascade() {
        // Three overlapping chords - each first chord of a pair moves left
        let bounds = vec![
            make_bounds(10.0, 30.0), // ends at 40
            make_bounds(42.0, 30.0), // starts at 42, ends at 72 (gap 2)
            make_bounds(74.0, 30.0), // starts at 74 (gap 2)
        ];
        let result = resolve_chord_collisions(&bounds, 4.0, 0.0, 200.0);
        assert!(result.had_collisions);
        // First chord shifts left for first pair collision
        assert!(result.adjustments[0] < 0.0);
        // Middle chord shifts left for second pair collision
        assert!(result.adjustments[1] < 0.0);
        // Last chord should not move (no chord after it)
        assert!((result.adjustments[2]).abs() < 0.001);
    }
}
