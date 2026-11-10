//! Tokens that carry beats besides a chord: slash runs, ties, rests,
//! spaces and `.` repeats, and making room in the bar for a chord.

use super::LineState;

use crate::text::chart::parser::ChartParser;
use crate::text::chart::parser::chords::measure::dotted_slash_beats;
use crate::text::chart::types::{
    ChordInstance, Measure, RestInstance, RhythmElement, SpaceInstance,
};
use crate::text::chord::{ChordRhythm, LilySyntax};
use crate::text::parsing::Lexer;
use crate::text::time::{AbsolutePosition, MusicalDuration, MusicalPosition, MusicalPositionExt};
use keyflow_proto::chart::RepeatMark;

impl<'a> ChartParser<'a> {
    /// A run of rhythm slashes (`/`, `//`, `//.`): set, extend or continue the
    /// previous chord's duration. Returns `false` when the run is left for the
    /// tie handling (the previous chord is tied) or nothing took it.
    pub(super) fn apply_slash_run(&self, st: &mut LineState, token_str: &str) -> bool {
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
                let dotted_duration = slash_count as f64 * dotted_slash_beats(st.time_sig);

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

                if st.last_token_was_slash && !st.current_measure.chords.is_empty() {
                    let space_duration = MusicalDuration::from_beats(dotted_duration, st.time_sig);
                    let space = SpaceInstance::new(
                        dotted_slash_rhythm.clone(),
                        space_duration,
                        AbsolutePosition::new(
                            MusicalPosition::try_new(
                                st.measures.len() as i32,
                                st.current_measure_beats as i32,
                                0,
                            )
                            .unwrap_or_else(|_| MusicalPosition::start()),
                            self.sections.len(),
                        ),
                        token_str.to_string(),
                    );
                    st.current_measure
                        .rhythm_elements
                        .push(RhythmElement::Space(space));
                    st.current_measure_beats += dotted_duration;
                    st.measure_has_slash_rhythm = true;

                    if !st.just_processed_separator
                        && (st.current_measure_beats - st.beats_per_measure).abs() < 0.001
                    {
                        st.measures.push(st.current_measure.clone());
                        st.current_measure = Self::fresh_measure(st.time_sig);
                        st.current_measure_beats = 0.0;
                        st.measure_has_slash_rhythm = false;
                    }

                    st.last_token_was_slash = true;
                    return true;
                }

                let applied = if let Some(last_chord) = st.current_measure.chords.last_mut() {
                    last_chord.rhythm = dotted_slash_rhythm.clone();
                    last_chord.duration = MusicalDuration::from_beats(dotted_duration, st.time_sig);

                    // Also update the chord in rhythm_elements
                    for elem in st.current_measure.rhythm_elements.iter_mut().rev() {
                        if let RhythmElement::Chord(c) = elem {
                            c.rhythm = dotted_slash_rhythm.clone();
                            c.duration = MusicalDuration::from_beats(dotted_duration, st.time_sig);
                            break;
                        }
                    }
                    true
                } else if !st.measures.is_empty() {
                    // Current measure is empty - apply to previous measure's last chord
                    // This happens when the chord auto-completed a measure (e.g., "C /.")
                    if let Some(last_measure) = st.measures.last_mut() {
                        if let Some(last_chord) = last_measure.chords.last_mut() {
                            last_chord.rhythm = dotted_slash_rhythm.clone();
                            last_chord.duration =
                                MusicalDuration::from_beats(dotted_duration, st.time_sig);

                            // Also update in rhythm_elements
                            for elem in last_measure.rhythm_elements.iter_mut().rev() {
                                if let RhythmElement::Chord(c) = elem {
                                    c.rhythm = dotted_slash_rhythm.clone();
                                    c.duration =
                                        MusicalDuration::from_beats(dotted_duration, st.time_sig);
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
                    if !st.current_measure.chords.is_empty()
                        || !st.current_measure.rhythm_elements.is_empty()
                    {
                        st.current_measure_beats = st
                            .current_measure
                            .chords
                            .iter()
                            .map(|c| c.duration.to_beats(st.time_sig))
                            .sum();
                        st.measure_has_slash_rhythm = true;
                    } else if !st.measures.is_empty() {
                        let prev_measure_beats: f64 = st
                            .measures
                            .last()
                            .unwrap()
                            .chords
                            .iter()
                            .map(|c| c.duration.to_beats(st.time_sig))
                            .sum();

                        if prev_measure_beats < st.beats_per_measure - 0.001 {
                            st.current_measure = st.measures.pop().unwrap();
                            st.current_measure_beats = prev_measure_beats;
                            st.measure_has_slash_rhythm = true;
                        }
                    }
                    st.last_token_was_slash = true;
                    return true;
                }
            }
        }

        if token_str.chars().all(|c| c == '/') {
            let slash_count = token_str.len() as u8;
            if slash_count > 0 {
                // Check if last chord is tied - if so, skip this block
                // and let the tie handling code (below) process this slash
                let last_chord_is_tied = st
                    .current_measure
                    .chords
                    .last()
                    .is_some_and(|c| c.rhythm.is_tied())
                    || (!st.measures.is_empty()
                        && st
                            .measures
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
                    let applied = if let Some(last_chord) = st.current_measure.chords.last_mut() {
                        // Only accumulate if previous token was also a slash (consecutive slashes).
                        // This prevents accumulating on top of auto-filled slash rhythm.
                        // "C // G //" - when we see first "/" after C, it should SET to 1 (not accumulate).
                        // When we see second "/" after first, it should accumulate to 2.
                        let new_count = if st.last_token_was_slash {
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
                            MusicalDuration::from_beats(f64::from(new_count), st.time_sig);

                        // Also update the chord in rhythm_elements
                        for elem in st.current_measure.rhythm_elements.iter_mut().rev() {
                            if let RhythmElement::Chord(c) = elem {
                                c.rhythm = ChordRhythm::slashes(new_count);
                                c.duration =
                                    MusicalDuration::from_beats(f64::from(new_count), st.time_sig);
                                break;
                            }
                        }
                        true
                    } else if !st.measures.is_empty() {
                        // Current measure is empty - slashes apply to previous measure's chord
                        // Check if the previous chord has EXPLICIT rhythm (slashes) vs DEFAULT (auto-fill)
                        // - If previous chord has Slashes: measure is explicitly full, add SPACES
                        // - If previous chord has Default: it was auto-filled, so SET the duration
                        let prev_chord_has_explicit_rhythm = st
                            .measures
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
                        let prev_other_beats: f64 = st
                            .measures
                            .last()
                            .map(|m| {
                                let idx = m.chords.len().saturating_sub(1);
                                m.chords[..idx]
                                    .iter()
                                    .map(|c| c.duration.to_beats(st.time_sig))
                                    .sum()
                            })
                            .unwrap_or(0.0);
                        let next_measure_index = st.measures.len();
                        let sections_len = self.sections.len();

                        if let Some(last_measure) = st.measures.last_mut() {
                            if let Some(last_chord) = last_measure.chords.last_mut() {
                                // If after explicit | separator OR previous chord has explicit rhythm,
                                // add SPACES to current measure instead of modifying previous chord.
                                // "Abmaj9 //// | //" = Abmaj9 (4 slashes) + 2 spaces (continuation)
                                // "C //// //" = C (4 slashes) + 2 spaces (explicit slashes = full)
                                // "Cm/Eb / Eb ///" = SET to 1 beat (Default rhythm = auto-filled)
                                if st.measure_was_created_by_separator
                                    || prev_chord_has_explicit_rhythm
                                {
                                    // After explicit separator: add SPACES to current measure
                                    // as continuation of the previous chord. These render as
                                    // rhythm slashes without a chord symbol.

                                    // Create a space for each beat of continuation
                                    for i in 0..slash_count {
                                        let space_duration =
                                            MusicalDuration::from_beats(1.0, st.time_sig);
                                        let space_rhythm = ChordRhythm::slashes(1);
                                        let space = SpaceInstance::new(
                                            space_rhythm,
                                            space_duration,
                                            AbsolutePosition::new(
                                                MusicalPosition::try_new(
                                                    st.measures.len() as i32,
                                                    i as i32,
                                                    0,
                                                )
                                                .unwrap_or_else(|_| MusicalPosition::start()),
                                                self.sections.len(),
                                            ),
                                            format!("/{}", "/".repeat(i as usize)),
                                        );
                                        st.current_measure
                                            .rhythm_elements
                                            .push(RhythmElement::Space(space));
                                    }
                                    st.current_measure_beats += f64::from(slash_count);
                                    true
                                } else {
                                    // Auto-completed measure: only accumulate if previous was slash
                                    let new_count = if st.last_token_was_slash {
                                        if let ChordRhythm::Slashes {
                                            count: existing, ..
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
                                        > st.beats_per_measure + 0.001
                                    {
                                        // Setting the fill chord to `new_count` would
                                        // overflow its already-full bar (e.g. the trailing
                                        // `//` in `Eb/D / Eb // Eb/F //`). Tie the chord
                                        // across the barline: it fills the rest of this bar,
                                        // and the leftover beats become continuation slashes
                                        // at the start of the next measure.
                                        let in_bar = (st.beats_per_measure - other_beats).max(1.0);
                                        let overflow = (f64::from(new_count) - in_bar).max(0.0);
                                        let tied = ChordRhythm::Slashes {
                                            count: in_bar as u8,
                                            dotted: false,
                                            tied: true,
                                        };
                                        let in_bar_dur =
                                            MusicalDuration::from_beats(in_bar, st.time_sig);
                                        last_chord.rhythm = tied.clone();
                                        last_chord.duration = in_bar_dur;
                                        for elem in last_measure.rhythm_elements.iter_mut().rev() {
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
                                                MusicalDuration::from_beats(1.0, st.time_sig),
                                                AbsolutePosition::new(
                                                    MusicalPosition::try_new(
                                                        next_measure_index as i32,
                                                        i as i32,
                                                        0,
                                                    )
                                                    .unwrap_or_else(|_| MusicalPosition::start()),
                                                    sections_len,
                                                ),
                                                format!("/{}", "/".repeat(i as usize)),
                                            );
                                            st.current_measure
                                                .rhythm_elements
                                                .push(RhythmElement::Space(space));
                                        }
                                        st.current_measure_beats += f64::from(overflow_beats);
                                    } else {
                                        last_chord.rhythm = ChordRhythm::slashes(new_count);
                                        last_chord.duration = MusicalDuration::from_beats(
                                            f64::from(new_count),
                                            st.time_sig,
                                        );

                                        // Also update the chord in rhythm_elements
                                        for elem in last_measure.rhythm_elements.iter_mut().rev() {
                                            if let RhythmElement::Chord(c) = elem {
                                                c.rhythm = ChordRhythm::slashes(new_count);
                                                c.duration = MusicalDuration::from_beats(
                                                    f64::from(new_count),
                                                    st.time_sig,
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
                        if !st.current_measure.chords.is_empty()
                            || !st.current_measure.rhythm_elements.is_empty()
                        {
                            // Resync beats with the (possibly extended) chord
                            // durations. Without this, a slash group that
                            // accumulates onto the last chord — e.g. the second
                            // `//` in `Bb // 4-3 //`, or `Bb // //` — would leave
                            // current_measure_beats stale and let the next chord
                            // wrongly share the measure. Mirrors the dotted-slash
                            // path above.
                            st.current_measure_beats =
                                Self::measure_beats(&st.current_measure, st.time_sig);
                            // Mark that this measure has slash rhythm, so subsequent
                            // chords can fill remaining beats
                            st.measure_has_slash_rhythm = true;
                        } else if !st.measures.is_empty() {
                            // The slash was applied to the previous measure (SET mode).
                            // Check if it now has room and we should "un-pop" it.
                            let prev_measure_beats =
                                Self::measure_beats(st.measures.last().unwrap(), st.time_sig);

                            if prev_measure_beats < st.beats_per_measure - 0.001 {
                                // Previous measure now has room - pop it back to current_measure
                                // so subsequent chords can be added to it
                                st.current_measure = st.measures.pop().unwrap();
                                st.current_measure_beats = prev_measure_beats;
                                // Mark that this measure has slash rhythm since we un-popped it
                                st.measure_has_slash_rhythm = true;
                            }
                            // Note: if we didn't un-pop (measure is full), don't set
                            // measure_has_slash_rhythm for the empty current measure
                        }
                        st.last_token_was_slash = true;
                        return true;
                    }
                } // end else !last_chord_is_tied
            }
        }
        false
    }

    /// `~`: mark the previous chord tied, so the next slash run adds to it.
    pub(super) fn mark_tie(st: &mut LineState) {
        // Find the last chord and mark it as having a pending tie
        // The next slash/duration token will add to this chord's duration
        let last_chord_found = if let Some(last_chord) = st.current_measure.chords.last_mut() {
            // Set tied flag using the new method
            last_chord.rhythm = last_chord.rhythm.clone().with_tie();
            // Also update in rhythm_elements
            for elem in st.current_measure.rhythm_elements.iter_mut().rev() {
                if let RhythmElement::Chord(c) = elem {
                    c.rhythm = c.rhythm.clone().with_tie();
                    break;
                }
            }
            true
        } else if !st.measures.is_empty() {
            if let Some(last_measure) = st.measures.last_mut() {
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
    }

    /// A slash run after a tie adds its beats to the tied chord. Returns
    /// whether there was a tied chord to extend.
    pub(super) fn extend_tied_chord(st: &mut LineState, token_str: &str) -> bool {
        let slash_count = token_str.len() as u8;

        // Check if the last chord is tied - if so, add to its duration
        let _last_chord_is_tied = {
            if let Some(last_chord) = st.current_measure.chords.last() {
                last_chord.rhythm.is_tied()
            } else if !st.measures.is_empty() {
                if let Some(last_measure) = st.measures.last() {
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
        {
            let mut found = false;
            if let Some(last_chord) = st.current_measure.chords.last_mut() {
                if last_chord.rhythm.is_tied() {
                    // Add slash duration to the tied chord
                    let current_beats = last_chord.duration.to_beats(st.time_sig);
                    let new_beats = current_beats + f64::from(slash_count);
                    last_chord.duration = MusicalDuration::from_beats(new_beats, st.time_sig);
                    // Clear the tie flag after extending
                    last_chord.rhythm.clear_tie();
                    // Update in rhythm_elements too
                    for elem in st.current_measure.rhythm_elements.iter_mut().rev() {
                        if let RhythmElement::Chord(c) = elem {
                            c.duration = last_chord.duration;
                            c.rhythm.clear_tie();
                            break;
                        }
                    }
                    found = true;
                }
            } else if !st.measures.is_empty() {
                if let Some(last_measure) = st.measures.last_mut() {
                    if let Some(last_chord) = last_measure.chords.last_mut() {
                        if last_chord.rhythm.is_tied() {
                            let current_beats = last_chord.duration.to_beats(st.time_sig);
                            let new_beats = current_beats + f64::from(slash_count);
                            last_chord.duration =
                                MusicalDuration::from_beats(new_beats, st.time_sig);
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
        }
    }

    /// A standalone rest (`r4`, `r8t`, …) as a rhythm element. Returns `false`
    /// when the token is not a rest.
    pub(super) fn place_rest(
        &self,
        st: &mut LineState,
        rest_token: &str,
        written_token: &str,
    ) -> bool {
        // Try to parse as a rest
        let mut lexer = Lexer::new(rest_token.to_string());
        let tokens = lexer.tokenize();
        if let Ok((rhythm, _)) = ChordRhythm::parse(&tokens) {
            if rhythm.is_rest() {
                let duration = rhythm.to_duration(st.time_sig);
                let chord_beats = duration.to_beats(st.time_sig);

                // Check if we need to start a new measure
                if !st.just_processed_separator
                    && st.current_measure_beats + chord_beats > st.beats_per_measure + 0.001
                {
                    if !st.current_measure.chords.is_empty()
                        || !st.current_measure.rhythm_elements.is_empty()
                    {
                        st.measures.push(st.current_measure.clone());
                    }
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                }
                st.just_processed_separator = false;
                st.measure_was_created_by_separator = false;

                // Create a RestInstance and add it to rhythm_elements
                let rest_instance = RestInstance::new(
                    rhythm,
                    duration,
                    AbsolutePosition::new(
                        MusicalPosition::try_new(
                            st.measures.len() as i32,
                            st.current_measure_beats as i32,
                            0,
                        )
                        .unwrap_or_else(|_| MusicalPosition::start()),
                        self.sections.len(),
                    ),
                    written_token.to_string(),
                );

                // A bar that opens on a rest still owns its `|:`; leaving it
                // pending hands it to the next chord, bars later.
                if st.pending_start_repeat && st.current_measure.chords.is_empty() {
                    st.current_measure.start_repeat = RepeatMark::Forward;
                    st.pending_start_repeat = false;
                }
                st.current_measure
                    .rhythm_elements
                    .push(RhythmElement::Rest(rest_instance));
                st.current_measure_beats += chord_beats;

                // Check if measure is complete
                if (st.current_measure_beats - st.beats_per_measure).abs() < 0.001 {
                    st.measures.push(st.current_measure.clone());
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                }
                return true;
            }
        }
        false
    }

    /// A standalone space (`s1`, `s4`, …) as a rhythm element. Returns `false`
    /// when the token is not a space.
    pub(super) fn place_space(
        &self,
        st: &mut LineState,
        rest_token: &str,
        written_token: &str,
    ) -> bool {
        // Try to parse as a space
        let mut lexer = Lexer::new(rest_token.to_string());
        let tokens = lexer.tokenize();
        if let Ok((rhythm, _)) = ChordRhythm::parse(&tokens) {
            if rhythm.is_space() {
                let duration = rhythm.to_duration(st.time_sig);
                let chord_beats = duration.to_beats(st.time_sig);

                // Check if we need to start a new measure
                if !st.just_processed_separator
                    && st.current_measure_beats + chord_beats > st.beats_per_measure + 0.001
                {
                    if !st.current_measure.chords.is_empty()
                        || !st.current_measure.rhythm_elements.is_empty()
                    {
                        st.measures.push(st.current_measure.clone());
                    }
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                }
                st.just_processed_separator = false;
                st.measure_was_created_by_separator = false;

                // Create a SpaceInstance (not a chord - just a placeholder for auto-fill)
                let space_instance = SpaceInstance::new(
                    rhythm,
                    duration,
                    AbsolutePosition::new(
                        MusicalPosition::try_new(
                            st.measures.len() as i32,
                            st.current_measure_beats as i32,
                            0,
                        )
                        .unwrap_or_else(|_| MusicalPosition::start()),
                        self.sections.len(),
                    ),
                    written_token.to_string(),
                );

                // A bar that opens on a space still owns its `|:`.
                if st.pending_start_repeat && st.current_measure.chords.is_empty() {
                    st.current_measure.start_repeat = RepeatMark::Forward;
                    st.pending_start_repeat = false;
                }
                // Add to rhythm_elements only (not chords - space is not a chord)
                st.current_measure
                    .rhythm_elements
                    .push(RhythmElement::Space(space_instance));
                st.current_measure_beats += chord_beats;

                // Check if measure is complete
                if (st.current_measure_beats - st.beats_per_measure).abs() < 0.001 {
                    st.measures.push(st.current_measure.clone());
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                }
                return true;
            }
        }
        false
    }

    /// `.` / `.N`: repeat the last chord, as long as it lasted, `dot_repeats`
    /// times.
    pub(super) fn place_dot_repeats(
        &self,
        st: &mut LineState,
        dot_repeats: usize,
        last_chord: &Option<ChordInstance>,
    ) {
        // Take the chord as it stands *now*, not as it was when its
        // token was read. A slash run is a token of its own, so in
        // `G // .` the `//` lands on `G` after `last_chord` was
        // snapshotted — and a `.` reading the snapshot inherits a
        // whole bar instead of the two beats `G` actually got, which
        // turns `G // . . .` into four measures instead of two.
        let live_chord = st
            .current_measure
            .chords
            .last()
            .or_else(|| st.measures.last().and_then(|m| m.chords.last()))
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

                let chord_beats = repeat_chord.duration.to_beats(st.time_sig);

                // Handle measure boundaries (same logic as regular chord)
                if !st.just_processed_separator
                    && st.current_measure_beats + chord_beats > st.beats_per_measure + 0.001
                {
                    if !st.current_measure.chords.is_empty()
                        || !st.current_measure.rhythm_elements.is_empty()
                    {
                        st.measures.push(st.current_measure.clone());
                    }
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                }
                st.just_processed_separator = false;
                st.measure_was_created_by_separator = false;

                st.current_measure
                    .rhythm_elements
                    .push(RhythmElement::Chord(repeat_chord.clone()));
                st.current_measure.chords.push(repeat_chord);
                st.current_measure_beats += chord_beats;

                // Auto-advance measure if full
                if !st.just_processed_separator
                    && (st.current_measure_beats - st.beats_per_measure).abs() < 0.001
                {
                    st.measures.push(st.current_measure.clone());
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                }
            }
        }
    }

    /// Before a chord goes in: start a new bar if it would overflow this one,
    /// or shorten it to fill the bar when slashes are coming for it. Returns
    /// the beats the chord takes.
    pub(super) fn make_room_for_chord(
        st: &mut LineState,
        chord: &mut ChordInstance,
        tokens_str: &[&str],
        token_idx: usize,
    ) -> f64 {
        let mut chord_beats = chord.duration.to_beats(st.time_sig);

        // If we just processed a separator, we're already in a new measure
        // Don't create another one automatically
        if !st.just_processed_separator {
            // Check if adding this chord would exceed the measure
            if st.current_measure_beats + chord_beats > st.beats_per_measure + 0.001 {
                // Look ahead to see if slashes are coming for this chord
                // If so, fill remaining beats instead of creating a new measure
                let slashes_coming = tokens_str[token_idx + 1..]
                    .iter()
                    .take_while(|t| {
                        // Stop at measure separator or new chord
                        **t != "|" && !t.chars().next().is_none_or(|c| c.is_alphabetic())
                    })
                    .any(|t| t.chars().all(|c| c == '/') && !t.is_empty());

                // If the measure has slash rhythm and the chord has default rhythm,
                // OR if slashes are coming for this chord,
                // adjust to fill remaining beats instead of starting a new measure.
                // e.g., "Cm/Eb / Eb ///" - Cm/Eb takes 1 beat (from /), Eb will take
                // remaining 3 beats when its /// is processed
                let remaining_beats = st.beats_per_measure - st.current_measure_beats;
                if (st.measure_has_slash_rhythm || slashes_coming)
                    && chord.rhythm == ChordRhythm::Default
                    && remaining_beats > 0.001
                {
                    // Adjust chord to fill remaining beats (duration only)
                    // IMPORTANT: Keep rhythm as Default so actual slashes can override.
                    // If we set rhythm to Slashes here, subsequent slashes would think
                    // it's explicitly set and create spaces instead of setting duration.
                    chord.duration = MusicalDuration::from_beats(remaining_beats, st.time_sig);
                    chord_beats = remaining_beats;
                } else {
                    // small epsilon for float comparison
                    // Current measure is full, start a new one
                    if !st.current_measure.chords.is_empty()
                        || !st.current_measure.rhythm_elements.is_empty()
                    {
                        st.measures.push(st.current_measure.clone());
                    }
                    st.current_measure = Measure::new();
                    st.current_measure.time_signature =
                        (st.time_sig.numerator as u8, st.time_sig.denominator as u8);
                    st.current_measure_beats = 0.0;
                    st.measure_has_slash_rhythm = false; // Reset for new measure
                    let _measure_was_created_by_separator = false; // Auto-created, not by separator
                }
            }
        }
        st.just_processed_separator = false; // Reset flag after processing chord
        st.measure_was_created_by_separator = false; // Reset flag - measure now has content
        chord_beats
    }
}
