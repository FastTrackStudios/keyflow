//! The Keyflow keyboard.
//!
//! A system keyboard for writing Keyflow, in the spirit of Musician
//! Keyboard: the characters a musician actually needs are one tap away
//! instead of three modifier planes deep. `|`, `°`, `♭`, `𝄆`, section
//! headers, and chord qualities are not on any stock keyboard, and reaching
//! them through the numeric and symbol planes is what makes writing a chart
//! on a phone unpleasant today.
//!
//! # Why the layout lives in Rust
//!
//! An iOS custom keyboard is a `UIInputViewController` — an app extension,
//! which must be Swift or Objective-C. But the *interesting* part of a
//! language keyboard is not the view: it is knowing which tokens are legal
//! here, what a tap should insert, and what to suggest next. That is
//! knowledge the Keyflow parser already has, and duplicating it in Swift
//! would guarantee the keyboard and the language drift apart.
//!
//! So the split is: this module owns the layout and the suggestions and is
//! tested here; the Swift extension is a renderer that draws
//! [`KeyboardLayout`] and forwards taps. See `ios/README.md`.
//!
//! # Constraints the extension imposes
//!
//! An iOS keyboard extension has no network access, a hard memory ceiling
//! (roughly 30–50 MB before the system kills it), and only sees the
//! document text through `UITextDocumentProxy` — which exposes the text
//! immediately before and after the caret, not the whole document. So
//! [`suggestions`] is deliberately written against a *fragment*, not a
//! parsed chart: it must work with the little context the proxy gives.

use std::fmt;

/// What a key does when tapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Insert this text at the caret.
    Insert(&'static str),
    /// Insert this text and then a newline — section headers, mostly.
    InsertLine(&'static str),
    /// Delete backwards one character.
    Backspace,
    /// Switch to another plane.
    Plane(PlaneId),
}

/// A single key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// What the key face shows.
    pub label: &'static str,
    /// What tapping it does.
    pub action: Action,
    /// Long-press explanation, and the accessibility label.
    pub hint: &'static str,
}

impl Key {
    const fn insert(label: &'static str, hint: &'static str) -> Self {
        Self {
            label,
            action: Action::Insert(label),
            hint,
        }
    }

    /// A key whose face and inserted text differ — `barline` shows `|` but
    /// inserts `| ` so the user is not left typing against the bar.
    const fn insert_as(label: &'static str, text: &'static str, hint: &'static str) -> Self {
        Self {
            label,
            action: Action::Insert(text),
            hint,
        }
    }

    const fn line(label: &'static str, text: &'static str, hint: &'static str) -> Self {
        Self {
            label,
            action: Action::InsertLine(text),
            hint,
        }
    }
}

/// Which plane of the keyboard is showing.
///
/// Four planes rather than one crowded grid, because the four are used at
/// different moments: you lay out sections, then fill in changes, then add
/// rhythm, and only occasionally reach for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneId {
    /// Roots, qualities, extensions, slash bass.
    Chords,
    /// Bars, repeats, endings, rhythmic subdivision.
    Rhythm,
    /// Section headers.
    Sections,
    /// Accidentals, key/meter, annotation marks.
    Symbols,
}

impl PlaneId {
    /// Every plane, in tab order.
    pub const ALL: &'static [Self] = &[Self::Chords, Self::Rhythm, Self::Sections, Self::Symbols];

    /// The tab label.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Chords => "Chords",
            Self::Rhythm => "Rhythm",
            Self::Sections => "Sections",
            Self::Symbols => "Symbols",
        }
    }
}

impl fmt::Display for PlaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

/// One plane: rows of keys, drawn top to bottom.
pub struct Plane {
    /// Which plane this is.
    pub id: PlaneId,
    /// Key rows, in draw order.
    pub rows: &'static [&'static [Key]],
}

/// The whole keyboard.
pub struct KeyboardLayout;

impl KeyboardLayout {
    /// The plane for `id`.
    #[must_use]
    pub fn plane(id: PlaneId) -> Plane {
        let rows: &'static [&'static [Key]] = match id {
            PlaneId::Chords => CHORD_ROWS,
            PlaneId::Rhythm => RHYTHM_ROWS,
            PlaneId::Sections => SECTION_ROWS,
            PlaneId::Symbols => SYMBOL_ROWS,
        };
        Plane { id, rows }
    }
}

