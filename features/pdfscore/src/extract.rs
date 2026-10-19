//! Interpret a PDF page's content stream into positioned marks.
//!
//! Only what engraved music needs is modelled: the transformation stack
//! (`q`/`Q`/`cm`), paths (`m l c v y h re`) and how they are painted
//! (stroked, filled, both), line width and dashing, text (`BT … ET` with
//! `Tf Tm Td TD T* Tc Tw TL Tz Ts Tj TJ ' "`), images and form XObjects
//! (`Do`). Colours, clipping and shading are ignored: a printed part is ink
//! on paper.
//!
//! Coordinates are page points with the origin at the top left and y down,
//! the way the rest of the engraver works.

use std::collections::HashMap;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape as _};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Encoding, Object, ObjectId};

use crate::Error;

/// One character drawn by a font.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// The font's base name without a subset prefix (`HelsinkiStd`).
    pub font: String,
    /// The character, through the font's encoding (`œ` for a black notehead
    /// in a legacy music font; a SMuFL codepoint in a SMuFL font).
    pub ch: char,
    /// The raw character code in the string (a glyph id for Identity-H).
    pub code: u32,
    /// Origin of the glyph (its baseline start), page points.
    pub origin: Point,
    /// Font size in page points (the text space's vertical scale).
    pub size: f64,
    /// The advance the glyph moved the pen, page points.
    pub advance: f64,
}

/// A stroked path.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub path: BezPath,
    /// Line width in page points.
    pub width: f64,
    /// Drawn with a dash pattern (a dashed barline).
    pub dashed: bool,
}

/// A filled region (a beam, a slur, a tie, a filled box).
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub path: BezPath,
    /// Also stroked (`B`), with this width.
    pub stroke_width: Option<f64>,
}

/// An embedded raster image (a pasted screenshot).
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// Where it is drawn, page points.
    pub bounds: Rect,
    /// Its XObject name.
    pub name: String,
}

/// Everything drawn on one page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    /// 1-based.
    pub number: u32,
    pub width: f64,
    pub height: f64,
    pub glyphs: Vec<Glyph>,
    pub strokes: Vec<Stroke>,
    pub fills: Vec<Fill>,
    pub images: Vec<Image>,
}

/// Read every page of a PDF.
///
/// # Errors
/// The bytes are not a PDF lopdf can parse.
pub fn read_pdf(bytes: &[u8]) -> Result<Vec<Page>, Error> {
    let doc = Document::load_mem(bytes)?;
    let mut pages = Vec::new();
    for (number, page_id) in doc.get_pages() {
        pages.push(read_page(&doc, number, page_id)?);
    }
    Ok(pages)
}

/// What a font needs for decoding and advancing.
struct FontInfo<'d> {
    name: String,
    encoding: Option<Encoding<'d>>,
    /// Bytes per character code (2 for Identity-H composite fonts).
    code_len: usize,
    /// Advance widths in 1/1000 em by code; `default` for the rest.
    widths: HashMap<u32, f64>,
    default_width: f64,
}

impl FontInfo<'_> {
    fn width(&self, code: u32) -> f64 {
        self.widths
            .get(&code)
            .copied()
            .unwrap_or(self.default_width)
    }

    fn decode(&self, code: u32) -> char {
        let bytes: Vec<u8> = if self.code_len == 2 {
            vec![(code >> 8) as u8, code as u8]
        } else {
            vec![code as u8]
        };
        self.encoding
            .as_ref()
            .and_then(|e| Document::decode_text(e, &bytes).ok())
            .and_then(|s| s.chars().next())
            .unwrap_or(char::from_u32(code).unwrap_or('\u{FFFD}'))
    }
}

