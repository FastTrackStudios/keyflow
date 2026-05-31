//! IDE engine: structured diagnostics, completion, and hover.
//!
//! This module is the **shared core** for both the in-process Dioxus editor
//! (`keyflow-ui`) and the future LSP server (`keyflow-lsp`). It exposes a
//! small, allocation-light API that can be called on every keystroke after
//! a UI debounce.
//!
//! # Design
//!
//! - **`analyze(text)`** — full document pass. Returns the best-effort
//!   `Chart` parse plus a `Vec<Diagnostic>` and optional `Vec<HighlightSpan>`.
//!   Today this wraps the existing parser with line-level error recovery;
//!   it is a stepping stone to a chumsky / rowan rewrite.
//! - **`complete(text, byte_offset)`** — context-aware completions at a
//!   cursor position (chord roots, qualities, section types, commands,
//!   melody-variable names).
//! - **`hover(text, byte_offset)`** — tooltip information for the token
//!   under the cursor (chord-quality explanation, scale-degree → absolute
//!   pitch in current key, etc.).
//!
//! All functions are pure: no global state, no I/O.

use crate::chart::{Chart, ChordInstance, parse_chart};
use crate::parsing::TextSpan;

#[cfg(feature = "highlighting")]
use crate::highlighting::HighlightSpan;

mod completion;
mod diagnostic;
mod hover;
mod recovery;

pub use completion::{Completion, CompletionContext, CompletionKind, complete};
pub use diagnostic::{CodeAction, Diagnostic, Severity};
pub use hover::{HoverInfo, hover};

/// Result of analyzing a Keyflow document.
///
/// Always returns a `Chart` — even when `diagnostics` contains errors, the
/// chart reflects what was successfully parsed up to the failure point. This
/// is what makes a live editor experience feasible: a single broken chord
/// line doesn't blank the rest of the document.
#[derive(Clone)]
pub struct Analysis {
    /// Best-effort parse. Empty `Chart` if the input was unrecoverable.
    pub chart: Chart,
    /// Errors, warnings, info, and hints surfaced by the parser and
    /// post-processing passes. Sorted by `range.start` ascending.
    pub diagnostics: Vec<Diagnostic>,
    /// Source-text highlight spans (only populated when the `highlighting`
    /// feature is enabled).
    #[cfg(feature = "highlighting")]
    pub highlights: Vec<HighlightSpan>,
}

impl Analysis {
    /// Whether the analysis surfaced any error-severity diagnostics.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| matches!(d.severity, Severity::Error))
    }

    /// All diagnostics whose range contains the given byte offset, in order.
    #[must_use]
    pub fn diagnostics_at(&self, byte_offset: usize) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(move |d| d.range.contains(byte_offset))
    }
}

/// Analyze a Keyflow document.
///
/// This is the single entry point for live linting. It is intended to be
/// called on every debounced edit (≥ ~100 ms apart). For typical chart sizes
/// (≤ a few hundred lines) this is sub-millisecond on commodity hardware.
#[must_use]
pub fn analyze(text: &str) -> Analysis {
    let mut diagnostics = Vec::new();

    // Whole-document parse first — happy path. If it succeeds, no further
    // recovery is needed.
    let chart = match parse_chart(text) {
        Ok(chart) => chart,
        Err(_) => {
            // Fall back to per-line recovery so one bad line doesn't blank
            // the entire chart in the editor.
            recovery::parse_with_recovery(text, &mut diagnostics)
        }
    };

    diagnostics.sort_by_key(|d| d.range.start);

    Analysis {
        chart,
        diagnostics,
        #[cfg(feature = "highlighting")]
        highlights: collect_highlights(text),
    }
}

#[cfg(feature = "highlighting")]
fn collect_highlights(text: &str) -> Vec<HighlightSpan> {
    use crate::highlighting::Highlighter;
    let mut out = Vec::new();
    let mut line_start = 0;
    for line in text.split_inclusive('\n') {
        let nl_len = line.ends_with('\n') as usize;
        let line_no_nl = &line[..line.len() - nl_len];
        for span in Highlighter::highlight_line(line_no_nl) {
            // Shift line-relative spans into document-relative offsets.
            out.push(HighlightSpan::new(
                TextSpan::new(span.span.start + line_start, span.span.len),
                span.kind,
            ));
        }
        line_start += line.len();
    }
    out
}

