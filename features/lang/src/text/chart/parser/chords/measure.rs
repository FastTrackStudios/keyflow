//! Measure bookkeeping shared by the line parsers: fresh measures, beat
//! sums, separators, simile (`%`) and dot (`.`) repeats, voltas and the
//! `\duration` / `\ChordLength` value.

use super::groups::parse_lily_duration_beats;
use crate::text::chart::parser::ChartParser;
use crate::text::chart::types::{Measure, RhythmElement};
use crate::text::chord::{ChordRhythm, NotationSystem};
use crate::text::time::{MusicalDuration, MusicalPositionExt, TimeSignature};
use keyflow_proto::chart::{RepeatMark, Volta};

impl<'a> ChartParser<'a> {
    pub(super) fn parse_chord_length_value(
        token: &str,
        time_sig: TimeSignature,
    ) -> Option<(ChordRhythm, MusicalDuration)> {
        if let Some((beats, dotted)) = parse_lily_duration_beats(token, time_sig) {
            return Some((
                ChordRhythm::Slashes {
                    count: beats.round().max(1.0) as u8,
                    dotted,
                    tied: false,
                },
                MusicalDuration::from_beats(beats, time_sig),
            ));
        }

        let dotted = token.ends_with('.');
        let slashes = token.trim_end_matches('.');
        if !slashes.is_empty() && slashes.chars().all(|c| c == '/') {
            let count = slashes.len() as u8;
            let beats = if dotted {
                f64::from(count) * dotted_slash_beats(time_sig)
            } else {
                f64::from(count)
            };
            return Some((
                ChordRhythm::Slashes {
                    count,
                    dotted,
                    tied: false,
                },
                MusicalDuration::from_beats(beats, time_sig),
            ));
        }

        None
    }

    pub(super) fn token_has_explicit_chord_length(token: &str) -> bool {
        token.contains('_') || token.contains("//") || token.ends_with("/.") || token.ends_with('/')
    }