fn font_info<'d>(doc: &'d Document, dict: &'d Dictionary) -> FontInfo<'d> {
    let name = dict
        .get(b"BaseFont")
        .and_then(Object::as_name)
        .map(|n| {
            let n = String::from_utf8_lossy(n).to_string();
            // Subset fonts carry a six-letter tag: `ABCDEF+HelsinkiStd`.
            match n.split_once('+') {
                Some((tag, rest)) if tag.len() == 6 => rest.to_string(),
                _ => n,
            }
        })
        .unwrap_or_default();
    let subtype = dict
        .get(b"Subtype")
        .and_then(Object::as_name)
        .unwrap_or(b"");
    let composite = subtype == b"Type0";
    let mut widths = HashMap::new();
    let mut default_width = if composite { 1000.0 } else { 0.0 };
    if composite {
        let desc = dict
            .get(b"DescendantFonts")
            .and_then(|o| deref(doc, o).as_array())
            .ok()
            .and_then(|a| a.first())
            .and_then(|o| deref(doc, o).as_dict().ok());
        if let Some(desc) = desc {
            if let Ok(dw) = desc.get(b"DW").and_then(number) {
                default_width = dw;
            }
            if let Ok(w) = desc.get(b"W").and_then(|o| deref(doc, o).as_array()) {
                read_cid_widths(doc, w, &mut widths);
            }
        }
    } else if let (Ok(first), Ok(w)) = (
        dict.get(b"FirstChar").and_then(Object::as_i64),
        dict.get(b"Widths").and_then(|o| deref(doc, o).as_array()),
    ) {
        for (k, o) in w.iter().enumerate() {
            if let Ok(v) = number(deref(doc, o)) {
                widths.insert((first as u32) + k as u32, v);
            }
        }
    }
    FontInfo {
        name,
        encoding: dict.get_font_encoding(doc).ok(),
        code_len: if composite { 2 } else { 1 },
        widths,
        default_width,
    }
}

/// A CIDFont `W` array: `c [w1 w2 …]` or `c_first c_last w`.
fn read_cid_widths(doc: &Document, w: &[Object], out: &mut HashMap<u32, f64>) {
    let mut i = 0;
    while i < w.len() {
        let Ok(first) = number(deref(doc, &w[i])) else {
            break;
        };
        match w.get(i + 1).map(|o| deref(doc, o)) {
            Some(Object::Array(list)) => {
                for (k, o) in list.iter().enumerate() {
                    if let Ok(v) = number(deref(doc, o)) {
                        out.insert(first as u32 + k as u32, v);
                    }
                }
                i += 2;
            }
            Some(last) => {
                let (Ok(last), Some(Ok(v))) =
                    (number(last), w.get(i + 2).map(|o| number(deref(doc, o))))
                else {
                    break;
                };
                for c in first as u32..=last as u32 {
                    out.insert(c, v);
                }
                i += 3;
            }
            None => break,
        }
    }
}

fn deref<'d>(doc: &'d Document, o: &'d Object) -> &'d Object {
    match o {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(o),
        other => other,
    }
}

fn number(o: &Object) -> lopdf::Result<f64> {
    match o {
        Object::Integer(i) => Ok(*i as f64),
        Object::Real(r) => Ok(f64::from(*r)),
        _ => Err(lopdf::Error::ObjectType {
            expected: "number",
            found: o.enum_variant(),
        }),
    }
}

fn operand(ops: &[Object], i: usize) -> f64 {
    ops.get(i).and_then(|o| number(o).ok()).unwrap_or(0.0)
}

fn read_page(doc: &Document, page_number: u32, page_id: ObjectId) -> Result<Page, Error> {
    let page_dict = doc.get_dictionary(page_id)?;
    let media = page_dict
        .get(b"MediaBox")
        .and_then(|o| deref(doc, o).as_array())
        .map(|a| {
            a.iter()
                .map(|o| number(deref(doc, o)).unwrap_or(0.0))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|_| vec![0.0, 0.0, 612.0, 792.0]);
    let (x0, y0, x1, y1) = (media[0], media[1], media[2], media[3]);
    let mut page = Page {
        number: page_number,
        width: x1 - x0,
        height: y1 - y0,
        ..Page::default()
    };
    // PDF user space is y up from the bottom; the engraver's is y down.
    let to_page = Affine::new([1.0, 0.0, 0.0, -1.0, -x0, y1]);
    let content = doc.get_page_content(page_id)?;
    let resources = page_resources(doc, page_id);
    let mut interp = Interpreter::new(doc);
    interp.run(&content, resources, to_page, &mut page, 0);
    Ok(page)
}

/// The page's resource dictionary (its own, or inherited).
fn page_resources(doc: &Document, page_id: ObjectId) -> Option<&Dictionary> {
    let (own, inherited) = doc.get_page_resources(page_id).ok()?;
    own.or_else(|| {
        inherited
            .first()
            .and_then(|id| doc.get_dictionary(*id).ok())
    })
}

#[derive(Clone)]
struct GState {
    ctm: Affine,
    line_width: f64,
    dashed: bool,
    // Text state that survives BT/ET.
    font: Option<Vec<u8>>,
    font_size: f64,
    char_spacing: f64,
    word_spacing: f64,
    h_scale: f64,
    leading: f64,
    rise: f64,
}

struct Interpreter<'d> {
    doc: &'d Document,
    fonts: HashMap<(usize, Vec<u8>), FontInfo<'d>>,
}

