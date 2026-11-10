//! Push / pull modifiers: the leading `'` anticipation and the trailing
//! delay marks on a chord token.

use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::helpers::PushPullModifier;
use crate::text::chord::LilySyntax;

impl<'a> ChartParser<'a> {
    /// Extract leading push modifiers for push notation
    /// Examples: "'C" -> (modifier with count=1, "C")
    ///           "''Em" -> (modifier with count=2, "Em")
    ///           "'tC" -> (modifier with count=1, triplet=true, "C")
    ///           "':5C" -> (modifier with count=1, tuplet=5, "C")
    ///           "'_4C" -> (modifier with duration=Quarter, "C")
    ///           "'_8tC" -> (modifier with duration=Eighth, triplet=true, "C")
    ///           "'_4.C" -> (modifier with duration=Quarter, dotted=true, "C")
    ///           "C" -> (empty modifier, "C")
    pub(in crate::text::chart::parser) fn extract_leading_push_modifiers(
        token: &str,
    ) -> (PushPullModifier, &str) {
        let mut modifier = PushPullModifier::default();
        let mut pos = 0;
        let bytes = token.as_bytes();

        // Count leading apostrophes
        while pos < bytes.len() && bytes[pos] == b'\'' {
            modifier.count += 1;
            pos += 1;
        }

        if modifier.count == 0 {
            return (modifier, token);
        }

        // Check for duration-based push/pull ('_4, '_8, '_8t, '_4.)
        if pos < bytes.len() && bytes[pos] == b'_' {
            let underscore_pos = pos;
            pos += 1;

            // Try to match valid durations: 32, 16, 8, 4, 2, 1 (longest first)
            // This handles ambiguous cases like '_44' (quarter note push on chord "4")
            let remaining = &token[pos..];
            let duration_candidates = ["32", "16", "8", "4", "2", "1"];

            for candidate in &duration_candidates {
                if remaining.starts_with(candidate) {
                    if let Some(duration) = LilySyntax::from_number(candidate) {
                        modifier.duration = Some(duration);
                        modifier.count = 1; // Duration-based push/pull ignores count
                        pos += candidate.len();

                        // Check for dotted (.) or triplet (t) modifier after duration
                        if pos < bytes.len() {
                            if bytes[pos] == b'.' {
                                modifier.duration_dotted = true;
                                pos += 1;
                            } else if bytes[pos] == b't' {
                                modifier.duration_triplet = true;
                                pos += 1;
                            }
                        }
                        return (modifier, &token[pos..]);
                    }
                }
            }

            // No valid duration found after underscore - revert and treat as regular push
            pos = underscore_pos;
        }

        // Check for triplet 't' marker
        if pos < bytes.len() && bytes[pos] == b't' {
            modifier.is_triplet = true;
            pos += 1;
        }
        // Check for tuplet ':N' marker
        else if pos < bytes.len() && bytes[pos] == b':' {
            pos += 1;
            // Parse the number
            let num_start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            if pos > num_start {
                if let Ok(n) = token[num_start..pos].parse::<u8>() {
                    if n >= 3 {
                        modifier.tuplet = Some(n);
                    }
                }
            }
        }

        (modifier, &token[pos..])
    }

    /// Extract trailing pull modifiers for pull notation
    /// Examples: "C'" -> ("C", modifier with count=1)
    ///           "Em''" -> ("Em", modifier with count=2)
    ///           "C't" -> ("C", modifier with count=1, triplet=true)
    ///           "C':5" -> ("C", modifier with count=1, tuplet=5)
    ///           "C'_4" -> ("C", modifier with duration=Quarter)
    ///           "C'_8t" -> ("C", modifier with duration=Eighth, triplet=true)
    ///           "C'_4." -> ("C", modifier with duration=Quarter, dotted=true)
    ///           "D'//  -> ("D//", modifier with count=1) - stops at rhythm notation
    ///           "C" -> ("C", empty modifier)
    ///
    /// Find where rhythm notation starts in a token, distinguishing slash chords from rhythm slashes.
    ///
    /// Rhythm notation is:
    /// - "//" (multiple slashes for duration continuation)
    /// - "/" at end of token (standalone continuation)
    /// - "/" followed by '_' or '\'' (rhythm with duration/pull)
    ///
    /// Slash chord (NOT rhythm) is:
    /// - "/" followed by a letter (the bass note, e.g., "Gm7/D")
    /// - "/" followed by a digit (Nashville number bass, e.g., "1/3")
    ///
    /// Returns the position where rhythm notation starts, or token.len() if no rhythm notation.
    fn find_rhythm_slash_position(token: &str) -> usize {
        let bytes = token.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'/' {
                // Check what follows the slash
                if i + 1 >= bytes.len() {
                    // Slash at end - this is rhythm notation
                    return i;
                }

                let next_char = bytes[i + 1];
                if next_char == b'/' {
                    // Another slash follows - this is rhythm notation (e.g., "//")
                    return i;
                } else if next_char == b'_' || next_char == b'\'' {
                    // Duration or pull modifier follows - this is rhythm notation
                    return i;
                } else if next_char.is_ascii_alphabetic() || next_char.is_ascii_digit() {
                    // A letter or digit follows - this is a slash chord bass note
                    // Skip past the bass note to continue looking for rhythm
                    i += 1;
                    // Skip the bass note (letters, digits, accidentals)
                    while i < bytes.len()
                        && (bytes[i].is_ascii_alphanumeric()
                            || bytes[i] == b'#'
                            || bytes[i] == b'b')
                    {
                        i += 1;
                    }
                    continue;
                } else {
                    // Something unexpected follows - treat as rhythm notation
                    return i;
                }
            }
            i += 1;
        }

