//! chordsheet.com → Keyflow.
//!
//! [Chord Sheet Maker Online](https://www.chordsheet.com) ("chordsheet.com")
//! is a long-running web tool for writing chords-only charts. Its source
//! format is deliberately terse — no delimiters, one chord to a bar, four
//! bars to a line:
//!
//! ```text
//! =INTRO
//! -"The song starts on the & of 3"
//! (A D/A_G_D/F#)x3 AD"Let it Ring Out for 6 Beats"%
//! ```
//!
//! This crate reads that language and translates it to a keyflow
//! [`Chart`](keyflow_proto::chart::Chart). Pair it with
//! `keyflow_text::chart::exporter::chart_to_keyflow` to write `.kf`, which is
//! what `kf chordsheet` does.
//!
//! # Two layers
//!
//! [`parse`] returns a lossless [`ast::Document`] — every line, every bar,
//! every marker, each with the byte span it came from. [`to_chart`] then
//! translates that into keyflow's model. Keeping them apart means the parts
//! keyflow has no room for are *available* rather than gone.
//!
//! # What crosses over
//!
//! | chordsheet.com | keyflow |
//! |---|---|
//! | `= TITLE` / `: TITLE` | a `ChartSection`; `=` also closes the previous bar with a double line |
//! | one chord per bar | a `Measure` with one chord |
//! | `AD/A`, `D/F#G` | two chords in one bar — an uppercase root starts the next chord, no separator needed |
//! | `Dm7_G7` (glue) | a split measure; the bar divides evenly among its cells |
//! | `Gm,,,`, `A,A,A,A,A,G#,C#C#` | one slot per comma, the chord riding the slot it precedes — eight chords in a 4/4 bar come out as eight eighths |
//! | `( … )` | `|:` … `:|` repeat marks |
//! | `( … )x3` | the pattern written out three times, the count on the first pass |
//! | `1.` `2.` `1.-3.+5.` | `Volta` brackets |
//! | `%` `%%` `%1` `%2` `%4` | the bars they stand for, copied forward |
//! | `r1` `r2` `r4` | rests in the measure's rhythm |
//! | `<C` `<<C` | a pushed chord (eighth / sixteenth) |
//! | `C^` | a tied chord |
//! | `C?` | the chord printed in parentheses |
//! | `C fermata` | a fermata command on the chord |
//! | `C"text"` | `StaffText` at that beat |
//! | `- text`, `; text`, `<- 5`, `>= A` | `StaffText` on the following bar |
//! | `3:4` | a time-signature change from that bar on |
//! | `\|\|` | a double barline |
//! | `fine` `$` `o` `O` `D.C.` `D.S. al coda` | boxed `StaffText` navigation marks |
//! | `*` | a bar with no chord |
//!
//! # What does not
//!
//! Page furniture, which keyflow's model has no place for and its engraver
//! decides for itself: palette colors (`P1`, `p4`), rulers (`RULER`,
//! `DOTTED5`), TAB rows (`TAB1`), page breaks (`+`), bar indents (`X`), and
//! chord-diagram definitions (`// C7 x32310`) along with the `.` and `'`
//! markers that place them. All of it is in the [`ast::Document`] if you want
//! it.
//!
//! # Where the translation decides something
//!
//! Four places where chordsheet.com says one thing and keyflow can only say
//! another. Each is deliberate, and each changes the chart:
//!
//! - **`%` and `%N` are expanded.** keyflow has no simile mark, so both become
//!   the bars they stand for. The music is right; the shorthand is gone.
//! - **A section with no bars recalls the last one of its kind.** `= CHORUS`
//!   under a chorus that already happened is the site's way of saying "again";
//!   the recalled section is flagged `from_template`, and whatever the author
//!   wrote under the header (`-"Same as VERSE 1"`) stays as its comment. With
//!   nothing to recall — a form-only chart, all headers and no chords — the
//!   section gets one empty bar, because keyflow cannot write a section of
//!   zero measures and dropping the header would throw away the only thing the
//!   chart says.
//! - **The first bar's meter becomes the chart's meter.** The source has no
//!   song-wide time signature, so a chart that isn't in 4/4 opens with a `6:8`
//!   on its first bar line. Note the corollary: a meter change *mid-chart*
//!   reaches the [`Chart`] as a `TimeSignatureChange` and as the affected
//!   measures' own signature, but `chart_to_keyflow` writes only the header
//!   meter — so a `.kf` export flattens it.
//! - **A crowded bar subdivides.** chordsheet.com has no notion of duration
//!   and will draw as many chords in a bar as you write. keyflow does, so the
//!   bar is split on the coarsest note grid that fits — and if a bar holds
//!   more than a 32nd-note grid can, it runs long rather than losing chords.
//!
//! # Metadata
//!
//! A chordsheet.com `.txt` holds no title, artist, key or tempo — the site
//! keeps those in database columns. An account backup writes them to
//! `manifest.json`, which [`manifest::Manifest`] reads; failing that,
//! [`ImportOptions::from_stem`] recovers title and artist from the
//! `ARTIST - TITLE.txt` filename the backup uses.
//!
//! # Example
//!
//! ```
//! use keyflow_chordsheet::{parse, to_chart, ImportOptions};
//!
//! let doc = parse("=VERSE\nG C Em D\n");
//! let chart = to_chart(&doc, &ImportOptions::from_stem("The Band - A Song"));
//!
//! assert_eq!(chart.metadata.artist.as_deref(), Some("The Band"));
//! assert_eq!(chart.sections.len(), 1);
//! assert_eq!(chart.sections[0].measures().len(), 4);
//! ```

pub mod ast;
pub mod convert;
#[cfg(feature = "manifest")]
pub mod manifest;
mod parser;

use std::path::{Path, PathBuf};

pub use ast::Document;
pub use convert::{to_chart, ImportOptions};
pub use parser::parse;

use keyflow_proto::chart::Chart;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[cfg(feature = "manifest")]
    #[error("parsing manifest {path}: {source}")]
    Manifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Parse chordsheet.com source text and translate it in one step.
#[must_use]
pub fn import_str(source: &str, options: &ImportOptions) -> Chart {
    to_chart(&parse(source), options)
}

/// Read a chordsheet.com `.txt` and translate it.
///
/// Title and artist come from the filename ([`ImportOptions::from_stem`]).
/// When a backup's `manifest.json` is at hand, prefer
/// [`manifest::Manifest::options_for`] and [`import_str`] — the manifest
/// knows the real artist name, which the filename can only approximate
/// (`AC/DC` is stored as `AC-DC`).
pub fn import_file(path: impl AsRef<Path>) -> Result<Chart, ImportError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    Ok(import_str(&source, &ImportOptions::from_stem(stem)))
}
