//! Chart state in the URL.
//!
//! A Keyflow chart is small, plain text, and highly compressible — a
//! two-hundred-bar arrangement is a couple of kilobytes of ASCII with
//! enormous repetition. That makes it a good fit for the address bar: a
//! chart can be shared as a link with no account, no database, and no
//! server round trip, and the recipient gets the exact document the sender
//! was looking at.
//!
//! The encoding is deflate then base64url (no padding), so the result is
//! safe in a path segment. Compression is what makes it viable — the
//! Thriller example is 1.6 KB of source and encodes to roughly 600
//! characters.
//!
//! # Limits
//!
//! Browsers and CDNs disagree about how long a URL may be; the practical
//! ceiling is around 2000 characters, and some proxies cut off earlier.
//! [`MAX_URL_CHART_LEN`] is the point past which the site should stop
//! offering a link and ask the user to save the chart instead. Encoding a
//! larger chart still works — it just may not survive every hop.

use std::io::{Read, Write};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

/// Longest encoded chart the site will hand out as a shareable link.
///
/// Not a hard limit of the codec — a limit of the transport. See the module
/// docs.
// The encoder currently has no caller. `/c/:data` still decodes — a link
// someone already holds keeps working — but the control that produced
// those links was removed from the editor along with its toolbar row, so
// nothing in the UI mints one any more.
//
// Kept, not deleted: encoding a chart into its URL is this site's whole
// persistence story until accounts exist, and the round-trip is covered
// by the tests below. It wants a new home, not a rewrite.
#[allow(dead_code)]
pub const MAX_URL_CHART_LEN: usize = 1800;

/// Why a chart could not be read back out of a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The segment was not valid base64url.
    NotBase64,
    /// The bytes decoded, but were not a deflate stream.
    NotDeflate,
    /// The stream inflated, but not to UTF-8.
    NotUtf8,
    /// The stream inflated past [`MAX_DECOMPRESSED`] and was abandoned.
    ///
    /// A URL is attacker-supplied, and deflate is trivially made to expand
    /// enormously from a short input. The decoder is bounded so a crafted
    /// link cannot exhaust the tab's memory.
    TooLarge,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::NotBase64 => "the link's chart data is not valid base64",
            Self::NotDeflate => "the link's chart data is not a valid compressed stream",
            Self::NotUtf8 => "the link's chart data is not valid text",
            Self::TooLarge => "the link's chart is too large to open",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for DecodeError {}

/// Ceiling on the inflated size, in bytes. Well above any real chart.
const MAX_DECOMPRESSED: u64 = 4 * 1024 * 1024;

/// Compress and encode a chart for use as a URL path segment.
///
/// Infallible in practice: deflate over an in-memory buffer has no failure
/// mode that is not a bug, so a write error panics rather than propagating a
/// `Result` every caller would `expect` on.
#[must_use]
#[allow(dead_code)]
pub fn encode(source: &str) -> String {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(source.as_bytes())
        .expect("writing to an in-memory deflate encoder cannot fail");
    let compressed = encoder
        .finish()
        .expect("finishing an in-memory deflate encoder cannot fail");
    URL_SAFE_NO_PAD.encode(compressed)
}

/// Read a chart back out of a URL path segment.
///
/// # Errors
///
/// Returns [`DecodeError`] if the segment is not base64url, is not a deflate
/// stream, does not inflate to UTF-8, or inflates past [`MAX_DECOMPRESSED`].
pub fn decode(encoded: &str) -> Result<String, DecodeError> {
    let compressed = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| DecodeError::NotBase64)?;

    let mut out = Vec::new();
    DeflateDecoder::new(compressed.as_slice())
        .take(MAX_DECOMPRESSED)
        .read_to_end(&mut out)
        .map_err(|_| DecodeError::NotDeflate)?;

    if out.len() as u64 >= MAX_DECOMPRESSED {
        return Err(DecodeError::TooLarge);
    }

    String::from_utf8(out).map_err(|_| DecodeError::NotUtf8)
}

/// Whether an encoded chart is short enough to share as a link.
#[must_use]
#[allow(dead_code)]
pub fn fits_in_url(encoded: &str) -> bool {
    encoded.len() <= MAX_URL_CHART_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_chart() {
        let source = keyflow_ui::examples::EXAMPLE_THRILLER;
        let encoded = encode(source);
        assert_eq!(decode(&encoded).unwrap(), source);
    }

    #[test]
    fn round_trips_empty_and_unicode() {
        for source in ["", "Café — 4/4 #G\n\nVS 1: | 1 4 |\n"] {
            assert_eq!(decode(&encode(source)).unwrap(), source);
        }
    }

    #[test]
    fn encoded_chart_is_url_safe() {
        let encoded = encode(keyflow_ui::examples::EXAMPLE_THRILLER);
        assert!(
            encoded
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "encoded chart must survive a URL path segment unescaped"
        );
    }

    #[test]
    fn a_real_chart_fits_in_a_link() {
        let encoded = encode(keyflow_ui::examples::EXAMPLE_THRILLER);
        assert!(
            fits_in_url(&encoded),
            "Thriller encoded to {} chars, over the {MAX_URL_CHART_LEN} budget",
            encoded.len()
        );
    }

    #[test]
    fn compression_is_what_makes_this_viable() {
        let source = keyflow_ui::examples::EXAMPLE_THRILLER;
        let encoded = encode(source);
        assert!(
            encoded.len() < source.len(),
            "encoding grew the chart ({} -> {}); the URL scheme only works \
             because charts compress",
            source.len(),
            encoded.len()
        );
    }

    #[test]
    fn rejects_garbage_rather_than_panicking() {
        assert_eq!(decode("not base64!!"), Err(DecodeError::NotBase64));
        assert_eq!(decode("aGVsbG8"), Err(DecodeError::NotDeflate));
    }

    #[test]
    fn rejects_a_decompression_bomb() {
        // ~8 MiB of zeros deflates to a few KB and would otherwise inflate
        // straight into the tab's heap.
        let bomb = encode(&"0".repeat(8 * 1024 * 1024));
        assert_eq!(decode(&bomb), Err(DecodeError::TooLarge));
    }
}
