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

use std::sync::Mutex;
use std::collections::{BTreeMap, BTreeSet};
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
    let engraver = std::sync::Arc::new(Engraver::new());

    // The editor renders the guide's body, so a `kf` fence is engraved by
    // whatever its registry holds for "kf". Register the engraver above,
    // which is also the thing that tracks glyph usage for the font
    // subsetting — see the `FenceRenderer` impl.
    editor_state::fence_renderer::register_fence_renderer("kf", engraver.clone());

    // The body is rendered by the EDITOR, not by ssg's markdown pass.
    //
    // These notes are written in the editor, and it knows things ssg's
    // pass does not: twelve callout types, task lists, block references,
    // and every fence language a plugin has been registered for. Two
    // renderers over the same syntax drift — a chapter drafted with
    // `> [!question]` rendered as a plain quote until ssg learned
    // callouts, silently, because a blockquote is a perfectly good
    // blockquote. One renderer cannot drift from itself.
    //
    // ssg still owns everything around the body: it resolves wikilinks
    // before this sees the text (so what arrives is ordinary markdown
    // with real links), parses the headings for the table of contents
    // and the search index, and reports broken links.
    ssg_build::Vault::at("../../docs/guides/keyflow")
        .link_base("/guide")
        .fence(|info, body| engraver.fence(info, body))
        .body_renderer(|markdown| editor_state::html::render_markdown_html(markdown))
        // A broken cross-reference is a printed warning here, not a hard
        // error, because `rendering-test.md` deliberately contains one:
        // "unresolved wikilinks render red" is one of the behaviours that
        // page exists to show, and the check is all-or-nothing per vault.
        // The warning still names every broken target at build time, so a
        // real typo in a chapter is still visible — just not fatal.
        .allow_broken_links()
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
    /// Every `font-family` the rendered charts referenced, and every
    /// character they set in it.
    // `Mutex`, not `RefCell`: this is registered as the editor's fence
    // renderer, and that trait is `Send + Sync`.
    used: Mutex<BTreeMap<String, BTreeSet<char>>>,
}

impl Engraver {
    fn new() -> Self {
        Self {
            pipeline: ChartPipeline::shared().expect("the engraving fonts are compiled in"),
            used: Mutex::new(BTreeMap::new()),
        }
    }

