//! One chord line into measures: the token loop.

mod marks;
mod rhythm;

use crate::text::chart::cues::TextCue;
use crate::text::chart::dynamics::DynamicMarking;
use crate::text::chart::melody::Melody;
use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::chords::groups::{beats_to_lily_suffix, expand_chord_groups};
use crate::text::chart::parser::chords::tokens::tokenize_with_spans;
use crate::text::chart::types::{ChordInstance, Measure, RhythmElement};
use crate::text::chord::{ChordRhythm, NotationSystem};
use crate::text::key::Key;
use crate::text::parsing::TextSpan;
use crate::text::sections::SectionType;
use crate::text::time::{
    AbsolutePosition, MusicalDuration, MusicalPosition, MusicalPositionExt, TimeSignature,
    TimeSignatureExt,
};
use keyflow_proto::chart::{Placement, RepeatMark, StaffText};

/// The token loop's working state: the bars finished so far, the one being
/// filled, and the flags that decide where the next token lands. Every
/// kind of token is handled against it — here, in `marks` or in `rhythm`.
struct LineState {
    /// The meter in force; a one-shot `!T` changes it for a single bar.
    time_sig: TimeSignature,
    beats_per_measure: f64,
    measures: Vec<Measure>,
    current_measure: Measure,
    current_measure_beats: f64,
    /// Track if we just processed a | separator
    just_processed_separator: bool,
    /// Track if current measure was created by |
    measure_was_created_by_separator: bool,
    /// Track if current measure has chords with slash rhythm
    measure_has_slash_rhythm: bool,
    /// Track for slash accumulation (consecutive slashes accumulate)
    last_token_was_slash: bool,
    pending_start_repeat: bool,
}

