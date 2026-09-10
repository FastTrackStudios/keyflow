//! Folding a chart down to what it actually says.
//!
//! Charts repeat. A vamp written out eight times is eight bars of ink saying
//! one thing, and the change when it finally comes is buried in the middle of
//! it. A player reading that chart is doing the folding in their head anyway.
//!
//! [`fold_similes`] does it on the page instead: a bar that repeats the one
//! before it becomes a simile mark. Nothing is removed — every measure keeps
//! its chords, so playback, transposition, the cursor and analysis all see the
//! chart they saw before. Only the drawing changes, which is why this is safe
//! to run on a chart the user did not write that way.

use super::Chart;
use super::types::{ChordInstance, Measure};

/// Mark every bar that merely repeats the one before it as a simile.
///
/// A bar folds only when it says nothing the bar before it did not: the same
/// chords with the same rhythm, in the same meter, and no annotation of its
/// own. Anything written *on* a bar — a cue, a dynamic, an ending bracket, a
/// repeat sign, a melody — keeps it visible, because that is the thing the
/// reader would lose. The first bar of a section never folds; a section has to
/// open by saying what it is.
///
/// Idempotent, and folding across a section boundary never happens: a section
/// that starts on the same chord the last one ended on still shows it.
pub fn fold_similes(chart: &mut Chart) {
    for section in &mut chart.sections {
        for track in &mut section.tracks {
            fold_measures(&mut track.measures);
        }
    }
}

/// Collapse every section that is note-for-note an earlier one.
///
/// A chorus written out for the third time is eight bars of ink saying
/// "chorus". [`ChartSection::folded`] asks the engraver to draw a titled rule
/// instead — one row, and the reader knows exactly where they are.
///
/// A section folds only against an earlier section of the *same kind*, and
/// only when every bar matches: same chords, same rhythm, same meter, same
/// repeat structure, and no annotation of its own. A chorus with a cue on it
/// that the first chorus did not have is a different chorus, and gets written.
///
/// The first occurrence never folds — something has to say what the chorus is.
///
/// [`ChartSection::folded`]: super::types::ChartSection::folded
pub fn fold_sections(chart: &mut Chart) {
    for idx in 1..chart.sections.len() {
        let (earlier, rest) = chart.sections.split_at_mut(idx);
        let current = &mut rest[0];
        if current.measures().is_empty() {
            continue;
        }
        let matched = earlier.iter().rev().any(|previous| {
            !previous.folded
                && previous.section.section_type == current.section.section_type
                && section_repeats(previous.measures(), current.measures())
        });
        if matched {
            current.folded = true;
        }
    }
}

/// Undo [`fold_sections`] — every section draws its bars again.
pub fn unfold_sections(chart: &mut Chart) {
    for section in &mut chart.sections {
        section.folded = false;
    }
}

/// Whether `current` is bar-for-bar what `previous` was.
fn section_repeats(previous: &[Measure], current: &[Measure]) -> bool {
    previous.len() == current.len()
        && !previous.is_empty()
        && previous
            .iter()
            .zip(current)
            .all(|(before, after)| same_bar(before, after))
}

/// Like [`repeats`], but structure has to match rather than be absent: two
/// choruses that both carry the same repeat sign are still the same chorus.
fn same_bar(previous: &Measure, current: &Measure) -> bool {
    current.time_signature == previous.time_signature
        && current.repeat_count == previous.repeat_count
        && current.start_repeat == previous.start_repeat
        && current.end_repeat == previous.end_repeat
        && current.end_barline == previous.end_barline
        && current.volta_start == previous.volta_start
        && !carries_annotation(current)
        && !carries_annotation(previous)
        && current.chords.len() == previous.chords.len()
        && current
            .chords
            .iter()
            .zip(&previous.chords)
            .all(|(a, b)| same_chord(a, b))
}

/// Write every repeat out in full, and take the signs away.
///
/// The inverse of folding, for the reading where nothing is implied: a
/// `|: … :|` span becomes the bars it stands for, twice, with the marks
/// cleared. A player proof-reading a chart wants every bar in front of them
/// rather than a instruction to go back.
///
/// A span carrying a volta is left exactly as it is, signs and all. First and
/// second endings do not expand by duplication — the second pass skips the
/// first ending and plays the second — and a wrong expansion is worse than an
/// unexpanded one. Those spans keep their repeat signs, because that is the
/// only honest way left to draw them.
pub fn expand_repeats(chart: &mut Chart) {
    for section in &mut chart.sections {
        for track in &mut section.tracks {
            track.measures = expanded(&track.measures);
        }
    }
}

