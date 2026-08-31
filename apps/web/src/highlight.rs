//! Keyflow source, syntax-highlighted for the page.
//!
//! The same highlighter the editor and the LSP use
//! (`keyflow::text::highlighting`), rendered to classed spans rather than
//! inline styles so the colours come from the site's stylesheet and
//! follow its theme. `Renderer::to_html_inline` would bake one theme's
//! hex values into the markup and the source pane would stay dark in
//! light mode.
//!
//! # Why line by line
//!
//! `Highlighter::highlight_line` returns spans measured from the start of
//! the *line*, while `Renderer::to_html_classes` indexes into whatever
//! source it is given. Feeding it the whole document with line-relative
//! spans silently mis-slices every line after the first — the offsets are
//! valid, just wrong, so it produces plausible HTML with the colours
//! creeping leftward as the document goes on. Rendering one line at a
//! time means there is no offset arithmetic to get wrong.

use dioxus::prelude::*;
use keyflow::text::highlighting::{Highlighter, Renderer};

/// Render `source` as highlighted HTML.
///
/// Newlines are preserved literally: the output goes inside a `<pre>`,
/// which is what keeps a chart's columns lined up.
pub fn to_html(source: &str) -> String {
    source
        .split('\n')
        .map(|line| Renderer::to_html_classes(line, &Highlighter::highlight_line(line)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A `<pre>` of Keyflow source, highlighted.
#[component]
pub fn HighlightedSource(
    /// The chart source to show.
    source: String,
    /// Extra classes for the `<pre>`.
    #[props(default = String::new())]
    class: String,
) -> Element {
    let html = use_memo(use_reactive!(|(source)| to_html(&source)));

    rsx! {
        // Our own renderer over our own highlighter, and it escapes the
        // text it did not produce — see `Renderer::escape_html`.
        pre {
            class: "kf-source {class}",
            dangerous_inner_html: "{html}",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chart_comes_back_highlighted() {
        let html = to_html("Midnight Dreams\n72bpm 4/4 #A\n\nVS\nA D F#m E\n");
        for class in ["kf-tempo", "kf-section", "kf-root", "kf-quality"] {
            assert!(
                html.contains(class),
                "nothing was marked `{class}`:\n{html}"
            );
        }
    }

    #[test]
    fn a_bare_title_line_is_left_alone() {
        // Not a bug to fix here, and worth pinning so nobody "fixes" it
        // in the site: `highlight_line` is per-line and stateless, so a
        // title with no ` - ` in it is indistinguishable from a line of
        // chords. `Highlighter::highlight_title_line` handles both forms
        // but only runs once something upstream has decided the line IS a
        // title, which needs document context this call does not have.
        //
        // Deciding it here would put "what counts as a title" in the
        // website, which is the language's business, not the page's.
        assert!(!to_html("Midnight Dreams\n").contains("kf-title"));
        assert!(to_html("Vienna - Billy Joel\n").contains("kf-title"));
    }

    #[test]
    fn the_text_survives_intact() {
        // Highlighting is presentation. If a span boundary lands mid-token
        // the chart still has to READ correctly — a dropped or duplicated
        // character in the hero would be the page misquoting its own
        // format. Strip the tags and the source must come back exactly.
        for source in crate::typewriter::DEMO_CHARTS {
            let stripped = strip_tags(&to_html(source));
            assert_eq!(
                stripped,
                *source,
                "highlighting changed the text of `{}`",
                source.lines().next().unwrap_or_default(),
            );
        }
    }

    #[test]
    fn a_partly_typed_chart_does_not_panic() {
        // The hero feeds this every prefix of every demo chart, one
        // character at a time, so "half a chord symbol" is the normal
        // case rather than the edge case.
        for source in crate::typewriter::DEMO_CHARTS {
            let chars: Vec<char> = source.chars().collect();
            for len in 0..=chars.len() {
                let prefix: String = chars[..len].iter().collect();
                let _ = to_html(&prefix);
            }
        }
    }

    /// Undo `to_html`: drop the tags, unescape the entities.
    fn strip_tags(html: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }
}
