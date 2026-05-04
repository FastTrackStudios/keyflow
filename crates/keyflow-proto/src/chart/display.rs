use crate::chart::Chart;
use crate::chord::ChordRhythm;
use crate::time::TimeSignatureExt;

/// Represents a group of measures with optional repeat annotation
#[derive(Debug)]
struct MeasureGroup {
    start_idx: usize,
    count: usize,          // Repeat count (1 = no repeat)
    pattern_length: usize, // Number of measures in the pattern before repeat
}

/// Format rhythm notation for display
/// Default = "/" (single slash), Slashes = "///", Explicit = "_2." etc.
fn format_rhythm(rhythm: &ChordRhythm) -> String {
    use crate::core::duration::NoteValue;

    match rhythm {
        ChordRhythm::Default => "/".to_string(),
        ChordRhythm::Slashes {
            count,
            dotted,
            tied,
        } => {
            let mut s = "/".repeat(*count as usize);
            if *dotted {
                s.push('.');
            }
            if *tied {
                s.push('~');
            }
            s
        }
        ChordRhythm::Explicit(nd) => {
            // Convert NoteValue to lily notation (1, 2, 4, 8, 16, 32)
            let lily_val = match nd.note_value {
                NoteValue::Whole => 1,
                NoteValue::Half => 2,
                NoteValue::Quarter => 4,
                NoteValue::Eighth => 8,
                NoteValue::Sixteenth => 16,
                NoteValue::ThirtySecond => 32,
                NoteValue::SixtyFourth => 64,
            };

            let mut s = format!("_{}", lily_val);
            if nd.dots > 0 {
                s.push_str(&".".repeat(nd.dots as usize));
            }
            if let Some(tuplet) = &nd.tuplet {
                // Triplets (3) display as 't', other tuplets as ':n'
                if tuplet.numerator == 3 {
                    s.push('t');
                } else {
                    s.push_str(&format!(":{}", tuplet.numerator));
                }
            }
            s
        }
    }
}

// Plain text formatters (no colors for easier log reading)
mod colors {
    pub const RESET: &str = "";
    pub const BOLD: &str = "";
    pub const DIM: &str = "";
    pub const METADATA: &str = "";
    pub const SECTION: &str = "";
    pub const DURATION: &str = "";
    pub const CHORD: &str = "";
    pub const KEY_SIG: &str = "";
    pub const TIME_SIG: &str = "";
    pub const TEMPO: &str = "";
    pub const MEASURE_NUM: &str = "";
    pub const BORDER: &str = "";

    // String formatters that return plain strings
    pub fn format_metadata(s: &str) -> String {
        s.to_string()
    }

    pub fn format_section(s: &str) -> String {
        s.to_string()
    }

    pub fn format_duration(s: &str) -> String {
        s.to_string()
    }

    pub fn format_chord(s: &str) -> String {
        // Convert "maj" to "M" in chord symbols (e.g., "2maj" -> "2M", "Cmaj7" -> "CM7")
        s.replace("maj", "M")
    }

    pub fn format_key_sig(s: &str) -> String {
        s.to_string()
    }

    pub fn format_time_sig(s: &str) -> String {
        s.to_string()
    }

    pub fn format_tempo(s: &str) -> String {
        s.to_string()
    }

    pub fn format_border(s: &str) -> String {
        s.to_string()
    }

    pub fn format_dim(s: &str) -> String {
        s.to_string()
    }
}

/// Format a chord symbol for display, converting "maj" to "M"
///
/// Examples:
/// - "2maj" -> "2M"
/// - "Cmaj7" -> "CM7"
/// - "Dm" -> "Dm" (unchanged)
pub fn format_chord(s: &str) -> String {
    // Convert "maj" to "M" in chord symbols (e.g., "2maj" -> "2M", "Cmaj7" -> "CM7")
    s.replace("maj", "M")
}

/// Formatting options for chart display
struct Formatting {
    border: &'static str,
    bold: &'static str,
    reset: &'static str,
    chord_color: &'static str,
    dim: &'static str,
}

impl Formatting {
    fn default() -> Self {
        Self {
            border: colors::BORDER,
            bold: colors::BOLD,
            reset: colors::RESET,
            chord_color: colors::CHORD,
            dim: colors::DIM,
        }
    }
}

impl Chart {
    /// Display measures using a grid-based system
    /// - 4 measures per line
    /// - Each measure divided into 16th note slots (16 slots per measure in 4/4)
    /// - Chords placed at their exact positions
    /// - Visual markers for beat divisions
    fn display_measures_grid(
        f: &mut std::fmt::Formatter<'_>,
        measures: &[crate::chart::types::Measure],
        section_start_measures: u32,
    ) -> std::fmt::Result {
        Self::display_measures_grid_with_next_section(f, measures, section_start_measures, None)
    }

