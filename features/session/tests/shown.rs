//! What a session's chart shows (`keyflow_session::shown`): the session's
//! regions as sections with their bars, its tempo map's tempos and meters,
//! its markers, and Keyflow's typed items in those bars — with or without
//! a chart typed into it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::missing_panics_doc
)]

use keyflow_lang::text::chart::parse_chart;
use keyflow_session::shown::{RoadMap, RoadMarker, RoadSection, RoadTempo};
use keyflow_session::typed::{Held, plan_items};
use session::doc::SessionDoc;
use session::time::TimeSignature;
use session::{MarkId, Marker, Region, TempoMap, Time};

/// A song as the DAW engine writes it into its session: a bar of click
/// before bar 1 (bar 1 at 1.5 s, 160 BPM 4/4), sections on a ruler of
/// their own (`Some(1)`), the whole song as one region on another, markers
/// on a third. Bar 13 (the bridge) drops to 3/4 at 140 BPM; bar 17 (the
/// outro) is back in 4/4 at 140.
fn song() -> SessionDoc {
    let doc = SessionDoc::new();
    let bar = 1.5; // seconds a 4/4 bar at 160
    let bridge = 1.5 + 12.0 * bar; // bar 13
    let bar34 = 3.0 * 60.0 / 140.0; // a 3/4 bar at 140
    let outro = bridge + 4.0 * bar34; // bar 17
    let tempo = TempoMap::new(
        160.0,
        TimeSignature::new(4, 4),
        vec![
            TempoMap::point(0.0, 160.0, Some(TimeSignature::new(4, 4))),
            TempoMap::point(bridge, 140.0, Some(TimeSignature::new(3, 4))),
            TempoMap::point(outro, 140.0, Some(TimeSignature::new(4, 4))),
        ],
    );
    doc.set_tempo(&tempo).unwrap();
    let region = |name: &str, from: f64, to: f64, ruler| Region {
        start: Time::Seconds(from),
        end: Time::Seconds(to),
        name: name.into(),
        color: None,
        ruler,
    };
    let sections = [
        ("IN", 1.5, 1.5 + 4.0 * bar),
        ("VS 1", 1.5 + 4.0 * bar, 1.5 + 8.0 * bar),
        ("CH 1", 1.5 + 8.0 * bar, bridge),
        ("BR", bridge, outro),
        ("OUT", outro, outro + 2.0 * 60.0 / 140.0 * 4.0),
    ];
    for (n, (name, from, to)) in sections.into_iter().enumerate() {
        doc.put_region(
            &MarkId(format!("region-{n}")),
            &region(name, from, to, Some(1)),
        )
        .unwrap();
    }
    doc.put_region(
        &MarkId::from("song"),
        &region("Test Song - Band", 1.5, outro + 8.0 * 60.0 / 140.0, Some(0)),
    )
    .unwrap();
    let marker = |name: &str, at: f64| Marker {
        at: Time::Seconds(at),
        name: name.into(),
        color: None,
        ruler: Some(2),
    };
    doc.put_marker(&MarkId::from("m0"), &marker("song.organized", 0.0))
        .unwrap();
    doc.put_marker(&MarkId::from("m1"), &marker("SONGSTART", 1.5))
        .unwrap();
    doc.put_marker(&MarkId::from("m2"), &marker("Drums in", 1.5 + 4.0 * bar))
        .unwrap();
    doc.set_bar_one(Some(Time::Seconds(1.5))).unwrap();
    doc
}

#[test]
fn a_session_with_no_chart_shows_its_road_map() {
    let held = Held::of_doc(&song());
    assert!(!held.is_typed(), "nobody has charted it");
    assert_eq!(
        held.road_map(),
        RoadMap {
            lead_in: 0,
            sections: vec![
                RoadSection {
                    name: "IN".into(),
                    first_bar: 1,
                    bars: 4
                },
                RoadSection {
                    name: "VS 1".into(),
                    first_bar: 5,
                    bars: 4
                },
                RoadSection {
                    name: "CH 1".into(),
                    first_bar: 9,
                    bars: 4
                },
                RoadSection {
                    name: "BR".into(),
                    first_bar: 13,
                    bars: 4
                },
                RoadSection {
                    name: "OUT".into(),
                    first_bar: 17,
                    bars: 2
                },
            ],
            tempo: vec![
                RoadTempo {
                    bar: 1,
                    bpm: 160.0,
                    meter: (4, 4)
                },
                RoadTempo {
                    bar: 13,
                    bpm: 140.0,
                    meter: (3, 4)
                },
                RoadTempo {
                    bar: 17,
                    bpm: 140.0,
                    meter: (4, 4)
                },
            ],
            // The one before bar 1 is not drawn.
            markers: vec![
                RoadMarker {
                    name: "SONGSTART".into(),
                    bar: 1,
                    beat: 1
                },
                RoadMarker {
                    name: "Drums in".into(),
                    bar: 5,
                    beat: 1
                },
            ],
        }
    );
}

