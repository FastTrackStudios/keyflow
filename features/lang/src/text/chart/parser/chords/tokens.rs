//! Token-level helpers: whitespace tokenizing with spans, chord case,
//! slash chords, the trailing `xN` repeat and auto-duration suffixes.

use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::helpers::RepeatCount;
use crate::text::parsing::TextSpan;

impl<'a> ChartParser<'a> {
    /// Normalize chord case - capitalize first letter for note names
    /// This allows "cmaj7", "dm7", "g7", "bbmaj7" to be parsed as "Cmaj7", "Dm7", "G7", "Bbmaj7"
    pub(in crate::text::chart::parser) fn normalize_chord_case(token: &str) -> String {
        if token.is_empty() {
            return token.to_string();
        }

        // A token starting with I/V/i/v is a Roman numeral — those letters are
        // never note names, so preserve case (lowercase = minor) regardless of
        // what follows. This keeps `v7`, `i:m7`, `viidim`, `IVmaj7` correct;
        // the old check only preserved case when the *second* char was also a
        // Roman letter, so single-numeral chords like `v7` lost their case.
        let first_char = token.chars().next().unwrap();
        if matches!(first_char, 'I' | 'V' | 'i' | 'v') {
            return token.to_string();
        }

        // If the first character is a lowercase letter (a-g), capitalize it
        if first_char.is_lowercase() && first_char.is_alphabetic() {
            let mut chars = token.chars();
            chars.next(); // skip first
            let mut result = first_char.to_uppercase().to_string();

            // Check if the second character is 'b' or '#' - if so, keep it as is
            // This handles "bbmaj7" -> "Bbmaj7" correctly
            result.push_str(chars.as_str());
            result
        } else {
            token.to_string()
        }
    }

    /// Split a slash chord into chord and bass parts
    /// Examples: "1/3" -> ("1", Some("3"))
    ///           "Cmaj7/E" -> ("Cmaj7", Some("E"))
    ///           "Fm/Ab_4" -> ("Fm_4", Some("Ab")) - preserves duration suffix on chord
    ///           "g//" -> ("g//", None)  (rhythm slashes, not a slash chord)
    ///           "g" -> ("g", None)
    pub(in crate::text::chart::parser) fn split_slash_chord(token: &str) -> (String, Option<&str>) {
        // Find the first slash
        if let Some(slash_pos) = token.find('/') {
            // Check if this is followed by another slash (rhythm notation)
            if slash_pos + 1 < token.len() {
                let after_slash = &token[slash_pos + 1..];
                if after_slash.starts_with('/')
                    || after_slash.starts_with('_')
                    || after_slash.starts_with('\'')
                    || after_slash.is_empty()
                {
                    // This is rhythm notation, not a slash chord
                    return (token.to_string(), None);
                }

                // Check if what follows the slash looks like a note/degree
                let bass_candidate = after_slash.chars().next().unwrap();
                if bass_candidate.is_alphabetic() || bass_candidate.is_ascii_digit() {
                    // This looks like a slash chord
                    // Extract just the bass note (stop at any rhythm notation)
                    let bass_end = after_slash
                        .find(['/', '_', '\''])
                        .unwrap_or(after_slash.len());

                    let chord_root = &token[..slash_pos];
                    let bass_note = &after_slash[..bass_end];
                    let rhythm_suffix = &after_slash[bass_end..];

                    // Reconstruct chord part with rhythm suffix attached
                    // e.g., "Fm/Ab_4" -> chord_root="Fm", bass="Ab", rhythm="_4" -> "Fm_4"
                    let chord_part = format!("{}{}", chord_root, rhythm_suffix);
                    return (chord_part, Some(bass_note));
                }
            }
        }

        (token.to_string(), None)
    }

    /// Extract repeat syntax from the end of a line
    /// Examples: "6 5 4 4 x4" -> ("6 5 4 4", 4)
    ///           "g c d" -> ("g c d", 1)
    pub(in crate::text::chart::parser) fn extract_repeat_syntax(line: &str) -> (&str, RepeatCount) {
        // Look for pattern like "x4" or "x^" at the end
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            return (line, RepeatCount::Fixed(1));
        }

        let last_token = tokens[tokens.len() - 1];

        // Check if last token matches x^ pattern (auto-repeat)
        if last_token == "x^" || last_token == "X^" {
            // Find where the last token starts in the original line
            if let Some(pos) = line.rfind(last_token) {
                let line_without_repeat = line[..pos].trim();
                return (line_without_repeat, RepeatCount::Auto);
            }
        }

        // Check if last token matches xN pattern (case insensitive)
        if (last_token.starts_with('x') || last_token.starts_with('X')) && last_token.len() > 1 {
            if let Ok(count) = last_token[1..].parse::<usize>() {
                if count > 0 {
                    // Find where the last token starts in the original line
                    if let Some(pos) = line.rfind(last_token) {
                        let line_without_repeat = line[..pos].trim();
                        return (line_without_repeat, RepeatCount::Fixed(count));
                    }
                }
            }
        }

