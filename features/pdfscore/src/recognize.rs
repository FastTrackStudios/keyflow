//! Recognition: a page's glyphs and lines, read back into a score.
//!
//! Exact, not statistical — everything is measured from the page:
//!
//! 1. **Staves**: runs of five evenly spaced full-width horizontal lines.
//!    The gap is the staff space (`sp`); every other tolerance is in `sp`.
//! 2. **Systems**: staves joined by a barline that crosses between them
//!    (a grand staff); otherwise each staff is its own system.
//! 3. **Barlines**: vertical lines running exactly from a staff's top line
//!    to a (possibly lower) staff's bottom line, grouped by x into thin,
//!    double, final and repeat barlines (repeat dots beside them).
//! 4. **Measures**: the spans between barlines. A span at a system's start
//!    with no notes or rests is its prelude — clef, key and time.
//! 5. **Notes**: noteheads take stems (verticals touching their left or
//!    right edge), stems take beams (straight fills crossing the stem's far
//!    end) and flags; together with the notehead's shape and its dots that
//!    is the written duration. Pitch is the staff position read through the
//!    clef, the key signature and the measure's accidentals.
//! 6. **Text**: chord symbols (the engraver's chord font) become harmony;
//!    other words above a system become staff text; the title is the
//!    largest line at the top; a `♩ = 166` in the text font is the tempo.
//!
//! The font tables are for Sibelius's Helsinki/Opus family (the Maestro
//! character layout) — the first engraver this reads. Another engraver is
//! another table, not another recogniser.

use std::collections::BTreeMap;

use engraver_score::model::{
    AttrChange, BarStyle, Chord, Clef, ClefSign, Direction, DirectionKind, Event, Measure, Note,
    NoteTypeValue, Part, Rest, Score, Step, TempoMark, WrittenDuration, WrittenPitch,
};
use kurbo::{Point, Shape};

use crate::extract::{Glyph, Page, as_segment};

/// Ticks per quarter note in the recognised score.
pub const DIVISIONS: u32 = 480;

/// What recognition produced, and what it could not account for.
#[derive(Debug, Clone)]
pub struct Recognized {
    pub score: Score,
    pub report: Report,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub staves: usize,
    pub systems: usize,
    pub measures: usize,
    pub notes: usize,
    pub rests: usize,
    /// Ties read (each counted at the note it starts on).
    pub ties: usize,
    /// Repeats as (measure index, start?) — the score model has no repeat
    /// barlines yet, so they ride here.
    pub repeats: Vec<(usize, bool)>,
    /// Measures whose content does not add up to the meter, and glyphs
    /// nothing claimed.
    pub warnings: Vec<String>,
}

// ───────────────────────── font tables ─────────────────────────

/// What a music-font character is (Maestro layout: Helsinki, Opus).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sym {
    Clef(ClefSign),
    Head(Head),
    Rest(RestKind),
    Flag(u8),
    Accidental(i8),
    Dot,
    Digit(u32),
    Staccato,
    Brace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Head {
    Black,
    Half,
    Whole,
    /// A rhythm slash (strum): its length comes from the bar.
    Slash,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RestKind {
    Measure,
    Of(NoteTypeValue),
}

fn is_music_font(font: &str) -> bool {
    let f = font.to_ascii_lowercase();
    // The music fonts, not their text companions (HelsinkiText carries the
    // metronome mark's note and is read as text).
    (f.contains("helsinki") || f.contains("opus") || f.contains("maestro"))
        && !f.contains("text")
        && !f.contains("chord")
}

fn is_chord_font(font: &str) -> bool {
    font.to_ascii_lowercase().contains("chord")
}

fn classify(g: &Glyph) -> Option<Sym> {
    if !is_music_font(&g.font) {
        return None;
    }
    let special = g.font.to_ascii_lowercase().contains("special");
    Some(match (special, g.ch) {
        (true, '{') => Sym::Brace,
        (true, '™') | (false, 'k') => Sym::Dot,
        (true, 'V') => Sym::Head(Head::Slash),
        (_, '&') => Sym::Clef(ClefSign::G),
        (_, '?') => Sym::Clef(ClefSign::F),
        (_, 'B') => Sym::Clef(ClefSign::C),
        (_, 'œ') => Sym::Head(Head::Black),
        // × notehead (dead / muted note): a black head for rhythm and pitch.
        (_, '¿') => Sym::Head(Head::Black),
        (_, '˙') => Sym::Head(Head::Half),
        (_, 'w') => Sym::Head(Head::Whole),
        (_, '∑') => Sym::Rest(RestKind::Measure),
        (_, 'Ó') => Sym::Rest(RestKind::Of(NoteTypeValue::Half)),
        (_, 'Œ') => Sym::Rest(RestKind::Of(NoteTypeValue::Quarter)),
        (_, '‰') => Sym::Rest(RestKind::Of(NoteTypeValue::Eighth)),
        (_, '≈') => Sym::Rest(RestKind::Of(NoteTypeValue::Sixteenth)),
        (_, '®') => Sym::Rest(RestKind::Of(NoteTypeValue::ThirtySecond)),
        (_, 'j' | 'J') => Sym::Flag(1),
        (_, 'r' | 'R') => Sym::Flag(2),
        (_, 'x' | 'X') => Sym::Flag(3),
        (_, '#') => Sym::Accidental(1),
        (_, 'b') => Sym::Accidental(-1),
        (_, 'n') => Sym::Accidental(0),
        (_, 'Ü') => Sym::Accidental(2),
        (_, 'º') => Sym::Accidental(-2),
        (_, '.') => Sym::Staccato,
        (_, c @ '0'..='9') => Sym::Digit(c as u32 - '0' as u32),
        _ => return None,
    })
}

/// Opus Chords: the legacy characters its ToUnicode map gives for chord
/// suffix pieces, back to plain chord text.
fn chord_char(c: char) -> &'static str {
    match c {
        '‹' | 'Œ' => "m",
        '„' => "a",
        'Š' => "j",
        '©' => "#",
        '¨' => "b",
        '“' => "sus",
        'ˆ' => "d",
        _ => "",
    }
}

// ───────────────────────── geometry ─────────────────────────

#[derive(Debug, Clone, Copy)]
struct Staff {
    /// y of the top line; lines go down by `sp`.
    top: f64,
    sp: f64,
    x0: f64,
    x1: f64,
}

impl Staff {
    fn bottom(&self) -> f64 {
        self.top + 4.0 * self.sp
    }
    fn mid(&self) -> f64 {
        self.top + 2.0 * self.sp
    }
    /// Half-spaces above the bottom line (0 = bottom line, 8 = top line).
    fn position(&self, y: f64) -> i32 {
        ((self.bottom() - y) / (self.sp / 2.0)).round() as i32
    }
}

#[derive(Debug, Clone, Copy)]
struct VLine {
    x: f64,
    y0: f64,
    y1: f64,
    width: f64,
}

#[derive(Debug, Clone, Copy)]
struct Beam {
    x0: f64,
    x1: f64,
    /// Centre-line y at x0 and x1.
    y0: f64,
    y1: f64,
}

impl Beam {
    fn y_at(&self, x: f64) -> f64 {
        if (self.x1 - self.x0).abs() < 1e-6 {
            self.y0
        } else {
            self.y0 + (self.y1 - self.y0) * (x - self.x0) / (self.x1 - self.x0)
        }
    }
}

#[derive(Debug, Clone)]
struct BarGroup {
    x0: f64,
    x1: f64,
    thick: Vec<bool>,
    dots_left: bool,
    dots_right: bool,
}

impl BarGroup {
    fn style(&self) -> BarStyle {
        match self.thick.as_slice() {
            [false] => BarStyle::Regular,
            [false, false] => BarStyle::LightLight,
            [false, true] => BarStyle::LightHeavy,
            [true, false] => BarStyle::HeavyLight,
            [true] => BarStyle::LightHeavy,
            _ => BarStyle::Regular,
        }
    }
}

