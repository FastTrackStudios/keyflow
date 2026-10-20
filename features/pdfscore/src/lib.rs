//! Read engraved sheet music out of vector PDFs.
//!
//! A PDF exported from notation software (Sibelius, Finale, Dorico,
//! MuseScore) is not a picture of the music: every notehead, clef, rest and
//! accidental is a character of a music font at an exact position, and every
//! staff line, stem, barline and beam is a vector path. Reading those back is
//! exact where image recognition guesses, so this crate does that instead of
//! OMR whenever the PDF allows it.
//!
//! The pipeline:
//!
//! 1. [`extract`]: interpret each page's content stream into positioned
//!    glyphs (font, character, origin, size), stroked paths and filled
//!    shapes, and embedded images (a pasted screenshot is left to image
//!    recognition).
//! 2. Font tables: each engraver's music font mapped to SMuFL.
//! 3. Recognition: staves, systems, barlines, notes, rhythms, chord symbols
//!    and text, into a score.

pub mod extract;
pub mod keyflow;
pub mod recognize;

pub use extract::{Fill, Glyph, Image, Page, Stroke, read_pdf};
pub use keyflow::to_keyflow;
pub use recognize::{Recognized, Report, recognize};

/// What can go wrong reading a PDF.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PDF: {0}")]
    Pdf(#[from] lopdf::Error),
}
