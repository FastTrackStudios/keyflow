//! Keyflow's section markers for parts.
//!
//! A part's rehearsal marks are a mix of three things a player reads
//! differently, and Finale stores them all as one kind of text:
//!
//! - **sections** of the song form — `VS 1`, `CH 2a`, `Sax Solo`,
//!   `Re-INTRO`, `Tag #7 FINAL` — which keyflow draws the way the master
//!   rhythm chart does: the chart's own margin badge when the section starts
//!   a system, and a small pill in the chart's colours over the barline when
//!   it starts mid-line;
//! - **letters and bar numbers** — `A`, `79` — the classical rehearsal
//!   marks, drawn the same way in a neutral colour;
//! - **callouts** — `*Band In:`, `Drums Enter`, `Big Rall>>>` — cues, set
//!   as a tinted label with an accent bar.
//!
//! [`classify`] makes that call from the text alone.
//!
//! Every marker also has a monochrome rendering for black-and-white
//! printers: solid black with white type, so it reads without colour and
//! survives a cheap laser printer. The colour rendering is not derived from
//! it — each is drawn the way it reads best. (The drawing lives in
//! `layout::vertical`.)

use keyflow_proto::sections::SectionType;
use peniko::Color;

/// How a part draws its rehearsal marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkStyle {
    /// Keyflow markers in the section colours.
    #[default]
    Color,
    /// Keyflow markers for a black-and-white printer.
    Mono,
    /// The source's own boxes and text, as Finale printed them.
    Classic,
}

impl MarkStyle {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "color" | "colour" => Some(Self::Color),
            "mono" | "bw" | "monochrome" => Some(Self::Mono),
            "classic" | "finale" => Some(Self::Classic),
            _ => None,
        }
    }
}

/// A section of the song form, read from a rehearsal mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionMark {
    pub kind: SectionType,
    /// What the marker shows: `VS 1`, `CH 2a`, `RE-IN`, `VS 1a/1b`.
    pub label: String,
    /// The rest of the mark (`Sax`, `midway`, `FINAL`, `Half-Time`), drawn
    /// beside the marker.
    pub note: Option<String>,
}

/// What a rehearsal mark is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkClass {
    Section(SectionMark),
    /// A rehearsal letter or bar number.
    Letter(String),
    /// A cue: anything that is not a section.
    Callout(String),
}

/// The section a single word names, if any. Deliberately stricter than
/// [`SectionType::parse`], whose fuzzy prefix match would read "Chords" as a
/// chorus: a cue is far more often mistaken for a section than the reverse.
/// Two-letter words that are also English (`in`, `out`) are left out —
/// "Band In" is a cue.
fn section_word(word: &str) -> Option<SectionType> {
    Some(match word {
        "vs" | "verse" | "verses" => SectionType::Verse,
        "ch" | "chorus" | "choruses" => SectionType::Chorus,
        "br" | "bridge" => SectionType::Bridge,
        "intro" => SectionType::Intro,
        "outro" => SectionType::Outro,
        "inst" | "instrumental" => SectionType::Instrumental,
        "solo" | "solos" => SectionType::Solo,
        "tag" | "tags" => SectionType::Tag,
        "interlude" => SectionType::Interlude,
        "breakdown" | "break" => SectionType::Breakdown,
        "transition" => SectionType::Transition,
        "vamp" => SectionType::Vamp,
        "refrain" => SectionType::Refrain,
        "turnaround" => SectionType::Turnaround,
        "countoff" | "count-off" | "countin" | "count-in" => SectionType::CountIn,
        "pre" | "prechorus" | "pre-chorus" | "pre-ch" => {
            SectionType::Pre(Box::new(SectionType::Chorus))
        }
        "postchorus" | "post-chorus" | "post-ch" => {
            SectionType::Post(Box::new(SectionType::Chorus))
        }
        _ => return None,
    })
}

