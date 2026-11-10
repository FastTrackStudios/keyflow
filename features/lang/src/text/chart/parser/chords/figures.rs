//! Figured bass, suspension figures, `^` inversions and the floating
//! slash bass (`/D`).

use super::annotations::{split_chord_attached_quote, unescape_quoted};
use crate::text::chart::parser::ChartParser;
use crate::text::chart::types::Measure;
use keyflow_proto::chart::{FiguredBass, FiguredBassRow, Placement, SuspensionFigure};

impl<'a> ChartParser<'a> {
    /// A floating slash-bass token like `/D`, `/Bb`, `/F#` — a leading slash
    /// followed by a bare note name (optional accidental) and nothing else.
    /// These inherit the previous chord's root (`Bb` → `Bb/D`) but display
    /// verbatim. Returns the bass-note text (without the slash).
    ///
    /// Deliberately rejects rhythm slashes (`//`, `/.`), durations (`/4`),
    /// commands (`\fermata`), and slash-family notation (`/maj7`).
    pub(super) fn parse_floating_slash_bass(token: &str) -> Option<&str> {
        let bass = token.strip_prefix('/')?;
        if bass.is_empty() || bass.contains('/') {
            return None;
        }
        let mut chars = bass.chars();
        let first = chars.next()?;
        if !matches!(first, 'A'..='G' | 'a'..='g') {
            return None;
        }
        // Remaining chars (if any) must be a single accidental.
        match chars.as_str() {
            "" | "#" | "b" | "n" | "\u{266d}" | "\u{266f}" | "\u{266e}" => Some(bass),
            _ => None,
        }
    }

    /// A floating suspension figure token. Only the hyphenated form (`4-3`,
    /// `2-3`) is recognized when standalone: a lone digit like `2`, `3`, or
    /// `4` is a valid scale-degree chord in keyflow, so accepting it here would
    /// swallow real chords. Single-digit figures must be written attached to a
    /// note-name root (`Eb2`, `F3`), which [`extract_suspension_suffix`]
    /// handles unambiguously. Returns the figure verbatim.
    pub(super) fn parse_bare_suspension_figure(token: &str) -> Option<String> {
        let (a, b) = token.split_once('-')?;
        (!a.is_empty()
            && !b.is_empty()
            && a.chars().all(|c| c.is_ascii_digit())
            && b.chars().all(|c| c.is_ascii_digit()))
        .then(|| token.to_string())
    }

    /// Split a trailing suspension figure off an attached chord token, e.g.
    /// `F4-3` → (`F`, `4-3`), `Eb2` → (`Eb`, `2`), `F3` → (`F`, `3`),
    /// `E3-4-3` → (`E`, `3-4-3`).
    ///
    /// Only fires when the chord part is a bare note root (letter + optional
    /// accidental) and the suffix is a figure: a hyphenated digit run (`d-d`,
    /// `d-d-d`, …, always a figure) or a lone `2`/`3`/`4` (the suspension
    /// degrees — so `Csus4`, `C6`, `Cm7`, `Gm7b5` stay intact).
    pub(super) fn extract_suspension_suffix(token: &str) -> (String, Option<String>) {
        // Slash chords and quoted suffixes are handled elsewhere.
        if token.contains('/') || token.contains('"') {
            return (token.to_string(), None);
        }
        // Length of the leading bare note root (letter + optional accidental).
        let root_len = Self::bare_note_root_len(token);
        if root_len == 0 {
            return (token.to_string(), None);
        }
        let (root, suffix) = token.split_at(root_len);
        let is_figure = if suffix.contains('-') {
            // Hyphenated digit run: every dash-separated part is digits.
            suffix
                .split('-')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        } else {
            // Lone suspension degree.
            matches!(suffix, "2" | "3" | "4")
        };
        if is_figure {
            (root.to_string(), Some(suffix.to_string()))
        } else {
            (token.to_string(), None)
        }
    }