/// A word of text: glyphs of one font on one baseline, closely spaced.
#[derive(Debug, Clone)]
struct Word {
    text: String,
    font: String,
    x0: f64,
    x1: f64,
    y: f64,
    size: f64,
}

fn staves_of(page: &Page) -> Vec<Staff> {
    let mut lines: Vec<(f64, f64, f64)> = page
        .strokes
        .iter()
        .filter_map(|s| as_segment(&s.path))
        .filter(|(a, b)| (a.y - b.y).abs() < 0.05 && (b.x - a.x).abs() > 50.0)
        .map(|(a, b)| (a.y, a.x.min(b.x), a.x.max(b.x)))
        .collect();
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines.dedup_by(|a, b| (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 1.0);
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < lines.len() {
        let w = &lines[i..i + 5];
        let gap = w[1].0 - w[0].0;
        let even = w.windows(2).all(|p| ((p[1].0 - p[0].0) - gap).abs() < 0.08 * gap);
        let aligned = w.iter().all(|l| (l.1 - w[0].1).abs() < 2.0 && (l.2 - w[0].2).abs() < 2.0);
        if even && aligned && gap > 2.0 && gap < 20.0 {
            out.push(Staff {
                top: w[0].0,
                sp: gap,
                x0: w[0].1,
                x1: w[0].2,
            });
            i += 5;
        } else {
            i += 1;
        }
    }
    out
}

fn vlines(page: &Page) -> Vec<VLine> {
    page.strokes
        .iter()
        .filter_map(|s| as_segment(&s.path).map(|seg| (seg, s.width)))
        .filter(|((a, b), _)| (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() > 1.0)
        .map(|((a, b), w)| VLine {
            x: a.x,
            y0: a.y.min(b.y),
            y1: a.y.max(b.y),
            width: w,
        })
        .collect()
}

fn beams_of(page: &Page, sp: f64) -> Vec<Beam> {
    page.fills
        .iter()
        .filter_map(|f| {
            let els = f.path.elements();
            let curved = els.iter().any(|e| {
                matches!(e, kurbo::PathEl::CurveTo(..) | kurbo::PathEl::QuadTo(..))
            });
            let b = f.path.bounding_box();
            if curved || els.len() > 7 || b.width() < 1.2 * sp {
                return None;
            }
            // The parallelogram's left and right edges: mid-heights there.
            let pts: Vec<Point> = els
                .iter()
                .filter_map(|e| match e {
                    kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => Some(*p),
                    _ => None,
                })
                .collect();
            let left: Vec<&Point> = pts.iter().filter(|p| (p.x - b.x0).abs() < 0.3).collect();
            let right: Vec<&Point> = pts.iter().filter(|p| (p.x - b.x1).abs() < 0.3).collect();
            if left.len() < 2 || right.len() < 2 {
                return None;
            }
            let mid = |v: &[&Point]| v.iter().map(|p| p.y).sum::<f64>() / v.len() as f64;
            let thick = (left.iter().map(|p| p.y).fold(f64::MIN, f64::max)
                - left.iter().map(|p| p.y).fold(f64::MAX, f64::min))
            .abs();
            (thick > 0.25 * sp && thick < 1.0 * sp).then(|| Beam {
                x0: b.x0,
                x1: b.x1,
                y0: mid(&left),
                y1: mid(&right),
            })
        })
        .collect()
}

fn words_of(glyphs: &[&Glyph]) -> Vec<Word> {
    let mut gs: Vec<&&Glyph> = glyphs.iter().collect();
    gs.sort_by(|a, b| {
        (a.origin.y.round() as i64, a.origin.x.total_cmp(&b.origin.x))
            .partial_cmp(&(b.origin.y.round() as i64, std::cmp::Ordering::Equal))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.origin.x.total_cmp(&b.origin.x))
    });
    let mut words: Vec<Word> = Vec::new();
    for g in gs {
        let adv = if g.advance > 0.0 { g.advance } else { 0.5 * g.size };
        // In a chord font a capital A-G not after '/' is a new chord's
        // root, however tight the spacing ("F#add2" then "G#m").
        let new_root = is_chord_font(&g.font)
            && ('A'..='G').contains(&g.ch)
            && words.last().is_some_and(|w| !w.text.ends_with('/'));
        if let Some(w) = words.last_mut()
            && !new_root
            && (w.y - g.origin.y).abs() < 0.3 * g.size
            && g.origin.x - w.x1 < 0.35 * g.size
            && g.origin.x >= w.x0
            && w.font == g.font
        {
            w.text.push(g.ch);
            w.x1 = w.x1.max(g.origin.x + adv);
            continue;
        }
        words.push(Word {
            text: g.ch.to_string(),
            font: g.font.clone(),
            x0: g.origin.x,
            x1: g.origin.x + adv,
            y: g.origin.y,
            size: g.size,
        });
    }
    words
}

/// Words on one baseline joined into lines (a space where the gap is a
/// word space, nothing where it is kerning).
fn lines_of(mut words: Vec<Word>) -> Vec<Word> {
    words.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x0.total_cmp(&b.x0)));
    let mut out: Vec<Word> = Vec::new();
    for w in words {
        if let Some(l) = out.last_mut()
            && (l.y - w.y).abs() < 0.3 * w.size
            && w.x0 >= l.x0
            && w.x0 - l.x1 < 1.2 * w.size
            && !is_chord_font(&l.font)
            && !is_chord_font(&w.font)
        {
            if w.x0 - l.x1 > 0.12 * w.size && !l.text.ends_with(' ') {
                l.text.push(' ');
            }
            l.text.push_str(&w.text);
            l.x1 = l.x1.max(w.x1);
            l.size = l.size.max(w.size);
            continue;
        }
        out.push(w);
    }
    for l in &mut out {
        l.text = l.text.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    out.retain(|l| !l.text.is_empty());
    out
}

/// Ties: filled crescents between two noteheads at one height, from the
/// middle of the first to the middle of the second (one space off the heads,
/// away from the stems). A tie running off a line's end starts at its left
/// head only; one coming in at a line's start ends at its right head only.
/// Slurs join different pitches and match nothing here.
/// Returns (heads a tie starts on, heads a tie ends on) as origins.
fn ties_of(page: &Page, staves: &[Staff], sp: f64) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let heads: Vec<(f64, f64)> = page
        .glyphs
        .iter()
        .filter(|g| matches!(classify(g), Some(Sym::Head(_))))
        .map(|g| (g.origin.x, g.origin.y))
        .collect();
    let (mut starts, mut stops) = (Vec::new(), Vec::new());
    for f in &page.fills {
        let curved = f
            .path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::CurveTo(..)))
            .count();
        let b = f.path.bounding_box();
        if curved < 2 || b.width() < 0.5 * sp || b.height() > 1.6 * sp {
            continue;
        }
        let mid_y = (b.y0 + b.y1) / 2.0;
        // A head whose origin sits within a notehead's width left of the
        // curve's end, at a height near the curve.
        let near = |x_end: f64| -> Vec<(f64, f64)> {
            heads
                .iter()
                .copied()
                .filter(|h| h.0 <= x_end && x_end - h.0 < 1.6 * sp && (h.1 - mid_y).abs() < 2.2 * sp)
                .collect()
        };
        let lefts = near(b.x0);
        // The right end sits over its head, or (Sibelius draws a tie into
        // a chord in pieces) stops just short of it.
        let rights: Vec<(f64, f64)> = heads
            .iter()
            .copied()
            .filter(|h| h.0 > b.x1 - 1.6 * sp && h.0 < b.x1 + 0.8 * sp && (h.1 - mid_y).abs() < 2.2 * sp)
            .collect();
        let pair = lefts
            .iter()
            .flat_map(|l| rights.iter().map(move |r| (*l, *r)))
            .filter(|(l, r)| r.0 > l.0 + 0.5 * sp && (l.1 - r.1).abs() < 0.3 * sp)
            .min_by(|a, b| (a.0.1 - mid_y).abs().total_cmp(&(b.0.1 - mid_y).abs()));
        if let Some((l, r)) = pair {
            starts.push(l);
            stops.push(r);
            continue;
        }
        // A short piece ending just before a head is the end of a tie drawn
        // in pieces: the head it reaches is tied from the note before (the
        // pairing pass finds which).
        if b.width() < 2.5 * sp
            && let Some(r) = heads
                .iter()
                .copied()
                .filter(|h| h.0 >= b.x1 - 0.2 * sp && h.0 < b.x1 + 0.8 * sp)
                .min_by(|a, c| (a.1 - mid_y).abs().total_cmp(&(c.1 - mid_y).abs()))
            && (r.1 - mid_y).abs() < 1.6 * sp
        {
            stops.push(r);
            continue;
        }
        // Over a system break: only one end has a head.
        let staff = staves.iter().find(|s| mid_y > s.top - 3.0 * sp && mid_y < s.bottom() + 3.0 * sp);
        if let Some(st) = staff {
            if b.x1 > st.x1 - 2.0 * sp
                && let Some(l) = lefts.iter().min_by(|a, b| (a.1 - mid_y).abs().total_cmp(&(b.1 - mid_y).abs()))
            {
                starts.push(*l);
            } else if b.x0 < st.x0 + 12.0 * sp
                && let Some(r) = rights.iter().min_by(|a, b| (a.1 - mid_y).abs().total_cmp(&(b.1 - mid_y).abs()))
            {
                stops.push(*r);
            }
        }
    }
    (starts, stops)
}

