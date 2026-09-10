//! The chordsheet.com source parser.
//!
//! Two passes worth of work in one: the first character of a line decides its
//! type (comment, text, literal text, boxed title, page break, ruler, TAB row,
//! diagram definitions, or bars), and a bar line is then scanned character by
//! character rather than split on whitespace.
//!
//! Scanning by character matters because chordsheet.com ignores whitespace
//! entirely — `Bm _ D7/A` and `Bm_D7/A` are the same split bar — and because
//! `(` is a repeat bracket at the start of a token but part of the chord in
//! `Eb(b5)`. Splitting on spaces gets both wrong.

use crate::ast::*;

/// Parse a chordsheet.com source document.
///
/// Never fails: the format has no illegal input, only input that renders
/// oddly. Anything the scanner can't classify ends up as a chord symbol,
/// which is what the site itself does.
#[must_use]
pub fn parse(source: &str) -> Document {
    let mut lines = Vec::new();
    let mut pending_floating: Vec<FloatingLabel> = Vec::new();
    let mut offset = 0usize;

    for raw in source.split_inclusive('\n') {
        let line_start = offset;
        offset += raw.len();
        let text = raw.trim_end_matches(['\n', '\r']);

        if let Some(label) = parse_floating_label(text, line_start) {
            pending_floating.push(label);
            continue;
        }

        let Some(kind) = parse_line(text, line_start) else {
            // A blank line. It carries no meaning of its own (a `-` line is
            // the spacer), but it must not swallow a floating label written
            // above it.
            continue;
        };

        lines.push(Line {
            kind,
            span: Span::new(line_start, text.len()),
            floating: std::mem::take(&mut pending_floating),
        });
    }

    Document { lines }
}

/// `<- 5`, `<= A`, `>- guitar joins`, `>= second time arpeggiate`.
fn parse_floating_label(text: &str, line_start: usize) -> Option<FloatingLabel> {
    let trimmed = text.trim_start();
    let indent = text.len() - trimmed.len();
    let mut chars = trimmed.chars();
    let align = match chars.next()? {
        '<' => FloatAlign::Left,
        '>' => FloatAlign::Right,
        _ => return None,
    };
    let boxed = match chars.next()? {
        '=' => true,
        '-' => false,
        _ => return None,
    };
    Some(FloatingLabel {
        text: trimmed[2..].trim().to_string(),
        align,
        boxed,
        span: Span::new(line_start + indent, trimmed.len()),
    })
}

fn parse_line(text: &str, line_start: usize) -> Option<LineKind> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let indent = text.len() - trimmed.len();
    let rest = &trimmed[1..];

    let kind = match trimmed.as_bytes()[0] {
        b'#' => LineKind::Comment(rest.trim().to_string()),
        b';' => LineKind::Literal(rest.to_string()),
        b':' => LineKind::SectionTitle {
            text: rest.trim().to_string(),
            delimiter: false,
        },
        b'=' => LineKind::SectionTitle {
            text: rest.trim().to_string(),
            delimiter: true,
        },
        b'+' => {
            let title = rest.trim();
            LineKind::PageBreak {
                title: (!title.is_empty()).then(|| title.to_string()),
            }
        }
        b'-' => LineKind::Text {
            columns: split_columns(rest),
        },
        b'/' => {
            let slashes = trimmed.bytes().take_while(|b| *b == b'/').count() as u8;
            LineKind::DiagramDefs {
                slashes,
                defs: parse_diagram_defs(&trimmed[slashes as usize..]),
            }
        }
        _ => {
            if let Some(kind) = parse_layout_keyword(trimmed) {
                kind
            } else {
                LineKind::Bars(scan_bars(trimmed, line_start + indent))
            }
        }
    };
    Some(kind)
}

