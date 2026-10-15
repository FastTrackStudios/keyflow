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

use super::{MeasureAnchors, TextFont, articulation_type, dynamic_type, styled_text};
use crate::metrics;
use crate::model::{self, Enclosure, RehearsalMark};
use crate::skyline::{SkyKind, Skyline};

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
}

pub(super) struct SystemInput<'a, 'm> {
    pub spatium: f64,
    /// Staff length (prefix + measures).
    pub width: f64,
    pub anchors: &'a [MeasureAnchors<'m>],
    pub items: Vec<ItemMarks<'a>>,
    pub numbers: Vec<NumberMark>,
    pub text_family: &'a str,
    pub words_font: &'a TextFont,
    pub ctx: &'a LayoutContextOwned,
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
    obstacles.extend(artic.iter().copied());
    place_slurs(input, &obstacles, &mut sky, &mut owners, &mut out.nodes, id);
    place_dynamics(input, &mut sky, &mut owners, &mut out.nodes, id);
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
    /// The stem direction the measure builder actually drew for entry `i`.
    fn stem_up_at(&self, i: usize, primary_line: i32) -> bool {
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

struct PlacedTie {
    rects: Vec<Rect>,
    /// For leave-space-for-tie: the entry it starts or ends on, and side.
    above: bool,
    entries: Vec<(usize, usize)>, // (anchor index, entry index)
}

fn place_ties(
    input: &SystemInput<'_, '_>,
    obstacles: &[Rect],
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
    id: &mut u64,
) -> Vec<PlacedTie> {
    let sp = input.spatium;
    let head_w = HEAD_W_SP * sp;
    let mut out = Vec::new();

    // Which way a tie on `line` of a chord bows: a single note opposite its
    // stem; in a chord the upper half up, the lower half down.
    let direction = |line: i32, chord: &[i32], up_stem: bool| -> bool {
        if chord.len() <= 1 {
            return !up_stem;
        }
        let mut sorted = chord.to_vec();
        sorted.sort_unstable();
        let rank = sorted.iter().position(|l| *l == line).unwrap_or(0);
        rank * 2 >= sorted.len()
    };

    // (line, start x of the head, above, (anchor, entry))
    let mut pending: Vec<(i32, f64, bool, (usize, usize))> = Vec::new();
    let emit = |line: i32,
                x_head1: Option<f64>,
                x_head2: Option<f64>,
                above: bool,
                entries: Vec<(usize, usize)>,
                sky: &mut Skyline,
                nodes: &mut Vec<SceneNode>,
                out: &mut Vec<PlacedTie>,
                id: &mut u64| {
        let sign = if above { -1.0 } else { 1.0 };
        let y_note = y_of(line, sp);
        // Outside placement: endpoints just beyond the heads, x inset from
        // the head centres (computeDefaultStartOrEndPoint).
        let mut y_end = y_note + sign * 0.7 * sp;
        // adjustY: keep the tips off a staff line.
        let rel = y_end / (0.5 * sp);
        let nearest = rel.round() * 0.5 * sp;
        let on_line = (rel.round() as i64) % 2 == 0 && (0.0..=4.0 * sp).contains(&nearest);
        if on_line && (y_end - nearest).abs() < 0.15 * sp {
            y_end = nearest + sign * 0.2 * sp;
        }
        let mut x1 = x_head1.map_or(0.0, |x| x + 0.65 * head_w);
        let mut x2 = x_head2.map_or(input.width, |x| x + 0.35 * head_w);
        if x_head1.is_none() {
            x1 = (x2 - 3.0 * sp).max(0.0);
        }
        if x_head2.is_none() {
            x2 = (x1 + 3.0 * sp).min(input.width);
        }
        // adjustX: clear accidentals before the end head and dots/flags
        // after the start head, within the height band the tie's arc will
        // sweep (a tie bowing toward its own flag must start past it).
        let shoulder_of = |len: f64| {
            (style::TIE_MIN_SHOULDER + 0.3 * (len / sp - 1.0).abs().sqrt() * 0.6)
                .clamp(style::TIE_MIN_SHOULDER, style::TIE_MAX_SHOULDER.min(1.2))
                * sp
        };
        let reach = shoulder_of(x2 - x1) * 0.75;
        let band = if above {
            (y_end - reach - 0.2 * sp, y_end + 0.35 * sp)
        } else {
            (y_end - 0.35 * sp, y_end + reach + 0.2 * sp)
        };
        for r in obstacles {
            if r.y1 < band.0 || r.y0 > band.1 || r.width() > 2.0 * sp {
                continue;
            }
            // Barlines span the staff and ties cross them by design.
            if r.height() >= 3.5 * sp && r.width() < 0.6 * sp {
                continue;
            }
            if let Some(h2) = x_head2
                && r.x1 <= h2 + 0.05 * sp
                && r.x0 > x1
                && r.x1 > x2 - 2.5 * sp
            {
                x2 = x2.min(r.x0 - 0.2 * sp);
            }
            // Right of the start head: dots, a flag, or — for a tie on the
            // stem side — the stem itself (it starts at the head's edge).
            if let Some(h1) = x_head1
                && r.x0 >= h1 + 0.7 * head_w
                && r.x0 < x2
                && r.x0 < x1 + 2.0 * sp
            {
                x1 = x1.max(r.x1 + 0.2 * sp);
            }
        }
        if x2 - x1 < sp {
            x2 = x1 + sp;
        }
        let p1 = Point::new(x1, y_end);
        let p2 = Point::new(x2, y_end);
        let shoulder = shoulder_of(x2 - x1);
        let (c1, c2) = bow_controls(p1, p2, sign * shoulder, 0.6);
        let curve = CubicBez::new(p1, c1, c2, p2);
        let path = crescent(
            p1,
            c1,
            c2,
            p2,
            style::SLUR_END_WIDTH * sp,
            style::SLUR_MID_WIDTH * sp,
            above,
        );
        *id += 1;
        nodes.push(curve_node(ElementType::Tie, *id, path));
        let rects = curve_rects(&curve, style::SLUR_MID_WIDTH * sp, 16);
        sky.add_all(&rects, SkyKind::Tie, *id);
        out.push(PlacedTie {
            rects,
            above,
            entries,
        });
    };

    for (ai, a) in input.anchors.iter().enumerate() {
        for i in 0..a.m.entries.len() {
            let ex = a.entry_x.get(i).copied().unwrap_or(0.0);
            let chord: Vec<i32> = a.m.pitches[i]
                .iter()
                .map(|(l, _)| *l)
                .chain(a.m.stacks[i].iter().map(|(l, _)| *l))
                .collect();
            for line in &a.m.tie_stop_lines[i] {
                if let Some(pos) = pending.iter().position(|(l, ..)| l == line) {
                    let (l, sx, above, from) = pending.remove(pos);
                    emit(
                        l,
                        Some(sx),
                        Some(ex),
                        above,
                        vec![from, (ai, i)],
                        sky,
                        nodes,
                        &mut out,
                        id,
                    );
                } else {
                    // Arriving from the previous system: half tie in.
                    let above = direction(*line, &chord, a.stem_up_at(i, *line));
                    emit(
                        *line,
                        None,
                        Some(ex),
                        above,
                        vec![(ai, i)],
                        sky,
                        nodes,
                        &mut out,
                        id,
                    );
                }
            }
            for line in &a.m.tie_start_lines[i] {
                pending.push((
                    *line,
                    ex,
                    direction(*line, &chord, a.stem_up_at(i, *line)),
                    (ai, i),
                ));
            }
        }
    }
    for (line, sx, above, from) in pending {
        emit(
            line,
            Some(sx),
            None,
            above,
            vec![from],
            sky,
            nodes,
            &mut out,
            id,
        );
    }
    let _ = owners;
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
) -> Vec<Rect> {
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
                placed.push(rect);
                let mut node = SceneNode::group(SemanticId::new(ElementType::Articulation, owner));
                node.add_command(PaintCommand::glyph(glyph, pos, sp, Color::BLACK));
                nodes.push(node);
            }
        }
    }
    placed
}

