//! An LRC's lyrics placed on a chart's bars and written back as timed
//! `[lyrics lrc]` lanes. The fixture's lyrics are made up — real song
//! lyrics are never committed here.

use keyflow_lang::text::chart::{Chart, TrackType, parse_chart};
use keyflow_lang::text::lyric_lane::{PlaceOptions, Timeline, place_lrc, write_lanes};
use keyflow_lang::text::proto::lrc;

const CHART: &str = include_str!("fixtures/lrc/lanterns.kf");
const LRC: &str = include_str!("fixtures/lrc/lanterns.lrc");

/// Bar 1 of the fixture starts one second into the "recording".
fn convert(source: &str) -> (String, Vec<String>) {
    let chart = parse_chart(source).expect("fixture parses");
    let timeline = Timeline::from_chart(&chart, 1.0);
    let placement = place_lrc(&lrc::parse(LRC), &timeline, &PlaceOptions::default());
    let unplaced = placement.unplaced.iter().map(|u| u.text.clone()).collect();
    (write_lanes(source, &chart, &placement).unwrap(), unplaced)
}

fn lane_text(chart: &Chart, section: usize) -> Option<String> {
    chart.sections[section]
        .tracks
        .iter()
        .find(|t| t.track_type == TrackType::Lyrics)
        .and_then(|t| t.lyrics.as_ref())
        .map(|l| l.to_timed_text())
}

#[test]
fn words_land_in_their_bar_on_the_nearest_eighth() {
    let (out, _) = convert(CHART);
    let chart = parse_chart(&out).unwrap();
    // 120 bpm, 2 s a bar; bar 3 of the verse is 3/4. A plain line's words
    // are spread by syllables over the time to the next line.
    assert_eq!(
        lane_text(&chart, 1).as_deref(),
        Some("@1 Paper @1:3 lanterns @2 drifting\n@3 over @3:3.5 sleepy @4:3 town")
    );
    // Per-word LRC times are used as they are.
    assert_eq!(
        lane_text(&chart, 2).as_deref(),
        Some("@1 Hum @1:3 along @2 tonight")
    );
    assert_eq!(lane_text(&chart, 0), None, "nothing is sung in the intro");
}

#[test]
fn a_recalled_section_keeps_its_chords_under_its_lyrics() {
    let (out, _) = convert(CHART);
    let before = parse_chart(CHART).unwrap();
    let after = parse_chart(&out).unwrap();
    let bars = |c: &Chart| {
        c.sections
            .iter()
            .map(|s| s.measures().len())
            .collect::<Vec<_>>()
    };
    assert_eq!(bars(&before), bars(&after));
    // The last `VS 4` has no body of its own: it is the verse again.
    let chords = |c: &Chart, s: usize| {
        c.sections[s]
            .measures()
            .iter()
            .map(|m| {
                m.chords
                    .iter()
                    .map(|ch| ch.full_symbol.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(chords(&after, 3), chords(&before, 1));
    assert_eq!(
        lane_text(&after, 3).as_deref(),
        Some("@1:3 Second @2 verse @2:2 arrives")
    );
}

#[test]
fn chords_and_other_lines_are_untouched() {
    let (out, _) = convert(CHART);
    let without_lanes: String = {
        let mut keep = Vec::new();
        let mut in_lane = false;
        for line in out.lines() {
            if line.trim() == "[lyrics lrc]" {
                in_lane = true;
                continue;
            }
            if in_lane && line.trim_start().starts_with('@') {
                continue;
            }
            in_lane = false;
            keep.push(line);
        }
        keep.join("\n") + "\n"
    };
    assert_eq!(without_lanes, CHART);
}

#[test]
fn running_it_again_replaces_the_lanes() {
    let (once, _) = convert(CHART);
    let (twice, _) = convert(&once);
    assert_eq!(once, twice);
    assert_eq!(twice.matches("[lyrics lrc]").count(), 3);
}

#[test]
fn lines_outside_the_chart_are_reported_not_dropped() {
    let (_, unplaced) = convert(CHART);
    assert_eq!(unplaced, vec!["Before the band", "After the end"]);
}

#[test]
fn a_measured_clock_follows_the_recording() {
    let chart = parse_chart(CHART).unwrap();
    // The band rushes: every bar 1.9 s, not 2. By the chorus the chart's
    // own tempo would be half a bar late.
    let starts: Vec<f64> = (0..14).map(|k| 1.0 + 1.9 * k as f64).collect();
    let timeline = Timeline::from_bar_starts(&chart, &starts);
    let line = "[00:12.40]<00:12.40>Hum\n";
    let placement = place_lrc(&lrc::parse(line), &timeline, &PlaceOptions::default());
    // 12.4 s is bar 7 (the chorus's bar 1) on the measured clock.
    let lane = &placement.lanes[&2];
    assert_eq!(lane.syllables[0].measure_index, 0);
    assert_eq!(lane.syllables[0].beat, 0.0);
}

#[test]
fn crowded_words_never_share_a_place_or_reorder() {
    let chart = parse_chart(CHART).unwrap();
    let timeline = Timeline::from_chart(&chart, 1.0);
    let line = "[00:05.00]<00:05.00>a <00:05.01>b <00:05.02>c <00:05.03>d\n";
    let placement = place_lrc(&lrc::parse(line), &timeline, &PlaceOptions::default());
    let places: Vec<(usize, f32)> = placement.lanes[&1]
        .syllables
        .iter()
        .map(|s| (s.measure_index, s.beat))
        .collect();
    assert_eq!(places, vec![(0, 0.0), (0, 0.5), (0, 1.0), (0, 1.5)]);
}