    /// `[1]`, `[1, 2]`, and with a length in bars after a colon, `[1:2]`
    /// (a first ending two bars long).
    pub(super) fn parse_volta_token(token: &str) -> Option<Volta> {
        let token = token.split_once('_').map_or(token, |(base, _)| base);
        let inner = token.strip_prefix('[')?.strip_suffix(']')?;
        let (inner, length) = match inner.split_once(':') {
            Some((nums, len)) => (nums, len.trim().parse::<u16>().ok().filter(|l| *l >= 1)?),
            None => (inner, 1),
        };
        let numbers = inner
            .split(',')
            .filter_map(|part| part.trim().parse::<u8>().ok())
            .collect::<Vec<_>>();
        if numbers.is_empty() {
            return None;
        }

        Some(Volta {
            label: format!(
                "{}.",
                numbers
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            numbers,
            length_measures: length,
        })
    }

    pub(super) fn finalize_measure_for_separator(
        measures: &mut Vec<Measure>,
        current_measure: &Measure,
        measure_was_created_by_separator: bool,
    ) {
        if !current_measure.chords.is_empty()
            || !current_measure.rhythm_elements.is_empty()
            || !current_measure.figured_bass.is_empty()
            || !current_measure.staff_text.is_empty()
            || !current_measure.text_cues.is_empty()
            || !current_measure.melodies.is_empty()
            || current_measure.volta_start.is_some()
            || !matches!(current_measure.start_repeat, RepeatMark::None)
            || !matches!(current_measure.end_repeat, RepeatMark::None)
            || measure_was_created_by_separator
        {
            measures.push(current_measure.clone());
        }
    }

    /// Whether a token on a chord line is music — a chord, rest, slash or
    /// simile — rather than text, a barline, a repeat count or a command.
    pub(super) fn is_music_token(token: &str) -> bool {
        let quoted = token.ends_with('"');
        let barline = token.starts_with('|') || token.starts_with(':');
        let command = token.starts_with('\\');
        let repeat_count = token
            .strip_prefix('x')
            .is_some_and(|n| n.is_empty() || n == "^" || n.chars().all(|c| c.is_ascii_digit()));
        !(quoted || barline || command || repeat_count || token.is_empty())
    }

    /// Whether a token is a chord (not a slash, rest, space, simile, text,
    /// barline, repeat count or command).
    pub(super) fn is_chord_token(token: &str) -> bool {
        Self::is_music_token(token)
            && !token.chars().all(|c| matches!(c, '/' | '.' | '%'))
            && !token.starts_with('%')
            && !token.starts_with(['r', 's'])
    }

    /// Where, in beats from the bar start, the bar's last chord begins: the
    /// bar's written length less that chord's own.
    pub(super) fn last_chord_onset(measure: &Measure, time_sig: TimeSignature) -> Option<f64> {
        let last = measure.chords.last()?;
        let chords: f64 = measure
            .chords
            .iter()
            .map(|c| c.duration.to_beats(time_sig))
            .sum();
        let rests: f64 = measure
            .rhythm_elements
            .iter()
            .map(|e| match e {
                RhythmElement::Rest(r) => r.duration.to_beats(time_sig),
                RhythmElement::Space(s) => s.duration.to_beats(time_sig),
                RhythmElement::Chord(_) => 0.0,
            })
            .sum();
        Some((chords + rests - last.duration.to_beats(time_sig)).max(0.0))
    }

    pub(super) fn fresh_measure(time_sig: TimeSignature) -> Measure {
        let mut measure = Measure::new();
        measure.time_signature = (time_sig.numerator as u8, time_sig.denominator as u8);
        measure
    }

    /// Beats a bar holds so far: its chords, and its rests and spaces —
    /// `r2 4 //` is a full bar.
    pub(super) fn measure_beats(measure: &Measure, time_sig: TimeSignature) -> f64 {
        let chords = measure.chords.iter().map(|c| c.duration);
        let others = measure.rhythm_elements.iter().filter_map(|e| match e {
            RhythmElement::Rest(rest) => Some(rest.duration),
            RhythmElement::Space(space) => Some(space.duration),
            _ => None,
        });
        chords.chain(others).map(|d| d.to_beats(time_sig)).sum()
    }

    /// `%` → 1, `%3` → 3. Anything else is not a simile mark.
    ///
    /// `%` on its own is `%1`: one more measure of what the last one was.
    /// `%0` is not a count, it is a typo — treated as no match so it surfaces
    /// as an unknown token rather than silently doing nothing.
    pub(in crate::text::chart::parser) fn parse_measure_repeat(token: &str) -> Option<usize> {
        let digits = token.strip_prefix('%')?;
        if digits.is_empty() {
            return Some(1);
        }
        digits.parse::<usize>().ok().filter(|n| *n > 0)
    }

    /// `.` → 1, `.3` → 3, `._4` → 1.
    ///
    /// `._N` is not a count: the auto-duration pass appends `_N` to bare
    /// tokens, so a `.` that has been through it arrives as `._4`. The
    /// duration is already carried by the chord being repeated.
    ///
    /// `.` is also the staccato prefix (`.C`), and in a chart written in
    /// scale degrees `.3` is staccato on the ♭III — a real chord — as much as
    /// it is three repeats. The notation system decides, the same way it
    /// decides that `b3` is a flat degree rather than the note B: in a degree
    /// chart the counted form is off, and repeats are written `. . .`.
    pub(in crate::text::chart::parser) fn parse_dot_repeat(
        token: &str,
        system: NotationSystem,
    ) -> Option<usize> {
        let rest = token.strip_prefix('.')?;
        if rest.is_empty() || rest.starts_with('_') {
            return Some(1);
        }
        if system == NotationSystem::Degree {
            return None;
        }
        rest.parse::<usize>().ok().filter(|n| *n > 0)
    }

    /// The measure a simile mark stands for: the same music, none of the
    /// annotation.
    ///
    /// A simile mark says "play that again", not "print that again". Cloning
    /// the previous measure whole would redraw its cues, its ending bracket
    /// and its repeat dots once per `%` — and `%3` would stack three copies of
    /// a cue that was written once.
    pub(super) fn simile_copy(measure: &Measure) -> Measure {
        let mut copy = measure.clone();
        copy.simile = true;
        copy.staff_text.clear();
        copy.text_cues.clear();
        copy.dynamics.clear();
        copy.classical_dynamics.clear();
        copy.hairpins.clear();
        copy.volta_start = None;
        copy.start_repeat = RepeatMark::None;
        copy.end_repeat = RepeatMark::None;
        copy.repeat_count = 1;
        copy.source_span = None;
        copy
    }
}

pub(super) fn dotted_slash_beats(time_sig: TimeSignature) -> f64 {
    1.5 * (time_sig.denominator as f64 / 4.0)
}
