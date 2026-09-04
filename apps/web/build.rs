//! Compile `docs/guides/keyflow/*.md` into the site — charts and all.
//!
//! The guide is a **vault**: notes with frontmatter, `[[wikilink]]`
//! cross-references, and ```kf fences carrying real charts. `ssg-build`
//! turns the markdown into HTML; the fence renderer below turns the
//! charts into **inline SVG** and their source into highlighted markup,
//! here, on the host.
//!
//! That is what this file is for. The site used to hand each note to the
//! editor in read-only mode, which is how it got wikilinks and engraved
//! fences — at the cost of putting the editor, its state machine, its
//! decoration pipeline and a WebGL2 chart surface in front of anyone
//! trying to read a paragraph. None of it could change after the build,
//! so it happens *at* the build: the engraver exports SVG with no GPU
//! (see `kf docs`, and `features/keyflow/examples/svg_smoke.rs`), and
//! the highlighter is the same one the editor runs.
//!
//! Two fence spellings, matching what the guide is written in:
//!
//! - ```` ```kf+ ```` — source *and* chart. What the guide uses almost
//!   everywhere: it is teaching the notation, so the text that produced
//!   the picture is part of the lesson.
//! - ```` ```kf ```` — the chart, with its source behind a `<details>`.
//!   A disclosure triangle rather than a scripted toggle, because the
//!   page should not need JavaScript to open it.
//!
//! A fence that fails to parse renders as an ordinary code block: a typo
//! in one chart shows that chart's source rather than failing a build or
//! blanking a page.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a build script reports failure by panicking; there is no other channel"
)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use keyflow::engraver::api::pipeline::ChartPipeline;
use keyflow::engraver::layout::chart::{ChartLayoutConfig, LayoutMode};
use keyflow::text::highlighting::{HighlightSpan, Highlighter, Renderer};

/// Layout width in points. The guide's prose column, near enough — a
/// chart engraved much wider than the text it sits in reads as a
/// different document.
const CHART_WIDTH: f64 = 720.0;

/// Padding around the shrink-wrapped content box, in points.
const PAD: f64 = 6.0;

fn main() {
    let engraver = Engraver::new();

    ssg_build::Vault::at("../../docs/guides/keyflow")
        .link_base("/guide")
        .fence(|info, body| engraver.fence(info, body))
        .emit();

    // After `emit`, because it is only now known which typefaces the
    // guide's charts actually reference.
    //
    // Into `assets/`, not `OUT_DIR`, because this has to be a *linked*
    // stylesheet: it is megabytes of embedded typeface, and inlining it
    // into every page would send the same megabytes nine times over.
    // Linked, `asset!` gives it a content-hashed URL and the browser
    // fetches it once for the whole guide. The file is generated and
    // gitignored; `asset!` resolves it because a build script always
    // runs before the crate that reads it compiles.
    let generated = std::path::Path::new("assets/chart-fonts.css");
    let css = engraver.font_css();
    // Only when it actually changed: `asset!` hashes the file, and
    // rewriting identical bytes still bumps the mtime, which makes dx
    // re-copy the asset on every build.
    if std::fs::read_to_string(generated).is_ok_and(|old| old == css) {
        return;
    }
    std::fs::write(generated, css).expect("cannot write the chart font stylesheet");
}

/// The engraver, set up once for the whole build.
///
/// `ChartPipeline::shared()` rather than a hand-wired bundle and engine:
/// the repo has one pipeline on purpose. Three places used to assemble
/// their own and drifted — one of them declared `MuseJazzText` where the
/// scene emits `MuseJazz Text`, so every chart it exported fell back to
/// a system sans and the `maj7` triangles came out blank. A
/// `font-family` that nothing declares does not error; it substitutes.
struct Engraver {
    pipeline: &'static ChartPipeline,
    /// Every `font-family` the rendered charts referenced.
    used_families: RefCell<BTreeSet<String>>,
}

impl Engraver {
    fn new() -> Self {
        Self {
            pipeline: ChartPipeline::shared().expect("the engraving fonts are compiled in"),
            used_families: RefCell::new(BTreeSet::new()),
        }
    }