/// `TAB`, `TAB1`…`TAB7`, `RULER`, `SOLID`, `DASHED5`, `DOTTED3`.
///
/// These are whole-line keywords. Case-sensitive uppercase, as the site
/// documents them — lowercasing would swallow the note name `b` and friends.
fn parse_layout_keyword(trimmed: &str) -> Option<LineKind> {
    let word = trimmed.split_whitespace().next()?;
    if trimmed.split_whitespace().count() != 1 {
        return None;
    }
    let (name, digits) = word.split_at(
        word.len()
            - word
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .count(),
    );
    let n: Option<u8> = digits.parse().ok();
    match name {
        "TAB" => Some(LineKind::TabRow {
            lines: n.filter(|n| (1..=7).contains(n)).unwrap_or(6),
        }),
        "RULER" | "SOLID" => Some(LineKind::Ruler {
            style: RulerStyle::Solid,
            width: n.unwrap_or(1),
        }),
        "DASHED" => Some(LineKind::Ruler {
            style: RulerStyle::Dashed,
            width: n.unwrap_or(1),
        }),
        "DOTTED" => Some(LineKind::Ruler {
            style: RulerStyle::Dotted,
            width: n.unwrap_or(1),
        }),
        _ => None,
    }
}

/// A double space pushes text to the next of up to three columns.
fn split_columns(rest: &str) -> Vec<String> {
    let mut columns: Vec<String> = rest
        .split("  ")
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .take(3)
        .collect();
    if columns.is_empty() {
        columns.push(String::new());
    }
    columns
}

fn parse_diagram_defs(rest: &str) -> Vec<DiagramDef> {
    let mut words = rest.split_whitespace();
    let mut out = Vec::new();
    while let (Some(chord), Some(shape)) = (words.next(), words.next()) {
        let (frets, fingering) = if shape.len() > 6 {
            (shape[..6].to_string(), Some(shape[6..].to_string()))
        } else {
            (shape.to_string(), None)
        };
        out.push(DiagramDef {
            chord: chord.to_string(),
            frets,
            fingering,
        });
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Bar-line scanning
// ─────────────────────────────────────────────────────────────────────────────

struct Scanner<'a> {
    src: &'a [u8],
    text: &'a str,
    pos: usize,
    /// Byte offset of `text` within the whole document, for spans.
    base: usize,
}

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }
    fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos + n).copied()
    }
    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }
    fn span_from(&self, start: usize) -> Span {
        Span::new(self.base + start, self.pos - start)
    }
    /// The next whitespace-delimited word, without consuming it.
    fn peek_word(&self) -> &'a str {
        let mut i = self.pos;
        while i < self.src.len() && (self.src[i] == b' ' || self.src[i] == b'\t') {
            i += 1;
        }
        let start = i;
        while i < self.src.len() && !matches!(self.src[i], b' ' | b'\t') {
            i += 1;
        }
        &self.text[start..i]
    }
    fn skip_word(&mut self) {
        self.skip_ws();
        while !matches!(self.peek(), None | Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }
}