/// A section number: `1`, `2a`, `#7`. Returns (number, what follows a
/// hyphen) — `3-INST` → (`3`, `INST`).
fn section_number(token: &str) -> Option<(String, Option<String>)> {
    let t = token.strip_prefix('#').unwrap_or(token);
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let mut end = digits;
    // One trailing letter: `1a`, `2b`.
    if let Some(c) = t[digits..].chars().next()
        && c.is_ascii_alphabetic()
        && !t[digits + 1..].starts_with(|c: char| c.is_ascii_alphabetic())
    {
        end += 1;
    }
    let rest = t[end..].trim_start_matches(['-', '.']);
    if !rest.is_empty() && !t[end..].starts_with('-') {
        return None; // `3rd`, `1st` are ordinals, not section numbers
    }
    Some((
        t[..end].to_string(),
        (!rest.is_empty()).then(|| rest.to_string()),
    ))
}

/// Normalise a mark's lines to one line: a line ending in a hyphen joins
/// the next without a space (`RE-` / `INTRO` → `RE-INTRO`).
fn flatten(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !out.is_empty() && !out.ends_with('-') {
            out.push(' ');
        }
        out.push_str(line);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Read a rehearsal mark.
#[must_use]
pub fn classify(text: &str) -> MarkClass {
    let flat = flatten(text);
    if flat.is_empty() {
        return MarkClass::Callout(flat);
    }
    // Rehearsal letters (`A`, `AA`) and bar numbers (`79`).
    if (flat.len() <= 2
        && flat.chars().all(|c| c.is_ascii_uppercase())
        && section_word(&flat.to_ascii_lowercase()).is_none())
        || (flat.len() <= 4 && flat.chars().all(|c| c.is_ascii_digit()))
    {
        return MarkClass::Letter(flat);
    }
    // Starred, bracketed and colon-terminated text is a cue by convention
    // (`*Band In:`, `[ Guitars ]`, `Strings 8va:`).
    if flat.starts_with(['*', '[']) || flat.contains(':') {
        return MarkClass::Callout(flat);
    }
    let tokens: Vec<&str> = flat.split(' ').collect();
    for (i, tok) in tokens.iter().enumerate() {
        let lower = tok.trim_end_matches(['.', ',', '!']).to_ascii_lowercase();
        // `Re-INTRO`, `RE- INTRO` (flattened to `RE-INTRO`).
        let (re, word) = match lower.strip_prefix("re-") {
            Some(w) => (true, w),
            None => (false, lower.as_str()),
        };
        let (re, kind) = match section_word(word) {
            Some(kind) => (re, kind),
            // `Re-` alone before the section word: `Re- INTRO`.
            None if lower == "re-" || lower == "re" => match tokens
                .get(i + 1)
                .and_then(|t| section_word(&t.to_ascii_lowercase()))
            {
                Some(kind) => return section(kind, true, &tokens[i + 2..], &tokens[..i]),
                None => continue,
            },
            None => continue,
        };
        return section(kind, re, &tokens[i + 1..], &tokens[..i]);
    }
    MarkClass::Callout(flat)
}

fn section(kind: SectionType, re: bool, after: &[&str], before: &[&str]) -> MarkClass {
    let abbr = kind.abbreviation();
    let mut label = if re { format!("RE-{abbr}") } else { abbr };
    let mut rest: Vec<String> = before.iter().map(ToString::to_string).collect();
    let mut after = after;
    if let Some((n, tail)) = after.first().and_then(|t| section_number(t)) {
        label.push(' ');
        label.push_str(&n);
        after = &after[1..];
        if let Some(t) = tail {
            rest.push(t);
        }
        // `VS 1a VS 1b`: one marker for the pair.
        if after.len() >= 2
            && section_word(&after[0].to_ascii_lowercase()).as_ref() == Some(&kind)
            && let Some((n2, None)) = section_number(after[1])
        {
            label.push('/');
            label.push_str(&n2);
            after = &after[2..];
        }
    }
    rest.extend(after.iter().map(ToString::to_string));
    let note = rest.join(" ");
    MarkClass::Section(SectionMark {
        kind,
        label,
        note: (!note.is_empty()).then_some(note),
    })
}

/// Callout colours: an accent bar, a tint behind the text, the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalloutPaint {
    pub bar: Color,
    pub tint: Option<Color>,
    pub text: Color,
}

