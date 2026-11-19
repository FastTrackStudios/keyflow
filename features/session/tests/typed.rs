//! The chart as typed data in a session document (`keyflow_session::typed`):
//! its bars the session's bars as played, its sections the session's
//! regions, its meters the session's tempo map; edited an item at a time,
//! and merged between two people.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::missing_panics_doc
)]

use crdt::loro::ExportMode;
use keyflow_lang::edit::select::{Step, push_pull_after};
use keyflow_lang::text::chart::parse_chart;
use keyflow_session::typed::{
    self, BARS, CHORD, CHORDS, CUES, Held, TypedChart, Untyped, chord_id, import, plan, read,
    set_flag, set_push,
};
use session::doc::SessionDoc;
use session::{ExtensionId, Item, ItemId, LaneId, MarkId, Time, Value};

/// A chart with a count-in, two sections, a meter change, pushes, marks,
/// cues, a bar that plays another again, and a repeat with two endings.
const SONG: &str = "Typed - Test\n#C 120bpm 4/4\n\nCount 2\n\nVS 4\n\"Pad in\" C // 'G //\n%\n>Am_2 .F_4 ''G_4\n!T3/4 Dm\n\nCH 4\n@drums \"Fill\"\n|: F | [1] G :| [2] Am | C\n";

fn chart() -> keyflow_proto::Chart {
    parse_chart(SONG).expect("the chart parses")
}

/// `from`'s edits brought into `to`.
fn sync(from: &SessionDoc, to: &SessionDoc) {
    let updates = from.doc().export(ExportMode::all_updates()).unwrap();
    to.doc().import(&updates).unwrap();
}

#[test]
fn the_bars_are_the_bars_as_played_each_with_its_own_number() {
    let typed = TypedChart::of(&chart());
    // Eight written bars; the chorus plays F, G, then F again and Am, C:
    // nine bars played, numbered straight through.
    let played: Vec<(usize, usize, u32)> = typed
        .bars
        .iter()
        .map(|b| (b.index + 1, b.written + 1, b.pass))
        .collect();
    assert_eq!(
        played,
        [
            (1, 1, 1),
            (2, 2, 1),
            (3, 3, 1),
            (4, 4, 1),
            (5, 5, 1),
            (6, 6, 1),
            (7, 5, 2),
            (8, 7, 2),
            (9, 8, 1),
        ]
    );
    // Three bars of 4/4, one of 3/4, then 4/4: where each starts.
    let starts: Vec<f64> = typed.bars.iter().map(|b| b.at).collect();
    assert_eq!(starts, [0.0, 4.0, 8.0, 12.0, 15.0, 19.0, 23.0, 27.0, 31.0]);
    assert_eq!(typed.bars[3].meter, (3, 4));
    // The repeat is still written on its bars: where it opens and closes,
    // and which pass each ending is for.
    assert!(
        typed.bars[4].repeat_start && typed.bars[6].repeat_start,
        "bar 5, both passes"
    );
    assert!(typed.bars[5].repeat_end);
    assert_eq!(typed.bars[5].repeat_count, 2);
    assert_eq!(typed.bars[5].ending, [1]);
    assert_eq!(typed.bars[7].ending, [2]);
    assert!(typed.bars[1].repeat_sign, "bar 2 is `%`");
    // The chorus is one section, all five bars of it as played.
    let sections = typed.sections();
    assert_eq!(sections.len(), 2);
    assert_eq!((sections[0].1, sections[0].2), (0.0, 15.0));
    assert_eq!((sections[1].1, sections[1].2), (15.0, 35.0));
}

