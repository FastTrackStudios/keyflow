//! Finale's own page layout: linked parts, page formats, systems,
//! multimeasure rests, page text, and measure-number settings.
//!
//! MusicXML carries the notes; it has nowhere to put "the Viola part breaks
//! its third system at bar 15 and its second page at bar 63". Finale stores
//! every linked part's layout in the same document as the score — records
//! tagged `part="N"` — so a part can be re-engraved exactly as its arranger
//! formatted it. This module reads that layout; the engraver consumes it.
//!
//! Units are Finale's: EVPU (288 per inch, so 4 per point) unless a field
//! says otherwise. Vertical distances are given as positive magnitudes
//! downward (Finale stores most of them negated).

use std::collections::{BTreeMap, HashMap};

use roxmltree::Node;

use crate::xml_in::{attr, child, children, has_child, path_text, text};
use crate::{Error, decode_xml};

/// Finale's default staff height in EFIX (1/64 EVPU): 96 EVPU = 4 spaces of 24.
pub const DEFAULT_STAFF_HEIGHT_EFIX: i32 = 6144;

/// EVPU → PostScript points.
#[must_use]
pub fn evpu_to_pt(evpu: f64) -> f64 {
    evpu / 4.0
}

/// Score-wide file information (Finale's `texts/fileInfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInfo {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub lyricist: Option<String>,
    pub copyright: Option<String>,
}

/// The whole document's layout: the score view and every linked part.
#[derive(Debug, Clone)]
pub struct DocumentLayout {
    pub file_info: FileInfo,
    /// The conductor score (`partDef` 0).
    pub score: PartLayout,
    /// Linked parts in Finale's part order.
    pub parts: Vec<PartLayout>,
    /// The score's page text (each [`PartLayout`] has its own merged copy).
    pub page_texts: Vec<PageText>,
    pub measure_numbers: Vec<MeasureNumberRegion>,
}

/// One linked part (or the score) as Finale lays it out.
#[derive(Debug, Clone)]
pub struct PartLayout {
    /// `partDef` cmper (0 = score).
    pub number: u32,
    pub order: u32,
    /// Display name (`^partname()`), styling stripped.
    pub name: String,
    /// Finale staff cmpers in this part, top to bottom.
    pub staves: Vec<u32>,
    /// The MusicXML part ids (`P{n}`, as [`crate::musx_to_musicxml`] emits
    /// them) these staves convert to, deduplicated, in staff order.
    pub part_ids: Vec<String>,
    /// Which staves (1-based) of the first converted part this part shows.
    /// A braced group the converter merges into one part (a choir's S-A
    /// over T-B) is two Finale parts of one staff each: the T-B part is
    /// staff 2 of the merged part.
    pub score_staves: Vec<u8>,
    /// Whether Finale marks this part for extraction/printing.
    pub extract: bool,
    pub pages: Vec<PageSpec>,
    pub systems: Vec<SystemSpec>,
    pub multirests: Vec<MultiRestSpec>,
    /// Per-measure width overrides (measure number → EVPU), the springs
    /// Finale justifies each system with.
    pub measure_widths: BTreeMap<u32, i32>,
    /// Every staff in the part hides measure numbers (`staffSpec/hideMeasNums`).
    pub hide_measure_numbers: bool,
    /// Page text as this part prints it (score text plus part overrides).
    pub page_texts: Vec<PageText>,
}

/// One page's format and the first system it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSpec {
    pub width: i32,
    pub height: i32,
    pub margin_top: i32,
    pub margin_left: i32,
    pub margin_bottom: i32,
    pub margin_right: i32,
    /// 1-based index into [`PartLayout::systems`].
    pub first_system: u32,
}

/// One system: a measure range and its vertical metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemSpec {
    pub start_measure: u32,
    /// Exclusive.
    pub end_measure: u32,
    /// Staff height in EFIX (1/64 EVPU); 6144 = unscaled.
    pub staff_height: i32,
    /// Extra left indent past the page margin.
    pub left: i32,
    /// Extra right indent (positive = narrower system).
    pub right: i32,
    /// Space above the top staff line.
    pub top: i32,
    /// Distance from the top staff line to the system's bottom, in the
    /// system's scaled space.
    pub bottom: i32,
    /// Gap after the previous system.
    pub distance_to_prev: i32,
}

impl SystemSpec {
    /// Staff space in EVPU.
    #[must_use]
    pub fn spatium(&self) -> f64 {
        f64::from(self.staff_height) / 64.0 / 4.0
    }

    /// Scale of the staff against Finale's default (1.0 = 24 EVPU spaces).
    #[must_use]
    pub fn scale(&self) -> f64 {
        f64::from(self.staff_height) / f64::from(DEFAULT_STAFF_HEIGHT_EFIX)
    }
}

/// A run of rest measures drawn as one multimeasure rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiRestSpec {
    pub start_measure: u32,
    /// Exclusive.
    pub end_measure: u32,
    /// Width Finale gives the block (its spring).
    pub width: i32,
}

/// Horizontal anchor of a page text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical anchor of a page text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Font face and effects of a Finale text block.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub family: Option<String>,
    /// Point size.
    pub size: Option<f64>,
    pub bold: bool,
    pub italic: bool,
}

