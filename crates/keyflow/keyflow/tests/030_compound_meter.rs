//! Compound meters engrave in the beat a musician counts.
//!
//! In 6/8 you count **two**, not six. A bar of one chord is two
//! dotted-quarter slashes (`/. /.`), not six eighth slashes — and that is
//! not a spacing preference. A six-slash 6/8 bar tells the player to feel
//! the bar in six, which is a different piece of music.
//!
//! The engraver got this wrong in three separate places, all of which
//! assumed the meter's *numerator* was the beat count and the *quarter*
//! was the beat:
//!
//! 1. the default fill for a bare chord (`extract_from_slash`),
//! 2. the pad that tops a measure up to full (`fill_to_measure`),
//! 3. the expansion of sustained notes into slashes (`expand_to_quarters`).
//!
//! A fourth was upstream of all of them: a section-length pad (`In 5`
//! given four chords) is emitted as a *chord* carrying a space duration,
//! and every `Explicit` rhythm counted as explicit — so the pad bar was
//! read as a whole note and rendered four plain slashes, in a bar that
//! means "carry on".

use keyflow::engraver::layout::chart::rhythm_builder::{beats_per_measure, is_compound_meter};
use keyflow::engraver::notation::Duration;

/// The chart that found all four. Kept verbatim; also shipped as
/// `keyflow_ui::examples::EXAMPLE_IN_MY_ROOM`.
const IN_MY_ROOM: &str = include_str!("data/in_my_room.kf");

#[test]
fn six_eight_is_compound_but_three_eight_is_not() {
    // 3/8 is simple triple — three eighth beats, one per eighth. Only a
    // multiple of three GREATER than three is compound, which is why the
    // rule is not a bare `% 3 == 0`.
    assert!(is_compound_meter((6, 8)));
    assert!(is_compound_meter((9, 8)));
    assert!(is_compound_meter((12, 8)));
    assert!(!is_compound_meter((3, 8)));
    assert!(!is_compound_meter((4, 4)));
    assert!(!is_compound_meter((7, 8)));
}

#[test]
fn quarter_denominated_meters_are_left_alone() {
    // 6/4 is genuinely ambiguous — compound duple in some music, simple
    // sextuple in other. Guessing would silently re-bar existing charts,
    // so it stays simple until someone asks otherwise.
    assert!(!is_compound_meter((6, 4)));
    assert_eq!(beats_per_measure((6, 4)), (6, Duration::Quarter));
}

#[test]
fn a_compound_bar_is_counted_in_dotted_beats() {
    assert_eq!(beats_per_measure((6, 8)), (2, Duration::DottedQuarter));
    assert_eq!(beats_per_measure((9, 8)), (3, Duration::DottedQuarter));
    assert_eq!(beats_per_measure((12, 8)), (4, Duration::DottedQuarter));
    assert_eq!(beats_per_measure((6, 16)), (2, Duration::DottedEighth));
}

#[test]
fn a_simple_bar_is_counted_in_plain_beats() {
    assert_eq!(beats_per_measure((4, 4)), (4, Duration::Quarter));
    assert_eq!(beats_per_measure((3, 4)), (3, Duration::Quarter));
    assert_eq!(beats_per_measure((3, 8)), (3, Duration::Quarter));
}

#[test]
fn the_beats_of_a_compound_bar_add_up_to_the_bar() {
    // The arithmetic that makes this safe rather than merely prettier: if
    // the beat count times the beat duration did not equal the measure,
    // every bar would be over- or under-full.
    for meter in [(6, 8), (9, 8), (12, 8), (4, 4), (3, 4), (2, 4)] {
        let (count, beat) = beats_per_measure(meter);
        let measure_ticks =
            keyflow::engraver::layout::chart::rhythm_builder::calculate_measure_ticks(meter);
        assert_eq!(
            i32::try_from(count).unwrap() * beat.ticks(),
            measure_ticks,
            "{meter:?}: {count} x {beat:?} does not fill the bar"
        );
    }
}

#[test]
fn in_my_room_parses() {
    let chart = keyflow::parse(IN_MY_ROOM).expect("In My Room should parse");
    assert_eq!(chart.metadata.title.as_deref(), Some("In My Room"));
    assert_eq!(chart.metadata.artist.as_deref(), Some("The Beach Boys"));
    let time_sig = chart.time_signature.as_ref().expect("6/8 header");
    assert_eq!((time_sig.numerator, time_sig.denominator), (6, 8));
}

#[test]
fn every_bar_of_in_my_room_engraves_as_two_dotted_beats() {
    // The end-to-end assertion, and the one that would have caught the
    // original bug: EVERY measure — including the `In 5` pad bar, which
    // took a different path and rendered four plain slashes long after
    // the others were fixed.
    let chart = keyflow::parse(IN_MY_ROOM).expect("parse");

    let mut checked = 0usize;
    for section in &chart.sections {
        // The count-in has its own renderer and its own conventions.
        if format!("{:?}", section.section.section_type).contains("CountIn") {
            continue;
        }
        for (idx, measure) in section.measures().iter().enumerate() {
            let rhythm = keyflow::engraver::layout::chart::rhythm_builder::build_rhythm(
                keyflow::engraver::layout::chart::rhythm_builder::RhythmSource::SlashNotation {
                    chords: &measure.chords,
                    spillbacks: None,
                },
                &keyflow::engraver::layout::chart::rhythm_builder::RhythmBuildConfig {
                    time_signature: (6, 8),
                    ..Default::default()
                },
            );

            let ticks: i32 = rhythm.entries.iter().map(|e| e.duration().ticks()).sum();
            assert_eq!(
                ticks, 1440,
                "{:?} measure {idx} does not fill a 6/8 bar",
                section.section.section_type
            );
            assert!(
                rhythm
                    .entries
                    .iter()
                    .all(|e| e.duration() == Duration::DottedQuarter),
                "{:?} measure {idx} engraved {:?} — a 6/8 bar reads in two dotted beats",
                section.section.section_type,
                rhythm.entries
            );
            checked += 1;
        }
    }
    assert!(checked > 30, "expected the whole chart, checked {checked}");
}