    /// `@font-face` rules for the guide's typefaces, subset to the
    /// glyphs it actually draws.
    ///
    /// The charts are exported *linked* — naming their typefaces rather
    /// than embedding them, which the repo rule requires: the embedding
    /// variants cost a few hundred kilobytes per chart, and this guide
    /// has sixty-one of them. So the document declares the faces once
    /// and every chart on it resolves against one download.
    ///
    /// Then that download is cut to size. Two passes:
    ///
    /// 1. Only the families a chart actually named. The bundle lists
    ///    fourteen, several of them aliases for the same bytes and one a
    ///    1.6 MB fallback that exists to tell a *rasteriser* what
    ///    `sans-serif` means — a browser already knows. (4.3 MB → 3.3 MB
    ///    of base64.)
    /// 2. Only the glyphs actually set. A guide's charts use chord
    ///    letters, digits, accidentals and a few dozen SMuFL symbols;
    ///    Bravura alone ships thousands. Every character that reached a
    ///    `<text>` element was recorded while rendering, mapped through
    ///    the font's own cmap, and everything else dropped.
    ///
    /// `CmapTarget::Unicode` because the subset has to keep a cmap the
    /// browser can use — the SVG addresses glyphs by *character*, not by
    /// id. (This is why `subsetter`, the obvious crate, is the wrong
    /// tool: it strips cmap for PDF CID embedding.)
    ///
    /// A face that fails to subset is embedded whole rather than
    /// dropped: a chart missing its noteheads is a worse outcome than a
    /// chart that costs more to load.
    fn font_css(&self) -> String {
        let used = self.used.lock().expect("font usage map");
        let mut out = String::new();

        for (family, bytes) in self.pipeline.fonts().embeddable_fonts() {
            let Some(chars) = used.get(family) else {
                continue;
            };
            let data = subset(bytes.as_ref(), chars).unwrap_or_else(|| bytes.as_ref().clone());
            let _ = write!(
                out,
                "@font-face {{\n  font-family: '{family}';\n  \
                 src: url('data:font/otf;base64,{}') format('opentype');\n  \
                 font-display: block;\n}}\n",
                base64(&data)
            );
        }

        out
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

    /// Note every `font-family` this chart named, and the characters it
    /// set in each.
    ///
    /// Read back out of the finished SVG rather than tracked through the
    /// scene: the serializer is what decides which family a run of text
    /// ends up in, and second-guessing it is how the three pipelines
    /// drifted before. What the file says is the truth.
    fn record_families(&self, svg: &str) {
        let mut used = self.used.lock().expect("font usage map");
        let mut rest = svg;

        while let Some(at) = rest.find("<text ") {
            rest = &rest[at..];
            let Some(tag_end) = rest.find('>') else { break };
            let (tag, after) = rest.split_at(tag_end + 1);

            let family = tag
                .split_once("font-family=\"")
                .and_then(|(_, r)| r.split_once('"'))
                .map(|(name, _)| name.to_owned());
            let content = after.split_once("</text>").map(|(text, _)| text);

            if let (Some(family), Some(content)) = (family, content) {
                let entry = used.entry(family).or_default();
                // The serializer escapes its text, so `&amp;` arrives as
                // five characters. Unescaping the three it emits keeps
                // the recorded set honest about what will be drawn.
                for ch in unescape(content).chars() {
                    entry.insert(ch);
                }
            }

            rest = after;
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
/// The engraver, as the editor's `kf` fence renderer.
///
/// The guide's body is rendered by the editor now, which means fences go
/// through the editor's plugin registry rather than ssg's `.fence()`
/// hook. Registering THIS engraver rather than `editor_keyflow::Fences`
/// is deliberate: it is the one that records which glyphs each chart
/// draws, and `font_css` subsets the faces down to those. Register the
/// other and every chart still engraves, silently, while the page grows
/// back the 3 MB of typeface the subsetting removed.
impl editor_state::fence_renderer::FenceRenderer for Engraver {
    fn render_svg(&self, source: &str) -> Option<String> {
        self.svg(source)
    }

    fn highlight_html(&self, source: &str) -> String {
        highlight(source.trim_end())
    }
}

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

/// A font cut down to `chars`, or `None` if it cannot be.
///
/// The glyph ids come from the font's own cmap, so a character the face
/// does not have is simply skipped — it was going to render as
/// `.notdef` either way. Glyph 0 is always kept, because `.notdef` is
/// what a subset renders for anything that slips through, and allsorts
/// requires it.
fn subset(data: &[u8], chars: &BTreeSet<char>) -> Option<Vec<u8>> {
    use allsorts::binary::read::ReadScope;
    use allsorts::font_data::FontData;
    use allsorts::subset::{CmapTarget, SubsetProfile};

    let face = ttf_parser::Face::parse(data, 0).ok()?;

    let mut glyphs: Vec<u16> = vec![0];
    for ch in chars {
        if let Some(id) = face.glyph_index(*ch) {
            if !glyphs.contains(&id.0) {
                glyphs.push(id.0);
            }
        }
    }
    // Nothing but `.notdef` means nothing in this face was drawn; the
    // caller should not have asked, and a one-glyph font helps no one.
    if glyphs.len() == 1 {
        return None;
    }

    let font_data = ReadScope::new(data).read::<FontData<'_>>().ok()?;
    let provider = font_data.table_provider(0).ok()?;
    allsorts::subset::subset(
        &provider,
        &glyphs,
        // `Minimal` rather than `Pdf`: the browser needs a valid
        // OpenType font, not the reduced table set a PDF embeds.
        &SubsetProfile::Minimal,
        // The SVG addresses glyphs by CHARACTER, so the subset must keep
        // a cmap the browser can look them up in — and a Unicode one,
        // because browsers reject a font carrying only Mac Roman.
        CmapTarget::Unicode,
    )
    .ok()
}

/// Standard base64, for a `data:` URI.
///
/// Written out rather than taken as a dependency: it is fifteen lines,
/// and this is a build script that four other crates wait on.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            // A short final chunk pads with `=` rather than encoding the
            // zero bytes it was padded with.
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - i * 6)) as usize & 0x3F]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The three entities the SVG serializer emits, put back.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