// The chord plane. Row one is scale degrees, because Nashville numbers are
// the default way charts are written here; row two is letter names for when
// a chart is in a fixed key; rows three and four are the qualities and
// extensions that turn a root into a chord.
static CHORD_ROWS: &[&[Key]] = &[
    &[
        Key::insert("1", "Scale degree 1"),
        Key::insert("2", "Scale degree 2"),
        Key::insert("3", "Scale degree 3"),
        Key::insert("4", "Scale degree 4"),
        Key::insert("5", "Scale degree 5"),
        Key::insert("6", "Scale degree 6"),
        Key::insert("7", "Scale degree 7"),
    ],
    &[
        Key::insert("C", "C root"),
        Key::insert("D", "D root"),
        Key::insert("E", "E root"),
        Key::insert("F", "F root"),
        Key::insert("G", "G root"),
        Key::insert("A", "A root"),
        Key::insert("B", "B root"),
    ],
    &[
        Key::insert("m", "Minor"),
        Key::insert("7", "Dominant 7th"),
        Key::insert("maj7", "Major 7th"),
        Key::insert("m7", "Minor 7th"),
        Key::insert("sus", "Suspended 4th"),
        Key::insert("add9", "Added 9th"),
    ],
    &[
        Key::insert("dim", "Diminished"),
        Key::insert("aug", "Augmented"),
        Key::insert("m7b5", "Half-diminished"),
        Key::insert("9", "Dominant 9th"),
        Key::insert("13", "Dominant 13th"),
        Key::insert_as("/", "/", "Slash bass"),
    ],
];

// The rhythm plane. The barline is the single most-typed character in a
// chart and the most awkward to reach on a stock keyboard, so it gets the
// widest key and inserts a trailing space.
static RHYTHM_ROWS: &[&[Key]] = &[
    &[
        Key::insert_as("|", "| ", "Barline"),
        Key::insert_as("‖", "|| ", "Double barline"),
        Key::insert_as("𝄆", "|: ", "Repeat open"),
        Key::insert_as("𝄇", ":| ", "Repeat close"),
    ],
    &[
        Key::insert_as("%", "% ", "Repeat previous bar"),
        Key::insert_as("×2", "x2 ", "Play twice"),
        Key::insert_as("×4", "x4 ", "Play four times"),
        Key::insert_as("1.", "|1. ", "First ending"),
        Key::insert_as("2.", "|2. ", "Second ending"),
    ],
    &[
        Key::insert_as("·", ". ", "Dotted"),
        Key::insert_as("–", "- ", "Hold / tie"),
        Key::insert_as("↑", "^ ", "Push (anticipate)"),
        Key::insert_as(">", "> ", "Accent"),
    ],
];

// The section plane. Labels are the abbreviations the parser accepts, and
// each inserts the `:` and a newline, because a section header is always
// followed by its content on the next line.
static SECTION_ROWS: &[&[Key]] = &[
    &[
        Key::line("Intro", "IN: ", "Intro"),
        Key::line("Verse", "VS: ", "Verse"),
        Key::line("Pre", "PreCH: ", "Pre-chorus"),
        Key::line("Chorus", "CH: ", "Chorus"),
    ],
    &[
        Key::line("Bridge", "Bridge: ", "Bridge"),
        Key::line("Solo", "Solo: ", "Solo"),
        Key::line("Inst", "INST: ", "Instrumental"),
        Key::line("Interlude", "Interlude: ", "Interlude"),
    ],
    &[
        Key::line("Outro", "Outro: ", "Outro"),
        Key::line("End", "End: ", "Ending tag"),
        Key::line("Hits", "HITS: ", "Hits / stops"),
    ],
];

// Accidentals and the header line. `#` and `b` are on the stock keyboard
// but two planes away; the musical glyphs are not there at all.
static SYMBOL_ROWS: &[&[Key]] = &[
    &[
        Key::insert_as("♯", "#", "Sharp"),
        Key::insert_as("♭", "b", "Flat"),
        Key::insert_as("♮", "n", "Natural"),
        Key::insert_as("°", "dim", "Diminished"),
    ],
    &[
        Key::insert_as("4/4", "4/4 ", "Common time"),
        Key::insert_as("3/4", "3/4 ", "Waltz time"),
        Key::insert_as("6/8", "6/8 ", "Compound duple"),
        Key::insert_as("bpm", "bpm ", "Tempo"),
    ],
    &[
        Key::insert_as("(", "(", "Open annotation"),
        Key::insert_as(")", ")", "Close annotation"),
        Key::insert_as("→", " -> ", "Leads to"),
        Key::insert_as("↵", "\n", "New line"),
    ],
];

