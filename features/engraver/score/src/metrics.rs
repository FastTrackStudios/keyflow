//! Ink metrics: the real extent of glyphs and text runs, measured from the
//! font outlines (y down, points). Shared by vertical placement (which
//! needs to know what a dynamic or a cue actually covers) and the collision
//! inspector (which must measure exactly what placement measured).

use std::collections::HashMap;

use engraver_proto::engraver::fonts::ChartFontBundle;
use engraver_proto::engraver::scene::node::SceneNode;
use engraver_proto::engraver::scene::paint::{FontStyle, FontWeight, PaintCommand, TextAnchor};
use kurbo::{Affine, Point, Rect};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider};

struct BoundsPen {
    min: Point,
    max: Point,
    any: bool,
}

impl BoundsPen {
    fn new() -> Self {
        Self {
            min: Point::new(f64::MAX, f64::MAX),
            max: Point::new(f64::MIN, f64::MIN),
            any: false,
        }
    }
    fn add(&mut self, x: f32, y: f32) {
        let (x, y) = (f64::from(x), f64::from(y));
        self.min.x = self.min.x.min(x);
        self.min.y = self.min.y.min(y);
        self.max.x = self.max.x.max(x);
        self.max.y = self.max.y.max(y);
        self.any = true;
    }
}

impl OutlinePen for BoundsPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.add(cx0, cy0);
        self.add(x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.add(cx0, cy0);
        self.add(cx1, cy1);
        self.add(x, y);
    }
    fn close(&mut self) {}
}

/// Per-character ink box and advance, in em units (y up), per font.
pub struct Faces {
    data: Vec<(&'static str, bool, bool, Vec<u8>)>,
    cache: HashMap<(usize, char), Option<(Rect, f64)>>,
}

impl Faces {
    #[must_use]
    pub fn new(bundle: &ChartFontBundle) -> Self {
        let mut data: Vec<(&'static str, bool, bool, Vec<u8>)> = vec![
            (
                "Leland",
                false,
                false,
                bundle.symbol_font_data().as_ref().clone(),
            ),
            (
                "Bravura",
                false,
                false,
                bundle.bravura_font_data().as_ref().clone(),
            ),
            (
                "Leland Text",
                false,
                false,
                bundle.leland_text_font_data().as_ref().clone(),
            ),
            (
                "FreeSans",
                false,
                false,
                bundle.freesans_font_data().as_ref().clone(),
            ),
            (
                "Chicago",
                false,
                false,
                bundle.chicago_font_data().as_ref().clone(),
            ),
        ];
        let faces = crate::fonts::serif_faces();
        for (i, (_, bytes)) in faces.iter().enumerate() {
            data.push((
                crate::fonts::SERIF_FAMILY,
                i == 1 || i == 3,
                i >= 2,
                bytes.to_vec(),
            ));
        }
        data.push(("FreeSans", true, false, crate::fonts::SANS_BOLD.to_vec()));
        Self {
            data,
            cache: HashMap::new(),
        }
    }

    fn face_index(&self, family: &str, bold: bool, italic: bool) -> usize {
        let family = match family {
            "LelandText" => "Leland Text",
            "sans-serif" => "FreeSans",
            "ChicagoFLF" => "Chicago",
            f => f,
        };
        self.data
            .iter()
            .position(|(f, b, i, _)| *f == family && *b == bold && *i == italic)
            .or_else(|| self.data.iter().position(|(f, _, _, _)| *f == family))
            .unwrap_or(3)
    }

    /// (ink box, advance) of one character, em units, y up.
    fn char_metrics(&mut self, face: usize, c: char) -> Option<(Rect, f64)> {
        if let Some(hit) = self.cache.get(&(face, c)) {
            return *hit;
        }
        let measured = (|| {
            let font = FontRef::new(&self.data[face].3).ok()?;
            let upem = f64::from(font.head().ok()?.units_per_em());
            let gid = font.charmap().map(c)?;
            let advance = font
                .glyph_metrics(Size::unscaled(), LocationRef::default())
                .advance_width(gid)
                .map_or(0.0, f64::from)
                / upem;
            let outlines = font.outline_glyphs();
            let mut pen = BoundsPen::new();
            if let Some(glyph) = outlines.get(gid) {
                let _ = glyph.draw(
                    DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                    &mut pen,
                );
            }
            let rect = if pen.any {
                Rect::new(
                    pen.min.x / upem,
                    pen.min.y / upem,
                    pen.max.x / upem,
                    pen.max.y / upem,
                )
            } else {
                Rect::ZERO
            };
            Some((rect, advance))
        })();
        self.cache.insert((face, c), measured);
        measured
    }