    fn display_measures_grid_with_next_section(
        f: &mut std::fmt::Formatter<'_>,
        measures: &[crate::chart::types::Measure],
        section_start_measures: u32,
        next_section_first_measure_chords: Option<&[&crate::chart::types::ChordInstance]>,
    ) -> std::fmt::Result {
        let fmt = Formatting::default();
        const MEASURES_PER_LINE: usize = 4;
        const SLOTS_PER_MEASURE: usize = 16; // 16th notes in 4/4 time
        const CHARS_PER_SLOT: usize = 4; // Fixed width per slot - accommodates chords like "2maj" (4 chars)

        let mut measure_idx = 0;
        let time_sig = if !measures.is_empty() {
            crate::time::TimeSignature::new(
                measures[0].time_signature.0 as u32,
                measures[0].time_signature.1 as u32,
            )
        } else {
            crate::time::TimeSignature::common_time()
        };
        let beats_per_measure = time_sig.numerator as usize;
        let slots_per_beat = SLOTS_PER_MEASURE / beats_per_measure; // Usually 4 for 4/4
        let _subdivisions_per_slot = (beats_per_measure * 1000) / SLOTS_PER_MEASURE; // Usually 250 for 4/4

        while measure_idx < measures.len() {
            // Start a new line
            write!(f, "{}{}║{} ", fmt.border, fmt.bold, fmt.reset)?;

            // Determine how many measures to show on this line
            let measures_on_line = MEASURES_PER_LINE.min(measures.len() - measure_idx);
            let line_end_idx = measure_idx + measures_on_line;

            // Collect all chords from measures on this line with their positions
            // Handle pushed chords that belong to previous measure
            // Also check next section's pushed chords if provided
            let mut chords_with_positions: Vec<(
                usize,
                usize,
                &crate::chart::types::ChordInstance,
            )> = Vec::new();

            // Check next section's pushed chords if provided and if last measure is on this line
            if let Some(next_chords) = next_section_first_measure_chords {
                let last_measure_abs = section_start_measures + measures.len() as u32 - 1;
                let last_measure_idx = measures.len() - 1;

                // Check if last measure is on this line
                if last_measure_idx >= measure_idx && last_measure_idx < line_end_idx {
                    for chord in next_chords.iter() {
                        let chord_abs_measures =
                            chord.position.total_duration.measure.max(0) as u32;
                        // If this chord belongs to the last measure of current section (pushed from next section)
                        if chord_abs_measures == last_measure_abs {
                            let chord_abs_beats = chord.position.total_duration.beat.max(0) as u32;
                            let chord_abs_subdivisions =
                                chord.position.total_duration.subdivision.clamp(0, 999) as u32;
                            let beats_in_slots = chord_abs_beats as usize * slots_per_beat;
                            let subdivisions_in_slots = (chord_abs_subdivisions as usize
                                * SLOTS_PER_MEASURE)
                                / (beats_per_measure * 1000);
                            let slot = beats_in_slots + subdivisions_in_slots;
                            let target_m_idx = last_measure_idx - measure_idx;
                            if target_m_idx < measures_on_line {
                                chords_with_positions.push((
                                    target_m_idx,
                                    slot.min(SLOTS_PER_MEASURE - 1),
                                    *chord,
                                ));
                            }
                        }
                    }
                }
            }

            // Collect all chords from all measures on this line, then place them by their actual position
            for (m_idx, measure) in measures[measure_idx..line_end_idx].iter().enumerate() {
                let _measure_abs_measures = section_start_measures + (measure_idx + m_idx) as u32;

                for chord in &measure.chords {
                    // Calculate position within the measure
                    // chord.position.total_duration is absolute from song start
                    let chord_abs_measures = chord.position.total_duration.measure.max(0) as u32;
                    let chord_abs_beats = chord.position.total_duration.beat.max(0) as u32;
                    let chord_abs_subdivisions =
                        chord.position.total_duration.subdivision.clamp(0, 999) as u32;

                    // Determine which measure on this line this chord belongs to based on its position
                    // Chords can be pushed into previous measures, so check all measures on this line
                    let target_measure_abs = chord_abs_measures;

                    // Check if this chord belongs to any measure on this line
                    let target_m_idx_opt = (0..measures_on_line).find(|&line_m_idx| {
                        let line_measure_abs =
                            section_start_measures + (measure_idx + line_m_idx) as u32;
                        target_measure_abs == line_measure_abs
                    });

                    if let Some(target_m_idx) = target_m_idx_opt {
                        // Calculate slot within the target measure
                        let beats_in_slots = chord_abs_beats as usize * slots_per_beat;
                        let subdivisions_in_slots = (chord_abs_subdivisions as usize
                            * SLOTS_PER_MEASURE)
                            / (beats_per_measure * 1000);
                        let slot = beats_in_slots + subdivisions_in_slots;
                        let slot = slot.min(SLOTS_PER_MEASURE - 1);
                        chords_with_positions.push((target_m_idx, slot, chord));
                    }
                }
            }

            // Create grid for this line: [measure][slot]
            let mut grid: Vec<Vec<Option<&crate::chart::types::ChordInstance>>> =
                vec![vec![None; SLOTS_PER_MEASURE]; measures_on_line];

            // Place chords in grid
            for (m_idx, slot, chord) in chords_with_positions {
                if m_idx < measures_on_line && slot < SLOTS_PER_MEASURE {
                    grid[m_idx][slot] = Some(chord);
                }
            }

            // Simple display: show chords with their positions and durations for debugging
            for measure_idx_in_line in 0..measures_on_line {
                let measure = &measures[measure_idx + measure_idx_in_line];

                // Show text cues if any
                if !measure.text_cues.is_empty() {
                    for cue in &measure.text_cues {
                        write!(f, "@{} \"{}\" ", cue.group, cue.text)?;
                    }
                }

                if let Some(measure_content) = Chart::display_measure_parallel_content(measure) {
                    write!(f, "{}{}{}", fmt.chord_color, fmt.bold, measure_content)?;
                    write!(f, "{}", fmt.reset)?;
                }

                // Measure separator (except for last measure on line)
                if measure_idx_in_line < measures_on_line - 1 {
                    write!(f, " {}|{} ", fmt.dim, fmt.reset)?;
                }
            }
            writeln!(f)?;
            measure_idx = line_end_idx;
        }

        Ok(())
    }