    /// `@font-face` rules for the typefaces the guide actually uses.
    ///
    /// The charts are exported *linked* — naming their typefaces rather
    /// than embedding them, which the repo rule requires: the embedding
    /// variants cost a few hundred kilobytes of font data per chart, and
    /// this guide has sixty-one of them. So the document declares the
    /// faces once and every chart on it resolves against one download.
    ///
    /// Narrower than `ChartPipeline::font_face_css`, which emits the
    /// bundle's whole list: fourteen entries, several of them aliases
    /// for the same bytes and one a 1.6 MB fallback that exists for
    /// *rasterisers*, which have to be told what `sans-serif` means. A
    /// browser already knows. Filtering to the families that actually
    /// appeared took 4.3 MB of base64 down to 3.3 MB.
    ///
    /// Still too much. Subsetting each face to the glyphs actually drawn
    /// is the real fix — dodeca does exactly this — and is the obvious
    /// next move.
    fn font_css(&self) -> String {
        let used = self.used_families.borrow();
        self.pipeline
            .fonts()
            .embeddable_fonts()
            .into_iter()
            .filter(|(family, _)| used.contains(*family))
            .fold(
                keyflow::engraver::export::svg::SvgExportConfig::default(),
                |config, (family, bytes)| config.with_embedded_font(family, bytes.as_ref().clone()),
            )
            .font_face_css()
    }

    /// Render one fence, or decline it.
    fn fence(&self, info: &str, body: &str) -> Option<String> {
        // The info string may carry more than the language; only the
        // first word selects the renderer.
        let lang = info.split_whitespace().next().unwrap_or(info);
        let fold_source = match lang {
            "kf+" => false,
            "kf" => true,
            _ => return None,
        };

        let svg = self.svg(body)?;
        let source = highlight(body.trim_end());

        let mut out = String::from("<figure class=\"kf-chart\">");
        if fold_source {
            // `<details>` rather than a button: it opens with no script,
            // which is the point of a page finished at build time.
            let _ = write!(
                out,
                "<details class=\"kf-chart-source\"><summary>Source</summary>\
                 <pre class=\"kf-source\"><code>{source}</code></pre></details>"
            );
        } else {
            let _ = write!(
                out,
                "<pre class=\"kf-source kf-chart-source\"><code>{source}</code></pre>"
            );
        }
        out.push_str(&svg);
        out.push_str("</figure>");
        Some(out)
    }

    /// Chart text to font-less, content-cropped SVG.
    ///
    /// `ContinuousScroll` and a viewBox shrink-wrapped to what was
    /// actually drawn: a two-bar example laid out on a page would sit in
    /// a tall white rectangle, most of it empty. `None` when the text
    /// does not parse or draws nothing — the caller then leaves the
    /// fence as source.
    fn svg(&self, source: &str) -> Option<String> {
        let chart = keyflow::parse(source).ok()?;
        let result = self.pipeline.layout_with_config(
            &chart,
            &LayoutMode::ContinuousScroll { width: CHART_WIDTH },
            &ChartLayoutConfig::master_rhythm().with_page_offsets(true),
        );
        // Nothing drawn — an empty fence. Decline it, and the caller
        // leaves the source as a code block.
        result.content_bounds()?;

        // The *linked* export: fonts named, not embedded. The document
        // declares them once — see `font_css`. Its white background is
        // right, and deliberate: the site's own comment is that a chart
        // is white paper on a dark ground, and it has a light theme too.
        let svg = self.pipeline.export_svg_snippet(&result, PAD);
        self.record_families(&svg);
        Some(svg)
    }

    /// Note every `font-family="…"` this chart named.
    fn record_families(&self, svg: &str) {
        let mut used = self.used_families.borrow_mut();
        let mut rest = svg;
        while let Some(at) = rest.find("font-family=\"") {
            rest = &rest[at + "font-family=\"".len()..];
            let Some(end) = rest.find('"') else { break };
            used.insert(rest[..end].to_owned());
            rest = &rest[end..];
        }
    }
}

/// Chart source to highlighted HTML, with CSS classes and no inline
/// colours.
///
/// Classes rather than baked-in styles because the site already has a
/// palette for them (`.kf-source .kf-root` and friends, from
/// `HighlightKind::css_class`) and it follows the light/dark theme.
/// Inline colours would pin one theme into the markup.
///
/// The highlighter is line-oriented, so each line's spans are shifted to
/// absolute offsets — the same pass `editor-keyflow` makes.
fn highlight(source: &str) -> String {
    let mut spans: Vec<HighlightSpan> = Vec::new();
    let mut line_start = 0usize;
    for line in source.split_inclusive('\n') {
        let content = line
            .strip_suffix('\n')
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or_else(|| line.strip_suffix('\n').unwrap_or(line));
        for span in Highlighter::highlight_line(content) {
            spans.push(HighlightSpan::from_range(
                line_start + span.span.start,
                span.span.len,
                span.kind,
            ));
        }
        line_start += line.len();
    }
    Renderer::to_html_classes(source, &spans)
}