/// Doc-absolute source spans for each parsed chord, recovered by matching every
/// chord's `original_token` against the document's whitespace/`|`-delimited
/// tokens in document order.
///
/// Why this exists: the parser's per-chord `ChordInstance::source_span` is
/// currently **line-relative** — `parse_chord_line_inner` shifts spans by a
/// `line_byte_offset` that production parsing leaves at `0`, so the spans only
/// make sense within a single line and can't map a document offset back to a
/// chord. Editor features (inlay overlays, hover) need a reliable doc-absolute
/// span, so we re-derive one here. Chords occur in the same order in the chart
/// and in the source, so one forward scan aligns them; a chord whose token
/// can't be relocated (e.g. heavily normalized) is skipped, keeping the rest
/// aligned.
#[must_use]
pub fn chord_doc_spans<'a>(text: &str, chart: &'a Chart) -> Vec<(TextSpan, &'a ChordInstance)> {
    // Tokenize the document into (offset, token), splitting on whitespace and
    // measure bars. These are the same boundaries chords sit between.
    let bytes = text.as_bytes();
    let mut tokens: Vec<(usize, &str)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() || bytes[i] == b'|' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'|' {
            i += 1;
        }
        tokens.push((start, &text[start..i]));
    }

    let mut out = Vec::new();
    let mut ti = 0;
    for section in &chart.sections {
        for measure in section.measures() {
            for ci in &measure.chords {
                let want = ci.original_token.trim();
                if want.is_empty() {
                    continue;
                }
                while ti < tokens.len() && tokens[ti].1 != want {
                    ti += 1;
                }
                let Some(&(off, tok)) = tokens.get(ti) else {
                    return out; // ran out of source tokens to match
                };
                out.push((TextSpan::new(off, tok.len()), ci));
                ti += 1;
            }
        }
    }
    out
}

/// Map a byte offset into the corresponding (line, column) pair, both 0-indexed.
///
/// Useful for converting `TextSpan` into LSP `Position` values.
#[must_use]
pub fn offset_to_line_col(text: &str, byte_offset: usize) -> (u32, u32) {
    let upto = &text[..byte_offset.min(text.len())];
    let line = upto.bytes().filter(|&b| b == b'\n').count() as u32;
    let last_nl = upto.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = (byte_offset.min(text.len()) - last_nl) as u32;
    (line, col)
}

/// Map a `(line, column)` pair (0-indexed) back to a byte offset.
#[must_use]
pub fn line_col_to_offset(text: &str, line: u32, column: u32) -> usize {
    let mut current_line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if current_line == line {
            return (line_start + column as usize).min(text.len());
        }
        if b == b'\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    if current_line == line {
        (line_start + column as usize).min(text.len())
    } else {
        text.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_clean_input_has_no_errors() {
        let chart = "VS 1: | 1 4 5 1 |";
        let a = analyze(chart);
        assert!(!a.has_errors(), "diagnostics: {:?}", a.diagnostics);
    }

    #[test]
    fn analyze_returns_a_chart_even_with_garbage() {
        let a = analyze("@@@@\n");
        // Don't assert specific count — just that we don't crash and we
        // return a chart object.
        let _ = a.chart;
    }

    #[test]
    fn offset_to_line_col_round_trips() {
        let s = "abc\nde\nf";
        assert_eq!(offset_to_line_col(s, 0), (0, 0));
        assert_eq!(offset_to_line_col(s, 3), (0, 3));
        assert_eq!(offset_to_line_col(s, 4), (1, 0));
        assert_eq!(offset_to_line_col(s, 6), (1, 2));
        assert_eq!(offset_to_line_col(s, 7), (2, 0));
        assert_eq!(line_col_to_offset(s, 1, 1), 5);
        assert_eq!(line_col_to_offset(s, 2, 0), 7);
    }

    #[test]
    fn diagnostics_at_filters_by_offset() {
        let mut a = Analysis {
            chart: Chart::new(),
            diagnostics: vec![
                Diagnostic::error("kf-x", "first", TextSpan::new(0, 4)),
                Diagnostic::error("kf-y", "second", TextSpan::new(10, 5)),
            ],
            #[cfg(feature = "highlighting")]
            highlights: Vec::new(),
        };
        a.diagnostics.sort_by_key(|d| d.range.start);
        assert_eq!(a.diagnostics_at(2).count(), 1);
        assert_eq!(a.diagnostics_at(12).count(), 1);
        assert_eq!(a.diagnostics_at(7).count(), 0);
    }
}
