//! `()` rhythm groups, rewritten into explicit per-chord lily durations
//! (`(C G)` -> `C_2 G_2`).

use crate::text::time::TimeSignature;

pub(super) fn parse_lily_duration_beats(
    token: &str,
    time_sig: TimeSignature,
) -> Option<(f64, bool)> {
    let dotted = token.ends_with('.');
    let base = token.trim_end_matches('.');
    let note_denominator = match base {
        "1" => 1.0,
        "2" => 2.0,
        "4" => 4.0,
        "8" => 8.0,
        "16" => 16.0,
        "32" => 32.0,
        _ => return None,
    };
    let beats = f64::from(time_sig.denominator) / note_denominator;
    Some((if dotted { beats * 1.5 } else { beats }, dotted))
}

/// Inverse of [`parse_lily_duration_beats`], extended with triplets: find the
/// lily duration token whose value equals `beats` in the given meter, and
/// return its suffix including the leading underscore (e.g. `"_2"`, `"_4."`,
/// `"_8t"`). Searches plain, dotted, triplet, then dotted-triplet forms of each
/// note value, longest note first, so the canonical token wins. Returns `None`
/// when no notatable note value matches (e.g. a quintuplet division). Used by
/// [`expand_chord_groups`] to rewrite `()` rhythm groups.
pub(super) fn beats_to_lily_suffix(beats: f64, time_sig: TimeSignature) -> Option<String> {
    const EPS: f64 = 1e-6;
    let denom = f64::from(time_sig.denominator);
    for value in [1u32, 2, 4, 8, 16, 32] {
        let base = denom / f64::from(value);
        // (beats for this form, suffix after the number)
        let candidates = [
            (base, ""),
            (base * 1.5, "."),
            (base * 2.0 / 3.0, "t"),
            (base * 1.5 * 2.0 / 3.0, "t."),
        ];
        for (cand, suffix) in candidates {
            if (cand - beats).abs() < EPS {
                return Some(format!("_{value}{suffix}"));
            }
        }
    }
    None
}

/// Resolve a `()` chord group's *target duration* in beats, starting at byte
/// index `after_paren` (the position just past the closing `)`). The target is,
/// in priority order:
///
/// 1. an attached lily suffix — `)_4`, `)_8t`, `)_2.`
/// 2. a slash run — `)//`, `) //`, `)/.` (each slash = one beat, `.` dots it)
/// 3. otherwise one full measure (`beats_per_measure`).
///
/// Returns the target beats and the byte index where parsing should resume
/// (past any consumed suffix/slashes). A slash run immediately followed by an
/// alphanumeric/`#` is a floating slash-bass (`/E`, `/3`), not a target, and is
/// left for the main parser.
fn group_target_beats(
    line: &str,
    after_paren: usize,
    beats_per_measure: f64,
    time_sig: TimeSignature,
) -> Result<(f64, usize), String> {
    let bytes = line.as_bytes();

    // 1. Attached lily suffix: `_4`, `_8t`, `_2.`
    if bytes.get(after_paren) == Some(&b'_') {
        let num_start = after_paren + 1;
        let mut k = num_start;
        while k < bytes.len() && bytes[k].is_ascii_digit() {
            k += 1;
        }
        let value: f64 = line[num_start..k]
            .parse()
            .map_err(|_| "Expected a duration after '_' on a chord group".to_string())?;
        let mut triplet = false;
        let mut dotted = false;
        if bytes.get(k) == Some(&b't') {
            triplet = true;
            k += 1;
        }
        if bytes.get(k) == Some(&b'.') {
            dotted = true;
            k += 1;
        }
        let mut beats = f64::from(time_sig.denominator) / value;
        if triplet {
            beats *= 2.0 / 3.0;
        }
        if dotted {
            beats *= 1.5;
        }
        return Ok((beats, k));
    }

    // 2. Slash run (allow whitespace between `)` and the slashes).
    let mut k = after_paren;
    while matches!(bytes.get(k), Some(&b' ') | Some(&b'\t')) {
        k += 1;
    }
    if bytes.get(k) == Some(&b'/') {
        let mut slashes = 0u32;
        let mut dotted = false;
        while k < bytes.len() {
            match bytes[k] {
                b'/' => {
                    slashes += 1;
                    k += 1;
                }
                b'.' => {
                    dotted = true;
                    k += 1;
                }
                _ => break,
            }
        }
        let next_is_token_char = bytes
            .get(k)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'#');
        if slashes > 0 && !next_is_token_char {
            let mut beats = f64::from(slashes);
            if dotted {
                beats *= 1.5;
            }
            return Ok((beats, k));
        }
    }

    // 3. Default: one full measure. Consume nothing.
    Ok((beats_per_measure, after_paren))
}

