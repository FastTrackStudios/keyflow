//! Finale `.musx` → engraved parts, formatted as the arranger formatted them.
//!
//! The notes come through MusicXML ([`keyflow_lang::musx::musx_to_musicxml`] then
//! [`crate::import`]); the layout comes straight from Finale's linked-part
//! records ([`keyflow_lang::musx::layout`]): each part's own system breaks, page
//! turns, multimeasure rests, measure widths, staff size, page text and
//! measure-number style. This module turns the second into
//! [`LayoutOptions`] for the first.
//!
//! Vertical placement, measured against Finale's own PDFs (EVPU):
//! - first system on a page: top staff line at
//!   `margin_top + system.top + system.distance_to_prev`;
//! - every later system: previous staff top + `bottom × staff scale` +
//!   `top` + `distance_to_prev` (only the staff-relative `bottom` scales
//!   with the staff size);
//! - x from `margin_left + system.left` to `width − margin_right − right`.

use keyflow_lang::musx::layout::{
    self as fl, DocumentLayout, MeasureNumberRegion, PartLayout as FinalePart, TextPiece, TextStyle,
};

use crate::import::{ImportError, import_bytes};
use crate::layout::{
    ExplicitLayout, ExplicitMultiRest, ExplicitSystem, HAlign, LayoutOptions, Margins,
    MeasureNumberMode, MeasureNumbers, PageText, TextFont, VAlign,
};
use crate::model::Score;

/// EVPU → points.
fn pt(evpu: f64) -> f64 {
    evpu / 4.0
}