#[must_use]
pub fn callout_paint(style: MarkStyle) -> CalloutPaint {
    match style {
        // Amber: reads as "attention" and is distinct from every section
        // colour a marker beside it could carry.
        MarkStyle::Color | MarkStyle::Classic => CalloutPaint {
            bar: Color::from_rgb8(0xd9, 0x77, 0x06),
            tint: Some(Color::from_rgb8(0xfe, 0xf3, 0xc7)),
            text: Color::from_rgb8(0x78, 0x35, 0x0f),
        },
        // A grey tint prints as a dither that muddies small type; a solid
        // bar does not.
        MarkStyle::Mono => CalloutPaint {
            bar: Color::BLACK,
            tint: None,
            text: Color::BLACK,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(text: &str) -> (String, Option<String>) {
        match classify(text) {
            MarkClass::Section(s) => (s.label, s.note),
            other => panic!("{text:?} → {other:?}"),
        }
    }

    #[test]
    fn reads_section_marks() {
        assert_eq!(sec("VS 1"), ("VS 1".into(), None));
        assert_eq!(sec("CH 2a"), ("CH 2a".into(), None));
        assert_eq!(sec("Chorus 3"), ("CH 3".into(), None));
        assert_eq!(sec("Verse 1"), ("VS 1".into(), None));
        assert_eq!(sec("BR"), ("BR".into(), None));
        assert_eq!(sec("TAG 2"), ("TAG 2".into(), None));
        assert_eq!(sec("Tag #7 FINAL"), ("TAG 7".into(), Some("FINAL".into())));
        assert_eq!(sec("VS 3-INST"), ("VS 3".into(), Some("INST".into())));
        assert_eq!(sec("VS 3 INST"), ("VS 3".into(), Some("INST".into())));
        assert_eq!(sec("VS 2\n ext."), ("VS 2".into(), Some("ext.".into())));
        assert_eq!(
            sec("CH 3\nHalf-Time"),
            ("CH 3".into(), Some("Half-Time".into()))
        );
        assert_eq!(sec("VS 1a\nVS 1b"), ("VS 1a/1b".into(), None));
        assert_eq!(sec("Sax Solo"), ("SOLO".into(), Some("Sax".into())));
        assert_eq!(sec("GTR\nSOLO"), ("SOLO".into(), Some("GTR".into())));
        assert_eq!(sec("GTR Solo #1"), ("SOLO 1".into(), Some("GTR".into())));
        assert_eq!(
            sec("1st Gtr. Solo"),
            ("SOLO".into(), Some("1st Gtr.".into()))
        );
        assert_eq!(sec("Last Chorus"), ("CH".into(), Some("Last".into())));
        assert_eq!(
            sec("Orchestra Bridge"),
            ("BR".into(), Some("Orchestra".into()))
        );
        assert_eq!(sec("Re-INTRO"), ("RE-IN".into(), None));
        assert_eq!(sec("RE-\nINTRO"), ("RE-IN".into(), None));
        assert_eq!(sec("Re- INTRO"), ("RE-IN".into(), None));
        assert_eq!(sec("OUTRO\n OD #1"), ("OUT".into(), Some("OD #1".into())));
        assert_eq!(sec("Interlude"), ("INT".into(), None));
        assert_eq!(sec("Transition"), ("TRANS".into(), None));
        assert_eq!(sec("Pre"), ("PRE-CH".into(), None));
        assert_eq!(sec("TAGs"), ("TAG".into(), None));
        assert_eq!(sec("String\nBreak"), ("BD".into(), Some("String".into())));
        assert_eq!(sec("WSOP\n CH 1"), ("CH 1".into(), Some("WSOP".into())));
        assert_eq!(sec("VS midway"), ("VS".into(), Some("midway".into())));
    }

    #[test]
    fn letters_and_numbers() {
        assert_eq!(classify("A"), MarkClass::Letter("A".into()));
        assert_eq!(classify("79"), MarkClass::Letter("79".into()));
        assert_eq!(classify("115"), MarkClass::Letter("115".into()));
    }

    #[test]
    fn cues_are_callouts() {
        for cue in [
            "*Band In:",
            "Drums Enter",
            "Band Enters",
            "Big Rall>>>",
            "[ Guitars ]",
            "Keys",
            "Vocoder:",
            "Add GTR/BASS:",
            "1st x: Gtr. Solo\n2nd x: Sax Solo",
            "Clap on 2 & 4 thru 112",
            "Perc. Loop",
            "\"The Raven\"",
            "Chords only",
            "Fade In",
        ] {
            assert!(
                matches!(classify(cue), MarkClass::Callout(_)),
                "{cue:?} → {:?}",
                classify(cue)
            );
        }
    }
}
