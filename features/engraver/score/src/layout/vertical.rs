//! Vertical placement of a system's markings, MuseScore's way.
//!
//! Port of the ordering and autoplace rules of MuseScore 4's
//! `SystemLayout::layoutSystemElements` (rendering/score/systemlayout.cpp),
//! the tie/slur shaping of `SlurTieLayout` (slurtielayout.cpp) and the
//! dynamics/hairpin snapping of `TLayout::layoutHairpinSegment`, GPLv3.
//!
//! The staff skyline starts with the notation (everything the measure
//! builder drew). Then, in this order, each kind of marking is positioned,
//! pushed clear of whatever the skyline already holds, and added to it:
//!
//! 1. ties (they hug the noteheads),
//! 2. articulations (close ones by the head, the rest outside the staff),
//! 3. slurs (reshaped until they clear everything between their ends),
//! 4. dynamics and hairpins (each pushed clear, then each snapped chain
//!    aligned to its outermost member),
//! 5. staff text (stacks: texts see each other),
//! 6. measure numbers,
//! 7. rehearsal marks — last, so they sit outside everything else.
//!
//! Positions are staff-local: y = 0 on the top line, y down, x from the
//! staff's left end. Where MuseScore subtracts the style offset before the
//! collision test and adds it after, we test at the offset position and push
//! from there (its older "rebase" behaviour): a dynamic sits its style
//! distance below the staff and moves only as far as an obstruction needs.

use std::collections::HashMap;

use engraver_proto::engraver::layout::context::LayoutContextOwned;
use engraver_proto::engraver::scene::id::{ElementType, SemanticId};
use engraver_proto::engraver::scene::node::SceneNode;
use engraver_proto::engraver::scene::paint::{FontStyle, FontWeight, PaintCommand, TextAnchor};
use kurbo::{Affine, BezPath, CubicBez, ParamCurve, Point, Rect, Vec2};
use peniko::Color;

use super::{InkPart, MeasureAnchors, TextFont, articulation_type, dynamic_type, styled_text_in};
use crate::metrics;
use crate::model::{self, Enclosure, RehearsalMark};
use crate::section_marks::{self, MarkClass, MarkStyle, TextRole};
use crate::skyline::{SkyKind, Skyline};
use engraver_proto::engraver::layout::chart::constants::REPEAT_COLOR;
use engraver_proto::engraver::layout::chart::section_layout::get_section_theme;
use engraver_proto::engraver::layout::tlayout::{
    MarginLabelParams, RehearsalMarkStyle, layout_margin_label, rehearsal_themes,
};
use keyflow_proto::sections::SectionType;

/// MuseScore style defaults (styledef.cpp), in spatiums.
mod style {
    pub const SKYLINE_H_CLEARANCE: f64 = 0.25;
    pub const DYNAMICS_MIN_DISTANCE: f64 = 0.5;
    /// Dynamics baseline below the bottom line (`dynamicsPosBelow`).
    pub const DYNAMICS_POS_BELOW: f64 = 2.0;
    pub const HAIRPIN_MIN_DISTANCE: f64 = 0.7;
    pub const HAIRPIN_HEIGHT: f64 = 1.15;
    pub const HAIRPIN_LINE_WIDTH: f64 = 0.12;
    pub const HAIRPIN_DYNAMICS_DISTANCE: f64 = 0.5;
    pub const STAFF_TEXT_MIN_DISTANCE: f64 = 0.5;
    /// Staff text baseline above the top line (`staffTextPosAbove`).
    pub const STAFF_TEXT_POS_ABOVE: f64 = 1.0;
    pub const MEASURE_NUMBER_MIN_DISTANCE: f64 = 0.5;
    pub const REHEARSAL_MIN_DISTANCE: f64 = 0.5;
    /// Rehearsal box bottom above the top line before autoplace.
    pub const REHEARSAL_POS_ABOVE: f64 = 1.5;
    pub const ARTICULATION_MIN_DISTANCE: f64 = 0.4;
    pub const PROPERTY_DISTANCE_HEAD: f64 = 0.4;
    pub const SLUR_END_WIDTH: f64 = 0.05;
    pub const SLUR_MID_WIDTH: f64 = 0.21;
    pub const TIE_MIN_SHOULDER: f64 = 0.3;
    pub const TIE_MAX_SHOULDER: f64 = 2.0;
}

/// A marking anchored to a whole item (a multirest or a measure's start)
/// rather than to a note: rehearsal marks and staff text on rests.
pub(super) struct ItemMarks<'a> {
    /// Item extent in system x.
    pub span: (f64, f64),
    /// First item of the system (its marks sit over the clef).
    pub first: bool,
    pub directions: &'a [model::Direction],
    /// The item is a notated measure (its notes anchor words/dynamics).
    pub notated: bool,
}

/// A measure number to place.
pub(super) struct NumberMark {
    pub text: String,
    pub font: TextFont,
    /// Default baseline position (system coords) from the number style.
    pub pos: Point,
    pub anchor: TextAnchor,
    pub color: Color,
}

pub(super) struct SystemInput<'a, 'm> {
    pub spatium: f64,
    /// Staff length (prefix + measures).
    pub width: f64,
    /// Where the measures start (after the clef/key/time prefix).
    pub content_start: f64,
    pub anchors: &'a [MeasureAnchors<'m>],
    pub items: Vec<ItemMarks<'a>>,
    pub numbers: Vec<NumberMark>,
    pub text_family: &'a str,
    pub words_font: &'a TextFont,
    pub ctx: &'a LayoutContextOwned,
    /// How rehearsal marks are drawn.
    pub marks: MarkStyle,
    /// Per-song text decisions, by [`section_marks::text_key`].
    pub roles: &'a HashMap<String, TextRole>,
    /// Room left of the staff for a section avatar (staff left edge to the
    /// printable page edge), points.
    pub margin_room: f64,
    /// Space the margin badge leaves before the staff (a grand staff's
    /// brace), points.
    pub badge_inset: f64,
    /// A grand staff's other staves, for anchoring a direction at a beat
    /// where only they have a note.
    pub others: Vec<&'a [MeasureAnchors<'m>]>,
}

/// The placed markings of one system, and its finished skyline.
pub(super) struct Placed {
    pub nodes: Vec<SceneNode>,
    pub skyline: Skyline,
}

/// Owners: every placed item gets a fresh id so it never collides with
/// itself, and same-kind ignore rules can tell items apart.
struct Ids(u64);
impl Ids {
    fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}

/// The ink of a scene node in system coordinates.
fn ink(node: &SceneNode) -> Vec<Rect> {
    metrics::node_ink(node, Affine::IDENTITY)
}

fn union(rects: &[Rect]) -> Option<Rect> {
    rects.iter().copied().reduce(|a, b| a.union(b))
}

/// Run the placement passes for one system. `notation` is everything the
/// system already drew (staff, prefix, measures, barlines, multirests).
pub(super) fn place_system(
    notation: &[SceneNode],
    input: &SystemInput<'_, '_>,
    id: &mut u64,
) -> Placed {
    let sp = input.spatium;
    let mut sky = Skyline::new(4.0 * sp);
    let mut owners = Ids(0);
    // Full notation ink (inside the staff too): slurs and ties avoid it.
    let mut obstacles: Vec<Rect> = Vec::new();
    for n in notation {
        let rects = ink(n);
        let owner = owners.next();
        sky.add_all(&rects, SkyKind::Notation, owner);
        // Staff lines (and the long hairlines of barline-to-barline rules)
        // are crossed by every slur and tie by design; they are not
        // obstacles (MuseScore's slur shapes leave them out too).
        obstacles.extend(
            rects
                .into_iter()
                .filter(|r| !(r.width() > 3.0 * sp && r.height() < 0.3 * sp)),
        );
    }
    let mut out = Placed {
        nodes: Vec::new(),
        skyline: Skyline::new(4.0 * sp),
    };

    let ties = place_ties(input, &obstacles, &mut sky, &mut owners, &mut out.nodes, id);
    let artic = place_articulations(input, &ties, &mut sky, &mut owners, &mut out.nodes);
    obstacles.extend(ties.iter().flat_map(|t| t.rects.iter().copied()));
    obstacles.extend(artic.iter().map(|a| a.rect));
    place_slurs(
        input,
        &obstacles,
        &artic,
        &mut sky,
        &mut owners,
        &mut out.nodes,
        id,
    );
    place_dynamics(input, &mut sky, &mut owners, &mut out.nodes, id);
    place_tempos(input, &mut sky, &mut owners, &mut out.nodes);
    place_staff_text(input, &mut sky, &mut owners, &mut out.nodes);
    place_measure_numbers(input, &mut sky, &mut owners, &mut out.nodes);
    place_rehearsals(input, &mut sky, &mut owners, &mut out.nodes);
    out.skyline = sky;
    out
}

// ───────────────────────── geometry helpers ─────────────────────────

const HEAD_W_SP: f64 = 1.18;

fn y_of(line: i32, sp: f64) -> f64 {
    2.0 * sp - f64::from(line) * sp / 2.0
}

/// Fallback stem rule (on/above the middle line → down) for entries the
/// builder drew without a stem.
fn stem_up(primary_line: i32) -> bool {
    primary_line < 0
}

impl MeasureAnchors<'_> {
    /// The stem direction actually drawn for entry `i`: read off the ink
    /// (stem above or below its heads) — beamed notes carry no builder
    /// direction — then the builder's report, then the line rule.
    fn stem_up_at(&self, i: usize, primary_line: i32) -> bool {
        if let Some(ink) = self.inks.get(i).and_then(|k| k.as_ref()) {
            let stem = ink
                .parts
                .iter()
                .filter(|(_, p)| *p == InkPart::Stem)
                .map(|(r, _)| *r)
                .reduce(|a, b| a.union(b));
            let heads = ink
                .parts
                .iter()
                .filter(|(_, p)| matches!(p, InkPart::Head(_)))
                .map(|(r, _)| *r)
                .reduce(|a, b| a.union(b));
            if let (Some(stem), Some(heads)) = (stem, heads) {
                return stem.center().y < heads.center().y;
            }
        }
        self.stems
            .get(i)
            .copied()
            .flatten()
            .unwrap_or_else(|| stem_up(primary_line))
    }
}

/// A filled slur/tie shape: two cubics (outer and inner edge) whose
/// thickness runs from `end_w` at the tips to `mid_w` at the apex. `bow` is
/// the signed shoulder vector (negative y = bows up).
fn crescent(
    p1: Point,
    c1: Point,
    c2: Point,
    p2: Point,
    end_w: f64,
    mid_w: f64,
    up: bool,
) -> BezPath {
    let sign = if up { -1.0 } else { 1.0 };
    let half_end = Vec2::new(0.0, sign * end_w / 2.0);
    let half_mid = Vec2::new(0.0, sign * mid_w / 2.0);
    let mut path = BezPath::new();
    // Outer edge (further from the notes), then the inner edge back.
    path.move_to(p1 + half_end);
    path.curve_to(c1 + half_mid * 1.33, c2 + half_mid * 1.33, p2 + half_end);
    path.line_to(p2 - half_end);
    path.curve_to(c2 - half_mid * 1.33, c1 - half_mid * 1.33, p1 - half_end);
    path.close_path();
    path
}

/// Thin rectangles following a curve: the skyline shape of a tie/slur
/// (MuseScore's `fillShape`).
fn curve_rects(curve: &CubicBez, thickness: f64, n: usize) -> Vec<Rect> {
    let mut out = Vec::with_capacity(n);
    let mut prev = curve.eval(0.0);
    for i in 1..=n {
        let p = curve.eval(i as f64 / n as f64);
        out.push(Rect::from_points(prev, p).inflate(0.0, thickness / 2.0));
        prev = p;
    }
    out
}