#[derive(Debug, thiserror::Error)]
pub enum FinaleError {
    #[error("musx: {0}")]
    Musx(#[from] keyflow_lang::musx::Error),
    #[error("import: {0}")]
    Import(#[from] ImportError),
}

/// A Finale document: the score's notes and its layout.
pub struct FinaleDocument {
    pub score: Score,
    pub layout: DocumentLayout,
}

impl FinaleDocument {
    /// Decode a `.musx` archive.
    pub fn from_musx(musx: &[u8]) -> Result<Self, FinaleError> {
        let (enigma, metadata) = keyflow_lang::musx::musx_parts(musx)?;
        let layout = fl::document_layout(&enigma)?;
        let musicxml = keyflow_lang::musx::enigmaxml_to_musicxml(&enigma, Some(&metadata))?;
        let score = import_bytes(musicxml.into_bytes())?;
        Ok(Self { score, layout })
    }

    /// The printed title (see [`DocumentLayout::printed_title`]).
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.layout
            .printed_title()
            .or_else(|| self.score.work_title.clone())
    }

    /// Every part Finale extracts, in part order, ready to lay out. A file
    /// with no linked parts (a rhythm chart) yields one job per score
    /// staff, laid out automatically.
    #[must_use]
    pub fn jobs(&self, print: &PrintInfo) -> Vec<PartJob> {
        let linked = self.parts(print);
        if !linked.is_empty() {
            return linked;
        }
        self.score
            .parts
            .iter()
            .enumerate()
            .map(|(i, p)| PartJob {
                name: p.name.clone(),
                part_index: i,
                options: LayoutOptions::default(),
                staves: usize::from(p.staves),
            })
            .collect()
    }

    /// Every part of the document, in Finale's part order, ready to lay out.
    #[must_use]
    pub fn parts(&self, print: &PrintInfo) -> Vec<PartJob> {
        // Every named part, not just Finale's `extractPart` ones: that flag
        // only records which parts were ticked in the last print run.
        // Same-named parts (a one-staff and a two-staff "Harp") keep the
        // fuller one.
        let mut parts: Vec<&FinalePart> = Vec::new();
        for p in self
            .layout
            .parts
            .iter()
            .filter(|p| !p.name.trim().is_empty())
        {
            match parts.iter_mut().find(|q| q.name == p.name) {
                Some(q) if p.staves.len() > q.staves.len() => *q = p,
                Some(_) => {}
                None => parts.push(p),
            }
        }
        parts
            .into_iter()
            .filter_map(|p| self.part_job(p, print))
            .collect()
    }

    /// Layout options for one Finale part, and the score part it engraves.
    #[must_use]
    pub fn part_job(&self, part: &FinalePart, print: &PrintInfo) -> Option<PartJob> {
        let first_id = part.part_ids.first()?;
        let part_index = self.score.parts.iter().position(|p| &p.id == first_id)?;
        Some(PartJob {
            name: part.name.clone(),
            part_index,
            options: part_options(&self.layout, part, &self.score, print),
            staves: part.staves.len(),
        })
    }
}

/// Values for page-text inserts that are not in the document.
#[derive(Debug, Clone, Default)]
pub struct PrintInfo {
    /// `^date()` — Finale prints the print date; a revision stamp ("Rev.
    /// 1/21/2015") reads best as the file's last-modified date.
    pub date: String,
    pub file_name: String,
}

/// One part to engrave.
#[derive(Debug, Clone)]
pub struct PartJob {
    /// Finale's part name ("Horn in F 1-2"), for headers and file names.
    pub name: String,
    /// Index into [`Score::parts`].
    pub part_index: usize,
    pub options: LayoutOptions,
    /// Finale staves in the part (2 = grand staff; layout draws staff 1).
    pub staves: usize,
}

/// Map a Finale font to a face we ship: serif for Times-style text (the
/// default), FreeSans for the sans families.
fn text_font(style: &TextStyle, default_size: f64) -> TextFont {
    let sans = style.family.as_deref().is_some_and(|f| {
        let f = f.to_ascii_lowercase();
        ["arial", "helvetica", "sans"].iter().any(|s| f.contains(s))
    });
    TextFont {
        family: if sans {
            "FreeSans".to_string()
        } else {
            crate::fonts::SERIF_FAMILY.to_string()
        },
        size: style.size.unwrap_or(default_size),
        bold: style.bold,
        italic: style.italic,
    }
}

fn resolve_text(
    pieces: &[TextPiece],
    layout: &DocumentLayout,
    part: &FinalePart,
    print: &PrintInfo,
) -> String {
    let info = &layout.file_info;
    let or_empty = |s: &Option<String>| s.clone().unwrap_or_default();
    pieces
        .iter()
        .map(|p| match p {
            TextPiece::Literal(s) => s.clone(),
            TextPiece::Title => or_empty(&info.title),
            TextPiece::Subtitle => or_empty(&info.subtitle),
            TextPiece::Composer => or_empty(&info.composer),
            TextPiece::Arranger => or_empty(&info.arranger),
            TextPiece::Lyricist => or_empty(&info.lyricist),
            TextPiece::Copyright => or_empty(&info.copyright),
            TextPiece::PartName => part.name.clone(),
            TextPiece::Page => "{page}".to_string(),
            TextPiece::TotalPages => "{pages}".to_string(),
            TextPiece::Date => print.date.clone(),
            TextPiece::FileName => print.file_name.clone(),
        })
        .collect()
}

/// Line advance for a line of `size` pt (Finale's 100% line spacing).
fn line_height(size: f64) -> f64 {
    size * 1.15
}

fn page_texts(layout: &DocumentLayout, part: &FinalePart, print: &PrintInfo) -> Vec<PageText> {
    let mut out = Vec::new();
    for t in &part.page_texts {
        let h_align = match t.h_align {
            fl::HAlign::Left => HAlign::Left,
            fl::HAlign::Center => HAlign::Center,
            fl::HAlign::Right => HAlign::Right,
        };
        let v_align = match t.v_align {
            fl::VAlign::Top => VAlign::Top,
            fl::VAlign::Center => VAlign::Center,
            fl::VAlign::Bottom => VAlign::Bottom,
        };
        // One PageText per line, stacked down from the block's anchor
        // (up from it, for a bottom-anchored block).
        let lines: Vec<(TextFont, String)> = t
            .text
            .lines
            .iter()
            .map(|l| {
                (
                    text_font(&l.style, t.text.style.size.unwrap_or(12.0)),
                    resolve_text(&l.pieces, layout, part, print),
                )
            })
            .collect();
        let heights: Vec<f64> = lines.iter().map(|(f, _)| line_height(f.size)).collect();
        // Finale's y points up; ours points down.
        let anchor_y = -pt(f64::from(t.y));
        for (i, (font, text)) in lines.into_iter().enumerate() {
            if text.trim().is_empty() {
                continue;
            }
            let y = match v_align {
                VAlign::Bottom => anchor_y - heights[i + 1..].iter().sum::<f64>(),
                _ => anchor_y + heights[..i].iter().sum::<f64>(),
            };
            out.push(PageText {
                text,
                font,
                first_page: t.start_page as usize,
                last_page: t.end_page.map(|p| p as usize),
                h_align,
                v_align,
                x: pt(f64::from(t.x)),
                y,
            });
        }
    }
    out
}

fn measure_numbers(regions: &[MeasureNumberRegion], part: &FinalePart) -> MeasureNumbers {
    if part.hide_measure_numbers {
        return MeasureNumbers {
            mode: MeasureNumberMode::None,
            ..MeasureNumbers::default()
        };
    }
    // Finale shows a measure's number from the first region covering it.
    let Some(region) = regions
        .iter()
        .filter(|r| r.start_measure <= 1 && r.end_measure >= 1)
        .min_by_key(|r| r.start_measure)
        .or_else(|| regions.first())
    else {
        return MeasureNumbers::default();
    };
    let s = &region.parts;
    if s.multiple_of {
        MeasureNumbers {
            mode: MeasureNumberMode::Every(s.incidence),
            exclude_first: s.except_first,
            multirest_ranges: s.multirest_range,
            font: text_font(&s.multiple_font, 10.0),
            multirest_font: text_font(&s.multirest_font, 10.0),
            offset: (pt(f64::from(s.multiple_x)), -pt(f64::from(s.multiple_y))),
            multirest_offset: (pt(f64::from(s.multirest_x)), -pt(f64::from(s.multirest_y))),
        }
    } else {
        MeasureNumbers {
            mode: MeasureNumberMode::SystemStart,
            exclude_first: s.except_first,
            multirest_ranges: false,
            font: text_font(&s.start_font, 10.0),
            multirest_font: text_font(&s.multirest_font, 10.0),
            offset: (pt(f64::from(s.start_x)), -pt(f64::from(s.start_y))),
            multirest_offset: (0.0, 0.0),
        }
    }
}

/// The explicit system/page layout of a part, in points.
fn explicit_layout(
    part: &FinalePart,
    measure_count: usize,
) -> Option<(ExplicitLayout, fl::PageSpec)> {
    let first_page = *part.pages.first()?;
    if part.systems.is_empty() {
        return None;
    }
    // Page of each (1-based) system.
    let page_of = |system: usize| -> usize {
        part.pages
            .iter()
            .rposition(|p| p.first_system as usize <= system)
            .unwrap_or(0)
    };

    let mut systems = Vec::new();
    let mut prev: Option<(usize, f64, &fl::SystemSpec)> = None; // (page, staff top EVPU, spec)
    for (i, sys) in part.systems.iter().enumerate() {
        let page = page_of(i + 1);
        let spec = part.pages.get(page).copied().unwrap_or(first_page);
        let staff_top = match prev {
            Some((prev_page, prev_top, prev_sys)) if prev_page == page => {
                prev_top
                    + f64::from(prev_sys.bottom) * prev_sys.scale()
                    + f64::from(sys.top)
                    + f64::from(sys.distance_to_prev)
            }
            _ => f64::from(spec.margin_top) + f64::from(sys.top) + f64::from(sys.distance_to_prev),
        };
        let start = (sys.start_measure.max(1) - 1) as usize;
        let end = ((sys.end_measure.max(1) - 1) as usize).min(measure_count);
        systems.push(ExplicitSystem {
            measures: start..end.max(start + 1),
            page,
            staff_top: pt(staff_top),
            left: pt(f64::from(spec.margin_left + sys.left)),
            right: pt(f64::from(spec.width - spec.margin_right - sys.right)),
            spatium: pt(sys.spatium()),
        });
        prev = Some((page, staff_top, sys));
    }

    let multirests = part
        .multirests
        .iter()
        .map(|r| ExplicitMultiRest {
            measures: (r.start_measure.max(1) - 1) as usize
                ..((r.end_measure.max(1) - 1) as usize).min(measure_count),
            width: f64::from(r.width),
        })
        .collect();

    // Finale's springs; a measure without its own width takes the part's
    // typical one.
    let mut known: Vec<i32> = part.measure_widths.values().copied().collect();
    known.sort_unstable();
    let typical = known.get(known.len() / 2).copied().unwrap_or(252);
    let measure_widths = (0..measure_count)
        .map(|i| {
            Some(f64::from(
                part.measure_widths
                    .get(&(i as u32 + 1))
                    .copied()
                    .unwrap_or(typical),
            ))
        })
        .collect();

    Some((
        ExplicitLayout {
            systems,
            multirests,
            measure_widths,
        },
        first_page,
    ))
}

/// Build the [`LayoutOptions`] that reproduce a Finale part.
#[must_use]
pub fn part_options(
    layout: &DocumentLayout,
    part: &FinalePart,
    score: &Score,
    print: &PrintInfo,
) -> LayoutOptions {
    let measure_count = part
        .part_ids
        .first()
        .and_then(|id| score.parts.iter().find(|p| &p.id == id))
        .map_or(0, |p| p.measures.len());
    let mut opts = LayoutOptions {
        page_texts: Some(page_texts(layout, part, print)),
        measure_numbers: measure_numbers(&layout.measure_numbers, part),
        staves: (!part.score_staves.is_empty()).then(|| part.score_staves.clone()),
        part_name: Some(part.name.clone()),
        ..LayoutOptions::default()
    };
    if let Some((explicit, page)) = explicit_layout(part, measure_count) {
        opts.page_width = pt(f64::from(page.width));
        opts.page_height = pt(f64::from(page.height));
        opts.margins = Some(Margins {
            top: pt(f64::from(page.margin_top)),
            right: pt(f64::from(page.margin_right)),
            bottom: pt(f64::from(page.margin_bottom)),
            left: pt(f64::from(page.margin_left)),
        });
        if let Some(first) = explicit.systems.first() {
            opts.spatium = first.spatium;
        }
        opts.explicit = Some(explicit);
    }
    opts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viola() -> FinalePart {
        FinalePart {
            number: 23,
            order: 23,
            name: "Viola".into(),
            staves: vec![34],
            part_ids: vec!["P24".into()],
            score_staves: vec![1],
            extract: true,
            pages: vec![
                fl::PageSpec {
                    width: 2592,
                    height: 3456,
                    margin_top: 180,
                    margin_left: 144,
                    margin_bottom: 180,
                    margin_right: 144,
                    first_system: 1,
                },
                fl::PageSpec {
                    width: 2592,
                    height: 3456,
                    margin_top: 180,
                    margin_left: 144,
                    margin_bottom: 180,
                    margin_right: 144,
                    first_system: 3,
                },
            ],
            systems: vec![
                fl::SystemSpec {
                    start_measure: 1,
                    end_measure: 7,
                    staff_height: 5532,
                    left: 122,
                    right: 0,
                    top: 432,
                    bottom: 200,
                    distance_to_prev: 0,
                },
                fl::SystemSpec {
                    start_measure: 7,
                    end_measure: 11,
                    staff_height: 5532,
                    left: 0,
                    right: 0,
                    top: 68,
                    bottom: 200,
                    distance_to_prev: 76,
                },
                fl::SystemSpec {
                    start_measure: 11,
                    end_measure: 15,
                    staff_height: 5532,
                    left: 0,
                    right: 0,
                    top: 68,
                    bottom: 200,
                    distance_to_prev: 76,
                },
            ],
            multirests: vec![fl::MultiRestSpec {
                start_measure: 1,
                end_measure: 3,
                width: 360,
            }],
            measure_widths: [(8, 378)].into_iter().collect(),
            hide_measure_numbers: true,
            page_texts: Vec::new(),
        }
    }

    /// Positions measured off Finale's own PDF of this part (288 dpi = EVPU).
    #[test]
    fn systems_land_where_finale_put_them() {
        let (explicit, page) = explicit_layout(&viola(), 129).unwrap();
        assert_eq!(page.width, 2592);
        let s = &explicit.systems;
        // Page 1: staff tops at 612 and 936 EVPU; page 2 starts at 324.
        assert!(
            (s[0].staff_top - pt(612.0)).abs() < 0.5,
            "{}",
            s[0].staff_top
        );
        assert!(
            (s[1].staff_top - pt(936.0)).abs() < 0.5,
            "{}",
            s[1].staff_top
        );
        assert_eq!(s[2].page, 1);
        assert!(
            (s[2].staff_top - pt(324.0)).abs() < 0.5,
            "{}",
            s[2].staff_top
        );
        // x: 266 on the indented first system, 144 after; right edge 2448.
        assert!((s[0].left - pt(266.0)).abs() < 1e-9);
        assert!((s[1].left - pt(144.0)).abs() < 1e-9);
        assert!((s[1].right - pt(2448.0)).abs() < 1e-9);
        assert_eq!(s[0].measures, 0..6);
        assert_eq!(explicit.multirests[0].measures, 0..2);
        assert_eq!(explicit.measure_widths[7], Some(378.0));
    }
}
