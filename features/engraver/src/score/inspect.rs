//! Collision inspection: where does a laid-out part put ink on ink?
//!
//! Walks a [`PartLayout`]'s scene, measures every glyph and text run by its
//! real outline (not an advance-width estimate), classifies each by the
//! nearest semantic ancestor (note, dynamic, rehearsal mark, tie…), and
//! reports overlapping pairs. Ties and slurs are tested along the curve
//! itself — a bounding box of an arc would "hit" every note it spans.
//!
//! This is the regression metric for vertical placement: a change to the
//! autoplace pass should move these numbers down, never up.

use std::collections::BTreeMap;

use crate::engraver::fonts::ChartFontBundle;
use crate::engraver::scene::id::ElementType;
use crate::engraver::scene::node::SceneNode;
use crate::engraver::scene::paint::{FontStyle, FontWeight, PaintCommand, TextAnchor};
use kurbo::{Affine, Point, Rect, Shape as _};

use crate::score::layout::PartLayout;

/// What a piece of ink is, for deciding which overlaps matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InkKind {
    /// Noteheads, rests, accidentals, dots, flags, beams — the measure body.
    Notation,
    /// Clef / key / time signature.
    Prefix,
    Articulation,
    Dynamic,
    Hairpin,
    /// Rehearsal marks and their boxes.
    Rehearsal,
    /// Staff text, tempo, technique cues.
    Text,
    MeasureNumber,
    /// Multirest bar and count.
    MultiRest,
    Tie,
    Slur,
    /// Headers, titles, footers.
    PageText,
}

impl InkKind {
    fn is_placed(self) -> bool {
        // Elements a vertical placement pass is responsible for.
        matches!(
            self,
            InkKind::Articulation
                | InkKind::Dynamic
                | InkKind::Hairpin
                | InkKind::Rehearsal
                | InkKind::Text
                | InkKind::MeasureNumber
                | InkKind::Tie
                | InkKind::Slur
        )
    }
}

/// One measured piece of ink, in page-scene coordinates.
#[derive(Debug, Clone)]
pub struct Ink {
    pub kind: InkKind,
    pub rect: Rect,
    /// Text content, when it is text.
    pub label: String,
    /// Elements sharing an owner never collide with each other (a chord's
    /// notehead and its accidental, a rehearsal box and its text).
    owner: u64,
    /// For ties/slurs: points along the curve's outline.
    samples: Vec<Point>,
}

/// An overlap between two pieces of ink.
#[derive(Debug, Clone)]
pub struct Collision {
    /// 1-based page.
    pub page: usize,
    pub a: (InkKind, String),
    pub b: (InkKind, String),
    /// Where (page coordinates, points).
    pub at: Point,
    /// Overlap area in pt² (0 for curve hits).
    pub area: f64,
}

/// Summary for a part.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub collisions: Vec<Collision>,
    /// Count per (kind, kind) pair, kinds ordered.
    pub by_kind: BTreeMap<(InkKind, InkKind), usize>,
}

impl Report {
    #[must_use]
    pub fn total(&self) -> usize {
        self.collisions.len()
    }
}

// ───────────────────────── scene walk ─────────────────────────

fn kind_of(t: ElementType) -> Option<InkKind> {
    Some(match t {
        ElementType::Chord
        | ElementType::Note
        | ElementType::NoteHead
        | ElementType::Rest
        | ElementType::Accidental
        | ElementType::Dot
        | ElementType::Flag
        | ElementType::Beam
        | ElementType::Tuplet => InkKind::Notation,
        ElementType::Clef | ElementType::KeySignature | ElementType::TimeSignature => {
            InkKind::Prefix
        }
        ElementType::Articulation | ElementType::Fermata => InkKind::Articulation,
        ElementType::Dynamic => InkKind::Dynamic,
        ElementType::Hairpin => InkKind::Hairpin,
        ElementType::RehearsalMark => InkKind::Rehearsal,
        ElementType::Text | ElementType::Tempo => InkKind::Text,
        ElementType::MeasureNumber => InkKind::MeasureNumber,
        ElementType::Tie => InkKind::Tie,
        ElementType::Slur => InkKind::Slur,
        _ => return None,
    })
}