fn curve_node(kind: ElementType, id: u64, path: BezPath) -> SceneNode {
    SceneNode::leaf(
        SemanticId::new(kind, id),
        vec![PaintCommand::Fill {
            path,
            color: Color::BLACK,
            fill_rule: Default::default(),
        }],
    )
}

/// Control points of a curve from p1 to p2 bowed by `shoulder` (absolute,
/// sign = direction) with shoulder width `w` (MuseScore `computeBezier`).
fn bow_controls(p1: Point, p2: Point, shoulder: f64, w: f64) -> (Point, Point) {
    let d = p2 - p1;
    let len = d.hypot().max(1e-6);
    let along = d / len;
    let normal = Vec2::new(-along.y, along.x); // +normal = down for left→right
    let c1 = p1 + along * (len * (1.0 - w) / 2.0) + normal * shoulder;
    let c2 = p1 + along * (len * (1.0 + w) / 2.0) + normal * shoulder;
    (c1, c2)
}

// ───────────────────────── 1. ties ─────────────────────────
//
// Port of MuseScore's tie layout (SlurTieLayout::layoutTieFor/Back,
// calculateDirection, computeDefaultStartOrEndPoint, adjustX, adjustY,
// adjustYforLedgerLines, computeBezier(TieSegment), computeMidThickness,
// resolveVerticalTieCollisions), working on the built chords' real ink.

struct PlacedTie {
    rects: Vec<Rect>,
    /// For leave-space-for-tie: the entry it starts or ends on, and side.
    above: bool,
    entries: Vec<(usize, usize)>, // (anchor index, entry index)
}

/// One end of a tie: which entry, which note.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TieEnd {
    ai: usize,
    i: usize,
    line: i32,
}

/// A tie being laid out (one system's segment).
struct TieGeom {
    start: Option<TieEnd>,
    end: Option<TieEnd>,
    up: bool,
    p1: Point,
    p2: Point,
    /// Extra shoulder height (MuseScore's shoulder offset), y-down sign.
    shoulder_off: f64,
}

/// Every notehead line of entry `i`.
fn chord_lines(a: &MeasureAnchors<'_>, i: usize) -> Vec<i32> {
    a.m.pitches[i]
        .iter()
        .map(|(l, _)| *l)
        .chain(a.m.stacks[i].iter().map(|(l, _)| *l))
        .collect()
}

/// `calculateDirection`: single notes opposite the stem (mixed stems → up);
/// chords pivot on a single second/unison among tied notes, otherwise the
/// tie follows which side holds more ties (or notes), dead centre opposite
/// the stem. Lines here are y-up (higher line = higher note).
fn tie_up(input: &SystemInput<'_, '_>, start: Option<TieEnd>, end: Option<TieEnd>) -> bool {
    let Some(primary) = start.or(end) else {
        return true;
    };
    let a = &input.anchors[primary.ai];
    let stem_of = |e: TieEnd| {
        let an = &input.anchors[e.ai];
        an.stem_up_at(e.i, an.m.pitches[e.i].map_or(e.line, |(l, _)| l))
    };
    let notes = chord_lines(a, primary.i);
    if notes.len() <= 1 {
        if let (Some(s), Some(e)) = (start, end)
            && stem_of(s) != stem_of(e)
        {
            return true;
        }
        return !stem_of(primary);
    }
    let tied: Vec<i32> = if start.is_some() {
        a.m.tie_start_lines[primary.i].clone()
    } else {
        a.m.tie_stop_lines[primary.i].clone()
    };
    // Pivot: the lower note of the only second/unison between tied notes.
    let mut pivot: Option<i32> = None;
    let mut multiple = false;
    for (x, &l1) in tied.iter().enumerate() {
        for &l2 in &tied[x + 1..] {
            if (l1 - l2).abs() <= 1 && !multiple {
                if pivot.is_some() {
                    multiple = true;
                    pivot = None;
                } else {
                    pivot = Some(l1.min(l2));
                }
            }
        }
    }
    let line = primary.line;
    match pivot {
        Some(p) if p == line => false,
        Some(p) => line >= p,
        None => {
            let above = notes.iter().filter(|l| **l > line).count();
            let below = notes.iter().filter(|l| **l < line).count();
            let ties_above = tied.iter().filter(|l| **l > line).count();
            let ties_below = tied.iter().filter(|l| **l < line).count();
            let unison = tied
                .iter()
                .filter(|l| **l == line)
                .count()
                .saturating_sub(1);
            if ties_above == 0 && ties_below == 0 && unison == 0 {
                if above == below {
                    !stem_of(primary)
                } else {
                    above < below
                }
            } else if ties_above == ties_below {
                !stem_of(primary)
            } else {
                ties_above < ties_below
            }
        }
    }
}

/// The notehead box of a tie end (built ink, or the 1.18 sp estimate).
fn head_rect(input: &SystemInput<'_, '_>, e: TieEnd) -> Rect {
    let sp = input.spatium;
    let a = &input.anchors[e.ai];
    if let Some(r) = a
        .inks
        .get(e.i)
        .and_then(|ink| ink.as_ref())
        .and_then(|ink| ink.head(e.line))
    {
        return r;
    }
    let x = a.entry_x.get(e.i).copied().unwrap_or(0.0);
    let y = y_of(e.line, sp);
    Rect::new(x, y - 0.5 * sp, x + HEAD_W_SP * sp, y + 0.5 * sp)
}

/// `isOuterTieOfChord`: the top note tied upward or the bottom one down.
fn is_outer(input: &SystemInput<'_, '_>, e: TieEnd, up: bool) -> bool {
    let lines = chord_lines(&input.anchors[e.ai], e.i);
    if up {
        lines.iter().all(|l| *l <= e.line)
    } else {
        lines.iter().all(|l| *l >= e.line)
    }
}

/// `computeDefaultStartOrEndPoint`: x at the head's optical centre inset
/// 0.1 sp toward the other end (0.45 sp when another head sits straight
/// above/below on the tie side), y just beyond the head on the tie side.
fn tie_default_point(input: &SystemInput<'_, '_>, e: TieEnd, up: bool, start: bool) -> Point {
    let sp = input.spatium;
    let head = head_rect(input, e);
    let up_sign = if up { -1.0 } else { 1.0 };
    let lr_sign = if start { 1.0 } else { -1.0 };
    let neighbour = if up { e.line + 1 } else { e.line - 1 };
    let stacked = input.anchors[e.ai]
        .inks
        .get(e.i)
        .and_then(|ink| ink.as_ref())
        .and_then(|ink| ink.head(neighbour))
        .is_some_and(|r| (r.center().x - head.center().x).abs() < 0.3 * sp);
    let inset = if stacked { 0.45 } else { 0.1 } * sp;
    Point::new(
        head.center().x + inset * lr_sign,
        head.center().y + up_sign * head.height() / 2.0 + 0.2 * sp * up_sign,
    )
}

/// `adjustX`: clear the chord's own ink (stem on the tie side, dots, other
/// heads, accidentals) at the tie's height; hooks and ledger lines ignored.
fn tie_adjust_x(input: &SystemInput<'_, '_>, e: TieEnd, up: bool, start: bool, point: &mut Point) {
    let sp = input.spatium;
    let a = &input.anchors[e.ai];
    let Some(ink) = a.inks.get(e.i).and_then(|ink| ink.as_ref()) else {
        return;
    };
    let padding = 0.2 * sp * if start { 1.0 } else { -1.0 };
    let outer = is_outer(input, e, up);
    let stem_up = a.stem_up_at(e.i, a.m.pitches[e.i].map_or(e.line, |(l, _)| l));
    let avoid_stem = ink.has_stem() && stem_up == up;
    if outer && !avoid_stem {
        return;
    }
    let (arc_margin, point_margin) = (0.3 * sp, 0.15 * sp);
    let y_lo = point.y - if up { arc_margin } else { point_margin };
    let y_hi = point.y + if up { point_margin } else { arc_margin };
    let head = head_rect(input, e);
    let mut edge: Option<f64> = None;
    for (r, part) in &ink.parts {
        let ignored = match part {
            InkPart::Head(l) => *l == e.line,
            // MuseScore ignores hooks; a tie leaving a flagged note on the
            // stem side then grazes the flag, so the start clears it too.
            InkPart::Flag => !start,
            InkPart::Ledger => true,
            InkPart::Dot => start && outer,
            // MuseScore lets an outer tie arriving at its note pass over
            // an accidental; a flat's stem reaches up into the arc there,
            // so the tie stops short of it like any other ink in its band.
            _ => false,
        };
        if ignored || r.y1 < y_lo || r.y0 > y_hi {
            continue;
        }
        // Only ink on the tie's side of the head.
        if start && r.x1 <= head.x0 || !start && r.x0 >= head.x1 {
            continue;
        }
        edge = Some(match (edge, start) {
            (None, true) => r.x1,
            (None, false) => r.x0,
            (Some(x), true) => x.max(r.x1),
            (Some(x), false) => x.min(r.x0),
        });
    }
    if let Some(edge) = edge {
        let target = edge + padding;
        point.x = if start {
            point.x.max(target)
        } else {
            point.x.min(target)
        };
    }
}

/// `adjustYforLedgerLines`: keep the tie end 0.4 sp clear of a ledger line
/// on its side.
fn tie_adjust_y_ledgers(input: &SystemInput<'_, '_>, e: TieEnd, up: bool, g: &mut TieGeom) {
    let sp = input.spatium;
    let Some(ink) = input.anchors[e.ai].inks.get(e.i).and_then(|i| i.as_ref()) else {
        return;
    };
    let up_sign = if up { -1.0 } else { 1.0 };
    let margin = 0.4 * sp;
    let y = if g.start == Some(e) { g.p1.y } else { g.p2.y };
    for (r, part) in &ink.parts {
        if *part != InkPart::Ledger {
            continue;
        }
        let y_diff = up_sign * (r.center().y - y);
        if y_diff > 0.0 && y_diff < margin {
            let dy = -up_sign * (margin - y_diff);
            g.p1.y += dy;
            g.p2.y += dy;
            return;
        }
    }
}

/// Tie thickness at the apex (`computeMidThickness`): full for ties over
/// 4 sp, thinning toward 0.15 sp at the minimum tie length.
fn tie_mid_thickness(len_sp: f64, sp: f64) -> f64 {
    let end_w = style::SLUR_END_WIDTH * sp;
    let normal = style::SLUR_MID_WIDTH * sp - end_w;
    let min_t = 0.15 * sp - end_w;
    let (min_len, short_limit) = (1.0, 4.0);
    if len_sp > short_limit {
        return normal;
    }
    let a = 1.0 / (short_limit - min_len);
    let b = normal - min_t;
    let c = short_limit * min_t - min_len * normal;
    (a * (b * len_sp + c)).clamp(0.0, normal)
}

/// The tie's centre-line bezier (`computeBezier(TieSegment)`).
fn tie_curve(g: &TieGeom, sp: f64) -> CubicBez {
    let len_sp = (g.p2 - g.p1).hypot() / sp;
    let shoulder = (style::TIE_MIN_SHOULDER * sp + 0.3 * sp * (len_sp - 1.0).abs().sqrt())
        .clamp(style::TIE_MIN_SHOULDER * sp, style::TIE_MAX_SHOULDER * sp)
        + g.shoulder_off;
    let sign = if g.up { -1.0 } else { 1.0 };
    let (c1, c2) = bow_controls(g.p1, g.p2, sign * shoulder, 0.6);
    CubicBez::new(g.p1, c1, c2, g.p2)
}

