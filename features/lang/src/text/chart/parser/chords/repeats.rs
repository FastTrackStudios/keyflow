//! Phrase repeats (`xN`, `x^`), barred lines under a default chord length,
//! and `|:` / `:|` barline normalisation.

use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::helpers::RepeatCount;
use crate::text::chart::types::Measure;
use crate::text::chord::ChordRhythm;
use crate::text::sections::SectionType;
use crate::text::time::{MusicalDuration, MusicalPositionExt, TimeSignature};

impl<'a> ChartParser<'a> {
    /// Resolve a phrase's repeat count (explicit `xN` or auto `x^`) and expand
    /// the measure list accordingly. Split out of `parse_chord_line_inner`:
    /// it runs once after the token loop and touches none of the loop state,
    /// only the completed `measures` plus the prevailing meter and section
    /// context.
    pub(super) fn expand_phrase_repeats(
        &self,
        mut measures: Vec<Measure>,
        repeat_count: RepeatCount,
        time_sig: TimeSignature,
        beats_per_measure: f64,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
    ) -> Result<Vec<Measure>, String> {
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

                // Get section length in measures. Inference fallbacks (in order):
                //   1. Explicit count from the section header (e.g. `VS 16`).
                //   2. Most recent prior section of the same type (chart.sections).
                //   3. The chart's section_measure_memory (set by earlier sections).
                //   4. Bail out with the same error as before.
                let section_measures = if let Some(count) = section_measure_count {
                    count
                } else if let Some(prior_count) = self
                    .chart
                    .sections
                    .iter()
                    .rev()
                    .find(|s| s.section.section_type == *section_type)
                    .map(|s| s.measures().len())
                    .filter(|&n| n > 0)
                {
                    tracing::debug!(
                        "x^: inferred section length {} measures from prior {:?}",
                        prior_count,
                        section_type
                    );
                    prior_count
                } else if let Some(&remembered) = self
                    .chart
                    .section_measure_memory
                    .get(section_type)
                    .filter(|&&n| n > 0)
                {
                    tracing::debug!(
                        "x^: inferred section length {} measures from section_measure_memory",
                        remembered
                    );
                    remembered
                } else {
                    return Err(format!(
                        "Cannot use x^ without explicit section measure count \
                         (no prior {:?} section to infer from). Add a count: e.g. 'VS 16'",
                        section_type
                    ));
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

    pub(super) fn parse_barred_line_with_default_chord_length(
        &mut self,
        line: &str,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
        line_byte_offset: usize,
        default_chord_length: Option<(ChordRhythm, MusicalDuration)>,
        default_melody_duration: Option<String>,
        default_melody_octave: Option<u8>,
    ) -> Result<Vec<Measure>, String> {
        let mut measures: Vec<Measure> = Vec::new();
        let mut split_found_something = false;
        // Each bar parses on its own, so a `%` bar finds no measure before it
        // inside its own parse and falls back to `carried_measure`. That has to
        // be the bar just before it on this line, not the last bar of the line
        // above: `| Ebm | % |` repeats Ebm.
        let carried_into_line = self.carried_measure.clone();
        for (part_offset, part) in Self::split_top_level_measures_spanned(line) {
            let trim_left = part.len() - part.trim_start().len();
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            split_found_something = true;
            self.carried_measure = measures
                .last()
                .cloned()
                .or_else(|| carried_into_line.clone());
            let mut parsed = self.parse_chord_line_inner(
                trimmed,
                section_type,
                Some(1),
                line_byte_offset + part_offset + trim_left,
                default_chord_length.clone(),
                default_melody_duration.clone(),
                default_melody_octave,
            )?;
            measures.append(&mut parsed);
        }
        self.carried_measure = carried_into_line;

        // Retry the whole line only when the split found nothing to try. A
        // split that found bars and parsed no measures out of them is an
        // answer — an empty one — and re-entering here with the same line
        // lands straight back in this function. `\ChordLength /` over a line
        // whose only bar is `%` did exactly that, and recursed until the
        // stack ran out.
        if measures.is_empty() && !split_found_something {
            self.parse_chord_line_inner(
                line,
                section_type,
                section_measure_count,
                line_byte_offset,
                default_chord_length,
                default_melody_duration,
                default_melody_octave,
            )
        } else {
            Ok(measures)
        }
    }

    /// Rewrite the repeat barlines into the marker tokens the chord loop
    /// reads, keeping any count written on the closing one.
    ///
    /// `:|x4` and `:|4` both mean "play it four times", and both used to be
    /// dropped: `:|` was replaced first, leaving the count stuck to the
    /// barline that replaced it. The count is carried on the token instead,
    /// as `@repeat-end:4`.
    ///
    /// Note that a *line-level* `x4` is a different thing — it duplicates the
    /// measures — so the two spellings do not mean the same, and only the one
    /// attached to `:|` leaves the phrase written once.
    pub(super) fn normalize_repeat_barlines(input: &str) -> String {
        let mut out = String::with_capacity(input.len() + 16);
        let bytes = input.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"|:") {
                out.push_str("| @repeat-start ");
                i += 2;
                continue;
            }
            if bytes[i..].starts_with(b":|") {
                i += 2;

                // An optional count on the closing barline: `:|x4`, `:|4`.
                let mut j = i;
                if matches!(bytes.get(j), Some(b'x' | b'X')) {
                    j += 1;
                }
                let digits_start = j;
                while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                let count = &input[digits_start..j];
                if !count.is_empty() {
                    i = j;
                }

                out.push_str(" @repeat-end");
                if !count.is_empty() {
                    out.push(':');
                    out.push_str(count);
                }

                // One barline between two measures. A repeat that ends where
                // the next begins is written `:|:` (or `:|x4 |:`), and both
                // have to come out as a single `|` — two would parse as an
                // empty measure sitting between the repeats.
                let mut k = i;
                while matches!(bytes.get(k), Some(b' ' | b'\t')) {
                    k += 1;
                }
                let opens_next = bytes.get(k) == Some(&b':')
                    || (bytes[k..].starts_with(b"|:") && {
                        k += 1;
                        true
                    });
                if opens_next {
                    out.push_str(" | @repeat-start ");
                    i = k + 1;
                } else {
                    out.push_str(" |");
                }
                continue;
            }
            let ch = input[i..].chars().next().unwrap_or_default();
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }
}