impl<'d> Interpreter<'d> {
    fn new(doc: &'d Document) -> Self {
        Self {
            doc,
            fonts: HashMap::new(),
        }
    }

    fn font(&mut self, resources: Option<&'d Dictionary>, name: &[u8]) -> Option<&FontInfo<'d>> {
        let key = (
            resources.map_or(0, |r| r as *const Dictionary as usize),
            name.to_vec(),
        );
        if !self.fonts.contains_key(&key) {
            let doc = self.doc;
            let dict = resources?
                .get(b"Font")
                .ok()
                .map(|o| deref(doc, o))
                .and_then(|o| o.as_dict().ok())?
                .get(name)
                .ok()
                .map(|o| deref(doc, o))
                .and_then(|o| o.as_dict().ok())?;
            self.fonts.insert(key.clone(), font_info(doc, dict));
        }
        self.fonts.get(&key)
    }

    #[allow(clippy::too_many_lines)]
    fn run(
        &mut self,
        content: &[u8],
        resources: Option<&'d Dictionary>,
        base: Affine,
        page: &mut Page,
        depth: usize,
    ) {
        let Ok(content) = Content::decode(content) else {
            tracing::warn!(page = page.number, "content stream did not decode");
            return;
        };
        let mut gs = GState {
            ctm: base,
            line_width: 1.0,
            dashed: false,
            font: None,
            font_size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scale: 1.0,
            leading: 0.0,
            rise: 0.0,
        };
        let mut stack: Vec<GState> = Vec::new();
        let mut path = BezPath::new();
        let mut current = Point::ZERO;
        // Text matrix and line matrix (text space → user space).
        let mut tm = Affine::IDENTITY;
        let mut tlm = Affine::IDENTITY;

        for op in &content.operations {
            let o = &op.operands;
            match op.operator.as_str() {
                "q" => stack.push(gs.clone()),
                "Q" => {
                    if let Some(prev) = stack.pop() {
                        gs = prev;
                    }
                }
                "cm" => {
                    let m = Affine::new([
                        operand(o, 0),
                        operand(o, 1),
                        operand(o, 2),
                        operand(o, 3),
                        operand(o, 4),
                        operand(o, 5),
                    ]);
                    gs.ctm *= m;
                }
                "w" => gs.line_width = operand(o, 0),
                "d" => {
                    gs.dashed = o
                        .first()
                        .and_then(|a| a.as_array().ok())
                        .is_some_and(|a| !a.is_empty());
                }
                // ── paths ──
                "m" => {
                    current = Point::new(operand(o, 0), operand(o, 1));
                    path.move_to(current);
                }
                "l" => {
                    current = Point::new(operand(o, 0), operand(o, 1));
                    path.line_to(current);
                }
                "c" => {
                    current = Point::new(operand(o, 4), operand(o, 5));
                    path.curve_to(
                        Point::new(operand(o, 0), operand(o, 1)),
                        Point::new(operand(o, 2), operand(o, 3)),
                        current,
                    );
                }
                "v" => {
                    let start = current;
                    current = Point::new(operand(o, 2), operand(o, 3));
                    path.curve_to(start, Point::new(operand(o, 0), operand(o, 1)), current);
                }
                "y" => {
                    current = Point::new(operand(o, 2), operand(o, 3));
                    path.curve_to(Point::new(operand(o, 0), operand(o, 1)), current, current);
                }
                "h" => path.close_path(),
                "re" => {
                    let r = Rect::new(
                        operand(o, 0),
                        operand(o, 1),
                        operand(o, 0) + operand(o, 2),
                        operand(o, 1) + operand(o, 3),
                    );
                    path.extend(r.path_elements(0.1));
                }
                "S" | "s" => {
                    if op.operator == "s" {
                        path.close_path();
                    }
                    let width = gs.line_width * scale_of(gs.ctm);
                    page.strokes.push(Stroke {
                        path: gs.ctm * std::mem::take(&mut path),
                        width,
                        dashed: gs.dashed,
                    });
                }
                "f" | "F" | "f*" => page.fills.push(Fill {
                    path: gs.ctm * std::mem::take(&mut path),
                    stroke_width: None,
                }),
                "B" | "B*" | "b" | "b*" => {
                    if op.operator.starts_with('b') {
                        path.close_path();
                    }
                    page.fills.push(Fill {
                        path: gs.ctm * std::mem::take(&mut path),
                        stroke_width: Some(gs.line_width * scale_of(gs.ctm)),
                    });
                }
                "n" => path = BezPath::new(),
                // ── text ──
                "BT" => {
                    tm = Affine::IDENTITY;
                    tlm = Affine::IDENTITY;
                }
                "Tf" => {
                    gs.font = o.first().and_then(|n| n.as_name().ok()).map(<[u8]>::to_vec);
                    gs.font_size = operand(o, 1);
                }
                "Tc" => gs.char_spacing = operand(o, 0),
                "Tw" => gs.word_spacing = operand(o, 0),
                "Tz" => gs.h_scale = operand(o, 0) / 100.0,
                "TL" => gs.leading = operand(o, 0),
                "Ts" => gs.rise = operand(o, 0),
                "Tm" => {
                    tlm = Affine::new([
                        operand(o, 0),
                        operand(o, 1),
                        operand(o, 2),
                        operand(o, 3),
                        operand(o, 4),
                        operand(o, 5),
                    ]);
                    tm = tlm;
                }
                "Td" => {
                    tlm *= Affine::translate((operand(o, 0), operand(o, 1)));
                    tm = tlm;
                }
                "TD" => {
                    gs.leading = -operand(o, 1);
                    tlm *= Affine::translate((operand(o, 0), operand(o, 1)));
                    tm = tlm;
                }
                "T*" => {
                    tlm *= Affine::translate((0.0, -gs.leading));
                    tm = tlm;
                }
                "Tj" | "'" | "\"" | "TJ" => {
                    if op.operator == "'" || op.operator == "\"" {
                        if op.operator == "\"" {
                            gs.word_spacing = operand(o, 0);
                            gs.char_spacing = operand(o, 1);
                        }
                        tlm *= Affine::translate((0.0, -gs.leading));
                        tm = tlm;
                    }
                    let items: Vec<&Object> = if op.operator == "TJ" {
                        o.first()
                            .and_then(|a| a.as_array().ok())
                            .map(|a| a.iter().collect())
                            .unwrap_or_default()
                    } else {
                        o.last().into_iter().collect()
                    };
                    let Some(font_name) = gs.font.clone() else {
                        continue;
                    };
                    for item in items {
                        match item {
                            Object::String(bytes, _) => {
                                self.show_text(resources, &font_name, bytes, &gs, &mut tm, page);
                            }
                            other => {
                                // A TJ adjustment: thousandths of an em, back.
                                let adj = number(other).unwrap_or(0.0);
                                let tx = -adj / 1000.0 * gs.font_size * gs.h_scale;
                                tm *= Affine::translate((tx, 0.0));
                            }
                        }
                    }
                }
                // ── XObjects ──
                "Do" => {
                    if depth < 8
                        && let Some(name) = o.first().and_then(|n| n.as_name().ok())
                    {
                        self.do_xobject(resources, name, &gs, page, depth);
                    }
                }
                _ => {}
            }
        }
    }

    fn show_text(
        &mut self,
        resources: Option<&'d Dictionary>,
        font_name: &[u8],
        bytes: &[u8],
        gs: &GState,
        tm: &mut Affine,
        page: &mut Page,
    ) {
        let Some(font) = self.font(resources, font_name) else {
            return;
        };
        let codes: Vec<u32> = if font.code_len == 2 {
            bytes
                .chunks(2)
                .map(|c| u32::from(c[0]) << 8 | u32::from(*c.get(1).unwrap_or(&0)))
                .collect()
        } else {
            bytes.iter().map(|&b| u32::from(b)).collect()
        };
        for code in codes {
            let ch = font.decode(code);
            // Text rendering matrix: font size and rise in text space, then
            // the text matrix, then the CTM.
            let trm = gs.ctm
                * *tm
                * Affine::new([
                    gs.font_size * gs.h_scale,
                    0.0,
                    0.0,
                    gs.font_size,
                    0.0,
                    gs.rise,
                ]);
            let origin = trm * Point::ZERO;
            let size = (trm.as_coeffs()[2].powi(2) + trm.as_coeffs()[3].powi(2)).sqrt();
            let w = font.width(code) / 1000.0;
            let space = if code == 32 && font.code_len == 1 {
                gs.word_spacing
            } else {
                0.0
            };
            let tx = (w * gs.font_size + gs.char_spacing + space) * gs.h_scale;
            let advance_vec = (gs.ctm * *tm) * Point::new(tx, 0.0) - (gs.ctm * *tm) * Point::ZERO;
            page.glyphs.push(Glyph {
                font: font.name.clone(),
                ch,
                code,
                origin,
                size,
                advance: advance_vec.hypot(),
            });
            *tm *= Affine::translate((tx, 0.0));
        }
    }

    fn do_xobject(
        &mut self,
        resources: Option<&'d Dictionary>,
        name: &[u8],
        gs: &GState,
        page: &mut Page,
        depth: usize,
    ) {
        let doc = self.doc;
        let Some(stream) = resources
            .and_then(|r| r.get(b"XObject").ok())
            .map(|o| deref(doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(name).ok())
            .map(|o| deref(doc, o))
            .and_then(|o| o.as_stream().ok())
        else {
            return;
        };
        match stream.dict.get(b"Subtype").and_then(Object::as_name) {
            Ok(b"Image") => {
                let bounds = (gs.ctm * Rect::new(0.0, 0.0, 1.0, 1.0).to_path(0.1)).bounding_box();
                page.images.push(Image {
                    bounds,
                    name: String::from_utf8_lossy(name).to_string(),
                });
            }
            Ok(b"Form") => {
                let matrix = stream
                    .dict
                    .get(b"Matrix")
                    .and_then(|o| deref(doc, o).as_array())
                    .ok()
                    .map(|a| {
                        let v: Vec<f64> = a
                            .iter()
                            .map(|o| number(deref(doc, o)).unwrap_or(0.0))
                            .collect();
                        Affine::new([v[0], v[1], v[2], v[3], v[4], v[5]])
                    })
                    .unwrap_or(Affine::IDENTITY);
                let form_resources = stream
                    .dict
                    .get(b"Resources")
                    .ok()
                    .map(|o| deref(doc, o))
                    .and_then(|o| o.as_dict().ok())
                    .or(resources);
                let Ok(content) = stream
                    .decompressed_content()
                    .or_else(|_| Ok::<_, lopdf::Error>(stream.content.clone()))
                else {
                    return;
                };
                self.run(&content, form_resources, gs.ctm * matrix, page, depth + 1);
            }
            _ => {}
        }
    }
}

/// The CTM's average scale (for line widths).
fn scale_of(m: Affine) -> f64 {
    let [a, b, c, d, _, _] = m.as_coeffs();
    ((a * d - b * c).abs()).sqrt()
}

/// The path elements of a stroke as straight segments, when it is one line.
#[must_use]
pub fn as_segment(path: &BezPath) -> Option<(Point, Point)> {
    let els = path.elements();
    match els {
        [PathEl::MoveTo(a), PathEl::LineTo(b)] => Some((*a, *b)),
        _ => None,
    }
}