    /// Group measures for display, respecting explicit repeat annotations
    /// Returns groups where each group represents either:
    /// - A sequence of measures with a repeat annotation at the end
    /// - A single measure without repeat
    fn group_repeating_measures(measures: &[crate::chart::types::Measure]) -> Vec<MeasureGroup> {
        if measures.is_empty() {
            return vec![];
        }

        let mut groups = Vec::new();
        let mut i = 0;

        while i < measures.len() {
            // Look ahead to find if there's a repeat annotation in the upcoming measures
            let mut pattern_length = 1;
            let mut repeat_count = 1;

            // Scan forward to find a measure with repeat_count > 1
            for (j, measure) in measures.iter().enumerate().skip(i) {
                if measure.repeat_count > 1 {
                    // Found a repeat annotation - this marks the end of the pattern
                    pattern_length = j - i + 1;
                    repeat_count = measure.repeat_count;
                    break;
                } else if j > i && measure.repeat_count == 1 {
                    // This is a regular measure, include it
                    pattern_length = j - i + 1;
                }
            }

            groups.push(MeasureGroup {
                start_idx: i,
                count: repeat_count,
                pattern_length,
            });

            // Skip: pattern_length for display + (pattern_length * (repeat_count - 1)) for duplicates
            // This way we show the pattern once, but skip all the duplicated measures
            i += pattern_length * repeat_count;
        }

        groups
    }
}

impl Chart {
    /// Calculate the width needed for displaying 4 measures with the grid system
    fn calculate_content_width(&self) -> usize {
        const MEASURES_PER_LINE: usize = 4;
        const CHARS_PER_SLOT: usize = 2;

        // Get time signature (default to 4/4 if not set)
        let time_sig = self
            .time_signature
            .unwrap_or_else(crate::time::TimeSignature::common_time);
        let beats_per_measure = time_sig.numerator as usize;
        let slots_per_measure = 16; // 16th notes (works for 4/4, adjust for other time sigs if needed)

        // Calculate width for one measure:
        // - Slots: slots_per_measure * CHARS_PER_SLOT
        // - Beat markers: (beats_per_measure - 1) * 1 (the · character)
        let measure_width = (slots_per_measure * CHARS_PER_SLOT) + (beats_per_measure - 1);

        // Calculate width for 4 measures:
        // - 4 measures
        // - 3 measure separators (|) with spacing: 3 chars each
        let content_width = (measure_width * MEASURES_PER_LINE) + (3 * (MEASURES_PER_LINE - 1));

        // Add border padding: "║ " at start (2 chars) + " " at end (1 char) = 3
        content_width + 3
    }

    fn display_measure_parallel_content(measure: &crate::chart::types::Measure) -> Option<String> {
        let chord_parts: Vec<String> = measure
            .chords
            .iter()
            .map(|chord| {
                let mut chord_text = String::new();

                if let Some((is_push, _amount)) = chord.push_pull
                    && is_push
                {
                    chord_text.push('\'');
                }

                chord_text.push_str(&chord.full_symbol);

                if let Some((is_push, _amount)) = chord.push_pull
                    && !is_push
                {
                    chord_text.push('\'');
                }

                chord_text
            })
            .collect();

        let melody_parts: Vec<String> = measure
            .melodies
            .iter()
            .map(std::string::ToString::to_string)
            .collect();

        match (chord_parts.is_empty(), melody_parts.is_empty()) {
            (false, false) => Some(format!(
                "<< {} ; {} >>",
                chord_parts.join(" "),
                melody_parts.join(" ; ")
            )),
            (false, true) => Some(chord_parts.join(" ")),
            (true, false) => Some(melody_parts.join(" ; ")),
            (true, true) => None,
        }
    }
}