/// Suggestions for the fragment immediately before the caret.
///
/// Written against a text fragment rather than a parsed chart because that
/// is all an iOS keyboard extension can see: `UITextDocumentProxy` exposes
/// the text around the caret, not the document. Best-effort by design — a
/// wrong suggestion costs a tap, a crash costs the keyboard.
///
/// Returns at most `limit` labels, ordered most-likely-first.
#[must_use]
pub fn suggestions(before_caret: &str, limit: usize) -> Vec<&'static str> {
    let token = before_caret
        .rsplit(|c: char| c.is_whitespace() || c == '|')
        .next()
        .unwrap_or("");

    // Immediately after a root, the useful next tap is a quality.
    let after_root = token
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || ('A'..='G').contains(&c))
        && token.len() == 1;

    let pool: &[&'static str] = if token.is_empty() {
        // Start of a bar: roots.
        &["1", "4", "5", "6m", "2m", "3m", "|"]
    } else if after_root {
        &["m", "7", "maj7", "m7", "sus", "/", "add9"]
    } else {
        // Mid-token: extensions that can still be appended.
        &["7", "9", "11", "13", "b5", "#5", "b9"]
    };

    pool.iter().copied().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_keys() -> Vec<&'static Key> {
        PlaneId::ALL
            .iter()
            .flat_map(|id| KeyboardLayout::plane(*id).rows)
            .flat_map(|row| row.iter())
            .collect()
    }

    #[test]
    fn every_plane_has_keys() {
        for id in PlaneId::ALL {
            let plane = KeyboardLayout::plane(*id);
            assert!(!plane.rows.is_empty(), "{id} has no rows");
            for row in plane.rows {
                assert!(!row.is_empty(), "{id} has an empty row");
            }
        }
    }

    #[test]
    fn no_row_is_too_wide_for_a_phone() {
        // Above about seven keys a row is untappable on the narrowest
        // supported device. This is the constraint that shapes the layout,
        // so it is asserted rather than left to review.
        for id in PlaneId::ALL {
            for row in KeyboardLayout::plane(*id).rows {
                assert!(row.len() <= 7, "{id} has a row of {} keys", row.len());
            }
        }
    }

    #[test]
    fn every_key_is_labelled_and_explained() {
        for key in all_keys() {
            assert!(!key.label.is_empty(), "a key has no face");
            assert!(!key.hint.is_empty(), "key `{}` has no hint", key.label);
        }
    }

    #[test]
    fn section_keys_insert_headers_the_parser_accepts() {
        // The section plane is only useful if what it types actually parses.
        // These are the abbreviations completion offers, so they must round
        // trip through the parser.
        for key in KeyboardLayout::plane(PlaneId::Sections)
            .rows
            .iter()
            .flat_map(|r| r.iter())
        {
            let Action::InsertLine(text) = key.action else {
                panic!("section key `{}` should insert a line", key.label);
            };
            let chart = format!("{text}| 1 4 |\n");
            assert!(
                keyflow::text::chart::parse_chart(&chart).is_ok(),
                "section key `{}` types `{text}`, which does not parse",
                key.label
            );
        }
    }

    #[test]
    fn chord_keys_compose_into_parseable_bars() {
        // A root plus a quality is the core gesture of the chord plane.
        for root in ["1", "4", "C", "G"] {
            for quality in ["", "m", "7", "maj7", "m7", "sus", "add9"] {
                let chart = format!("VS: | {root}{quality} |\n");
                assert!(
                    keyflow::text::chart::parse_chart(&chart).is_ok(),
                    "`{root}{quality}` does not parse"
                );
            }
        }
    }

    #[test]
    fn suggestions_respect_the_limit() {
        for fragment in ["", "C", "| 1 4 ", "Cmaj"] {
            assert!(suggestions(fragment, 4).len() <= 4);
        }
    }

    #[test]
    fn a_bare_root_suggests_qualities() {
        assert!(suggestions("| C", 8).contains(&"m7"));
        assert!(suggestions("| 1", 8).contains(&"m"));
    }

    #[test]
    fn the_start_of_a_bar_suggests_roots() {
        assert!(suggestions("VS: | ", 8).contains(&"1"));
    }

    #[test]
    fn suggestions_never_panic_on_odd_input() {
        for fragment in ["", " ", "|||", "🎹", "\n\n", "a".repeat(4096).as_str()] {
            let _ = suggestions(fragment, 6);
        }
    }
}