/// Outer and inner apex y of a tie (for staff-line and stacking checks).
fn tie_apex(g: &TieGeom, sp: f64) -> (f64, f64, f64) {
    let curve = tie_curve(g, sp);
    let mid = curve.eval(0.5).y;
    let thick = tie_mid_thickness((g.p2.x - g.p1.x) / sp, sp);
    let sign = if g.up { -1.0 } else { 1.0 };
    // (outer, inner, height)
    (
        mid + sign * thick,
        mid - sign * thick,
        (mid - g.p1.y).abs() + thick,
    )
}

/// `adjustY`: keep the end points off staff lines, then the arc's apex.
fn tie_adjust_y(g: &mut TieGeom, sp: f64, outer_start: bool, single_inner_small_ok: bool) {
    let up_sign = if g.up { -1.0 } else { 1.0 };
    let line_w = 0.1 * sp;
    let half_line = 0.5 * line_w * up_sign;
    // 1. End points protruding onto a line (inside the staff).
    let y = g.p1.y;
    let closest = if g.up {
        (y / sp).floor()
    } else {
        (y / sp).ceil()
    };
    let inside = (0.0..=4.0).contains(&closest);
    let protrusion = (y - (closest * sp - half_line)).abs();
    let limit = 0.15 * sp;
    if protrusion < limit && (inside || !outer_start) {
        let corrected = closest * sp + half_line + limit * up_sign;
        g.p2.y += corrected - y;
        g.p1.y = corrected;
    }
    // 2. The arc's apex sitting on a line.
    let (outer_y, inner_y, height) = tie_apex(g, sp);
    let mid_y = 0.5 * (outer_y + inner_y);
    let line = (mid_y / sp).round();
    if !(0.0..=4.0).contains(&line) {
        return;
    }
    let outward = -up_sign * (outer_y - (line * sp - half_line));
    let inward = up_sign * (inner_y - (line * sp + half_line));
    let length = g.p2.x - g.p1.x;
    let arc_limit = if length < 3.0 * sp {
        0.1 * sp
    } else {
        0.15 * sp
    };
    let increase = outward - 0.5 * arc_limit < inward;
    let mut out_fix = inward < arc_limit && increase;
    let in_fix = outward < arc_limit && !increase;
    if !in_fix && !out_fix {
        return;
    }
    let small = length < 2.0 * sp && height < 0.7 * sp;
    if small {
        if single_inner_small_ok {
            // Small ties move whole into the staff space.
            let dy = -up_sign * (arc_limit - outward);
            g.p1.y += dy;
            g.p2.y += dy;
            return;
        }
        out_fix = true;
    }
    let mut correction = if out_fix {
        arc_limit - inward
    } else {
        arc_limit - outward
    };
    let max_corr = 0.75 * height;
    let rest = (correction - max_corr).max(0.0);
    correction = correction.min(max_corr);
    if out_fix {
        if rest > 0.0 {
            g.p1.y += rest * up_sign;
            g.p2.y += rest * up_sign;
        }
        g.shoulder_off += correction;
    } else {
        g.shoulder_off -= correction;
    }
}

fn place_ties(
    input: &SystemInput<'_, '_>,
    _obstacles: &[Rect],
    sky: &mut Skyline,
    _owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
    id: &mut u64,
) -> Vec<PlacedTie> {
    let sp = input.spatium;
    // Pair tie starts with their stops, by note line, in order.
    let mut specs: Vec<(Option<TieEnd>, Option<TieEnd>)> = Vec::new();
    let mut pending: Vec<TieEnd> = Vec::new();
    for (ai, a) in input.anchors.iter().enumerate() {
        for i in 0..a.m.entries.len() {
            for &line in &a.m.tie_stop_lines[i] {
                let end = TieEnd { ai, i, line };
                match pending.iter().position(|p| p.line == line) {
                    Some(pos) => specs.push((Some(pending.remove(pos)), Some(end))),
                    None => specs.push((None, Some(end))), // from the system before
                }
            }
            for &line in &a.m.tie_start_lines[i] {
                pending.push(TieEnd { ai, i, line });
            }
        }
    }
    specs.extend(pending.into_iter().map(|p| (Some(p), None))); // into the next system

    let mut geoms: Vec<TieGeom> = Vec::new();
    for (start, end) in specs {
        let up = tie_up(input, start, end);
        let mut p1 = start.map(|s| tie_default_point(input, s, up, true));
        let mut p2 = end.map(|e| tie_default_point(input, e, up, false));
        if let (Some(s), Some(p)) = (start, p1.as_mut()) {
            tie_adjust_x(input, s, up, true, p);
        }
        if let (Some(e), Some(p)) = (end, p2.as_mut()) {
            tie_adjust_x(input, e, up, false, p);
        }
        // Segments across a system break: to the staff end, or from just
        // before the system's first note.
        let (p1, p2) = match (p1, p2) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) => (a, Point::new(input.width - 0.25 * sp, a.y)),
            (None, Some(b)) => (
                Point::new((b.x - 3.0 * sp).max(input.content_start), b.y),
                b,
            ),
            (None, None) => continue,
        };
        let p2 = if p2.x - p1.x < 0.8 * sp {
            Point::new(p1.x + 0.8 * sp, p2.y)
        } else {
            p2
        };
        let mut g = TieGeom {
            start,
            end,
            up,
            p1,
            p2,
            shoulder_off: 0.0,
        };
        if let Some(e) = start.or(end) {
            tie_adjust_y_ledgers(input, e, up, &mut g);
        }
        let outer_start = start.is_some_and(|s| is_outer(input, s, up));
        let single = start.is_some_and(|s| chord_lines(&input.anchors[s.ai], s.i).len() <= 1);
        tie_adjust_y(&mut g, sp, outer_start, single);
        geoms.push(g);
    }

    // `resolveVerticalTieCollisions`: ties stacked on one chord keep
    // 0.15 sp between one's outer edge and the next one's inner edge.
    let clearance = 0.15 * sp;
    let mut groups: HashMap<(usize, usize, bool), Vec<usize>> = HashMap::new();
    for (k, g) in geoms.iter().enumerate() {
        if let Some(s) = g.start {
            groups.entry((s.ai, s.i, g.up)).or_default().push(k);
        }
    }
    for ((_, _, up), mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        // Innermost first: for upward ties the lowest, downward the highest.
        members.sort_by(|a, b| {
            let (ya, yb) = (geoms[*a].p1.y, geoms[*b].p1.y);
            if up {
                yb.total_cmp(&ya)
            } else {
                ya.total_cmp(&yb)
            }
        });
        let up_sign = if up { -1.0 } else { 1.0 };
        for w in members.windows(2) {
            let (this_outer, ..) = tie_apex(&geoms[w[0]], sp);
            let (_, next_inner, _) = tie_apex(&geoms[w[1]], sp);
            if up_sign * (next_inner - this_outer) >= clearance {
                continue;
            }
            let mut mid = 0.5 * (this_outer + next_inner);
            let half = 0.5 * sp;
            let line = (mid / half).round();
            if (-1.0..=9.0).contains(&line) {
                mid = line * half;
            }
            let this_fix = mid - up_sign * 0.5 * clearance - this_outer;
            let next_fix = mid + up_sign * 0.5 * clearance - next_inner;
            geoms[w[0]].shoulder_off += -up_sign * this_fix;
            geoms[w[1]].shoulder_off += -up_sign * next_fix;
        }
    }

    let mut out = Vec::new();
    for g in geoms {
        let curve = tie_curve(&g, sp);
        let thick = tie_mid_thickness((g.p2.x - g.p1.x) / sp, sp);
        let end_w = style::SLUR_END_WIDTH * sp;
        let path = crescent(
            curve.p0,
            curve.p1,
            curve.p2,
            curve.p3,
            end_w,
            1.5 * thick + end_w,
            g.up,
        );
        *id += 1;
        nodes.push(curve_node(ElementType::Tie, *id, path));
        let n = ((g.p2.x - g.p1.x) / sp * 5.0).round().clamp(20.0, 50.0) as usize;
        let rects = curve_rects(&curve, 2.0 * thick, n);
        sky.add_all(&rects, SkyKind::Tie, *id);
        out.push(PlacedTie {
            rects,
            above: g.up,
            entries: g
                .start
                .iter()
                .chain(g.end.iter())
                .map(|e| (e.ai, e.i))
                .collect(),
        });
    }
    out
}

// ───────────────────────── 2. articulations ─────────────────────────

/// "Close" articulations sit by the notehead (MuseScore's
/// `layoutArticulations`); the rest are staff-anchored and autoplaced.
fn is_close(tag: &str) -> bool {
    matches!(
        tag,
        "staccato" | "staccatissimo" | "tenuto" | "detached-legato" | "spiccato"
    )
}

fn place_articulations(
    input: &SystemInput<'_, '_>,
    ties: &[PlacedTie],
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) -> Vec<PlacedArtic> {
    let sp = input.spatium;
    let head_w = HEAD_W_SP * sp;
    let mut placed = Vec::new();
    for (ai, a) in input.anchors.iter().enumerate() {
        for i in 0..a.m.entries.len() {
            if a.m.arts[i].is_empty() {
                continue;
            }
            let Some((primary, _)) = a.m.pitches[i] else {
                continue;
            };
            let ex = a.entry_x.get(i).copied().unwrap_or(0.0);
            let lines: Vec<i32> = a.m.stacks[i]
                .iter()
                .map(|(l, _)| *l)
                .chain(std::iter::once(primary))
                .collect();
            let top = *lines.iter().max().unwrap_or(&primary);
            let bottom = *lines.iter().min().unwrap_or(&primary);
            let up_stem = a.stem_up_at(i, primary);
            let owner = owners.next();
            // Stacked marks on one side grow outward.
            let mut stack: HashMap<bool, f64> = HashMap::new();
            // Close marks first, then staff-anchored ones outside them.
            let mut arts: Vec<&(String, Option<bool>)> = a.m.arts[i].iter().collect();
            arts.sort_by_key(|(t, _)| !is_close(t));
            for (tag, pinned) in arts {
                let Some(art) = articulation_type(tag) else {
                    continue;
                };
                let above = pinned.unwrap_or(!up_stem);
                let glyph = if above {
                    art.smufl_codepoint_above()
                } else {
                    art.smufl_codepoint_below()
                };
                let x = ex + head_w / 2.0;
                // Measure the glyph at the origin, then move it.
                let probe = PaintCommand::glyph(glyph, Point::ZERO, sp, Color::BLACK);
                let Some(g) =
                    metrics::with_faces(|f| metrics::command_ink(f, &probe, Affine::IDENTITY))
                else {
                    continue;
                };
                let tied_here = ties
                    .iter()
                    .any(|t| t.above == above && t.entries.contains(&(ai, i)));
                let extra = stack.get(&above).copied().unwrap_or(0.0);
                let (target_edge, mut dy);
                if is_close(tag) && pinned.is_none() {
                    // By the head, clear of a tie on the same side.
                    let gap =
                        style::PROPERTY_DISTANCE_HEAD * sp + if tied_here { 0.5 * sp } else { 0.0 };
                    if above {
                        target_edge = y_of(top, sp) - 0.5 * sp - gap - extra;
                        dy = target_edge - g.y1;
                    } else {
                        target_edge = y_of(bottom, sp) + 0.5 * sp + gap + extra;
                        dy = target_edge - g.y0;
                    }
                    // Inside the staff: centre in a space, not on a line.
                    let mid = (g.y0 + g.y1) / 2.0 + dy;
                    if mid > 0.0 && mid < 4.0 * sp {
                        let space = ((mid / sp - 0.5).round() + 0.5) * sp;
                        let space = if above {
                            space.min(mid)
                        } else {
                            space.max(mid)
                        };
                        dy += space - mid;
                    }
                } else {
                    // Staff-anchored: outside the staff and the stem tip,
                    // then pushed clear of anything else.
                    if above {
                        target_edge = (-style::PROPERTY_DISTANCE_HEAD * sp)
                            .min(y_of(top, sp) - 0.5 * sp - style::PROPERTY_DISTANCE_HEAD * sp)
                            - extra;
                        dy = target_edge - g.y1;
                    } else {
                        target_edge = (4.0 * sp + style::PROPERTY_DISTANCE_HEAD * sp)
                            .max(y_of(bottom, sp) + 0.5 * sp + style::PROPERTY_DISTANCE_HEAD * sp)
                            + extra;
                        dy = target_edge - g.y0;
                    }
                    let shape = [Rect::new(
                        x - g.width() / 2.0,
                        g.y0 + dy,
                        x + g.width() / 2.0,
                        g.y1 + dy,
                    )];
                    dy += sky.distance_to_move(
                        &shape,
                        above,
                        style::ARTICULATION_MIN_DISTANCE * sp,
                        0.0,
                        |k, o| k == SkyKind::Articulation && o != owner,
                    );
                }
                let pos = Point::new(x - g.width() / 2.0 - g.x0, dy);
                let rect = Rect::new(pos.x + g.x0, g.y0 + dy, pos.x + g.x1, g.y1 + dy);
                stack.insert(
                    above,
                    extra + g.height() + style::ARTICULATION_MIN_DISTANCE * sp,
                );
                sky.add(rect, SkyKind::Articulation, owner);
                placed.push(PlacedArtic {
                    ai,
                    i,
                    above,
                    close: is_close(tag) && pinned.is_none(),
                    rect,
                });
                let mut node = SceneNode::group(SemanticId::new(ElementType::Articulation, owner));
                node.add_command(PaintCommand::glyph(glyph, pos, sp, Color::BLACK));
                nodes.push(node);
            }
        }
    }
    placed
}

