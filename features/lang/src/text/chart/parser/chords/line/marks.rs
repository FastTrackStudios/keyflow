//! Marks in a chord line that place no beats of their own: figures, inline
//! dynamics and hairpins, `%` simile, staff text, commands, stops, repeat
//! ends, key changes and `^` inversions.

use super::LineState;
use keyflow_proto::chart::commands::Command;

use crate::text::chart::parser::ChartParser;
use crate::text::chart::types::{ChordInstance, KeyChange, RhythmElement};
use crate::text::chord::ChordRhythm;
use crate::text::key::Key;
use crate::text::primitives::{Accidental, RootNotation};
use crate::text::time::{AbsolutePosition, MusicalDuration, MusicalPositionExt};
use keyflow_proto::chart::{Dynamic, Hairpin, HairpinKind, Placement, RepeatMark, StaffText};

impl<'a> ChartParser<'a> {
    /// A floating suspension figure written after a chord: the chord keeps
    /// sounding, the figure is recorded at the current beat, and a following
    /// slash group extends the held chord.
    pub(super) fn place_floating_suspension(
        st: &mut LineState,
        figure: &str,
        tokens_str: &[&str],
        token_idx: usize,
        skip_next_token: &mut bool,
    ) {
        let slash_follows = tokens_str
            .get(token_idx + 1)
            .is_some_and(|t| !t.is_empty() && t.trim_end_matches('.').chars().all(|c| c == '/'));

        if !st.current_measure.chords.is_empty() {
            // Case A — the current bar is still open (the held chord
            // carried explicit slashes, e.g. `Bb // 4-3 //`). The figure
            // continues that chord in the same bar; a following slash
            // group extends it (treated as a continuation).
            let beat_anchor = st
                .current_measure_beats
                .min((st.beats_per_measure - 1.0).max(0.0))
                .max(0.0);
            Self::push_suspension_at_current_beat(
                &mut st.current_measure,
                &figure,
                Placement::Above,
                beat_anchor,
                true,
            );
            st.last_token_was_slash = true;
            return;
        }

        if let Some(prev) = st.measures.last().and_then(|m| m.chords.last()).cloned() {
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
                let max = st.beats_per_measure as usize;
                tokens_str
                    .get(token_idx + 1)
                    .map(|t| t.chars().filter(|c| *c == '/').count())
                    .unwrap_or(1)
                    .clamp(1, max) as u8
            } else {
                st.beats_per_measure as u8
            };
            bar_chord.rhythm = ChordRhythm::slashes(figure_beats);
            bar_chord.duration = MusicalDuration::from_beats(f64::from(figure_beats), st.time_sig);
            st.current_measure
                .rhythm_elements
                .push(RhythmElement::Chord(bar_chord.clone()));
            st.current_measure.chords.push(bar_chord);
            Self::push_suspension_at_current_beat(
                &mut st.current_measure,
                &figure,
                Placement::Above,
                0.0,
                true,
            );
            st.current_measure_beats = f64::from(figure_beats);
            // The slash group's beats are now owned by the figure chord;
            // consume that token so it isn't re-applied.
            if slash_follows {
                *skip_next_token = true;
            }
            st.measure_has_slash_rhythm = true;
            st.last_token_was_slash = false;
            if (st.current_measure_beats - st.beats_per_measure).abs() < 0.001 {
                st.measures.push(st.current_measure.clone());
                st.current_measure = Self::fresh_measure(st.time_sig);
                st.current_measure_beats = 0.0;
                st.measure_has_slash_rhythm = false;
            }
            return;
        }

