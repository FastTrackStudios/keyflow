//! Parallel containers (`<< ... >>`): joining multi-line containers,
//! splitting branches and merging their measures.

use crate::text::chart::melody::Melody;
use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::helpers::RepeatCount;
use crate::text::chart::types::Measure;
use crate::text::chord::ChordRhythm;
use crate::text::sections::SectionType;
use crate::text::time::{MusicalDuration, TimeSignature, TimeSignatureExt};

impl<'a> ChartParser<'a> {
    pub(in crate::text::chart::parser) fn join_multiline_parallel_containers(
        lines: &[&str],
    ) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = String::new();
        let mut parallel_depth = 0usize;
        let mut let_block_depth = 0usize;

        for line in lines {
            let trimmed = line.trim();
            if parallel_depth == 0 && let_block_depth == 0 {
                current.clear();
            } else if !current.is_empty() {
                if let_block_depth > 0 {
                    current.push('\n');
                } else {
                    current.push(' ');
                }
            }
            current.push_str(trimmed);
            parallel_depth = parallel_depth_for_line(parallel_depth, trimmed);
            let_block_depth = let_block_depth_for_line(let_block_depth, trimmed);

            if parallel_depth == 0 && let_block_depth == 0 {
                out.push(std::mem::take(&mut current));
            }
        }

        if !current.trim().is_empty() {
            out.push(current);
        }