/// Expand `()` rhythm groups into explicit per-chord lily durations.
///
/// A `(a b c …)` group has a *target duration* (see [`group_target_beats`])
/// divided equally among its N inner chords. Each chord is rewritten with the
/// lily token for `target / N`, so the existing chord-line parser renders the
/// division — including triplets — with no further special-casing:
/// ```text
///   (C G)       -> C_2 G_2            two half notes
///   (D Em G)    -> D_2t Em_2t G_2t    half-note triplet over the bar
///   (D Em)//    -> D_4 Em_4           group = 2 beats, one each
///   (D Em G)_4  -> D_8t Em_8t G_8t    eighth-note triplet over a quarter
/// ```
/// Quoted text and `m{…}` melody blocks are copied verbatim, so their
/// parentheses (e.g. melody octaves) are left untouched. Note: byte offsets of
/// expanded lines no longer map 1:1 to the source, so source spans on a line
/// that contains a group are approximate.
pub(super) fn expand_chord_groups(line: &str, time_sig: TimeSignature) -> Result<String, String> {
    if !line.contains('(') {
        return Ok(line.to_string());
    }
    let beats_per_measure = f64::from(time_sig.numerator);
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = line[i..].chars().next().unwrap();

        // Copy quoted strings verbatim.
        if ch == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // include the closing quote
            }
            out.push_str(&line[start..i]);
            continue;
        }

        // Copy `m{…}` melody blocks verbatim (track brace depth).
        if ch == 'm' && bytes.get(i + 1) == Some(&b'{') {
            let start = i;
            i += 2;
            let mut depth = 1u32;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            out.push_str(&line[start..i]);
            continue;
        }

        // A `(` attached to the token before it is part of that chord —
        // `5(add4)`, `C7(b9)` — not a rhythm group. Read as a group it
        // became `add4` with a lily length, and that length carried on to
        // the chords after it.
        let attached = line[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '#' | '+' | '^' | '°' | 'ø'));
        if ch == '(' && attached {
            let close = line[i..].find(')').map_or(bytes.len(), |at| i + at + 1);
            out.push_str(&line[i..close]);
            i = close;
            continue;
        }

        if ch == '(' {
            let inner_start = i + 1;
            let mut j = inner_start;
            let mut depth = 1u32;
            while j < bytes.len() {
                match bytes[j] {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if depth != 0 {
                return Err("Unclosed '(' in chord rhythm group".to_string());
            }
            let elems: Vec<&str> = line[inner_start..j].split_whitespace().collect();
            if elems.is_empty() {
                return Err("Empty '()' chord rhythm group".to_string());
            }
            let (target_beats, resume) =
                group_target_beats(line, j + 1, beats_per_measure, time_sig)?;
            let each = target_beats / elems.len() as f64;
            let suffix = beats_to_lily_suffix(each, time_sig).ok_or_else(|| {
                format!(
                    "Chord group of {} chords cannot be evenly notated over {} beat(s); \
                     supported divisions are powers of two and triplets",
                    elems.len(),
                    target_beats
                )
            })?;
            for (idx, e) in elems.iter().enumerate() {
                if idx > 0 {
                    out.push(' ');
                }
                out.push_str(e);
                out.push_str(&suffix);
            }
            i = resume;
            continue;
        }

        out.push(ch);
        i += ch.len_utf8();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::time::TimeSignatureExt;

    // --- `()` rhythm grouping ---------------------------------------------

    #[test]
    fn expand_chord_groups_rewrites_to_lily_durations() {
        let c44 = TimeSignature::common_time();
        // Default target is one measure, split equally.
        assert_eq!(expand_chord_groups("(C G)", c44).unwrap(), "C_2 G_2");
        // Three over a measure → half-note triplet.
        assert_eq!(
            expand_chord_groups("(D Em G)", c44).unwrap(),
            "D_2t Em_2t G_2t"
        );
        // Slash run sets the target: two beats, one each.
        assert_eq!(expand_chord_groups("(D Em)//", c44).unwrap(), "D_4 Em_4");
        assert_eq!(expand_chord_groups("(D Em) //", c44).unwrap(), "D_4 Em_4");
        // Attached lily target: a quarter split three ways → eighth triplet.
        assert_eq!(
            expand_chord_groups("(D Em G)_4", c44).unwrap(),
            "D_8t Em_8t G_8t"
        );
        // A group is just one element among bare chords.
        assert_eq!(
            expand_chord_groups("G C (Em D) G", c44).unwrap(),
            "G C Em_2 D_2 G"
        );
        // Chord descriptors inside a group are preserved.
        assert_eq!(expand_chord_groups("(Dm7 G7)", c44).unwrap(), "Dm7_2 G7_2");
    }

    #[test]
    fn expand_chord_groups_respects_meter() {
        // In 6/8 a measure is six eighth-beats; two chords → dotted quarters.
        let c68 = TimeSignature::new(6, 8);
        assert_eq!(expand_chord_groups("(A B)", c68).unwrap(), "A_4. B_4.");
    }

    #[test]
    fn expand_chord_groups_leaves_non_groups_untouched() {
        let c44 = TimeSignature::common_time();
        // No parens → identity (so spans/tokens are unchanged for normal lines).
        assert_eq!(expand_chord_groups("C F G Am", c44).unwrap(), "C F G Am");
        // Parens inside quotes and m{…} melody blocks are left alone.
        assert_eq!(
            expand_chord_groups(r#"C "a (b) c" G"#, c44).unwrap(),
            r#"C "a (b) c" G"#
        );
        // A `( )` inside a melody block is left alone — chord-group expansion
        // never reaches into `m{…}`.
        assert_eq!(
            expand_chord_groups("C m{ (D E) } G", c44).unwrap(),
            "C m{ (D E) } G"
        );
        // A `(` attached to a chord is its addition, not a group; one after
        // a barline or another group still is a group.
        assert_eq!(
            expand_chord_groups("5(add4) // 1", c44).unwrap(),
            "5(add4) // 1"
        );
        assert_eq!(expand_chord_groups("C7(b9) F", c44).unwrap(), "C7(b9) F");
        assert_eq!(expand_chord_groups("|(C G)|", c44).unwrap(), "|C_2 G_2|");
    }

    #[test]
    fn expand_chord_groups_reports_errors() {
        let c44 = TimeSignature::common_time();
        assert!(expand_chord_groups("(C G", c44).is_err());
        assert!(expand_chord_groups("()", c44).is_err());
        // Five over a measure has no power-of-two/triplet notation → error.
        assert!(expand_chord_groups("(C D E F G)", c44).is_err());
    }

    #[test]
    fn beats_to_lily_suffix_picks_canonical_token() {
        let c44 = TimeSignature::common_time();
        assert_eq!(beats_to_lily_suffix(2.0, c44).as_deref(), Some("_2")); // half
        assert_eq!(beats_to_lily_suffix(1.0, c44).as_deref(), Some("_4")); // quarter
        assert_eq!(beats_to_lily_suffix(1.5, c44).as_deref(), Some("_4.")); // dotted quarter
        assert_eq!(beats_to_lily_suffix(4.0 / 3.0, c44).as_deref(), Some("_2t"));
        assert_eq!(beats_to_lily_suffix(1.0 / 3.0, c44).as_deref(), Some("_8t"));
        assert_eq!(beats_to_lily_suffix(0.8, c44), None); // quintuplet — unsupported
    }
}
