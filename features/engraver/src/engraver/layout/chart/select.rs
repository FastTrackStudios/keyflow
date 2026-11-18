//! What on a laid-out chart can be selected, where, and in what order: the
//! engraved elements a click or a box picks (MuseScore's `EngravingItem`s
//! as the score view hit-tests them).
//!
//! An element is a scene node the layout named: a chord symbol (its
//! `element_type`), a rest or a note (its `SemanticId`), a cue or a
//! dynamic (their metadata), and so on ([`kind_of`]); and every bar, from
//! the layout's own measure boxes. Its bounds are measured from what it
//! draws, at the time of asking (`scene_ink_bounds`): a node's stored
//! bounds go stale when a later pass moves its commands.
//!
//! The list is in reading order (page, system, left to right), which a
//! Shift-click's range follows. Keys are positional: they name an element
//! in this layout, and an edit that moves things renumbers them.

use kurbo::{Affine, Point, Rect};

use super::notation_renderer::scene_ink_bounds;
use super::types::ChartLayoutResult;
use crate::engraver::scene::{ElementType, SceneNode, metadata_keys};
use keyflow_proto::TextSpan;

/// What kind of thing an element is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Bar,
    Chord,
    Note,
    Rest,
    Tie,
    Section,
    Cue,
    Text,
    Dynamic,
    Hairpin,
    Volta,
    Mark,
    Key,
    Time,
    Clef,
    MeasureNumber,
    Lyrics,
    Simile,
}

impl Kind {
    /// Its name, as a selection reports it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Chord => "chord",
            Self::Note => "note",
            Self::Rest => "rest",
            Self::Tie => "tie",
            Self::Section => "section",
            Self::Cue => "cue",
            Self::Text => "text",
            Self::Dynamic => "dynamic",
            Self::Hairpin => "hairpin",
            Self::Volta => "volta",
            Self::Mark => "mark",
            Self::Key => "key",
            Self::Time => "time",
            Self::Clef => "clef",
            Self::MeasureNumber => "measure-number",
            Self::Lyrics => "lyrics",
            Self::Simile => "simile",
        }
    }
}

/// One element that can be selected.
#[derive(Clone, Debug, PartialEq)]
pub struct Selectable {
    /// Its key in this layout: its kind and its place among its kind in
    /// reading order (`chord#12`; a bar's is its measure, `bar#7`).
    pub key: String,
    pub kind: Kind,
    /// Where it is, in chart points.
    pub bounds: Rect,
    /// Its text in the chart's source, when the layout kept it.
    pub span: Option<TextSpan>,
    /// The scene node that draws it, as child indices from the layout's
    /// root: what a highlight draws again in another colour. `None` for a
    /// bar, which is the layout's box, not a node.
    pub node: Option<Vec<usize>>,
}

/// One element found on the walk: its kind, bounds, span and node.
type Found = (Kind, Rect, Option<TextSpan>, Option<Vec<usize>>);

/// The metadata keys that name an element, and its kind.
const MARKED: &[(&str, Kind)] = &[
    ("text_cue", Kind::Cue),
    ("staff_text", Kind::Text),
    ("dynamic", Kind::Dynamic),
    ("hairpin", Kind::Hairpin),
    ("volta", Kind::Volta),
    ("measure_number", Kind::MeasureNumber),
    ("push_marker", Kind::Mark),
    ("accent", Kind::Mark),
    ("marcato", Kind::Mark),
    ("staccato", Kind::Mark),
    ("fermata", Kind::Mark),
    ("stop_sign", Kind::Mark),
    ("stop_groove", Kind::Mark),
    ("figured_bass", Kind::Text),
    ("suspension", Kind::Text),
    ("lyric_lane", Kind::Lyrics),
    ("simile", Kind::Simile),
    ("folded_section", Kind::Section),
    ("section_label_text", Kind::Section),
    ("key", Kind::Key),
    ("timesig", Kind::Time),
    ("clef-type", Kind::Clef),
];