// ───────────────────────── 3. slurs ─────────────────────────

fn place_slurs(
    input: &SystemInput<'_, '_>,
    obstacles: &[Rect],
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
    id: &mut u64,
) {
    let sp = input.spatium;
    let head_w = HEAD_W_SP * sp;
    // (x of head, primary line, stem up)
    let mut open: Option<(f64, i32, bool)> = None;
    for a in input.anchors {
        for i in 0..a.m.entries.len() {
            let ex = a.entry_x.get(i).copied().unwrap_or(0.0);
            let line = a.m.pitches[i].map(|(l, _)| l).unwrap_or(0);
            let lines: Vec<i32> = a.m.pitches[i]
                .iter()
                .map(|(l, _)| *l)
                .chain(a.m.stacks[i].iter().map(|(l, _)| *l))
                .collect();
            if a.m.slur_stops[i]
                && let Some((sx, sl, s_up)) = open.take()
            {
                let e_up = a.stem_up_at(i, line);
                // computeUp: opposite the stem; mixed stems → above.
                let above = if s_up == e_up { !s_up } else { true };
                let end_line = if above {
                    *lines.iter().max().unwrap_or(&line)
                } else {
                    *lines.iter().min().unwrap_or(&line)
                };
                *id += 1;
                let owner = owners.next();
                let path_rects = shape_slur(
                    (sx, sl),
                    (ex, end_line),
                    above,
                    sp,
                    head_w,
                    obstacles,
                    nodes,
                    *id,
                );
                sky.add_all(&path_rects, SkyKind::Slur, owner);
            }
            if a.m.slur_starts[i] && open.is_none() {
                let start_line = line;
                open = Some((ex, start_line, a.stem_up_at(i, line)));
            }
        }
    }
}