impl std::fmt::Display for Chart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::chord::PushPullBase;

        // Simple title
        if let Some(ref title) = self.metadata.title {
            writeln!(f, "{}", title)?;
        }

        // Default push command if set
        if let Some(ref default_push) = self.default_push_amount {
            write!(f, "/push = ")?;
            match &default_push.base {
                PushPullBase::Standard => {
                    // Standard push (8th note)
                    writeln!(f, "8")?;
                }
                PushPullBase::Triplet => {
                    writeln!(f, "triplet")?;
                }
                PushPullBase::Tuplet(n) => {
                    writeln!(f, "tuplet:{}", n)?;
                }
                PushPullBase::Duration {
                    duration,
                    dotted,
                    triplet,
                } => {
                    write!(f, "{}", duration.value())?;
                    if *dotted {
                        write!(f, ".")?;
                    }
                    if *triplet {
                        write!(f, "t")?;
                    }
                    writeln!(f)?;
                }
            }
        }

        // Sections - one line per section
        for section in &self.sections {
            // Section header
            write!(f, "{}: ", section.section.display_name())?;

            // Collect all rhythm elements (chords and rests) from all measures
            let mut all_elements = Vec::new();
            for measure in section.measures() {
                if measure.rhythm_elements.is_empty() {
                    continue;
                }

                // Collect rhythm element strings for this measure
                let mut element_strings = Vec::new();
                for elem in &measure.rhythm_elements {
                    match elem {
                        crate::chart::types::RhythmElement::Chord(chord) => {
                            let mut chord_text = String::new();

                            // Accent command
                            if chord.commands.iter().any(|c| c.is_accent()) {
                                chord_text.push('>');
                            }

                            // Push/pull notation
                            if let Some((is_push, amount)) = &chord.push_pull
                                && *is_push
                            {
                                // Check if this push matches the default
                                let matches_default = self
                                    .default_push_amount
                                    .as_ref()
                                    .map(|default| {
                                        // Compare the push amounts
                                        format!("{:?}", amount) == format!("{:?}", default)
                                    })
                                    .unwrap_or(false);

                                if matches_default {
                                    // Just use apostrophe
                                    chord_text.push('\'');
                                } else {
                                    // Use full notation with amount
                                    chord_text.push_str(&Chart::format_push_pull_amount(amount));
                                }
                            }

                            chord_text.push_str(&chord.full_symbol);

                            // Note: Bass note is already included in full_symbol (from Chord::to_string())
                            // so we don't need to add it again here

                            // Pull notation (always show amount for pulls)
                            if let Some((is_push, _amount)) = &chord.push_pull
                                && !*is_push
                            {
                                chord_text.push('\'');
                            }

                            // Add rhythm notation
                            // - Explicit durations attach directly: Ab9_8t
                            // - Slashes are space-separated: 'Eb ///
                            // - Default (1 beat) = single slash: /
                            // - Whole measure (4 slashes in 4/4) = omitted
                            let ts = measure.time_signature;
                            match &chord.rhythm {
                                ChordRhythm::Slashes { count, .. } if *count == ts.0 => {
                                    // Whole measure — omit slashes entirely,
                                    // chord name alone implies full measure
                                }
                                ChordRhythm::Default => {
                                    // Single beat — show as /
                                    chord_text.push_str(" /");
                                }
                                ChordRhythm::Slashes { .. } => {
                                    // Partial slashes — space then slashes
                                    chord_text.push(' ');
                                    chord_text.push_str(&format_rhythm(&chord.rhythm));
                                }
                                _ => {
                                    // Explicit duration — attach directly
                                    chord_text.push_str(&format_rhythm(&chord.rhythm));
                                }
                            }

                            element_strings.push(chord_text);
                        }
                        crate::chart::types::RhythmElement::Rest(rest) => {
                            // Show rest with its original token (e.g., "r8t", "r2")
                            element_strings.push(rest.original_token.clone());
                        }
                        crate::chart::types::RhythmElement::Space(space) => {
                            // Show space with its original token (e.g., "s1", "s4")
                            element_strings.push(space.original_token.clone());
                        }
                    }
                }

                if !element_strings.is_empty() {
                    all_elements.push(element_strings.join(" "));
                }
            }

            // Print all elements separated by |
            if !all_elements.is_empty() {
                writeln!(f, "{}", all_elements.join(" | "))?;
            } else {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}