fn expanded(measures: &[Measure]) -> Vec<Measure> {
    use super::notations::RepeatMark;

    let mut out: Vec<Measure> = Vec::with_capacity(measures.len());
    let mut span_start = 0usize;
    let mut idx = 0usize;

    while idx < measures.len() {
        let measure = &measures[idx];
        if matches!(measure.start_repeat, RepeatMark::Forward) {
            span_start = idx;
        }

        if matches!(measure.end_repeat, RepeatMark::Backward) {
            let span = &measures[span_start..=idx];
            // An ending bracket means the two passes differ, and duplicating
            // the span would play the first ending twice.
            if span.iter().any(|m| m.volta_start.is_some()) {
                out.extend(measures[out.len().max(span_start)..=idx].iter().cloned());
                idx += 1;
                span_start = idx;
                continue;
            }

            // The bars before the span are already in `out`; the span itself
            // goes in twice, clean.
            out.truncate(span_start.min(out.len()));
            let passes = measure.repeat_count.max(2);
            for _ in 0..passes {
                out.extend(span.iter().map(clear_repeat_marks));
            }
            idx += 1;
            span_start = idx;
            continue;
        }

        out.push(measure.clone());
        idx += 1;
    }
    out
}

fn clear_repeat_marks(measure: &Measure) -> Measure {
    use super::notations::RepeatMark;

    let mut copy = measure.clone();
    copy.start_repeat = RepeatMark::None;
    copy.end_repeat = RepeatMark::None;
    copy.repeat_count = 1;
    copy
}

/// Undo [`fold_similes`] — every bar draws its own chords again.
///
/// The chords never went anywhere, so this is just clearing the flag. Useful
/// for showing a folded chart against the one it folded.
pub fn unfold_similes(chart: &mut Chart) {
    for section in &mut chart.sections {
        for track in &mut section.tracks {
            for measure in &mut track.measures {
                measure.simile = false;
            }
        }
    }
}

fn fold_measures(measures: &mut [Measure]) {
    for idx in 1..measures.len() {
        let (before, after) = measures.split_at_mut(idx);
        let previous = &before[idx - 1];
        let current = &mut after[0];
        if repeats(previous, current) {
            current.simile = true;
        }
    }
}

/// Whether `current` says exactly what `previous` said, and nothing else.
fn repeats(previous: &Measure, current: &Measure) -> bool {
    if current.time_signature != previous.time_signature || current.repeat_count != 1 {
        return false;
    }

    // Anything written on this bar is a reason to keep it drawn — folding it
    // away would take the annotation with it.
    if carries_annotation(current) || current.volta_start.is_some() {
        return false;
    }

    // A repeat sign or a section-closing barline is structure, not decoration:
    // a simile mark drawn over one hides where the music goes next.
    use super::notations::{BarlineStyle, RepeatMark};
    if !matches!(current.start_repeat, RepeatMark::None)
        || !matches!(current.end_repeat, RepeatMark::None)
        || !matches!(previous.end_barline, BarlineStyle::Normal)
    {
        return false;
    }

    // A bar with nothing in it is not a repeat of anything.
    if current.chords.is_empty() || previous.chords.is_empty() {
        return false;
    }

    current.chords.len() == previous.chords.len()
        && current
            .chords
            .iter()
            .zip(&previous.chords)
            .all(|(a, b)| same_chord(a, b))
}

/// Whether anything is written *on* this bar beyond its chords.
fn carries_annotation(measure: &Measure) -> bool {
    !measure.staff_text.is_empty()
        || !measure.text_cues.is_empty()
        || !measure.dynamics.is_empty()
        || !measure.classical_dynamics.is_empty()
        || !measure.hairpins.is_empty()
        || !measure.figured_bass.is_empty()
        || !measure.suspensions.is_empty()
        || !measure.melodies.is_empty()
}