        // No rhythm notation found
        token.len()
    }

    pub(in crate::text::chart::parser) fn extract_trailing_pull_modifiers(
        token: &str,
    ) -> (String, PushPullModifier) {
        // First, find where the chord part ends and rhythm notation begins
        // Rhythm notation: / (but NOT '_4 style duration which is part of pull notation)
        // For pull notation, we need to distinguish between:
        // - "C'_4" = pull by quarter note (the _4 is part of the pull modifier)
        // - "C_4" = chord with quarter note duration (the _4 is rhythm notation)
        // So we only split on '/' for standalone slash rhythm
        //
        // IMPORTANT: We must NOT split at slash chords like "Gm7/D" where "/D" is the bass note.
        // Rhythm notation is: "//" (multiple slashes), "/" at end, or "/" followed by '_' or '\''
        // Slash chord is: "/" followed by a letter (the bass note)
        let rhythm_start = Self::find_rhythm_slash_position(token);

        // Only extract modifiers between chord and rhythm
        let chord_and_modifiers = &token[..rhythm_start];
        let rhythm_part = &token[rhythm_start..];

        let mut modifier = PushPullModifier::default();

        // Work backwards from end of chord_and_modifiers
        let bytes = chord_and_modifiers.as_bytes();
        let mut end_pos = bytes.len();

        // Check for duration-based pull notation (e.g., "'_4", "'_8t", "'_4." at the end)
        // We need to find "'_" followed by a valid duration (32, 16, 8, 4, 2, 1)
        // and optionally followed by 't' or '.'
        // This handles ambiguous cases like "4'_4" (chord "4" pulled by quarter note)
        if let Some(apostrophe_underscore_pos) = chord_and_modifiers.rfind("'_") {
            let after_underscore = &chord_and_modifiers[apostrophe_underscore_pos + 2..];
            let duration_candidates = ["32", "16", "8", "4", "2", "1"];

            for candidate in &duration_candidates {
                if let Some(after_duration) = after_underscore.strip_prefix(candidate) {
                    if let Some(duration) = LilySyntax::from_number(candidate) {
                        // Check for optional dotted (.) or triplet (t) modifier
                        let (has_dot, has_triplet, suffix_len) = if after_duration.starts_with('.')
                        {
                            (true, false, 1)
                        } else if after_duration.starts_with('t') {
                            (false, true, 1)
                        } else {
                            (false, false, 0)
                        };

                        // Verify nothing else follows (or it's end of string)
                        if after_duration.len() == suffix_len {
                            modifier.duration = Some(duration);
                            modifier.count = 1;
                            modifier.duration_dotted = has_dot;
                            modifier.duration_triplet = has_triplet;
                            end_pos = apostrophe_underscore_pos;

                            let chord_only = &chord_and_modifiers[..end_pos];
                            let result = format!("{}{}", chord_only, rhythm_part);
                            return (result, modifier);
                        }
                    }
                }
            }
        }

        // Check for trailing tuplet number first (e.g., ":5" at the end)
        // Pattern: ':' followed by digits at the very end
        if end_pos >= 2 {
            let num_end = end_pos;
            let mut num_start = end_pos;

            // Find digits at end
            while num_start > 0 && bytes[num_start - 1].is_ascii_digit() {
                num_start -= 1;
            }

            // Check if preceded by ':'
            if num_start > 0 && num_start < num_end && bytes[num_start - 1] == b':' {
                if let Ok(n) = chord_and_modifiers[num_start..num_end].parse::<u8>() {
                    if n >= 3 {
                        modifier.tuplet = Some(n);
                        end_pos = num_start - 1; // Position before ':'
                    }
                }
            }
        }

        // Check for trailing 't' marker
        if end_pos > 0 && bytes[end_pos - 1] == b't' {
            // Make sure it's not part of a chord name - check if preceded by apostrophe
            if end_pos > 1 && bytes[end_pos - 2] == b'\'' {
                modifier.is_triplet = true;
                end_pos -= 1;
            }
        }

        // Count trailing apostrophes
        while end_pos > 0 && bytes[end_pos - 1] == b'\'' {
            modifier.count += 1;
            end_pos -= 1;
        }

        if modifier.count > 0 {
            // Remove modifiers but keep rhythm
            let chord_only = &chord_and_modifiers[..end_pos];
            let result = format!("{}{}", chord_only, rhythm_part);
            (result, modifier)
        } else {
            (token.to_string(), modifier)
        }
    }
}