        (line, RepeatCount::Fixed(1))
    }

    /// Strip auto-assigned duration suffix from a token.
    ///
    /// The auto-duration system adds suffixes like `_1`, `_2`, `_4`, `_8`, `_16`
    /// to tokens. This function removes those suffixes so we can tell if the
    /// user explicitly specified a duration or if it was auto-assigned.
    pub(in crate::text::chart::parser) fn strip_auto_duration_suffix(token: &str) -> String {
        // Auto-duration patterns: _1, _2, _4, _8, _16
        // These are whole, half, quarter, eighth, sixteenth notes
        for suffix in &["_16", "_8", "_4", "_2", "_1"] {
            if let Some(stripped) = token.strip_suffix(suffix) {
                return stripped.to_string();
            }
        }
        token.to_string()
    }

    /// Extract the root portion from the original token (before quality info and rhythm notation)
    /// Examples: "cmaj7" -> "C", "Dm7" -> "D", "g#m" -> "G#", "I" -> "I", "vi" -> "vi", "1" -> "1"
    /// Also handles rhythm notation: "e//" -> "E", "g_4" -> "G", "1///" -> "1"
    pub(in crate::text::chart::parser) fn extract_root_from_token(token: &str) -> String {
        if token.is_empty() {
            return token.to_string();
        }

        // First, strip any rhythm notation (slashes, underscores, etc.)
        // Find the first occurrence of /, _, or ' and take everything before it
        let chord_part = if let Some(slash_pos) = token.find('/') {
            &token[..slash_pos]
        } else if let Some(underscore_pos) = token.find('_') {
            &token[..underscore_pos]
        } else if let Some(apostrophe_pos) = token.find('\'') {
            &token[..apostrophe_pos]
        } else {
            token
        };

        if chord_part.is_empty() {
            return chord_part.to_string();
        }

        let mut chars = chord_part.chars();
        let first = chars.next().unwrap();

        // If it's a digit (scale degree 1-7), return just the digit
        if first.is_ascii_digit() {
            return first.to_string();
        }

        // If it's a Roman numeral (I-VII), extract the Roman numeral part (preserve case!)
        if first == 'I' || first == 'V' || first == 'i' || first == 'v' {
            let roman_part: String = chord_part
                .chars()
                .take_while(|c| *c == 'I' || *c == 'V' || *c == 'i' || *c == 'v')
                .collect();
            return roman_part;
        }

        // Otherwise, it's a note name - capitalize it and include accidental if present
        let first_upper = first.to_uppercase().to_string();
        let second = chars.next();
        if let Some(c) = second {
            if c == 'b' || c == '#' {
                return format!("{}{}", first_upper, c);
            }
        }

        first_upper
    }
}

/// Walk `line` and return a `TextSpan` for each whitespace-separated token,
/// in order. Spans are byte offsets relative to `line` (line/column = 0).
pub(super) fn tokenize_with_spans(line: &str) -> Vec<TextSpan> {
    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // Skip whitespace
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        let mut in_quote = false;
        let mut escaped = false;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if escaped {
                escaped = false;
                i += 1;
                continue;
            }
            if ch == '\\' && in_quote {
                escaped = true;
                i += 1;
                continue;
            }
            if ch == '"' {
                in_quote = !in_quote;
                i += 1;
                continue;
            }
            if !in_quote && ch.is_whitespace() {
                break;
            }
            i += 1;
        }
        spans.push(TextSpan::new(start, i - start));
    }
    spans
}

#[cfg(test)]
mod span_helper_tests {
    use super::tokenize_with_spans;

    #[test]
    fn tokenize_with_spans_matches_split_whitespace() {
        let line = "  G_2  C_2 |  Am ";
        let spans = tokenize_with_spans(line);
        let toks: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(spans.len(), toks.len());
        for (span, tok) in spans.iter().zip(toks.iter()) {
            assert_eq!(&line[span.as_range()], *tok);
            assert_eq!(span.len, tok.len());
        }
    }

    #[test]
    fn tokenize_with_spans_handles_empty_and_whitespace_only() {
        assert!(tokenize_with_spans("").is_empty());
        assert!(tokenize_with_spans("   \t  ").is_empty());
    }

    #[test]
    fn tokenize_with_spans_keeps_quoted_text_together() {
        let line = r#"| "Ac. Gtr. groove" A |"#;
        let spans = tokenize_with_spans(line);
        let toks: Vec<&str> = spans.iter().map(|span| &line[span.as_range()]).collect();
        assert_eq!(toks, vec!["|", r#""Ac. Gtr. groove""#, "A", "|"]);
    }
}