#[test]
fn the_road_map_reads_as_a_chart_of_the_same_bars_tempo_and_meters() {
    let source = Held::of_doc(&song()).shown_source("Test Song");
    let chart = parse_chart(&source).unwrap_or_else(|e| panic!("{e}\n{source}"));
    let sections: Vec<(String, usize)> = chart
        .sections
        .iter()
        .map(|s| (s.section.display_name(), s.measures().len()))
        .collect();
    assert_eq!(
        sections.iter().map(|s| s.1).collect::<Vec<_>>(),
        [4, 4, 4, 4, 2],
        "{sections:?}\n{source}"
    );
    assert_eq!(chart.metadata.title.as_deref(), Some("Test Song"));
    // The opening tempo is the header's, the bridge's is a change.
    assert_eq!(chart.opening_tempo().map(|t| t.bpm), Some(160.0));
    assert_eq!(chart.tempo_changes.len(), 1, "{source}");
    assert_eq!(chart.tempo_changes[0].to_tempo.bpm, 140.0);
    // Each bar in its meter: 3/4 for the bridge only.
    let meters: Vec<(u8, u8)> = chart
        .sections
        .iter()
        .flat_map(|s| s.measures().iter().map(|m| m.time_signature))
        .collect();
    assert_eq!(meters.len(), 18);
    assert!(meters[..12].iter().all(|m| *m == (4, 4)), "{meters:?}");
    assert!(meters[12..16].iter().all(|m| *m == (3, 4)), "{meters:?}");
    assert!(meters[16..].iter().all(|m| *m == (4, 4)), "{meters:?}");
    // No chords: a road map of empty bars.
    assert!(
        chart
            .sections
            .iter()
            .flat_map(|s| s.measures())
            .all(|m| m.chords.iter().all(|c| c.full_symbol.trim() == "s")),
        "{source}"
    );
}

#[test]
fn chords_typed_into_the_session_are_drawn_in_its_bars() {
    let doc = song();
    let held = Held::of_doc(&doc);
    // An edit of the road map: chords typed into the verse's first bar.
    let edited = held.shown_source("Test Song").replacen(
        "VS 4\n^\"Drums in\" s4 s4 s4 s4",
        "VS 4\n^\"Drums in\" C // G //",
        1,
    );
    assert!(edited.contains("C // G //"), "{edited}");
    let chart = parse_chart(&edited).unwrap();
    for request in plan_items(&chart, &held) {
        apply(&doc, request);
    }
    // The session's structure is untouched: its tempo map and regions.
    let after = Held::of_doc(&doc);
    assert_eq!(after.regions, held.regions);
    assert_eq!(
        session::service::TempoMapData::of(&after.tempo),
        session::service::TempoMapData::of(&held.tempo)
    );
    assert!(after.is_typed());
    let typed = after.chart();
    let verse: Vec<(usize, &str, f64)> = typed
        .chords
        .iter()
        .filter(|c| c.symbol != "s")
        .map(|c| (c.bar, c.symbol.as_str(), c.beat))
        .collect();
    assert_eq!(verse, [(4, "C", 1.0), (4, "G", 3.0)]);
    // Read again, the chart shows them where they were typed.
    let shown = parse_chart(&after.shown_source("Test Song")).unwrap();
    let bar5: Vec<String> = shown.sections[1].measures()[0]
        .chords
        .iter()
        .map(|c| c.full_symbol.clone())
        .collect();
    assert_eq!(bar5, ["C", "G"]);
}

/// One request applied as Keyflow, the way a provider answers it.
fn apply(doc: &SessionDoc, request: session::service::Request) {
    let sessions = session::service::Sessions::default();
    sessions.open("song".to_owned(), doc.clone());
    let method = session::service::action_for(&request);
    let call = session::extension::call_json("song", request).unwrap();
    session::extension::answer(&sessions, method, &call, Some("keyflow")).unwrap();
}