// ───────────────────────── recognition ─────────────────────────

#[derive(Debug, Clone)]
struct Item {
    g: Glyph,
    sym: Sym,
    staff: usize,
    used: bool,
}

/// A note or rest event before ticks are assigned.
#[derive(Debug, Clone)]
struct Ev {
    x: f64,
    staff_in_system: u8,
    /// Stem direction (None: no stem, or a rest).
    up: Option<bool>,
    /// Vertical centre, for placing rests in a voice.
    y: f64,
    kind: EvKind,
}

#[derive(Debug, Clone)]
enum EvKind {
    Chord {
        heads: Vec<(f64, f64)>, // (x, y) of each notehead
        written: WrittenDuration,
        accidentals: Vec<Option<i8>>,
        staccato: bool,
        /// A rhythm slash: an attack with no pitch of its own.
        slash: bool,
    },
    Rest {
        written: WrittenDuration,
        measure: bool,
    },
}

fn quarters_of(t: NoteTypeValue) -> f64 {
    match t {
        NoteTypeValue::Whole => 4.0,
        NoteTypeValue::Half => 2.0,
        NoteTypeValue::Quarter => 1.0,
        NoteTypeValue::Eighth => 0.5,
        NoteTypeValue::Sixteenth => 0.25,
        NoteTypeValue::ThirtySecond => 0.125,
        NoteTypeValue::SixtyFourth => 0.0625,
        NoteTypeValue::Breve => 8.0,
        _ => 0.0,
    }
}

fn ticks_of(w: &WrittenDuration) -> u32 {
    let base = w.note_type.map_or(0.0, quarters_of);
    let dotted = match w.dots {
        0 => base,
        1 => base * 1.5,
        2 => base * 1.75,
        _ => base * 1.875,
    };
    let tup = w.tuplet.map_or(1.0, |(a, n)| f64::from(n) / f64::from(a));
    (dotted * tup * f64::from(DIVISIONS)).round() as u32
}

fn shorter(t: NoteTypeValue, by: u8) -> NoteTypeValue {
    const ORDER: [NoteTypeValue; 7] = [
        NoteTypeValue::Whole,
        NoteTypeValue::Half,
        NoteTypeValue::Quarter,
        NoteTypeValue::Eighth,
        NoteTypeValue::Sixteenth,
        NoteTypeValue::ThirtySecond,
        NoteTypeValue::SixtyFourth,
    ];
    let i = ORDER.iter().position(|o| *o == t).unwrap_or(2);
    ORDER[(i + usize::from(by)).min(ORDER.len() - 1)]
}

/// Bottom-line diatonic number (C0 = 0) for a clef.
fn bottom_line(clef: ClefSign) -> i32 {
    match clef {
        ClefSign::F => 2 * 7 + 4, // G2
        ClefSign::C => 3 * 7 + 3, // F3 (alto)
        _ => 4 * 7 + 2,           // E4
    }
}

fn step_of(d: i32) -> Step {
    [Step::C, Step::D, Step::E, Step::F, Step::G, Step::A, Step::B][d.rem_euclid(7) as usize]
}

/// Key-signature alteration per step for a circle-of-fifths count.
fn key_alter(fifths: i8, step: Step) -> i8 {
    const SHARPS: [Step; 7] = [Step::F, Step::C, Step::G, Step::D, Step::A, Step::E, Step::B];
    const FLATS: [Step; 7] = [Step::B, Step::E, Step::A, Step::D, Step::G, Step::C, Step::F];
    if fifths > 0 {
        i8::from(SHARPS[..fifths as usize].contains(&step))
    } else if fifths < 0 {
        -i8::from(FLATS[..(-fifths) as usize].contains(&step))
    } else {
        0
    }
}