// ───────────────────────── 3. slurs ─────────────────────────
//
// Port of MuseScore's `SlurTieLayout::slurPos` anchors, `computeUp`, and
// (simplified) `avoidCollisions`, on the built chords' ink.

/// An articulation as placed (slur ends move out over close ones).
pub(super) struct PlacedArtic {
    ai: usize,
    i: usize,
    above: bool,
    close: bool,
    rect: Rect,
}

/// One slur end: the outer note's head, and the stem tip if it has one.
struct SlurEndInk {
    head: Rect,
    stem: Option<Rect>,
    stem_up: bool,
    flag: bool,
    beamed: bool,
    ink: Vec<Rect>,
}

fn slur_end_ink(input: &SystemInput<'_, '_>, ai: usize, i: usize, above: bool) -> SlurEndInk {
    let sp = input.spatium;
    let a = &input.anchors[ai];
    let lines = chord_lines(a, i);
    let line = if above {
        *lines.iter().max().unwrap_or(&0)
    } else {
        *lines.iter().min().unwrap_or(&0)
    };
    let primary = a.m.pitches[i].map_or(line, |(l, _)| l);
    let stem_up = a.stem_up_at(i, primary);
    let ink = a.inks.get(i).and_then(|x| x.as_ref());
    let head = ink.and_then(|k| k.head(line)).unwrap_or_else(|| {
        let x = a.entry_x.get(i).copied().unwrap_or(0.0);
        let y = y_of(line, sp);
        Rect::new(x, y - 0.5 * sp, x + HEAD_W_SP * sp, y + 0.5 * sp)
    });
    let stem = ink.and_then(|k| {
        k.parts
            .iter()
            .filter(|(_, p)| *p == InkPart::Stem)
            .map(|(r, _)| *r)
            .reduce(|a, b| a.union(b))
    });
    let flag = ink.is_some_and(|k| k.parts.iter().any(|(_, p)| *p == InkPart::Flag));
    // Beamed: a stem but no flag on an eighth-or-shorter is beam-joined;
    // the builder draws beamed notes without Chord groups, which is what
    // `stems` reflect, so approximate by "stem, no flag, short value".
    let short = matches!(
        a.m.entries.get(i),
        Some(engraver_proto::engraver::notation::RhythmEntry::Note(d)) if d.ticks() < 480
    );
    SlurEndInk {
        head,
        stem,
        stem_up,
        flag,
        beamed: stem.is_some() && !flag && short,
        ink: ink
            .map(|k| k.parts.iter().map(|(r, _)| *r).collect())
            .unwrap_or_default(),
    }
}

fn place_slurs(
    input: &SystemInput<'_, '_>,
    obstacles: &[Rect],
    artics: &[PlacedArtic],
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
    id: &mut u64,
) {
    let sp = input.spatium;
    let mut open: Option<(usize, usize)> = None;
    for (ai, a) in input.anchors.iter().enumerate() {
        for i in 0..a.m.entries.len() {
            if a.m.slur_stops[i]
                && let Some((sai, si)) = open.take()
            {
                let s_line = input.anchors[sai].m.pitches[si].map_or(0, |(l, _)| l);
                let e_line = a.m.pitches[i].map_or(0, |(l, _)| l);
                let s_up = input.anchors[sai].stem_up_at(si, s_line);
                let e_up = a.stem_up_at(i, e_line);
                // computeUp: opposite the stem; mixed stems → above.
                let above = if s_up == e_up { !s_up } else { true };
                let start = slur_end_ink(input, sai, si, above);
                let end = slur_end_ink(input, ai, i, above);
                let up_sign = if above { -1.0 } else { 1.0 };

                // slurPos anchors.
                let stem_anchor_start = start.stem.is_some()
                    && ((start.flag && start.stem_up == above)
                        || (s_up == e_up && s_up == above && !start.beamed));
                let stem_anchor_end = end.stem.is_some() && s_up == e_up && e_up == above;
                let tip = |e: &SlurEndInk| -> (f64, f64) {
                    let st = e.stem.unwrap_or(e.head);
                    (st.center().x, if above { st.y0 } else { st.y1 })
                };
                let default_point = |e: &SlurEndInk| -> Point {
                    Point::new(e.head.center().x, e.head.center().y + 0.9 * sp * up_sign)
                };
                let mut p1 = if stem_anchor_start {
                    let (x, y) = tip(&start);
                    Point::new(x + 0.35 * sp, y - 0.5 * sp * up_sign)
                } else if start.beamed && start.stem_up == above {
                    // Beamed start on the stem side: clear the beam.
                    let (x, y) = tip(&start);
                    Point::new(x + 0.15 * sp, y + up_sign * (0.25 + 0.35) * sp)
                } else {
                    default_point(&start)
                };
                let mut p2 = if stem_anchor_end {
                    let (x, y) = tip(&end);
                    let inset = if end.beamed { -0.75 } else { 0.5 };
                    Point::new(x - 0.35 * sp, y - inset * sp * up_sign)
                } else {
                    default_point(&end)
                };
                // fixArticulations: over/under a close articulation on the
                // slur's side, nudged 0.1 sp inward.
                for (pt, (eai, ei), inward) in [(&mut p1, (sai, si), 1.0), (&mut p2, (ai, i), -1.0)]
                {
                    for art in artics
                        .iter()
                        .filter(|x| x.ai == eai && x.i == ei && x.close && x.above == above)
                    {
                        pt.x = art.rect.center().x + 0.1 * sp * inward;
                        let edge =
                            if above { art.rect.y0 } else { art.rect.y1 } + 0.5 * sp * up_sign;
                        pt.y = if above {
                            pt.y.min(edge)
                        } else {
                            pt.y.max(edge)
                        };
                    }
                }
                if p2.x - p1.x < sp {
                    p2.x = p1.x + sp;
                }
                // Obstacles: everything between, minus the end chords' own
                // ink and articulations (MuseScore's getSegmentShapes).
                let own: Vec<Rect> = start.ink.iter().chain(end.ink.iter()).copied().collect();
                let own_artics: Vec<Rect> = artics
                    .iter()
                    .filter(|x| (x.ai, x.i) == (sai, si) || (x.ai, x.i) == (ai, i))
                    .map(|x| x.rect)
                    .collect();
                let local: Vec<Rect> = obstacles
                    .iter()
                    .filter(|r| r.x1 > p1.x && r.x0 < p2.x)
                    .filter(|r| !own.iter().chain(own_artics.iter()).any(|o| o == *r))
                    .copied()
                    .collect();
                *id += 1;
                let owner = owners.next();
                let rects = shape_slur(p1, p2, above, sp, &local, nodes, *id);
                sky.add_all(&rects, SkyKind::Slur, owner);
            }
            if a.m.slur_starts[i] && open.is_none() {
                open = Some((ai, i));
            }
        }
    }
}

/// MuseScore's `SlurTieLayout::avoidCollisions`, simplified: start from the
/// default bezier, then alternately raise the shoulder over the colliding
/// third and move the endpoints outward, until the curve (plus its arc
/// clearance) clears every obstacle between the ends — at most 30 rounds.
fn shape_slur(
    mut p1: Point,
    mut p2: Point,
    above: bool,
    sp: f64,
    obstacles: &[Rect],
    nodes: &mut Vec<SceneNode>,
    id: u64,
) -> Vec<Rect> {
    let sign = if above { -1.0 } else { 1.0 };
    let d = (p2 - p1).hypot() / sp;
    let w = if d < 2.0 {
        0.6
    } else if d < 10.0 {
        0.5
    } else if d < 18.0 {
        0.6
    } else {
        0.7
    };
    let shoulder = (d / 4.0).sqrt() * sp;
    // addMinClearanceToShapes: pad obstacles toward the slur.
    let local: Vec<Rect> = obstacles
        .iter()
        .map(|r| r.inflate(0.1 * sp, 0.3 * sp))
        .collect();
    let clearance = (0.04 * d).clamp(0.1, 0.5) * sp;
    let step = (0.3 * sp * if d > 16.0 { d / 16.0 } else { 1.0 }).min(1.5 * sp);
    let mut lift = [0.0_f64; 3]; // extra shoulder per third (left, mid, right)

    let build = |p1: Point, p2: Point, lift: &[f64; 3]| -> CubicBez {
        let (mut c1, mut c2) = bow_controls(p1, p2, sign * shoulder, w);
        c1.y += sign * (lift[0] + lift[1]);
        c2.y += sign * (lift[2] + lift[1]);
        CubicBez::new(p1, c1, c2, p2)
    };
    let mut curve = build(p1, p2, &lift);
    for iter in 0..30 {
        let test = CubicBez::new(
            curve.p0,
            curve.p1 + Vec2::new(0.0, sign * clearance),
            curve.p2 + Vec2::new(0.0, sign * clearance),
            curve.p3,
        );
        let samples = curve_rects(&test, style::SLUR_MID_WIDTH * sp, 19);
        let mut hit = [false; 3];
        for (k, r) in samples.iter().enumerate() {
            if local.iter().any(|o| o.overlaps(*r)) {
                hit[(k * 3 / samples.len()).min(2)] = true;
            }
        }
        if !hit.iter().any(|h| *h) {
            break;
        }
        if iter % 2 == 0 {
            for (t, h) in hit.iter().enumerate() {
                if *h {
                    lift[t] += step;
                }
            }
        } else {
            if hit[0] || hit[1] {
                p1.y += sign * step * 0.4;
            }
            if hit[2] || hit[1] {
                p2.y += sign * step * 0.4;
            }
        }
        let len = (p2 - p1).hypot();
        for l in &mut lift {
            *l = l.min(len);
        }
        curve = build(p1, p2, &lift);
    }
    let path = crescent(
        curve.p0,
        curve.p1,
        curve.p2,
        curve.p3,
        style::SLUR_END_WIDTH * sp,
        style::SLUR_MID_WIDTH * sp,
        above,
    );
    nodes.push(curve_node(ElementType::Slur, id, path));
    curve_rects(&curve, style::SLUR_MID_WIDTH * sp, 24)
}

