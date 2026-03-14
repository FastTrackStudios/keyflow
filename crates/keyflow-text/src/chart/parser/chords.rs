//! Chord parsing for charts
//!
//! Handles parsing of chord lines, individual chord tokens, and related
//! functionality including duration calculation, slash chords, and push/pull notation.

use super::helpers::{PushPullModifier, RepeatCount};
use super::ChartParser;
use crate::chart::cues::TextCue;
use crate::chart::dynamics::DynamicMarking;
use crate::chart::melody::Melody;
use crate::chart::types::{
    ChordInstance, KeyChange, Measure, RestInstance, RhythmElement, SpaceInstance,
};
use crate::chord::{ChordRhythm, LilySyntax};
use crate::key::Key;
use crate::parsing::{Lexer, TextSpan};
use crate::primitives::RootNotation;
use crate::sections::SectionType;
use crate::time::{
    AbsolutePosition, MusicalDuration, MusicalPosition, MusicalPositionExt, TimeSignature,
    TimeSignatureExt,
};

// region:    --- Token Helpers

impl<'a> ChartParser<'a> {
    /// Normalize chord case - capitalize first letter for note names
    /// This allows "cmaj7", "dm7", "g7", "bbmaj7" to be parsed as "Cmaj7", "Dm7", "G7", "Bbmaj7"
    pub(super) fn normalize_chord_case(token: &str) -> String {
        if token.is_empty() {
            return token.to_string();
        }

        // Check if this is a Roman numeral (starts with I, V, i, or v)
        // Roman numerals should preserve their case
        let first_char = token.chars().next().unwrap();
        if first_char == 'I' || first_char == 'V' || first_char == 'i' || first_char == 'v' {
            // Check if the second character is also a Roman numeral character
            if let Some(second_char) = token.chars().nth(1) {
                if second_char == 'I'
                    || second_char == 'V'
                    || second_char == 'i'
                    || second_char == 'v'
                    || second_char == '/'
                    || second_char == '_'
                    || second_char == '\''
                {
                    // This is a Roman numeral - preserve case
                    return token.to_string();
                }
            } else {
                // Single character I, V, i, or v - likely a Roman numeral
                return token.to_string();
            }
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

    /// Extract leading push modifiers for push notation
    /// Examples: "'C" -> (modifier with count=1, "C")
    ///           "''Em" -> (modifier with count=2, "Em")
    ///           "'tC" -> (modifier with count=1, triplet=true, "C")
    ///           "':5C" -> (modifier with count=1, tuplet=5, "C")
    ///           "'_4C" -> (modifier with duration=Quarter, "C")
    ///           "'_8tC" -> (modifier with duration=Eighth, triplet=true, "C")
    ///           "'_4.C" -> (modifier with duration=Quarter, dotted=true, "C")
    ///           "C" -> (empty modifier, "C")
    pub(super) fn extract_leading_push_modifiers(token: &str) -> (PushPullModifier, &str) {
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

    pub(super) fn extract_trailing_pull_modifiers(token: &str) -> (String, PushPullModifier) {
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

    /// Split a slash chord into chord and bass parts
    /// Examples: "1/3" -> ("1", Some("3"))
    ///           "Cmaj7/E" -> ("Cmaj7", Some("E"))
    ///           "Fm/Ab_4" -> ("Fm_4", Some("Ab")) - preserves duration suffix on chord
    ///           "g//" -> ("g//", None)  (rhythm slashes, not a slash chord)
    ///           "g" -> ("g", None)
    pub(super) fn split_slash_chord(token: &str) -> (String, Option<&str>) {
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

    /// Apply automatic durations to chords between measure separators
    /// If chords are between | separators, split the measure evenly
    /// Examples:
    ///   "| G C |" → "| G_2 C_2 |" (2 chords = half notes each)
    ///   "G C | D" → "G_2 C_2 | D_1" (2 chords before | = half notes, 1 after = whole)
    ///   "G C E D | A" → "G_4 C_4 E_4 D_4 | A_1" (4 chords before | = quarter notes, 1 after = whole)
    pub(super) fn apply_auto_durations_between_separators(
        line: &str,
        beats_per_measure: f64,
    ) -> String {
        // Check if line has any separators - if not, return as-is
        if !line.contains('|') {
            return line.to_string();
        }

        // Split by | to get segments
        let segments: Vec<&str> = line.split('|').collect();
        let mut result = String::new();

        for (i, segment) in segments.iter().enumerate() {
            let segment = segment.trim();

            // Handle empty segments (multiple | in a row or at start/end)
            if segment.is_empty() {
                // Add separator if not at the very end
                if i < segments.len() - 1 {
                    if !result.is_empty() {
                        result.push(' ');
                    }
                    result.push('|');
                }
                continue;
            }

            // Count chords in this segment (exclude commands, cues, dot repeats, etc.)
            // Count ALL chords, even those with explicit durations, to calculate segment duration
            let tokens: Vec<&str> = segment.split_whitespace().collect();

            // Check if segment has standalone slashes (e.g., "/", "//", "///", "////")
            // If so, don't apply auto-duration - the slashes provide duration info
            let has_standalone_slashes = tokens
                .iter()
                .any(|t| t.chars().all(|c| c == '/') && !t.is_empty());

            let chord_count = tokens
                .iter()
                .filter(|t| {
                    // Count as chord if it's not a command, cue, or other special token
                    // Dot repeats ARE counted - they occupy time in the measure
                    !t.starts_with('/') && !t.starts_with('@') && !t.starts_with('"')
                })
                .count();

            // Count chords WITHOUT explicit durations (these need auto-duration)
            // If segment has standalone slashes, skip auto-duration entirely
            let chords_needing_duration = if has_standalone_slashes {
                0 // Don't apply auto-duration when slashes are present
            } else {
                tokens
                    .iter()
                    .filter(|t| {
                        !t.starts_with('/')
                            && !t.starts_with('@')
                            && !t.starts_with('"')
                            && !t.contains('_')
                            && **t != "." // Dot repeats handle their own duration
                    })
                    .count()
            };

            // Calculate duration per chord
            // If all chords have explicit durations, use whole note as default
            // Otherwise, split the measure evenly among all chords (including those with explicit durations)
            let duration_per_chord = if chord_count > 0 {
                beats_per_measure / chord_count as f64
            } else {
                beats_per_measure
            };

            // Apply auto-duration if there are chords that need it.
            // For multi-chord segments (including the last one), chords should share
            // the measure evenly. A single chord in the last segment takes a full measure.
            let should_apply_auto_duration = chords_needing_duration > 0;

            // Convert beats to LilySyntax duration
            // beats_per_measure = 4 in 4/4, so:
            // 4 beats = Whole (1)
            // 2 beats = Half (2)
            // 1 beat = Quarter (4)
            // 0.5 beats = Eighth (8)
            let lily_duration = if (duration_per_chord - beats_per_measure).abs() < 0.001 {
                "1" // Whole note
            } else if (duration_per_chord - beats_per_measure / 2.0).abs() < 0.001 {
                "2" // Half note
            } else if (duration_per_chord - beats_per_measure / 4.0).abs() < 0.001 {
                "4" // Quarter note
            } else if (duration_per_chord - beats_per_measure / 8.0).abs() < 0.001 {
                "8" // Eighth note
            } else if (duration_per_chord - beats_per_measure / 16.0).abs() < 0.001 {
                "16" // Sixteenth note
            } else {
                // Default to whole note if we can't match exactly
                "1"
            };

            // Rebuild segment with durations
            let mut segment_result = String::new();
            for token in &tokens {
                if !segment_result.is_empty() {
                    segment_result.push(' ');
                }

                // Check if this is a chord (not a command, cue, dot repeat, etc.)
                let is_dot_repeat = *token == ".";
                if !token.starts_with('/')
                    && !token.starts_with('@')
                    && !token.starts_with('"')
                    && !is_dot_repeat
                {
                    // Check if token already has a duration
                    if token.contains('_') {
                        // Already has duration, keep as is
                        segment_result.push_str(token);
                    } else if should_apply_auto_duration {
                        // Add automatic duration
                        segment_result.push_str(token);
                        segment_result.push('_');
                        segment_result.push_str(lily_duration);
                    } else {
                        // No chords need duration, keep as is
                        segment_result.push_str(token);
                    }
                } else {
                    // Keep non-chord tokens (commands, cues, dot repeats) as is
                    segment_result.push_str(token);
                }
            }

            if !result.is_empty() && !segment_result.is_empty() {
                result.push(' ');
            }
            result.push_str(&segment_result);

            // Add separator if not last segment
            if i < segments.len() - 1 {
                result.push(' ');
                result.push('|');
            }
        }

        result
    }

    /// Extract repeat syntax from the end of a line
    /// Examples: "6 5 4 4 x4" -> ("6 5 4 4", 4)
    ///           "g c d" -> ("g c d", 1)
    pub(super) fn extract_repeat_syntax(line: &str) -> (&str, RepeatCount) {
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
    pub(super) fn strip_auto_duration_suffix(token: &str) -> String {
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
    pub(super) fn extract_root_from_token(token: &str) -> String {
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

// endregion: --- Token Helpers

// region:    --- Chord Line Parsing

impl<'a> ChartParser<'a> {
    /// Parse section content lines into measures
    pub(super) fn parse_section_measures(
        &mut self,
        lines: &[&str],
        section_type: &SectionType,
        section_measure_count: Option<usize>,
    ) -> Result<Vec<Measure>, String> {
        let mut measures: Vec<Measure> = Vec::new();
        let mut pending_cues: Vec<TextCue> = Vec::new();
        let mut pending_dynamics: Vec<DynamicMarking> = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            // Check for melody variable definition (e.g., "mainRiff = m{ C_8 D_8 E_4 }")
            if trimmed.contains("= m{") || trimmed.starts_with("m{") {
                match Melody::parse_block(trimmed) {
                    Ok((name, melody)) => {
                        if let Some(var_name) = name {
                            // Store as a variable
                            self.melody_variables.set(var_name, melody);
                        }
                        // Note: Inline melodies without variable names on their own line
                        // are currently not attached to any measure - they're just stored
                        // as a definition. Use inline syntax within chord lines to attach
                        // melodies to specific measures.
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse melody block '{}': {}", trimmed, e);
                    }
                }
                continue;
            }

            // Check if this is a text cue line
            if trimmed.starts_with('@') {
                // Parse text cue - always keep it pending for the next chord line
                match TextCue::parse(line) {
                    Ok(cue) => {
                        pending_cues.push(cue);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse text cue '{}': {}", line, e);
                    }
                }
            } else if trimmed.starts_with('<') && trimmed.contains('>') {
                // Check if this is a standalone dynamic marking line
                // Could be just "<Build>" or "<Build>:3" on its own line
                match DynamicMarking::parse(trimmed) {
                    Ok(dynamic) => {
                        pending_dynamics.push(dynamic);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse dynamic marking '{}': {}", line, e);
                    }
                }
            } else {
                // Parse chords from line
                // Pass section measure_count for x^ calculation
                let mut line_measures =
                    self.parse_chord_line(line, section_type, section_measure_count)?;

                // If we have pending cues, attach them to the first new measure
                if !pending_cues.is_empty() && !line_measures.is_empty() {
                    line_measures[0].text_cues.append(&mut pending_cues);
                }

                // If we have pending dynamics, attach them to the first new measure
                if !pending_dynamics.is_empty() && !line_measures.is_empty() {
                    line_measures[0].dynamics.append(&mut pending_dynamics);
                }

                measures.extend(line_measures);
            }
        }

        Ok(measures)
    }

    /// Parse a line of chords into measures
    #[allow(clippy::too_many_lines)]
    pub(super) fn parse_chord_line(
        &mut self,
        line: &str,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
    ) -> Result<Vec<Measure>, String> {
        use crate::chart::commands::Command;
        use crate::chart::types::TempoChange;

        let mut time_sig = self.time_signature.unwrap_or(TimeSignature::common_time());
        let mut beats_per_measure = time_sig.numerator as f64;

        // Check for repeat syntax at the end of the line (e.g., "6 5 4 4 x4")
        let (line_to_parse, repeat_count) = Self::extract_repeat_syntax(line);

        if line_to_parse.contains("<<") {
            return self.parse_parallel_chord_line(
                line_to_parse,
                repeat_count,
                section_type,
                section_measure_count,
            );
        }

        let line_to_parse = Self::normalize_parallel_container_syntax(line_to_parse);

        // Preprocess: Calculate automatic durations for chords between measure separators
        // If chords are between | separators, split the measure evenly
        // e.g., "| G C |" → "G_2 C_2", "G C | D" → "G_2 C_2 D_1"
        let line_with_auto_durations =
            Self::apply_auto_durations_between_separators(&line_to_parse, beats_per_measure);

        let tokens_str: Vec<&str> = line_with_auto_durations.split_whitespace().collect();
        let mut measures: Vec<Measure> = Vec::new();
        let mut current_measure = Measure::new();
        let mut current_measure_beats = 0.0;
        let mut pending_cue: Option<TextCue> = None;
        let mut pending_dynamic: Option<DynamicMarking> = None;
        let mut just_processed_separator = false; // Track if we just processed a | separator
        let mut measure_was_created_by_separator = false; // Track if current measure was created by |
        let mut in_melody_block = false; // Track if we're inside a m{...} block
        let mut last_chord: Option<ChordInstance> = None; // Track last chord for dot repeat
        let mut measure_has_slash_rhythm = false; // Track if current measure has chords with slash rhythm
        let mut last_token_was_slash = false; // Track for slash accumulation (consecutive slashes accumulate)

        for (token_idx, token_str) in tokens_str.iter().enumerate() {
            // Check for command (e.g., "/fermata", "/accent")
            // Commands are applied to the PREVIOUS chord
            if token_str.starts_with('/') {
                if let Some(cmd) = Command::parse_slash(token_str) {
                    // Apply command to the last chord in rhythm_elements (source of truth)
                    // and also to chords for backward compatibility during parsing
                    let applied = if let Some(RhythmElement::Chord(c)) =
                        current_measure.rhythm_elements.last_mut()
                    {
                        c.commands.push(cmd.clone());
                        true
                    } else {
                        false
                    };

                    // Also apply to chords vec for backward compatibility
                    if let Some(last_chord) = current_measure.chords.last_mut() {
                        last_chord.commands.push(cmd.clone());
                    }

                    // If current measure has no chord, try previous measure
                    if !applied && !measures.is_empty() {
                        if let Some(last_measure) = measures.last_mut() {
                            if let Some(RhythmElement::Chord(c)) =
                                last_measure.rhythm_elements.last_mut()
                            {
                                c.commands.push(cmd.clone());
                            }
                            if let Some(last_chord) = last_measure.chords.last_mut() {
                                last_chord.commands.push(cmd);
                            }
                        }
                    }
                    continue;
                }

                // Check for standalone slash duration notation (e.g., "//", "///", "////")
                // This allows syntax like "Ab9' //" where the slashes are separated by a space
                //
                // NEW BEHAVIOR: Single "/" between chords represents a continuation beat,
                // not a duration modifier. This allows "Cm/Eb / 'Eb //" to work correctly:
                // - Cm/Eb takes 1 beat
                // - / adds 1 beat of continuation (beat 2)
                // - 'Eb is pushed, creating triplet on beat 2
                // - // adds 2 more beats (beats 3-4)
                //
                // DOTTED SLASHES: /. = 1.5 beats, //. = 2.5 beats, etc.
                // The dot adds 50% to the slash duration.

                // First check for dotted slash notation (e.g., "/.", "//.", "///.")
                let is_dotted_slash = token_str.ends_with('.')
                    && token_str.len() >= 2
                    && token_str[..token_str.len() - 1].chars().all(|c| c == '/');

                if is_dotted_slash {
                    let slash_count = token_str.len() - 1; // Exclude the dot
                    if slash_count > 0 {
                        // Dotted slashes: each slash is 1 beat, dot adds 50%
                        // /. = 1.5 beats (dotted quarter)
                        // //. = 2.5 beats (half + dotted quarter... approximated as 2.5)
                        // Actually: interpret as (slashes * 1 beat) * 1.5 for the dotted effect
                        // So /. = 1 * 1.5 = 1.5 beats
                        // //. = 2 * 1.5 = 3.0 beats? Or is it 2 + 0.5 = 2.5?
                        // Let's use: /. = dotted quarter (1.5), //. = dotted half (3.0)
                        let dotted_duration = slash_count as f64 * 1.5;

                        // Convert to Lily rhythm for proper representation
                        let _lily_duration = match slash_count {
                            1 => LilySyntax::Quarter,   // /. = dotted quarter
                            2 => LilySyntax::Half,      // //. = dotted half
                            3 | 4 => LilySyntax::Whole, // ///. or ////. = dotted whole
                            _ => LilySyntax::Quarter,
                        };

                        // Use dotted slash notation
                        let dotted_slash_rhythm = ChordRhythm::Slashes {
                            count: slash_count as u8,
                            dotted: true,
                            tied: false,
                        };

                        let applied = if let Some(last_chord) = current_measure.chords.last_mut() {
                            last_chord.rhythm = dotted_slash_rhythm.clone();
                            last_chord.duration =
                                MusicalDuration::from_beats(dotted_duration, time_sig);

                            // Also update the chord in rhythm_elements
                            for elem in current_measure.rhythm_elements.iter_mut().rev() {
                                if let RhythmElement::Chord(c) = elem {
                                    c.rhythm = dotted_slash_rhythm.clone();
                                    c.duration =
                                        MusicalDuration::from_beats(dotted_duration, time_sig);
                                    break;
                                }
                            }
                            true
                        } else if !measures.is_empty() {
                            // Current measure is empty - apply to previous measure's last chord
                            // This happens when the chord auto-completed a measure (e.g., "C /.")
                            if let Some(last_measure) = measures.last_mut() {
                                if let Some(last_chord) = last_measure.chords.last_mut() {
                                    last_chord.rhythm = dotted_slash_rhythm.clone();
                                    last_chord.duration =
                                        MusicalDuration::from_beats(dotted_duration, time_sig);

                                    // Also update in rhythm_elements
                                    for elem in last_measure.rhythm_elements.iter_mut().rev() {
                                        if let RhythmElement::Chord(c) = elem {
                                            c.rhythm = dotted_slash_rhythm.clone();
                                            c.duration = MusicalDuration::from_beats(
                                                dotted_duration,
                                                time_sig,
                                            );
                                            break;
                                        }
                                    }
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if applied {
                            measure_has_slash_rhythm = true;
                            last_token_was_slash = true;
                            continue;
                        }
                    }
                }

                if token_str.chars().all(|c| c == '/') {
                    let slash_count = token_str.len() as u8;
                    if slash_count > 0 {
                        // Check if last chord is tied - if so, skip this block
                        // and let the tie handling code (below) process this slash
                        let last_chord_is_tied = current_measure
                            .chords
                            .last()
                            .is_some_and(|c| c.rhythm.is_tied())
                            || (!measures.is_empty()
                                && measures
                                    .last()
                                    .and_then(|m| m.chords.last())
                                    .is_some_and(|c| c.rhythm.is_tied()));

                        if last_chord_is_tied {
                            // Fall through to tie handling code below
                        } else {
                            // Apply slash rhythm to the last chord
                            // Within same measure: slashes SET the duration (explicit slashes override auto-fill)
                            // Across bar line (after |): slashes ADD to existing duration for cross-barline chords
                            // "Cm/Eb /" means Cm/Eb for 1 beat
                            // "Eb ///" means Eb for 3 beats
                            // "Abmaj9 //// | //" means Abmaj9 for 6 beats (spans 1.5 bars)
                            let applied = if let Some(last_chord) =
                                current_measure.chords.last_mut()
                            {
                                // Only accumulate if previous token was also a slash (consecutive slashes).
                                // This prevents accumulating on top of auto-filled slash rhythm.
                                // "C // G //" - when we see first "/" after C, it should SET to 1 (not accumulate).
                                // When we see second "/" after first, it should accumulate to 2.
                                let new_count = if last_token_was_slash {
                                    if let ChordRhythm::Slashes {
                                        count: existing, ..
                                    } = &last_chord.rhythm
                                    {
                                        existing + slash_count
                                    } else {
                                        slash_count
                                    }
                                } else {
                                    // First slash after a chord - SET (override auto-fill)
                                    slash_count
                                };
                                last_chord.rhythm = ChordRhythm::slashes(new_count);
                                last_chord.duration =
                                    MusicalDuration::from_beats(f64::from(new_count), time_sig);

                                // Also update the chord in rhythm_elements
                                for elem in current_measure.rhythm_elements.iter_mut().rev() {
                                    if let RhythmElement::Chord(c) = elem {
                                        c.rhythm = ChordRhythm::slashes(new_count);
                                        c.duration = MusicalDuration::from_beats(
                                            f64::from(new_count),
                                            time_sig,
                                        );
                                        break;
                                    }
                                }
                                true
                            } else if !measures.is_empty() {
                                // Current measure is empty - slashes apply to previous measure's chord
                                // Check if the previous chord has EXPLICIT rhythm (slashes) vs DEFAULT (auto-fill)
                                // - If previous chord has Slashes: measure is explicitly full, add SPACES
                                // - If previous chord has Default: it was auto-filled, so SET the duration
                                let prev_chord_has_explicit_rhythm = measures
                                    .last()
                                    .and_then(|m| m.chords.last())
                                    .is_some_and(|c| {
                                        matches!(
                                            c.rhythm,
                                            ChordRhythm::Slashes { .. } | ChordRhythm::Explicit(_)
                                        )
                                    });

                                if let Some(last_measure) = measures.last_mut() {
                                    if let Some(last_chord) = last_measure.chords.last_mut() {
                                        // If after explicit | separator OR previous chord has explicit rhythm,
                                        // add SPACES to current measure instead of modifying previous chord.
                                        // "Abmaj9 //// | //" = Abmaj9 (4 slashes) + 2 spaces (continuation)
                                        // "C //// //" = C (4 slashes) + 2 spaces (explicit slashes = full)
                                        // "Cm/Eb / Eb ///" = SET to 1 beat (Default rhythm = auto-filled)
                                        if measure_was_created_by_separator
                                            || prev_chord_has_explicit_rhythm
                                        {
                                            // After explicit separator: add SPACES to current measure
                                            // as continuation of the previous chord. These render as
                                            // rhythm slashes without a chord symbol.

                                            // Create a space for each beat of continuation
                                            for i in 0..slash_count {
                                                let space_duration =
                                                    MusicalDuration::from_beats(1.0, time_sig);
                                                let space_rhythm = ChordRhythm::slashes(1);
                                                let space = SpaceInstance::new(
                                                    space_rhythm,
                                                    space_duration,
                                                    AbsolutePosition::new(
                                                        MusicalPosition::try_new(
                                                            measures.len() as i32,
                                                            i as i32,
                                                            0,
                                                        )
                                                        .unwrap_or_else(|_| {
                                                            MusicalPosition::start()
                                                        }),
                                                        self.sections.len(),
                                                    ),
                                                    format!("/{}", "/".repeat(i as usize)),
                                                );
                                                current_measure
                                                    .rhythm_elements
                                                    .push(RhythmElement::Space(space));
                                            }
                                            current_measure_beats += f64::from(slash_count);
                                            true
                                        } else {
                                            // Auto-completed measure: only accumulate if previous was slash
                                            let new_count = if last_token_was_slash {
                                                if let ChordRhythm::Slashes {
                                                    count: existing,
                                                    ..
                                                } = &last_chord.rhythm
                                                {
                                                    existing + slash_count
                                                } else {
                                                    slash_count
                                                }
                                            } else {
                                                slash_count
                                            };
                                            last_chord.rhythm = ChordRhythm::slashes(new_count);
                                            last_chord.duration = MusicalDuration::from_beats(
                                                f64::from(new_count),
                                                time_sig,
                                            );

                                            // Also update the chord in rhythm_elements
                                            for elem in
                                                last_measure.rhythm_elements.iter_mut().rev()
                                            {
                                                if let RhythmElement::Chord(c) = elem {
                                                    c.rhythm = ChordRhythm::slashes(new_count);
                                                    c.duration = MusicalDuration::from_beats(
                                                        f64::from(new_count),
                                                        time_sig,
                                                    );
                                                    break;
                                                }
                                            }
                                            true
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if applied {
                                // Recalculate total beats for current measure since the duration changed
                                if !current_measure.chords.is_empty()
                                    || !current_measure.rhythm_elements.is_empty()
                                {
                                    // Mark that this measure has slash rhythm, so subsequent
                                    // chords can fill remaining beats
                                    measure_has_slash_rhythm = true;
                                } else if !measures.is_empty() {
                                    // The slash was applied to the previous measure (SET mode).
                                    // Check if it now has room and we should "un-pop" it.
                                    let prev_measure_beats: f64 = measures
                                        .last()
                                        .unwrap()
                                        .chords
                                        .iter()
                                        .map(|c| c.duration.to_beats(time_sig))
                                        .sum();

                                    if prev_measure_beats < beats_per_measure - 0.001 {
                                        // Previous measure now has room - pop it back to current_measure
                                        // so subsequent chords can be added to it
                                        current_measure = measures.pop().unwrap();
                                        current_measure_beats = prev_measure_beats;
                                        // Mark that this measure has slash rhythm since we un-popped it
                                        measure_has_slash_rhythm = true;
                                    }
                                    // Note: if we didn't un-pop (measure is full), don't set
                                    // measure_has_slash_rhythm for the empty current measure
                                }
                                last_token_was_slash = true;
                                continue;
                            }
                        } // end else !last_chord_is_tied
                    }
                }
            }

            // Reset slash tracking for non-slash tokens
            last_token_was_slash = false;

            // Check for tie symbol (~)
            // The tie connects the previous chord's duration to the following duration.
            // Example: "C // ~ /" = C with 2 beats + 1 beat = 3 beats total
            if *token_str == "~" {
                // Find the last chord and mark it as having a pending tie
                // The next slash/duration token will add to this chord's duration
                let last_chord_found = if let Some(last_chord) = current_measure.chords.last_mut() {
                    // Set tied flag using the new method
                    last_chord.rhythm = last_chord.rhythm.clone().with_tie();
                    // Also update in rhythm_elements
                    for elem in current_measure.rhythm_elements.iter_mut().rev() {
                        if let RhythmElement::Chord(c) = elem {
                            c.rhythm = c.rhythm.clone().with_tie();
                            break;
                        }
                    }
                    true
                } else if !measures.is_empty() {
                    if let Some(last_measure) = measures.last_mut() {
                        if let Some(last_chord) = last_measure.chords.last_mut() {
                            last_chord.rhythm = last_chord.rhythm.clone().with_tie();
                            // Also update in rhythm_elements
                            for elem in last_measure.rhythm_elements.iter_mut().rev() {
                                if let RhythmElement::Chord(c) = elem {
                                    c.rhythm = c.rhythm.clone().with_tie();
                                    break;
                                }
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if last_chord_found {
                    // Mark that the next duration token should add to the tied chord
                    // We'll handle this by checking for slashes that follow ties
                }
                continue;
            }

            // Check for slashes following a tie: these should ADD to the previous chord's duration
            // Check if the previous token was a tie (we handle this by checking rhythm flags)
            // Since we can't easily look back at previous tokens, we check the chord's tied flag
            if token_str.chars().all(|c| c == '/') && !token_str.is_empty() {
                let slash_count = token_str.len() as u8;

                // Check if the last chord is tied - if so, add to its duration
                let _last_chord_is_tied = {
                    if let Some(last_chord) = current_measure.chords.last() {
                        last_chord.rhythm.is_tied()
                    } else if !measures.is_empty() {
                        if let Some(last_measure) = measures.last() {
                            if let Some(last_chord) = last_measure.chords.last() {
                                last_chord.rhythm.is_tied()
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                // Check if we have a tied chord that we should extend
                // Look for a chord with tied=true
                let extended_tied = {
                    let mut found = false;
                    if let Some(last_chord) = current_measure.chords.last_mut() {
                        if last_chord.rhythm.is_tied() {
                            // Add slash duration to the tied chord
                            let current_beats = last_chord.duration.to_beats(time_sig);
                            let new_beats = current_beats + f64::from(slash_count);
                            last_chord.duration = MusicalDuration::from_beats(new_beats, time_sig);
                            // Clear the tie flag after extending
                            last_chord.rhythm.clear_tie();
                            // Update in rhythm_elements too
                            for elem in current_measure.rhythm_elements.iter_mut().rev() {
                                if let RhythmElement::Chord(c) = elem {
                                    c.duration = last_chord.duration;
                                    c.rhythm.clear_tie();
                                    break;
                                }
                            }
                            found = true;
                        }
                    } else if !measures.is_empty() {
                        if let Some(last_measure) = measures.last_mut() {
                            if let Some(last_chord) = last_measure.chords.last_mut() {
                                if last_chord.rhythm.is_tied() {
                                    let current_beats = last_chord.duration.to_beats(time_sig);
                                    let new_beats = current_beats + f64::from(slash_count);
                                    last_chord.duration =
                                        MusicalDuration::from_beats(new_beats, time_sig);
                                    last_chord.rhythm.clear_tie();
                                    for elem in last_measure.rhythm_elements.iter_mut().rev() {
                                        if let RhythmElement::Chord(c) = elem {
                                            c.duration = last_chord.duration;
                                            c.rhythm.clear_tie();
                                            break;
                                        }
                                    }
                                    found = true;
                                }
                            }
                        }
                    }
                    found
                };

                if extended_tied {
                    continue;
                }
                // If not a tied chord, fall through to normal slash handling below
            }

            // Check for melody variable reference (e.g., "$mainRiff")
            if let Some(var_name) = token_str.strip_prefix('$') {
                if let Some(melody) = self.melody_variables.get(var_name).cloned() {
                    // Attach the melody to the current measure
                    current_measure.melodies.push(melody);
                } else {
                    tracing::warn!("Unknown melody variable '{}'", var_name);
                }
                continue;
            }

            // Check for inline melody block (e.g., "m{ C_8 D_8 E_4 }")
            if token_str.starts_with("m{") {
                in_melody_block = true;
                // Find the full melody block in the original line
                if let Some(m_pos) = line_to_parse.find("m{") {
                    let melody_start = &line_to_parse[m_pos..];
                    // Find the closing brace
                    if let Some(close_pos) = melody_start.find('}') {
                        let melody_str = &melody_start[..close_pos + 1];
                        match Melody::parse_block(melody_str) {
                            Ok((name, melody)) => {
                                if let Some(var_name) = name {
                                    // Store as a variable
                                    self.melody_variables.set(var_name, melody.clone());
                                }
                                // Attach melody to current measure
                                current_measure.melodies.push(melody);
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to parse inline melody '{}': {}",
                                    melody_str,
                                    e
                                );
                            }
                        }
                    }
                }
                // Check if this token contains the closing brace (e.g., "m{C_4}")
                if token_str.contains('}') {
                    in_melody_block = false;
                }
                continue;
            }

            // Skip tokens that are inside a melody block
            if in_melody_block {
                // Check if this token ends the melody block
                if token_str.contains('}') {
                    in_melody_block = false;
                }
                continue;
            }

            // Check for inline text cue (e.g., "@keys", "synth", "here" followed by closing quote logic)
            if token_str.starts_with('@') {
                // Start of an inline cue - collect tokens until we find the closing quote
                // For simplicity, assume the cue is in format: @group "text in quotes"
                // We need to look ahead and collect the full cue string
                // For now, we'll handle this by reconstructing the cue from remaining tokens

                // Find the cue in the original line starting from this position
                if let Some(at_pos) = line_to_parse.find(token_str) {
                    let cue_start = &line_to_parse[at_pos..];
                    // Try to parse the cue
                    if let Some(quote_start) = cue_start.find('"') {
                        if let Some(quote_end) = cue_start[quote_start + 1..].find('"') {
                            let cue_str = &cue_start[..quote_start + 1 + quote_end + 1];
                            if let Ok(cue) = TextCue::parse(cue_str) {
                                pending_cue = Some(cue);
                            }
                        }
                    }
                }
                continue;
            }

            // Skip tokens that are part of the cue text (between quotes)
            if pending_cue.is_some() && (token_str.starts_with('"') || token_str.ends_with('"')) {
                continue;
            }

            // Check for inline dynamic marking (e.g., "<Build>", "<Down>", "<Hit>:3")
            if token_str.starts_with('<') && token_str.contains('>') {
                if let Ok(dynamic) = DynamicMarking::parse(token_str) {
                    pending_dynamic = Some(dynamic);
                }
                continue;
            }

            // Check for tempo change (e.g., "->140bpm", "->120")
            if token_str.starts_with("->") {
                if let Some(new_tempo) = TempoChange::parse_syntax(token_str) {
                    // Record the tempo change
                    let position = AbsolutePosition::new(
                        MusicalPosition::try_new(
                            measures.len() as i32,
                            current_measure_beats as i32,
                            0,
                        )
                        .unwrap_or_else(|_| MusicalPosition::start()),
                        self.sections.len(),
                    );
                    let tempo_change =
                        TempoChange::new(position, self.tempo, new_tempo, self.sections.len());
                    self.tempo_changes.push(tempo_change);
                    self.tempo = Some(new_tempo);
                }
                continue;
            }

            // Check for measure separator (|)
            // This forces a measure boundary regardless of beat count
            if *token_str == "|" {
                // Finalize current measure if it has chords, or if it was created by a previous separator
                // (This allows multiple | in a row to create empty measures)
                // But don't push auto-created empty measures (created when a measure fills up)
                if !current_measure.chords.is_empty() || measure_was_created_by_separator {
                    measures.push(current_measure.clone());
                }
                // Always start a new measure after |
                current_measure = Measure::new();
                current_measure.time_signature =
                    (time_sig.numerator as u8, time_sig.denominator as u8);
                current_measure_beats = 0.0;
                just_processed_separator = true; // Mark that we just processed a separator
                measure_was_created_by_separator = true; // Mark that this measure was created by |
                measure_has_slash_rhythm = false; // Reset for new measure
                continue;
            }

            // Check for time signature change (e.g., "6/8", "3/4")
            if token_str.contains('/') && !token_str.starts_with('/') {
                if let Some((num, den)) = Self::parse_time_signature(token_str) {
                    // Update the time signature for subsequent measures
                    time_sig = TimeSignature::new(num as u32, den as u32);
                    self.time_signature = Some(time_sig);

                    // If we have a current measure, finalize it before the time sig change
                    if !current_measure.chords.is_empty() {
                        measures.push(current_measure.clone());
                        current_measure = Measure::new();
                        current_measure_beats = 0.0;
                    }

                    // Update time signature for new measures
                    current_measure.time_signature = (num, den);
                    beats_per_measure = num as f64;
                    continue;
                }
            }

            // Check for key change - should be ONLY a key signature (short token like "#G", "bBb")
            // Not a chord like "bbmaj7" or "g7"
            let looks_like_key_sig =
                token_str.len() <= 3 && (token_str.starts_with('#') || token_str.starts_with('b'));

            if looks_like_key_sig {
                if let Ok(new_key) = Key::parse(token_str) {
                    // Track key change
                    let position = AbsolutePosition::at_beginning(); // TODO: Calculate actual position
                    let section_index = 0; // TODO: Track current section index
                    let key_change = KeyChange::new(
                        position,
                        self.current_key.clone(),
                        new_key.clone(),
                        section_index,
                    );
                    self.key_changes.push(key_change);
                    self.current_key = Some(new_key);
                    continue;
                }
            }

            // Check for standalone rest token (r4, r8, r8t, r4t, r2, etc.)
            // These don't have a chord symbol but should be stored as rhythm elements
            if token_str.starts_with('r') && token_str.len() >= 2 {
                // Try to parse as a rest
                let mut lexer = Lexer::new(token_str.to_string());
                let tokens = lexer.tokenize();
                if let Ok((rhythm, _)) = ChordRhythm::parse(&tokens) {
                    if rhythm.is_rest() {
                        let duration = rhythm.to_duration(time_sig);
                        let chord_beats = duration.to_beats(time_sig);

                        // Check if we need to start a new measure
                        if !just_processed_separator
                            && current_measure_beats + chord_beats > beats_per_measure + 0.001
                        {
                            if !current_measure.chords.is_empty()
                                || !current_measure.rhythm_elements.is_empty()
                            {
                                measures.push(current_measure.clone());
                            }
                            current_measure = Measure::new();
                            current_measure.time_signature =
                                (time_sig.numerator as u8, time_sig.denominator as u8);
                            current_measure_beats = 0.0;
                        }
                        just_processed_separator = false;
                        measure_was_created_by_separator = false;

                        // Create a RestInstance and add it to rhythm_elements
                        let rest_instance = RestInstance::new(
                            rhythm,
                            duration,
                            AbsolutePosition::new(
                                MusicalPosition::try_new(
                                    measures.len() as i32,
                                    current_measure_beats as i32,
                                    0,
                                )
                                .unwrap_or_else(|_| MusicalPosition::start()),
                                self.sections.len(),
                            ),
                            token_str.to_string(),
                        );

                        current_measure
                            .rhythm_elements
                            .push(RhythmElement::Rest(rest_instance));
                        current_measure_beats += chord_beats;

                        // Check if measure is complete
                        if (current_measure_beats - beats_per_measure).abs() < 0.001 {
                            measures.push(current_measure.clone());
                            current_measure = Measure::new();
                            current_measure.time_signature =
                                (time_sig.numerator as u8, time_sig.denominator as u8);
                            current_measure_beats = 0.0;
                        }
                        continue;
                    }
                }
            }

            // Check for standalone space token (s1, s2, s4, s8, etc.)
            // These represent "invisible" duration - the measure will be filled with automatic slashes
            if token_str.starts_with('s') && token_str.len() >= 2 {
                // Try to parse as a space
                let mut lexer = Lexer::new(token_str.to_string());
                let tokens = lexer.tokenize();
                if let Ok((rhythm, _)) = ChordRhythm::parse(&tokens) {
                    if rhythm.is_space() {
                        let duration = rhythm.to_duration(time_sig);
                        let chord_beats = duration.to_beats(time_sig);

                        // Check if we need to start a new measure
                        if !just_processed_separator
                            && current_measure_beats + chord_beats > beats_per_measure + 0.001
                        {
                            if !current_measure.chords.is_empty()
                                || !current_measure.rhythm_elements.is_empty()
                            {
                                measures.push(current_measure.clone());
                            }
                            current_measure = Measure::new();
                            current_measure.time_signature =
                                (time_sig.numerator as u8, time_sig.denominator as u8);
                            current_measure_beats = 0.0;
                        }
                        just_processed_separator = false;
                        measure_was_created_by_separator = false;

                        // Create a SpaceInstance (not a chord - just a placeholder for auto-fill)
                        let space_instance = SpaceInstance::new(
                            rhythm,
                            duration,
                            AbsolutePosition::new(
                                MusicalPosition::try_new(
                                    measures.len() as i32,
                                    current_measure_beats as i32,
                                    0,
                                )
                                .unwrap_or_else(|_| MusicalPosition::start()),
                                self.sections.len(),
                            ),
                            token_str.to_string(),
                        );

                        // Add to rhythm_elements only (not chords - space is not a chord)
                        current_measure
                            .rhythm_elements
                            .push(RhythmElement::Space(space_instance));
                        current_measure_beats += chord_beats;

                        // Check if measure is complete
                        if (current_measure_beats - beats_per_measure).abs() < 0.001 {
                            measures.push(current_measure.clone());
                            current_measure = Measure::new();
                            current_measure.time_signature =
                                (time_sig.numerator as u8, time_sig.denominator as u8);
                            current_measure_beats = 0.0;
                        }
                        continue;
                    }
                }
            }

            // Check for dot repeat token (. repeats the last chord)
            // Note: apply_auto_durations may add _N suffix, so check for "." or "._N" pattern
            if *token_str == "." || token_str.starts_with("._") {
                if let Some(ref prev_chord) = last_chord {
                    // Clone the last chord with a fresh position
                    let mut repeat_chord = prev_chord.clone();
                    repeat_chord.original_token = ".".to_string();
                    repeat_chord.position = AbsolutePosition::at_beginning(); // Will be recalculated
                                                                              // Clear push/pull - the dot repeat doesn't inherit the timing modifier
                    repeat_chord.push_pull = None;
                    // Inherit the source chord's rhythm and duration
                    // "F/C ." = two measures (F/C for 4 beats, then F/C repeated for 4 beats)
                    // The rhythm and duration are already set from the clone

                    let chord_beats = repeat_chord.duration.to_beats(time_sig);

                    // Handle measure boundaries (same logic as regular chord)
                    if !just_processed_separator
                        && current_measure_beats + chord_beats > beats_per_measure + 0.001
                    {
                        if !current_measure.chords.is_empty()
                            || !current_measure.rhythm_elements.is_empty()
                        {
                            measures.push(current_measure.clone());
                        }
                        current_measure = Measure::new();
                        current_measure.time_signature =
                            (time_sig.numerator as u8, time_sig.denominator as u8);
                        current_measure_beats = 0.0;
                    }
                    just_processed_separator = false;
                    measure_was_created_by_separator = false;

                    current_measure
                        .rhythm_elements
                        .push(RhythmElement::Chord(repeat_chord.clone()));
                    current_measure.chords.push(repeat_chord);
                    current_measure_beats += chord_beats;

                    // Auto-advance measure if full
                    if !just_processed_separator
                        && (current_measure_beats - beats_per_measure).abs() < 0.001
                    {
                        measures.push(current_measure.clone());
                        current_measure = Measure::new();
                        current_measure.time_signature =
                            (time_sig.numerator as u8, time_sig.denominator as u8);
                        current_measure_beats = 0.0;
                    }
                }
                continue;
            }

            // Parse chord
            // TODO: Compute source_span from line offset map and token position
            match self.parse_chord_token(token_str, section_type, time_sig, None) {
                Ok(mut chord) => {
                    let mut chord_beats = chord.duration.to_beats(time_sig);

                    // If we just processed a separator, we're already in a new measure
                    // Don't create another one automatically
                    if !just_processed_separator {
                        // Check if adding this chord would exceed the measure
                        if current_measure_beats + chord_beats > beats_per_measure + 0.001 {
                            // Look ahead to see if slashes are coming for this chord
                            // If so, fill remaining beats instead of creating a new measure
                            let slashes_coming = tokens_str[token_idx + 1..]
                                .iter()
                                .take_while(|t| {
                                    // Stop at measure separator or new chord
                                    **t != "|"
                                        && !t.chars().next().is_none_or(|c| c.is_alphabetic())
                                })
                                .any(|t| t.chars().all(|c| c == '/') && !t.is_empty());

                            // If the measure has slash rhythm and the chord has default rhythm,
                            // OR if slashes are coming for this chord,
                            // adjust to fill remaining beats instead of starting a new measure.
                            // e.g., "Cm/Eb / Eb ///" - Cm/Eb takes 1 beat (from /), Eb will take
                            // remaining 3 beats when its /// is processed
                            let remaining_beats = beats_per_measure - current_measure_beats;
                            if (measure_has_slash_rhythm || slashes_coming)
                                && chord.rhythm == ChordRhythm::Default
                                && remaining_beats > 0.001
                            {
                                // Adjust chord to fill remaining beats (duration only)
                                // IMPORTANT: Keep rhythm as Default so actual slashes can override.
                                // If we set rhythm to Slashes here, subsequent slashes would think
                                // it's explicitly set and create spaces instead of setting duration.
                                chord.duration =
                                    MusicalDuration::from_beats(remaining_beats, time_sig);
                                chord_beats = remaining_beats;
                            } else {
                                // small epsilon for float comparison
                                // Current measure is full, start a new one
                                if !current_measure.chords.is_empty()
                                    || !current_measure.rhythm_elements.is_empty()
                                {
                                    measures.push(current_measure.clone());
                                }
                                current_measure = Measure::new();
                                current_measure.time_signature =
                                    (time_sig.numerator as u8, time_sig.denominator as u8);
                                current_measure_beats = 0.0;
                                measure_has_slash_rhythm = false; // Reset for new measure
                                let _measure_was_created_by_separator = false; // Auto-created, not by separator
                            }
                        }
                    }
                    just_processed_separator = false; // Reset flag after processing chord
                    measure_was_created_by_separator = false; // Reset flag - measure now has content

                    // Store as last chord for dot repeat
                    last_chord = Some(chord.clone());

                    // Add chord to both chords (for backward compat) and rhythm_elements
                    current_measure
                        .rhythm_elements
                        .push(RhythmElement::Chord(chord.clone()));
                    current_measure.chords.push(chord);
                    current_measure_beats += chord_beats;

                    // Attach pending cue to this measure if present
                    if let Some(cue) = pending_cue.take() {
                        current_measure.text_cues.push(cue);
                    }

                    // Attach pending dynamic to this measure if present
                    if let Some(dynamic) = pending_dynamic.take() {
                        current_measure.dynamics.push(dynamic);
                    }

                    // If we've completed exactly one measure, start a new one
                    // (but only if we didn't just process a separator)
                    // When we just processed a separator, we're already in a fresh measure,
                    // so we don't want to auto-create another one
                    if !just_processed_separator
                        && (current_measure_beats - beats_per_measure).abs() < 0.001
                    {
                        // small epsilon for float comparison
                        measures.push(current_measure.clone());
                        current_measure = Measure::new();
                        current_measure.time_signature =
                            (time_sig.numerator as u8, time_sig.denominator as u8);
                        current_measure_beats = 0.0;
                        measure_has_slash_rhythm = false; // Reset for new measure
                                                          // Auto-created measure, not by separator
                    }
                }
                Err(_e) => {
                    // Skip unparseable tokens silently
                    // (might be a formatting token or invalid chord)
                }
            }
        }

        // Add last measure if it has content
        // (If we just processed a separator, the empty measure was already pushed)
        if !current_measure.chords.is_empty() || !current_measure.rhythm_elements.is_empty() {
            measures.push(current_measure);
        }

        // Handle repeat count
        let final_repeat_count = match repeat_count {
            RepeatCount::Fixed(count) => count,
            RepeatCount::Auto => {
                // Calculate auto-repeat based on section length vs phrase length
                // We need to calculate phrase length in BEATS, not measures,
                // because chords with explicit durations (like 6_2) might not fill complete measures

                if measures.is_empty() {
                    // Empty phrase, can't calculate
                    return Err("Cannot use x^ with empty phrase".to_string());
                }

                // Calculate total beats in the phrase
                let phrase_beats: f64 = measures
                    .iter()
                    .flat_map(|m| &m.chords)
                    .map(|chord| chord.duration.to_beats(time_sig))
                    .sum();

                // Get section length in measures
                let section_measures = if let Some(count) = section_measure_count {
                    count
                } else {
                    // If no explicit measure count, we can't calculate auto-repeat
                    // This is a limitation - x^ requires an explicit section length
                    return Err(
                        "Cannot use x^ without explicit section measure count (e.g., 'VS 16')"
                            .to_string(),
                    );
                };

                // Convert section length to beats
                let section_beats = section_measures as f64 * beats_per_measure;

                // Calculate how many times to repeat: section_beats / phrase_beats
                let repeat_count_f = section_beats / phrase_beats;

                // Check if it's a whole number (within floating point precision)
                let repeat_count_rounded = repeat_count_f.round();
                if (repeat_count_f - repeat_count_rounded).abs() > 0.001 {
                    return Err(format!(
                        "Cannot use x^: section length ({} measures = {} beats) is not evenly divisible by phrase length ({} beats). Would need {} repeats.",
                        section_measures, section_beats, phrase_beats, repeat_count_f
                    ));
                }

                let repeat_count_int = repeat_count_rounded as usize;
                if repeat_count_int == 0 {
                    return Err(format!(
                        "Cannot use x^: phrase length ({} beats) is longer than section length ({} measures = {} beats)",
                        phrase_beats, section_measures, section_beats
                    ));
                }

                repeat_count_int
            }
        };

        // If repeat count specified, store it on the last measure of the pattern
        // and duplicate the measures
        if final_repeat_count > 1 && !measures.is_empty() {
            let pattern_length = measures.len();

            // Store repeat count on the last measure of the pattern for display purposes
            // Do this BEFORE cloning so it's preserved in the original
            measures[pattern_length - 1].repeat_count = final_repeat_count;

            // Duplicate all measures for the actual playback/structure
            let original_measures = measures.clone();
            for _ in 1..final_repeat_count {
                let mut repeated = original_measures.clone();
                // Clear repeat_count on duplicates so only first occurrence shows it
                for measure in &mut repeated {
                    measure.repeat_count = 1;
                }
                measures.extend(repeated);
            }
        }

        Ok(measures)
    }

    fn normalize_parallel_container_syntax(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();
        let mut parallel_depth = 0usize;

        while let Some(ch) = chars.next() {
            if ch == '<' && chars.peek() == Some(&'<') {
                chars.next();
                parallel_depth += 1;
                continue;
            }

            if ch == '>' && chars.peek() == Some(&'>') {
                chars.next();
                parallel_depth = parallel_depth.saturating_sub(1);
                continue;
            }

            if parallel_depth > 0 && ch == ';' {
                out.push(' ');
            } else {
                out.push(ch);
            }
        }

        out
    }

    fn parse_parallel_chord_line(
        &mut self,
        line: &str,
        repeat_count: RepeatCount,
        section_type: &SectionType,
        _section_measure_count: Option<usize>,
    ) -> Result<Vec<Measure>, String> {
        let measure_parts = Self::split_top_level_measures(line);
        let mut measures = Vec::new();

        for part in measure_parts {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("<<") && trimmed.ends_with(">>") {
                measures.push(self.parse_parallel_measure(trimmed, section_type)?);
            } else {
                let mut parsed = self.parse_chord_line(trimmed, section_type, Some(1))?;
                if let Some(measure) = parsed.into_iter().next() {
                    measures.push(measure);
                }
            }
        }

        if let RepeatCount::Fixed(count) = repeat_count {
            if count > 1 {
                if let Some(last) = measures.last_mut() {
                    last.repeat_count = count;
                }
            }
        }

        Ok(measures)
    }

    fn parse_parallel_measure(
        &mut self,
        measure_text: &str,
        section_type: &SectionType,
    ) -> Result<Measure, String> {
        let inner = measure_text
            .trim()
            .strip_prefix("<<")
            .and_then(|s| s.strip_suffix(">>"))
            .ok_or_else(|| format!("Invalid parallel container: {}", measure_text))?
            .trim();

        let branches = Self::split_top_level_parallel_branches(inner);
        let mut merged = Measure::new();

        for branch in branches {
            let trimmed = branch.trim();
            if trimmed.is_empty() {
                continue;
            }

            let mut parsed = self.parse_chord_line(trimmed, section_type, Some(1))?;
            let Some(branch_measure) = parsed.into_iter().next() else {
                continue;
            };

            merged.chords.extend(branch_measure.chords);
            merged.rhythm_elements.extend(branch_measure.rhythm_elements);
            merged.rhythm_slashes.extend(branch_measure.rhythm_slashes);
            merged.text_cues.extend(branch_measure.text_cues);
            merged.dynamics.extend(branch_measure.dynamics);
            merged.melodies.extend(branch_measure.melodies);
            merged.repeat_count = merged.repeat_count.max(branch_measure.repeat_count);
            merged.time_signature = branch_measure.time_signature;
            if merged.source_span.is_none() {
                merged.source_span = branch_measure.source_span;
            }
        }

        Ok(merged)
    }

    fn split_top_level_measures(input: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = input.chars().peekable();
        let mut parallel_depth = 0usize;
        let mut brace_depth = 0usize;

        while let Some(ch) = chars.next() {
            if ch == '<' && chars.peek() == Some(&'<') {
                parallel_depth += 1;
                current.push('<');
                current.push('<');
                chars.next();
                continue;
            }

            if ch == '>' && chars.peek() == Some(&'>') {
                parallel_depth = parallel_depth.saturating_sub(1);
                current.push('>');
                current.push('>');
                chars.next();
                continue;
            }

            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                '|' if parallel_depth == 0 && brace_depth == 0 => {
                    parts.push(current.trim().to_string());
                    current.clear();
                    continue;
                }
                _ => {}
            }

            current.push(ch);
        }

        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
        }

        parts
    }

    fn split_top_level_parallel_branches(input: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut brace_depth = 0usize;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                ';' if brace_depth == 0 => {
                    parts.push(current.trim().to_string());
                    current.clear();
                    continue;
                }
                _ => {}
            }

            current.push(ch);
        }

        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
        }

        parts
    }
}

// endregion: --- Chord Line Parsing

// region:    --- Chord Token Parsing

impl<'a> ChartParser<'a> {
    /// Parse a single chord token
    ///
    /// # Arguments
    /// * `token` - The chord token string (e.g., "Cmaj7", "Am7/G")
    /// * `section_type` - Current section type for chord memory
    /// * `time_sig` - Current time signature for duration calculation
    /// * `source_span` - Optional source text span for linking back to input
    pub(super) fn parse_chord_token(
        &mut self,
        token: &str,
        section_type: &SectionType,
        time_sig: TimeSignature,
        source_span: Option<TextSpan>,
    ) -> Result<ChordInstance, String> {
        use crate::chart::commands::Command;
        use crate::chord::Chord;

        // Check for accent prefix (>) BEFORE push - indicates accent on the pushed beat
        // Supports: >'C (accent on the anticipation beat 4.66)
        let (accent_before_push, token_after_leading_accent) = if token.starts_with('>') {
            (true, token.strip_prefix('>').unwrap_or(token))
        } else {
            (false, token)
        };

        // Check for one-time override (prefix !)
        let (is_override, token_clean) = if token_after_leading_accent.starts_with('!') {
            (
                true,
                token_after_leading_accent
                    .strip_prefix('!')
                    .unwrap_or(token_after_leading_accent),
            )
        } else {
            (false, token_after_leading_accent)
        };

        // Check for push notation (leading apostrophes with optional triplet/tuplet: 'C, ''Em, 'tC, ':5C)
        let (push_modifier, token_after_push) = Self::extract_leading_push_modifiers(token_clean);

        // Check for accent AFTER push prefix - indicates accent on the downbeat
        // Supports: '>C (accent on beat 1, the downbeat)
        let (accent_after_push, token_after_post_accent) = if token_after_push.starts_with('>') {
            (
                true,
                token_after_push
                    .strip_prefix('>')
                    .unwrap_or(token_after_push),
            )
        } else {
            (false, token_after_push)
        };

        // Check for accent shorthand (->) - legacy syntax, also supported (treated as regular accent)
        let (has_accent_inline, token_no_accent) = if token_after_post_accent.contains("->") {
            (true, token_after_post_accent.replace("->", ""))
        } else {
            (false, token_after_post_accent.to_string())
        };
        let token_after_post_accent = token_no_accent.as_str();

        // Check for pull notation (trailing apostrophes with optional triplet/tuplet: C', Em'', C't, C':5)
        let (token_after_pull, pull_modifier) =
            Self::extract_trailing_pull_modifiers(token_after_post_accent);

        // Check for slash chord (e.g., "1/3", "Cmaj7/E", "g/b")
        // But NOT rhythm slashes (e.g., "g//", "C///")
        let (chord_part, bass_part) = Self::split_slash_chord(&token_after_pull);

        // Normalize case for chord parsing - capitalize first letter if it's a note name
        // This allows "cmaj7" to be parsed as "Cmaj7"
        let normalized_token = Self::normalize_chord_case(&chord_part);

        // Parse the chord using the Chord parser
        let mut lexer = Lexer::new(normalized_token.clone());
        let tokens = lexer.tokenize();

        let mut chord = Chord::parse(&tokens)
            .map_err(|e| format!("Failed to parse chord '{}': {:?}", chord_part, e))?;

        // Add bass note if this is a slash chord
        if let Some(bass_str) = bass_part {
            let bass_normalized = Self::normalize_chord_case(bass_str);
            let bass_notation = RootNotation::from_string(&bass_normalized)
                .ok_or_else(|| format!("Invalid bass note: {}", bass_str))?;
            chord.bass = Some(bass_notation);
        }

        // Extract the root from the ORIGINAL token (before normalization, but after apostrophes)
        // This preserves the original casing/format (lowercase note, scale degree, Roman numeral)
        // For scale degrees with quality (e.g., "2maj"), we need to extract just the number part
        // But we pass the full chord_part to process_chord so it can detect explicit quality
        let root_from_token = Self::extract_root_from_token(&chord_part);

        // For slash chords with just a root (no explicit quality), don't recall from memory.
        // Writing "F/C" means "F major over C bass", not "whatever F I used before with C bass".
        let is_slash_chord_with_just_root =
            chord.bass.is_some() && chord_part.len() <= root_from_token.len() + 1; // +1 for possible accidental

        // Use ChordMemory to process this chord and get the appropriate full symbol
        // Pass chord_part (which includes quality like "2maj") so it can detect explicit quality
        let current_key = self.current_key.clone();
        let mut full_symbol = if is_slash_chord_with_just_root {
            // Slash chord with just root - use the normalized symbol, don't recall from memory
            chord.normalized.clone()
        } else {
            self.chord_memory.process_chord(
                &root_from_token,
                &chord_part, // Use chord_part (after apostrophes stripped) - includes "2maj" not just "2"
                &chord.normalized,
                section_type,
                is_override,
                current_key.as_ref(),
            )
        };

        // Append bass note to full_symbol for slash chords (e.g., "F" + "/C" -> "F/C")
        if let Some(bass) = &chord.bass {
            full_symbol = format!("{}/{}", full_symbol, bass);
        }

        // Get rhythm and duration (push/pull will be applied later)
        let rhythm = chord.duration.clone().unwrap_or(ChordRhythm::Default);
        let duration = rhythm.to_duration(time_sig);

        // Store push/pull info for later adjustment
        // Use settings.push_mode as the default base, but explicit 't' or ':N' modifiers override it
        let default_push_mode = self.settings.push_mode;
        let push_pull_info = if push_modifier.is_present() {
            push_modifier
                .to_amount(default_push_mode)
                .map(|amt| (true, amt))
        } else if pull_modifier.is_present() {
            pull_modifier
                .to_amount(default_push_mode)
                .map(|amt| (false, amt))
        } else {
            None
        };

        // Create a RootNotation from the original token to preserve casing
        // (e.g., "vi" stays "vi", not "Vi")
        // Fall back to the parsed chord's root if we can't parse the original
        let root_notation =
            RootNotation::from_string(&root_from_token).unwrap_or_else(|| chord.root.clone());

        // Store original token without auto-assigned duration suffix
        // Auto-duration adds _1, _2, _4, _8, _16 to tokens, but we want to preserve
        // whether the USER explicitly specified a duration
        let original_token = Self::strip_auto_duration_suffix(token);

        // Create chord instance
        let mut instance = ChordInstance::new(
            root_notation,
            full_symbol,
            chord,
            rhythm,
            original_token,
            duration,
            AbsolutePosition::at_beginning(), // Will be calculated in post-processing
        )
        .with_push_pull(push_pull_info);

        // Add source span for click-to-highlight and editing
        if let Some(span) = source_span {
            instance = instance.with_source_span(span);
        }

        // Add accent command if present
        // The order of > and ' determines which beat gets the accent:
        // - >'C = accent before push = AccentOnPush (accent on the anticipation beat)
        // - '>C = accent after push = Accent (accent on the downbeat)
        let has_push = push_modifier.is_present();
        if accent_before_push && has_push {
            // Accent comes before push marker (>'C) - accent goes on the pushed/anticipation beat
            instance = instance.add_command(Command::AccentOnPush);
        } else if accent_before_push || accent_after_push || has_accent_inline {
            // Regular accent - on the downbeat (or no push involved)
            instance = instance.add_command(Command::Accent);
        }

        Ok(instance)
    }
}

// endregion: --- Chord Token Parsing

// region:    --- Tests

#[cfg(test)]
mod tests {
    use super::ChartParser as Chart;
    use super::*;
    use crate::chart::parse_chart;

    #[test]
    fn test_duration_push_parsing_ambiguous_cases() {
        // Test '_44 - scale degree 4 pushed by quarter note
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_44");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
        assert_eq!(remaining, "4");

        // Test '_84 - scale degree 4 pushed by eighth note
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_84");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
        assert_eq!(remaining, "4");

        // Test '_164 - scale degree 4 pushed by sixteenth note
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_164");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Sixteenth);
        assert_eq!(remaining, "4");

        // Test '_324 - scale degree 4 pushed by thirty-second note
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_324");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::ThirtySecond);
        assert_eq!(remaining, "4");

        // Test '_4C - C pushed by quarter note (non-ambiguous)
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_4C");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
        assert_eq!(remaining, "C");

        // Test '_8tC - C pushed by triplet eighth
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_8tC");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
        assert!(modifier.duration_triplet);
        assert_eq!(remaining, "C");

        // Test '_4.C - C pushed by dotted quarter
        let (modifier, remaining) = Chart::extract_leading_push_modifiers("'_4.C");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
        assert!(modifier.duration_dotted);
        assert_eq!(remaining, "C");
    }

    #[test]
    fn test_duration_pull_parsing_ambiguous_cases() {
        // Test 4'_4 - scale degree 4 pulled by quarter note
        let (chord, modifier) = Chart::extract_trailing_pull_modifiers("4'_4");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
        assert_eq!(chord, "4");

        // Test C'_8 - C pulled by eighth note
        let (chord, modifier) = Chart::extract_trailing_pull_modifiers("C'_8");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
        assert_eq!(chord, "C");

        // Test C'_8t - C pulled by triplet eighth
        let (chord, modifier) = Chart::extract_trailing_pull_modifiers("C'_8t");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Eighth);
        assert!(modifier.duration_triplet);
        assert_eq!(chord, "C");

        // Test C'_4. - C pulled by dotted quarter
        let (chord, modifier) = Chart::extract_trailing_pull_modifiers("C'_4.");
        assert!(modifier.duration.is_some());
        assert_eq!(modifier.duration.unwrap(), LilySyntax::Quarter);
        assert!(modifier.duration_dotted);
        assert_eq!(chord, "C");
    }

    #[test]
    fn test_accent_prefix_parsing() {
        use crate::chart::commands::Command;
        use crate::sections::SectionType;

        // Test that >C parses as C with an accent command
        let input = r#"
Accent Test
120bpm 4/4 #C

VS
>C
"#;
        let chart = parse_chart(input).expect("Should parse");
        let measures: Vec<_> = chart
            .sections
            .iter()
            .filter(|s| {
                !s.section.section_type.is_compact() && s.section.section_type != SectionType::End
            })
            .flat_map(|s| s.measures())
            .collect();

        assert!(!measures.is_empty(), "Should have at least one measure");
        let chords: Vec<_> = measures[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        assert!(
            !chords.is_empty(),
            "Should have at least one non-space chord"
        );

        let chord = &chords[0];
        assert_eq!(chord.full_symbol, "C", "Chord symbol should be C");
        assert!(
            chord.commands.iter().any(|c| matches!(c, Command::Accent)),
            "Chord should have accent command"
        );
    }

    #[test]
    fn test_accent_on_push_and_downbeat_parsing() {
        use crate::chart::commands::Command;
        use crate::sections::SectionType;

        // Test that >'C parses as AccentOnPush (accent on anticipation)
        // and '>C parses as Accent (accent on downbeat)
        // Based on real-world chart: Thriller - Dirty Loops
        let input = r#"
Thriller - Dirty Loops, Cory Wong
Transcribed By: Cody Wright
120bpm 4/4 #Ab
/push = triplet

COUNT 2

IN
r8t >Ab9_8t r8t r8t r8t >F9_8t r2 | s1

VS
>'F/C . Cm . 'F/C . Cm . 'F/C . Cm . 'F/C . Cm Cm9

CH
>Cm/Eb / 'Eb /// | 'Eb / 'F/C / 'Cm // | 'F/A //// | 'Fm9  ////
>Cm/Eb / 'Eb /// | 'Eb / 'F/C / 'Cm // | 'F/A | r8t >Ab9_8t r8t r8t >'F9_8t r8t r4 Fm/Ab_4 | s1

BR
'_4F7 | . |  Abmaj9 //// | // r8t Abmaj9_8t r8t Bb_8t r8t Cm7_8t | Cm7 | Ebmaj7/Bb | Am7b5 | Abmaj7 | G7sus4 | 'G7

VS
'F/C . Cm . 'F/C . Cm . 'F/C . Cm . 'F/C . Cm Cm9
"#;
        let chart = parse_chart(input).expect("Should parse");

        // Find VS section (first one after COUNT and IN)
        let verse_sections: Vec<_> = chart
            .sections
            .iter()
            .filter(|s| s.section.section_type == SectionType::Verse)
            .collect();
        assert!(!verse_sections.is_empty(), "Should have VS sections");

        // First VS measure should have >'F/C which is AccentOnPush
        let vs_measures = verse_sections[0].measures();
        let vs_chords: Vec<_> = vs_measures[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        assert!(!vs_chords.is_empty(), "VS should have non-space chords");

        let vs_chord = vs_chords[0];
        assert_eq!(vs_chord.full_symbol, "F/C", "VS chord should be F/C");
        assert!(
            vs_chord
                .commands
                .iter()
                .any(|c| matches!(c, Command::AccentOnPush)),
            ">'F/C should have AccentOnPush command (accent on anticipation)"
        );
        assert!(
            vs_chord.push_pull.is_some(),
            "VS chord should have push_pull"
        );

        // Find CH section to test both accent types
        let chorus_sections: Vec<_> = chart
            .sections
            .iter()
            .filter(|s| s.section.section_type == SectionType::Chorus)
            .collect();
        assert!(!chorus_sections.is_empty(), "Should have CH sections");

        // CH measure 7 (second-to-last) has >'F9 which is AccentOnPush
        // Input: r8t >Ab9_8t r8t r8t >'F9_8t r8t r4 Fm/Ab_4
        let ch_measures = chorus_sections[0].measures();
        assert!(ch_measures.len() >= 8, "CH should have at least 8 measures");

        let ch7_chords: Vec<_> = ch_measures[7]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
            .collect();

        // Should have Ab9, F9, and Fm/Ab
        assert!(
            ch7_chords.len() >= 2,
            "CH measure 7 should have at least 2 chords"
        );

        // Ab9 has regular accent (>Ab9)
        let ab9_chord = ch7_chords.iter().find(|c| c.full_symbol == "Ab9");
        assert!(ab9_chord.is_some(), "Should have Ab9 chord");
        let ab9 = ab9_chord.unwrap();
        assert!(
            ab9.commands.iter().any(|c| matches!(c, Command::Accent)),
            ">Ab9 should have regular Accent (no push)"
        );

        // F9 has AccentOnPush (>'F9 with push)
        let f9_chord = ch7_chords.iter().find(|c| c.full_symbol == "F9");
        assert!(f9_chord.is_some(), "Should have F9 chord");
        let f9 = f9_chord.unwrap();
        assert!(
            f9.commands
                .iter()
                .any(|c| matches!(c, Command::AccentOnPush)),
            ">'F9 should have AccentOnPush (accent before push marker)"
        );
        assert!(f9.push_pull.is_some(), "F9 should have push_pull");
    }

    #[test]
    fn test_accent_not_in_chord_memory() {
        use crate::chart::commands::Command;
        use crate::sections::SectionType;

        // With the new chord memory behavior:
        // - Basic chords (C, Cm) don't participate in memory at all
        // - Writing "C" gives "C" (basic major), NOT a recalled Cmaj7
        // This test verifies that accented chords don't bleed accents into basic chords
        let input = r#"
Accent Memory Test
120bpm 4/4 #C

VS
>Cmaj7 | C D E F
"#;
        let chart = parse_chart(input).expect("Should parse");

        // Get all verse sections
        let verse_sections: Vec<_> = chart
            .sections
            .iter()
            .filter(|s| s.section.section_type == SectionType::Verse)
            .collect();

        eprintln!(
            "All sections: {:?}",
            chart
                .sections
                .iter()
                .map(|s| format!("{:?}", s.section.section_type))
                .collect::<Vec<_>>()
        );
        eprintln!("Verse sections: {}", verse_sections.len());

        assert!(
            !verse_sections.is_empty(),
            "Should have at least 1 verse section"
        );

        // VS measure 0: >Cmaj7 should have accent
        let vs_measures = verse_sections[0].measures();
        assert!(vs_measures.len() >= 2, "VS should have at least 2 measures");

        let m0_chords: Vec<_> = vs_measures[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        assert!(!m0_chords.is_empty(), "M0 should have non-space chords");

        // First chord: >Cmaj7 should have accent
        let first_chord = m0_chords[0];
        assert_eq!(first_chord.full_symbol, "Cmaj7");
        assert!(
            first_chord
                .commands
                .iter()
                .any(|c| matches!(c, Command::Accent)),
            "First chord should have accent"
        );

        // VS measure 1: C (basic major) should recall Cmaj7 from major family memory
        // Basic chords CAN recall but don't store - split memory by chord family
        let m1_chords: Vec<_> = vs_measures[1]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        assert!(!m1_chords.is_empty(), "M1 should have non-space chords");

        let second_chord = m1_chords[0];
        // Basic chords DO recall from their family's memory
        assert_eq!(
            second_chord.full_symbol, "Cmaj7",
            "Basic C should recall Cmaj7 from major family memory"
        );
        // But the accent is NOT recalled (it's a modifier on the chord instance, not stored in memory)
        assert!(
            !second_chord
                .commands
                .iter()
                .any(|c| matches!(c, Command::Accent)),
            "Recalled chord should NOT have accent from memory. Commands: {:?}",
            second_chord.commands
        );
    }

    #[test]
    fn test_accent_preserved_in_section_template() {
        use crate::chart::commands::Command;
        use crate::sections::SectionType;

        // Accents CAN be committed to section memory (templates).
        // If we define VS with >C, then recall VS, the accent should be preserved.
        let input = r#"
Accent Template Test
120bpm 4/4 #C

VS
>C D E F

VS
"#;
        let chart = parse_chart(input).expect("Should parse");
        let verse_sections: Vec<_> = chart
            .sections
            .iter()
            .filter(|s| s.section.section_type == SectionType::Verse)
            .collect();

        assert_eq!(verse_sections.len(), 2, "Should have 2 verse sections");

        // VS1 (template definition): >C should have accent
        let vs1_chords: Vec<_> = verse_sections[0].measures()[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        assert!(!vs1_chords.is_empty());
        assert!(
            vs1_chords[0]
                .commands
                .iter()
                .any(|c| matches!(c, Command::Accent)),
            "VS1 first chord should have accent"
        );

        // VS2 (recalled from template): should also have accent on first chord
        let vs2_chords: Vec<_> = verse_sections[1].measures()[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s")
            .collect();
        assert!(!vs2_chords.is_empty());
        assert!(
            vs2_chords[0]
                .commands
                .iter()
                .any(|c| matches!(c, Command::Accent)),
            "VS2 recalled chord should have accent - templates preserve commands"
        );
    }

    #[test]
    fn test_push_duration_then_accent() {
        use crate::chart::commands::Command;

        // '_4>F7 = push with quarter duration, then accent (accent on downbeat)
        // >'_4F7 = accent before push (accent on pushed beat)
        let input = r#"
Test
120bpm 4/4

BR
'_4>F7 | >'_4G7 |
"#;
        let chart = parse_chart(input).expect("Should parse");

        // Find BR section
        let br_sections: Vec<_> = chart
            .sections
            .iter()
            .filter(|s| s.section.section_type == crate::sections::SectionType::Bridge)
            .collect();
        assert!(!br_sections.is_empty(), "Should have BR section");

        let measures = br_sections[0].measures();

        // Measure 0: '_4>F7 - accent AFTER push = regular Accent (downbeat)
        let m0_chords: Vec<_> = measures[0]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
            .collect();
        assert!(!m0_chords.is_empty(), "Measure 0 should have chords");
        let f7 = &m0_chords[0];
        eprintln!(
            "F7 chord: symbol='{}' commands={:?} push_pull={:?}",
            f7.full_symbol, f7.commands, f7.push_pull
        );
        assert_eq!(f7.full_symbol, "F7", "Should be F7");
        assert!(f7.push_pull.is_some(), "F7 should have push");
        assert!(
            f7.commands.iter().any(|c| matches!(c, Command::Accent)),
            "'_4>F7 should have Accent (accent after push = downbeat). Got: {:?}",
            f7.commands
        );
        assert!(
            !f7.commands
                .iter()
                .any(|c| matches!(c, Command::AccentOnPush)),
            "'_4>F7 should NOT have AccentOnPush. Got: {:?}",
            f7.commands
        );

        // Measure 1: >'_4G7 - accent BEFORE push = AccentOnPush (pushed beat)
        let m1_chords: Vec<_> = measures[1]
            .chords
            .iter()
            .filter(|c| c.full_symbol != "s" && c.full_symbol != "r")
            .collect();
        assert!(!m1_chords.is_empty(), "Measure 1 should have chords");
        let g7 = &m1_chords[0];
        eprintln!(
            "G7 chord: symbol='{}' commands={:?} push_pull={:?}",
            g7.full_symbol, g7.commands, g7.push_pull
        );
        assert_eq!(g7.full_symbol, "G7", "Should be G7");
        assert!(g7.push_pull.is_some(), "G7 should have push");
        assert!(
            g7.commands
                .iter()
                .any(|c| matches!(c, Command::AccentOnPush)),
            ">'_4G7 should have AccentOnPush (accent before push = pushed beat). Got: {:?}",
            g7.commands
        );
        assert!(
            !g7.commands.iter().any(|c| matches!(c, Command::Accent)),
            ">'_4G7 should NOT have regular Accent. Got: {:?}",
            g7.commands
        );
    }
}

// endregion: --- Tests