/// The kind of element `node` is, if it is one.
#[must_use]
pub fn kind_of(node: &SceneNode) -> Option<Kind> {
    match node.get_element_type() {
        Some("chord") => return Some(Kind::Chord),
        Some("chord_tie" | "push_tie") => return Some(Kind::Tie),
        _ => {}
    }
    if let Some((_, kind)) = MARKED.iter().find(|(key, _)| node.has_metadata(key)) {
        return Some(*kind);
    }
    match node.id.as_ref()?.element_type {
        ElementType::Chord | ElementType::Note => Some(Kind::Note),
        ElementType::Rest => Some(Kind::Rest),
        ElementType::RehearsalMark => Some(Kind::Section),
        ElementType::TimeSignature => Some(Kind::Time),
        ElementType::MeasureNumber => Some(Kind::MeasureNumber),
        ElementType::Dynamic => Some(Kind::Dynamic),
        ElementType::Hairpin => Some(Kind::Hairpin),
        ElementType::Tie | ElementType::Slur => Some(Kind::Tie),
        ElementType::Lyrics => Some(Kind::Lyrics),
        _ => None,
    }
}

/// Every element of `layout` that can be selected, in reading order.
#[must_use]
pub fn selectables(layout: &ChartLayoutResult) -> Vec<Selectable> {
    let mut found: Vec<Found> = Vec::new();
    walk(&layout.scene, Affine::IDENTITY, &mut Vec::new(), &mut found);

    // Every bar, from the layout's own boxes: a measure that wraps is one
    // per system it sits on. The count-in's beats (negative ticks, their
    // own small boxes in the header) are not bars: their measures count
    // from 0 too, and would take `bar#0` from the chart's first bar
    // (`export::follow::bar_boxes` leaves them out the same way).
    let bar_beats = || {
        layout
            .beat_positions
            .iter()
            .filter(|b| b.absolute_tick >= 0)
    };
    for beat in bar_beats() {
        let rect = Rect::new(
            beat.measure_x0,
            beat.staff_y,
            beat.measure_x1,
            beat.staff_y + beat.staff_height,
        );
        let seen = found
            .iter()
            .any(|(kind, r, _, _)| *kind == Kind::Bar && *r == rect);
        if !seen && rect.width() > 0.0 {
            found.push((Kind::Bar, rect, None, None));
        }
    }

    let systems = systems(layout);
    let page_of = |r: &Rect| page_of(layout, r.center());
    let system_of = |r: &Rect| {
        let page = page_of(r);
        systems
            .iter()
            .enumerate()
            .filter(|(_, s)| s.0 == page)
            .min_by(|a, b| {
                let d = |s: &(usize, f64, f64)| (r.center().y - (s.1 + s.2 / 2.0)).abs();
                d(a.1).total_cmp(&d(b.1))
            })
            .map_or(0, |(i, _)| i)
    };
    found.sort_by(|a, b| {
        (page_of(&a.1), system_of(&a.1))
            .cmp(&(page_of(&b.1), system_of(&b.1)))
            .then(a.1.x0.total_cmp(&b.1.x0))
            .then(a.0.cmp(&b.0))
    });

    let mut counts = std::collections::HashMap::<Kind, usize>::new();
    let bars: Vec<(Rect, usize)> = bar_beats()
        .map(|b| {
            (
                Rect::new(
                    b.measure_x0,
                    b.staff_y,
                    b.measure_x1,
                    b.staff_y + b.staff_height,
                ),
                b.measure,
            )
        })
        .collect();
    found
        .into_iter()
        .map(|(kind, bounds, span, node)| {
            let key = if kind == Kind::Bar {
                let measure = bars
                    .iter()
                    .find(|(r, _)| *r == bounds)
                    .map_or(0, |(_, m)| *m);
                let n = counts.entry(kind).or_default();
                *n += 1;
                // A wrapped measure's second box is `bar#7.2`.
                let parts = bars.iter().filter(|(_, m)| *m == measure).count();
                if parts > 1 {
                    format!("bar#{measure}.{n}")
                } else {
                    format!("bar#{measure}")
                }
            } else {
                let n = counts.entry(kind).or_default();
                let key = format!("{}#{n}", kind.name());
                *n += 1;
                key
            };
            Selectable {
                key,
                kind,
                bounds,
                span,
                node,
            }
        })
        .collect()
}

/// Collects every named element under `node`; an element inside another
/// (a note in a measure's group) is one of its own.
fn walk(node: &SceneNode, parent: Affine, path: &mut Vec<usize>, found: &mut Vec<Found>) {
    if !node.visible {
        return;
    }
    let transform = parent * node.transform;
    if let Some(kind) = kind_of(node)
        && let Some(bounds) = scene_ink_bounds(node, parent)
    {
        let span = node.get_json_metadata::<TextSpan>(metadata_keys::SOURCE_SPAN);
        found.push((kind, bounds, span, Some(path.clone())));
        // A chord symbol's own parts (its quality, its bass) are it.
        if kind == Kind::Chord {
            return;
        }
    }
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        walk(child, transform, path, found);
        path.pop();
    }
}