// ───────────────────────── 4. dynamics + hairpins ─────────────────────────

/// Baseline (y of the first text/glyph command) of a node, local coords.
fn baseline_of(node: &SceneNode) -> Option<f64> {
    for cmd in &node.commands {
        match cmd {
            PaintCommand::Text { position, .. } | PaintCommand::Glyph { position, .. } => {
                return Some(node.transform.translation().y + position.y);
            }
            _ => {}
        }
    }
    node.children
        .iter()
        .find_map(|c| baseline_of(c).map(|y| y + node.transform.translation().y))
}

struct DynamicItem {
    node: SceneNode,
    /// Ink at its current position.
    rect: Rect,
    /// Optical centre y (MuseScore aligns chains on it).
    center: f64,
    tick_x: f64,
    owner: u64,
}

fn place_dynamics(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
    id: &mut u64,
) {
    use engraver_proto::engraver::layout::tlayout::{
        DynamicsAlign, DynamicsParams, DynamicsPlacement, layout_dynamic,
    };
    let sp = input.spatium;
    let head_w = HEAD_W_SP * sp;
    let anchor_x = |a: &MeasureAnchors<'_>, tick: u32| -> f64 { direction_x(input, a, tick) };
    let ignore_dyn = |k: SkyKind, _| matches!(k, SkyKind::Dynamic | SkyKind::Hairpin);

    // Dynamics, each pushed clear of the skyline (not of each other).
    let mut dynamics: Vec<DynamicItem> = Vec::new();
    // Hairpins as (start x, end x, crescendo).
    let mut wedges: Vec<(f64, f64, bool)> = Vec::new();
    let mut wedge_open: Option<(f64, bool)> = None;
    for a in input.anchors {
        for d in &a.m.directions {
            match &d.kind {
                model::DirectionKind::Dynamic(name) => {
                    *id += 1;
                    let x = anchor_x(a, d.tick);
                    let (_, node) = layout_dynamic(
                        &DynamicsParams {
                            id: *id,
                            dynamic_type: dynamic_type(name),
                            custom_text: Some(name.clone()),
                            placement: DynamicsPlacement::Below,
                            align: DynamicsAlign::Center,
                            x,
                            note_width: head_w,
                            center_on_notehead: true,
                        },
                        &input.ctx.as_context(),
                    );
                    let Some(r0) = union(&ink(&node)) else {
                        continue;
                    };
                    let base = baseline_of(&node).unwrap_or(r0.y1);
                    // Default: baseline the style distance below the staff.
                    let mut dy = 4.0 * sp + style::DYNAMICS_POS_BELOW * sp - base;
                    let shape = [r0 + Vec2::new(0.0, dy)];
                    dy += sky.distance_to_move(
                        &shape,
                        false,
                        style::DYNAMICS_MIN_DISTANCE * sp,
                        style::SKYLINE_H_CLEARANCE * sp,
                        ignore_dyn,
                    );
                    let mut rect = r0 + Vec2::new(0.0, dy);
                    // Two dynamics on one spot (sf then p on one note): set
                    // the later one just right of the earlier.
                    let mut dx = 0.0;
                    for prev in &dynamics {
                        let moved = rect + Vec2::new(dx, 0.0);
                        if moved.overlaps(prev.rect.inflate(0.15 * sp, 0.0)) {
                            dx += prev.rect.x1 + 0.3 * sp - moved.x0;
                        }
                    }
                    rect = rect + Vec2::new(dx, 0.0);
                    let mut container =
                        SceneNode::group(SemanticId::new(ElementType::Dynamic, *id));
                    container.transform = Affine::translate((dx, dy));
                    container.add_child(node);
                    dynamics.push(DynamicItem {
                        node: container,
                        rect,
                        center: base + dy - 0.46 * sp,
                        tick_x: x + dx,
                        owner: owners.next(),
                    });
                }
                model::DirectionKind::WedgeStart { crescendo } => {
                    wedge_open = Some((anchor_x(a, d.tick), *crescendo));
                }
                model::DirectionKind::WedgeStop => {
                    if let Some((x1, cresc)) = wedge_open.take() {
                        wedges.push((x1, anchor_x(a, d.tick) + head_w, cresc));
                    }
                }
                _ => {}
            }
        }
    }
    if let Some((x1, cresc)) = wedge_open {
        wedges.push((x1, input.width, cresc));
    }

    // Hairpins: snapped between their dynamics, pushed clear, then each
    // chain (dynamic – hairpin – dynamic) aligned to its lowest member.
    let pad = style::HAIRPIN_DYNAMICS_DISTANCE * sp;
    let half_h = style::HAIRPIN_HEIGHT * sp / 2.0;
    for (x1, x2, cresc) in wedges {
        // Snap: start after a dynamic at the start, stop before one at the end.
        let before = dynamics
            .iter()
            .position(|d| (d.tick_x - x1).abs() < 0.5 * sp);
        let after = dynamics
            .iter()
            .position(|d| (d.tick_x - (x2 - head_w)).abs() < 0.5 * sp);
        let start = before.map_or(x1, |i| dynamics[i].rect.x1 + pad);
        let end = after
            .map_or(x2, |i| dynamics[i].rect.x0 - pad)
            .min(input.width);
        if end - start < 1.5 * sp {
            continue;
        }
        let mut center = 4.0 * sp + style::DYNAMICS_POS_BELOW * sp - 0.46 * sp;
        let shape = [Rect::new(start, center - half_h, end, center + half_h)];
        center += sky.distance_to_move(
            &shape,
            false,
            style::HAIRPIN_MIN_DISTANCE * sp,
            0.0,
            ignore_dyn,
        );
        // Chain alignment: everyone to the outermost (lowest) centre.
        let chain: Vec<usize> = before.into_iter().chain(after).collect();
        let target = chain
            .iter()
            .map(|i| dynamics[*i].center)
            .fold(center, f64::max);
        for i in &chain {
            let dy = target - dynamics[*i].center;
            if dy.abs() > 1e-6 {
                let dyn_item = &mut dynamics[*i];
                dyn_item.node.transform = Affine::translate((0.0, dy)) * dyn_item.node.transform;
                dyn_item.rect = dyn_item.rect + Vec2::new(0.0, dy);
                dyn_item.center += dy;
            }
        }
        let center = target;
        let open = |x: f64| -> f64 {
            let t = ((x - start) / (end - start)).clamp(0.0, 1.0);
            if cresc {
                t * half_h
            } else {
                (1.0 - t) * half_h
            }
        };
        let width = style::HAIRPIN_LINE_WIDTH * sp;
        let mut cmds = Vec::new();
        for s in [-1.0, 1.0] {
            cmds.push(PaintCommand::Line {
                start: Point::new(start, center + s * open(start)),
                end: Point::new(end, center + s * open(end)),
                width,
                color: Color::BLACK,
                line_cap: Default::default(),
            });
        }
        let owner = owners.next();
        sky.add(
            Rect::new(start, center - half_h, end, center + half_h),
            SkyKind::Hairpin,
            owner,
        );
        let mut node = SceneNode::group(SemanticId::new(ElementType::Hairpin, owner));
        node.add_commands(cmds);
        nodes.push(node);
    }
    for d in dynamics {
        sky.add(d.rect, SkyKind::Dynamic, d.owner);
        nodes.push(d.node);
    }
}

/// Where a direction at `tick` sits: on this staff's note there, or — on a
/// grand staff — on another staff's note when only it has one (the harp's
/// left hand under a right-hand rest); otherwise interpolated here.
fn direction_x(input: &SystemInput<'_, '_>, a: &MeasureAnchors<'_>, tick: u32) -> f64 {
    if a.m.entry_ticks.contains(&tick) {
        return a.x_at(tick);
    }
    for staff in &input.others {
        if let Some(o) = staff.iter().find(|o| o.m.index == a.m.index)
            && o.m.entry_ticks.contains(&tick)
        {
            return o.x_at(tick);
        }
    }
    a.x_at(tick)
}

/// Which face a character of staff text is set in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RunFace {
    Text,
    /// A SMuFL symbol meant to sit in text (a metronome note): Leland
    /// Text at the text's size.
    MusicText,
    /// A SMuFL symbol drawn at staff scale (a harp pedal diagram): Leland.
    Music,
}

fn run_face(c: char) -> RunFace {
    match c {
        '\u{E680}'..='\u{E68F}' => RunFace::Music,
        '\u{E000}'..='\u{F8FF}' => RunFace::MusicText,
        _ => RunFace::Text,
    }
}