    /// Byte length of a leading bare note root: a letter `A`–`G` (either case)
    /// plus an optional single accidental. Returns 0 if the token doesn't
    /// start with a note name.
    fn bare_note_root_len(s: &str) -> usize {
        let mut chars = s.char_indices();
        match chars.next() {
            Some((_, 'A'..='G' | 'a'..='g')) => {}
            _ => return 0,
        }
        match chars.next() {
            Some((i, c @ ('#' | 'b' | 'n' | '\u{266d}' | '\u{266f}' | '\u{266e}'))) => {
                i + c.len_utf8() // root = note letter + this accidental
            }
            Some((i, _)) => i, // root is just the note letter
            None => s.len(),   // single-char token, all root
        }
    }

    /// Root portion of the most recent chord (current measure first, then the
    /// last completed measure). A floating slash-bass inherits this — existing
    /// bass is dropped so `F/Eb` then `/D` yields `F/D`.
    pub(super) fn previous_chord_root(current: &Measure, measures: &[Measure]) -> Option<String> {
        let prev = current
            .chords
            .last()
            .or_else(|| measures.iter().rev().find_map(|m| m.chords.last()))?;
        let base = prev
            .full_symbol
            .split('/')
            .next()
            .unwrap_or(&prev.full_symbol);
        (!base.is_empty()).then(|| base.to_string())
    }

    pub(super) fn push_suspension_at_current_beat(
        measure: &mut Measure,
        figure: &str,
        placement: Placement,
        current_measure_beats: f64,
        standalone: bool,
    ) {
        let beat = current_measure_beats.floor().max(0.0) as u8 + 1;
        measure.suspensions.push(SuspensionFigure {
            figure: figure.to_string(),
            beat,
            placement,
            standalone,
        });
    }

    pub(super) fn extract_figured_bass_suffix(
        token: &str,
    ) -> (String, Option<Vec<FiguredBassRow>>) {
        if let Some((chord_token, quoted)) = split_chord_attached_quote(token) {
            let text = unescape_quoted(quoted);
            if let Some(rows) = Self::parse_figured_bass_text(&text) {
                return (chord_token.to_string(), Some(rows));
            }
        }

        (token.to_string(), None)
    }

    /// Recognise a `^`-marked **inversion** figure on a chord (`V^6`, `V^65`,
    /// `V^43`, …) and return how to realise it as a real inverted chord:
    /// `(chord_token, display, append_seventh, bass_thirds)`.
    ///
    /// - `chord_token` is what to actually parse (the root, plus `7` for the
    ///   seventh-chord figures), with any trailing `_dur` kept.
    /// - `display` is the original `root^figure` to show in the chart.
    /// - `bass_thirds` is how many thirds above the root the bass tone sits
    ///   (1 = 3rd, 2 = 5th, 3 = 7th), used to set the slash bass.
    ///
    /// Suspension figures (`4-3`) and anything unrecognised return `None` and
    /// fall through to [`extract_caret_figure`] (a plain figured-bass figure).
    pub(super) fn extract_caret_inversion(token: &str) -> Option<(String, String, bool, u8)> {
        let caret = token.find('^')?;
        let after = &token[caret + 1..];
        let fig_len = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        let figure = &after[..fig_len];
        let (append_seventh, bass_thirds) = match figure {
            "6" => (false, 1),
            "64" => (false, 2),
            "65" => (true, 1),
            "43" => (true, 2),
            "42" | "2" => (true, 3),
            _ => return None,
        };
        let root_part = &token[..caret];
        let trailing = &after[fig_len..]; // e.g. a `_4` duration
        // The figure states the chord exactly, so it must ignore chord memory:
        // append `7` for the seventh figures (explicit family), and prefix `!`
        // on the triad figures so a remembered seventh can't sneak in
        // (`V^65 V^6` → the `V^6` stays a triad).
        let chord_token = if append_seventh {
            format!("{root_part}7{trailing}")
        } else {
            format!("!{root_part}{trailing}")
        };
        let display = format!("{root_part}^{figure}");
        Some((chord_token, display, append_seventh, bass_thirds))
    }