#[test]
fn what_a_bar_plays_is_on_every_bar_that_plays_it() {
    let typed = TypedChart::of(&chart());
    let chords: Vec<(usize, &str, f64, f64)> = typed
        .chords
        .iter()
        .map(|c| (c.bar + 1, c.symbol.as_str(), c.beat, c.length))
        .collect();
    // (Bar 8, the second ending, is checked by its chord alone: where the
    // parser puts a chord written after `[2]` is the parser's to say.)
    assert_eq!(chords[11].0, 8);
    assert_eq!(chords[11].1, "Am");
    let mut chords = chords;
    chords[11] = (8, "Am", 1.0, 4.0);
    assert_eq!(
        chords,
        [
            (1, "C", 1.0, 2.0),
            (1, "G", 3.0, 2.0),
            // Bar 2 is `%`: bar 1's chords, played again.
            (2, "C", 1.0, 2.0),
            (2, "G", 3.0, 2.0),
            (3, "Am", 1.0, 2.0),
            (3, "F", 3.0, 1.0),
            (3, "G", 4.0, 1.0),
            (4, "Dm", 1.0, 3.0),
            (5, "F", 1.0, 4.0),
            (6, "G", 1.0, 4.0),
            // The repeat's second pass: bar 7 is the F again.
            (7, "F", 1.0, 4.0),
            (8, "Am", 1.0, 4.0),
            (9, "C", 1.0, 4.0),
        ]
    );
    // One written chord, wherever it is played.
    assert_eq!(typed.chords[8].written_bar, typed.chords[10].written_bar);
    assert_eq!(typed.chords[2].written_bar, 0, "bar 2 plays bar 1's");
    // The push is the chord's own field, at its level; the marks too.
    let g = &typed.chords[1];
    assert_eq!(g.push.map(|(early, a)| (early, a.level)), Some((true, 1)));
    assert_eq!(
        typed.chords[6].push.map(|(e, a)| (e, a.level)),
        Some((true, 2))
    );
    assert!(typed.chords[4].accent && typed.chords[5].staccato);
    // Where it is written, not where it sounds: beat 3 of bar 1.
    assert_eq!(g.at, 2.0);
    // A cue is its own bar's: not on the pass that plays the bar again.
    let cues: Vec<(usize, &str)> = typed
        .cues
        .iter()
        .map(|c| (c.bar + 1, c.text.as_str()))
        .collect();
    assert_eq!(cues, [(1, "Pad in"), (5, "Fill"), (7, "Fill")]);
    assert_eq!(typed.cues[1].group, "drums");
}