/// Read every page into one score (one part; a grand staff is one part of
/// two staves).
#[must_use]
pub fn recognize(pages: &[Page]) -> Recognized {
    let mut report = Report::default();
    let mut measures: Vec<Measure> = Vec::new();
    // Each staff's accidentals at the end of the last measure, for a note
    // tied over the barline (it keeps its alteration, unprinted).
    let mut prev_alters: BTreeMap<(u8, i32), i8> = BTreeMap::new();
    let mut title: Option<String> = None;
    let mut part_staves: u8 = 1;
    // Running state across systems and pages.
    let mut clefs: Vec<ClefSign> = Vec::new();
    let mut key: Option<i8> = None;
    let mut time: Option<(u32, u32)> = None;

    for page in pages {
        let staves = staves_of(page);
        if staves.is_empty() {
            continue;
        }
        report.staves += staves.len();
        let sp = staves[0].sp;
        let vl = vlines(page);
        let beams = beams_of(page, sp);
        let (tie_starts, tie_stops) = ties_of(page, &staves, sp);

        // Barlines: from some staff's top to some staff's bottom.
        // A barline ends on the staff lines; a stem that happens to end
        // a quarter-space off them is still a stem.
        let at_line = |y: f64, line: f64| (y - line).abs() < 0.2 * sp;
        let mut barlines: Vec<(VLine, usize, usize)> = Vec::new();
        let mut stems: Vec<VLine> = Vec::new();
        for v in &vl {
            let top = staves.iter().position(|s| at_line(v.y0, s.top));
            let bot = staves.iter().rposition(|s| at_line(v.y1, s.bottom()));
            match (top, bot) {
                (Some(t), Some(b)) if b >= t => barlines.push((*v, t, b)),
                _ if v.width < 0.25 * sp && v.y1 - v.y0 > 2.0 * sp => stems.push(*v),
                _ => {}
            }
        }
        // Systems: staves joined by a barline crossing between them.
        let mut sys_of: Vec<usize> = (0..staves.len()).collect();
        for (_, t, b) in &barlines {
            for k in *t..=*b {
                sys_of[k] = sys_of[*t];
            }
        }
        let mut systems: Vec<Vec<usize>> = Vec::new();
        for (i, s) in sys_of.iter().enumerate() {
            if i == 0 || *s != sys_of[i - 1] {
                systems.push(vec![i]);
            } else if let Some(last) = systems.last_mut() {
                last.push(i);
            }
        }
        report.systems += systems.len();
        part_staves = part_staves.max(systems.iter().map(Vec::len).max().unwrap_or(1) as u8);

        // Music glyphs, each assigned to its nearest staff.
        let mut items: Vec<Item> = page
            .glyphs
            .iter()
            .filter_map(|g| {
                let sym = classify(g)?;
                let staff = staves
                    .iter()
                    .enumerate()
                    .min_by(|a, b| {
                        (a.1.mid() - g.origin.y).abs().total_cmp(&(b.1.mid() - g.origin.y).abs())
                    })?
                    .0;
                Some(Item {
                    g: g.clone(),
                    sym,
                    staff,
                    used: false,
                })
            })
            .collect();

        // Text: chord symbols and words.
        // Opus overprints some chord glyphs (drawn twice on one spot): keep
        // one of each.
        let mut chord_glyphs: Vec<&Glyph> = Vec::new();
        for g in page.glyphs.iter().filter(|g| is_chord_font(&g.font)) {
            let twin = chord_glyphs.iter().any(|h| {
                h.ch == g.ch && (h.origin.x - g.origin.x).abs() < 0.3 && (h.origin.y - g.origin.y).abs() < 0.3
            });
            if !twin {
                chord_glyphs.push(g);
            }
        }
        let mut chords: Vec<Word> = words_of(&chord_glyphs);
        for c in &mut chords {
            c.text = c
                .text
                .chars()
                .map(|ch| {
                    let m = chord_char(ch);
                    if m.is_empty() { ch.to_string() } else { m.to_string() }
                })
                .collect();
        }
        // Overprinted duplicates (Opus draws some pieces twice).
        chords.dedup_by(|a, b| a.text == b.text && (a.x0 - b.x0).abs() < 0.5 && (a.y - b.y).abs() < 0.5);
        let text_glyphs: Vec<&Glyph> = page
            .glyphs
            .iter()
            .filter(|g| !is_music_font(&g.font) && !is_chord_font(&g.font))
            .collect();
        let lines = lines_of(words_of(&text_glyphs));
        let first_top = staves[0].top;
        if title.is_none() {
            title = lines
                .iter()
                .filter(|l| l.y < first_top)
                .max_by(|a, b| a.size.total_cmp(&b.size))
                .map(|l| l.text.clone());
        }

        for sys in &systems {
            let first = staves[sys[0]];
            if clefs.len() < sys.len() {
                clefs.resize(sys.len(), ClefSign::G);
            }
            // Bar groups for this system.
            // A barline crosses every staff of its system — as one line or a
            // line per staff at one x. A line on one staff of a grand staff
            // with nothing at that x on the other is a stem.
            let in_sys: Vec<&(VLine, usize, usize)> = barlines
                .iter()
                .filter(|(_, t, b)| sys.contains(t) && sys.contains(b))
                .collect();
            let covered = |x: f64| -> bool {
                sys.iter().all(|si| {
                    in_sys
                        .iter()
                        .any(|(v, t, b)| (v.x - x).abs() < 0.6 * sp && t <= si && si <= b)
                })
            };
            let mut xs: Vec<(f64, f64)> = in_sys
                .iter()
                .filter(|(v, _, _)| covered(v.x))
                .map(|(v, _, _)| (v.x, v.width))
                .collect();
            xs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut groups: Vec<BarGroup> = Vec::new();
            for (x, w) in xs {
                let thick = w > 1.5 * 0.78f64.max(0.15 * sp);
                if let Some(g) = groups.last_mut()
                    && x - g.x1 < 1.2 * sp
                {
                    if (x - g.x1).abs() > 0.05 {
                        g.thick.push(thick);
                    }
                    g.x1 = x;
                    continue;
                }
                groups.push(BarGroup {
                    x0: x,
                    x1: x,
                    thick: vec![thick],
                    dots_left: false,
                    dots_right: false,
                });
            }
            // The system's opening line is not a measure boundary.
            groups.retain(|g| g.x1 - first.x0 > 0.5 * sp || g.thick.len() > 1);
            for it in items.iter_mut().filter(|i| sys.contains(&i.staff) && i.sym == Sym::Dot) {
                for g in &mut groups {
                    if it.g.origin.x > g.x1 && it.g.origin.x - g.x1 < 1.5 * sp {
                        g.dots_right = true;
                        it.used = true;
                    } else if it.g.origin.x < g.x0 && g.x0 - it.g.origin.x < 1.5 * sp {
                        g.dots_left = true;
                        it.used = true;
                    }
                }
            }

            // Clefs, key and time at the system start (and wherever they
            // change).
            for (k, &si) in sys.iter().enumerate() {
                if let Some(c) = items
                    .iter_mut()
                    .filter(|i| i.staff == si && matches!(i.sym, Sym::Clef(_)))
                    .min_by(|a, b| a.g.origin.x.total_cmp(&b.g.origin.x))
                {
                    if let Sym::Clef(sign) = c.sym {
                        clefs[k] = sign;
                    }
                    c.used = true;
                }
            }
            let first_event_x = items
                .iter()
                .filter(|i| sys.contains(&i.staff) && matches!(i.sym, Sym::Head(_) | Sym::Rest(_)))
                .map(|i| i.g.origin.x)
                .fold(f64::MAX, f64::min);
            // Key signature: accidentals before the first event of the top
            // staff that no notehead claims.
            let key_accs: Vec<i8> = items
                .iter()
                .filter(|i| i.staff == sys[0] && i.g.origin.x < first_event_x)
                .filter_map(|i| match i.sym {
                    Sym::Accidental(a) if a != 0 => Some(a),
                    _ => None,
                })
                .collect();
            for it in items
                .iter_mut()
                .filter(|i| sys.contains(&i.staff) && i.g.origin.x < first_event_x)
                .filter(|i| matches!(i.sym, Sym::Accidental(_)))
            {
                it.used = true;
            }
            if !key_accs.is_empty() || key.is_none() {
                key = Some(key_accs.iter().map(|a| a.signum()).sum::<i8>());
            }
            // Time signature: digits stacked on the top staff.
            let digits: Vec<(f64, f64, u32)> = items
                .iter()
                .filter(|i| i.staff == sys[0])
                .filter_map(|i| match i.sym {
                    Sym::Digit(d) => Some((i.g.origin.x, i.g.origin.y, d)),
                    _ => None,
                })
                .collect();
            let mut new_time = None;
            if !digits.is_empty() {
                let mid = first.mid();
                let num: String = digits.iter().filter(|d| d.1 < mid).map(|d| char::from_digit(d.2, 10).unwrap_or('4')).collect();
                let den: String = digits.iter().filter(|d| d.1 >= mid).map(|d| char::from_digit(d.2, 10).unwrap_or('4')).collect();
                if let (Ok(n), Ok(d)) = (num.parse::<u32>(), den.parse::<u32>()) {
                    new_time = Some((n, d));
                }
                for it in items.iter_mut().filter(|i| sys.contains(&i.staff) && matches!(i.sym, Sym::Digit(_))) {
                    it.used = true;
                }
            }
            // Measure spans: system start, then between bar groups.
            let mut bounds: Vec<(f64, Option<BarGroup>)> = vec![(first.x0, None)];
            for g in &groups {
                bounds.push((g.x1, Some(g.clone())));
            }
            if bounds.last().is_some_and(|(x, _)| (first.x1 - x).abs() > sp) {
                bounds.push((first.x1, None));
            }
            let mut first_in_system = true;
            for w in bounds.windows(2) {
                let (x0, ref open) = w[0];
                let (x1, ref close) = w[1];
                let inside = |x: f64| x > x0 && x < x1;
                let has_events = items.iter().any(|i| {
                    sys.contains(&i.staff) && matches!(i.sym, Sym::Head(_) | Sym::Rest(_)) && inside(i.g.origin.x)
                });
                if !has_events {
                    continue; // the prelude
                }
                let index = measures.len();
                // The first measure of a system also owns the text printed
                // over the system's prelude.
                let span_start = if first_in_system { first.x0 } else { x0 };
                let repeat_start = open.as_ref().is_some_and(|g| g.dots_right);
                let repeat_end = close.as_ref().is_some_and(|g| g.dots_left);
                if repeat_start {
                    report.repeats.push((index, true));
                }
                if repeat_end {
                    report.repeats.push((index, false));
                }
                let mut m = Measure {
                    number: (index + 1).to_string(),
                    divisions: DIVISIONS,
                    repeat_start,
                    repeat_end,
                    barline: close.as_ref().map_or(BarStyle::Regular, BarGroup::style),
                    ..Measure::default()
                };
                if index == 0 || first_in_system {
                    let mut a = AttrChange {
                        tick: 0,
                        ..AttrChange::default()
                    };
                    if index == 0 {
                        a.divisions = Some(DIVISIONS);
                        a.staves = Some(sys.len() as u8);
                        a.key_fifths = key;
                        a.time = new_time.or(Some((4, 4)));
                        a.clefs = clefs
                            .iter()
                            .take(sys.len())
                            .enumerate()
                            .map(|(k, s)| {
                                (k as u8 + 1, Clef { sign: *s, line: None, octave_change: 0 })
                            })
                            .collect();
                        time = a.time;
                    } else if new_time.is_some() && new_time != time {
                        a.time = new_time;
                        time = new_time;
                    }
                    if a.divisions.is_some() || a.time.is_some() {
                        m.attributes.push(a);
                    }
                }
                first_in_system = false;
                let (tn, td) = time.unwrap_or((4, 4));
                m.len_ticks = DIVISIONS * 4 * tn / td;

                // Events per staff.
                let mut events: Vec<Ev> = Vec::new();
                for (k, &si) in sys.iter().enumerate() {
                    let staff = staves[si];
                    let in_m: Vec<usize> = (0..items.len())
                        .filter(|&i| items[i].staff == si && inside(items[i].g.origin.x))
                        .collect();
                    let head_w = 1.2 * sp;
                    // Stems near this staff in this measure.
                    let my_stems: Vec<VLine> = stems
                        .iter()
                        .filter(|s| inside(s.x) && s.y1 > staff.top - 5.0 * sp && s.y0 < staff.bottom() + 5.0 * sp)
                        .copied()
                        .collect();
                    // Group noteheads by stem.
                    let mut by_stem: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                    let mut stemless: Vec<usize> = Vec::new();
                    for &i in &in_m {
                        let Sym::Head(h) = items[i].sym else { continue };
                        let g = &items[i].g;
                        let hw = if g.advance > 0.0 { g.advance } else { head_w };
                        let found = my_stems.iter().position(|s| {
                            let at_edge = (s.x - g.origin.x).abs() < 0.35 * sp
                                || (s.x - (g.origin.x + hw)).abs() < 0.45 * sp;
                            at_edge && g.origin.y > s.y0 - 0.7 * sp && g.origin.y < s.y1 + 0.7 * sp
                        });
                        match (h, found) {
                            (Head::Whole, _) | (_, None) => stemless.push(i),
                            (_, Some(s)) => by_stem.entry(s).or_default().push(i),
                        }
                    }
                    let make_chord = |heads: Vec<usize>, stem: Option<VLine>, items: &mut Vec<Item>| {
                        let head = match items[heads[0]].sym {
                            Sym::Head(h) => h,
                            _ => Head::Black,
                        };
                        let mut stem_up = None;
                        let mut note_type = match head {
                            Head::Whole => NoteTypeValue::Whole,
                            Head::Half => NoteTypeValue::Half,
                            Head::Black | Head::Slash => NoteTypeValue::Quarter,
                        };
                        if let Some(s) = stem {
                            // The stem's far end: away from its noteheads.
                            let hy: f64 = heads.iter().map(|&i| items[i].g.origin.y).sum::<f64>() / heads.len() as f64;
                            let up = (s.y0 - hy).abs() > (s.y1 - hy).abs();
                            stem_up = Some(up);
                            let tip = if up { s.y0 } else { s.y1 };
                            let n_beams = beams
                                .iter()
                                .filter(|b| s.x > b.x0 - 0.3 && s.x < b.x1 + 0.3)
                                .filter(|b| {
                                    let y = b.y_at(s.x);
                                    y > s.y0 - 0.6 * sp && y < s.y1 + 0.6 * sp && (y - tip).abs() < 2.5 * sp
                                })
                                .count();
                            let mut n_flags = 0;
                            for it in items.iter_mut() {
                                if let Sym::Flag(n) = it.sym
                                    && !it.used
                                    && (it.g.origin.x - s.x).abs() < 0.6 * sp
                                    && (it.g.origin.y - tip).abs() < 1.6 * sp
                                {
                                    n_flags += n;
                                    it.used = true;
                                }
                            }
                            note_type = shorter(note_type, n_beams as u8 + n_flags);
                        }
                        // Dots right of the rightmost notehead.
                        let rx = heads.iter().map(|&i| items[i].g.origin.x).fold(f64::MIN, f64::max) + head_w;
                        let head_ys: Vec<f64> = heads.iter().map(|&i| items[i].g.origin.y).collect();
                        // Every notehead of a dotted chord carries its own
                        // dot: count dot COLUMNS, not dots.
                        let mut dot_xs: Vec<f64> = Vec::new();
                        for it in items.iter_mut() {
                            if it.sym == Sym::Dot
                                && !it.used
                                && it.g.origin.x > rx - 0.2 * sp
                                && it.g.origin.x < rx + 1.4 * sp
                                && head_ys.iter().any(|hy| (hy - it.g.origin.y).abs() < 0.8 * sp)
                            {
                                if !dot_xs.iter().any(|x| (x - it.g.origin.x).abs() < 0.4 * sp) {
                                    dot_xs.push(it.g.origin.x);
                                }
                                it.used = true;
                            }
                        }
                        let dots = dot_xs.len().min(3) as u8;
                        let mut hs: Vec<(f64, f64)> = heads.iter().map(|&i| (items[i].g.origin.x, items[i].g.origin.y)).collect();
                        hs.sort_by(|a, b| b.1.total_cmp(&a.1));
                        let mut accidentals = vec![None; hs.len()];
                        for (n, (hx, hy)) in hs.iter().enumerate() {
                            for it in items.iter_mut() {
                                if let Sym::Accidental(a) = it.sym
                                    && !it.used
                                    && it.g.origin.x < *hx
                                    && *hx - it.g.origin.x < 3.0 * sp
                                    && (it.g.origin.y - hy).abs() < 0.3 * sp
                                {
                                    accidentals[n] = Some(a);
                                    it.used = true;
                                }
                            }
                        }
                        let x = hs.iter().map(|h| h.0).fold(f64::MAX, f64::min);
                        let mut staccato = false;
                        for it in items.iter_mut() {
                            if it.sym == Sym::Staccato && !it.used && (it.g.origin.x - x).abs() < 1.0 * sp {
                                if hs.iter().any(|h| (h.1 - it.g.origin.y).abs() < 2.5 * sp) {
                                    staccato = true;
                                    it.used = true;
                                }
                            }
                        }
                        for &i in &heads {
                            items[i].used = true;
                        }
                        let y = hs.iter().map(|h| h.1).sum::<f64>() / hs.len() as f64;
                        Ev {
                            x,
                            staff_in_system: k as u8 + 1,
                            up: stem_up,
                            y,
                            kind: EvKind::Chord {
                                heads: hs,
                                written: WrittenDuration { note_type: Some(note_type), dots, tuplet: None },
                                accidentals,
                                staccato,
                                slash: head == Head::Slash,
                            },
                        }
                    };
                    for (s, heads) in by_stem {
                        let stem = my_stems[s];
                        events.push(make_chord(heads, Some(stem), &mut items));
                    }
                    // Stemless noteheads (whole notes) stacked at one x are
                    // one chord.
                    stemless.sort_by(|a, b| items[*a].g.origin.x.total_cmp(&items[*b].g.origin.x));
                    let mut stacks: Vec<Vec<usize>> = Vec::new();
                    for i in stemless {
                        match stacks.last_mut() {
                            Some(st) if (items[st[0]].g.origin.x - items[i].g.origin.x).abs() < 0.8 * sp => st.push(i),
                            _ => stacks.push(vec![i]),
                        }
                    }
                    for st in stacks {
                        events.push(make_chord(st, None, &mut items));
                    }
                    // Rests.
                    for &i in &in_m {
                        let Sym::Rest(kind) = items[i].sym else { continue };
                        let g = items[i].g.clone();
                        let mut dots = 0u8;
                        for it in items.iter_mut() {
                            if it.sym == Sym::Dot
                                && !it.used
                                && it.g.origin.x > g.origin.x
                                && it.g.origin.x - g.origin.x < 2.0 * sp
                                && (it.g.origin.y - g.origin.y).abs() < 1.5 * sp
                            {
                                dots += 1;
                                it.used = true;
                            }
                        }
                        items[i].used = true;
                        events.push(Ev {
                            x: g.origin.x,
                            staff_in_system: k as u8 + 1,
                            up: None,
                            y: g.origin.y,
                            kind: match kind {
                                RestKind::Measure => EvKind::Rest {
                                    written: WrittenDuration { note_type: Some(NoteTypeValue::Whole), dots: 0, tuplet: None },
                                    measure: true,
                                },
                                RestKind::Of(t) => EvKind::Rest {
                                    written: WrittenDuration { note_type: Some(t), dots, tuplet: None },
                                    measure: false,
                                },
                            },
                        });
                    }
                }

                // Rhythm slashes carry no length of their own: the bar's
                // time left after its notes and rests, shared equally — when
                // that comes out to a written value (six slashes and two
                // eighth rests in 4/4 are eighths).
                for k in 1..=sys.len() as u8 {
                    let is_slash = |e: &Ev| matches!(&e.kind, EvKind::Chord { slash: true, .. });
                    let count = events.iter().filter(|e| e.staff_in_system == k && is_slash(e)).count() as u32;
                    if count == 0 {
                        continue;
                    }
                    let other: u32 = events
                        .iter()
                        .filter(|e| e.staff_in_system == k && !is_slash(e))
                        .map(|e| ev_ticks(e, m.len_ticks))
                        .sum();
                    let left = m.len_ticks.saturating_sub(other);
                    let each = if left % count == 0 { left / count } else { 0 };
                    let value = match each {
                        x if x == DIVISIONS * 4 => Some(NoteTypeValue::Whole),
                        x if x == DIVISIONS * 2 => Some(NoteTypeValue::Half),
                        x if x == DIVISIONS => Some(NoteTypeValue::Quarter),
                        x if x == DIVISIONS / 2 => Some(NoteTypeValue::Eighth),
                        x if x == DIVISIONS / 4 => Some(NoteTypeValue::Sixteenth),
                        _ => None,
                    };
                    if let Some(v) = value {
                        for e in events.iter_mut().filter(|e| e.staff_in_system == k) {
                            if let EvKind::Chord { slash: true, written, .. } = &mut e.kind {
                                written.note_type = Some(v);
                            }
                        }
                    }
                }

                // Ticks, pitches, per staff; the top staff's (x, tick) pairs
                // place the text.
                let mut onsets: Vec<(f64, u32)> = Vec::new();
                let mut alters: BTreeMap<(u8, i32), i8> = BTreeMap::new();
                let at = |list: &[(f64, f64)], x: f64, y: f64| {
                    list.iter().any(|p| (p.0 - x).abs() < 0.2 && (p.1 - y).abs() < 0.2)
                };
                for k in 1..=sys.len() as u8 {
                    let staff = staves[sys[usize::from(k) - 1]];
                    let clef = clefs[usize::from(k) - 1];
                    let mut evs: Vec<&Ev> = events.iter().filter(|e| e.staff_in_system == k).collect();
                    evs.sort_by(|a, b| a.x.total_cmp(&b.x));
                    let voices = split_voices(&evs, m.len_ticks, staff.mid(), sp);
                    for (v, evs) in voices.into_iter().enumerate() {
                    let voice = (u32::from(k) - 1) * 4 + v as u32 + 1;
                    let mut tick = 0u32;
                    for (e, start) in evs {
                        // A late-entering second voice starts where the
                        // split put it, not at the barline.
                        tick = tick.max(start);
                        // Every staff's onsets place the text: when the top
                        // staff rests, the bass carries the rhythm. A
                        // whole-bar rest is centred, not on a beat.
                        if !matches!(e.kind, EvKind::Rest { measure: true, .. }) {
                            onsets.push((e.x, tick));
                        }
                        match &e.kind {
                            EvKind::Rest { written, measure } => {
                                let dur = if *measure { m.len_ticks } else { ticks_of(written) };
                                m.events.push(Event::Rest(Rest {
                                    tick,
                                    duration: dur,
                                    voice,
                                    staff: k,
                                    written: *written,
                                    measure_rest: *measure,
                                }));
                                report.rests += 1;
                                tick += dur;
                            }
                            EvKind::Chord { heads, written, accidentals, staccato, slash } => {
                                let dur = ticks_of(written);
                                let notes = heads
                                    .iter()
                                    .zip(accidentals)
                                    .map(|((x, y), acc)| {
                                        let d = bottom_line(clef) + staff.position(*y);
                                        let tie_start = at(&tie_starts, *x, *y);
                                        let tie_stop = at(&tie_stops, *x, *y);
                                        if let Some(a) = acc {
                                            alters.insert((k, d), *a);
                                        } else if tie_stop
                                            && !alters.contains_key(&(k, d))
                                            && let Some(a) = prev_alters.get(&(k, d))
                                        {
                                            // Tied over the barline: the alteration
                                            // carries, and holds for the bar.
                                            alters.insert((k, d), *a);
                                        }
                                        let step = step_of(d);
                                        let alter = alters
                                            .get(&(k, d))
                                            .copied()
                                            .unwrap_or_else(|| key_alter(key.unwrap_or(0), step));
                                        Note {
                                            pitch: WrittenPitch { step, alter, octave: d.div_euclid(7) as i8 },
                                            tie_start,
                                            tie_stop,
                                            accidental: acc.map(|a| {
                                                match a {
                                                    2 => "double-sharp",
                                                    1 => "sharp",
                                                    0 => "natural",
                                                    -1 => "flat",
                                                    _ => "flat-flat",
                                                }
                                                .to_string()
                                            }),
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                report.notes += notes.len();
                                let mut articulations = std::collections::BTreeSet::new();
                                if *staccato {
                                    articulations.insert("staccato".to_string());
                                }
                                // Read by the chart writer: an attack, not a pitch.
                                if *slash {
                                    articulations.insert("rhythm-slash".to_string());
                                }
                                m.events.push(Event::Chord(Chord {
                                    tick,
                                    duration: dur,
                                    voice,
                                    staff: k,
                                    written: *written,
                                    grace: false,
                                    notes,
                                    slur_start: false,
                                    slur_stop: false,
                                    articulations,
                                    articulation_above: BTreeMap::new(),
                                }));
                                tick += dur;
                            }
                        }
                    }
                    // A second voice may stop short (Sibelius hides its
                    // rests); any voice running long was misread.
                    // A voice of nothing but rests says "silent" however it
                    // is spelled (a lone half rest for the bar).
                    let only_rests = m.events.iter().filter(|e| e.voice() == voice).all(|e| matches!(e, Event::Rest(_)));
                    let wrong = !only_rests && if v == 0 { tick != m.len_ticks } else { tick > m.len_ticks };
                    if wrong && tick != 0 {
                        report.warnings.push(format!(
                            "measure {} staff {k} voice {}: content {} quarters, meter {}",
                            index + 1,
                            v + 1,
                            f64::from(tick) / f64::from(DIVISIONS),
                            f64::from(m.len_ticks) / f64::from(DIVISIONS)
                        ));
                    }
                    }
                }

                // The nearest onset; with none near (a bar of rests under
                // a chord change), the x position through the bar, to the
                // eighth.
                let bar_len = m.len_ticks;
                let tick_at = |x: f64| -> u32 {
                    let near = onsets
                        .iter()
                        .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
                        .filter(|o| (o.0 - x).abs() < 2.5 * sp);
                    match near {
                        Some(o) => o.1,
                        None => {
                            let frac = ((x - x0) / (x1 - x0).max(1.0)).clamp(0.0, 0.999);
                            let eighth = DIVISIONS / 2;
                            ((frac * f64::from(bar_len) / f64::from(eighth)).round() as u32 * eighth)
                                .min(bar_len.saturating_sub(eighth))
                        }
                    }
                };
                // Text belongs to the measure whose span holds its left
                // edge (the first measure also takes text over its
                // prelude), between this system and the one above.
                let above_limit = first.top - 14.0 * sp;
                let in_span = |x: f64| x >= span_start && x < x1;
                // By the text's centre: a cue set just left of a barline
                // still belongs to the bar it is centred over.
                let centre = |w: &Word| (w.x0 + w.x1) / 2.0;
                // A chord symbol is left-aligned with its note (a cue is
                // centred over its spot, hence `centre` for the bar above).
                for c in chords.iter().filter(|c| c.y < first.top && c.y > above_limit && in_span(c.x0 + 1.0 * sp)) {
                    m.directions.push(Direction {
                        tick: tick_at(c.x0 + 0.3 * sp),
                        kind: DirectionKind::Harmony(c.text.clone()),
                    });
                }
                // Ending labels hang inside their bracket, a little below
                // the top line; other text stays above the staff.
                let above = |l: &Word| {
                    l.y < first.top || (is_ending_label(&l.text) && l.y < first.top + 1.5 * sp)
                };
                for l in lines.iter().filter(|l| above(l) && l.y > above_limit && in_span(centre(l))) {
                    if title.as_deref() == Some(l.text.as_str()) {
                        continue;
                    }
                    // A credit beside the title, not a cue for the band.
                    let lower = l.text.to_ascii_lowercase();
                    if ["arranged by", "arr.", "arr ", "transcribed by", "composed by", "words and music", "copyright"]
                        .iter()
                        .any(|p| lower.starts_with(p))
                    {
                        continue;
                    }
                    // A measure number over the system start.
                    if l.text.chars().all(|c| c.is_ascii_digit()) && (l.x0 - first.x0).abs() < 2.0 * sp {
                        continue;
                    }
                    if let Some(t) = tempo_of(l) {
                        m.directions.push(Direction { tick: 0, kind: DirectionKind::Tempo(t) });
                        continue;
                    }
                    // "1." / "2." over a bracket: an ending, not a cue.
                    if is_ending_label(&l.text) {
                        m.ending = Some(l.text.replace(' ', ""));
                        continue;
                    }
                    m.directions.push(Direction { tick: tick_at(l.x0), kind: DirectionKind::Words(l.text.clone()) });
                }
                prev_alters = alters;
                measures.push(m);
            }
        }
        for it in items.iter().filter(|i| !i.used && !matches!(i.sym, Sym::Brace | Sym::Clef(_))) {
            report.warnings.push(format!(
                "unclaimed {:?} '{}' at ({:.1}, {:.1}) page {}",
                it.sym, it.g.ch, it.g.origin.x, it.g.origin.y, page.number
            ));
        }
    }
    pair_ties(&mut measures);
    report.ties = measures
        .iter()
        .flat_map(|m| &m.events)
        .filter_map(|e| match e {
            Event::Chord(c) => Some(c.notes.iter().filter(|n| n.tie_start).count()),
            Event::Rest(_) => None,
        })
        .sum();
    report.measures = measures.len();
    let score = Score {
        work_title: title,
        parts: vec![Part {
            id: "P1".into(),
            name: if part_staves > 1 { "Keys".into() } else { "Part".into() },
            abbreviation: None,
            staves: part_staves,
            transpose: None,
            measures,
        }],
        ..Score::default()
    };
    Recognized { score, report }
}

fn ev_ticks(e: &Ev, len: u32) -> u32 {
    match &e.kind {
        EvKind::Rest { measure: true, .. } => len,
        EvKind::Rest { written, .. } | EvKind::Chord { written, .. } => ticks_of(written),
    }
}

/// One voice when the staff's events fill the bar; otherwise the best
/// split into two. Engraved time runs left to right, so a split must keep
/// it: events stacked at one x start together and so sit in different
/// voices, and an event further right never starts earlier. Among splits
/// where the first voice fills the bar (the second may stop short — its
/// rests are often hidden), prefer one stem direction per voice, the
/// second voice filling the bar too, and the upper voice on top.
fn split_voices<'e>(evs: &[&'e Ev], len: u32, mid: f64, sp: f64) -> Vec<Vec<(&'e Ev, u32)>> {
    // One voice, each event starting where the last ended.
    let single = || {
        let mut t = 0u32;
        vec![
            evs.iter()
                .map(|e| {
                    let at = t;
                    t += ev_ticks(e, len);
                    (*e, at)
                })
                .collect::<Vec<_>>(),
        ]
    };
    let total: u32 = evs.iter().map(|e| ev_ticks(e, len)).sum();
    if total <= len || evs.len() > 22 {
        return single();
    }
    struct Search<'a, 'e> {
        evs: &'a [&'e Ev],
        len: u32,
        sp: f64,
        assign: Vec<u8>,
        onset: Vec<u32>,
        best: Option<(i64, Vec<u8>, Vec<u32>)>,
        mid: f64,
    }
    impl Search<'_, '_> {
        fn score(&self, sums: [u32; 2]) -> i64 {
            let mut score = 0i64;
            for v in 0..2u8 {
                let ups: Vec<bool> = (0..self.evs.len())
                    .filter(|&i| self.assign[i] == v)
                    .filter_map(|i| self.evs[i].up)
                    .collect();
                if ups.iter().any(|u| *u) && ups.iter().any(|u| !*u) {
                    score -= 10;
                }
            }
            if sums[1] == self.len {
                score += 20;
            }
            // The first voice above the second.
            let avg = |v: u8| {
                let ys: Vec<f64> = (0..self.evs.len())
                    .filter(|&i| self.assign[i] == v)
                    .map(|i| self.evs[i].y)
                    .collect();
                if ys.is_empty() { self.mid } else { ys.iter().sum::<f64>() / ys.len() as f64 }
            };
            if avg(0) <= avg(1) {
                score += 5;
            }
            score
        }
        fn go(&mut self, i: usize, sums: [u32; 2]) {
            if i == self.evs.len() {
                if sums[0] == self.len && sums[1] > 0 {
                    let sc = self.score(sums);
                    if self.best.as_ref().is_none_or(|(b, _, _)| sc > *b) {
                        self.best = Some((sc, self.assign.clone(), self.onset.clone()));
                    }
                }
                return;
            }
            let e = self.evs[i];
            let d = ev_ticks(e, self.len);
            for v in 0..2u8 {
                let mut start = sums[usize::from(v)];
                // The second voice may come in late (its rests hidden): it
                // starts with whatever it stands over in the first voice.
                if v == 1
                    && let Some(j) = (0..i).find(|&j| {
                        self.assign[j] == 0 && (e.x - self.evs[j].x).abs() < 0.4 * self.sp
                    })
                {
                    start = start.max(self.onset[j]);
                }
                if start + d > self.len {
                    continue;
                }
                // Consistent with every event already placed in the other
                // voice: same x, same start; further left, no later start.
                let ok = (0..i).all(|j| {
                    if self.assign[j] == v {
                        return true;
                    }
                    let dx = e.x - self.evs[j].x;
                    if dx.abs() < 0.4 * self.sp {
                        self.onset[j] == start
                    } else if dx > 0.0 {
                        self.onset[j] <= start
                    } else {
                        self.onset[j] >= start
                    }
                });
                // Two events at one x cannot share a voice.
                let clash = (0..i).any(|j| self.assign[j] == v && (e.x - self.evs[j].x).abs() < 0.4 * self.sp);
                if !ok || clash {
                    continue;
                }
                self.assign[i] = v;
                self.onset[i] = start;
                let mut next = sums;
                next[usize::from(v)] = start + d;
                self.go(i + 1, next);
            }
        }
    }
    let mut search = Search {
        evs,
        len,
        sp,
        assign: vec![0; evs.len()],
        onset: vec![0; evs.len()],
        best: None,
        mid,
    };
    search.go(0, [0, 0]);
    match search.best {
        Some((_, assign, onset)) => {
            let pick = |v: u8| -> Vec<(&'e Ev, u32)> {
                (0..evs.len()).filter(|&i| assign[i] == v).map(|i| (evs[i], onset[i])).collect()
            };
            vec![pick(0), pick(1)]
        }
        None => single(),
    }
}