/// MuseScore's `SlurTieLayout::avoidCollisions`, simplified: start from the
/// default bezier, then alternately raise the shoulder over the colliding
/// third and move the endpoints outward, until the curve (plus its arc
/// clearance) clears every obstacle between the ends — at most 30 rounds.
#[allow(clippy::too_many_arguments)]
fn shape_slur(
    start: (f64, i32),
    end: (f64, i32),
    above: bool,
    sp: f64,
    head_w: f64,
    obstacles: &[Rect],
    nodes: &mut Vec<SceneNode>,
    id: u64,
) -> Vec<Rect> {
    let sign = if above { -1.0 } else { 1.0 };
    let mut p1 = Point::new(start.0 + head_w / 2.0, y_of(start.1, sp) + sign * 0.9 * sp);
    let mut p2 = Point::new(end.0 + head_w / 2.0, y_of(end.1, sp) + sign * 0.9 * sp);
    if p2.x - p1.x < sp {
        p2.x = p1.x + sp;
    }
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
    let mut shoulder = (d / 4.0).sqrt() * sp;
    // Obstacles strictly between the end chords, padded toward the slur.
    let lo = p1.x + 0.6 * head_w;
    let hi = p2.x - 0.6 * head_w;
    let local: Vec<Rect> = obstacles
        .iter()
        .filter(|r| r.x1 > lo && r.x0 < hi)
        .map(|r| r.inflate(0.1 * sp, 0.3 * sp))
        .collect();
    let clearance = (0.04 * d).clamp(0.1, 0.5) * sp;
    let step = (0.3 * sp * if d > 16.0 { d / 16.0 } else { 1.0 }).min(1.5 * sp);
    let mut lift = [0.0_f64; 3]; // extra shoulder per third (left, mid, right)

    let build = |p1: Point, p2: Point, shoulder: f64, lift: &[f64; 3]| -> CubicBez {
        let (mut c1, mut c2) = bow_controls(p1, p2, sign * shoulder, w);
        c1.y += sign * (lift[0] + lift[1]);
        c2.y += sign * (lift[2] + lift[1]);
        CubicBez::new(p1, c1, c2, p2)
    };
    let mut curve = build(p1, p2, shoulder, &lift);
    for iter in 0..30 {
        // Sample the curve pushed out by the arc clearance.
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
            // Move the ends outward, favouring the colliding side.
            if hit[0] || hit[1] {
                p1.y += sign * step * 0.4;
            }
            if hit[2] || hit[1] {
                p2.y += sign * step * 0.4;
            }
        }
        // Shoulder never taller than the slur is long.
        let len = (p2 - p1).hypot();
        shoulder = shoulder.min(len);
        for l in &mut lift {
            *l = l.min(len);
        }
        curve = build(p1, p2, shoulder, &lift);
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
    let anchor_x = |a: &MeasureAnchors<'_>, tick: u32| -> f64 { a.x_at(tick) };
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

// ───────────────────────── 5. staff text ─────────────────────────

fn place_staff_text(
    input: &SystemInput<'_, '_>,
    sky: &mut Skyline,
    owners: &mut Ids,
    nodes: &mut Vec<SceneNode>,
) {
    let sp = input.spatium;
    let font = input.words_font;
    let mut words: Vec<(f64, &str)> = Vec::new();
    for a in input.anchors {
        for d in &a.m.directions {
            if let model::DirectionKind::Words(w) = &d.kind {
                words.push((a.x_at(d.tick), w));
            }
        }
    }
    // Staff text on rests/multirests (tempo changes, cues).
    for item in input.items.iter().filter(|i| !i.notated) {
        for d in item.directions {
            if let model::DirectionKind::Words(w) = &d.kind {
                words.push((item.span.0 + 0.5 * sp, w));
            }
        }
    }
    for (x, w) in words {
        // A cue may hold several lines ("Sul ponticello" over "Start trill
        // slowly…"): stack them, last line on the baseline.
        let lines: Vec<&str> = w.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let leading = 1.15 * font.size;
        let n = lines.len();
        let mut block: Vec<(f64, &str, Rect)> = Vec::new(); // (line offset, text, ink)
        for (k, line) in lines.iter().enumerate() {
            let Some(r) = metrics::text_ink(
                &font.family,
                font.size,
                font.bold,
                font.italic,
                line,
                Point::ZERO,
                TextAnchor::Start,
            ) else {
                continue;
            };
            let off = -((n - 1 - k) as f64) * leading;
            block.push((off, line, r + Vec2::new(0.0, off)));
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
        for (off, line, r) in &block {
            sky.add(*r + Vec2::new(x, base), SkyKind::StaffText, owner);
            node.add_child(styled_text(
                line,
                font,
                Point::new(x, base + off),
                TextAnchor::Start,
            ));
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
        node.add_child(styled_text(
            &n.text,
            &n.font,
            Point::new(n.pos.x, n.pos.y + dy),
            n.anchor,
        ));
        nodes.push(node);
    }
}

// ───────────────────────── 7. rehearsal marks ─────────────────────────

fn place_rehearsals(
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
        let mut boxes: Vec<(Rect, &RehearsalMark, f64, f64)> = Vec::new(); // (box, mark, text x, pad)
        for mark in &marks {
            let size = mark.font_size.unwrap_or(12.0);
            let Some(t) = metrics::text_ink(
                input.text_family,
                size,
                mark.bold,
                mark.italic,
                &mark.text,
                Point::ZERO,
                TextAnchor::Start,
            ) else {
                continue;
            };
            let enclosed = mark.enclosure != Enclosure::None;
            let pad = if enclosed {
                0.3 * sp + 0.12 * size
            } else {
                0.0
            };
            // Box around the ink (baseline 0), text starting at `cursor+pad`.
            let b = Rect::new(
                cursor,
                t.y0 - pad,
                cursor + t.x1 + 2.0 * pad,
                t.y1.max(0.0) + pad,
            );
            boxes.push((b, mark, cursor + pad, pad));
            cursor = b.x1 + 0.5 * size;
        }
        // Default: box bottoms just above the staff; then push clear.
        let Some(bottom) = boxes.iter().map(|(b, ..)| b.y1).reduce(f64::max) else {
            continue;
        };
        let mut dy = -style::REHEARSAL_POS_ABOVE * sp - bottom;
        let shape: Vec<Rect> = boxes
            .iter()
            .map(|(b, ..)| *b + Vec2::new(0.0, dy))
            .collect();
        dy += sky.distance_to_move(
            &shape,
            true,
            style::REHEARSAL_MIN_DISTANCE * sp,
            style::SKYLINE_H_CLEARANCE * sp,
            |_, _| false,
        );
        let owner = owners.next();
        let mut node = SceneNode::group(SemanticId::new(ElementType::RehearsalMark, owner));
        for (b, mark, text_x, _) in boxes {
            let b = b + Vec2::new(0.0, dy);
            sky.add(b, SkyKind::Rehearsal, owner);
            let size = mark.font_size.unwrap_or(12.0);
            let mut cmds = Vec::new();
            if mark.enclosure != Enclosure::None {
                cmds.push(PaintCommand::Rect {
                    rect: b,
                    fill: None,
                    stroke: Some(Color::BLACK),
                    stroke_width: 0.16 * sp,
                    corner_radius: (mark.enclosure == Enclosure::Oval).then_some(b.height() / 2.0),
                });
            }
            cmds.push(PaintCommand::Text {
                text: mark.text.clone(),
                font_family: input.text_family.to_string(),
                font_size: size,
                position: Point::new(text_x, dy),
                color: Color::BLACK,
                anchor: TextAnchor::Start,
                weight: if mark.bold {
                    FontWeight::Bold
                } else {
                    FontWeight::Normal
                },
                style: if mark.italic {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                },
            });
            node.add_child(SceneNode::anonymous_leaf(cmds));
        }
        nodes.push(node);
    }
}