/// Two chords are the same for folding when they read the same and last the
/// same. Positions and source spans differ by construction and say nothing
/// about what is played.
fn same_chord(a: &ChordInstance, b: &ChordInstance) -> bool {
    a.full_symbol == b.full_symbol
        && a.rhythm == b.rhythm
        && a.duration == b.duration
        && a.push_pull == b.push_pull
        && a.commands == b.commands
        && a.display_override == b.display_override
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::types::ChartSection;
    use crate::chord::{Chord, ChordRhythm};
    use crate::parsing::Lexer;
    use crate::primitives::RootNotation;
    use crate::sections::{Section, SectionType};
    use crate::time::{AbsolutePosition, MusicalDuration};

    fn measure(symbols: &[&str]) -> Measure {
        let mut m = Measure::new();
        for symbol in symbols {
            let mut lexer = Lexer::new((*symbol).to_string());
            let parsed = Chord::parse(&lexer.tokenize()).expect("chord");
            m.chords.push(ChordInstance::new(
                RootNotation::from_string(&symbol[..1]).expect("root"),
                (*symbol).to_string(),
                parsed,
                ChordRhythm::Default,
                (*symbol).to_string(),
                MusicalDuration::new(0, 4, 0),
                AbsolutePosition::at_beginning(),
            ));
        }
        m
    }

    fn chart_of(sections: Vec<Vec<Measure>>) -> Chart {
        let mut chart = Chart::new();
        for measures in sections {
            chart
                .sections
                .push(ChartSection::new(Section::new(SectionType::Verse)).with_measures(measures));
        }
        chart
    }

    fn flags(chart: &Chart) -> Vec<Vec<bool>> {
        chart
            .sections
            .iter()
            .map(|s| s.measures().iter().map(|m| m.simile).collect())
            .collect()
    }

    #[test]
    fn a_run_of_identical_bars_folds_after_the_first() {
        let mut chart = chart_of(vec![vec![
            measure(&["G"]),
            measure(&["G"]),
            measure(&["G"]),
            measure(&["C"]),
        ]]);
        fold_similes(&mut chart);
        assert_eq!(flags(&chart), vec![vec![false, true, true, false]]);
    }

    #[test]
    fn a_split_bar_folds_only_against_the_same_split() {
        let mut chart = chart_of(vec![vec![
            measure(&["G", "C"]),
            measure(&["G", "C"]),
            measure(&["G", "D"]),
        ]]);
        fold_similes(&mut chart);
        assert_eq!(flags(&chart), vec![vec![false, true, false]]);
    }

    /// A section has to open by saying what it is, even when it opens on the
    /// chord the last one ended on.
    #[test]
    fn folding_never_crosses_a_section_boundary() {
        let mut chart = chart_of(vec![
            vec![measure(&["G"]), measure(&["G"])],
            vec![measure(&["G"]), measure(&["G"])],
        ]);
        fold_similes(&mut chart);
        assert_eq!(
            flags(&chart),
            vec![vec![false, true], vec![false, true]],
            "each section's first bar stays drawn"
        );
    }

    /// Anything written on a bar keeps it visible — folding it away would take
    /// the annotation with it.
    #[test]
    fn a_bar_that_carries_something_of_its_own_stays_drawn() {
        use crate::chart::notations::{Placement, StaffText};

        let mut annotated = measure(&["G"]);
        annotated.staff_text.push(StaffText {
            text: "Build".to_string(),
            beat: 1,
            placement: Placement::Above,
            source_default_x: None,
            boxed: false,
            bold: false,
            italic: false,
        });
        let mut chart = chart_of(vec![vec![measure(&["G"]), annotated, measure(&["G"])]]);
        fold_similes(&mut chart);
        assert_eq!(flags(&chart), vec![vec![false, false, true]]);
    }

    /// A repeat sign says where the music goes next; a simile drawn over one
    /// hides it.
    #[test]
    fn a_repeat_sign_is_structure_and_is_not_folded_away() {
        use crate::chart::notations::RepeatMark;

        let mut repeated = measure(&["G"]);
        repeated.end_repeat = RepeatMark::Backward;
        let mut chart = chart_of(vec![vec![measure(&["G"]), repeated, measure(&["G"])]]);
        fold_similes(&mut chart);
        assert_eq!(flags(&chart), vec![vec![false, false, true]]);
    }

    fn section_flags(chart: &Chart) -> Vec<bool> {
        chart.sections.iter().map(|s| s.folded).collect()
    }

    fn typed(section_type: SectionType, measures: Vec<Measure>) -> ChartSection {
        ChartSection::new(Section::new(section_type)).with_measures(measures)
    }

    /// A chorus written out for the third time is eight bars saying "chorus".
    #[test]
    fn a_section_that_repeats_an_earlier_one_folds() {
        let chorus = || vec![measure(&["C"]), measure(&["G"])];
        let mut chart = Chart::new();
        chart.sections.push(typed(SectionType::Chorus, chorus()));
        chart.sections.push(typed(
            SectionType::Verse,
            vec![measure(&["A"]), measure(&["F"])],
        ));
        chart.sections.push(typed(SectionType::Chorus, chorus()));
        fold_sections(&mut chart);
        assert_eq!(section_flags(&chart), vec![false, false, true]);
    }

    /// Only against its own kind, and only when every bar matches.
    #[test]
    fn a_section_does_not_fold_against_a_different_kind_or_a_different_shape() {
        let bars = || vec![measure(&["C"]), measure(&["G"])];
        let mut chart = Chart::new();
        chart.sections.push(typed(SectionType::Chorus, bars()));
        // Same music, different kind.
        chart.sections.push(typed(SectionType::Verse, bars()));
        // Same kind, one chord different.
        chart.sections.push(typed(
            SectionType::Chorus,
            vec![measure(&["C"]), measure(&["D"])],
        ));
        fold_sections(&mut chart);
        assert_eq!(section_flags(&chart), vec![false, false, false]);
    }

    /// A chorus that carries a cue the first one did not is a different
    /// chorus, and gets written out.
    #[test]
    fn a_section_carrying_its_own_annotation_stays_written() {
        use crate::chart::notations::{Placement, StaffText};

        let mut annotated = measure(&["G"]);
        annotated.staff_text.push(StaffText {
            text: "Down".to_string(),
            beat: 1,
            placement: Placement::Above,
            source_default_x: None,
            boxed: false,
            bold: false,
            italic: false,
        });
        let mut chart = Chart::new();
        chart.sections.push(typed(
            SectionType::Chorus,
            vec![measure(&["C"]), measure(&["G"])],
        ));
        chart
            .sections
            .push(typed(SectionType::Chorus, vec![measure(&["C"]), annotated]));
        fold_sections(&mut chart);
        assert_eq!(section_flags(&chart), vec![false, false]);
    }

    /// A folded section is never the thing a later one folds against — the
    /// reader has to be able to find the bars somewhere.
    #[test]
    fn folding_always_points_back_to_a_written_section() {
        let bars = || vec![measure(&["C"]), measure(&["G"])];
        let mut chart = Chart::new();
        for _ in 0..3 {
            chart.sections.push(typed(SectionType::Chorus, bars()));
        }
        fold_sections(&mut chart);
        assert_eq!(section_flags(&chart), vec![false, true, true]);

        unfold_sections(&mut chart);
        assert_eq!(section_flags(&chart), vec![false, false, false]);
    }

    // region: --- Expanding

    fn symbols(measures: &[Measure]) -> Vec<String> {
        measures
            .iter()
            .map(|m| m.chords[0].full_symbol.clone())
            .collect()
    }

    #[test]
    fn a_repeat_span_expands_to_the_bars_it_stands_for() {
        use crate::chart::notations::RepeatMark;

        let mut a = measure(&["A"]);
        a.start_repeat = RepeatMark::Forward;
        let mut b = measure(&["B"]);
        b.end_repeat = RepeatMark::Backward;
        let mut chart = chart_of(vec![vec![measure(&["G"]), a, b, measure(&["C"])]]);

        expand_repeats(&mut chart);
        let measures = chart.sections[0].measures();
        assert_eq!(symbols(measures), ["G", "A", "B", "A", "B", "C"]);
        assert!(
            measures
                .iter()
                .all(|m| matches!(m.start_repeat, RepeatMark::None)
                    && matches!(m.end_repeat, RepeatMark::None)),
            "the signs go once the bars are written out"
        );
    }

    /// First and second endings do not expand by duplication — the second pass
    /// skips the first ending — so the span keeps its signs rather than being
    /// expanded wrongly.
    #[test]
    fn a_span_with_an_ending_keeps_its_repeat_signs() {
        use crate::chart::notations::{RepeatMark, Volta};

        let mut a = measure(&["A"]);
        a.start_repeat = RepeatMark::Forward;
        let mut b = measure(&["B"]);
        b.end_repeat = RepeatMark::Backward;
        b.volta_start = Some(Volta {
            numbers: vec![1],
            label: String::new(),
            length_measures: 1,
        });
        let mut chart = chart_of(vec![vec![a, b]]);

        expand_repeats(&mut chart);
        let measures = chart.sections[0].measures();
        assert_eq!(symbols(measures), ["A", "B"], "left exactly as written");
        assert!(matches!(measures[0].start_repeat, RepeatMark::Forward));
        assert!(matches!(measures[1].end_repeat, RepeatMark::Backward));
    }

    #[test]
    fn a_chart_with_no_repeats_is_unchanged_by_expanding() {
        let mut chart = chart_of(vec![vec![measure(&["G"]), measure(&["C"])]]);
        let before = symbols(chart.sections[0].measures());
        expand_repeats(&mut chart);
        assert_eq!(symbols(chart.sections[0].measures()), before);
    }

    // endregion: --- Expanding

    #[test]
    fn folding_is_idempotent_and_reversible() {
        let mut chart = chart_of(vec![vec![measure(&["G"]), measure(&["G"])]]);
        fold_similes(&mut chart);
        let once = flags(&chart);
        fold_similes(&mut chart);
        assert_eq!(flags(&chart), once);

        unfold_similes(&mut chart);
        assert_eq!(flags(&chart), vec![vec![false, false]]);
    }
}
