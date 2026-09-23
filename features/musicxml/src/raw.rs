//! Format-level MusicXML decoding, shared by every importer in the repo.
//!
//! Nothing here knows about a `Chart`, a `Score`, or any other domain
//! model — these are the parts of reading MusicXML that are the same no
//! matter what you are building out of it. They live here because three
//! crates had grown their own copies:
//! [`keyflow_musicxml`] itself, `keyflow-orchestra`'s score parser, and
//! `engraver-score`'s importer.
//!
//! The copies had already drifted: `articulation_tag` spelled its
//! `OtherArticulation` arm two different ways, and each `extract_mxl`
//! wrote its own error strings for identical failures. A tag table
//! duplicated across crates is drift waiting to happen, because nothing
//! makes the copies disagree loudly.

use musicxml::datatypes::StartStop;
use musicxml::elements::{ArticulationsType, DynamicsType, Tie};

/// A failure while unwrapping a compressed `.mxl` container.
///
/// Deliberately a plain message rather than a rich enum: every caller
/// folds it straight into its own error type, and none of them branch on
/// the cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxlError(pub String);

impl std::fmt::Display for MxlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MxlError {}

/// Unwrap a compressed `.mxl` container to the score XML inside it.
///
/// The `musicxml` crate ships its own zip reader, but it fails on
/// multi-file archives and on inner names that do not end in
/// `.musicxml` — MuseScore writes `score.xml` — so the container is
/// unpacked here instead.
///
/// `META-INF/container.xml` names the rootfile; when it is missing or
/// unreadable, the first top-level XML entry outside `META-INF` is used.
///
/// # Errors
///
/// Returns [`MxlError`] if the data is not a readable zip, if no score
/// XML can be located inside it, or if the entry cannot be decompressed.
pub fn extract_mxl(data: Vec<u8>) -> Result<Vec<u8>, MxlError> {
    use std::io::Read;

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|e| MxlError(format!("bad .mxl zip: {e}")))?;

    let mut root_path: Option<String> = None;
    if let Ok(mut container) = archive.by_name("META-INF/container.xml") {
        let mut text = String::new();
        if container.read_to_string(&mut text).is_ok() {
            if let Some(idx) = text.find("full-path=\"") {
                let rest = &text[idx + "full-path=\"".len()..];
                if let Some(end) = rest.find('"') {
                    root_path = Some(rest[..end].to_string());
                }
            }
        }
    }

    let root_path = match root_path {
        Some(p) => p,
        None => {
            // Fallback: the first top-level XML entry outside META-INF.
            let mut found = None;
            for i in 0..archive.len() {
                let name = archive
                    .by_index(i)
                    .map_err(|e| MxlError(e.to_string()))?
                    .name()
                    .to_string();
                let lower = name.to_lowercase();
                if !name.starts_with("META-INF")
                    && (lower.ends_with(".musicxml") || lower.ends_with(".xml"))
                {
                    found = Some(name);
                    break;
                }
            }
            found.ok_or_else(|| MxlError("no score XML in .mxl archive".into()))?
        }
    };

    let mut file = archive
        .by_name(&root_path)
        .map_err(|e| MxlError(format!("missing rootfile {root_path}: {e}")))?;
    let mut xml = Vec::new();
    file.read_to_end(&mut xml)
        .map_err(|e| MxlError(format!("decompress failed: {e}")))?;
    Ok(xml)
}

/// Decode XML predefined entities, iteratively.
///
/// Up to three passes, because lazy exporters double-encode: `&amp;amp;`
/// has to resolve all the way to `&`, not stop at `&amp;`. The loop
/// stops as soon as a pass changes nothing or no `&` remains.
#[must_use]
pub fn decode_entities(s: &str) -> String {
    let mut cur = s.to_string();
    for _ in 0..3 {
        if !cur.contains('&') {
            break;
        }
        let next = cur
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'");
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

/// Whether a note's `<tie>` elements start and/or stop a tie.
///
/// Both may be true at once: a note in the middle of a tie chain stops
/// the incoming tie and starts the outgoing one.
#[must_use]
pub fn tie_flags(ties: &[Tie]) -> (bool, bool) {
    let mut start = false;
    let mut stop = false;
    for t in ties {
        match t.attributes.r#type {
            StartStop::Start => start = true,
            StartStop::Stop => stop = true,
        }
    }
    (start, stop)
}

/// The conventional name for a `<dynamics>` element, if it has one.
///
/// Returns `None` for the marks that carry no conventional short name,
/// which callers generally skip rather than render.
#[must_use]
pub fn dynamics_tag(d: &DynamicsType) -> Option<&'static str> {
    use DynamicsType as D;
    Some(match d {
        D::P(_) => "p",
        D::Pp(_) => "pp",
        D::Ppp(_) => "ppp",
        D::Pppp(_) => "pppp",
        D::F(_) => "f",
        D::Ff(_) => "ff",
        D::Fff(_) => "fff",
        D::Ffff(_) => "ffff",
        D::Mp(_) => "mp",
        D::Mf(_) => "mf",
        D::Sf(_) => "sf",
        D::Sfz(_) => "sfz",
        D::Sffz(_) => "sffz",
        D::Fp(_) => "fp",
        D::Fz(_) => "fz",
        D::Rf(_) => "rf",
        D::Rfz(_) => "rfz",
        D::Sfp(_) => "sfp",
        _ => return None,
    })
}

/// The MusicXML element name for an `<articulations>` child.
///
/// The string is the element's own name, so it round-trips: whatever
/// consumes these tags can match the vocabulary the format defines
/// rather than a per-crate abbreviation.
#[must_use]
pub fn articulation_tag(a: &ArticulationsType) -> &'static str {
    match a {
        ArticulationsType::Accent(_) => "accent",
        ArticulationsType::StrongAccent(_) => "strong-accent",
        ArticulationsType::Staccato(_) => "staccato",
        ArticulationsType::Tenuto(_) => "tenuto",
        ArticulationsType::DetachedLegato(_) => "detached-legato",
        ArticulationsType::Staccatissimo(_) => "staccatissimo",
        ArticulationsType::Spiccato(_) => "spiccato",
        ArticulationsType::Scoop(_) => "scoop",
        ArticulationsType::Plop(_) => "plop",
        ArticulationsType::Doit(_) => "doit",
        ArticulationsType::Falloff(_) => "falloff",
        ArticulationsType::BreathMark(_) => "breath-mark",
        ArticulationsType::Caesura(_) => "caesura",
        ArticulationsType::Stress(_) => "stress",
        ArticulationsType::Unstress(_) => "unstress",
        ArticulationsType::SoftAccent(_) => "soft-accent",
        ArticulationsType::OtherArticulation(_) => "other-articulation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_five_predefined_entities() {
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_entities("&quot;q&quot; &apos;a&apos;"), "\"q\" 'a'");
    }

    #[test]
    fn double_encoded_entities_resolve_all_the_way() {
        // Lazy exporters emit `&amp;amp;` for a literal `&`; decoding
        // must not stop at `&amp;`.
        assert_eq!(decode_entities("a &amp;amp; b"), "a & b");
        assert_eq!(decode_entities("&amp;lt;"), "<");
    }

    #[test]
    fn a_non_mxl_payload_is_rejected() {
        assert!(extract_mxl(b"<score-partwise/>".to_vec()).is_err());
    }
}
