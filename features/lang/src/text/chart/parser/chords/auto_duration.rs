//! Even split of the chords between `|` separators into explicit durations.

use super::tokens::tokenize_with_spans;
use crate::text::chart::parser::ChartParser;

impl<'a> ChartParser<'a> {
    /// Apply automatic durations to chords between measure separators
    /// If chords are between | separators, split the measure evenly
    /// Examples:
    ///   "| G C |" → "| G_2 C_2 |" (2 chords = half notes each)
    ///   "G C | D" → "G_2 C_2 | D_1" (2 chords before | = half notes, 1 after = whole)
    ///   "G C E D | A" → "G_4 C_4 E_4 D_4 | A_1" (4 chords before | = quarter notes, 1 after = whole)
    pub(in crate::text::chart::parser) fn apply_auto_durations_between_separators(
        line: &str,
        beats_per_measure: f64,
    ) -> String {
        use keyflow_proto::chart::commands::Command;

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
            let token_spans = tokenize_with_spans(segment);
            let tokens: Vec<&str> = token_spans
                .iter()
                .map(|span| &segment[span.as_range()])
                .collect();

            // Build a mask of which tokens are inside m{...} melody blocks
            // so we can skip them for chord counting and duration assignment
            let mut in_melody = false;
            let melody_mask: Vec<bool> = tokens
                .iter()
                .map(|t| {
                    if t.starts_with("m{") {
                        in_melody = true;
                        return true; // the m{ token itself is part of the melody
                    }
                    if in_melody {
                        if t.contains('}') {
                            in_melody = false;
                        }
                        return true;
                    }
                    false
                })
                .collect();

            // Mask annotation tokens that carry no rhythmic duration:
            // `dyn <level> [placement]` and `hairpin <dir> <span> [placement]`.
            // Like melody blocks, these must not be counted as chords or receive
            // auto-durations, or they steal beats from the real chords and
            // inflate the measure count (e.g. `dyn mp C#m //.` -> two measures).
            let annotation_mask: Vec<bool> = {
                let mut mask = vec![false; tokens.len()];
                let mut i = 0;
                while i < tokens.len() {
                    let consumed = match tokens[i] {
                        "dyn" | "dynamic" => {
                            // keyword + level + optional placement
                            let mut n = 1;
                            if i + n < tokens.len() {
                                n += 1;
                            }
                            if i + n < tokens.len()
                                && Self::parse_placement(tokens[i + n]).is_some()
                            {
                                n += 1;
                            }
                            n
                        }
                        "hairpin" => {
                            // keyword + direction + span + optional placement
                            let mut n = 1;
                            if i + n < tokens.len() {
                                n += 1;
                            }
                            if i + n < tokens.len() {
                                n += 1;
                            }
                            if i + n < tokens.len()
                                && Self::parse_placement(tokens[i + n]).is_some()
                            {
                                n += 1;
                            }
                            n
                        }
                        _ => 0,
                    };
                    if consumed > 0 {
                        for slot in mask.iter_mut().skip(i).take(consumed) {
                            *slot = true;
                        }
                        i += consumed;
                    } else {
                        i += 1;
                    }
                }
                mask
            };

            // Check if segment has standalone slashes (e.g., "/", "//", "///", "////")
            // If so, don't apply auto-duration - the slashes provide duration info
            let has_standalone_slashes = tokens
                .iter()
                .any(|t| t.chars().all(|c| c == '/') && !t.is_empty());
            let has_chord_length_directive = tokens
                .iter()
                .any(|t| *t == "\\ChordLength" || *t == "\\duration" || *t == "\\Duration");

            let chord_count = tokens
                .iter()
                .enumerate()
                .filter(|(i, t)| {
                    // Skip tokens inside melody blocks or annotation sequences
                    if melody_mask[*i] || annotation_mask[*i] {
                        return false;
                    }
                    // Count as chord if it's not a command, cue, or other special token.
                    // Dot repeats ARE counted - they occupy time in the measure.
                    // `$name` melody-variable recall is NOT a chord — it shouldn't
                    // shrink the chord-duration share for the real chords in the bar.
                    !t.starts_with('\\')
                        && !t.starts_with('/')
                        && !t.starts_with('@')
                        && !t.starts_with('"')
                        && !t.starts_with('$')
                        && **t != "%"
                        && Command::parse_stop_token(t).is_none()
                })
                .count();

            // Check if segment has melody blocks — don't apply auto-duration to melody measures
            // as it would make the chord look like explicit rhythm notation
            let has_melody_block = melody_mask.iter().any(|&m| m);

            // Count chords WITHOUT explicit durations (these need auto-duration)
            // If segment has standalone slashes or melody blocks, skip auto-duration entirely
            let chords_needing_duration =
                if has_standalone_slashes || has_melody_block || has_chord_length_directive {
                    0 // Don't apply auto-duration when slashes or melodies are present
                } else {
                    tokens
                        .iter()
                        .enumerate()
                        .filter(|(i, t)| {
                            // Skip tokens inside melody blocks or annotation sequences
                            if melody_mask[*i] || annotation_mask[*i] {
                                return false;
                            }
                            !t.starts_with('\\')
                            && !t.starts_with('/')
                            && !t.starts_with('@')
                            && !t.starts_with('"')
                            && !t.starts_with('$')
                            && **t != "%"
                            && !t.contains('_')
                            && **t != "." // Dot repeats handle their own duration
                            && Command::parse_stop_token(t).is_none()
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
            for (tok_idx, token) in tokens.iter().enumerate() {
                if !segment_result.is_empty() {
                    segment_result.push(' ');
                }

                // Keep melody-block and annotation tokens as-is (no duration mangling)
                if melody_mask[tok_idx] || annotation_mask[tok_idx] {
                    segment_result.push_str(token);
                    continue;
                }

                // Check if this is a chord (not a command, cue, dot repeat, stop token,
                // or `$name` melody-variable recall — those must round-trip unchanged).
                let is_dot_repeat = *token == ".";
                let is_measure_repeat = *token == "%";
                let is_stop_token = Command::parse_stop_token(token).is_some();
                // `^"…"` and `_"…"` are staff text, not chords. Appending a
                // duration to one leaves `^"Back to top"_2`, which no longer
                // ends in a quote and so stops being staff text at all — the
                // cue was silently dropped from any bar that also carried an
                // explicit duration.
                let is_annotation =
                    token.starts_with('"') || token.starts_with("^\"") || token.starts_with("_\"");
                if !token.starts_with('\\')
                    && !token.starts_with('/')
                    && !token.starts_with('@')
                    && !is_annotation
                    && !token.starts_with('$')
                    && !is_dot_repeat
                    && !is_measure_repeat
                    && !is_stop_token
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
}