struct Walker<'f> {
    faces: &'f mut crate::score::metrics::Faces,
    out: Vec<Ink>,
    next_owner: u64,
}

impl Walker<'_> {
    fn walk(
        &mut self,
        node: &SceneNode,
        transform: Affine,
        kind: Option<InkKind>,
        owner: u64,
        depth: usize,
    ) {
        let transform = transform * node.transform;
        let own_kind = node.id.as_ref().and_then(|id| kind_of(id.element_type));
        let parent_kind = kind;
        let kind = own_kind.or(kind);
        // A new owner at each semantic element that is not nested in one of
        // its own kind (a dynamic's letters are one dynamic), and at every
        // top-level child of a system (anonymous leaves drawn separately
        // are separate objects).
        let owner = if (own_kind.is_some() && own_kind != parent_kind) || depth <= 2 {
            self.next_owner += 1;
            self.next_owner
        } else {
            owner
        };
        for cmd in &node.commands {
            self.command(cmd, transform, kind, owner);
        }
        for child in &node.children {
            self.walk(child, transform, kind, owner, depth + 1);
        }
    }

    fn command(&mut self, cmd: &PaintCommand, t: Affine, kind: Option<InkKind>, owner: u64) {
        let world = |r: Rect| t.transform_rect_bbox(r);
        match cmd {
            PaintCommand::Text {
                text,
                font_family,
                font_size,
                position,
                anchor,
                weight,
                style,
                ..
            } => {
                let music = matches!(font_family.as_str(), "Leland" | "Bravura");
                let kind = kind.unwrap_or(if music {
                    InkKind::Notation
                } else {
                    InkKind::Text
                });
                if let Some(r) = self.faces.text_ink(
                    font_family,
                    *font_size,
                    matches!(weight, FontWeight::Bold),
                    matches!(style, FontStyle::Italic),
                    text,
                    *position,
                    *anchor,
                ) {
                    self.out.push(Ink {
                        kind,
                        rect: world(r),
                        label: text.clone(),
                        owner,
                        samples: Vec::new(),
                    });
                }
            }
            PaintCommand::Glyph {
                codepoint,
                position,
                size,
                ..
            } => {
                if let Some(r) = self.faces.text_ink(
                    "Leland",
                    size * 4.0,
                    false,
                    false,
                    &codepoint.to_string(),
                    *position,
                    TextAnchor::Start,
                ) {
                    self.out.push(Ink {
                        kind: kind.unwrap_or(InkKind::Articulation),
                        rect: world(r),
                        label: format!("U+{:04X}", u32::from(*codepoint)),
                        owner,
                        samples: Vec::new(),
                    });
                }
            }
            PaintCommand::Fill { path, .. } => {
                let kind = kind.unwrap_or(InkKind::Notation);
                let mut samples = Vec::new();
                if matches!(kind, InkKind::Tie | InkKind::Slur) {
                    kurbo::flatten(path, 0.25, |el| {
                        if let kurbo::PathEl::LineTo(p) | kurbo::PathEl::MoveTo(p) = el {
                            samples.push(t * p);
                        }
                    });
                }
                self.out.push(Ink {
                    kind,
                    rect: world(path.bounding_box()),
                    label: String::new(),
                    owner,
                    samples,
                });
            }
            PaintCommand::Rect {
                rect, fill, stroke, ..
            } => {
                // A stroked box is a rehearsal enclosure; a filled bar is a
                // multirest's H-bar.
                let kind = match (fill, stroke) {
                    // Keyflow markers fill their pills and callout tints.
                    (Some(_), _) if kind == Some(InkKind::Rehearsal) => InkKind::Rehearsal,
                    (Some(_), _) => InkKind::MultiRest,
                    _ => kind.unwrap_or(InkKind::Rehearsal),
                };
                self.out.push(Ink {
                    kind,
                    rect: world(*rect),
                    label: String::new(),
                    owner,
                    samples: Vec::new(),
                });
            }
            // Staff lines, barlines, stems, hairpin strokes: thin, and their
            // overlaps are by design.
            _ => {}
        }
    }
}

