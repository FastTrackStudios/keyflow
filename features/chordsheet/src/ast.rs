//! The chordsheet.com source AST.
//!
//! Lossless enough to round-trip: every line and every bar keeps the byte
//! [`Span`] it came from, and tokens the converter has no keyflow equivalent
//! for (palette commands, rulers, TAB rows, page breaks) survive as their own
//! variants rather than being dropped at parse time. The conversion to a
//! keyflow [`Chart`](keyflow_proto::chart::Chart) is where information is
//! deliberately discarded — see [`crate::convert`].

use std::fmt;

/// Byte range in the original source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub len: usize,
}

impl Span {
    pub const fn new(start: usize, len: usize) -> Self {
        Self { start, len }
    }
    pub const fn end(&self) -> usize {
        self.start + self.len
    }
}

/// A parsed chordsheet.com document: its lines, in source order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub lines: Vec<Line>,
}

impl Document {
    /// Every bar in the document, in source order, flattened across lines.
    pub fn bars(&self) -> impl Iterator<Item = &Bar> {
        self.lines
            .iter()
            .flat_map(|line| match &line.kind {
                LineKind::Bars(items) => items.iter(),
                _ => [].iter(),
            })
            .filter_map(|item| match item {
                Item::Bar(bar) => Some(bar),
                _ => None,
            })
    }

    /// Section titles in source order (`:` and `=` lines).
    pub fn section_titles(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().filter_map(|line| match &line.kind {
            LineKind::SectionTitle { text, .. } => Some(text.as_str()),
            _ => None,
        })
    }
}

/// One source line, with the span it occupies.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub kind: LineKind,
    pub span: Span,
    /// Floating labels (`<- 5`, `>= A`) written on the lines *above* this one.
    /// chordsheet.com renders them in the left or right margin of the line
    /// that follows, so the parser attaches them here rather than leaving
    /// them as standalone lines.
    pub floating: Vec<FloatingLabel>,
}

/// A short label pinned to the margin of the following line.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingLabel {
    pub text: String,
    pub align: FloatAlign,
    /// `true` for the `<=` / `>=` forms, which render boxed like a `:` title.
    pub boxed: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatAlign {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineKind {
    /// `# ...` — dropped when rendering.
    Comment(String),
    /// `- ...` — a text annotation. Up to three columns, split on a double
    /// space. An empty `-` is a spacer line (all columns empty).
    Text { columns: Vec<String> },
    /// `; ...` — a text annotation with whitespace preserved. Never split
    /// into columns.
    Literal(String),
    /// `:` (boxed) or `=` (boxed *and* a musical delimiter — the previous bar
    /// closes with a double line and the next opens with one).
    SectionTitle { text: String, delimiter: bool },
    /// `+` — a page break. `+ Chorus` is shorthand for a page break followed
    /// by `= Chorus`, so `title` carries the section name when present.
    PageBreak { title: Option<String> },
    /// `TAB` / `TAB1` … `TAB7` — a row of blank TAB lines to write on.
    TabRow { lines: u8 },
    /// `RULER` / `SOLID` / `DASHED5` / `DOTTED3` — a divider.
    Ruler { style: RulerStyle, width: u8 },
    /// `// C7 x32310` — chord-diagram definitions. The leading slash count
    /// sets scope and visibility (2 = visible chart-wide, 3 = hidden
    /// chart-wide, 4 = visible page, 5 = hidden page).
    DiagramDefs { slashes: u8, defs: Vec<DiagramDef> },
    /// A line of bars and the markers between them.
    Bars(Vec<Item>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerStyle {
    Solid,
    Dashed,
    Dotted,
}

/// `// C7 x32310` or `// C7 x32310032410` — frets, then optional fingering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramDef {
    pub chord: String,
    pub frets: String,
    pub fingering: Option<String>,
}

/// One item on a bar line: a bar, or a marker that sits between bars.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Bar(Bar),
    /// `(` or `[` or `|:` — a repeat opens here.
    RepeatOpen {
        span: Span,
    },
    /// `)` / `]` / `:|`, optionally with a count (`)x3`, `)3x`).
    RepeatClose {
        times: Option<u32>,
        span: Span,
    },
    /// `1.`, `2.`, `1.-3.+5.` — a volta bracket starts at the next bar.
    Ending {
        numbers: Vec<u8>,
        span: Span,
    },
    /// `||` — an explicit double barline.
    DoubleBar {
        span: Span,
    },
    /// `X` — an indent spacer one bar wide. Layout only.
    Indent {
        span: Span,
    },
    /// `3:4` — a time-signature change taking effect at the next bar.
    TimeSignature {
        numerator: u8,
        denominator: u8,
        span: Span,
    },
    /// `p1`-`p9` (next chord only) or `P0`-`P9` (until the next command),
    /// optionally redefining the slot with `P1 #f53497` / `P1 yellowgreen`.
    Palette {
        slot: u8,
        sticky: bool,
        define: Option<String>,
        span: Span,
    },
    /// `fine`, `$`, `o`, `O`, `D.C. al coda`, `D.S. al coda con rep.`, …
    Navigation {
        marker: NavMarker,
        span: Span,
    },
}

/// Navigational markers (chordsheet.com's "DaCapo" set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavMarker {
    Fine,
    Segno,
    /// Lowercase `o` — coda *from* here.
    CodaFrom,
    /// Uppercase `O` — coda *to* here.
    CodaTo,
    /// `D.C.` with whatever qualifier followed (`al fine`, `al coda con rep.`).
    DaCapo {
        qualifier: Option<String>,
    },
    /// `D.S.` and its qualifiers.
    DalSegno {
        qualifier: Option<String>,
    },
}

