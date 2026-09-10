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
    let carries_its_own = !current.staff_text.is_empty()
        || !current.text_cues.is_empty()
        || !current.dynamics.is_empty()
        || !current.classical_dynamics.is_empty()
        || !current.hairpins.is_empty()
        || !current.figured_bass.is_empty()
        || !current.suspensions.is_empty()
        || !current.melodies.is_empty()
        || current.volta_start.is_some();
    if carries_its_own {
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