fn scan_bars(text: &str, base: usize) -> Vec<Item> {
    let mut s = Scanner {
        src: text.as_bytes(),
        text,
        pos: 0,
        base,
    };
    let mut items: Vec<Item> = Vec::new();
    let mut line_diagrams = false;

    loop {
        s.skip_ws();
        let start = s.pos;
        let Some(b) = s.peek() else { break };

        match b {
            b'(' | b'[' => {
                s.pos += 1;
                items.push(Item::RepeatOpen {
                    span: s.span_from(start),
                });
            }
            b')' | b']' => {
                s.pos += 1;
                let times = scan_repeat_count(&mut s);
                items.push(Item::RepeatClose {
                    times,
                    span: s.span_from(start),
                });
            }
            b'|' => {
                s.pos += 1;
                if s.eat(b':') {
                    items.push(Item::RepeatOpen {
                        span: s.span_from(start),
                    });
                } else {
                    s.eat(b'|');
                    items.push(Item::DoubleBar {
                        span: s.span_from(start),
                    });
                }
            }
            b':' if s.peek_at(1) == Some(b'|') => {
                s.pos += 2;
                let times = scan_repeat_count(&mut s);
                items.push(Item::RepeatClose {
                    times,
                    span: s.span_from(start),
                });
            }
            b'$' => {
                s.pos += 1;
                items.push(Item::Navigation {
                    marker: NavMarker::Segno,
                    span: s.span_from(start),
                });
            }
            b'X' if is_word_boundary(&s, 1) => {
                s.pos += 1;
                items.push(Item::Indent {
                    span: s.span_from(start),
                });
            }
            b'.' if is_word_boundary(&s, 1) => {
                s.pos += 1;
                line_diagrams = true;
            }
            b'%' => items.push(Item::Bar(scan_repeat_bar(&mut s))),
            b'P' | b'p'
                if s.peek_at(1).is_some_and(|c| c.is_ascii_digit()) && is_word_boundary(&s, 2) =>
            {
                let sticky = b == b'P';
                let slot = s.peek_at(1).unwrap() - b'0';
                s.pos += 2;
                let define = scan_palette_definition(&mut s);
                items.push(Item::Palette {
                    slot,
                    sticky,
                    define,
                    span: s.span_from(start),
                });
            }
            _ => {
                if let Some(item) = scan_keyword_item(&mut s, start) {
                    items.push(item);
                } else if let Some((numerator, denominator)) = scan_time_signature(&mut s) {
                    items.push(Item::TimeSignature {
                        numerator,
                        denominator,
                        span: s.span_from(start),
                    });
                } else if let Some(numbers) = scan_ending(&mut s) {
                    items.push(Item::Ending {
                        numbers,
                        span: s.span_from(start),
                    });
                } else {
                    let before = s.pos;
                    let bar = scan_bar(&mut s);
                    if s.pos == before {
                        // Nothing consumed — an unrecognized byte. Skip it
                        // rather than spin.
                        s.pos += 1;
                        continue;
                    }
                    items.push(Item::Bar(bar));
                }
            }
        }
    }

    if line_diagrams {
        for item in &mut items {
            if let Item::Bar(bar) = item {
                bar.show_diagram = true;
            }
        }
    }
    items
}

fn is_word_boundary(s: &Scanner<'_>, at: usize) -> bool {
    matches!(s.peek_at(at), None | Some(b' ' | b'\t'))
}

/// `x3`, `3x`, or `x 3` after a closing bracket.
fn scan_repeat_count(s: &mut Scanner<'_>) -> Option<u32> {
    let save = s.pos;
    s.skip_ws();
    if s.eat(b'x') || s.eat(b'X') {
        s.skip_ws();
        if let Some(n) = scan_number(s) {
            return Some(n);
        }
        s.pos = save;
        return None;
    }
    if let Some(n) = scan_number(s) {
        s.skip_ws();
        if s.eat(b'x') || s.eat(b'X') {
            return Some(n);
        }
    }
    s.pos = save;
    None
}

fn scan_number(s: &mut Scanner<'_>) -> Option<u32> {
    let start = s.pos;
    while s.peek().is_some_and(|b| b.is_ascii_digit()) {
        s.pos += 1;
    }
    if s.pos == start {
        return None;
    }
    s.text[start..s.pos].parse().ok()
}

/// `#f53497` after a `P<n>` — redefines that palette slot.
fn scan_palette_definition(s: &mut Scanner<'_>) -> Option<String> {
    let save = s.pos;
    s.skip_ws();
    if s.peek() != Some(b'#') {
        s.pos = save;
        return None;
    }
    let start = s.pos;
    while !matches!(s.peek(), None | Some(b' ' | b'\t')) {
        s.pos += 1;
    }
    Some(s.text[start..s.pos].to_string())
}

/// `3:4` — a per-bar time signature.
fn scan_time_signature(s: &mut Scanner<'_>) -> Option<(u8, u8)> {
    let save = s.pos;
    let numerator = scan_number(s)?;
    if !s.eat(b':') {
        s.pos = save;
        return None;
    }
    let Some(denominator) = scan_number(s) else {
        s.pos = save;
        return None;
    };
    if !is_word_boundary(s, 0) {
        s.pos = save;
        return None;
    }
    match (u8::try_from(numerator), u8::try_from(denominator)) {
        (Ok(n), Ok(d)) if n > 0 && d > 0 => Some((n, d)),
        _ => {
            s.pos = save;
            None
        }
    }
}