impl fmt::Display for NavMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fine => f.write_str("Fine"),
            Self::Segno => f.write_str("Segno"),
            Self::CodaFrom => f.write_str("Coda"),
            Self::CodaTo => f.write_str("Coda"),
            Self::DaCapo { qualifier } => match qualifier {
                Some(q) => write!(f, "D.C. {q}"),
                None => f.write_str("D.C."),
            },
            Self::DalSegno { qualifier } => match qualifier {
                Some(q) => write!(f, "D.S. {q}"),
                None => f.write_str("D.S."),
            },
        }
    }
}

/// One bar. Whitespace separates bars; `_` glues cells inside one.
///
/// A bar with a single cell is the common case — "one chord makes a bar". A
/// bar with several cells is a split bar (`Dm7_G7`), and the cells divide the
/// bar evenly, which is what chordsheet.com draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub cells: Vec<Cell>,
    /// `%` (repeat one bar) or `%%` (repeat two), as written.
    pub simile: Option<u8>,
    /// `%1`, `%2`, `%4` — reprint this many preceding bars *in place of* this
    /// one. Unlike `simile`, the chords are redrawn rather than marked.
    pub reprint: Option<u32>,
    /// `.` before the line, or `.` after a chord — show the chord diagram.
    pub show_diagram: bool,
    pub span: Span,
}

impl Bar {
    /// A bar that only carries a simile or reprint marker holds no chords.
    pub fn is_marker(&self) -> bool {
        self.cells.is_empty() && (self.simile.is_some() || self.reprint.is_some())
    }
}

/// One chord slot inside a bar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    /// The chord symbol as written (`Dm7`, `D/F#`, `N.C.`). `None` for `*`
    /// (an intentionally blank bar) and for a cell that is only strokes.
    pub symbol: Option<String>,
    /// `r1` / `r2` / `r4` — a rest instead of a chord.
    pub rest: Option<Rest>,
    /// `"…"` — free text printed next to the chord.
    pub annotation: Option<String>,
    /// A `,` closed this cell — chordsheet.com draws a stroke symbol on it.
    /// The comma is a slot marker, not a duration: a bar divides evenly among
    /// its cells however many there are, which is exactly what the site
    /// renders.
    pub stroke: bool,
    /// `<>` or the word `diamond`.
    pub diamond: bool,
    /// The word `fermata`.
    pub fermata: bool,
    /// `^` — tie into the next chord.
    pub tie: bool,
    /// `<` (eighth) or `<<` (sixteenth) written *before* the chord.
    pub push: Option<Push>,
    /// `?` — print the chord in parentheses (an optional chord).
    pub optional: bool,
    /// `.` — show this chord's diagram.
    pub show_diagram: bool,
    /// `'` … `'''` — which voicing variant of the chord to draw.
    pub voicing: Option<u8>,
    pub span: Span,
}

impl Cell {
    /// A cell that contributes nothing but a blank slot (`*`).
    pub fn is_blank(&self) -> bool {
        self.symbol.is_none() && self.rest.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    /// `r1` — a full bar.
    Whole,
    /// `r2` — half a bar.
    Half,
    /// `r4` — a quarter note.
    Quarter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    Eighth,
    Sixteenth,
}