        // Degenerate: figure with no preceding chord. Record it and move
        // on rather than dropping it.
        Self::push_suspension_at_current_beat(
            &mut st.current_measure,
            &figure,
            Placement::Above,
            0.0,
            true,
        );
    }

    /// An inline `dyn <level>[@beat] [above|below]`. Returns how many of the
    /// tokens after it the annotation consumed.
    pub(super) fn inline_dynamic(
        st: &mut LineState,
        tokens_str: &[&str],
        token_idx: usize,
    ) -> usize {
        let mut skip_tokens = 0;
        if let Some(level_tok) = tokens_str.get(token_idx + 1) {
            let default_beat = st.current_measure_beats.floor().max(0.0) as u8 + 1;
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
                st.current_measure.classical_dynamics.push(Dynamic {
                    level,
                    beat,
                    placement: placement.unwrap_or(Placement::Below),
                });
            }
        }
        skip_tokens
    }

    /// An inline `hairpin <dir> <span> [above|below]`. Returns how many of the
    /// tokens after it the annotation consumed.
    pub(super) fn inline_hairpin(
        st: &mut LineState,
        tokens_str: &[&str],
        token_idx: usize,
    ) -> usize {
        let mut skip_tokens = 0;
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
                let (end_measure_offset, end_beat) = Self::parse_hairpin_end(end, start_beat);
                skip_tokens = 2;
                let placement = tokens_str
                    .get(token_idx + 3)
                    .and_then(|t| Self::parse_placement(t));
                if placement.is_some() {
                    skip_tokens = 3;
                }
                st.current_measure.hairpins.push(Hairpin {
                    kind,
                    start_beat,
                    end_measure_offset,
                    end_beat,
                    placement: placement.unwrap_or(Placement::Below),
                });
            }
        }
        skip_tokens
    }

    /// `%` / `%N`: close the bar being written, then repeat the bar before
    /// it (or the row before's last bar) `repeats` times.
    pub(super) fn push_simile(&self, st: &mut LineState, repeats: usize) {
        if !st.current_measure.chords.is_empty()
            || !st.current_measure.rhythm_elements.is_empty()
            || !st.current_measure.figured_bass.is_empty()
            || !st.current_measure.staff_text.is_empty()
            || !st.current_measure.text_cues.is_empty()
        {
            st.measures.push(st.current_measure.clone());
            st.current_measure = Self::fresh_measure(st.time_sig);
            st.current_measure_beats = 0.0;
            st.measure_has_slash_rhythm = false;
        }

        let previous_measure = st
            .measures
            .last()
            .cloned()
            .or_else(|| self.carried_measure.clone());
        if let Some(previous_measure) = previous_measure {
            let repeated = Self::simile_copy(&previous_measure);
            for _ in 0..repeats {
                st.measures.push(repeated.clone());
            }
        }

        st.just_processed_separator = false;
        st.measure_was_created_by_separator = false;
    }

    /// Quoted staff text (`"Let ring"`, `^"…"`): decide which bar and beat
    /// it belongs to, or hold it in `pending_staff_text` for the bar it opens.
    pub(super) fn place_staff_text(
        st: &mut LineState,
        text: String,
        placement: Placement,
        tokens_str: &[&str],
        token_idx: usize,
        pending_staff_text: &mut Vec<(StaffText, usize)>,
    ) {
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
        let current_has_music =
            !st.current_measure.chords.is_empty() || !st.current_measure.rhythm_elements.is_empty();
        // A full bar not yet closed (a bare chord may still take
        // slashes) is the bar before: text in front of the next
        // chord opens the next bar, at its first beat — not beat
        // five of this one.
        if current_has_music
            && st.current_measure_beats > st.beats_per_measure - 0.001
            && next_opens_a_bar
        {
            pending_staff_text.push((staff_text(1), st.measures.len()));
            return;
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
            let in_current = !st.current_measure.chords.is_empty();
            let target = if in_current {
                Some(&mut st.current_measure)
            } else {
                st.measures.last_mut()
            };
            if let Some(target) = target
                && let Some(onset) = Self::last_chord_onset(target, st.time_sig)
            {
                let beat = (onset.floor().max(0.0) as u8 + 1)
                    .min(st.beats_per_measure.ceil().max(1.0) as u8);
                target.staff_text.push(staff_text(beat));
                return;
            }
        }
        let beat = (st.current_measure_beats.floor().max(0.0) as u8 + 1)
            .min(st.beats_per_measure.ceil().max(1.0) as u8);
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
            st.measure_was_created_by_separator || st.measures.is_empty() || next_opens_a_bar;
        let current_is_empty = st.current_measure.chords.is_empty()
            && st.current_measure.rhythm_elements.is_empty()
            && st.current_measure.figured_bass.is_empty()
            && st.current_measure.staff_text.is_empty();
        let target_measure = if current_is_empty && !opens_this_measure {
            st.measures.last_mut().unwrap_or(&mut st.current_measure)
        } else {
            &mut st.current_measure
        };
        target_measure.staff_text.push(staff_text(beat));
    }

    /// A `\command` applies to the previous chord: the last one in this bar,
    /// or the previous bar's last when this bar has none yet.
    pub(super) fn apply_command(st: &mut LineState, cmd: Command) {
        // Apply command to the last chord in rhythm_elements (source of truth)
        // and also to chords for backward compatibility during parsing
        let applied =
            if let Some(RhythmElement::Chord(c)) = st.current_measure.rhythm_elements.last_mut() {
                c.commands.push(cmd.clone());
                true
            } else {
                false
            };

        // Also apply to chords vec for backward compatibility
        if let Some(last_chord) = st.current_measure.chords.last_mut() {
            last_chord.commands.push(cmd.clone());
        }

        // If current measure has no chord, try previous measure
        if !applied && !st.measures.is_empty() {
            if let Some(last_measure) = st.measures.last_mut() {
                if let Some(RhythmElement::Chord(c)) = last_measure.rhythm_elements.last_mut() {
                    c.commands.push(cmd.clone());
                }
                if let Some(last_chord) = last_measure.chords.last_mut() {
                    last_chord.commands.push(cmd);
                }
            }
        }
    }

    /// `@repeat-end[:N]`: the bar it closes gets the backward repeat and its
    /// pass count, and a fresh bar starts.
    pub(super) fn close_repeat(st: &mut LineState, passes: &str) {
        // A repeat with no count on it plays twice — that is what the
        // sign means — so the count is always known and can always be
        // printed.
        let passes: usize = passes
            .strip_prefix(':')
            .and_then(|n| n.parse().ok())
            .filter(|n| *n > 1)
            .unwrap_or(2);
        if st.current_measure.chords.is_empty()
            && st.current_measure.rhythm_elements.is_empty()
            && st.current_measure.figured_bass.is_empty()
            && st.current_measure.volta_start.is_none()
        {
            if let Some(measure) = st.measures.last_mut() {
                measure.end_repeat = RepeatMark::Backward;
                measure.repeat_count = passes;
            }
        } else {
            st.current_measure.end_repeat = RepeatMark::Backward;
            st.current_measure.repeat_count = passes;
            Self::finalize_measure_for_separator(
                &mut st.measures,
                &st.current_measure,
                st.measure_was_created_by_separator,
            );
        }
        st.current_measure = Self::fresh_measure(st.time_sig);
        st.current_measure_beats = 0.0;
        st.just_processed_separator = true;
        st.measure_was_created_by_separator = false;
        st.measure_has_slash_rhythm = false;
    }

    /// A stop sign (`!STOP`): after the chord before it, or pending for the
    /// next chord when a barline came first.
    pub(super) fn place_stop(
        st: &mut LineState,
        stop_cmd: Command,
        pending_stop: &mut Option<Command>,
    ) {
        // If there's already a chord in this measure, attach as "after" to it
        if let Some(last) = st.current_measure.chords.last_mut() {
            last.commands.push(stop_cmd.clone().to_stop_after());
            // Also update rhythm_elements for consistency
            if let Some(crate::text::chart::types::RhythmElement::Chord(c)) =
                st.current_measure.rhythm_elements.last_mut()
            {
                c.commands.push(stop_cmd.to_stop_after());
            }
        } else if !st.just_processed_separator && !st.measure_was_created_by_separator {
            // Current measure is empty but NOT because of a `|` separator —
            // it was auto-pushed when the previous chord filled the measure.
            // (e.g., "C !STOP" where C's auto-duration fills the measure)
            if let Some(prev_measure) = st.measures.last_mut() {
                if let Some(last) = prev_measure.chords.last_mut() {
                    last.commands.push(stop_cmd.clone().to_stop_after());
                }
                if let Some(crate::text::chart::types::RhythmElement::Chord(c)) =
                    prev_measure.rhythm_elements.last_mut()
                {
                    c.commands.push(stop_cmd.to_stop_after());
                }
            } else {
                *pending_stop = Some(stop_cmd);
            }
        } else {
            // After a `|` separator — pend for the next chord (renders before)
            *pending_stop = Some(stop_cmd);
        }
    }

    /// A key change (`#G`, `bBb`) at the current, section-relative position.
    pub(super) fn record_key_change(&mut self, st: &LineState, new_key: Key) {
        // Section index = number of sections already pushed onto the chart
        // (the in-progress section hasn't been pushed yet).
        let section_index = self.chart.sections.len();

        // Position is section-relative: beats consumed by earlier
        // lines of this section (`section_beats_offset`, published
        // by parse_section_measures) plus measures finished in this
        // line plus beats into the current measure.
        let beats_into_section = self.section_beats_offset
            + (st.measures.len() as f64) * st.beats_per_measure
            + st.current_measure_beats;
        let position = AbsolutePosition::new(
            MusicalDuration::from_beats(beats_into_section, st.time_sig),
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
    }

    /// Realise a `^` inversion: set the slash bass to the chord tone
    /// `bass_thirds` thirds above the root, keeping `display` for the chart.
    pub(super) fn realise_caret_inversion(
        &self,
        chord: &mut ChordInstance,
        inversion: &(String, u8),
    ) {
        let (display, bass_thirds) = inversion;
        if let Some(root_deg) = chord.parsed.root.scale_degree() {
            // Diatonic position of the bass tone (root + N thirds).
            let bass_deg = ((u16::from(root_deg) - 1 + u16::from(*bass_thirds) * 2) % 7) as u8 + 1;
            // Spell it exactly: with a key in hand the bass is the
            // chord's *actual* 3rd/5th/7th, so a chromatic chord
            // tone (e.g. the G♯ of `III`) gets the right accidental.
            let accidental = self.current_key.as_ref().and_then(|key| {
                let notes = chord.parsed.notes(Some(key))?;
                let actual = notes.get(*bass_thirds as usize)?;
                let diatonic = key.get_scale_degree(bass_deg)?;
                match (i16::from(actual.semitone) - i16::from(diatonic.semitone)).rem_euclid(12) {
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
}