/// `1.`, `2.`, `1.-3.+5.`, `4.+6.+7.` — volta numbers.
fn scan_ending(s: &mut Scanner<'_>) -> Option<Vec<u8>> {
    let save = s.pos;
    let mut numbers: Vec<u8> = Vec::new();
    while let Some(n) = scan_number(s) {
        if !s.eat(b'.') {
            break;
        }
        let Ok(n) = u8::try_from(n) else { break };
        numbers.push(n);
        match s.peek() {
            Some(b'+') => {
                s.pos += 1;
            }
            Some(b'-') => {
                s.pos += 1;
                // A range: `1.-3.` means 1, 2, 3.
                let range_save = s.pos;
                if let Some(end) = scan_number(s) {
                    if s.eat(b'.') {
                        if let Ok(end) = u8::try_from(end) {
                            for v in (n + 1)..=end {
                                numbers.push(v);
                            }
                            match s.peek() {
                                Some(b'+') => {
                                    s.pos += 1;
                                    continue;
                                }
                                _ => break,
                            }
                        }
                    }
                }
                s.pos = range_save;
                break;
            }
            _ => break,
        }
    }
    if numbers.is_empty() || !is_word_boundary(s, 0) {
        s.pos = save;
        return None;
    }
    Some(numbers)
}

/// `fine`, `o`, `O`, `D.C. al coda con rep.`, `D.S. al fine`.
fn scan_keyword_item(s: &mut Scanner<'_>, start: usize) -> Option<Item> {
    let word = s.peek_word();
    let marker = match word {
        "fine" | "Fine" | "FINE" => NavMarker::Fine,
        "o" => NavMarker::CodaFrom,
        "O" => NavMarker::CodaTo,
        "D.C." | "D.C" | "DC" => NavMarker::DaCapo { qualifier: None },
        "D.S." | "D.S" | "DS" => NavMarker::DalSegno { qualifier: None },
        _ => return None,
    };
    s.skip_word();
    let marker = match marker {
        NavMarker::DaCapo { .. } => NavMarker::DaCapo {
            qualifier: scan_nav_qualifier(s),
        },
        NavMarker::DalSegno { .. } => NavMarker::DalSegno {
            qualifier: scan_nav_qualifier(s),
        },
        other => other,
    };
    Some(Item::Navigation {
        marker,
        span: s.span_from(start),
    })
}

/// `al fine`, `al coda`, `al coda con rep.`, `al coda senza rep.`
fn scan_nav_qualifier(s: &mut Scanner<'_>) -> Option<String> {
    const PARTS: [&str; 7] = ["al", "fine", "coda", "con", "senza", "rep.", "rep"];
    let mut words: Vec<String> = Vec::new();
    loop {
        let word = s.peek_word();
        if word.is_empty() || !PARTS.contains(&word) {
            break;
        }
        words.push(word.to_string());
        s.skip_word();
    }
    (!words.is_empty()).then(|| words.join(" "))
}

/// `%`, `%%`, `%1`, `%2`, `%4`.
fn scan_repeat_bar(s: &mut Scanner<'_>) -> Bar {
    let start = s.pos;
    s.pos += 1; // the '%'
    let (simile, reprint) = if let Some(n) = scan_number(s) {
        (None, Some(n))
    } else {
        let mut count = 1u8;
        while s.eat(b'%') {
            count += 1;
        }
        (Some(count), None)
    };
    Bar {
        cells: Vec::new(),
        simile,
        reprint,
        show_diagram: false,
        span: s.span_from(start),
    }
}