/// Measure every piece of ink in a laid-out part.
#[must_use]
pub fn ink(layout: &PartLayout, fonts: &ChartFontBundle) -> Vec<Ink> {
    let mut faces = crate::score::metrics::Faces::new(fonts);
    let mut walker = Walker {
        faces: &mut faces,
        out: Vec::new(),
        next_owner: 0,
    };
    walker.walk(&layout.scene, Affine::IDENTITY, None, 0, 0);
    walker.out
}

fn page_of(layout: &PartLayout, y: f64) -> usize {
    layout
        .pages
        .iter()
        .position(|p| y >= p.y_offset && y < p.y_offset + p.height)
        .map_or(0, |i| i + 1)
}

/// Find overlapping ink in a laid-out part.
#[must_use]
pub fn collisions(layout: &PartLayout, fonts: &ChartFontBundle) -> Report {
    // Overlaps smaller than this (points) are kerning, not collisions.
    const TOLERANCE: f64 = 0.4;
    let inks = ink(layout, fonts);
    let mut report = Report::default();
    let label = |i: &Ink| (i.kind, i.label.clone());

    for (ai, a) in inks.iter().enumerate() {
        for b in &inks[ai + 1..] {
            if a.owner == b.owner {
                continue;
            }
            // Something a placement pass owns must be involved; notation
            // against notation is the horizontal spacer's business.
            if !a.kind.is_placed() && !b.kind.is_placed() {
                continue;
            }
            // Ties/slurs: test the curve, not its box.
            let curve_hit = |c: &Ink, other: &Ink| -> Option<Point> {
                let solid = matches!(
                    other.kind,
                    InkKind::Notation
                        | InkKind::Articulation
                        | InkKind::Dynamic
                        | InkKind::Rehearsal
                        | InkKind::Text
                        | InkKind::MeasureNumber
                );
                if !solid {
                    return None;
                }
                // The ends sit on their own noteheads by design (a tie
                // starts over its head): skip ~1 staff space at each end.
                let (Some(first), Some(last)) = (c.samples.first(), c.samples.last()) else {
                    return None;
                };
                let end_zone = 6.0;
                let r = other.rect.inset(-TOLERANCE);
                c.samples
                    .iter()
                    .filter(|p| (p.x - first.x).abs() > end_zone && (last.x - p.x).abs() > end_zone)
                    .copied()
                    .find(|p| r.contains(*p))
            };
            let hit = match (
                matches!(a.kind, InkKind::Tie | InkKind::Slur),
                matches!(b.kind, InkKind::Tie | InkKind::Slur),
            ) {
                (true, true) => None,
                (true, false) => curve_hit(a, b).map(|p| (p, 0.0)),
                (false, true) => curve_hit(b, a).map(|p| (p, 0.0)),
                (false, false) => {
                    let i = a.rect.inset(-TOLERANCE).intersect(b.rect.inset(-TOLERANCE));
                    (i.width() > 0.0 && i.height() > 0.0).then(|| (i.center(), i.area()))
                }
            };
            if let Some((at, area)) = hit {
                let (ka, kb) = if a.kind <= b.kind {
                    (a.kind, b.kind)
                } else {
                    (b.kind, a.kind)
                };
                *report.by_kind.entry((ka, kb)).or_default() += 1;
                report.collisions.push(Collision {
                    page: page_of(layout, at.y),
                    a: label(a),
                    b: label(b),
                    at,
                    area,
                });
            }
        }
    }
    report
}