#[test]
fn the_sections_are_the_sessions_regions_and_the_meters_its_tempo_map() {
    let doc = SessionDoc::new();
    let typed = import(&doc, &chart()).unwrap();
    assert_eq!(
        typed,
        TypedChart::of(&chart()),
        "read back from the session"
    );

    // The sections are regions of the session: nothing else holds them.
    let regions = doc.regions();
    let names: Vec<&str> = regions.values().map(|r| r.name.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert_eq!(
        regions[&MarkId::from("keyflow.section.1")].start,
        Time::QuarterNotes(15.0)
    );
    for bar in doc.items().values().filter(|i| i.lane.as_str() == BARS) {
        assert!(bar.payload.get("section").is_none(), "no second copy");
        assert!(bar.payload.get("numerator").is_none(), "nor of the meter");
    }
    // Renamed in the session, it is the chart's section renamed.
    let id = MarkId::from("keyflow.section.0");
    let mut verse = regions[&id].clone();
    verse.name = "Verse (half time)".to_owned();
    doc.put_region(&id, &verse).unwrap();
    let now = read(&doc);
    assert_eq!(now.bars[0].section, "Verse (half time)");
    assert_eq!(now.bars[3].section, "Verse (half time)");
    assert_eq!(now.bars[4].section, names[1]);
    // And an import of the same chart leaves the session's name alone.
    assert!(plan(&chart(), &Held::of_doc(&doc)).is_empty());

    // The meters are the session's tempo map: bar 4 is in 3/4 because the
    // map says so, and the session counts the same bars the chart does.
    let tempo = doc.tempo();
    for bar in &typed.bars {
        let at = tempo.bar_beat(bar.at);
        assert_eq!(
            (at.bar, at.beat),
            (bar.index as i32 + 1, 1),
            "bar {}",
            bar.index + 1
        );
    }
    let in_bar_4 = Time::QuarterNotes(13.0).seconds(&tempo);
    let meter = tempo.meter_at(in_bar_4);
    assert_eq!((meter.numerator, meter.denominator), (3, 4));
    assert!((tempo.bpm_at(in_bar_4) - 120.0).abs() < 1e-6);
}

#[test]
fn importing_again_changes_nothing() {
    let doc = SessionDoc::new();
    import(&doc, &chart()).unwrap();
    for lane in [BARS, CHORDS, CUES] {
        assert_eq!(doc.lanes()[&LaneId::from(lane)].owner.as_str(), "keyflow");
    }
    assert_eq!(
        doc.read_ext(&ExtensionId::from("keyflow"))
            .get(typed::SCHEMA_ENTRY),
        Some(&session::Entry::Register(Value::Int(typed::SCHEMA)))
    );
    let before = doc.doc().oplog_vv();
    import(&doc, &chart()).unwrap();
    assert_eq!(doc.doc().oplog_vv(), before, "not one write");
}

#[test]
fn the_charts_bar_one_is_the_sessions_and_its_lead_in_is_kept() {
    let doc = SessionDoc::new();
    // Two bars of count-in at 100 BPM before the song's bar 1.
    let lead_in =
        session::TempoMap::new(100.0, session::time::TimeSignature::new(4, 4), Vec::new());
    doc.set_tempo(&lead_in).unwrap();
    doc.set_bar_one(Some(Time::QuarterNotes(8.0))).unwrap();
    let typed = import(&doc, &chart()).unwrap();
    assert_eq!(typed, TypedChart::of(&chart()));
    let first = doc.item(&typed::bar_id(0)).unwrap();
    assert_eq!(first.at, Time::QuarterNotes(8.0));
    assert_eq!(first.end, Some(Time::QuarterNotes(12.0)));
    // The count-in keeps its tempo; the song has the chart's from bar 1.
    let tempo = doc.tempo();
    assert!((tempo.bpm_at(1.0) - 100.0).abs() < 1e-6);
    let bar_one = Time::QuarterNotes(8.0).seconds(&tempo);
    assert!((bar_one - 4.8).abs() < 1e-9, "eight quarters at 100");
    assert!((tempo.bpm_at(bar_one + 0.1) - 120.0).abs() < 1e-6);
    // The session's bar 3 is the song's bar 1, and they count on together.
    let at = tempo.bar_beat(8.0 + typed.bars[8].at);
    assert_eq!((at.bar, at.beat), (3 + 8, 1));
}

#[test]
fn a_push_is_written_to_the_chord_wherever_it_is_played() {
    let doc = SessionDoc::new();
    import(&doc, &chart()).unwrap();
    // The chorus's F (written bar 5), played in bars 5 and 7.
    let eighth = push_pull_after(None, true, None).unwrap();
    assert_eq!(set_push(&doc, 4, 0, eighth).unwrap(), 2, "both passes");
    let now = read(&doc);
    let f: Vec<_> = now
        .chords
        .iter()
        .filter(|c| c.symbol == "F" && c.written_bar == 4)
        .collect();
    assert_eq!(f.len(), 2);
    assert!(f.iter().all(|c| c.push == eighth));
    // Nothing else moved.
    let before = TypedChart::of(&chart());
    for (a, b) in before.chords.iter().zip(&now.chords) {
        if a.written_bar != 4 {
            assert_eq!(a, b);
        }
        assert_eq!((a.at, a.length, &a.symbol), (b.at, b.length, &b.symbol));
    }
    // Every amount the type holds is kept: a triplet's, an explicit value's.
    for step in [Step::TripletEighth, Step::Quarter, Step::ThirtySecond] {
        let field = push_pull_after(None, true, Some(step)).unwrap();
        set_push(&doc, 2, 0, field).unwrap();
        assert_eq!(read(&doc).chords[4].push, field, "{step:?}");
    }
    // Pulled back to nothing: the fields are gone, not zeroed.
    set_push(&doc, 2, 0, None).unwrap();
    assert_eq!(read(&doc).chords[4].push, None);
    let Value::Map(payload) = doc.item(&chord_id(2, 0)).unwrap().payload else {
        panic!("a map");
    };
    assert!(
        payload.keys().all(|k| !k.starts_with("push_")),
        "{payload:?}"
    );
    assert_eq!(set_push(&doc, 99, 0, None).unwrap(), 0, "no such chord");
}

#[test]
fn two_people_editing_one_chord_both_land() {
    let ours = SessionDoc::new();
    import(&ours, &chart()).unwrap();
    let theirs = SessionDoc::new();
    sync(&ours, &theirs);
    assert_eq!(read(&theirs), read(&ours));

    // At once: we push bar 9's C, they put a staccato on it; and they
    // push bar 4's Dm.
    let eighth = push_pull_after(None, true, None).unwrap();
    set_push(&ours, 7, 0, eighth).unwrap();
    set_flag(&theirs, &chord_id(8, 0), "staccato", true).unwrap();
    set_push(&theirs, 3, 0, eighth).unwrap();
    sync(&ours, &theirs);
    sync(&theirs, &ours);

    let (a, b) = (read(&ours), read(&theirs));
    assert_eq!(a, b, "both hold the same chart");
    let c = a.chords.iter().find(|c| c.bar == 8).unwrap();
    assert_eq!(c.push, eighth, "our push");
    assert!(c.staccato, "their staccato");
    assert_eq!(a.chords.iter().find(|c| c.bar == 3).unwrap().push, eighth);
}

#[test]
fn what_the_chart_did_not_bring_is_left() {
    let doc = SessionDoc::new();
    import(&doc, &chart()).unwrap();
    let theirs = Item {
        owner: ExtensionId::from("ignition"),
        lane: LaneId::from(CHORDS),
        kind: "ignition.cue".into(),
        at: Time::QuarterNotes(4.0),
        end: None,
        payload: Value::Null,
    };
    doc.put_item(&ItemId::from("ignition.cue.1"), &theirs)
        .unwrap();
    let song = session::Region {
        start: Time::QuarterNotes(0.0),
        end: Time::QuarterNotes(35.0),
        name: "Typed".into(),
        color: None,
        ruler: Some(1),
    };
    doc.put_region(&MarkId::from("set.song.1"), &song).unwrap();
    // A shorter chart: Keyflow's items and sections it has no more go.
    let shorter = parse_chart("Typed - Test\n#C 120bpm 4/4\n\nVS 2\nC G\n").unwrap();
    let typed = import(&doc, &shorter).unwrap();
    assert_eq!(typed.bars.len(), 2);
    assert_eq!(doc.items().values().filter(|i| i.kind == CHORD).count(), 2);
    assert_eq!(doc.item(&ItemId::from("ignition.cue.1")), Some(theirs));
    assert_eq!(
        doc.regions().len(),
        2,
        "its one section, and the set's song"
    );
    assert_eq!(doc.regions()[&MarkId::from("set.song.1")], song);
}

#[test]
fn what_is_not_typed_yet_is_counted() {
    assert_eq!(
        TypedChart::untyped(&parse_chart("Plain - Test\n#C 120bpm 4/4\n\nVS 2\nC G\n").unwrap()),
        Untyped::default()
    );
    let more = "More - Test\n#C 120bpm 4/4\n\nVS 4\ndyn mf\nC_2 r2\n<< F ; m{ C:44 D:44 E:42 } >>\n|: G :|\nC\n";
    let untyped = TypedChart::untyped(&parse_chart(more).unwrap());
    assert!(!untyped.is_nothing());
    assert_eq!(untyped.bars_with_notes, 1, "{untyped:?}");
    assert_eq!(untyped.bars_unplaced, 0);
}

#[test]
fn rests_and_dynamics_are_items_too() {
    let src = "Hits - Test\n#C 120bpm 4/4\n\nVS 3\ndyn mf\nC_2 r2\n>F_8 r8 r2.\ndyn ff\nG\n";
    let chart = parse_chart(src).unwrap();
    let typed = TypedChart::of(&chart);
    let rests: Vec<(usize, f64, f64)> = typed
        .rests
        .iter()
        .map(|r| (r.bar, r.beat, r.length))
        .collect();
    assert_eq!(rests, [(0, 3.0, 2.0), (1, 1.5, 0.5), (1, 2.0, 3.0)]);
    let dynamics: Vec<(usize, &str)> = typed
        .dynamics
        .iter()
        .map(|d| (d.bar, d.text.as_str()))
        .collect();
    assert_eq!(dynamics, [(0, "mf"), (2, "ff")]);
    assert!(
        TypedChart::untyped(&chart).is_nothing(),
        "the whole chart is typed"
    );
    let doc = SessionDoc::new();
    assert_eq!(read(&doc), TypedChart::default());
    assert_eq!(import(&doc, &chart).unwrap(), typed);
}

/// Through `fts.session`, as the chart view reaches a session held by any
/// provider: the chart typed into it (its structure by the `structure`
/// action), read back, and a chord pushed.
#[test]
fn a_session_held_by_a_provider_is_typed_through_the_service() {
    use keyflow_session::{ChartSource, EXTENSION, Provider};
    use session::service::{STRUCTURE, WRITE};

    let doc = SessionDoc::new();
    doc.set_bar_one(Some(Time::QuarterNotes(8.0))).unwrap();
    let id = uuid::Uuid::new_v4();
    let provider = Provider::default();
    provider.open_doc(id, doc.clone());
    let source = ChartSource::Session {
        doc: id.to_string(),
    };
    let ask = |call: &keyflow_session::ServiceCall| {
        provider
            .answer(call.method, &call.args, Some(EXTENSION))
            .unwrap()
    };
    let held = || {
        let [t, s, d] = source.timeline_calls().unwrap().unwrap();
        Held::of_replies(&ask(&t), &ask(&s), &ask(&d)).unwrap()
    };

    let now = held();
    assert_eq!(now.origin, 8.0, "the session's bar 1");
    assert!(!now.is_typed());
    let calls = source.typed_calls(plan(&chart(), &now)).unwrap();
    assert!(calls.iter().filter(|c| c.method == WRITE).count() > 20);
    for call in &calls {
        ask(call);
    }
    // The tempo map and each region went by the structure action.
    assert_eq!(
        calls.iter().filter(|c| c.method == STRUCTURE).count(),
        1 + doc.regions().len()
    );
    assert!(
        now.brings().tempo && now.brings().sections,
        "it had none of its own"
    );
    assert_eq!(doc.regions().len(), 2);
    // What the provider's document holds is the chart, and so is what the
    // service reads back; a second plan has nothing to do.
    let wanted = TypedChart::of(&chart());
    assert_eq!(read(&doc), wanted);
    let now = held();
    assert!(now.is_typed());
    assert_eq!(now.chart(), wanted);
    assert!(plan(&chart(), &now).is_empty());

    // Push the chorus's F: an item put for each time it is played.
    let pushed = push_pull_after(None, true, None).unwrap();
    let calls = source
        .typed_calls(typed::push_requests(&now.items, 4, 0, pushed))
        .unwrap();
    assert_eq!(calls.len(), 2);
    for call in &calls {
        ask(call);
    }
    // With bar 1's pushed G, which bar 2 plays again: four.
    assert_eq!(
        read(&doc)
            .chords
            .iter()
            .filter(|c| c.push == pushed)
            .count(),
        4
    );

    // Another extension reads where the chords are, not what they are.
    let [t, ..] = source.timeline_calls().unwrap().unwrap();
    let theirs = provider
        .answer(t.method, &t.args, Some("ignition"))
        .unwrap();
    let session::service::Response::Timeline { items, .. } =
        session::extension::response_of(&theirs).unwrap()
    else {
        panic!("the timeline");
    };
    let chord = &items[&chord_id(0, 1)];
    assert_eq!(chord.kind, CHORD);
    assert_eq!(
        chord.at,
        Time::QuarterNotes(10.0),
        "bar 1 beat 3, from the session's bar 1"
    );
    assert_eq!(chord.payload, Value::Null, "Keyflow's payload is Keyflow's");
    // A document has no timeline to type.
    assert!(
        ChartSource::Document("a.kf".into())
            .timeline_calls()
            .unwrap()
            .is_none()
    );
}

/// A session that has its own tempo map and sections from bar 1 (a
/// recording, measured and arranged) keeps them: the chart is typed
/// against them, never over them, and a chart that does not fit is told.
#[test]
fn a_sessions_own_structure_is_kept_and_the_chart_read_against_it() {
    use session::time::TimeSignature;
    let doc = SessionDoc::new();
    // Measured: 118 BPM, easing to 121 in bar 3, with bar 4 in 3/4.
    let meter = |n, d| Some(TimeSignature::new(n, d));
    let bar = |q: f64| q * 60.0 / 118.0;
    let own = session::TempoMap::new(
        118.0,
        TimeSignature::new(4, 4),
        vec![
            session::TempoMap::point(bar(8.0), 121.0, None),
            session::TempoMap::point(bar(8.0) + 4.0 * 60.0 / 121.0, 121.0, meter(3, 4)),
            session::TempoMap::point(bar(8.0) + 7.0 * 60.0 / 121.0, 121.0, meter(4, 4)),
        ],
    );
    doc.set_tempo(&own).unwrap();
    let arranged = session::Region {
        start: Time::QuarterNotes(0.0),
        end: Time::QuarterNotes(15.0),
        name: "Verse A".into(),
        color: Some(0x33_66_99),
        ruler: None,
    };
    doc.put_region(&MarkId::from("arr.1"), &arranged).unwrap();
    doc.put_region(
        &MarkId::from("arr.2"),
        &session::Region {
            start: Time::QuarterNotes(15.0),
            end: Time::QuarterNotes(35.0),
            name: "Chorus A".into(),
            ..arranged.clone()
        },
    )
    .unwrap();

    let held = Held::of_doc(&doc);
    assert_eq!(
        held.brings(),
        typed::Brings {
            tempo: false,
            sections: false
        }
    );
    assert!(
        held.disagreements(&chart()).is_empty(),
        "{:?}",
        held.disagreements(&chart())
    );
    let typed = import(&doc, &chart()).unwrap();
    // The session's structure is as it was: its tempo, its regions only.
    assert_eq!(
        session::service::TempoMapData::of(&doc.tempo()),
        session::service::TempoMapData::of(&own)
    );
    assert_eq!(doc.regions().len(), 2);
    assert_eq!(doc.regions()[&MarkId::from("arr.1")], arranged);
    // And the chart's sections are those: by their names.
    assert_eq!(typed.bars[0].section, "Verse A");
    assert_eq!(typed.bars[4].section, "Chorus A");
    assert_eq!(typed.bars[3].meter, (3, 4), "from the session's map");
    // Still kept on a later edit, read again from the document.
    let again = Held::of_doc(&doc);
    assert!(!again.brings().tempo);
    assert!(plan(&chart(), &again).is_empty());

    // A chart that does not fit is told where, bar by bar.
    let other = parse_chart("Other - Test\n#C 120bpm 4/4\n\nVS 6\nC G Am F C G\n").unwrap();
    let why = again.disagreements(&other);
    assert!(
        why.iter().any(|w| w.contains("Bar 4") && w.contains("3/4")),
        "{why:?}"
    );
    assert!(
        why.iter()
            .any(|w| w.starts_with("Bar 5 of the chart starts at")),
        "{why:?}"
    );
}
