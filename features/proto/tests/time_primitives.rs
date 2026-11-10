//! The time primitives keyflow owns (`TimeSignature`, `Tempo`,
//! `MusicalPosition` and the seconds / quarter-note / PPQ positions and
//! durations). They were transplanted from `daw-proto` without tests.

use keyflow_proto::time::MusicalPositionExt;
use keyflow_proto::time::primitives::{
    Duration, DurationInBeats, DurationInQuarterNotes, MusicalPosition, Position, PositionInBeats,
    PositionInPpq, PositionInQuarterNotes, PositionInSeconds, Tempo, TimeRange, TimeSignature,
};

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

#[test]
fn time_signature_measures_in_quarter_notes() {
    assert_eq!(TimeSignature::new(4, 4).quarter_notes_per_measure(), 4.0);
    assert_eq!(TimeSignature::new(3, 4).quarter_notes_per_measure(), 3.0);
    // Compound and cut time: six eighths are three quarters, two halves four.
    assert_eq!(TimeSignature::new(6, 8).quarter_notes_per_measure(), 3.0);
    assert_eq!(TimeSignature::new(12, 8).quarter_notes_per_measure(), 6.0);
    assert_eq!(TimeSignature::CUT_TIME.quarter_notes_per_measure(), 4.0);
    assert_eq!(TimeSignature::new(7, 8).quarter_notes_per_measure(), 3.5);
    assert_eq!(TimeSignature::new(5, 4).beats_per_measure(), 5.0);

    assert_eq!(TimeSignature::default(), TimeSignature::COMMON_TIME);
    assert_eq!(TimeSignature::WALTZ.to_string(), "3/4");
    assert_eq!(TimeSignature::new(6, 8).to_string(), "6/8");
}

#[test]
fn time_signature_rejects_zero() {
    assert!(TimeSignature::try_new(0, 4).is_err());
    assert!(TimeSignature::try_new(4, 0).is_err());
    assert_eq!(TimeSignature::try_new(5, 8), Ok(TimeSignature::new(5, 8)));
    assert!(std::panic::catch_unwind(|| TimeSignature::new(0, 4)).is_err());
}

#[test]
fn tempo_must_be_positive() {
    assert!(Tempo::try_from_bpm(0.0).is_err());
    assert!(Tempo::try_from_bpm(-120.0).is_err());
    assert_eq!(Tempo::try_from_bpm(96.0).map(|t| t.bpm()), Ok(96.0));
    assert_eq!(Tempo::default(), Tempo::ONE_TWENTY);
    assert_eq!(Tempo::from_bpm(127.0).to_string(), "127.00 BPM");
}

/// Bars and beats are zero-based in the value and one-based on screen, the
/// way a DAW's ruler reads.
#[test]
fn musical_position_displays_one_based() {
    assert_eq!(MusicalPosition::ZERO.to_string(), "1.1.000");
    assert_eq!(MusicalPosition::new(3, 2, 500).to_string(), "4.3.500");
    assert!(std::panic::catch_unwind(|| MusicalPosition::new(0, 0, 1000)).is_err());
    assert!(MusicalPosition::try_new(0, 0, 1000).is_err());
    assert!(MusicalPosition::try_new(0, 0, -1).is_err());
}

/// At 120 bpm a beat is half a second: bar 3 beat 2 in 4/4 is
/// 2 * 4 + 1 = 9 beats in, 4.5 s.
#[test]
fn musical_position_to_seconds_in_four_four() {
    let ts = TimeSignature::COMMON_TIME;
    let pos = MusicalPosition::new(2, 1, 0);
    assert!(close(
        pos.to_seconds(Tempo::ONE_TWENTY, ts).as_seconds(),
        4.5
    ));
    // A half-beat subdivision is a quarter of a second at 120.
    let off = MusicalPosition::new(0, 0, 500);
    assert!(close(
        off.to_seconds(Tempo::ONE_TWENTY, ts).as_seconds(),
        0.25
    ));
    // In 3/4 the same bar number lands sooner.
    let waltz = MusicalPosition::new(2, 0, 0).to_seconds(Tempo::ONE_TWENTY, TimeSignature::WALTZ);
    assert!(close(waltz.as_seconds(), 3.0));
}

#[test]
fn seconds_to_musical_and_back() {
    let ts = TimeSignature::COMMON_TIME;
    let tempo = Tempo::from_bpm(90.0);
    for pos in [
        MusicalPosition::new(0, 0, 0),
        MusicalPosition::new(0, 3, 0),
        MusicalPosition::new(7, 2, 250),
        MusicalPosition::new(31, 1, 500),
    ] {
        let secs = pos.to_seconds(tempo, ts);
        assert_eq!(secs.to_musical(tempo, ts), pos, "{pos}");
    }
}

/// In 4/4 a quarter note is a beat, so a musical position and its
/// quarter-note count agree both ways.
#[test]
fn quarter_notes_round_trip_in_four_four() {
    let ts = TimeSignature::COMMON_TIME;
    let pos = MusicalPosition::new(4, 3, 500);
    let qn = pos.to_quarter_notes(ts);
    assert!(close(qn.as_quarter_notes(), 19.5));
    assert_eq!(qn.to_musical(ts), pos);
}