/// One line of staff text, which may mix text with SMuFL symbols, laid out
/// left to right from the origin (baseline y = 0). Returns its commands and
/// its ink.
fn rich_line(
    line: &str,
    font: &TextFont,
    sp: f64,
    color: Color,
) -> (Vec<PaintCommand>, Option<Rect>) {
    // Runs of one face; a tuplet's bracket marks split runs and are carried
    // as `Mark`s (drawn as a bracket over the notes they enclose).
    enum Piece {
        Run(RunFace, String),
        Mark(char),
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for c in line.chars() {
        if matches!(c, '\u{E1FE}' | '\u{E1FF}' | '\u{E200}') {
            pieces.push(Piece::Mark(c));
            continue;
        }
        let face = run_face(c);
        match pieces.last_mut() {
            Some(Piece::Run(f, s)) if *f == face => s.push(c),
            _ => pieces.push(Piece::Run(face, c.to_string())),
        }
    }
    let mut x = 0.0;
    let mut cmds = Vec::new();
    let mut ink: Option<Rect> = None;
    let space = 0.28 * font.size;
    // Open tuplet: (index of its first run's ink, number shown).
    let mut tuplet: Option<(usize, Option<&str>)> = None;
    let mut run_inks: Vec<Rect> = Vec::new();
    for piece in pieces {
        let (face, text) = match piece {
            Piece::Run(face, text) => (face, text),
            Piece::Mark('\u{E1FE}') => {
                // Room for the bracket's hook before its first note.
                x += 0.25 * font.size;
                tuplet = Some((run_inks.len(), None));
                continue;
            }
            Piece::Mark('\u{E1FF}') => {
                if let Some((_, n)) = &mut tuplet {
                    *n = Some("3");
                }
                continue;
            }
            Piece::Mark(_) => {
                if let Some((first, number)) = tuplet.take()
                    && let Some(span) = run_inks[first..].iter().copied().reduce(|a, b| a.union(b))
                {
                    let (bracket, b_ink) = tuplet_bracket(span, number, font, color);
                    cmds.extend(bracket);
                    ink = Some(ink.map_or(b_ink, |i| i.union(b_ink)));
                    x = x.max(b_ink.x1 + 0.12 * font.size);
                }
                continue;
            }
        };
        let (family, size, bold, italic, dy) = match face {
            RunFace::Text => (font.family.as_str(), font.size, font.bold, font.italic, 0.0),
            RunFace::MusicText => ("Leland Text", font.size, false, false, 0.0),
            RunFace::Music => ("Leland", 4.0 * sp, false, false, 0.0),
        };
        let lead = text.len() - text.trim_start().len();
        if lead > 0 && x > 0.0 {
            x += space;
        }
        let shown = text.trim();
        if shown.is_empty() {
            // Notes under a tuplet bracket stand further apart, as the
            // bracket's number sits between them.
            x += if tuplet.is_some() { 1.8 * space } else { space };
            continue;
        }
        let r = metrics::text_ink(
            family,
            size,
            bold,
            italic,
            shown,
            Point::new(x, dy),
            TextAnchor::Start,
        )
        .unwrap_or(Rect::new(
            x,
            dy - 0.7 * size,
            x + 0.5 * size * shown.chars().count() as f64,
            dy,
        ));
        ink = Some(ink.map_or(r, |i| i.union(r)));
        run_inks.push(r);
        cmds.push(marker_text(
            shown,
            size,
            Point::new(x, dy),
            color,
            TextAnchor::Start,
            family,
            bold,
            italic,
        ));
        x = r.x1
            + if text.ends_with(' ') {
                space
            } else {
                0.12 * font.size
            };
    }
    (cmds, ink)
}

/// A tuplet bracket over `span` (the enclosed notes' ink): a line clear of
/// their stems with hooks down at each end, broken by the number.
fn tuplet_bracket(
    span: Rect,
    number: Option<&str>,
    font: &TextFont,
    color: Color,
) -> (Vec<PaintCommand>, Rect) {
    let size = font.size;
    let y = span.y0 - 0.3 * size;
    let hook = 0.3 * size;
    let width = 0.06 * size;
    let (x0, x1) = (span.x0 - 0.12 * size, span.x1 + 0.12 * size);
    let mid = (x0 + x1) / 2.0;
    let num_size = 0.7 * size;
    let mut cmds = Vec::new();
    let mut gap = (mid, mid);
    if let Some(n) = number {
        let r = metrics::text_ink(
            &font.family,
            num_size,
            false,
            true,
            n,
            Point::ZERO,
            TextAnchor::Middle,
        )
        .unwrap_or(Rect::new(
            -0.25 * num_size,
            -0.7 * num_size,
            0.25 * num_size,
            0.0,
        ));
        let base = y - (r.y0 + r.y1) / 2.0;
        gap = (mid + r.x0 - 0.15 * size, mid + r.x1 + 0.15 * size);
        cmds.push(marker_text(
            n,
            num_size,
            Point::new(mid, base),
            color,
            TextAnchor::Middle,
            &font.family,
            false,
            true,
        ));
    }
    let line = |a: Point, b: Point| PaintCommand::line(a, b, color, width);
    cmds.push(line(Point::new(x0, y + hook), Point::new(x0, y)));
    cmds.push(line(Point::new(x0, y), Point::new(gap.0, y)));
    cmds.push(line(Point::new(gap.1, y), Point::new(x1, y)));
    cmds.push(line(Point::new(x1, y), Point::new(x1, y + hook)));
    let ink = Rect::new(x0, y - 0.5 * num_size, x1, y + hook);
    (cmds, ink)
}

// ───────────────────────── 5a. tempo marks ─────────────────────────

/// SMuFL metronome note for a MusicXML beat unit (`metNote…Up`).
fn metronome_note(beat_unit: &str) -> char {
    match beat_unit {
        "whole" => '\u{ECA2}',
        "half" => '\u{ECA3}',
        "eighth" => '\u{ECA7}',
        "16th" => '\u{ECA9}',
        _ => '\u{ECA5}',
    }
}

/// Tempo marks — `Allegro ♩ = 120`, `♩ = 63 BPM` — bold, black (they are
/// the conductor's, not a colour family), above the staff where staff text
/// goes; the note is Leland Text's metronome glyph at the text's size.
fn place_tempos(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) {
    let sp = input.spatium;
    let size = 12.0;
    let mut marks: Vec<(f64, &model::TempoMark)> = Vec::new();
    for a in input.anchors {
        for d in &a.m.directions {
            if let model::DirectionKind::Tempo(t) = &d.kind {
                marks.push((direction_x(input, a, d.tick), t));
            }
        }
    }
    for item in input.items.iter().filter(|i| !i.notated) {
        for d in item.directions {
            if let model::DirectionKind::Tempo(t) = &d.kind {
                marks.push((item.span.0 + 0.5 * sp, t));
            }
        }
    }
    for (x, t) in marks {
        // (text, family, bold) pieces, set left to right.
        let mut pieces: Vec<(String, &str, bool)> = Vec::new();
        if let Some(text) = &t.text {
            pieces.push((text.clone(), input.text_family, true));
        }
        let mut note = metronome_note(&t.beat_unit).to_string();
        for _ in 0..t.dots {
            note.push('\u{ECB7}'); // metAugmentationDot
        }
        pieces.push((note, "Leland Text", false));
        pieces.push((format!("= {}", t.per_minute), input.text_family, true));
        if let Some(suffix) = &t.suffix {
            pieces.push((suffix.clone(), input.text_family, true));
        }
        let mut cursor = 0.0;
        let mut cmds = Vec::new();
        let mut ink: Option<Rect> = None;
        for (text, family, bold) in &pieces {
            let r = metrics::text_ink(
                family,
                size,
                *bold,
                false,
                text,
                Point::new(cursor, 0.0),
                TextAnchor::Start,
            )
            .unwrap_or(Rect::new(
                cursor,
                -0.7 * size,
                cursor + 0.5 * size * text.len() as f64,
                0.0,
            ));
            ink = Some(ink.map_or(r, |i| i.union(r)));
            cmds.push(marker_text(
                text,
                size,
                Point::new(cursor, 0.0),
                Color::BLACK,
                TextAnchor::Start,
                family,
                *bold,
                false,
            ));
            cursor = r.x1 + 0.28 * size;
        }
        let Some(ink) = ink else {
            continue;
        };
        let x = x.min((input.width - ink.width()).max(0.0));
        let base = -style::STAFF_TEXT_POS_ABOVE * sp;
        let at = |dy: f64| ink + Vec2::new(x, dy);
        let dy = base
            + sky.distance_to_move(
                &[at(base)],
                true,
                style::STAFF_TEXT_MIN_DISTANCE * sp,
                style::SKYLINE_H_CLEARANCE * sp,
                |_, _| false,
            );
        let owner = owners.next();
        sky.add(at(dy), SkyKind::StaffText, owner);
        let mut node = SceneNode::group(SemanticId::new(ElementType::Tempo, owner));
        node.add_child(SceneNode::anonymous_leaf(cmds).with_transform(Affine::translate((x, dy))));
        nodes.push(node);
    }
}

// ───────────────────────── 5. staff text ─────────────────────────

fn place_staff_text(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) {
    let sp = input.spatium;
    let font = input.words_font;
    // Text for the player is red, as on the master rhythm chart (where
    // staff text, dynamics and repeats share one red); black for a mono
    // printer and in Finale's own look.
    let text_color = instruction_color(input.marks);
    let mut words: Vec<(f64, &str)> = Vec::new();
    for a in input.anchors {
        for d in &a.m.directions {
            if let model::DirectionKind::Words(w) = &d.kind
                && !is_section_text(input, w)
            {
                words.push((direction_x(input, a, d.tick), w));
            }
        }
    }
    // Staff text on rests/multirests (tempo changes, cues).
    for item in input.items.iter().filter(|i| !i.notated) {
        for d in item.directions {
            if let model::DirectionKind::Words(w) = &d.kind
                && !is_section_text(input, w)
            {
                words.push((item.span.0 + 0.5 * sp, w));
            }
        }
    }
    for (x, w) in words {
        let cleaned = crate::harp::clean_pedal_text(w);
        let w = cleaned.as_str();
        // A staff text the song's decisions call a remark ("Band in" typed
        // as staff text) is drawn as one, in the staff text's place.
        if input.marks != MarkStyle::Classic
            && input.roles.get(&section_marks::text_key(w)) == Some(&TextRole::Remark)
        {
            let piece = callout_piece(w, &section_marks::text_key(w), x, sp, input.marks);
            let x = x.min((input.width - piece.frame.width()).max(0.0)) - x;
            let base = -style::STAFF_TEXT_POS_ABOVE * sp;
            let at = |dy: f64| piece.frame + Vec2::new(x, dy);
            let dy = base
                + sky.distance_to_move(
                    &[at(base)],
                    true,
                    style::STAFF_TEXT_MIN_DISTANCE * sp,
                    style::SKYLINE_H_CLEARANCE * sp,
                    |_, _| false,
                );
            let owner = owners.next();
            sky.add(at(dy), SkyKind::StaffText, owner);
            let mut node = SceneNode::group(SemanticId::new(ElementType::Text, owner));
            node.add_child(
                SceneNode::anonymous_leaf(piece.cmds).with_transform(Affine::translate((x, dy))),
            );
            nodes.push(node);
            continue;
        }
        // A cue may hold several lines ("Sul ponticello" over "Start trill
        // slowly…"): stack them, last line on the baseline.
        let lines: Vec<&str> = w.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let leading = 1.15 * font.size;
        let n = lines.len();
        // (line offset, its commands at the origin, ink). Lines stack up
        // from the last one's baseline, each clear of the one below by its
        // real ink (a pedal diagram is taller than a line of text), never
        // closer than the text's leading.
        let _ = n;
        let mut block: Vec<(f64, Vec<PaintCommand>, Rect)> = Vec::new();
        let mut below: Option<(f64, Rect)> = None; // (offset, ink at offset)
        for line in lines.iter().rev() {
            let (cmds, ink) = rich_line(line, font, sp, text_color);
            let Some(r) = ink else {
                continue;
            };
            let off = match below {
                None => 0.0,
                Some((off_below, ink_below)) => {
                    (off_below - leading).min(ink_below.y0 - 0.25 * font.size - r.y1)
                }
            };
            below = Some((off, r + Vec2::new(0.0, off)));
            block.push((off, cmds, r + Vec2::new(0.0, off)));
        }
        let Some(extent) = block.iter().map(|(_, _, r)| *r).reduce(|a, b| a.union(b)) else {
            continue;
        };
        // Keep long strings on the page.
        let x = x.min((input.width - extent.width()).max(0.0));
        let mut base = -style::STAFF_TEXT_POS_ABOVE * sp;
        let shape: Vec<Rect> = block
            .iter()
            .map(|(_, _, r)| *r + Vec2::new(x, base))
            .collect();
        base += sky.distance_to_move(
            &shape,
            true,
            style::STAFF_TEXT_MIN_DISTANCE * sp,
            style::SKYLINE_H_CLEARANCE * sp,
            |_, _| false,
        );
        let owner = owners.next();
        let mut node = SceneNode::group(SemanticId::new(ElementType::Text, owner));
        for (off, cmds, r) in block {
            sky.add(r + Vec2::new(x, base), SkyKind::StaffText, owner);
            node.add_child(
                SceneNode::anonymous_leaf(cmds).with_transform(Affine::translate((x, base + off))),
            );
        }
        nodes.push(node);
    }
}

// ───────────────────────── 6. measure numbers ─────────────────────────

fn place_measure_numbers(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) {
    let sp = input.spatium;
    for n in &input.numbers {
        let Some(r) = metrics::text_ink(
            &n.font.family,
            n.font.size,
            n.font.bold,
            n.font.italic,
            &n.text,
            n.pos,
            n.anchor,
        ) else {
            continue;
        };
        let above = r.y1 < 2.0 * sp;
        let dy = sky.distance_to_move(
            &[r],
            above,
            style::MEASURE_NUMBER_MIN_DISTANCE * sp,
            0.0,
            |_, _| false,
        );
        let owner = owners.next();
        sky.add(r + Vec2::new(0.0, dy), SkyKind::MeasureNumber, owner);
        let mut node = SceneNode::group(SemanticId::new(ElementType::MeasureNumber, owner));
        node.add_child(styled_text_in(
            &n.text,
            &n.font,
            Point::new(n.pos.x, n.pos.y + dy),
            n.anchor,
            n.color,
        ));
        nodes.push(node);
    }
}

// ───────────────────────── 7. rehearsal marks ─────────────────────────

/// Keyflow marker geometry, in spatiums.
mod marker {
    /// Narrowest margin (points) a badge is drawn in; below this the mark
    /// becomes a pill over the clef.
    pub const BADGE_MIN_ROOM: f64 = 20.0;
    /// The chart badge's type size, as a fraction of the staff height
    /// (`layout_margin_label`'s base size).
    pub const BADGE_TEXT: f64 = 0.7;
    /// Mid-line badges are the margin badge a step smaller.
    pub const MIDLINE_SCALE: f64 = 0.82;
    /// Remark text size.
    pub const CALLOUT_SIZE: f64 = 1.45;
}

/// The master rhythm chart's colours for a section (`get_section_theme`),
/// or solid black with white type for a black-and-white printer. Letters,
/// bar numbers and unrecognised sections take the chart's custom style.
fn section_theme(kind: Option<&SectionType>, look: MarkStyle) -> RehearsalMarkStyle {
    match (look, kind) {
        (MarkStyle::Mono, _) => RehearsalMarkStyle {
            background_color: Color::BLACK,
            text_color: Color::WHITE,
            border_color: None,
            ..Default::default()
        },
        (_, Some(kind)) => get_section_theme(kind),
        (_, None) => rehearsal_themes::custom(),
    }
}

/// The sans the markers are set in (FreeSans Bold, see `fonts`).
const MARKER_FAMILY: &str = "FreeSans";

fn marker_text(
    text: &str,
    size: f64,
    at: Point,
    color: Color,
    anchor: TextAnchor,
    family: &str,
    bold: bool,
    italic: bool,
) -> PaintCommand {
    PaintCommand::Text {
        text: text.to_string(),
        font_family: family.to_string(),
        font_size: size,
        position: at,
        color,
        anchor,
        weight: if bold {
            FontWeight::Bold
        } else {
            FontWeight::Normal
        },
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }
}

/// Baseline that centres `text`'s ink on `center_y`, and its ink width.
fn centred_baseline(text: &str, size: f64, center_y: f64, bold: bool) -> (f64, f64) {
    metrics::text_ink(
        MARKER_FAMILY,
        size,
        bold,
        false,
        text,
        Point::ZERO,
        TextAnchor::Start,
    )
    .map_or(
        (center_y + 0.35 * size, 0.6 * size * text.len() as f64),
        |r| (center_y - (r.y0 + r.y1) / 2.0, r.width()),
    )
}

/// One piece of a row of marks above the staff, in row coordinates
/// (y = 0 at the row's bottom).
struct RowPiece {
    frame: Rect,
    cmds: Vec<PaintCommand>,
}

/// A section's note (`Sax`, `midway`) beside its marker, centred on the
/// marker's middle.
fn note_piece(
    note: &str,
    x: f64,
    center_y: f64,
    size: f64,
    color: Color,
    family: &str,
) -> RowPiece {
    let r = metrics::text_ink(
        family,
        size,
        true,
        true,
        note,
        Point::ZERO,
        TextAnchor::Start,
    )
    .unwrap_or(Rect::new(
        0.0,
        -0.7 * size,
        0.5 * size * note.len() as f64,
        0.0,
    ));
    let base = center_y - (r.y0 + r.y1) / 2.0;
    RowPiece {
        frame: r + Vec2::new(x, base),
        cmds: vec![marker_text(
            note,
            size,
            Point::new(x, base),
            color,
            TextAnchor::Start,
            family,
            true,
            true,
        )],
    }
}

fn place_rehearsals(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) {
    if input.marks == MarkStyle::Classic {
        place_rehearsals_classic(input, sky, owners, nodes);
        return;
    }
    let sp = input.spatium;
    let look = input.marks;
    // The margin badge needs a printable strip left of the staff; the
    // chart's badge sizes its text to whatever width it gets.
    let badge_room = (input.margin_room >= marker::BADGE_MIN_ROOM).then_some(input.margin_room);
    for item in &input.items {
        // Rehearsal marks, plus staff text the song's decisions call a
        // section (some arrangers type `VS 1` as plain text).
        let marks: Vec<RehearsalMark> = item
            .directions
            .iter()
            .filter_map(|d| match &d.kind {
                model::DirectionKind::Rehearsal(r) => Some(r.clone()),
                model::DirectionKind::Words(w) if is_section_text(input, w) => {
                    Some(RehearsalMark::boxed(w.clone()))
                }
                _ => None,
            })
            .collect();
        if marks.is_empty() {
            continue;
        }
        let owner = owners.next();
        let mut group = SceneNode::group(SemanticId::new(ElementType::RehearsalMark, owner));
        let mut row: Vec<RowPiece> = Vec::new();
        // Left edge of the next piece in the row; `None` until the first
        // marker over a mid-line barline centres itself on it.
        let mut cursor: Option<f64> = item.first.then_some(0.0);
        let mut badge_used = false;
        let role_of = |mark: &RehearsalMark| {
            input
                .roles
                .get(&section_marks::text_key(&mark.text))
                .copied()
                .unwrap_or_else(|| TextRole::default_for_rehearsal(&mark.text))
        };
        let is_letter = |mark: &RehearsalMark| {
            role_of(mark) == TextRole::Section
                && matches!(section_marks::classify(&mark.text), MarkClass::Letter(_))
        };
        // A rehearsal letter or bar number at the same bar as a section
        // (`A` beside `VS 1`) rides on the section's badge instead of
        // getting one of its own.
        let has_section = marks
            .iter()
            .any(|m| role_of(m) == TextRole::Section && !is_letter(m));
        let mut letters: Vec<String> = if has_section {
            marks
                .iter()
                .filter(|m| is_letter(m))
                .map(|m| section_marks::text_key(&m.text))
                .collect()
        } else {
            Vec::new()
        };
        for mark in &marks {
            if has_section && is_letter(mark) {
                continue;
            }
            let key = section_marks::text_key(&mark.text);
            let role = role_of(mark);
            let x = cursor.unwrap_or(item.span.0 + 0.5 * sp);
            let (label, note, kind) = match (role, section_marks::classify(&mark.text)) {
                (TextRole::Remark, _) => {
                    let piece = callout_piece(&mark.text, &key, x, sp, look);
                    cursor = Some(piece.frame.x1 + 0.6 * sp);
                    row.push(piece);
                    continue;
                }
                (TextRole::Instruction, _) => {
                    let piece = instruction_piece(&mark.text, x, input.words_font, sp, look);
                    cursor = Some(piece.frame.x1 + 0.6 * sp);
                    row.push(piece);
                    continue;
                }
                (TextRole::Section, MarkClass::Section(s)) => (s.label, s.note, Some(s.kind)),
                (TextRole::Section, MarkClass::Letter(l)) => (l, None, None),
                // A section the vocabulary does not know ("Freedom" #1 in a
                // medley): a neutral badge with its own name.
                (TextRole::Section, MarkClass::Callout(_)) => (key.clone(), None, None),
            };
            // The first section takes the bar's letters.
            let letters = std::mem::take(&mut letters);
            let theme = section_theme(kind.as_ref(), look);
            // A section that opens a system: the master rhythm chart's own
            // margin badge, drawn by the chart's layout function so the two
            // can never drift apart. It spans the margin from the printable
            // edge to the staff, top-aligned with the staff, and carries
            // the section's note (`Sax`, `midway`) beneath it in caps.
            if item.first
                && !badge_used
                && let Some(room) = badge_room
            {
                badge_used = true;
                // In the margin the letter goes on its own line, as the
                // chart sets its section letters (`CH 1` over `B`).
                let label = if letters.is_empty() {
                    label
                } else {
                    format!("{label}\n{}", letters.join(" "))
                };
                let params = MarginLabelParams {
                    label_override: Some(label),
                    page_x: -room - 4.0,
                    margin_width: room + 4.0 - input.badge_inset,
                    staff_y: 0.0,
                    staff_height: 4.0 * sp,
                    style: theme,
                    ..Default::default()
                };
                let (capsule, mut badge) = layout_margin_label(&params, &input.ctx.as_context());
                // The chart names generic families (`sans-serif`, its
                // `section-comment` style), which a part's PDF font
                // database does not resolve: name the face.
                for cmd in &mut badge.commands {
                    if let PaintCommand::Text { font_family, .. } = cmd
                        && matches!(font_family.as_str(), "sans-serif" | "section-comment")
                    {
                        *font_family = MARKER_FAMILY.to_string();
                    }
                }
                // The note goes beneath in caps, as the chart's comment
                // does — but a part's margin is narrower than a chart's, so
                // it shrinks to the badge's width instead of running into
                // the clef ("ORCHESTRA" from "Orchestra Bridge").
                if let Some(note) = note {
                    let note = note.to_uppercase();
                    let full = 0.75 * marker::BADGE_TEXT * 4.0 * sp;
                    let (_, w1) = centred_baseline(&note, 1.0, 0.0, false);
                    let size = full.min(capsule.bounds.width() / w1.max(1e-6));
                    let base = capsule.bounds.y1 + 3.0 + 0.75 * size;
                    badge.add_command(marker_text(
                        &note,
                        size,
                        Point::new(capsule.bounds.center().x, base),
                        Color::BLACK,
                        TextAnchor::Middle,
                        MARKER_FAMILY,
                        false,
                        false,
                    ));
                }
                group.add_child(badge);
                continue;
            }
            // Otherwise the margin badge's smaller sibling over the barline
            // mid-line (at the row's cursor when several marks share a
            // measure): the chart's capsule — same face, weight and corner
            // — with its type a step down, so a player finds sections by the
            // same shape wherever they start.
            let label = if letters.is_empty() {
                label
            } else {
                format!("{label} · {}", letters.join(" · "))
            };
            let size = marker::MIDLINE_SCALE * marker::BADGE_TEXT * 4.0 * sp;
            let pad_v = marker::MIDLINE_SCALE * 3.0;
            let pill_h = size + 2.0 * pad_v;
            let (base, w) = centred_baseline(&label, size, -pill_h / 2.0, false);
            let width = (w + 1.1 * size).max(pill_h * 1.5);
            let x0 = cursor.unwrap_or(item.span.0 - width / 2.0);
            let frame = Rect::new(x0, -pill_h, x0 + width, 0.0);
            row.push(RowPiece {
                frame,
                cmds: vec![
                    PaintCommand::Rect {
                        rect: frame,
                        fill: Some(theme.background_color),
                        stroke: theme.border_color,
                        stroke_width: theme.border_width.min(0.12 * sp),
                        // The chart capsule's single-line radius.
                        corner_radius: Some(pill_h / 4.0),
                    },
                    marker_text(
                        &label,
                        size,
                        Point::new(frame.center().x, base),
                        theme.text_color,
                        TextAnchor::Middle,
                        MARKER_FAMILY,
                        false,
                        false,
                    ),
                ],
            });
            cursor = Some(frame.x1 + 0.4 * sp);
            if let Some(note) = note {
                let piece = note_piece(
                    &note,
                    frame.x1 + 0.4 * sp,
                    -pill_h / 2.0,
                    0.8 * size,
                    Color::BLACK,
                    input.text_family,
                );
                cursor = Some(piece.frame.x1 + 0.6 * sp);
                row.push(piece);
            }
        }
        if !row.is_empty() {
            // The row sits just above the staff, then clears the skyline.
            let base_dy = -style::REHEARSAL_POS_ABOVE * sp;
            let shape: Vec<Rect> = row
                .iter()
                .map(|p| p.frame + Vec2::new(0.0, base_dy))
                .collect();
            let dy = base_dy
                + sky.distance_to_move(
                    &shape,
                    true,
                    style::REHEARSAL_MIN_DISTANCE * sp,
                    style::SKYLINE_H_CLEARANCE * sp,
                    |_, _| false,
                );
            let mut leaf = Vec::new();
            for p in row {
                sky.add(p.frame + Vec2::new(0.0, dy), SkyKind::Rehearsal, owner);
                leaf.extend(p.cmds);
            }
            group.add_child(
                SceneNode::anonymous_leaf(leaf).with_transform(Affine::translate((0.0, dy))),
            );
        }
        nodes.push(group);
    }
}

/// Staff text the song's decisions make a section badge.
fn is_section_text(input: &SystemInput<'_, '_>, text: &str) -> bool {
    input.marks != MarkStyle::Classic
        && input.roles.get(&section_marks::text_key(text)) == Some(&TextRole::Section)
}

/// The player's own instruction, written where a cue would sit: the staff
/// text face in the chart's red (black for mono), lines kept.
fn instruction_piece(raw: &str, x: f64, font: &TextFont, sp: f64, look: MarkStyle) -> RowPiece {
    let color = instruction_color(look);
    // As written: the player's text keeps its own punctuation, like staff
    // text does; music symbols in it are set in the music font.
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let leading = 1.15 * font.size;
    let n = lines.len();
    let mut frame: Option<Rect> = None;
    let mut cmds = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        let base = -((n - 1 - k) as f64) * leading;
        let (line_cmds, ink) = rich_line(line, font, sp, color);
        let Some(r) = ink else {
            continue;
        };
        let shift = Affine::translate((x, base));
        frame = Some(frame.map_or(shift.transform_rect_bbox(r), |f| {
            f.union(shift.transform_rect_bbox(r))
        }));
        cmds.extend(line_cmds.into_iter().map(|c| translate_command(c, x, base)));
    }
    let frame = frame.unwrap_or(Rect::new(x, -font.size, x, 0.0));
    // Bottom of the row at y = 0.
    let shift = -frame.y1.max(0.0);
    RowPiece {
        frame: frame + Vec2::new(0.0, shift),
        cmds: cmds
            .into_iter()
            .map(|c| translate_command(c, 0.0, shift))
            .collect(),
    }
}

