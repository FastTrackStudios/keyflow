//! Staff skylines and autoplace — MuseScore's vertical collision model.
//!
//! Port of MuseScore 4's `SkylineLine` (src/engraving/infrastructure/
//! skyline.cpp, shape.cpp) and `Autoplace::distanceToMoveItem` (rendering/
//! score/autoplace.cpp), GPLv3 like this crate.
//!
//! A skyline is the ink a staff already carries, as plain rectangles in
//! staff-local coordinates (y = 0 on the top line, y down). Its NORTH line
//! keeps only what pokes above the staff, its SOUTH line only what hangs
//! below; ink inside the staff is not an obstacle (the staff itself never
//! pushes anything — an element's style offset puts it clear of the lines).
//! Items are placed in a fixed order, each pushed clear of everything
//! already in the skyline by its minimum distance and then added to it, so
//! what is placed first sits closest to the staff.

use kurbo::Rect;

/// What a skyline rectangle belongs to, for the ignore rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkyKind {
    /// Notes, rests, stems, beams, accidentals, clefs — the measure body.
    Notation,
    Tie,
    Articulation,
    Slur,
    Dynamic,
    Hairpin,
    StaffText,
    MeasureNumber,
    Rehearsal,
}

#[derive(Debug, Clone, Copy)]
struct SkyRect {
    rect: Rect,
    kind: SkyKind,
    /// Distinguishes items of one kind (articulations on different notes
    /// ignore each other; items never collide with themselves).
    owner: u64,
}

/// One staff's skyline (north + south), staff-local coordinates.
#[derive(Debug, Clone, Default)]
pub struct Skyline {
    north: Vec<SkyRect>,
    south: Vec<SkyRect>,
    /// y of the bottom staff line.
    staff_height: f64,
}

/// `Shape::intersects` horizontal test: [a, b] against [c, d] with
/// clearance. Zero-width spans never intersect.
fn h_overlap(a: f64, b: f64, c: f64, d: f64, clearance: f64) -> bool {
    !(a == b || c == d) && b + clearance > c && a < d + clearance
}

impl Skyline {
    #[must_use]
    pub fn new(staff_height: f64) -> Self {
        Self {
            north: Vec::new(),
            south: Vec::new(),
            staff_height,
        }
    }

    /// Add ink. Only the parts outside the staff matter: north keeps a
    /// rectangle whose top is above the top line, south one whose bottom is
    /// below the bottom line (MuseScore's `SkylineLine::add`).
    pub fn add(&mut self, rect: Rect, kind: SkyKind, owner: u64) {
        if rect.height() <= 0.0 || rect.width() <= 0.0 {
            return;
        }
        // A hair of tolerance so the staff lines' own stroke width (and
        // barlines drawn line to line) do not register.
        let eps = 0.05;
        let item = SkyRect { rect, kind, owner };
        if rect.y0 < -eps {
            self.north.push(item);
        }
        if rect.y1 > self.staff_height + eps {
            self.south.push(item);
        }
    }

    pub fn add_all(&mut self, rects: &[Rect], kind: SkyKind, owner: u64) {
        for r in rects {
            self.add(*r, kind, owner);
        }
    }

    /// How far to move `shape` (up for `above`, down otherwise) so it clears
    /// the skyline by `min_distance`: MuseScore's `distanceToMoveItem`.
    /// Returns a signed dy (negative = up); 0 if already clear.
    #[must_use]
    pub fn distance_to_move(
        &self,
        shape: &[Rect],
        above: bool,
        min_distance: f64,
        clearance: f64,
        ignore: impl Fn(SkyKind, u64) -> bool,
    ) -> f64 {
        let line = if above { &self.north } else { &self.south };
        // `minVerticalDistance`: the largest overlap (positive) between any
        // horizontally-overlapping pair — upper.bottom − lower.top.
        let mut d = f64::NEG_INFINITY;
        for s in shape {
            for k in line {
                if ignore(k.kind, k.owner) {
                    continue;
                }
                if !h_overlap(s.x0, s.x1, k.rect.x0, k.rect.x1, clearance) {
                    continue;
                }
                let overlap = if above {
                    s.y1 - k.rect.y0 // shape above the skyline item
                } else {
                    k.rect.y1 - s.y0 // skyline item above the shape
                };
                d = d.max(overlap);
            }
        }
        // Already clear by at least `min_distance` (or nothing overlaps).
        if d <= -min_distance {
            return 0.0;
        }
        let push = d + min_distance;
        if above { -push } else { push }
    }