/// One bar: every cell up to the next whitespace, plus anything `_` glues on
/// across it.
///
/// Whitespace is the only thing that ends a bar. Inside one, cells run
/// together without a separator, which is the whole point of the format —
/// `*_*_D/F#G_D/F#A` is one bar of six chords and two blanks.
fn scan_bar(s: &mut Scanner<'_>) -> Bar {
    let start = s.pos;
    let mut cells = Vec::new();
    let mut show_diagram = false;

    while let Some(cell) = scan_cell(s) {
        show_diagram |= cell.show_diagram;
        cells.push(cell);

        // `_` glues across whitespace: chordsheet.com ignores spaces, so
        // `Bm _ D7/A` is the same split bar as `Bm_D7/A`.
        let save = s.pos;
        s.skip_ws();
        if s.eat(b'_') {
            s.skip_ws();
            continue;
        }
        s.pos = save;

        // A comma keeps the bar open, and nothing else does.
        //
        // This is the rule that decides how long a run of chords is. `_` glues
        // cells into one bar and a comma marks a slot inside one, but a chord
        // written straight after a chord that did not end in either starts the
        // *next bar*: `C#C#/CF#` is three bars of one chord, not one bar of
        // three, and `A,A,A,A,A,G#,C#C#` is a seven-slot bar followed by a bar
        // of C# — which is exactly what the site's own PDFs draw.
        if cells.last().is_some_and(|c: &Cell| c.stroke) && starts_cell(s.peek()) {
            continue;
        }
        break;
    }

    Bar {
        cells,
        simile: None,
        reprint: None,
        show_diagram,
        span: s.span_from(start),
    }
}

fn starts_cell(b: Option<u8>) -> bool {
    matches!(b, Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'*' | b'<' | b',' | b'"' | b'#'))
}

fn scan_cell(s: &mut Scanner<'_>) -> Option<Cell> {
    let start = s.pos;
    let mut cell = Cell::default();

    // `<` / `<<` before the chord is a push. `<>` after one is a diamond, so
    // only treat `<` as a push when it isn't the opening half of `<>`.
    if s.peek() == Some(b'<') && s.peek_at(1) != Some(b'>') {
        s.pos += 1;
        if s.peek() == Some(b'<') && s.peek_at(1) != Some(b'>') {
            s.pos += 1;
            cell.push = Some(Push::Sixteenth);
        } else {
            cell.push = Some(Push::Eighth);
        }
    }

    if s.eat(b'*') {
        // An intentionally blank bar. No symbol, but modifiers still apply.
    } else if let Some(rest) = scan_rest(s) {
        cell.rest = Some(rest);
    } else {
        let symbol = scan_symbol(s);
        if !symbol.is_empty() {
            cell.symbol = Some(symbol);
        }
    }

    scan_modifiers(s, &mut cell);

    cell.span = s.span_from(start);
    if s.pos == start {
        None
    } else {
        Some(cell)
    }
}

fn scan_rest(s: &mut Scanner<'_>) -> Option<Rest> {
    if s.peek() != Some(b'r') {
        return None;
    }
    let rest = match s.peek_at(1) {
        Some(b'1') => Rest::Whole,
        Some(b'2') => Rest::Half,
        Some(b'4') => Rest::Quarter,
        _ => return None,
    };
    // `r16` is a sixteenth rest nowhere in this language — only r1, r2 and r4
    // exist — but a digit after the value would still be part of the token, so
    // refuse it rather than read `r1` out of `r16`.
    if s.peek_at(2).is_some_and(|b| b.is_ascii_digit()) {
        return None;
    }
    s.pos += 2;
    Some(rest)
}