/// The node at `path` (child indices from `root`) and the transform its
/// parent places it with, from `root`'s parent's space: what
/// [`Selectable::node`] names.
#[must_use]
pub fn node_at<'a>(root: &'a SceneNode, path: &[usize]) -> Option<(&'a SceneNode, Affine)> {
    let mut node = root;
    let mut parent = Affine::IDENTITY;
    for &i in path {
        parent *= node.transform;
        node = node.children.get(i)?;
    }
    Some((node, parent))
}

/// Each system: its page, its staff's top and its height, in layout order.
fn systems(layout: &ChartLayoutResult) -> Vec<(usize, f64, f64)> {
    let mut out: Vec<(usize, f64, f64)> = Vec::new();
    for b in &layout.beat_positions {
        let page = page_of(layout, Point::new(b.measure_x0, b.staff_y));
        if !out
            .iter()
            .any(|s| s.0 == page && (s.1 - b.staff_y).abs() < 0.5)
        {
            out.push((page, b.staff_y, b.staff_height));
        }
    }
    out
}

/// The page `point` falls on (pages sit side by side), by index.
fn page_of(layout: &ChartLayoutResult, point: Point) -> usize {
    layout
        .pages
        .iter()
        .position(|p| point.x >= p.x_offset && point.x < p.x_offset + p.width)
        .unwrap_or(0)
}

/// The element a click at `point` (chart points) picks: the smallest one
/// under it, so a chord beats the bar it sits in. `slop` widens each by
/// as much, so a thin one can be hit.
#[must_use]
pub fn pick(items: &[Selectable], point: Point, slop: f64) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, s)| s.bounds.inflate(slop, slop).contains(point))
        .min_by(|(_, a), (_, b)| a.bounds.area().total_cmp(&b.bounds.area()))
        .map(|(i, _)| i)
}

/// What a box dragged over `rect` (chart points) selects: every element it
/// touches, and a bar only when it holds the whole of it.
#[must_use]
pub fn in_box(items: &[Selectable], rect: Rect) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            if s.kind == Kind::Bar {
                rect.contains(Point::new(s.bounds.x0, s.bounds.y0))
                    && rect.contains(Point::new(s.bounds.x1, s.bounds.y1))
            } else {
                rect.intersect(s.bounds).area() > 0.0
            }
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, kind: Kind, r: (f64, f64, f64, f64)) -> Selectable {
        Selectable {
            key: key.to_owned(),
            kind,
            bounds: Rect::new(r.0, r.1, r.2, r.3),
            span: None,
            node: None,
        }
    }

    #[test]
    fn a_click_picks_the_smallest_element_under_it() {
        let items = [
            item("bar#0", Kind::Bar, (0.0, 0.0, 100.0, 40.0)),
            item("chord#0", Kind::Chord, (10.0, 5.0, 30.0, 15.0)),
        ];
        assert_eq!(
            pick(&items, Point::new(20.0, 10.0), 0.0),
            Some(1),
            "the chord"
        );
        assert_eq!(
            pick(&items, Point::new(60.0, 30.0), 0.0),
            Some(0),
            "the bar"
        );
        assert_eq!(pick(&items, Point::new(200.0, 30.0), 0.0), None);
        assert_eq!(pick(&items, Point::new(31.0, 10.0), 2.0), Some(1), "slop");
    }

    #[test]
    fn a_box_takes_what_it_touches_and_only_whole_bars() {
        let items = [
            item("bar#0", Kind::Bar, (0.0, 0.0, 100.0, 40.0)),
            item("chord#0", Kind::Chord, (10.0, 5.0, 30.0, 15.0)),
            item("bar#1", Kind::Bar, (100.0, 0.0, 200.0, 40.0)),
        ];
        let touched = in_box(&items, Rect::new(20.0, 0.0, 150.0, 50.0));
        assert_eq!(touched, [1], "the chord it touches, no bar it only crosses");
        let all = in_box(&items, Rect::new(-1.0, -1.0, 201.0, 41.0));
        assert_eq!(all, [0, 1, 2]);
    }
}