/// A run of Finale text: literals and the inserts that resolve per part/page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPiece {
    Literal(String),
    Title,
    Subtitle,
    Composer,
    Arranger,
    Lyricist,
    Copyright,
    PartName,
    /// `^page(n)` — the page number.
    Page,
    /// `^totpages()`.
    TotalPages,
    /// `^date(n)` — the print date.
    Date,
    FileName,
}

/// Parsed Finale enigma text (`^fontid(2)^size(12)^nfx(0)^title()`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnigmaText {
    /// The style in effect at the first visible character.
    pub style: TextStyle,
    /// Every line's pieces, run together.
    pub pieces: Vec<TextPiece>,
    /// The text broken at its newlines, each line with the style in effect
    /// where it starts (a title block is often a 24pt title over a 12pt
    /// credit line).
    pub lines: Vec<EnigmaLine>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnigmaLine {
    pub style: TextStyle,
    pub pieces: Vec<TextPiece>,
}

impl EnigmaText {
    /// Plain text with every insert dropped, lines joined by `\n`.
    #[must_use]
    pub fn literal_lines(&self) -> String {
        self.lines
            .iter()
            .map(|l| {
                l.pieces
                    .iter()
                    .filter_map(|p| match p {
                        TextPiece::Literal(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Plain text with every insert dropped.
    #[must_use]
    pub fn literal(&self) -> String {
        self.pieces
            .iter()
            .filter_map(|p| match p {
                TextPiece::Literal(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Page text placed on a range of pages.
#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
    pub text: EnigmaText,
    /// 1-based first page.
    pub start_page: u32,
    /// 1-based last page; `None` = to the end.
    pub end_page: Option<u32>,
    pub h_align: HAlign,
    pub v_align: VAlign,
    /// Offset right of the anchor.
    pub x: i32,
    /// Offset UP from the anchor (Finale's sign).
    pub y: i32,
}

/// Measure-number display for one region of measures, as parts show it.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureNumberRegion {
    pub start_measure: u32,
    /// Inclusive.
    pub end_measure: u32,
    pub parts: MeasureNumberStyle,
    pub score: MeasureNumberStyle,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeasureNumberStyle {
    /// Number every `incidence` measures (otherwise only system starts).
    pub multiple_of: bool,
    pub incidence: u32,
    /// Skip the first measure of the piece.
    pub except_first: bool,
    /// Under a multimeasure rest, print its range ("11-14").
    pub multirest_range: bool,
    pub start_font: TextStyle,
    pub multiple_font: TextStyle,
    pub multirest_font: TextStyle,
    /// Offsets from the measure's top-left, Finale sign (y up).
    pub start_x: i32,
    pub start_y: i32,
    pub multiple_x: i32,
    pub multiple_y: i32,
    pub multirest_x: i32,
    pub multirest_y: i32,
}

impl DocumentLayout {
    /// The title as page 1 prints it: the largest-type page-1 text block.
    /// Files started from an older template often keep that template's
    /// `fileInfo` title ("THE LITTLE DRUMMER CHILD") while the page shows
    /// a typed-in one, so the printed text wins; `fileInfo` is the fallback.
    #[must_use]
    pub fn printed_title(&self) -> Option<String> {
        let candidate = self
            .page_texts
            .iter()
            .filter(|t| t.start_page == 1)
            .filter(|t| {
                !t.text.pieces.iter().any(|p| {
                    matches!(
                        p,
                        TextPiece::PartName
                            | TextPiece::Page
                            | TextPiece::Composer
                            | TextPiece::Date
                    )
                })
            })
            .max_by(|a, b| {
                let size = |t: &PageText| t.text.style.size.unwrap_or(0.0);
                size(a).total_cmp(&size(b))
            })?;
        let first_line = candidate.text.lines.iter().find(|l| !l.pieces.is_empty())?;
        let text: String = first_line
            .pieces
            .iter()
            .map(|p| match p {
                TextPiece::Literal(s) => s.clone(),
                TextPiece::Title => self.file_info.title.clone().unwrap_or_default(),
                TextPiece::Subtitle => self.file_info.subtitle.clone().unwrap_or_default(),
                _ => String::new(),
            })
            .collect();
        let first_line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
        let title = first_line
            .trim_matches(|c| matches!(c, '"' | '“' | '”'))
            .trim();
        (!title.is_empty())
            .then(|| title.to_string())
            .or_else(|| self.file_info.title.clone())
    }
}

/// Read the layout of a decoded EnigmaXML document.
pub fn document_layout(enigmaxml: &[u8]) -> Result<DocumentLayout, Error> {
    crate::xml_in::reset_index();
    let xml = decode_xml(enigmaxml);
    let doc = roxmltree::Document::parse(&xml).map_err(|e| Error::Xml(e.to_string()))?;
    Ok(read_layout(doc.root_element()))
}

/// Read the layout straight from a `.musx` archive.
pub fn musx_layout(musx_bytes: &[u8]) -> Result<DocumentLayout, Error> {
    document_layout(&crate::musx_to_enigmaxml(musx_bytes)?)
}

/// Elements under `/finale/<container>` grouped by local name.
struct Index<'a, 'i> {
    by_name: HashMap<&'a str, Vec<Node<'a, 'i>>>,
}

impl<'a, 'i> Index<'a, 'i> {
    fn new(root: Node<'a, 'i>, containers: &[&str]) -> Self {
        let mut by_name: HashMap<&'a str, Vec<Node<'a, 'i>>> = HashMap::new();
        for container in containers {
            for c in children(root, container) {
                for el in c.children().filter(Node::is_element) {
                    by_name.entry(el.tag_name().name()).or_default().push(el);
                }
            }
        }
        Self { by_name }
    }

    fn all(&self, name: &str) -> &[Node<'a, 'i>] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    /// Records of `name` belonging to `part` (`None` = the score's own,
    /// untagged records).
    fn for_part(&self, name: &str, part: Option<u32>) -> Vec<Node<'a, 'i>> {
        let want = part.map(|p| p.to_string());
        self.all(name)
            .iter()
            .copied()
            .filter(|n| n.attribute("part").map(str::to_string) == want)
            .collect()
    }

    fn by_cmper(&self, name: &str, cmper: &str) -> Option<Node<'a, 'i>> {
        self.all(name)
            .iter()
            .copied()
            .find(|n| n.attribute("cmper") == Some(cmper) && n.attribute("part").is_none())
    }
}

fn int(node: Node, name: &str) -> Option<i32> {
    path_text(node, &[name]).and_then(|s| s.trim().parse().ok())
}

fn cmper(node: Node) -> Option<u32> {
    attr(node, "cmper").and_then(|s| s.parse().ok())
}

fn read_layout(root: Node) -> DocumentLayout {
    let idx = Index::new(root, &["others", "details", "texts", "options"]);
    let fonts = font_table(&idx);

    let file_info = read_file_info(&idx);

    // Part membership: the staves listed in each part's scroll-view
    // instrument list (`instUsed` cmper 0, part-tagged).
    let mut part_staves: BTreeMap<u32, Vec<(i32, u32)>> = BTreeMap::new();
    for iu in idx.all("instUsed") {
        if attr(*iu, "cmper").as_deref() != Some("0") {
            continue;
        }
        let Some(part) = attr(*iu, "part").and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        let Some(inst) = int(*iu, "inst") else {
            continue;
        };
        let inci = attr(*iu, "inci").and_then(|s| s.parse().ok()).unwrap_or(0);
        part_staves
            .entry(part)
            .or_default()
            .push((inci, inst as u32));
    }
    let score_staves: Vec<u32> = {
        let mut v: Vec<(i32, u32)> = idx
            .for_part("instUsed", None)
            .into_iter()
            .filter(|n| attr(*n, "cmper").as_deref() == Some("0"))
            .filter_map(|n| {
                let inci = attr(n, "inci").and_then(|s| s.parse().ok()).unwrap_or(0);
                int(n, "inst").map(|i| (inci, i as u32))
            })
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, s)| s).collect()
    };

    let staff_names = staff_full_names(&idx, &fonts);
    let staff_ids: Vec<(u32, String)> = crate::converter::staff_part_ids(root)
        .into_iter()
        .filter_map(|(c, id)| Some((c.parse().ok()?, id)))
        .collect();
    let id_of_staff: HashMap<u32, String> = staff_ids.iter().cloned().collect();
    // A part's staves as staff numbers of its first converted part.
    let score_staves_of = |staves: &[u32]| -> Vec<u8> {
        let Some(first) = staves.iter().find_map(|s| id_of_staff.get(s)) else {
            return Vec::new();
        };
        let order: Vec<u32> = staff_ids
            .iter()
            .filter(|(_, id)| id == first)
            .map(|(c, _)| *c)
            .collect();
        staves
            .iter()
            .filter_map(|s| order.iter().position(|c| c == s))
            .map(|i| i as u8 + 1)
            .collect()
    };
    let part_ids_of = |staves: &[u32]| -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for s in staves {
            if let Some(id) = id_of_staff.get(s) {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
        }
        ids
    };
    let hidden_numbers: std::collections::HashSet<u32> = idx
        .for_part("staffSpec", None)
        .into_iter()
        .filter(|s| has_child(*s, "hideMeasNums"))
        .filter_map(cmper)
        .collect();

    let mut score = None;
    let mut parts = Vec::new();
    for def in idx.all("partDef") {
        let Some(number) = cmper(*def) else {
            continue;
        };
        let staves: Vec<u32> = if number == 0 {
            score_staves.clone()
        } else {
            let mut v = part_staves.get(&number).cloned().unwrap_or_default();
            v.sort_unstable();
            v.into_iter().map(|(_, s)| s).collect()
        };
        let name = part_name(&idx, &fonts, *def, &staves, &staff_names);
        let layout_part = (number != 0).then_some(number);
        let layout = PartLayout {
            number,
            order: int(*def, "partOrder").unwrap_or(number as i32) as u32,
            name,
            extract: has_child(*def, "extractPart") || number == 0,
            pages: read_pages(&idx, layout_part),
            systems: read_systems(&idx, layout_part),
            multirests: read_multirests(&idx, layout_part),
            measure_widths: read_measure_widths(&idx, layout_part),
            hide_measure_numbers: !staves.is_empty()
                && staves.iter().all(|s| hidden_numbers.contains(s)),
            part_ids: part_ids_of(&staves),
            score_staves: score_staves_of(&staves),
            page_texts: read_page_texts(&idx, &fonts, layout_part),
            staves,
        };
        if number == 0 {
            score = Some(layout);
        } else {
            parts.push(layout);
        }
    }
    parts.sort_by_key(|p| p.order);

    let score = score.unwrap_or_else(|| PartLayout {
        number: 0,
        order: 0,
        name: "Score".to_string(),
        part_ids: part_ids_of(&score_staves),
        score_staves: score_staves_of(&score_staves),
        page_texts: read_page_texts(&idx, &fonts, None),
        staves: score_staves,
        extract: true,
        pages: read_pages(&idx, None),
        systems: read_systems(&idx, None),
        multirests: read_multirests(&idx, None),
        measure_widths: read_measure_widths(&idx, None),
        hide_measure_numbers: false,
    });

    DocumentLayout {
        file_info,
        score,
        parts,
        page_texts: read_page_texts(&idx, &fonts, None),
        measure_numbers: read_measure_numbers(&idx, &fonts),
    }
}

fn font_table(idx: &Index) -> HashMap<u32, String> {
    idx.all("fontName")
        .iter()
        .filter_map(|n| Some((cmper(*n)?, path_text(*n, &["name"])?)))
        .collect()
}

fn read_file_info(idx: &Index) -> FileInfo {
    let mut info = FileInfo::default();
    for fi in idx.all("fileInfo") {
        let Some(value) = text(*fi)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        match attr(*fi, "type").as_deref() {
            Some("title") => info.title = Some(value),
            Some("subtitle") => info.subtitle = Some(value),
            Some("composer") => info.composer = Some(value),
            Some("arranger") => info.arranger = Some(value),
            Some("lyricist") => info.lyricist = Some(value),
            Some("copyright") => info.copyright = Some(value),
            _ => {}
        }
    }
    info
}

/// Text of a `textBlock` (via its `textID` → `texts/blockText`).
fn block_text(idx: &Index, block_cmper: &str) -> Option<String> {
    let block = idx.by_cmper("textBlock", block_cmper)?;
    let text_id = path_text(block, &["textID"])?;
    idx.all("blockText")
        .iter()
        .find(|n| attr(**n, "number").as_deref() == Some(text_id.as_str()))
        .and_then(|n| text(*n))
}

fn staff_full_names(idx: &Index, fonts: &HashMap<u32, String>) -> HashMap<u32, String> {
    let mut names = HashMap::new();
    for spec in idx.for_part("staffSpec", None) {
        let Some(c) = cmper(spec) else {
            continue;
        };
        if let Some(name) = path_text(spec, &["fullName"])
            .and_then(|id| block_text(idx, &id))
            .map(|t| parse_enigma_text(&t, fonts).literal())
            .filter(|n| !n.trim().is_empty())
        {
            names.insert(c, name.trim().to_string());
        }
    }
    // Finale's automatic instrument numbering: same-named staves flagged
    // `useAutoNum` become "Flute 1", "Flute 2"… (or "Violin I", "II" with
    // `romanSuffix`), counted in score order.
    let mut auto: Vec<(u32, String, String)> = Vec::new(); // (staff, base name, style)
    for spec in idx.for_part("staffSpec", None) {
        let Some(c) = cmper(spec) else {
            continue;
        };
        if !has_child(spec, "useAutoNum") {
            continue;
        }
        if let Some(name) = names.get(&c) {
            let style = path_text(spec, &["autoNum"]).unwrap_or_else(|| "arabicSuffix".into());
            auto.push((c, name.clone(), style));
        }
    }
    let score_order: Vec<u32> = idx
        .for_part("instUsed", None)
        .into_iter()
        .filter(|n| attr(*n, "cmper").as_deref() == Some("0"))
        .filter_map(|n| int(n, "inst").map(|i| i as u32))
        .collect();
    auto.sort_by_key(|(c, _, _)| {
        score_order
            .iter()
            .position(|s| s == c)
            .unwrap_or(usize::MAX)
    });
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, base, _) in &auto {
        *counts.entry(base.clone()).or_default() += 1;
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (c, base, style) in &auto {
        if counts.get(base).copied().unwrap_or(0) < 2 {
            continue;
        }
        let e = seen.entry(base.clone()).or_default();
        *e += 1;
        let n = *e;
        names.insert(*c, auto_number(base, n, style));
    }

    // Grand-staff instruments (harp, piano) name the group, not the staves.
    for group in idx.for_part("staffGroup", None) {
        let (Some(start), Some(end)) = (int(group, "startInst"), int(group, "endInst")) else {
            continue;
        };
        if start == end {
            continue;
        }
        let Some(name) = path_text(group, &["fullID"])
            .and_then(|id| block_text(idx, &id))
            .map(|t| parse_enigma_text(&t, fonts).literal())
            .filter(|n| !n.trim().is_empty())
        else {
            continue;
        };
        names
            .entry(start as u32)
            .or_insert_with(|| name.trim().to_string());
    }
    names
}

/// Finale's auto-numbered instrument name.
fn auto_number(base: &str, n: usize, style: &str) -> String {
    const ROMAN: [&str; 12] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
    ];
    let ordinal = |n: usize| -> String {
        let suffix = match (n % 10, n % 100) {
            (_, 11..=13) => "th",
            (1, _) => "st",
            (2, _) => "nd",
            (3, _) => "rd",
            _ => "th",
        };
        format!("{n}{suffix}")
    };
    match style {
        "romanSuffix" => format!("{base} {}", ROMAN.get(n - 1).copied().unwrap_or("?")),
        "ordinalPrefix" => format!("{} {base}", ordinal(n)),
        "alphaSuffix" => format!("{base} {}", char::from(b'A' + ((n - 1) % 26) as u8)),
        "arabicPrefix" => format!("{n}. {base}"),
        _ => format!("{base} {n}"),
    }
}

fn part_name(
    idx: &Index,
    fonts: &HashMap<u32, String>,
    def: Node,
    staves: &[u32],
    staff_names: &HashMap<u32, String>,
) -> String {
    // nameID ≠ 0 → the part has its own name block.
    if let Some(id) = int(def, "nameID").filter(|id| *id != 0)
        && let Some(t) = block_text(idx, &id.to_string())
    {
        let name = parse_enigma_text(&t, fonts).literal();
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    let named_staff = int(def, "defaultNameStaff").map(|s| s as u32);
    named_staff
        .into_iter()
        .chain(staves.iter().copied())
        .find_map(|s| staff_names.get(&s).cloned())
        .or_else(|| {
            // Grouped instruments name the group (harp: `defaultNameGroup`).
            let g = int(def, "defaultNameGroup")?;
            let group = idx.for_part("staffGroup", None).into_iter().find(|n| {
                attr(*n, "cmper2").and_then(|s| s.parse::<i32>().ok()) == Some(g)
                    || cmper(*n).map(|c| c as i32) == Some(g)
            })?;
            let t = block_text(idx, &path_text(group, &["fullID"])?)?;
            Some(parse_enigma_text(&t, fonts).literal().trim().to_string())
        })
        .unwrap_or_default()
}

fn read_pages(idx: &Index, part: Option<u32>) -> Vec<PageSpec> {
    let mut pages: Vec<(u32, PageSpec)> = idx
        .for_part("pageSpec", part)
        .into_iter()
        .filter_map(|n| {
            let c = cmper(n)?;
            // pageSpec 0 is the page-format template, not a page.
            (c > 0).then(|| {
                (
                    c,
                    PageSpec {
                        width: int(n, "width").unwrap_or(2448),
                        height: int(n, "height").unwrap_or(3168),
                        margin_top: int(n, "margTop").unwrap_or(0).abs(),
                        margin_left: int(n, "margLeft").unwrap_or(0).abs(),
                        margin_bottom: int(n, "margBottom").unwrap_or(0).abs(),
                        margin_right: int(n, "margRight").unwrap_or(0).abs(),
                        first_system: int(n, "firstSystem").unwrap_or(1).max(1) as u32,
                    },
                )
            })
        })
        .collect();
    pages.sort_by_key(|(c, _)| *c);
    pages.into_iter().map(|(_, p)| p).collect()
}

fn read_systems(idx: &Index, part: Option<u32>) -> Vec<SystemSpec> {
    let mut systems: Vec<(u32, SystemSpec)> = idx
        .for_part("staffSystemSpec", part)
        .into_iter()
        .filter_map(|n| {
            let c = cmper(n)?;
            Some((
                c,
                SystemSpec {
                    start_measure: int(n, "startMeas")? as u32,
                    end_measure: int(n, "endMeas")? as u32,
                    staff_height: int(n, "staffHeight").unwrap_or(DEFAULT_STAFF_HEIGHT_EFIX),
                    left: int(n, "left").unwrap_or(0),
                    right: int(n, "right").unwrap_or(0),
                    top: int(n, "top").unwrap_or(0).abs(),
                    bottom: int(n, "bottom").unwrap_or(0).abs(),
                    distance_to_prev: int(n, "distanceToPrev").unwrap_or(0).abs(),
                },
            ))
        })
        .collect();
    systems.sort_by_key(|(c, _)| *c);
    systems.into_iter().map(|(_, s)| s).collect()
}

fn read_multirests(idx: &Index, part: Option<u32>) -> Vec<MultiRestSpec> {
    let mut rests: Vec<MultiRestSpec> = idx
        .for_part("mmRest", part)
        .into_iter()
        .filter_map(|n| {
            let start = cmper(n)?;
            let end = int(n, "nextMeas")? as u32;
            (end > start).then(|| MultiRestSpec {
                start_measure: start,
                end_measure: end,
                width: int(n, "meaSpace").unwrap_or(360),
            })
        })
        .collect();
    rests.sort_by_key(|r| r.start_measure);
    rests
}

fn read_measure_widths(idx: &Index, part: Option<u32>) -> BTreeMap<u32, i32> {
    // Part measSpecs override the score's per measure; inherit the rest.
    let mut widths: BTreeMap<u32, i32> = idx
        .for_part("measSpec", None)
        .into_iter()
        .filter_map(|n| Some((cmper(n)?, int(n, "width")?)))
        .collect();
    if part.is_some() {
        for n in idx.for_part("measSpec", part) {
            if let (Some(c), Some(w)) = (cmper(n), int(n, "width")) {
                widths.insert(c, w);
            }
        }
    }
    widths
}

/// Page text as `part` shows it (`None` = the score): the score's
/// assignments, with any part-specific (`shared="false"`) record replacing
/// the score's record of the same `(cmper, inci)` — a part can move, hide,
/// or retext a header. Empty `shared="true"` records just mean "as score".
fn read_page_texts(idx: &Index, fonts: &HashMap<u32, String>, part: Option<u32>) -> Vec<PageText> {
    let key = |n: Node| (attr(n, "cmper"), attr(n, "inci"));
    let score_records = idx.for_part("pageTextAssign", None);
    let overrides: Vec<Node> = match part {
        Some(p) => idx
            .for_part("pageTextAssign", Some(p))
            .into_iter()
            .filter(|n| attr(*n, "shared").as_deref() == Some("false"))
            .collect(),
        None => Vec::new(),
    };
    // When the document carries part-scoped page-text records at all, a
    // part shows only the score texts it has a record for (shared or its
    // own) — score-only labels stay off the parts. Documents without any
    // part records share every page text.
    let part_records: Vec<Node> = match part {
        Some(p) => idx.for_part("pageTextAssign", Some(p)),
        None => Vec::new(),
    };
    let any_part_records = idx
        .all("pageTextAssign")
        .iter()
        .any(|n| n.attribute("part").is_some());
    let mut out = Vec::new();
    for score_n in score_records {
        if part.is_some()
            && any_part_records
            && !part_records.iter().any(|o| key(*o) == key(score_n))
        {
            continue;
        }
        let n = overrides
            .iter()
            .copied()
            .find(|o| key(*o) == key(score_n))
            .unwrap_or(score_n);
        // A part record may omit fields it does not change: fall back to
        // the score record for each.
        let field = |name: &str| path_text(n, &[name]).or_else(|| path_text(score_n, &[name]));
        let int_field = |name: &str| field(name).and_then(|s| s.trim().parse::<i32>().ok());
        if has_child(n, "hidden") {
            continue;
        }
        let Some(block) = field("block") else {
            continue;
        };
        let Some(raw) = block_text(idx, &block) else {
            continue;
        };
        // cmper 0 = a multi-page assignment; cmper N = page N only.
        let page = cmper(score_n).unwrap_or(0);
        let start_page = int_field("startPage")
            .map(|p| p.max(1) as u32)
            .unwrap_or(page.max(1));
        let end_page = match int_field("endPage") {
            Some(e) if e > 0 => Some(e as u32),
            _ if page > 0 => Some(page),
            _ => None,
        };
        let h_align = match field("hposLp").as_deref() {
            Some("center") => HAlign::Center,
            Some("right") => HAlign::Right,
            _ => HAlign::Left,
        };
        let v_align = match field("vpos").as_deref() {
            Some("center") => VAlign::Center,
            Some("bottom") => VAlign::Bottom,
            _ => VAlign::Top,
        };
        out.push(PageText {
            text: parse_enigma_text(&raw, fonts),
            start_page,
            end_page,
            h_align,
            v_align,
            x: int_field("xdisp").unwrap_or(0),
            y: int_field("ydisp").unwrap_or(0),
        });
    }
    out
}

fn read_font(node: Option<Node>, fonts: &HashMap<u32, String>) -> TextStyle {
    let Some(node) = node else {
        return TextStyle::default();
    };
    TextStyle {
        family: int(node, "fontID").and_then(|id| fonts.get(&(id as u32)).cloned()),
        size: int(node, "fontSize").map(f64::from),
        bold: child(node, "efx").is_some_and(|e| has_child(e, "bold")),
        italic: child(node, "efx").is_some_and(|e| has_child(e, "italic")),
    }
}

fn read_number_style(node: Option<Node>, fonts: &HashMap<u32, String>) -> MeasureNumberStyle {
    let Some(n) = node else {
        return MeasureNumberStyle::default();
    };
    MeasureNumberStyle {
        multiple_of: has_child(n, "multipleOf"),
        incidence: int(n, "incidence").unwrap_or(1).max(1) as u32,
        except_first: has_child(n, "exceptFirstMeas"),
        multirest_range: has_child(n, "mmRestRange"),
        start_font: read_font(child(n, "startFont"), fonts),
        multiple_font: read_font(child(n, "multipleFont"), fonts),
        multirest_font: read_font(child(n, "mmRestFont"), fonts),
        start_x: int(n, "startXdisp").unwrap_or(0),
        start_y: int(n, "startYdisp").unwrap_or(0),
        multiple_x: int(n, "multipleXdisp").unwrap_or(0),
        multiple_y: int(n, "multipleYdisp").unwrap_or(0),
        multirest_x: int(n, "mmRestXdisp").unwrap_or(0),
        multirest_y: int(n, "mmRestYdisp").unwrap_or(0),
    }
}

fn read_measure_numbers(idx: &Index, fonts: &HashMap<u32, String>) -> Vec<MeasureNumberRegion> {
    idx.all("measNumbRegion")
        .iter()
        .filter(|n| n.attribute("part").is_none())
        .filter_map(|n| {
            Some(MeasureNumberRegion {
                start_measure: int(*n, "startMeas")? as u32,
                end_measure: int(*n, "endMeas")? as u32,
                parts: read_number_style(child(*n, "partData"), fonts),
                score: read_number_style(child(*n, "scoreData"), fonts),
            })
        })
        .collect()
}

/// Parse Finale enigma text: `^tag(args)` commands interleaved with literal
/// text. Font commands set [`EnigmaText::style`] (first one wins for each
/// attribute, which is the style of the visible text in every block Finale
/// writes for page text and expressions).
#[must_use]
pub fn parse_enigma_text(raw: &str, fonts: &HashMap<u32, String>) -> EnigmaText {
    let mut out = EnigmaText::default();
    // Style as the commands so far set it.
    let mut current = TextStyle::default();
    let mut line = EnigmaLine::default();
    let mut line_started = false; // a visible piece is on this line
    let mut literal = String::new();
    let mut seen_text = false;

    fn flush(literal: &mut String, line: &mut EnigmaLine) {
        if !literal.is_empty() {
            line.pieces
                .push(TextPiece::Literal(std::mem::take(literal)));
        }
    }
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\r' {
            i += 1;
            continue;
        }
        if c == '\n' {
            flush(&mut literal, &mut line);
            out.lines.push(std::mem::take(&mut line));
            line.style = current.clone();
            line_started = false;
            i += 1;
            continue;
        }
        if c != '^' {
            if !line_started {
                line.style = current.clone();
                line_started = true;
            }
            if !seen_text {
                out.style = current.clone();
                seen_text = true;
            }
            literal.push(c);
            i += 1;
            continue;
        }
        // `^^` is a literal caret.
        if chars.get(i + 1) == Some(&'^') {
            literal.push('^');
            i += 2;
            continue;
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < chars.len() && chars[j].is_ascii_alphanumeric() {
            j += 1;
        }
        let name: String = chars[name_start..j].iter().collect();
        let mut args = String::new();
        if chars.get(j) == Some(&'(') {
            let mut k = j + 1;
            while k < chars.len() && chars[k] != ')' {
                args.push(chars[k]);
                k += 1;
            }
            j = (k + 1).min(chars.len());
        }
        i = j;

        match name.as_str() {
            "fontid" => {
                if let Ok(id) = args.trim().parse::<u32>() {
                    current.family = fonts.get(&id).cloned().or(current.family.take());
                }
            }
            "font" | "fontTxt" => {
                if let Some(family) = args.split(',').next().map(str::trim)
                    && !family.is_empty()
                {
                    current.family = Some(family.to_string());
                }
            }
            "size" => {
                if let Ok(size) = args.trim().parse::<f64>() {
                    current.size = Some(size);
                }
            }
            "nfx" => {
                if let Ok(fx) = args.trim().parse::<u32>() {
                    current.bold = fx & 1 != 0;
                    current.italic = fx & 2 != 0;
                }
            }
            "fontMus" => {}
            "flat" => literal.push('\u{266D}'),
            "sharp" => literal.push('\u{266F}'),
            "natural" => literal.push('\u{266E}'),
            insert => {
                let piece = match insert {
                    "title" => Some(TextPiece::Title),
                    "subtitle" => Some(TextPiece::Subtitle),
                    "composer" => Some(TextPiece::Composer),
                    "arranger" => Some(TextPiece::Arranger),
                    "lyricist" => Some(TextPiece::Lyricist),
                    "copyright" => Some(TextPiece::Copyright),
                    "partname" => Some(TextPiece::PartName),
                    "page" => Some(TextPiece::Page),
                    "totpages" => Some(TextPiece::TotalPages),
                    "date" => Some(TextPiece::Date),
                    "filename" => Some(TextPiece::FileName),
                    _ => None,
                };
                if let Some(piece) = piece {
                    if !line_started {
                        line.style = current.clone();
                        line_started = true;
                    }
                    if !seen_text {
                        out.style = current.clone();
                        seen_text = true;
                    }
                    flush(&mut literal, &mut line);
                    line.pieces.push(piece);
                }
            }
        }
    }
    flush(&mut literal, &mut line);
    out.lines.push(line);
    if !seen_text {
        out.style = current;
    }
    // Run the lines together (no separators) for single-line consumers,
    // merging adjacent literals.
    for l in &out.lines {
        for p in &l.pieces {
            match (out.pieces.last_mut(), p) {
                (Some(TextPiece::Literal(prev)), TextPiece::Literal(next)) => prev.push_str(next),
                _ => out.pieces.push(p.clone()),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> HashMap<u32, String> {
        HashMap::from([(2, "Times New Roman".to_string())])
    }

    #[test]
    fn parses_page_header_template() {
        let t = parse_enigma_text(
            "^fontid(2)^size(12)^nfx(0)^title() - ^partname() - Pg. ^page(0)",
            &fonts(),
        );
        assert_eq!(t.style.family.as_deref(), Some("Times New Roman"));
        assert_eq!(t.style.size, Some(12.0));
        assert!(!t.style.bold);
        assert_eq!(
            t.pieces,
            vec![
                TextPiece::Title,
                TextPiece::Literal(" - ".into()),
                TextPiece::PartName,
                TextPiece::Literal(" - Pg. ".into()),
                TextPiece::Page,
            ]
        );
    }

    #[test]
    fn parses_bold_expression_and_symbols() {
        let t = parse_enigma_text(
            "^fontTxt(Times New Roman,8192)^size(14)^nfx(1)Perc. Loop",
            &fonts(),
        );
        assert!(t.style.bold);
        assert_eq!(t.style.size, Some(14.0));
        assert_eq!(t.literal(), "Perc. Loop");

        let t = parse_enigma_text(
            "^fontid(2)^size(12)^nfx(0)Clarinet in B^size(18)^flat()",
            &fonts(),
        );
        assert_eq!(t.literal(), "Clarinet in B\u{266D}");
        // A later size change does not restyle the block.
        assert_eq!(t.style.size, Some(12.0));
    }

    #[test]
    fn splits_lines_with_their_own_styles() {
        let t = parse_enigma_text(
            "^fontid(2)^size(24)^nfx(5)\"Cool Change\"^size(18)^nfx(0)\n^size(12)Arranged by Tom",
            &fonts(),
        );
        assert_eq!(t.lines.len(), 2);
        assert_eq!(t.lines[0].style.size, Some(24.0));
        assert!(t.lines[0].style.bold);
        assert_eq!(t.lines[1].style.size, Some(12.0));
        assert!(!t.lines[1].style.bold);
        assert_eq!(t.literal(), "\"Cool Change\"Arranged by Tom");
    }

    #[test]
    fn reads_part_layout_records() {
        let enigma = r#"<?xml version="1.0" encoding="UTF-8"?>
<finale xmlns="http://www.makemusic.com/2012/finale">
  <others>
    <fontName cmper="2"><name>Times New Roman</name></fontName>
    <staffSpec cmper="34"><fullName>5</fullName><hideMeasNums/></staffSpec>
    <textBlock cmper="5"><textID>9</textID></textBlock>
    <instUsed cmper="0" inci="0"><inst>34</inst></instUsed>
    <instUsed cmper="0" inci="0" part="23" shared="false"><inst>34</inst></instUsed>
    <partDef cmper="0"><nameID>1</nameID><partOrder>0</partOrder></partDef>
    <partDef cmper="23"><nameID>0</nameID><partOrder>23</partOrder><extractPart/><defaultNameStaff>34</defaultNameStaff></partDef>
    <pageSpec cmper="1" part="23" shared="false"><height>3456</height><width>2592</width><firstSystem>1</firstSystem><margTop>-180</margTop><margLeft>144</margLeft><margBottom>180</margBottom><margRight>-144</margRight></pageSpec>
    <staffSystemSpec cmper="1" part="23" shared="false"><startMeas>1</startMeas><endMeas>7</endMeas><staffHeight>5532</staffHeight><top>-432</top><left>122</left><bottom>-200</bottom></staffSystemSpec>
    <mmRest cmper="1" part="23" shared="false"><meaSpace>360</meaSpace><nextMeas>3</nextMeas></mmRest>
    <measSpec cmper="8"><width>200</width></measSpec>
    <measSpec cmper="8" part="23" shared="false"><width>378</width></measSpec>
  </others>
  <texts>
    <fileInfo type="title">Don&apos;t Answer Me</fileInfo>
    <fileInfo type="composer">Alan Parsons</fileInfo>
    <blockText number="9">^fontid(2)^size(12)^nfx(0)Viola</blockText>
  </texts>
</finale>"#;
        let layout = document_layout(enigma.as_bytes()).unwrap();
        assert_eq!(layout.file_info.title.as_deref(), Some("Don't Answer Me"));
        assert_eq!(layout.parts.len(), 1);
        let viola = &layout.parts[0];
        assert_eq!(viola.name, "Viola");
        assert_eq!(viola.staves, vec![34]);
        assert!(viola.hide_measure_numbers);
        assert_eq!(viola.pages[0].width, 2592);
        assert_eq!(viola.pages[0].margin_top, 180);
        assert_eq!(viola.systems[0].start_measure, 1);
        assert_eq!(viola.systems[0].end_measure, 7);
        assert_eq!(viola.systems[0].top, 432);
        assert!((viola.systems[0].spatium() - 21.609).abs() < 0.01);
        assert_eq!(
            viola.multirests,
            vec![MultiRestSpec {
                start_measure: 1,
                end_measure: 3,
                width: 360
            }]
        );
        assert_eq!(viola.measure_widths.get(&8), Some(&378));
    }
}
