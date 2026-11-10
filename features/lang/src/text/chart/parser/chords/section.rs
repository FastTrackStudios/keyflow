//! A section's content lines: directives, cues, melody blocks and the
//! chord lines they decorate, and the `parse_chord_line*` entry points.

use super::tokens::tokenize_with_spans;
use crate::text::chart::cues::TextCue;
use crate::text::chart::dynamics::DynamicMarking;
use crate::text::chart::melody::{Melody, MelodyNote};
use crate::text::chart::parser::ChartParser;
use crate::text::chart::types::Measure;
use crate::text::chord::ChordRhythm;
use crate::text::sections::SectionType;
use crate::text::time::{MusicalDuration, TimeSignature, TimeSignatureExt};
use keyflow_proto::chart::{Dynamic, Hairpin, StaffText};

impl<'a> ChartParser<'a> {
    /// Parse section content lines into measures
    pub(in crate::text::chart::parser) fn parse_section_measures(
        &mut self,
        lines: &[&str],
        section_type: &SectionType,
        section_measure_count: Option<usize>,
    ) -> Result<Vec<Measure>, String> {
        let mut measures: Vec<Measure> = Vec::new();
        let mut pending_cues: Vec<TextCue> = Vec::new();
        let mut pending_dynamics: Vec<DynamicMarking> = Vec::new();
        let mut pending_classical_dynamics: Vec<Dynamic> = Vec::new();
        let mut pending_hairpins: Vec<Hairpin> = Vec::new();
        let mut pending_staff_text: Vec<StaffText> = Vec::new();

        // Seed each section from the chart-wide `\Duration` default (if any).
        // A `\Duration` inside the section overrides these below.
        let mut section_chord_length: Option<(ChordRhythm, MusicalDuration)> =
            self.default_duration.as_ref().and_then(|v| {
                Self::parse_chord_length_value(
                    v,
                    self.time_signature.unwrap_or(TimeSignature::common_time()),
                )
            });
        let mut section_melody_duration: Option<String> = self.default_duration.clone();
        let mut section_melody_octave: Option<u8> = self.melody_octave_memory;
        let logical_lines = Self::join_multiline_parallel_containers(lines);

        for line in &logical_lines {
            let line = line.as_str();
            let trimmed = line.trim();

            if let Some((name, value)) = Self::parse_alias_declaration(trimmed) {
                self.aliases.insert(name, value);
                continue;
            }

            if let Some(value) = trimmed
                .strip_prefix("\\duration ")
                .or_else(|| trimmed.strip_prefix("\\Duration "))
            {
                let value = value.trim();
                section_chord_length = Self::parse_chord_length_value(
                    value,
                    self.time_signature.unwrap_or(TimeSignature::common_time()),
                );
                section_melody_duration = Some(value.to_string());
                continue;
            }

            if let Some(value) = trimmed
                .strip_prefix("\\octave ")
                .or_else(|| trimmed.strip_prefix("\\Octave "))
            {
                let value = value.trim();
                if let Some((octave, melody_block)) = value.split_once(char::is_whitespace) {
                    if melody_block.trim_start().starts_with("m{")
                        || melody_block.trim_start().starts_with("m {")
                    {
                        let inline_octave = octave.parse::<u8>().ok().or(section_melody_octave);
                        match Melody::parse_block_with_defaults(
                            melody_block.trim(),
                            section_melody_duration.as_deref(),
                            inline_octave,
                        ) {
                            Ok((name, melody)) => {
                                if let Some(var_name) = name {
                                    self.melody_variables.set(var_name, melody);
                                } else {
                                    if let Some(octave) = Self::last_melody_context_octave(&melody)
                                    {
                                        section_melody_octave = Some(octave);
                                    }
                                    let time_sig =
                                        self.time_signature.unwrap_or(TimeSignature::common_time());
                                    let mut measure = Self::fresh_measure(time_sig);
                                    measure.melodies.push(melody);
                                    let mut melody_measures =
                                        self.split_long_melody_branch_measures(vec![measure]);

                                    if !pending_cues.is_empty() && !melody_measures.is_empty() {
                                        melody_measures[0].text_cues.append(&mut pending_cues);
                                    }
                                    if !pending_dynamics.is_empty() && !melody_measures.is_empty() {
                                        melody_measures[0].dynamics.append(&mut pending_dynamics);
                                    }
                                    if !pending_staff_text.is_empty() && !melody_measures.is_empty()
                                    {
                                        melody_measures[0]
                                            .staff_text
                                            .append(&mut pending_staff_text);
                                    }
                                    if !pending_classical_dynamics.is_empty()
                                        && !melody_measures.is_empty()
                                    {
                                        melody_measures[0]
                                            .classical_dynamics
                                            .append(&mut pending_classical_dynamics);
                                    }
                                    if !pending_hairpins.is_empty() && !melody_measures.is_empty() {
                                        melody_measures[0].hairpins.append(&mut pending_hairpins);
                                    }

                                    measures.extend(melody_measures);
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to parse melody block '{}': {}",
                                    melody_block.trim(),
                                    e
                                );
                            }
                        }
                        continue;
                    }
                }
                section_melody_octave = value.parse::<u8>().ok();
                continue;
            }

            if let Some(value) = trimmed.strip_prefix("\\ChordLength ") {
                section_chord_length = Self::parse_chord_length_value(
                    value.trim(),
                    self.time_signature.unwrap_or(TimeSignature::common_time()),
                );
                continue;
            }

            if let Some(staff_text) = self.parse_standalone_staff_text_line(trimmed) {
                pending_staff_text.extend(staff_text);
                continue;
            }

            if let Some(dynamic) = Self::parse_classical_dynamic_line(trimmed) {
                pending_classical_dynamics.push(dynamic);
                continue;
            }

            if let Some(hairpin) = Self::parse_hairpin_line(trimmed) {
                pending_hairpins.push(hairpin);
                continue;
            }

            // Check for melody variable definition (e.g., "mainRiff = m{ C_8 D_8 E_4 }")
            if trimmed.contains("= m{")
                || trimmed.contains("= m {")
                || trimmed.starts_with("m{")
                || trimmed.starts_with("m {")
            {
                match Melody::parse_block_with_defaults(
                    trimmed,
                    section_melody_duration.as_deref(),
                    section_melody_octave,
                ) {
                    Ok((name, melody)) => {
                        if let Some(var_name) = name {
                            // Store as a variable
                            self.melody_variables.set(var_name, melody);
                        } else {
                            if let Some(octave) = Self::last_melody_context_octave(&melody) {
                                section_melody_octave = Some(octave);
                            }
                            let time_sig =
                                self.time_signature.unwrap_or(TimeSignature::common_time());
                            let mut measure = Self::fresh_measure(time_sig);
                            measure.melodies.push(melody);
                            let mut melody_measures =
                                self.split_long_melody_branch_measures(vec![measure]);

                            if !pending_cues.is_empty() && !melody_measures.is_empty() {
                                melody_measures[0].text_cues.append(&mut pending_cues);
                            }
                            if !pending_dynamics.is_empty() && !melody_measures.is_empty() {
                                melody_measures[0].dynamics.append(&mut pending_dynamics);
                            }
                            if !pending_staff_text.is_empty() && !melody_measures.is_empty() {
                                melody_measures[0]
                                    .staff_text
                                    .append(&mut pending_staff_text);
                            }
                            if !pending_classical_dynamics.is_empty() && !melody_measures.is_empty()
                            {
                                melody_measures[0]
                                    .classical_dynamics
                                    .append(&mut pending_classical_dynamics);
                            }
                            if !pending_hairpins.is_empty() && !melody_measures.is_empty() {
                                melody_measures[0].hairpins.append(&mut pending_hairpins);
                            }

                            measures.extend(melody_measures);
                        }
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
            } else if trimmed.starts_with('<')
                && !trimmed.starts_with("<<")
                && trimmed.contains('>')
                && !tokenize_with_spans(trimmed)
                    .first()
                    .and_then(|span| {
                        trimmed[span.as_range()]
                            .strip_prefix('<')
                            .and_then(|token| token.strip_suffix('>'))
                    })
                    .map(|name| self.aliases.contains_key(name))
                    .unwrap_or(false)
                && !self
                    .expand_aliases_in_line(trimmed)
                    .trim_start()
                    .starts_with("<<")
            {
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
                //
                // Positions computed inside the line parse (key changes) are
                // line-relative; publish the beats already consumed by earlier
                // lines of this section so they come out section-relative.
                self.section_beats_offset =
                    measures.iter().map(|m| f64::from(m.time_signature.0)).sum();
                self.carried_measure = measures.last().cloned();
                // A `->140bpm` inside the line records the bar it falls in
                // counted from the line's start; shifted below to the
                // section's count, which is what a reader of the chart means.
                let tempo_changes_before = self.tempo_changes.len();
                let mut line_measures = self.parse_chord_line_with_default_chord_length(
                    line,
                    section_type,
                    section_measure_count,
                    section_chord_length.clone(),
                    section_melody_duration.clone(),
                    section_melody_octave,
                )?;

                // If we have pending cues, attach them to the first new measure
                if !pending_cues.is_empty() && !line_measures.is_empty() {
                    line_measures[0].text_cues.append(&mut pending_cues);
                }

                // If we have pending dynamics, attach them to the first new measure
                if !pending_dynamics.is_empty() && !line_measures.is_empty() {
                    line_measures[0].dynamics.append(&mut pending_dynamics);
                }
                if !pending_staff_text.is_empty() && !line_measures.is_empty() {
                    line_measures[0].staff_text.append(&mut pending_staff_text);
                }
                if !pending_classical_dynamics.is_empty() && !line_measures.is_empty() {
                    line_measures[0]
                        .classical_dynamics
                        .append(&mut pending_classical_dynamics);
                }
                if !pending_hairpins.is_empty() && !line_measures.is_empty() {
                    line_measures[0].hairpins.append(&mut pending_hairpins);
                }

                let line_base = i32::try_from(measures.len()).unwrap_or(i32::MAX);
                for change in &mut self.tempo_changes[tempo_changes_before..] {
                    change.position.total_duration.measure += line_base;
                }
                measures.extend(line_measures);
            }
        }

        self.melody_octave_memory = section_melody_octave;
        self.section_beats_offset = 0.0;
        self.carried_measure = None;
        Ok(measures)
    }

    fn last_melody_context_octave(melody: &Melody) -> Option<u8> {
        melody
            .notes
            .iter()
            .rev()
            .find(|note| !note.is_rest() && !note.is_space())
            .and_then(Self::lowest_octave_in_note)
    }

    fn lowest_octave_in_note(note: &MelodyNote) -> Option<u8> {
        let mut lowest = note.octave;
        for (_, octave) in &note.extra_pitches {
            if let Some(octave) = octave {
                lowest = Some(lowest.map_or(*octave, |current| current.min(*octave)));
            }
        }
        lowest
    }

    /// Parse a line of chords into measures.
    ///
    /// Spans recorded for each chord token are byte-relative to `line` (with
    /// line/column = 0). When a parser is processing a sub-slice of a larger
    /// source line (parallel containers, repeats), use
    /// [`Self::parse_chord_line_with_offset`] to shift those spans into the
    /// original-line coordinate system.
    ///
    /// Test-only: production parses through
    /// `parse_chord_line_with_default_chord_length`. Kept as a convenience
    /// entry point for the parser unit tests.
    #[cfg(test)]
    pub(in crate::text::chart::parser) fn parse_chord_line(
        &mut self,
        line: &str,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
    ) -> Result<Vec<Measure>, String> {
        self.parse_chord_line_with_offset(line, section_type, section_measure_count, 0)
    }

    pub(in crate::text::chart::parser) fn parse_chord_line_with_default_chord_length(
        &mut self,
        line: &str,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
        default_chord_length: Option<(ChordRhythm, MusicalDuration)>,
        default_melody_duration: Option<String>,
        default_melody_octave: Option<u8>,
    ) -> Result<Vec<Measure>, String> {
        self.parse_chord_line_inner(
            line,
            section_type,
            section_measure_count,
            0,
            default_chord_length,
            default_melody_duration,
            default_melody_octave,
        )
    }

    /// Same as [`Self::parse_chord_line`] but shifts every emitted token span
    /// by `line_byte_offset` so spans line up with the surrounding source.
    ///
    /// Test-only: only reached via [`Self::parse_chord_line`].
    #[cfg(test)]
    #[allow(clippy::too_many_lines)]
    pub(in crate::text::chart::parser) fn parse_chord_line_with_offset(
        &mut self,
        line: &str,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
        line_byte_offset: usize,
    ) -> Result<Vec<Measure>, String> {
        self.parse_chord_line_inner(
            line,
            section_type,
            section_measure_count,
            line_byte_offset,
            None,
            None,
            None,
        )
    }
}