/// Give every tie both ends: a note tied into (`tie_stop`) makes the same
/// pitch in the voice's previous chord start the tie, and a tie that starts
/// makes the same pitch in the next chord end it — within the bar or over
/// the barline. A tie read in pieces, or over a system break, is often seen
/// at one end only.
fn pair_ties(measures: &mut [Measure]) {
    // (measure, event) of every chord, per voice, in time order.
    let mut by_voice: BTreeMap<u32, Vec<(usize, usize)>> = BTreeMap::new();
    for (mi, m) in measures.iter().enumerate() {
        let mut idx: Vec<usize> = (0..m.events.len())
            .filter(|&e| matches!(m.events[e], Event::Chord(_)))
            .collect();
        idx.sort_by_key(|&e| m.events[e].tick());
        for e in idx {
            by_voice.entry(m.events[e].voice()).or_default().push((mi, e));
        }
    }
    for chords in by_voice.values() {
        for w in chords.windows(2) {
            let ((ma, ea), (mb, eb)) = (w[0], w[1]);
            let pitches = |mi: usize, ei: usize| -> Vec<(WrittenPitch, bool, bool)> {
                match &measures[mi].events[ei] {
                    Event::Chord(c) => c.notes.iter().map(|n| (n.pitch, n.tie_start, n.tie_stop)).collect(),
                    Event::Rest(_) => Vec::new(),
                }
            };
            let (a, b) = (pitches(ma, ea), pitches(mb, eb));
            let same = |p: &WrittenPitch, q: &WrittenPitch| p.step == q.step && p.octave == q.octave;
            let starts: Vec<WrittenPitch> = a
                .iter()
                .filter(|(p, start, _)| *start || b.iter().any(|(q, _, stop)| *stop && same(p, q)))
                .map(|(p, _, _)| *p)
                .filter(|p| b.iter().any(|(q, _, _)| same(p, q)))
                .collect();
            for p in &starts {
                if let Event::Chord(c) = &mut measures[ma].events[ea] {
                    for n in c.notes.iter_mut().filter(|n| same(&n.pitch, p)) {
                        n.tie_start = true;
                    }
                }
                if let Event::Chord(c) = &mut measures[mb].events[eb] {
                    for n in c.notes.iter_mut().filter(|n| same(&n.pitch, p)) {
                        n.tie_stop = true;
                    }
                }
            }
        }
    }
}