/// A paint command moved by (dx, dy) (text and lines, which is what
/// `rich_line` draws).
fn translate_command(cmd: PaintCommand, dx: f64, dy: f64) -> PaintCommand {
    let d = Vec2::new(dx, dy);
    match cmd {
        PaintCommand::Text {
            text,
            font_family,
            font_size,
            position,
            color,
            anchor,
            weight,
            style,
        } => PaintCommand::Text {
            text,
            font_family,
            font_size,
            position: position + d,
            color,
            anchor,
            weight,
            style,
        },
        PaintCommand::Line {
            start,
            end,
            width,
            color,
            line_cap,
        } => PaintCommand::Line {
            start: start + d,
            end: end + d,
            width,
            color,
            line_cap,
        },
        other => other,
    }
}

/// Red for the player's text on the colour page, as the master chart has it.
fn instruction_color(look: MarkStyle) -> Color {
    if look == MarkStyle::Color {
        REPEAT_COLOR
    } else {
        Color::BLACK
    }
}

/// A remark — what the rest of the band is doing ("Perc. Loop", "Band
/// Enters"), not an instruction to this player. Set quieter than anything
/// the player acts on: regular-weight sans on a pale tint with a thin
/// accent bar, Finale's `*cue:` / `[ cue ]` decoration stripped.
fn callout_piece(raw: &str, flat: &str, x: f64, sp: f64, style: MarkStyle) -> RowPiece {
    let paint = section_marks::callout_paint(style);
    let size = marker::CALLOUT_SIZE * sp;
    let mut lines: Vec<String> = raw
        .lines()
        .map(section_marks::remark_text)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        lines.push(section_marks::remark_text(flat));
    }
    let leading = 1.22 * size;
    let n = lines.len();
    let bar_w = 0.2 * sp;
    let pad_l = bar_w + 0.45 * sp;
    let pad_r = 0.5 * sp;
    let pad_v = 0.35 * sp;
    let face = TextFont {
        family: MARKER_FAMILY.to_string(),
        size,
        bold: false,
        italic: false,
    };
    let mut ink: Option<Rect> = None;
    // Each line's commands, already at its place (music symbols in a
    // remark set in the music font).
    let mut texts: Vec<PaintCommand> = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        let base = -((n - 1 - k) as f64) * leading;
        let (line_cmds, line_ink) = rich_line(line, &face, sp, paint.text);
        let r = line_ink.map_or(
            Rect::new(x + pad_l, base - 0.7 * size, x + pad_l, base),
            |r| r + Vec2::new(x + pad_l, base),
        );
        ink = Some(ink.map_or(r, |i| i.union(r)));
        texts.extend(
            line_cmds
                .into_iter()
                .map(|c| translate_command(c, x + pad_l, base)),
        );
    }
    let ink = ink.unwrap_or_default();
    // Cap height to baseline, not the ink's descenders: rows of callouts
    // line up on their text, and a "g" does not make its box taller.
    let top = ink.y0 - pad_v;
    let bottom = 0.0f64.max(ink.y1) + pad_v;
    let frame = Rect::new(x, top, ink.x1 + pad_r, bottom);
    // Shift so the frame's bottom is the row's bottom (y = 0).
    let shift = -bottom;
    let frame = frame + Vec2::new(0.0, shift);
    let mut cmds = Vec::new();
    if let Some(tint) = paint.tint {
        cmds.push(PaintCommand::Rect {
            rect: frame,
            fill: Some(tint),
            stroke: None,
            stroke_width: 0.0,
            corner_radius: Some(0.25 * sp),
        });
    }
    cmds.push(PaintCommand::Rect {
        rect: Rect::new(frame.x0, frame.y0, frame.x0 + bar_w, frame.y1),
        fill: Some(paint.bar),
        stroke: None,
        stroke_width: 0.0,
        corner_radius: Some(0.5 * bar_w),
    });
    cmds.extend(texts.into_iter().map(|c| translate_command(c, 0.0, shift)));
    RowPiece { frame, cmds }
}