    /// Outermost ink above the staff (most negative y), if any.
    #[must_use]
    pub fn north_extent(&self) -> f64 {
        self.north.iter().map(|k| k.rect.y0).fold(0.0, f64::min)
    }

    /// Outermost ink below the staff (largest y), at least the staff.
    #[must_use]
    pub fn south_extent(&self) -> f64 {
        self.south
            .iter()
            .map(|k| k.rect.y1)
            .fold(self.staff_height, f64::max)
    }

    /// Smallest top-line-to-top-line distance at which `below` (the next
    /// system's staff) clears this skyline: this south line against the
    /// other's north line, horizontal clearance 0.25 sp.
    #[must_use]
    pub fn min_distance_to(&self, below: &Skyline, clearance: f64) -> f64 {
        let mut d = f64::NEG_INFINITY;
        for a in &self.south {
            for b in &below.north {
                if h_overlap(a.rect.x0, a.rect.x1, b.rect.x0, b.rect.x1, clearance) {
                    d = d.max(a.rect.y1 - b.rect.y0);
                }
            }
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SP: f64 = 5.0;

    fn sky() -> Skyline {
        Skyline::new(4.0 * SP)
    }

    #[test]
    fn ink_inside_the_staff_is_ignored() {
        let mut s = sky();
        s.add(Rect::new(0.0, 2.0, 10.0, 8.0), SkyKind::Notation, 1);
        let text = [Rect::new(0.0, -6.0, 20.0, 1.0)];
        // Text whose bottom dips 1pt into the staff: nothing to clear.
        assert_eq!(
            s.distance_to_move(&text, true, 0.5 * SP, 0.0, |_, _| false),
            0.0
        );
    }

    #[test]
    fn pushes_above_protruding_ink_by_min_distance() {
        let mut s = sky();
        // A stem tip 10pt above the staff.
        s.add(Rect::new(4.0, -10.0, 5.0, 15.0), SkyKind::Notation, 1);
        let text = [Rect::new(0.0, -8.0, 20.0, 0.0)];
        let dy = s.distance_to_move(&text, true, 2.5, 0.0, |_, _| false);
        // bottom (0) − top (−10) = 10 overlap, + 2.5 → move up 12.5.
        assert!((dy + 12.5).abs() < 1e-9, "{dy}");
        // Far to the side: clear.
        let away = [Rect::new(40.0, -8.0, 60.0, 0.0)];
        assert_eq!(s.distance_to_move(&away, true, 2.5, 0.0, |_, _| false), 0.0);
    }

    #[test]
    fn pushes_below_and_respects_ignore_rules() {
        let mut s = sky();
        s.add(Rect::new(0.0, 15.0, 30.0, 28.0), SkyKind::Dynamic, 7);
        let hairpin = [Rect::new(10.0, 22.0, 50.0, 26.0)];
        let dy = s.distance_to_move(&hairpin, false, 2.0, 0.0, |_, _| false);
        assert!((dy - 8.0).abs() < 1e-9, "{dy}"); // 28 − 22 + 2
        let ignored = s.distance_to_move(&hairpin, false, 2.0, 0.0, |k, _| k == SkyKind::Dynamic);
        assert_eq!(ignored, 0.0);
    }

    #[test]
    fn horizontal_clearance_widens_the_test() {
        let mut s = sky();
        s.add(Rect::new(0.0, -10.0, 10.0, 0.0), SkyKind::Notation, 1);
        let near = [Rect::new(11.0, -4.0, 20.0, 0.0)];
        assert_eq!(s.distance_to_move(&near, true, 1.0, 0.0, |_, _| false), 0.0);
        assert!(s.distance_to_move(&near, true, 1.0, 1.25, |_, _| false) < 0.0);
    }
}