/// In 6/8 a bar is three quarter notes long (six eighths). The quarter-note
/// count of bar 2 (zero-based 1) must therefore be 3, not 6.
#[test]
fn quarter_notes_in_compound_meter() {
    let ts = TimeSignature::new(6, 8);
    assert!(close(
        MusicalPosition::new(1, 0, 0)
            .to_quarter_notes(ts)
            .as_quarter_notes(),
        3.0
    ));
    // Beat 4 of a 6/8 bar is the fourth eighth: one and a half quarters in.
    assert!(close(
        MusicalPosition::new(0, 3, 0)
            .to_quarter_notes(ts)
            .as_quarter_notes(),
        1.5
    ));
    // And back: six quarter notes into a 6/8 piece is the top of bar 3.
    assert_eq!(
        PositionInQuarterNotes::from_quarter_notes(6.0).to_musical(ts),
        MusicalPosition::new(2, 0, 0)
    );
    // 2/2: a bar is four quarter notes, a beat is a half note.
    let cut = TimeSignature::CUT_TIME;
    assert!(close(
        MusicalPosition::new(1, 1, 0)
            .to_quarter_notes(cut)
            .as_quarter_notes(),
        6.0
    ));
}

#[test]
fn ppq_positions_convert_at_the_resolution() {
    let ppq = 960.0;
    let pos = PositionInPpq::from_ppq(1440);
    assert!(close(pos.to_quarter_notes(ppq).as_quarter_notes(), 1.5));
    assert!(close(
        pos.to_seconds(Tempo::ONE_TWENTY, ppq).as_seconds(),
        0.75
    ));
    assert!(PositionInPpq::from_ppq(-1) < PositionInPpq::ZERO);
}

#[test]
fn positions_can_be_negative_for_preroll() {
    let count_in = PositionInSeconds::from_seconds(-2.0);
    assert_eq!(count_in.to_string(), "-0:02.000");
    assert!(close(count_in.abs().as_seconds(), 2.0));
    assert!(Duration::try_from(count_in).is_err());
    assert_eq!(
        PositionInSeconds::from_seconds(75.25).to_string(),
        "1:15.250"
    );
    assert!(close(
        PositionInBeats::from_beats(-4.0)
            .to_seconds(Tempo::ONE_TWENTY)
            .as_seconds(),
        -2.0
    ));
}

#[test]
fn durations_are_never_negative() {
    assert!(Duration::try_from_seconds(-0.1).is_err());
    assert!(std::panic::catch_unwind(|| Duration::from_seconds(-1.0)).is_err());
    assert!(std::panic::catch_unwind(|| DurationInBeats::from_beats(-1.0)).is_err());
    assert!(
        DurationInQuarterNotes::try_from(PositionInQuarterNotes::from_quarter_notes(-1.0)).is_err()
    );

    let three = Duration::from_seconds(3.0);
    let five = Duration::from_seconds(5.0);
    assert_eq!(three.saturating_sub(five), Duration::ZERO);
    assert!(close(five.saturating_sub(three).as_seconds(), 2.0));
    assert_eq!(Duration::from_seconds(61.5).to_string(), "1:01.500");
}

#[test]
fn durations_convert_with_tempo() {
    // Four beats at 120 bpm last two seconds, and back.
    let bar = DurationInBeats::from_beats(4.0);
    assert!(close(bar.to_seconds(Tempo::ONE_TWENTY).as_seconds(), 2.0));
    assert!(close(
        Duration::from_seconds(2.0)
            .to_beats(Tempo::ONE_TWENTY)
            .as_beats(),
        4.0
    ));
    // A dotted quarter at 60 bpm is a second and a half.
    let dotted_quarter = DurationInQuarterNotes::from_quarter_notes(1.5);
    assert!(close(
        dotted_quarter
            .to_seconds(Tempo::from_bpm(60.0))
            .as_seconds(),
        1.5
    ));
    // A quarter-note triplet: three in the space of a half note.
    let triplet = DurationInQuarterNotes::from_quarter_notes(2.0 / 3.0);
    assert!(close(
        triplet.to_seconds(Tempo::ONE_TWENTY).as_seconds() * 3.0,
        1.0
    ));
}

#[test]
fn position_deltas() {
    let a = Position::from_time_and_musical(
        PositionInSeconds::from_seconds(10.0),
        MusicalPosition::new(4, 2, 0),
    );
    let b = Position::from_time_and_musical(
        PositionInSeconds::from_seconds(4.0),
        MusicalPosition::new(1, 3, 500),
    );
    let d = a.delta_from_with_ts(&b, TimeSignature::COMMON_TIME);
    assert_eq!(d.seconds, Some(6.0));
    // (4*4 + 2) - (1*4 + 3.5) = 10.5 quarter notes = 2 bars 2.5 beats.
    assert_eq!(d.quarter_notes, Some(10.5));
    assert_eq!(
        d.musical_string(TimeSignature::COMMON_TIME).as_deref(),
        Some("+2.2.500")
    );
    let back = b.delta_from_with_ts(&a, TimeSignature::COMMON_TIME);
    assert_eq!(
        back.musical_string(TimeSignature::COMMON_TIME).as_deref(),
        Some("-2.2.500")
    );

    // A position without a time component has no seconds, not zero.
    let musical_only = Position::from_musical(MusicalPosition::ZERO);
    assert_eq!(musical_only.seconds(), None);
    assert_eq!(musical_only.delta_from(&a).seconds, None);
}

#[test]
fn time_ranges() {
    let verse = TimeRange::from_seconds(10.0, 40.0);
    let chorus = TimeRange::from_seconds(40.0, 70.0);
    let bridge = TimeRange::from_seconds(80.0, 95.0);
    assert!(close(verse.duration_seconds(), 30.0));
    assert!(verse.contains(10.0) && verse.contains(40.0) && !verse.contains(40.1));
    assert!(
        verse.overlaps(&chorus),
        "touching ranges share their boundary"
    );
    assert!(!verse.overlaps(&bridge));
    // An inverted range has no length rather than a negative one.
    assert_eq!(TimeRange::from_seconds(5.0, 1.0).duration_seconds(), 0.0);
}