/// The source's own rehearsal boxes, as Finale printed them.
fn place_rehearsals_classic(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) {
    let sp = input.spatium;
    for item in &input.items {
        let marks: Vec<&RehearsalMark> = item
            .directions
            .iter()
            .filter_map(|d| match &d.kind {
                model::DirectionKind::Rehearsal(r) => Some(r),
                _ => None,
            })
            .collect();
        if marks.is_empty() {
            continue;
        }
        // A system's first cue sits at the staff's left end, over the clef;
        // later ones just right of their barline. Several in one measure
        // ("79" then "Sax Solo") side by side, all at one height.
        let mut cursor = if item.first { 0.0 } else { item.span.0 + 1.5 };
        // Each mark: its frame (baseline of its last line at 0), its face,
        // and its lines with their baselines.
        struct MarkBox<'m> {
            frame: Rect,
            mark: &'m RehearsalMark,
            family: String,
            lines: Vec<(String, f64, f64)>, // (text, x, baseline)
        }
        let mut boxes: Vec<MarkBox<'_>> = Vec::new();
        for mark in &marks {
            let size = mark.font_size.unwrap_or(12.0);
            // A sans face when the source asks for one (Finale's Arial
            // cues), otherwise the part's serif.
            let family = match mark.font_family.as_deref().map(str::to_lowercase) {
                Some(f) if ["arial", "helvetica", "sans"].iter().any(|s| f.contains(s)) => {
                    "FreeSans".to_string()
                }
                _ => input.text_family.to_string(),
            };
            let lines: Vec<&str> = mark
                .text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            let leading = 1.15 * size;
            let n = lines.len();
            let mut ink: Option<Rect> = None;
            let mut laid: Vec<(String, f64)> = Vec::new();
            for (k, line) in lines.iter().enumerate() {
                let base = -((n - 1 - k) as f64) * leading;
                if let Some(r) = metrics::text_ink(
                    &family,
                    size,
                    mark.bold,
                    mark.italic,
                    line,
                    Point::new(0.0, base),
                    TextAnchor::Start,
                ) {
                    ink = Some(ink.map_or(r, |i| i.union(r)));
                }
                laid.push((line.to_string(), base));
            }
            let Some(t) = ink else {
                continue;
            };
            let enclosed = mark.enclosure != Enclosure::None;
            let pad = if enclosed {
                0.3 * sp + 0.12 * size
            } else {
                0.0
            };
            let frame = Rect::new(
                cursor,
                t.y0 - pad,
                cursor + t.x1 + 2.0 * pad,
                t.y1.max(0.0) + pad,
            );
            boxes.push(MarkBox {
                frame,
                mark,
                family,
                lines: laid
                    .into_iter()
                    .map(|(l, b)| (l, cursor + pad, b))
                    .collect(),
            });
            cursor = frame.x1 + 0.5 * size;
        }
        // Default: frame bottoms just above the staff; then push clear.
        let Some(bottom) = boxes.iter().map(|b| b.frame.y1).reduce(f64::max) else {
            continue;
        };
        let mut dy = -style::REHEARSAL_POS_ABOVE * sp - bottom;
        let shape: Vec<Rect> = boxes.iter().map(|b| b.frame + Vec2::new(0.0, dy)).collect();
        dy += sky.distance_to_move(
            &shape,
            true,
            style::REHEARSAL_MIN_DISTANCE * sp,
            style::SKYLINE_H_CLEARANCE * sp,
            |_, _| false,
        );
        let owner = owners.next();
        let mut node = SceneNode::group(SemanticId::new(ElementType::RehearsalMark, owner));
        for b in boxes {
            let frame = b.frame + Vec2::new(0.0, dy);
            sky.add(frame, SkyKind::Rehearsal, owner);
            let size = b.mark.font_size.unwrap_or(12.0);
            let mut cmds = Vec::new();
            if b.mark.enclosure != Enclosure::None {
                cmds.push(PaintCommand::Rect {
                    rect: frame,
                    fill: None,
                    stroke: Some(Color::BLACK),
                    stroke_width: 0.16 * sp,
                    corner_radius: (b.mark.enclosure == Enclosure::Oval)
                        .then_some(frame.height() / 2.0),
                });
            }
            for (text, x, base) in b.lines {
                cmds.push(PaintCommand::Text {
                    text,
                    font_family: b.family.clone(),
                    font_size: size,
                    position: Point::new(x, base + dy),
                    color: Color::BLACK,
                    anchor: TextAnchor::Start,
                    weight: if b.mark.bold {
                        FontWeight::Bold
                    } else {
                        FontWeight::Normal
                    },
                    style: if b.mark.italic {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    },
                });
            }
            node.add_child(SceneNode::anonymous_leaf(cmds));
        }
        nodes.push(node);
    }
}
