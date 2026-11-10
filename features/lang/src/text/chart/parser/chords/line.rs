//! One chord line into measures: the token loop.

use super::groups::{beats_to_lily_suffix, expand_chord_groups};
use super::measure::dotted_slash_beats;
use super::tokens::tokenize_with_spans;
use crate::text::chart::cues::TextCue;
use crate::text::chart::dynamics::DynamicMarking;
use crate::text::chart::melody::Melody;
use crate::text::chart::parser::ChartParser;
use crate::text::chart::types::{
    ChordInstance, KeyChange, Measure, RestInstance, RhythmElement, SpaceInstance,
};
use crate::text::chord::{ChordRhythm, LilySyntax, NotationSystem};
use crate::text::key::Key;
use crate::text::parsing::{Lexer, TextSpan};
use crate::text::primitives::{Accidental, RootNotation};
use crate::text::sections::SectionType;
use crate::text::time::{
    AbsolutePosition, MusicalDuration, MusicalPosition, MusicalPositionExt, TimeSignature,
    TimeSignatureExt,
};
use keyflow_proto::chart::{Dynamic, Hairpin, HairpinKind, Placement, RepeatMark, StaffText};

impl<'a> ChartParser<'a> {
    pub(super) fn parse_chord_line_inner(
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

        let mut time_sig = self.time_signature.unwrap_or(TimeSignature::common_time());
        let mut beats_per_measure = time_sig.numerator as f64;

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
        let mut measures: Vec<Measure> = Vec::new();
        let mut current_measure = Self::fresh_measure(time_sig);
        let mut current_measure_beats: f64 = 0.0;
        let mut pending_cue: Option<TextCue> = None;
        // Text written after a full bar and before the next chord opens the
        // next bar. It waits here, with the bar count it was written at,
        // until that bar has been started.
        let mut pending_staff_text: Vec<(StaffText, usize)> = Vec::new();
        let mut pending_dynamic: Option<DynamicMarking> = None;
        let mut pending_stop: Option<Command> = None;
        let mut just_processed_separator = false; // Track if we just processed a | separator
        let mut measure_was_created_by_separator = false; // Track if current measure was created by |
        let mut in_melody_block = false; // Track if we're inside a m{...} block
        let mut melody_search_offset = 0usize; // Cursor for finding successive m{ blocks in line_to_parse
        let mut line_melody_octave = default_melody_octave;
        let mut last_chord: Option<ChordInstance> = None; // Track last chord for dot repeat
        let mut measure_has_slash_rhythm = false; // Track if current measure has chords with slash rhythm
        let mut last_token_was_slash = false; // Track for slash accumulation (consecutive slashes accumulate)
        let mut pending_start_repeat = false;
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

        for (token_idx, token_str) in tokens_str.iter().enumerate() {
            // Text waiting for the bar after a full one: once that bar has
            // begun, the text opens it.
            if pending_staff_text
                .first()
                .is_some_and(|(_, written_at)| measures.len() > *written_at)
            {
                for (text, _) in pending_staff_text.drain(..) {
                    current_measure.staff_text.push(text);
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
                if measures.len() > oneshot_measure_idx {
                    time_sig = revert_ts;
                    beats_per_measure = revert_bpm;
                    current_measure.time_signature =
                        (time_sig.numerator as u8, time_sig.denominator as u8);
                    oneshot_revert = None;
                }
            }

            // Floating suspension figure (`4-3`, `2-3`, `3`, `2`): the prior
            // chord keeps sounding; record the figure at the current beat and
            // let following slash groups extend the held chord. Consumes no
            // beats of its own and never counts as rhythm.
            if let Some(figure) = Self::parse_bare_suspension_figure(token_str) {
                let slash_follows = tokens_str.get(token_idx + 1).is_some_and(|t| {
                    !t.is_empty() && t.trim_end_matches('.').chars().all(|c| c == '/')
                });

                if !current_measure.chords.is_empty() {
                    // Case A — the current bar is still open (the held chord
                    // carried explicit slashes, e.g. `Bb // 4-3 //`). The figure
                    // continues that chord in the same bar; a following slash
                    // group extends it (treated as a continuation).
                    let beat_anchor = current_measure_beats
                        .min((beats_per_measure - 1.0).max(0.0))
                        .max(0.0);
                    Self::push_suspension_at_current_beat(
                        &mut current_measure,
                        &figure,
                        Placement::Above,
                        beat_anchor,
                        true,
                    );
                    last_token_was_slash = true;
                    continue;
                }

                if let Some(prev) = measures.last().and_then(|m| m.chords.last()).cloned() {
                    // Case B — the prior bare chord already filled its bar
                    // (`F 4-3 ///`). The figure opens a NEW bar that restates the
                    // same chord carrying the figure. The following slash group
                    // sets how many beats the figure holds (`4-3 ///` = 3 beats),
                    // and the rest of the bar is filled by what comes after (so
                    // `... /D` lands on beat 4). With no slash, the figure fills
                    // the whole bar on its own.
                    let mut bar_chord = prev;
                    bar_chord.commands.clear();
                    bar_chord.push_pull = None;
                    // The figure itself is the visible symbol for this bar (its
                    // own `4-3`); hide the restated chord's name so the held
                    // harmony stays for playback without printing `F` over it.
                    bar_chord.display_override = Some(String::new());
                    let figure_beats = if slash_follows {
                        let max = beats_per_measure as usize;
                        tokens_str
                            .get(token_idx + 1)
                            .map(|t| t.chars().filter(|c| *c == '/').count())
                            .unwrap_or(1)
                            .clamp(1, max) as u8
                    } else {
                        beats_per_measure as u8
                    };
                    bar_chord.rhythm = ChordRhythm::slashes(figure_beats);
                    bar_chord.duration =
                        MusicalDuration::from_beats(f64::from(figure_beats), time_sig);
                    current_measure
                        .rhythm_elements
                        .push(RhythmElement::Chord(bar_chord.clone()));
                    current_measure.chords.push(bar_chord);
                    Self::push_suspension_at_current_beat(
                        &mut current_measure,
                        &figure,
                        Placement::Above,
                        0.0,
                        true,
                    );
                    current_measure_beats = f64::from(figure_beats);
                    // The slash group's beats are now owned by the figure chord;
                    // consume that token so it isn't re-applied.
                    if slash_follows {
                        skip_next_token = true;
                    }
                    measure_has_slash_rhythm = true;
                    last_token_was_slash = false;
                    if (current_measure_beats - beats_per_measure).abs() < 0.001 {
                        measures.push(current_measure.clone());
                        current_measure = Self::fresh_measure(time_sig);
                        current_measure_beats = 0.0;
                        measure_has_slash_rhythm = false;
                    }
                    continue;
                }

                // Degenerate: figure with no preceding chord. Record it and move
                // on rather than dropping it.
                Self::push_suspension_at_current_beat(
                    &mut current_measure,
                    &figure,
                    Placement::Above,
                    0.0,
                    true,
                );
                continue;
            }

            // Floating slash-bass (`/D`): inherit the previous chord's root as
            // `<root>/D` for harmony, but display the token verbatim. Rewrite
            // to the synthetic chord token here and route it through normal
            // chord parsing below; the original `/D` becomes the display text.
            let synth_slash_bass: Option<String> = Self::parse_floating_slash_bass(token_str)
                .zip(Self::previous_chord_root(&current_measure, &measures))
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
                if let Some(level_tok) = tokens_str.get(token_idx + 1) {
                    let default_beat = current_measure_beats.floor().max(0.0) as u8 + 1;
                    let (level_token, beat) = level_tok
                        .split_once('@')
                        .map(|(level, beat)| (level, beat.parse::<u8>().ok().unwrap_or(1)))
                        .unwrap_or((*level_tok, default_beat));
                    if let Some(level) = Self::parse_dynamic_level(level_token) {
                        skip_tokens = 1;
                        let placement = tokens_str
                            .get(token_idx + 2)
                            .and_then(|t| Self::parse_placement(t));
                        if placement.is_some() {
                            skip_tokens = 2;
                        }
                        current_measure.classical_dynamics.push(Dynamic {
                            level,
                            beat,
                            placement: placement.unwrap_or(Placement::Below),
                        });
                    }
                }
                continue;
            }

            if *token_str == "hairpin" {
                if let (Some(kind_tok), Some(span_tok)) =
                    (tokens_str.get(token_idx + 1), tokens_str.get(token_idx + 2))
                {
                    let kind = match *kind_tok {
                        "<" | "crescendo" | "cresc" => Some(HairpinKind::Crescendo),
                        ">" | "decrescendo" | "decresc" | "dim" | "diminuendo" => {
                            Some(HairpinKind::Decrescendo)
                        }
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        let (start, end) = span_tok
                            .split_once("..")
                            .or_else(|| span_tok.split_once('-'))
                            .unwrap_or(("1", "1"));
                        let start_beat = start
                            .trim_start_matches('@')
                            .parse::<u8>()
                            .ok()
                            .unwrap_or(1);
                        let (end_measure_offset, end_beat) =
                            Self::parse_hairpin_end(end, start_beat);
                        skip_tokens = 2;
                        let placement = tokens_str
                            .get(token_idx + 3)
                            .and_then(|t| Self::parse_placement(t));
                        if placement.is_some() {
                            skip_tokens = 3;
                        }
                        current_measure.hairpins.push(Hairpin {
                            kind,
                            start_beat,
                            end_measure_offset,
                            end_beat,
                            placement: placement.unwrap_or(Placement::Below),
                        });
                    }
                }
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
                    chord_length_override = Self::parse_chord_length_value(next, time_sig);
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
                if !current_measure.chords.is_empty()
                    || !current_measure.rhythm_elements.is_empty()
                    || !current_measure.figured_bass.is_empty()
                    || !current_measure.staff_text.is_empty()
                    || !current_measure.text_cues.is_empty()
                {
                    measures.push(current_measure.clone());
                    current_measure = Self::fresh_measure(time_sig);
                    current_measure_beats = 0.0;
                    measure_has_slash_rhythm = false;
                }

                let previous_measure = measures
                    .last()
                    .cloned()
                    .or_else(|| self.carried_measure.clone());
                if let Some(previous_measure) = previous_measure {
                    let repeated = Self::simile_copy(&previous_measure);
                    for _ in 0..repeats {
                        measures.push(repeated.clone());
                    }
                }

                just_processed_separator = false;
                measure_was_created_by_separator = false;
                continue;
            }

            if let Some((text, placement)) = Self::parse_quoted_text_token(token_str) {
                let staff_text = |beat: u8| StaffText {
                    text: text.clone(),
                    beat,
                    placement,
                    source_default_x: None,
                    boxed: false,
                    bold: false,
                    italic: false,
                };
                // Text written before more music on the line is a cue for
                // what follows it; text at the end of the line trails the
                // bar before it.
                let next_music = tokens_str[token_idx + 1..]
                    .iter()
                    .find(|t| Self::is_music_token(t));
                // Slashes after a bare chord still belong to its bar
                // (`C#m "…" /.`): only a new chord opens the next one.
                let next_opens_a_bar =
                    next_music.is_some_and(|t| !t.chars().all(|c| matches!(c, '/' | '.')));
                let current_has_music = !current_measure.chords.is_empty()
                    || !current_measure.rhythm_elements.is_empty();
                // A full bar not yet closed (a bare chord may still take
                // slashes) is the bar before: text in front of the next
                // chord opens the next bar, at its first beat — not beat
                // five of this one.
                if current_has_music
                    && current_measure_beats > beats_per_measure - 0.001
                    && next_opens_a_bar
                {
                    pending_staff_text.push((staff_text(1), measures.len()));
                    continue;
                }
                // Text written right after a chord, with that chord's bar
                // going on (`G7 "Let ring" /`), is a cue for that chord (a
                // new chord after it opens the next bar instead): it
                // goes under the chord's own beat — the chord may already
                // have closed its bar, which would otherwise leave the cue
                // at beat 1.
                let after_a_chord = token_idx
                    .checked_sub(1)
                    .and_then(|i| tokens_str.get(i))
                    .is_some_and(|t| Self::is_chord_token(t));
                if after_a_chord && !next_opens_a_bar {
                    let in_current = !current_measure.chords.is_empty();
                    let target = if in_current {
                        Some(&mut current_measure)
                    } else {
                        measures.last_mut()
                    };
                    if let Some(target) = target
                        && let Some(onset) = Self::last_chord_onset(target, time_sig)
                    {
                        let beat = (onset.floor().max(0.0) as u8 + 1)
                            .min(beats_per_measure.ceil().max(1.0) as u8);
                        target.staff_text.push(staff_text(beat));
                        continue;
                    }
                }
                let beat = (current_measure_beats.floor().max(0.0) as u8 + 1)
                    .min(beats_per_measure.ceil().max(1.0) as u8);
                // Text sitting between two chords belongs to the measure it
                // opens, not the one it follows. `| Bm | ^"Back to top" G |`
                // is a cue over the second bar — attaching it to whatever
                // `measures.last()` happens to be put it over the first, and
                // when the split parse gave each bar its own pass there was no
                // previous measure to hold it and the cue was dropped
                // altogether.
                //
                // An empty current measure that a barline *just* opened is the
                // measure being written. One that is empty for any other
                // reason means the text trails the bar before it.
                let opens_this_measure =
                    measure_was_created_by_separator || measures.is_empty() || next_opens_a_bar;
                let current_is_empty = current_measure.chords.is_empty()
                    && current_measure.rhythm_elements.is_empty()
                    && current_measure.figured_bass.is_empty()
                    && current_measure.staff_text.is_empty();
                let target_measure = if current_is_empty && !opens_this_measure {
                    measures.last_mut().unwrap_or(&mut current_measure)
                } else {
                    &mut current_measure
                };
                target_measure.staff_text.push(staff_text(beat));
                continue;
            }

            let token_for_barline = token_str
                .split_once('_')
                .map_or(*token_str, |(base, _)| base);
            if token_for_barline.starts_with("|{") && token_for_barline.ends_with('}') {
                Self::finalize_measure_for_separator(
                    &mut measures,
                    &current_measure,
                    measure_was_created_by_separator,
                );
                current_measure = Self::fresh_measure(time_sig);
                current_measure_beats = 0.0;
                just_processed_separator = true;
                measure_was_created_by_separator = true;
                measure_has_slash_rhythm = false;
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
            }

            // Slash runs are rhythm, not keywords: `/`, `//`, `//.`. They used
            // to share the branch above, back when a command was also written
            // with a leading slash — splitting the two is the whole reason
            // commands moved to a backslash.
            if token_str.starts_with('/') && display_override.is_none() {
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
                        let dotted_duration = slash_count as f64 * dotted_slash_beats(time_sig);

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

                        if last_token_was_slash && !current_measure.chords.is_empty() {
                            let space_duration =
                                MusicalDuration::from_beats(dotted_duration, time_sig);
                            let space = SpaceInstance::new(
                                dotted_slash_rhythm.clone(),
                                space_duration,
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
                                .push(RhythmElement::Space(space));
                            current_measure_beats += dotted_duration;
                            measure_has_slash_rhythm = true;

                            if !just_processed_separator
                                && (current_measure_beats - beats_per_measure).abs() < 0.001
                            {
                                measures.push(current_measure.clone());
                                current_measure = Self::fresh_measure(time_sig);
                                current_measure_beats = 0.0;
                                measure_has_slash_rhythm = false;
                            }

                            last_token_was_slash = true;
                            continue;
                        }

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
                            if !current_measure.chords.is_empty()
                                || !current_measure.rhythm_elements.is_empty()
                            {
                                current_measure_beats = current_measure
                                    .chords
                                    .iter()
                                    .map(|c| c.duration.to_beats(time_sig))
                                    .sum();
                                measure_has_slash_rhythm = true;
                            } else if !measures.is_empty() {
                                let prev_measure_beats: f64 = measures
                                    .last()
                                    .unwrap()
                                    .chords
                                    .iter()
                                    .map(|c| c.duration.to_beats(time_sig))
                                    .sum();

                                if prev_measure_beats < beats_per_measure - 0.001 {
                                    current_measure = measures.pop().unwrap();
                                    current_measure_beats = prev_measure_beats;
                                    measure_has_slash_rhythm = true;
                                }
                            }
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
                                // Beats the previous bar already holds in chords *before* its
                                // last one — captured here so the tie-across-barline branch can
                                // tell whether extending the fill chord would overflow the bar
                                // (computing it later conflicts with the `&mut last_chord` borrow).
                                let prev_other_beats: f64 = measures
                                    .last()
                                    .map(|m| {
                                        let idx = m.chords.len().saturating_sub(1);
                                        m.chords[..idx]
                                            .iter()
                                            .map(|c| c.duration.to_beats(time_sig))
                                            .sum()
                                    })
                                    .unwrap_or(0.0);
                                let next_measure_index = measures.len();
                                let sections_len = self.sections.len();

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
                                            // Beats the previous bar holds in chords *other* than
                                            // this one (the fill chord we're about to extend),
                                            // captured before the `&mut last_chord` borrow above.
                                            let other_beats = prev_other_beats;
                                            if other_beats + f64::from(new_count)
                                                > beats_per_measure + 0.001
                                            {
                                                // Setting the fill chord to `new_count` would
                                                // overflow its already-full bar (e.g. the trailing
                                                // `//` in `Eb/D / Eb // Eb/F //`). Tie the chord
                                                // across the barline: it fills the rest of this bar,
                                                // and the leftover beats become continuation slashes
                                                // at the start of the next measure.
                                                let in_bar =
                                                    (beats_per_measure - other_beats).max(1.0);
                                                let overflow =
                                                    (f64::from(new_count) - in_bar).max(0.0);
                                                let tied = ChordRhythm::Slashes {
                                                    count: in_bar as u8,
                                                    dotted: false,
                                                    tied: true,
                                                };
                                                let in_bar_dur =
                                                    MusicalDuration::from_beats(in_bar, time_sig);
                                                last_chord.rhythm = tied.clone();
                                                last_chord.duration = in_bar_dur;
                                                for elem in
                                                    last_measure.rhythm_elements.iter_mut().rev()
                                                {
                                                    if let RhythmElement::Chord(c) = elem {
                                                        c.rhythm = tied.clone();
                                                        c.duration = in_bar_dur;
                                                        break;
                                                    }
                                                }
                                                let overflow_beats = overflow.round() as u32;
                                                for i in 0..overflow_beats {
                                                    let space = SpaceInstance::new(
                                                        ChordRhythm::slashes(1),
                                                        MusicalDuration::from_beats(1.0, time_sig),
                                                        AbsolutePosition::new(
                                                            MusicalPosition::try_new(
                                                                next_measure_index as i32,
                                                                i as i32,
                                                                0,
                                                            )
                                                            .unwrap_or_else(|_| {
                                                                MusicalPosition::start()
                                                            }),
                                                            sections_len,
                                                        ),
                                                        format!("/{}", "/".repeat(i as usize)),
                                                    );
                                                    current_measure
                                                        .rhythm_elements
                                                        .push(RhythmElement::Space(space));
                                                }
                                                current_measure_beats += f64::from(overflow_beats);
                                            } else {
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
                                    // Resync beats with the (possibly extended) chord
                                    // durations. Without this, a slash group that
                                    // accumulates onto the last chord — e.g. the second
                                    // `//` in `Bb // 4-3 //`, or `Bb // //` — would leave
                                    // current_measure_beats stale and let the next chord
                                    // wrongly share the measure. Mirrors the dotted-slash
                                    // path above.
                                    current_measure_beats =
                                        Self::measure_beats(&current_measure, time_sig);
                                    // Mark that this measure has slash rhythm, so subsequent
                                    // chords can fill remaining beats
                                    measure_has_slash_rhythm = true;
                                } else if !measures.is_empty() {
                                    // The slash was applied to the previous measure (SET mode).
                                    // Check if it now has room and we should "un-pop" it.
                                    let prev_measure_beats =
                                        Self::measure_beats(measures.last().unwrap(), time_sig);

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

            // Check for melody variable reference (e.g., "$mainRiff").
            //
            // If the previous chord auto-completed the measure (whole-note,
            // half-note that filled remaining beats, etc.), `current_measure`
            // is empty here; attach the recalled melody to the most recently
            // pushed measure instead. Mirrors the inline `m{...}` branch.
            if let Some(var_name) = token_str.strip_prefix('$') {
                if let Some(melody) = self.melody_variables.get(var_name).cloned() {
                    if current_measure.chords.is_empty() && !measures.is_empty() {
                        measures.last_mut().unwrap().melodies.push(melody);
                    } else {
                        current_measure.melodies.push(melody);
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
                                if current_measure.chords.is_empty() && !measures.is_empty() {
                                    measures.last_mut().unwrap().melodies.push(melody);
                                } else {
                                    current_measure.melodies.push(melody);
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
                Self::finalize_measure_for_separator(&mut measures, &current_measure, false);
                current_measure = Self::fresh_measure(time_sig);
                current_measure.start_repeat = RepeatMark::Forward;
                pending_start_repeat = true;
                current_measure_beats = 0.0;
                just_processed_separator = true;
                measure_was_created_by_separator = true;
                measure_has_slash_rhythm = false;
                continue;
            }

            if let Some(passes) = token_str.strip_prefix("@repeat-end") {
                // A repeat with no count on it plays twice — that is what the
                // sign means — so the count is always known and can always be
                // printed.
                let passes: usize = passes
                    .strip_prefix(':')
                    .and_then(|n| n.parse().ok())
                    .filter(|n| *n > 1)
                    .unwrap_or(2);
                if current_measure.chords.is_empty()
                    && current_measure.rhythm_elements.is_empty()
                    && current_measure.figured_bass.is_empty()
                    && current_measure.volta_start.is_none()
                {
                    if let Some(measure) = measures.last_mut() {
                        measure.end_repeat = RepeatMark::Backward;
                        measure.repeat_count = passes;
                    }
                } else {
                    current_measure.end_repeat = RepeatMark::Backward;
                    current_measure.repeat_count = passes;
                    Self::finalize_measure_for_separator(
                        &mut measures,
                        &current_measure,
                        measure_was_created_by_separator,
                    );
                }
                current_measure = Self::fresh_measure(time_sig);
                current_measure_beats = 0.0;
                just_processed_separator = true;
                measure_was_created_by_separator = false;
                measure_has_slash_rhythm = false;
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
                // If there's already a chord in this measure, attach as "after" to it
                if let Some(last) = current_measure.chords.last_mut() {
                    last.commands.push(stop_cmd.clone().to_stop_after());
                    // Also update rhythm_elements for consistency
                    if let Some(crate::text::chart::types::RhythmElement::Chord(c)) =
                        current_measure.rhythm_elements.last_mut()
                    {
                        c.commands.push(stop_cmd.to_stop_after());
                    }
                } else if !just_processed_separator && !measure_was_created_by_separator {
                    // Current measure is empty but NOT because of a `|` separator —
                    // it was auto-pushed when the previous chord filled the measure.
                    // (e.g., "C !STOP" where C's auto-duration fills the measure)
                    if let Some(prev_measure) = measures.last_mut() {
                        if let Some(last) = prev_measure.chords.last_mut() {
                            last.commands.push(stop_cmd.clone().to_stop_after());
                        }
                        if let Some(crate::text::chart::types::RhythmElement::Chord(c)) =
                            prev_measure.rhythm_elements.last_mut()
                        {
                            c.commands.push(stop_cmd.to_stop_after());
                        }
                    } else {
                        pending_stop = Some(stop_cmd);
                    }
                } else {
                    // After a `|` separator — pend for the next chord (renders before)
                    pending_stop = Some(stop_cmd);
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

            if let Some(volta) = Self::parse_volta_token(token_str) {
                current_measure.volta_start = Some(volta);
                continue;
            }

            // Check for measure separator (|)
            // This forces a measure boundary regardless of beat count
            if *token_str == "|" {
                // Finalize current measure if it has chords, or if it was created by a previous separator
                // (This allows multiple | in a row to create empty measures)
                // But don't push auto-created empty measures (created when a measure fills up)
                Self::finalize_measure_for_separator(
                    &mut measures,
                    &current_measure,
                    measure_was_created_by_separator,
                );
                // Always start a new measure after |
                current_measure = Self::fresh_measure(time_sig);
                current_measure_beats = 0.0;
                just_processed_separator = true; // Mark that we just processed a separator
                measure_was_created_by_separator = true; // Mark that this measure was created by |
                measure_has_slash_rhythm = false; // Reset for new measure
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
                        if !current_measure.chords.is_empty() {
                            measures.push(current_measure.clone());
                            current_measure = Self::fresh_measure(time_sig);
                            current_measure_beats = 0.0;
                        }

                        if oneshot {
                            // Remember what to restore once this single measure
                            // is pushed. Leave self.time_signature untouched so
                            // the prevailing meter still governs later lines.
                            oneshot_revert = Some((time_sig, beats_per_measure));
                            oneshot_measure_idx = measures.len();
                        } else {
                            // Persistent change: subsequent measures keep this.
                            self.time_signature = Some(TimeSignature::new(num as u32, den as u32));
                        }

                        time_sig = TimeSignature::new(num as u32, den as u32);
                        current_measure.time_signature = (num, den);
                        beats_per_measure = num as f64;
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
                    // Section index = number of sections already pushed onto the chart
                    // (the in-progress section hasn't been pushed yet).
                    let section_index = self.chart.sections.len();

                    // Position is section-relative: beats consumed by earlier
                    // lines of this section (`section_beats_offset`, published
                    // by parse_section_measures) plus measures finished in this
                    // line plus beats into the current measure.
                    let beats_into_section = self.section_beats_offset
                        + (measures.len() as f64) * beats_per_measure
                        + current_measure_beats;
                    let position = AbsolutePosition::new(
                        MusicalDuration::from_beats(beats_into_section, time_sig),
                        section_index,
                    );

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
                        .map_or(beats_per_measure, |(_, duration)| {
                            duration.to_beats(time_sig)
                        })
                };
                let length = beats_to_lily_suffix(beats, time_sig)
                    .unwrap_or_else(|| format!("_{}*{}", time_sig.denominator, time_sig.numerator));
                Some(format!("{token_str}{length}"))
            } else {
                None
            };
            let rest_token: &str = bare_rest_token.as_deref().unwrap_or(token_str);

            // Check for standalone rest token (r4, r8, r8t, r4t, r2, etc.)
            // These don't have a chord symbol but should be stored as rhythm elements
            if rest_token.starts_with('r') && rest_token.len() >= 2 {
                // Try to parse as a rest
                let mut lexer = Lexer::new(rest_token.to_string());
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
                            written_token.clone(),
                        );

                        // A bar that opens on a rest still owns its `|:`; leaving it
                        // pending hands it to the next chord, bars later.
                        if pending_start_repeat && current_measure.chords.is_empty() {
                            current_measure.start_repeat = RepeatMark::Forward;
                            pending_start_repeat = false;
                        }
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
            if rest_token.starts_with('s') && rest_token.len() >= 2 {
                // Try to parse as a space
                let mut lexer = Lexer::new(rest_token.to_string());
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
                            written_token.clone(),
                        );

                        // A bar that opens on a space still owns its `|:`.
                        if pending_start_repeat && current_measure.chords.is_empty() {
                            current_measure.start_repeat = RepeatMark::Forward;
                            pending_start_repeat = false;
                        }
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

            // `.` repeats the last chord for as long as that chord lasted —
            // a direct repeat, not a measure one. `.3` is three of them.
            // (`apply_auto_durations` may have appended `_N`, hence `._4`.)
            if let Some(dot_repeats) = Self::parse_dot_repeat(token_str, chord_system) {
                // Take the chord as it stands *now*, not as it was when its
                // token was read. A slash run is a token of its own, so in
                // `G // .` the `//` lands on `G` after `last_chord` was
                // snapshotted — and a `.` reading the snapshot inherits a
                // whole bar instead of the two beats `G` actually got, which
                // turns `G // . . .` into four measures instead of two.
                let live_chord = current_measure
                    .chords
                    .last()
                    .or_else(|| measures.last().and_then(|m| m.chords.last()))
                    .cloned()
                    .or_else(|| last_chord.clone())
                    .or_else(|| {
                        // Nothing on this line yet: the chord to repeat is the
                        // last one of the previous row.
                        self.carried_measure
                            .as_ref()
                            .and_then(|m| m.chords.last().cloned())
                    });
                if let Some(ref prev_chord) = live_chord {
                    for _ in 0..dot_repeats {
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
                }
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
                time_sig,
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
                    if let Some((display, bass_thirds)) = &inversion_apply {
                        if let Some(root_deg) = chord.parsed.root.scale_degree() {
                            // Diatonic position of the bass tone (root + N thirds).
                            let bass_deg = ((u16::from(root_deg) - 1 + u16::from(*bass_thirds) * 2)
                                % 7) as u8
                                + 1;
                            // Spell it exactly: with a key in hand the bass is the
                            // chord's *actual* 3rd/5th/7th, so a chromatic chord
                            // tone (e.g. the G♯ of `III`) gets the right accidental.
                            let accidental = self.current_key.as_ref().and_then(|key| {
                                let notes = chord.parsed.notes(Some(key))?;
                                let actual = notes.get(*bass_thirds as usize)?;
                                let diatonic = key.get_scale_degree(bass_deg)?;
                                match (i16::from(actual.semitone) - i16::from(diatonic.semitone))
                                    .rem_euclid(12)
                                {
                                    0 => None,
                                    1 => Some(Accidental::Sharp),
                                    11 => Some(Accidental::Flat),
                                    _ => None,
                                }
                            });
                            let bass = RootNotation::from_scale_degree(bass_deg, accidental);
                            chord.parsed.set_bass(bass);
                            chord.full_symbol = display.clone();
                            chord.display_override = Some(display.clone());
                        }
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
                    let starts_a_bar = current_measure_beats < 0.001
                        || current_measure_beats > beats_per_measure - 0.001;
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

                    // Attach pending stop command to this chord
                    if let Some(stop_cmd) = pending_stop.take() {
                        chord.commands.push(stop_cmd);
                    }

                    if pending_start_repeat && current_measure.chords.is_empty() {
                        current_measure.start_repeat = RepeatMark::Forward;
                        pending_start_repeat = false;
                    }

                    // Store as last chord for dot repeat
                    last_chord = Some(chord.clone());

                    // Add chord to both chords (for backward compat) and rhythm_elements
                    current_measure
                        .rhythm_elements
                        .push(RhythmElement::Chord(chord.clone()));
                    current_measure.chords.push(chord);

                    if let Some(rows) = figured_bass_rows {
                        Self::push_figured_bass_at_current_beat(
                            &mut current_measure,
                            &rows,
                            Placement::Above,
                            current_measure_beats,
                        );
                    }

                    // Attached suspension figure (`Eb2`, `F4-3`) anchors at the
                    // chord's own beat and hugs it as a superscript.
                    if let Some(figure) = &suspension_figure {
                        Self::push_suspension_at_current_beat(
                            &mut current_measure,
                            figure,
                            Placement::Above,
                            current_measure_beats,
                            false,
                        );
                    }

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

        // A one-shot meter change still waiting for its measure when the line
        // ends is a bar of that meter with no new chord in it — `!T2/4` on a
        // line of its own is "one bar of 2/4", the breakdown bar of a
        // structure-only chart. Without this the bar was dropped and the
        // section fell back to the prevailing meter.
        let oneshot_bar_pending = oneshot_revert.is_some() && measures.len() == oneshot_measure_idx;

        // Text still waiting for its bar when the line ends: the music after
        // it never opened one (it was a mark, not a chord), so it stays on
        // the bar it was written after, at that bar's last beat.
        for (mut text, _) in pending_staff_text.drain(..) {
            text.beat = text.beat.max(current_measure_beats.ceil().max(1.0) as u8);
            current_measure.staff_text.push(text);
        }

        // Add last measure if it has content
        // (If we just processed a separator, the empty measure was already pushed)
        if oneshot_bar_pending
            || !current_measure.chords.is_empty()
            || !current_measure.rhythm_elements.is_empty()
            || !current_measure.figured_bass.is_empty()
            || !current_measure.staff_text.is_empty()
            || !current_measure.text_cues.is_empty()
            || !current_measure.melodies.is_empty()
        {
            measures.push(current_measure);
        }

        self.expand_phrase_repeats(
            measures,
            repeat_count,
            time_sig,
            beats_per_measure,
            section_type,
            section_measure_count,
        )
    }
}
