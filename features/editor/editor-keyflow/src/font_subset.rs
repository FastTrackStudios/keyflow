//! The typefaces a page's charts actually draw, as `@font-face` rules.
//!
//! Charts are exported *linked* — naming their typefaces rather than
//! embedding them — because the embedding variants cost a few hundred
//! kilobytes per chart and a guide chapter has dozens. So the document
//! declares the faces once and every chart on it resolves against one
//! download.
//!
//! Then that download is cut to size, in two passes:
//!
//! 1. Only the families a chart actually named. The bundle lists
//!    fourteen, several aliases for the same bytes and one a 1.6 MB
//!    fallback that exists to tell a *rasteriser* what `sans-serif`
//!    means — a browser already knows.
//! 2. Only the glyphs actually set. A guide's charts use chord letters,
//!    digits, accidentals and a few dozen SMuFL symbols; Bravura alone
//!    ships thousands.
//!
//! ## Why this is a library and not a build script
//!
//! It was a build script — Keyflow's site subset its fonts in `build.rs`
//! and nothing else could. That was fine while the guide could only be
//! rendered at build time. A dev server that re-renders the guide on
//! save needs the same subset for the charts it just drew, and the
//! alternative to sharing this code was a second copy that would drift
//! from the one that ships.
//!
//! Behind the `subset` feature: `allsorts` is a large dependency and
//! nothing in a browser needs it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Mutex;

/// Which characters each font family was asked to draw.
///
/// Populated by [`Self::record`] as charts are rendered, then turned
/// into subset `@font-face` rules by [`Self::font_face_css`].
#[derive(Default)]
pub struct FontUsage {
    // `Mutex`, not `RefCell`: this lives inside a fence renderer, and
    // that trait is `Send + Sync`.
    used: Mutex<BTreeMap<String, BTreeSet<char>>>,
}

impl FontUsage {
    /// An empty record.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Note every family and character drawn by one rendered chart.
    ///
    /// Read back out of the finished SVG rather than tracked through the
    /// scene: the serializer is what decides which family a run of text
    /// ends up in, and second-guessing it is how three pipelines drifted
    /// apart once already. What the file says is the truth.
    pub fn record(&self, svg: &str) {
        let Ok(mut used) = self.used.lock() else {
            return;
        };
        let mut rest = svg;

        while let Some(at) = rest.find("<text ") {
            rest = &rest[at..];
            let Some(tag_end) = rest.find('>') else { break };
            let (tag, after) = rest.split_at(tag_end.saturating_add(1));

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

    /// Have any charts been recorded?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.used.lock().is_ok_and(|u| u.is_empty())
    }

    /// `@font-face` rules for the recorded families, subset to the
    /// recorded glyphs and inlined as `data:` URIs.
    ///
    /// A face that fails to subset is embedded whole rather than
    /// dropped: a chart missing its noteheads is a worse outcome than a
    /// chart that costs more to load.
    ///
    /// # Errors
    /// If the engraving pipeline cannot be built.
    pub fn font_face_css(&self) -> Result<String, crate::RenderError> {
        let Ok(used) = self.used.lock() else {
            return Ok(String::new());
        };
        let pipeline = crate::pipeline()?;
        let mut out = String::new();

        for (family, bytes) in pipeline.fonts().embeddable_fonts() {
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

        Ok(out)
    }
}

/// A font cut down to `chars`, or `None` if it cannot be.
///
/// The glyph ids come from the font's own cmap, so a character the face
/// does not have is simply skipped — it was going to render as `.notdef`
/// either way. Glyph 0 is always kept, because `.notdef` is what a
/// subset renders for anything that slips through, and allsorts requires
/// it.
fn subset(data: &[u8], chars: &BTreeSet<char>) -> Option<Vec<u8>> {
    use allsorts::binary::read::ReadScope;
    use allsorts::font_data::FontData;
    use allsorts::subset::{CmapTarget, SubsetProfile};

    let face = ttf_parser::Face::parse(data, 0).ok()?;

    let mut glyphs: Vec<u16> = vec![0];
    for ch in chars {
        if let Some(id) = face.glyph_index(*ch)
            && !glyphs.contains(&id.0)
        {
            glyphs.push(id.0);
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
        //
        // This is why `subsetter`, the obvious crate, is the wrong tool:
        // it strips cmap for PDF CID embedding.
        CmapTarget::Unicode,
    )
    .ok()
}

/// Standard base64, for a `data:` URI.
///
/// Written out rather than taken as a dependency: it is fifteen lines,
/// and this crate is on the path of a build script four other crates
/// wait on.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_reference_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn recording_reads_families_and_characters_out_of_the_svg() {
        let usage = FontUsage::new();
        assert!(usage.is_empty());
        usage.record(r#"<svg><text font-family="Bravura" x="0">&amp;C#</text></svg>"#);
        let used = usage.used.lock().expect("usage");
        let chars = used.get("Bravura").expect("family recorded");
        // `&amp;` is one character once unescaped, not five.
        assert!(chars.contains(&'&'), "{chars:?}");
        assert!(chars.contains(&'C'), "{chars:?}");
        assert!(chars.contains(&'#'), "{chars:?}");
        assert!(!chars.contains(&'a'), "the entity leaked: {chars:?}");
    }

    #[test]
    fn text_without_a_family_is_ignored() {
        let usage = FontUsage::new();
        usage.record("<svg><text x=\"0\">C</text></svg>");
        assert!(usage.is_empty());
    }
}