impl<'a> ChartParser<'a> {
    pub(in crate::text::chart::parser) fn parse_chord_line_inner(
        &mut self,
        line: &str,
        section_type: &SectionType,
        section_measure_count: Option<usize>,
        line_byte_offset: usize,
        default_chord_length: Option<(ChordRhythm, MusicalDuration)>,
        mut default_melody_duration: Option<String>,
        default_melody_octave: Option<u8>,
    ) -> Result<Vec<Measure>, String> {
        use crate::text::chart::types::TempoChange;
        use keyflow_proto::chart::commands::Command;

        let time_sig = self.time_signature.unwrap_or(TimeSignature::common_time());
        let beats_per_measure = time_sig.numerator as f64;

        // Check for repeat syntax at the end of the line (e.g., "6 5 4 4 x4")
        let (line_to_parse, repeat_count) = Self::extract_repeat_syntax(line);

        // Byte offset of `line_to_parse` within the original `line` (always 0
        // unless `extract_repeat_syntax` trimmed a prefix). Combined with the
        // outer offset, this lets every emitted token span be expressed in the
        // original document's byte coordinates.
        let trim_offset = (line_to_parse.as_ptr() as usize).saturating_sub(line.as_ptr() as usize);
        let outer_offset = line_byte_offset + trim_offset;

        let line_to_parse = self.expand_aliases_in_line(line_to_parse);

        let trimmed_parallel = line_to_parse.trim();
        if (trimmed_parallel.starts_with("<<") && trimmed_parallel.ends_with(">>"))
            || (line_to_parse.contains("<<")
                && line_to_parse.contains(">>")
                && !line_to_parse.contains("|:")
                && !line_to_parse.contains(":|"))
        {
            return self.parse_parallel_chord_line(
                &line_to_parse,
                repeat_count,
                section_type,
                section_measure_count,
                outer_offset,
                default_chord_length,
                default_melody_duration,
                default_melody_octave,
            );
        }

        if default_chord_length.is_some() && line_to_parse.contains('|') {
            return self.parse_barred_line_with_default_chord_length(
                &line_to_parse,
                section_type,
                section_measure_count,
                outer_offset,
                default_chord_length,
                default_melody_duration,
                default_melody_octave,
            );
        }

        let line_to_parse = Self::normalize_parallel_container_syntax(&line_to_parse);
        let line_to_parse = Self::normalize_repeat_barlines(&line_to_parse);
        let line_to_parse = line_to_parse.replace("m {", "m{");

        // Preprocess: Expand `()` rhythm groups into explicit per-chord lily
        // durations (e.g. `(C G)` → `C_2 G_2`, `(D Em G)` → `D_2t Em_2t G_2t`).
        // Identity for lines with no group, so spans/tokens are unchanged there.
        let line_to_parse = expand_chord_groups(&line_to_parse, time_sig)?;

        // Preprocess: Calculate automatic durations for chords between measure separators
        // If chords are between | separators, split the measure evenly
        // e.g., "| G C |" → "G_2 C_2", "G C | D" → "G_2 C_2 D_1"
        let line_with_auto_durations =
            Self::apply_auto_durations_between_separators(&line_to_parse, beats_per_measure);

        let line_with_auto_token_spans = tokenize_with_spans(&line_with_auto_durations);
        let tokens_str: Vec<&str> = line_with_auto_token_spans
            .iter()
            .map(|span| &line_with_auto_durations[span.as_range()])
            .collect();

        // Byte spans of each whitespace-separated token in the *pre-auto-duration* line.
        // `apply_auto_durations_between_separators` only appends `_N` suffixes to existing
        // tokens (it does not add, remove, or reorder them), so token-index 1:1 mapping
        // back to `line_to_parse` is safe for the non-parallel path. Spans are
        // line-relative; line/column resolution to the original document is the caller's
        // responsibility.
        let original_token_spans: Vec<TextSpan> = tokenize_with_spans(&line_to_parse)
            .into_iter()
            .map(|s| TextSpan::new(s.start + outer_offset, s.len))
            .collect();
        let mut pending_cue: Option<TextCue> = None;
        // Text written after a full bar and before the next chord opens the
        // next bar. It waits here, with the bar count it was written at,
        // until that bar has been started.
        let mut pending_staff_text: Vec<(StaffText, usize)> = Vec::new();
        let mut pending_dynamic: Option<DynamicMarking> = None;
        let mut pending_stop: Option<Command> = None;
        let mut in_melody_block = false; // Track if we're inside a m{...} block
        let mut melody_search_offset = 0usize; // Cursor for finding successive m{ blocks in line_to_parse
        let mut line_melody_octave = default_melody_octave;
        let mut last_chord: Option<ChordInstance> = None; // Track last chord for dot repeat
        let mut chord_length_override: Option<(ChordRhythm, MusicalDuration)> =
            default_chord_length.clone();
        // The declared default (`\duration`), which a written note value
        // only overrides until the barline.
        let mut declared_chord_length: Option<(ChordRhythm, MusicalDuration)> =
            default_chord_length;
        let mut skip_next_token = false;
        // Number of upcoming tokens to skip — used by multi-token annotations
        // like `dyn <level> [above]` and `hairpin <dir> <span> [above]` that
        // are consumed in one place but span several tokens.
        let mut skip_tokens: usize = 0;
        // One-shot meter change (`!T2/4`): apply the new meter to exactly the
        // next measure, then revert to the prevailing meter. `oneshot_revert`
        // holds the (time_sig, beats_per_measure) to restore; the revert fires
        // once `measures.len()` passes `oneshot_measure_idx` — i.e. the moment
        // the one-shot measure has been pushed.
        let mut oneshot_revert: Option<(TimeSignature, f64)> = None;
        let mut oneshot_measure_idx: usize = 0;

        // Notation system for this line, used to disambiguate `b<digit>` roots
        // (note B vs flat degree). Line wins, then the chart parsed so far.
        let chord_system = self.resolve_notation_system(&tokens_str);
        // Contribute this line's decisive tokens to the chart-wide tally so
        // later ambiguous lines can fall back to it.
        self.record_line_system_votes(&tokens_str);

        let mut st = LineState {
            time_sig,
            beats_per_measure,
            measures: Vec::new(),
            current_measure: Self::fresh_measure(time_sig),
            current_measure_beats: 0.0,
            just_processed_separator: false,
            measure_was_created_by_separator: false,
            measure_has_slash_rhythm: false,
            last_token_was_slash: false,
            pending_start_repeat: false,
        };

        for (token_idx, token_str) in tokens_str.iter().enumerate() {
            // Text waiting for the bar after a full one: once that bar has
            // begun, the text opens it.
            if pending_staff_text
                .first()
                .is_some_and(|(_, written_at)| st.measures.len() > *written_at)
            {
                for (text, _) in pending_staff_text.drain(..) {
                    st.current_measure.staff_text.push(text);
                }
            }
            if skip_tokens > 0 {
                skip_tokens -= 1;
                continue;
            }
            if skip_next_token {
                skip_next_token = false;
                continue;
            }

            // Once a one-shot meter measure has been pushed, restore the
            // prevailing meter. The freshly started measure was stamped with the
            // one-shot meter when it was created; correct it (it has no content
            // yet) so following chords land in the reverted time signature.
            if let Some((revert_ts, revert_bpm)) = oneshot_revert {
                if st.measures.len() > oneshot_measure_idx {
                    st.time_sig = revert_ts;
                    st.beats_per_measure = revert_bpm;
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    oneshot_revert = None;
                }
            }

            // Floating suspension figure (`4-3`, `2-3`, `3`, `2`): the prior
            // chord keeps sounding; record the figure at the current beat and
            // let following slash groups extend the held chord. Consumes no
            // beats of its own and never counts as rhythm.
            if let Some(figure) = Self::parse_bare_suspension_figure(token_str) {
                Self::place_floating_suspension(
                    &mut st,
                    &figure,
                    &tokens_str,
                    token_idx,
                    &mut skip_next_token,
                );
                continue;
            }

            // Floating slash-bass (`/D`): inherit the previous chord's root as
            // `<root>/D` for harmony, but display the token verbatim. Rewrite
            // to the synthetic chord token here and route it through normal
            // chord parsing below; the original `/D` becomes the display text.
            let synth_slash_bass: Option<String> = Self::parse_floating_slash_bass(token_str)
                .zip(Self::previous_chord_root(&st.current_measure, &st.measures))
                .map(|(bass, root)| format!("{root}/{bass}"));
            let (effective_token, display_override): (&str, Option<String>) =
                match &synth_slash_bass {
                    Some(synth) => (synth.as_str(), Some((*token_str).to_string())),
                    None => (token_str, None),
                };

            // Inline classical dynamic / hairpin: zero-duration annotations that
            // must NOT consume measure beats. Without this they fall through to
            // chord parsing and inflate the measure count (e.g. `dyn mp C#m //.`
            // would parse as two measures). Mirrors the standalone-line forms
            // `parse_classical_dynamic_line` / `parse_hairpin_line`.
            if *token_str == "dyn" || *token_str == "dynamic" {
                skip_tokens = Self::inline_dynamic(&mut st, &tokens_str, token_idx);
                continue;
            }

            if *token_str == "hairpin" {
                skip_tokens = Self::inline_hairpin(&mut st, &tokens_str, token_idx);
                continue;
            }

            if *token_str == "\\octave" {
                if let Some(next) = tokens_str.get(token_idx + 1) {
                    line_melody_octave = next.parse::<u8>().ok().or(line_melody_octave);
                    skip_next_token = true;
                }
                continue;
            }

            if *token_str == "\\ChordLength"
                || *token_str == "\\duration"
                || *token_str == "\\Duration"
            {
                if let Some(next) = tokens_str.get(token_idx + 1) {
                    chord_length_override = Self::parse_chord_length_value(next, st.time_sig);
                    declared_chord_length = chord_length_override.clone();
                    if *token_str == "\\duration" || *token_str == "\\Duration" {
                        default_melody_duration = Some(next.to_string());
                    }
                    skip_next_token = true;
                }
                continue;
            }

            // `%` is the simile mark: one more measure of what the last one
            // was. `%3` is three of them — `%` on its own is `%1`.
            if let Some(repeats) = Self::parse_measure_repeat(token_str) {
                self.push_simile(&mut st, repeats);
                continue;
            }

            if let Some((text, placement)) = Self::parse_quoted_text_token(token_str) {
                Self::place_staff_text(
                    &mut st,
                    text,
                    placement,
                    &tokens_str,
                    token_idx,
                    &mut pending_staff_text,
                );
                continue;
            }

            let token_for_barline = token_str
                .split_once('_')
                .map_or(*token_str, |(base, _)| base);
            if token_for_barline.starts_with("|{") && token_for_barline.ends_with('}') {
                Self::finalize_measure_for_separator(
                    &mut st.measures,
                    &st.current_measure,
                    st.measure_was_created_by_separator,
                );
                st.current_measure = Self::fresh_measure(st.time_sig);
                st.current_measure_beats = 0.0;
                st.just_processed_separator = true;
                st.measure_was_created_by_separator = true;
                st.measure_has_slash_rhythm = false;
                continue;
            }

            // Check for a command keyword (e.g. `\\fermata`, `\\accent`).
            // Commands are applied to the PREVIOUS chord.
            if token_str.starts_with('\\') && display_override.is_none() {
                if *token_str == "\\octave" {
                    skip_next_token = true;
                    continue;
                }

                if let Some(cmd) = Command::parse_slash(token_str) {
                    Self::apply_command(&mut st, cmd);
                    continue;
                }
            }

            // Slash runs are rhythm, not keywords: `/`, `//`, `//.`. They used
            // to share the branch above, back when a command was also written
            // with a leading slash — splitting the two is the whole reason
            // commands moved to a backslash.
            if token_str.starts_with('/') && display_override.is_none() {
                if self.apply_slash_run(&mut st, token_str) {
                    continue;
                }
            }

            // Reset slash tracking for non-slash tokens
            st.last_token_was_slash = false;

            // Check for tie symbol (~)
            // The tie connects the previous chord's duration to the following duration.
            // Example: "C // ~ /" = C with 2 beats + 1 beat = 3 beats total
            if *token_str == "~" {
                Self::mark_tie(&mut st);
                continue;
            }

            // Check for slashes following a tie: these should ADD to the previous chord's duration
            // Check if the previous token was a tie (we handle this by checking rhythm flags)
            // Since we can't easily look back at previous tokens, we check the chord's tied flag
            if token_str.chars().all(|c| c == '/') && !token_str.is_empty() {
                if Self::extend_tied_chord(&mut st, token_str) {
                    continue;
                }
                // If not a tied chord, fall through to normal slash handling below
            }

            // Check for melody variable reference (e.g., "$mainRiff").
            //
            // If the previous chord auto-completed the measure (whole-note,
            // half-note that filled remaining beats, etc.), `current_measure`
            // is empty here; attach the recalled melody to the most recently
            // pushed measure instead. Mirrors the inline `m{...}` branch.
            if let Some(var_name) = token_str.strip_prefix('$') {
                if let Some(melody) = self.melody_variables.get(var_name).cloned() {
                    if st.current_measure.chords.is_empty() && !st.measures.is_empty() {
                        st.measures.last_mut().unwrap().melodies.push(melody);
                    } else {
                        st.current_measure.melodies.push(melody);
                    }
                } else {
                    tracing::warn!("Unknown melody variable '{}'", var_name);
                }
                continue;
            }

            // Check for inline melody block (e.g., "m{ C_8 D_8 E_4 }")
            if token_str.starts_with("m{") {
                in_melody_block = true;
                // Find the full melody block in the original line (advancing cursor for successive blocks)
                if let Some(m_pos) = line_to_parse[melody_search_offset..].find("m{") {
                    let abs_pos = melody_search_offset + m_pos;
                    let melody_start = &line_to_parse[abs_pos..];
                    // Find the closing brace
                    if let Some(close_pos) = melody_start.find('}') {
                        let melody_str = &melody_start[..close_pos + 1];
                        // Advance cursor past this melody block for next search
                        melody_search_offset = abs_pos + close_pos + 1;
                        match Melody::parse_block_with_defaults(
                            melody_str,
                            default_melody_duration.as_deref(),
                            line_melody_octave,
                        ) {
                            Ok((name, melody)) => {
                                if let Some(var_name) = name {
                                    // Store as a variable
                                    self.melody_variables.set(var_name, melody.clone());
                                }
                                // Attach melody to current measure.
                                // If the current measure is empty (chord filled the previous
                                // measure exactly and it was auto-pushed), attach to the
                                // last pushed measure instead.
                                if st.current_measure.chords.is_empty() && !st.measures.is_empty() {
                                    st.measures.last_mut().unwrap().melodies.push(melody);
                                } else {
                                    st.current_measure.melodies.push(melody);
                                }
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

            if *token_str == "@repeat-start" {
                Self::finalize_measure_for_separator(&mut st.measures, &st.current_measure, false);
                st.current_measure = Self::fresh_measure(st.time_sig);
                st.current_measure.start_repeat = RepeatMark::Forward;
                st.pending_start_repeat = true;
                st.current_measure_beats = 0.0;
                st.just_processed_separator = true;
                st.measure_was_created_by_separator = true;
                st.measure_has_slash_rhythm = false;
                continue;
            }

            if let Some(passes) = token_str.strip_prefix("@repeat-end") {
                Self::close_repeat(&mut st, passes);
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

            // Check for an inline cue (e.g., "@Build", "@Hit:3")
            if token_str.len() > 1 && token_str.starts_with('@') {
                if let Ok(dynamic) = DynamicMarking::parse(token_str) {
                    pending_dynamic = Some(dynamic);
                }
                continue;
            }

            // Check for inline dynamic marking (e.g., "<Build>", "<Down>", "<Hit>:3")
            if token_str.starts_with('<') && token_str.contains('>') {
                if let Ok(dynamic) = DynamicMarking::parse(token_str) {
                    pending_dynamic = Some(dynamic);
                }
                continue;
            }

            // Check for stop sign tokens (e.g., "!STOP", "!STOPGROOVE")
            // Position depends on context:
            //   !STOP C  → Stop (before) on C — "stop, then hit C"
            //   C !STOP  → StopAfter on C — "hit C, then stop"
            if let Some(stop_cmd) = Command::parse_stop_token(token_str) {
                Self::place_stop(&mut st, stop_cmd, &mut pending_stop);
                continue;
            }

            // Check for tempo change (e.g., "->140bpm", "->120")
            if token_str.starts_with("->") {
                if let Some(new_tempo) = TempoChange::parse_syntax(token_str) {
                    // Record the tempo change
                    let position = AbsolutePosition::new(
                        MusicalPosition::try_new(
                            st.measures.len() as i32,
                            st.current_measure_beats as i32,
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

            if let Some(volta) = Self::parse_volta_token(token_str) {
                st.current_measure.volta_start = Some(volta);
                continue;
            }

            // Check for measure separator (|)
            // This forces a measure boundary regardless of beat count
            if *token_str == "|" {
                // Finalize current measure if it has chords, or if it was created by a previous separator
                // (This allows multiple | in a row to create empty measures)
                // But don't push auto-created empty measures (created when a measure fills up)
                Self::finalize_measure_for_separator(
                    &mut st.measures,
                    &st.current_measure,
                    st.measure_was_created_by_separator,
                );
                // Always start a new measure after |
                st.current_measure = Self::fresh_measure(st.time_sig);
                st.current_measure_beats = 0.0;
                st.just_processed_separator = true; // Mark that we just processed a separator
                st.measure_was_created_by_separator = true; // Mark that this measure was created by |
                st.measure_has_slash_rhythm = false; // Reset for new measure
                continue;
            }

            // Check for inline time signature change (e.g., "T6/8", "T3/4").
            // The leading `T` (Time) is required so meter changes never collide
            // with number/Nashville slash chords like `4/6`. Bare fractions are
            // only honored on the header line (see parse_metadata_line).
            //
            // A `!` prefix (`!T2/4`) makes it one-shot: the meter applies to the
            // single next measure, then reverts to the prevailing meter — the
            // same "one-time" sigil used for durations.
            let oneshot = token_str.starts_with("!T");
            let ts_token: &str = token_str.strip_prefix('!').unwrap_or(token_str);
            if let Some(ts_body) = ts_token.strip_prefix('T') {
                if ts_body.contains('/') {
                    if let Some((num, den)) = Self::parse_time_signature(ts_body) {
                        // Finalize the current measure before the time sig change
                        if !st.current_measure.chords.is_empty() {
                            st.measures.push(st.current_measure.clone());
                            st.current_measure = Self::fresh_measure(st.time_sig);
                            st.current_measure_beats = 0.0;
                        }

                        if oneshot {
                            // Remember what to restore once this single measure
                            // is pushed. Leave self.time_signature untouched so
                            // the prevailing meter still governs later lines.
                            oneshot_revert = Some((st.time_sig, st.beats_per_measure));
                            oneshot_measure_idx = st.measures.len();
                        } else {
                            // Persistent change: subsequent measures keep this.
                            self.time_signature = Some(TimeSignature::new(num as u32, den as u32));
                        }

                        st.time_sig = TimeSignature::new(num as u32, den as u32);
                        st.current_measure.time_signature = (num, den);
                        st.beats_per_measure = num as f64;
                        continue;
                    }
                }
            }

            // Check for key change - should be ONLY a key signature (short token like "#G", "bBb")
            // Not a chord like "bbmaj7" or "g7"
            let looks_like_key_sig =
                token_str.len() <= 3 && (token_str.starts_with('#') || token_str.starts_with('b'));

            if looks_like_key_sig {
                if let Ok(new_key) = Key::parse(token_str) {
                    self.record_key_change(&st, new_key);
                    continue;
                }
            }

            // A bare `r` or `s` is a bar of rest or space, the way a bare
            // chord is a bar of chord (or the carried chord length, when one
            // is in force). Slashes after it give it its length instead —
            // `r //` rests two beats. It is rewritten to the lily form the
            // rest and space parsing below reads; the written text stays the
            // element's original token.
            let mut written_token = token_str.to_string();
            let bare_rest_token = if matches!(*token_str, "r" | "s") {
                let slashes = tokens_str.get(token_idx + 1).copied().filter(|t| {
                    let run = t.trim_end_matches('.');
                    !run.is_empty() && run.chars().all(|c| c == '/') && t.len() - run.len() <= 1
                });
                let beats = if let Some(slashes) = slashes {
                    skip_next_token = true;
                    written_token = format!("{token_str} {slashes}");
                    let run = slashes.trim_end_matches('.').len() as f64;
                    if slashes.ends_with('.') {
                        run * 1.5
                    } else {
                        run
                    }
                } else {
                    chord_length_override
                        .as_ref()
                        .map_or(st.beats_per_measure, |(_, duration)| {
                            duration.to_beats(st.time_sig)
                        })
                };
                let length = beats_to_lily_suffix(beats, st.time_sig).unwrap_or_else(|| {
                    format!("_{}*{}", st.time_sig.denominator, st.time_sig.numerator)
                });
                Some(format!("{token_str}{length}"))
            } else {
                None
            };
            let rest_token: &str = bare_rest_token.as_deref().unwrap_or(token_str);

            // Check for standalone rest token (r4, r8, r8t, r4t, r2, etc.)
            // These don't have a chord symbol but should be stored as rhythm elements
            if self.place_rest(&mut st, rest_token, &written_token) {
                continue;
            }

            // Check for standalone space token (s1, s2, s4, s8, etc.)
            // These represent "invisible" duration - the measure will be filled with automatic slashes
            if self.place_space(&mut st, rest_token, &written_token) {
                continue;
            }

            // `.` repeats the last chord for as long as that chord lasted —
            // a direct repeat, not a measure one. `.3` is three of them.
            // (`apply_auto_durations` may have appended `_N`, hence `._4`.)
            if let Some(dot_repeats) = Self::parse_dot_repeat(token_str, chord_system) {
                self.place_dot_repeats(&mut st, dot_repeats, &last_chord);
                continue;
            }

            // Parse chord. Pass through the token's byte span in the original line
            // (line-relative) so diagnostics can point at real source positions.
            let token_span = original_token_spans.get(token_idx).copied();
            // In a degree-based context a bare `b<digit>` is a flat scale degree
            // (`b3` = ♭3) and a merged `17` is degree-1 + a 7th — not the note B
            // with a figured-bass/suspension figure. Skip the suffix extractors
            // so they don't strip the digit, and split `17` into `1:7`.
            // `^`-marked figured bass (`V^65`, `V^4-3`) is unambiguous in any
            // notation system, so strip it before the degree-specific handling.
            // An *inversion* figure (`V^65`) rewrites to a real inverted chord
            // (parse `V7`, then set the bass below); the original `V^65` is kept
            // for display. Other figures (`^4-3`) fall through to figured bass.
            let inversion_token;
            let (effective_token, inversion_apply) =
                match Self::extract_caret_inversion(effective_token) {
                    Some((ct, display, _append_seventh, bass_thirds)) => {
                        inversion_token = ct;
                        (inversion_token.as_str(), Some((display, bass_thirds)))
                    }
                    None => (effective_token, None),
                };
            let (caret_token, caret_rows) = Self::extract_caret_figure(effective_token);
            let effective_token = caret_token.as_str();
            // A leading flat degree is a flat degree whatever the rest of
            // the line is written in. This used to be gated on the scope
            // already being `Degree`, so in a letters-majority chart `b3`
            // fell through to the suffix extractors below, which read the
            // `3` as a suspension figure and left `b` — engraving `C b3 F G`
            // as `C B F G`. Uppercase `B` is the note; lowercase `b` before
            // a digit is a flat.
            let (chord_token, figured_bass_rows, suspension_figure) =
                if Self::is_leading_flat_degree(effective_token)
                    || (chord_system == NotationSystem::Degree
                        && Self::is_merged_degree(effective_token))
                {
                    (Self::split_merged_degree(effective_token), caret_rows, None)
                } else {
                    let (fb_token, fb_rows) = Self::extract_figured_bass_suffix(effective_token);
                    let (ct, susp) = Self::extract_suspension_suffix(&fb_token);
                    (ct, caret_rows.or(fb_rows), susp)
                };
            match self.parse_chord_token(
                &chord_token,
                section_type,
                st.time_sig,
                token_span,
                chord_system,
            ) {
                Ok(mut chord) => {
                    if let Some(text) = &display_override {
                        chord.display_override = Some(text.clone());
                    }
                    // Realise a `^` inversion: set the slash bass to the chord
                    // tone (3rd/5th/7th above the root) so the chord resolves as
                    // a real inversion, while the chart keeps showing `V^65`.
                    if let Some(inversion) = &inversion_apply {
                        self.realise_caret_inversion(&mut chord, inversion);
                    }
                    let token_has_explicit_length =
                        Self::token_has_explicit_chord_length(&chord_token);
                    // A rhythm-slash token immediately after this chord (`E/B /`)
                    // sets its duration explicitly, so the `\Duration` default
                    // must NOT pre-fill it — otherwise the slash would only add a
                    // continuation on top of the default instead of overriding it.
                    let slash_token_follows = tokens_str.get(token_idx + 1).is_some_and(|t| {
                        !t.is_empty() && t.trim_end_matches('.').chars().all(|c| c == '/')
                    });
                    // A written note value carries to the bare chords after it
                    // in the same bar (`C_8 D E F`), never across the barline:
                    // a bare chord that starts a bar means the whole bar (or
                    // the declared `\duration`), whatever the bar before it
                    // was written in. `D_2. G_4 G` is three bars' worth of
                    // music only if the last G is a whole bar.
                    let starts_a_bar = st.current_measure_beats < 0.001
                        || st.current_measure_beats > st.beats_per_measure - 0.001;
                    if starts_a_bar && !token_has_explicit_length {
                        chord_length_override = declared_chord_length.clone();
                    }
                    if let Some((rhythm, duration)) = &chord_length_override {
                        if chord.rhythm == ChordRhythm::Default
                            && !token_has_explicit_length
                            && !slash_token_follows
                        {
                            chord.rhythm = rhythm.clone();
                            chord.duration = *duration;
                        }
                    }
                    if token_has_explicit_length
                        && !chord_token.starts_with('!')
                        && chord.rhythm != ChordRhythm::Default
                    {
                        chord_length_override = Some((chord.rhythm.clone(), chord.duration));
                    }
                    let chord_beats =
                        Self::make_room_for_chord(&mut st, &mut chord, &tokens_str, token_idx);

                    // Attach pending stop command to this chord
                    if let Some(stop_cmd) = pending_stop.take() {
                        chord.commands.push(stop_cmd);
                    }

                    if st.pending_start_repeat && st.current_measure.chords.is_empty() {
                        st.current_measure.start_repeat = RepeatMark::Forward;
                        st.pending_start_repeat = false;
                    }

                    // Store as last chord for dot repeat
                    last_chord = Some(chord.clone());

                    // Add chord to both chords (for backward compat) and rhythm_elements
                    st.current_measure
                        .rhythm_elements
                        .push(RhythmElement::Chord(chord.clone()));
                    st.current_measure.chords.push(chord);

                    if let Some(rows) = figured_bass_rows {
                        Self::push_figured_bass_at_current_beat(
                            &mut st.current_measure,
                            &rows,
                            Placement::Above,
                            st.current_measure_beats,
                        );
                    }

                    // Attached suspension figure (`Eb2`, `F4-3`) anchors at the
                    // chord's own beat and hugs it as a superscript.
                    if let Some(figure) = &suspension_figure {
                        Self::push_suspension_at_current_beat(
                            &mut st.current_measure,
                            figure,
                            Placement::Above,
                            st.current_measure_beats,
                            false,
                        );
                    }

                    st.current_measure_beats += chord_beats;

                    // Attach pending cue to this measure if present
                    if let Some(cue) = pending_cue.take() {
                        st.current_measure.text_cues.push(cue);
                    }

                    // Attach pending dynamic to this measure if present
                    if let Some(dynamic) = pending_dynamic.take() {
                        st.current_measure.dynamics.push(dynamic);
                    }

                    // If we've completed exactly one measure, start a new one
                    // (but only if we didn't just process a separator)
                    // When we just processed a separator, we're already in a fresh measure,
                    // so we don't want to auto-create another one
                    if !st.just_processed_separator
                        && (st.current_measure_beats - st.beats_per_measure).abs() < 0.001
                    {
                        // small epsilon for float comparison
                        st.measures.push(st.current_measure.clone());
                        st.current_measure = Measure::new();
                        st.current_measure.time_signature =
                            (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                        st.current_measure_beats = 0.0;
                        st.measure_has_slash_rhythm = false; // Reset for new measure
                        // Auto-created measure, not by separator
                    }
                }
                Err(_e) => {
                    // Skip unparseable tokens silently
                    // (might be a formatting token or invalid chord)
                }
            }
        }

        // A one-shot meter change still waiting for its measure when the line
        // ends is a bar of that meter with no new chord in it — `!T2/4` on a
        // line of its own is "one bar of 2/4", the breakdown bar of a
        // structure-only chart. Without this the bar was dropped and the
        // section fell back to the prevailing meter.
        let oneshot_bar_pending =
            oneshot_revert.is_some() && st.measures.len() == oneshot_measure_idx;

        // Text still waiting for its bar when the line ends: the music after
        // it never opened one (it was a mark, not a chord), so it stays on
        // the bar it was written after, at that bar's last beat.
        for (mut text, _) in pending_staff_text.drain(..) {
            text.beat = text
                .beat
                .max(st.current_measure_beats.ceil().max(1.0) as u8);
            st.current_measure.staff_text.push(text);
        }

        // Add last measure if it has content
        // (If we just processed a separator, the empty measure was already pushed)
        if oneshot_bar_pending
            || !st.current_measure.chords.is_empty()
            || !st.current_measure.rhythm_elements.is_empty()
            || !st.current_measure.figured_bass.is_empty()
            || !st.current_measure.staff_text.is_empty()
            || !st.current_measure.text_cues.is_empty()
            || !st.current_measure.melodies.is_empty()
        {
            st.measures.push(st.current_measure);
        }

        self.expand_phrase_repeats(
            st.measures,
            repeat_count,
            st.time_sig,
            st.beats_per_measure,
            section_type,
            section_measure_count,
        )
    }
}