/// An ending's label: numbers each followed by a dot ("1.", "2.", "1., 2.").
fn is_ending_label(text: &str) -> bool {
    let t = text.replace(' ', "");
    !t.is_empty()
        && t.split(',').all(|p| {
            p.strip_suffix('.').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
}

/// `q = 166` in a music text font: the note glyph, then the number.
fn tempo_of(l: &Word) -> Option<TempoMark> {
    let f = l.font.to_ascii_lowercase();
    if !(f.contains("text") && (f.contains("helsinki") || f.contains("opus"))) {
        return None;
    }
    let t = l.text.replace(' ', "");
    let (note, rest) = t.split_once('=')?;
    let (beat_unit, dots) = match note {
        "q" => ("quarter", 0),
        "q." => ("quarter", 1),
        "h" => ("half", 0),
        "h." => ("half", 1),
        "e" => ("eighth", 0),
        "e." => ("eighth", 1),
        _ => return None,
    };
    let per_minute: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    (!per_minute.is_empty()).then(|| TempoMark {
        text: None,
        beat_unit: beat_unit.into(),
        dots,
        per_minute,
        suffix: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(x: f64, y: f64, up: Option<bool>, t: NoteTypeValue) -> Ev {
        Ev {
            x,
            staff_in_system: 1,
            up,
            y,
            kind: EvKind::Chord {
                heads: vec![(x, y)],
                written: WrittenDuration { note_type: Some(t), dots: 0, tuplet: None },
                accidentals: vec![None],
                staccato: false,
                slash: false,
            },
        }
    }

    fn rest(x: f64, y: f64, t: NoteTypeValue) -> Ev {
        Ev {
            x,
            staff_in_system: 1,
            up: None,
            y,
            kind: EvKind::Rest {
                written: WrittenDuration { note_type: Some(t), dots: 0, tuplet: None },
                measure: false,
            },
        }
    }

    #[test]
    fn a_held_chord_under_a_line_is_two_voices() {
        // Can't Stop the Feeling, bar 1: a quarter rest over a whole-note
        // chord at one x, then six eighths — every stem down.
        let mut evs = vec![rest(90.0, 158.0, NoteTypeValue::Quarter), note(90.0, 180.0, None, NoteTypeValue::Whole)];
        for k in 0..6 {
            evs.push(note(126.0 + 18.0 * f64::from(k), 156.0, Some(false), NoteTypeValue::Eighth));
        }
        let refs: Vec<&Ev> = evs.iter().collect();
        let voices = split_voices(&refs, 4 * DIVISIONS, 168.0, 5.0);
        assert_eq!(voices.len(), 2);
        let len = |v: &[(&Ev, u32)]| v.iter().map(|(e, _)| ev_ticks(e, 4 * DIVISIONS)).sum::<u32>();
        assert_eq!(len(&voices[0]), 4 * DIVISIONS);
        assert_eq!(len(&voices[1]), 4 * DIVISIONS);
    }

    #[test]
    fn a_second_voice_can_come_in_late() {
        // Bass bar: a full bar of eighths and quarters, and at the last beat
        // a second voice's quarter under it (its earlier rests hidden).
        let mut evs = Vec::new();
        for k in 0..6 {
            evs.push(note(100.0 + 20.0 * f64::from(k), 234.0, Some(true), NoteTypeValue::Eighth));
        }
        evs.push(note(230.0, 215.0, Some(false), NoteTypeValue::Quarter));
        evs.push(rest(230.0, 202.0, NoteTypeValue::Quarter));
        let refs: Vec<&Ev> = evs.iter().collect();
        let voices = split_voices(&refs, 4 * DIVISIONS, 230.0, 5.0);
        assert_eq!(voices.len(), 2);
        let late = voices.iter().find(|v| v.len() == 1).expect("a one-event voice");
        assert_eq!(late[0].1, 3 * DIVISIONS, "it starts on beat 4");
    }
}