/// A chord symbol.
///
/// Two things make this more than "read until whitespace":
///
/// - **Parentheses nest.** `Eb(b5)` and `G(#11)` carry their own brackets, so
///   a `)` only ends the symbol when nothing inside it is open — otherwise it
///   is the repeat bracket that closes the phrase.
/// - **A new root ends the chord.** chordsheet.com needs no separator between
///   chords in a bar: `AD/A` is `A` then `D/A`, and `D/F#G` is `D/F#` then
///   `G`. An uppercase A–G that is neither this chord's root nor the note
///   after its slash starts the next chord. Only *uppercase* — lowercase
///   letters are quality and extension (`Dm`, `Cmaj7`, `Gsus4`, `Bdim`), and
///   lowercase roots are legal too (`a(b6)`), so splitting on those would cut
///   every minor chord in half.
fn scan_symbol(s: &mut Scanner<'_>) -> String {
    // `N.C.` is the one symbol with dots in it; everywhere else a trailing
    // dot means "show the chord diagram".
    for literal in ["N.C.", "n.c."] {
        if s.text[s.pos..].starts_with(literal) {
            s.pos += literal.len();
            return "N.C.".to_string();
        }
    }

    let start = s.pos;
    let mut depth = 0u32;
    // The root, and the accidental that may follow it, are never a boundary.
    if s.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
        s.pos += 1;
        if matches!(s.peek(), Some(b'#' | b'b')) {
            s.pos += 1;
        }
    }
    while let Some(b) = s.peek() {
        match b {
            // `(` opens an alteration only when an alteration follows —
            // `Eb(b5)`, `G(#11)`, `D7(9)`. `B(B)x6` is a chord followed by a
            // repeated bar, and reading its bracket as part of the chord
            // swallows the repeat.
            b'(' if depth > 0
                || s.peek_at(1)
                    .is_some_and(|n| matches!(n, b'#' | b'b') || n.is_ascii_digit()) =>
            {
                depth += 1;
                s.pos += 1;
            }
            b')' | b']' if depth > 0 => {
                depth -= 1;
                s.pos += 1;
            }
            b'[' if depth > 0 => {
                depth += 1;
                s.pos += 1;
            }
            b'/' => {
                s.pos += 1;
                // Whatever follows the slash is the bass note, uppercase or
                // not, plus its accidental.
                if s.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
                    s.pos += 1;
                    if matches!(s.peek(), Some(b'#' | b'b')) {
                        s.pos += 1;
                    }
                }
            }
            b'A'..=b'G' if depth == 0 => break,
            // `Dr1D` is a chord, a whole-bar rest and a chord. No chord
            // quality or extension contains an `r`, so this is unambiguous.
            b'r' if depth == 0 && matches!(s.peek_at(1), Some(b'1' | b'2' | b'4')) => break,
            _ if b.is_ascii_alphanumeric() || matches!(b, b'#' | b'+' | b'-') => {
                s.pos += 1;
            }
            _ => break,
        }
    }
    // An unclosed `(` was a repeat bracket after all, not part of the chord.
    // Rewind past it so the caller sees it.
    if depth > 0 {
        if let Some(open) = s.text[start..s.pos].rfind('(') {
            s.pos = start + open;
        }
    }
    s.text[start..s.pos].to_string()
}

fn scan_modifiers(s: &mut Scanner<'_>, cell: &mut Cell) {
    loop {
        match s.peek() {
            // A comma closes the cell: it is the stroke drawn on this slot,
            // and any comma after it belongs to the next slot.
            Some(b',') => {
                s.pos += 1;
                cell.stroke = true;
                break;
            }
            Some(b'<') if s.peek_at(1) == Some(b'>') => {
                s.pos += 2;
                cell.diamond = true;
            }
            Some(b'^') => {
                s.pos += 1;
                cell.tie = true;
            }
            Some(b'?') => {
                s.pos += 1;
                cell.optional = true;
            }
            Some(b'\'') => {
                let mut n = 0u8;
                while s.eat(b'\'') {
                    n = n.saturating_add(1);
                }
                cell.voicing = Some(n);
            }
            Some(b'.') => {
                s.pos += 1;
                cell.show_diagram = true;
            }
            Some(b'"') => {
                s.pos += 1;
                let start = s.pos;
                while !matches!(s.peek(), None | Some(b'"')) {
                    s.pos += 1;
                }
                let text = s.text[start..s.pos].to_string();
                s.eat(b'"');
                cell.annotation = Some(match cell.annotation.take() {
                    Some(prev) => format!("{prev} {text}"),
                    None => text,
                });
            }
            _ => {
                // `diamond` and `fermata` are written as separate words.
                match s.peek_word() {
                    "diamond" => {
                        s.skip_word();
                        cell.diamond = true;
                    }
                    "fermata" => {
                        s.skip_word();
                        cell.fermata = true;
                    }
                    _ => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