    /// Ink box of a text run drawn at `pos` (baseline) with `anchor`, in
    /// y-down scene units.
    #[allow(clippy::too_many_arguments)]
    pub fn text_ink(
        &mut self,
        family: &str,
        size: f64,
        bold: bool,
        italic: bool,
        text: &str,
        pos: Point,
        anchor: TextAnchor,
    ) -> Option<Rect> {
        let face = self.face_index(family, bold, italic);
        let mut x = 0.0;
        let mut ink: Option<Rect> = None;
        for c in text.chars() {
            let (r, adv) = self.char_metrics(face, c).unwrap_or((Rect::ZERO, 0.5));
            if r.area() > 0.0 {
                let glyph = Rect::new(x + r.x0, -r.y1, x + r.x1, -r.y0);
                ink = Some(ink.map_or(glyph, |i| i.union(glyph)));
            }
            x += adv;
        }
        let shift = match anchor {
            TextAnchor::Middle => -x / 2.0,
            TextAnchor::End => -x,
            _ => 0.0,
        };
        ink.map(|r| {
            Rect::new(
                pos.x + (r.x0 + shift) * size,
                pos.y + r.y0 * size,
                pos.x + (r.x1 + shift) * size,
                pos.y + r.y1 * size,
            )
        })
    }
}

thread_local! {
    static FACES: std::cell::RefCell<Option<Faces>> = const { std::cell::RefCell::new(None) };
}

/// Measure with the thread's shared [`Faces`] (fonts parsed once).
pub fn with_faces<R>(f: impl FnOnce(&mut Faces) -> R) -> R {
    FACES.with(|cell| {
        let mut slot = cell.borrow_mut();
        let faces = slot
            .get_or_insert_with(|| Faces::new(ChartFontBundle::shared().expect("embedded fonts")));
        f(faces)
    })
}

/// Every rectangle of ink a scene subtree draws, in the coordinates of
/// `transform` (glyphs and text by outline, lines and paths by their
/// stroked extent). Staff lines and barlines included — a skyline drops
/// whatever lies inside the staff.
#[must_use]
pub fn node_ink(node: &SceneNode, transform: Affine) -> Vec<Rect> {
    with_faces(|faces| {
        let mut out = Vec::new();
        collect(faces, node, transform, &mut out);
        out
    })
}

fn collect(faces: &mut Faces, node: &SceneNode, transform: Affine, out: &mut Vec<Rect>) {
    let t = transform * node.transform;
    for cmd in &node.commands {
        if let Some(r) = command_ink(faces, cmd, t) {
            out.push(r);
        }
    }
    for child in &node.children {
        collect(faces, child, t, out);
    }
}

/// The ink box of one paint command under `t`.
pub fn command_ink(faces: &mut Faces, cmd: &PaintCommand, t: Affine) -> Option<Rect> {
    let local = match cmd {
        PaintCommand::Text {
            text,
            font_family,
            font_size,
            position,
            anchor,
            weight,
            style,
            ..
        } => faces.text_ink(
            font_family,
            *font_size,
            matches!(weight, FontWeight::Bold),
            matches!(style, FontStyle::Italic),
            text,
            *position,
            *anchor,
        )?,
        PaintCommand::Glyph {
            codepoint,
            position,
            size,
            ..
        } => faces.text_ink(
            "Leland",
            size * 4.0,
            false,
            false,
            &codepoint.to_string(),
            *position,
            TextAnchor::Start,
        )?,
        other => other.bounding_box()?,
    };
    Some(t.transform_rect_bbox(local))
}

/// Ink box of a text run (see [`Faces::text_ink`]) with the shared faces.
#[must_use]
pub fn text_ink(
    family: &str,
    size: f64,
    bold: bool,
    italic: bool,
    text: &str,
    pos: Point,
    anchor: TextAnchor,
) -> Option<Rect> {
    with_faces(|f| f.text_ink(family, size, bold, italic, text, pos, anchor))
}