        out
    }

    pub(super) fn normalize_parallel_container_syntax(input: &str) -> String {
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

    pub(super) fn parse_parallel_chord_line(
        &mut self,
        line: &str,
        repeat_count: RepeatCount,
        section_type: &SectionType,
        _section_measure_count: Option<usize>,
        line_byte_offset: usize,
        default_chord_length: Option<(ChordRhythm, MusicalDuration)>,
        default_melody_duration: Option<String>,
        default_melody_octave: Option<u8>,
    ) -> Result<Vec<Measure>, String> {
        let separated = Self::separate_parallel_blocks(line);
        let line = separated.as_str();
        let trimmed_line = line.trim();
        if trimmed_line.starts_with("<<")
            && trimmed_line.ends_with(">>")
            && Self::top_level_blocks(trimmed_line) == 1
        {
            let trim_left = line.len() - line.trim_start().len();
            let mut measures = self.parse_parallel_sequence(
                trimmed_line,
                section_type,
                line_byte_offset + trim_left,
                default_chord_length,
                default_melody_duration.clone(),
                default_melody_octave,
            )?;
            if let RepeatCount::Fixed(count) = repeat_count {
                if count > 1 {
                    if let Some(last) = measures.last_mut() {
                        last.repeat_count = count;
                    }
                }
            }
            return Ok(measures);
        }

        let measure_parts = Self::split_top_level_measures_spanned(line);
        let mut measures = Vec::new();

        for (part_offset, part) in measure_parts {
            let trimmed_full = part.as_str();
            let trim_left = trimmed_full.len() - trimmed_full.trim_start().len();
            let trimmed = trimmed_full.trim();
            if trimmed.is_empty() {
                continue;
            }
            let measure_offset = line_byte_offset + part_offset + trim_left;

            if trimmed.starts_with("<<") && trimmed.ends_with(">>") {
                measures.push(self.parse_parallel_measure(
                    trimmed,
                    section_type,
                    measure_offset,
                    default_chord_length.clone(),
                    default_melody_duration.clone(),
                    default_melody_octave,
                )?);
            } else {
                // Handing a bar that still holds a block back to the line
                // parser would land here again, forever.
                if trimmed.contains("<<") && trimmed.contains(">>") {
                    return Err(format!(
                        "a `<< … >>` block is a bar of its own: separate it from \
                         `{}` with `|`",
                        trimmed
                    ));
                }
                let parsed = self.parse_chord_line_inner(
                    trimmed,
                    section_type,
                    Some(1),
                    measure_offset,
                    default_chord_length.clone(),
                    default_melody_duration.clone(),
                    default_melody_octave,
                )?;
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

    /// Top-level `<< … >>` blocks in `line` (quotes skipped).
    fn top_level_blocks(line: &str) -> usize {
        let b = line.as_bytes();
        let (mut depth, mut count, mut in_quote, mut i) = (0usize, 0usize, false, 0usize);
        while i < b.len() {
            match b[i] {
                b'"' => in_quote = !in_quote,
                b'<' if !in_quote && b.get(i + 1) == Some(&b'<') => {
                    if depth == 0 {
                        count += 1;
                    }
                    depth += 1;
                    i += 1;
                }
                b'>' if !in_quote && b.get(i + 1) == Some(&b'>') => {
                    depth = depth.saturating_sub(1);
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }
        count
    }

    /// The two natural ways to write blocks without bar lines, spelled the
    /// way the parser reads them: back-to-back blocks (`<< … >> << … >>`)
    /// are separate bars, and staff text written just before a block
    /// (`^"Band In" << … >>`) goes inside it, on its chord lane. Positions
    /// shift only where a rewrite happened; spans in those lines are
    /// approximate.
    pub(super) fn separate_parallel_blocks(line: &str) -> String {
        if Self::top_level_blocks(line) == 0 {
            return line.to_string();
        }
        // Split into top-level pieces: blocks and the text between them.
        let b = line.as_bytes();
        let mut pieces: Vec<(bool, String)> = Vec::new();
        let (mut depth, mut in_quote, mut start, mut i) = (0usize, false, 0usize, 0usize);
        while i < b.len() {
            match b[i] {
                b'"' => in_quote = !in_quote,
                b'<' if !in_quote && b.get(i + 1) == Some(&b'<') => {
                    if depth == 0 {
                        pieces.push((false, line[start..i].to_string()));
                        start = i;
                    }
                    depth += 1;
                    i += 1;
                }
                b'>' if !in_quote && b.get(i + 1) == Some(&b'>') => {
                    depth = depth.saturating_sub(1);
                    i += 1;
                    if depth == 0 {
                        pieces.push((true, line[start..=i].to_string()));
                        start = i + 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        pieces.push((false, line[start..].to_string()));
        // Nothing but quoted notes: `^"Band In"`, `"quiet" ^"Build"`.
        let is_annotation = |t: &str| {
            let mut rest = t.trim();
            if rest.is_empty() {
                return false;
            }
            while !rest.is_empty() {
                rest = rest.strip_prefix('^').unwrap_or(rest);
                let Some(body) = rest.strip_prefix('"') else {
                    return false;
                };
                let Some(end) = body.find('"') else {
                    return false;
                };
                rest = body[end + 1..].trim_start();
            }
            true
        };
        let mut out = String::new();
        let mut changed = false;
        let mut pending_text: Option<String> = None;
        let mut last_was_block = false;
        for (k, (block, text)) in pieces.iter().enumerate() {
            if *block {
                if last_was_block {
                    out.push_str(" | ");
                    changed = true;
                }
                match pending_text.take() {
                    Some(note) => {
                        out.push_str("<< ");
                        out.push_str(note.trim());
                        out.push(' ');
                        out.push_str(text.trim_start_matches("<<").trim_start());
                    }
                    None => out.push_str(text),
                }
                last_was_block = true;
            } else if is_annotation(text) && pieces.get(k + 1).is_some_and(|(b, _)| *b) {
                pending_text = Some(text.clone());
                changed = true;
                if last_was_block {
                    out.push_str(" | ");
                    last_was_block = false;
                } else {
                    // leading space stays out; the block takes the note
                }
            } else {
                if !text.trim().is_empty() {
                    last_was_block = false;
                }
                out.push_str(text);
            }
        }
        if !changed {
            // Untouched lines keep their exact bytes, so spans stay exact.
            return line.to_string();
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn parse_parallel_sequence(
        &mut self,
        container_text: &str,
        section_type: &SectionType,
        container_byte_offset: usize,
        default_chord_length: Option<(ChordRhythm, MusicalDuration)>,
        default_melody_duration: Option<String>,
        default_melody_octave: Option<u8>,
    ) -> Result<Vec<Measure>, String> {
        let inner_with_caps = container_text
            .strip_prefix("<<")
            .and_then(|s| s.strip_suffix(">>"))
            .ok_or_else(|| format!("Invalid parallel container: {}", container_text))?;
        let inner = inner_with_caps.trim();
        let inner_lead = inner_with_caps.len() - inner_with_caps.trim_start().len();
        let inner_offset = container_byte_offset + 2 + inner_lead;

        let branches = Self::split_top_level_parallel_branches_spanned(inner);
        let mut merged: Vec<Measure> = Vec::new();

        for (branch_offset, branch) in branches {
            let trim_left = branch.len() - branch.trim_start().len();
            let trimmed = branch.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed = self.parse_chord_line_inner(
                trimmed,
                section_type,
                None,
                inner_offset + branch_offset + trim_left,
                default_chord_length.clone(),
                default_melody_duration.clone(),
                default_melody_octave,
            )?;
            let parsed = self.split_long_melody_branch_measures(parsed);

            if merged.len() < parsed.len() {
                let time_sig = self.time_signature.unwrap_or(TimeSignature::common_time());
                merged.resize_with(parsed.len(), || Self::fresh_measure(time_sig));
            }
            for (idx, branch_measure) in parsed.into_iter().enumerate() {
                Self::merge_parallel_branch_measure(&mut merged[idx], branch_measure);
            }
        }

        Ok(merged)
    }

    fn parse_parallel_measure(
        &mut self,
        measure_text: &str,
        section_type: &SectionType,
        measure_byte_offset: usize,
        default_chord_length: Option<(ChordRhythm, MusicalDuration)>,
        default_melody_duration: Option<String>,
        default_melody_octave: Option<u8>,
    ) -> Result<Measure, String> {
        let trimmed_outer = measure_text.trim();
        // Offset of `trimmed_outer` within `measure_text`.
        let outer_lead = measure_text.len() - measure_text.trim_start().len();

        let inner_with_caps = trimmed_outer
            .strip_prefix("<<")
            .and_then(|s| s.strip_suffix(">>"))
            .ok_or_else(|| format!("Invalid parallel container: {}", measure_text))?;
        let inner = inner_with_caps.trim();
        // Offset of `inner` within `measure_text`: outer trim + "<<" + inner trim.
        let inner_lead = inner_with_caps.len() - inner_with_caps.trim_start().len();
        let inner_offset = measure_byte_offset + outer_lead + 2 /* "<<" */ + inner_lead;

        let branches = Self::split_top_level_parallel_branches_spanned(inner);
        let time_sig = self.time_signature.unwrap_or(TimeSignature::common_time());
        let mut merged = Self::fresh_measure(time_sig);

        for (branch_offset, branch) in branches {
            let trim_left = branch.len() - branch.trim_start().len();
            let trimmed = branch.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parsed = self.parse_chord_line_inner(
                trimmed,
                section_type,
                Some(1),
                inner_offset + branch_offset + trim_left,
                default_chord_length.clone(),
                default_melody_duration.clone(),
                default_melody_octave,
            )?;
            let Some(branch_measure) = parsed.into_iter().next() else {
                continue;
            };

            Self::merge_parallel_branch_measure(&mut merged, branch_measure);
        }

        Ok(merged)
    }

    fn merge_parallel_branch_measure(merged: &mut Measure, branch_measure: Measure) {
        merged.chords.extend(branch_measure.chords);
        merged
            .rhythm_elements
            .extend(branch_measure.rhythm_elements);
        merged.rhythm_slashes.extend(branch_measure.rhythm_slashes);
        merged.text_cues.extend(branch_measure.text_cues);
        merged.staff_text.extend(branch_measure.staff_text);
        merged.figured_bass.extend(branch_measure.figured_bass);
        merged.dynamics.extend(branch_measure.dynamics);
        merged.melodies.extend(branch_measure.melodies);
        merged.repeat_count = merged.repeat_count.max(branch_measure.repeat_count);
        merged.time_signature = branch_measure.time_signature;
        if merged.source_span.is_none() {
            merged.source_span = branch_measure.source_span;
        }
    }

    pub(super) fn split_long_melody_branch_measures(&self, measures: Vec<Measure>) -> Vec<Measure> {
        if measures.len() != 1 {
            return measures;
        }

        let mut iter = measures.into_iter();
        let measure = iter.next().expect("one measure");
        if !measure.chords.is_empty()
            || !measure.rhythm_elements.is_empty()
            || !measure.figured_bass.is_empty()
            || !measure.staff_text.is_empty()
            || !measure.text_cues.is_empty()
            || measure.melodies.len() != 1
        {
            return vec![measure];
        }

        let time_sig = TimeSignature::new(
            measure.time_signature.0.into(),
            measure.time_signature.1.into(),
        );
        let beats_per_measure =
            f64::from(time_sig.numerator) * 4.0 / f64::from(time_sig.denominator);
        if beats_per_measure <= 0.0 {
            return vec![measure];
        }

        let melody = measure.melodies.into_iter().next().expect("one melody");
        let mut split = Vec::new();
        let mut current = Self::fresh_measure(time_sig);
        let mut current_notes = Vec::new();
        let mut current_beats = 0.0;

        for note in melody.notes {
            let note_beats = note.duration_beats();
            if !current_notes.is_empty() && current_beats + note_beats > beats_per_measure + 0.001 {
                current.melodies.push(Melody::with_notes(current_notes));
                split.push(current);
                current = Self::fresh_measure(time_sig);
                current_notes = Vec::new();
                current_beats = 0.0;
            }
            current_beats += note_beats;
            current_notes.push(note);

            if (current_beats - beats_per_measure).abs() < 0.001 {
                current.melodies.push(Melody::with_notes(current_notes));
                split.push(current);
                current = Self::fresh_measure(time_sig);
                current_notes = Vec::new();
                current_beats = 0.0;
            }
        }

        if !current_notes.is_empty() {
            current.melodies.push(Melody::with_notes(current_notes));
            split.push(current);
        }

        split
    }

    /// Like `split_top_level_measures`, but also returns the byte offset of
    /// each emitted part within `input`. Splits on `|` only at parallel-depth
    /// 0 and brace-depth 0, identical to the un-spanned version.
    /// Whitespace is preserved in the returned slice (callers re-trim and
    /// account for the leading whitespace when computing absolute offsets).
    pub(super) fn split_top_level_measures_spanned(input: &str) -> Vec<(usize, String)> {
        let mut parts: Vec<(usize, String)> = Vec::new();
        let mut current = String::new();
        let mut current_start: usize = 0;
        let mut last_byte_consumed: usize = 0;
        let mut parallel_depth = 0usize;
        let mut brace_depth = 0usize;
        let bytes = input.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            let b = bytes[i];
            // Two-byte tokens `<<` / `>>` first.
            if b == b'<' && bytes.get(i + 1) == Some(&b'<') {
                parallel_depth += 1;
                if current.is_empty() {
                    current_start = i;
                }
                current.push('<');
                current.push('<');
                i += 2;
                last_byte_consumed = i;
                continue;
            }
            if b == b'>' && bytes.get(i + 1) == Some(&b'>') {
                parallel_depth = parallel_depth.saturating_sub(1);
                if current.is_empty() {
                    current_start = i;
                }
                current.push('>');
                current.push('>');
                i += 2;
                last_byte_consumed = i;
                continue;
            }

            match b {
                b'{' => brace_depth += 1,
                b'}' => brace_depth = brace_depth.saturating_sub(1),
                b'|' if parallel_depth == 0 && brace_depth == 0 => {
                    if !current.trim().is_empty() {
                        parts.push((current_start, std::mem::take(&mut current)));
                    } else {
                        current.clear();
                    }
                    i += 1;
                    last_byte_consumed = i;
                    continue;
                }
                _ => {}
            }

            // Push character (handle multi-byte by slicing original input).
            let ch_end = next_utf8_boundary(input, i);
            if current.is_empty() {
                current_start = i;
            }
            current.push_str(&input[i..ch_end]);
            i = ch_end;
            last_byte_consumed = i;
        }
        let _ = last_byte_consumed;

        if !current.trim().is_empty() {
            parts.push((current_start, current));
        }
        parts
    }

    /// Like `split_top_level_parallel_branches`, but with byte offsets.
    /// Splits on `;` at brace-depth 0.
    pub(in crate::text::chart::parser) fn split_top_level_parallel_branches_spanned(
        input: &str,
    ) -> Vec<(usize, String)> {
        let mut parts: Vec<(usize, String)> = Vec::new();
        let mut current = String::new();
        let mut current_start: usize = 0;
        let mut brace_depth = 0usize;
        let bytes = input.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            let b = bytes[i];
            match b {
                b'{' => brace_depth += 1,
                b'}' => brace_depth = brace_depth.saturating_sub(1),
                b';' if brace_depth == 0 => {
                    if !current.trim().is_empty() {
                        parts.push((current_start, std::mem::take(&mut current)));
                    } else {
                        current.clear();
                    }
                    i += 1;
                    continue;
                }
                _ => {}
            }

            let ch_end = next_utf8_boundary(input, i);
            if current.is_empty() {
                current_start = i;
            }
            current.push_str(&input[i..ch_end]);
            i = ch_end;
        }

        if !current.trim().is_empty() {
            parts.push((current_start, current));
        }
        parts
    }
}

/// Find the byte index of the next UTF-8 char boundary at or after `start`.
/// `start` must already be a valid char boundary in `s`.
fn next_utf8_boundary(s: &str, start: usize) -> usize {
    let bytes = s.as_bytes();
    let mut end = start + 1;
    while end < bytes.len() && !s.is_char_boundary(end) {
        end += 1;
    }
    end
}

fn parallel_depth_for_line(mut depth: usize, line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut brace_depth = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                brace_depth += 1;
                i += 1;
            }
            b'}' => {
                brace_depth = brace_depth.saturating_sub(1);
                i += 1;
            }
            b'<' if brace_depth == 0 && bytes.get(i + 1) == Some(&b'<') => {
                depth += 1;
                i += 2;
            }
            b'>' if brace_depth == 0 && bytes.get(i + 1) == Some(&b'>') => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            _ => i += 1,
        }
    }
    depth
}

fn let_block_depth_for_line(mut depth: usize, line: &str) -> usize {
    if depth == 0 {
        let Some(rest) = line.trim_start().strip_prefix("let ") else {
            return 0;
        };
        let Some((_, value)) = rest.split_once('=') else {
            return 0;
        };
        if !value.trim_start().starts_with('{') {
            return 0;
        }
    }

    let mut in_quote = false;
    let mut escaped = false;
    for c in line.chars() {
        if in_quote {
            escaped = c == '\\' && !escaped;
            if c == '"' && !escaped {
                in_quote = false;
            }
            if c != '\\' {
                escaped = false;
            }
            continue;
        }

        match c {
            '"' => in_quote = true,
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}