    /// Pull an inline `^`-marked figured-bass / inversion figure off a chord
    /// token: `V^65`, `V^6`, `V^43`, `V^4-3`. The `^` keeps these distinct from
    /// a plain chord (`V6` is still an added-6th chord) and from the quoted
    /// `"4-3"` form. Returns the chord token (with the figure removed, any
    /// trailing duration preserved) and the parsed figured-bass rows. A `^` that
    /// introduces above-staff text (`^"…"`, its own token) or a non-figure is
    /// left untouched.
    pub(super) fn extract_caret_figure(token: &str) -> (String, Option<Vec<FiguredBassRow>>) {
        let Some(caret) = token.find('^') else {
            return (token.to_string(), None);
        };
        let after = &token[caret + 1..];
        if after.starts_with('"') {
            return (token.to_string(), None); // ^"text" is above-staff text
        }
        // The figure runs over figured-bass characters; anything past it (e.g. a
        // `_4` duration) stays on the chord token.
        let fig_len = after
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '#' | 'b' | 'n')))
            .unwrap_or(after.len());
        let figure = &after[..fig_len];
        let Some(rows) = Self::parse_figured_bass_text(figure) else {
            return (token.to_string(), None);
        };
        let chord_token = format!("{}{}", &token[..caret], &after[fig_len..]);
        (chord_token, Some(rows))
    }

    fn parse_figured_bass_rows(input: &str) -> Vec<FiguredBassRow> {
        let row_texts = if input.contains('/') || input.contains(',') {
            input
                .split(['/', ','])
                .map(str::trim)
                .filter(|row| !row.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        } else {
            split_figured_bass_words(input)
        };

        row_texts
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|row| !row.is_empty())
            .map(|row| {
                let (accidental, text) = Self::split_figured_bass_accidental(row);
                FiguredBassRow { accidental, text }
            })
            .collect()
    }

    fn parse_figured_bass_text(input: &str) -> Option<Vec<FiguredBassRow>> {
        let rows = Self::parse_figured_bass_rows(input);
        (!rows.is_empty()
            && rows
                .iter()
                .all(|row| looks_like_figured_bass_text(&row.text)))
        .then_some(rows)
    }

    fn split_figured_bass_accidental(row: &str) -> (String, String) {
        for accidental in ["bb", "##", "#", "b", "n", "\u{266e}"] {
            if let Some(text) = row.strip_prefix(accidental) {
                return (accidental.to_string(), text.to_string());
            }
        }
        (String::new(), row.to_string())
    }

    pub(super) fn push_figured_bass_at_current_beat(
        measure: &mut Measure,
        rows: &[FiguredBassRow],
        placement: Placement,
        current_measure_beats: f64,
    ) {
        let beat = current_measure_beats.floor().max(0.0) as u8 + 1;
        measure.figured_bass.push(FiguredBass {
            rows: rows.to_vec(),
            beat,
            placement,
            source_default_x: None,
            source_relative_x: None,
        });
    }
}

fn split_figured_bass_words(input: &str) -> Vec<String> {
    let parts = input.split_whitespace().collect::<Vec<_>>();
    if parts.len() > 1 {
        parts.into_iter().map(str::to_string).collect()
    } else if let Some(only) = parts.first() {
        split_compacted_figured_bass(only)
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        Vec::new()
    }
}

fn split_compacted_figured_bass(row: &str) -> Vec<&str> {
    let stripped = row
        .strip_prefix("bb")
        .or_else(|| row.strip_prefix("##"))
        .or_else(|| row.strip_prefix('#'))
        .or_else(|| row.strip_prefix('b'))
        .or_else(|| row.strip_prefix('n'))
        .unwrap_or(row);
    let accidental_len = row.len() - stripped.len();
    let chars = stripped.chars().collect::<Vec<_>>();
    if chars.len() == 6
        && chars[0].is_ascii_digit()
        && chars[1] == '-'
        && chars[2].is_ascii_digit()
        && chars[3].is_ascii_digit()
        && chars[4] == '-'
        && chars[5].is_ascii_digit()
    {
        vec![&row[..accidental_len + 3], &row[accidental_len + 3..]]
    } else {
        vec![row]
    }
}

fn looks_like_figured_bass_text(text: &str) -> bool {
    text.chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '#' | 'b' | 'n'))
        && text.chars().any(|ch| ch.is_ascii_digit())
}
