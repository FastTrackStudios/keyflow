//! The chart as typed data in a session document (`keyflow_session::typed`):
//! imported from a `.kf`, read back the same, edited an item at a time,
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
    self, BARS, CHORD, CHORDS, CUES, TypedChart, Untyped, chord_id, import, read, set_flag,
    set_push,
};
use session::doc::SessionDoc;
use session::{ExtensionId, Item, ItemId, LaneId, Time, Value};

/// A chart with a count-in, two sections, a meter change, pushes, marks,
/// cues and a bar that plays another again.
const SONG: &str = "Typed - Test\n#C 120bpm 4/4\n\nCount 2\n\nVS 4\n\"Pad in\" C // 'G //\n%\n>Am_2 .F_4 ''G_4\n!T3/4 Dm\n\nCH 2\n@drums \"Fill\"\nF G\n";

fn chart() -> keyflow_proto::Chart {
    parse_chart(SONG).expect("the chart parses")
}

/// `from`'s edits brought into `to`.
fn sync(from: &SessionDoc, to: &SessionDoc) {
    let updates = from.doc().export(ExportMode::all_updates()).unwrap();
    to.doc().import(&updates).unwrap();
}

#[test]
fn a_chart_is_bars_chords_and_cues_on_the_timeline() {
    let typed = TypedChart::of(&chart());
    // Six bars: the count-in is not one.
    assert_eq!(typed.bars.len(), 6);
    let starts: Vec<f64> = typed.bars.iter().map(|b| b.at).collect();
    // Three bars of 4/4, one of 3/4, then two of 4/4 again.
    assert_eq!(starts, [0.0, 4.0, 8.0, 12.0, 15.0, 19.0]);
    assert_eq!(typed.bars[3].meter, (3, 4));
    assert_eq!(typed.bars[1].plays, None);
    assert!(typed.bars[1].repeat_sign, "bar 2 is `%`");
    assert!(typed.bars[4].section_start && typed.bars[0].section_start);
    assert_ne!(typed.bars[4].section, typed.bars[0].section);

    let chords: Vec<(usize, &str, f64, f64)> = typed
        .chords
        .iter()
        .map(|c| (c.bar, c.symbol.as_str(), c.beat, c.length))
        .collect();
    assert_eq!(
        chords,
        [
            (0, "C", 1.0, 2.0),
            (0, "G", 3.0, 2.0),
            // Bar 2 writes none of its own.
            (2, "Am", 1.0, 2.0),
            (2, "F", 3.0, 1.0),
            (2, "G", 4.0, 1.0),
            (3, "Dm", 1.0, 3.0),
            (4, "F", 1.0, 4.0),
            (5, "G", 1.0, 4.0),
        ]
    );
    // The push is the chord's own field, at its level; the marks too.
    let g = &typed.chords[1];
    assert_eq!(g.push.map(|(early, a)| (early, a.level)), Some((true, 1)));
    assert_eq!(
        typed.chords[4].push.map(|(e, a)| (e, a.level)),
        Some((true, 2))
    );
    assert!(typed.chords[2].accent && !typed.chords[2].staccato);
    assert!(typed.chords[3].staccato);
    // Where it is written, not where it sounds: beat 3 of bar 1.
    assert_eq!(g.at, 2.0);

    let cues: Vec<(usize, &str)> = typed
        .cues
        .iter()
        .map(|c| (c.bar, c.text.as_str()))
        .collect();
    assert_eq!(cues, [(0, "Pad in"), (4, "Fill")]);
    assert_eq!(typed.cues[1].group, "drums");
}

#[test]
fn what_is_imported_reads_back_the_same_and_importing_again_changes_nothing() {
    let doc = SessionDoc::new();
    let typed = import(&doc, &chart()).unwrap();
    assert_eq!(read(&doc), typed);
    // Keyflow's lanes, and its schema's version in its namespace.
    let lanes = doc.lanes();
    for lane in [BARS, CHORDS, CUES] {
        assert_eq!(lanes[&LaneId::from(lane)].owner.as_str(), "keyflow");
    }
    assert_eq!(
        doc.read_ext(&ExtensionId::from("keyflow"))
            .get(typed::SCHEMA_ENTRY),
        Some(&session::Entry::Register(Value::Int(typed::SCHEMA)))
    );
    // A second import writes nothing: the document's version stays.
    let before = doc.doc().oplog_vv();
    import(&doc, &chart()).unwrap();
    assert_eq!(doc.doc().oplog_vv(), before);
}

#[test]
fn the_charts_bar_one_is_the_sessions() {
    let doc = SessionDoc::new();
    // Two bars of count-in before the song's bar 1.
    doc.set_bar_one(Some(Time::QuarterNotes(8.0))).unwrap();
    let typed = import(&doc, &chart()).unwrap();
    let first = doc.item(&typed::bar_id(0)).unwrap();
    assert_eq!(first.at, Time::QuarterNotes(8.0));
    assert_eq!(first.end, Some(Time::QuarterNotes(12.0)));
    // Read back from bar 1, whatever the session starts with.
    assert_eq!(read(&doc), typed);
    assert_eq!(typed.bars[0].at, 0.0);
}

#[test]
fn a_push_is_one_write_to_the_chords_own_field() {
    let doc = SessionDoc::new();
    import(&doc, &chart()).unwrap();
    // Bar 3's Am, not pushed: Push steps its field.
    let am = chord_id(2, 0);
    let was = read(&doc).chords[2].clone();
    assert_eq!(was.push, None);
    let pushed = push_pull_after(was.push, true, None).unwrap();
    assert!(set_push(&doc, &am, pushed).unwrap());
    let now = read(&doc).chords[2].clone();
    assert_eq!(now.push.map(|(early, a)| (early, a.level)), Some((true, 1)));
    // Nothing else about it moved, and no other item changed.
    assert_eq!(
        TypedChart {
            chords: vec![was.clone()],
            ..TypedChart::default()
        }
        .chords[0]
            .symbol,
        now.symbol
    );
    assert!(now.accent && now.at == was.at && now.length == was.length);
    assert_eq!(
        read(&doc)
            .chords
            .iter()
            .filter(|c| c.push.is_some())
            .count(),
        3
    );
    // Every amount the type holds is kept: a triplet's, an explicit value's.
    for step in [Step::TripletEighth, Step::Quarter, Step::ThirtySecond] {
        let field = push_pull_after(None, true, Some(step)).unwrap();
        set_push(&doc, &am, field).unwrap();
        assert_eq!(read(&doc).chords[2].push, field, "{step:?}");
    }
    // Pulled back to nothing: the fields are gone, not zeroed.
    set_push(&doc, &am, None).unwrap();
    assert_eq!(read(&doc).chords[2].push, None);
    let Value::Map(payload) = doc.item(&am).unwrap().payload else {
        panic!("a map");
    };
    assert!(
        payload.keys().all(|k| !k.starts_with("push_")),
        "{payload:?}"
    );
    assert!(!set_push(&doc, &ItemId::from("keyflow.chord.99.0"), None).unwrap());
}

#[test]
fn two_people_editing_one_chord_both_land() {
    let ours = SessionDoc::new();
    import(&ours, &chart()).unwrap();
    let theirs = SessionDoc::new();
    sync(&ours, &theirs);
    assert_eq!(read(&theirs), read(&ours));

    // At once: we push bar 5's F, they put a staccato on it; and they
    // push bar 6's G.
    let f = chord_id(4, 0);
    let eighth = push_pull_after(None, true, None).unwrap();
    set_push(&ours, &f, eighth).unwrap();
    set_flag(&theirs, &f, "staccato", true).unwrap();
    set_push(&theirs, &chord_id(5, 0), eighth).unwrap();
    sync(&ours, &theirs);
    sync(&theirs, &ours);

    let (a, b) = (read(&ours), read(&theirs));
    assert_eq!(a, b, "both hold the same chart");
    let f = a.chords.iter().find(|c| c.bar == 4).unwrap();
    assert_eq!(f.push, eighth, "our push");
    assert!(f.staccato, "their staccato");
    assert_eq!(a.chords.iter().find(|c| c.bar == 5).unwrap().push, eighth);
}

#[test]
fn another_extensions_items_on_the_lanes_are_left() {
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
    // A shorter chart: Keyflow's items it has no more are removed.
    let shorter = parse_chart("Typed - Test\n#C 120bpm 4/4\n\nVS 2\nC G\n").unwrap();
    let typed = import(&doc, &shorter).unwrap();
    assert_eq!(read(&doc), typed);
    assert_eq!(typed.bars.len(), 2);
    assert_eq!(doc.items().values().filter(|i| i.kind == CHORD).count(), 2);
    assert_eq!(doc.item(&ItemId::from("ignition.cue.1")), Some(theirs));
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
    assert_eq!(untyped.repeats, 1);
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
    import(&doc, &chart).unwrap();
    assert_eq!(read(&doc), typed);
}

/// Through `fts.session`, as the chart view reaches a session held by any
/// provider: the chart typed into it, read back, and a chord pushed with
/// one call.
#[test]
fn a_session_held_by_a_provider_is_typed_through_the_service() {
    use keyflow_session::{ChartSource, EXTENSION, Provider};
    use session::service::Request;

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
    let timeline = || {
        let [t, s] = source.timeline_calls().unwrap().unwrap();
        typed::Timeline::of(&ask(&t), &ask(&s)).unwrap()
    };

    // Typed into it: the plan against what it holds, call by call.
    let wanted = TypedChart::of(&chart());
    let now = timeline();
    assert_eq!(now.origin, 8.0, "the session's bar 1");
    let requests = typed::plan(&wanted, now.origin, &now.lanes, &now.items);
    assert!(requests.len() > wanted.bars.len());
    for call in source.typed_calls(requests).unwrap() {
        ask(&call);
    }
    // What the provider's document holds is the chart, and so is what the
    // service reads back; a second plan has nothing to do.
    assert_eq!(read(&doc), wanted);
    let now = timeline();
    assert_eq!(now.chart(), wanted);
    assert!(typed::plan(&wanted, now.origin, &now.lanes, &now.items).is_empty());

    // Push bar 3's Am: one item put.
    let am = chord_id(2, 0);
    let pushed = push_pull_after(None, true, None).unwrap();
    let item = typed::with_push(now.items[&am].clone(), pushed).unwrap();
    let calls = source
        .typed_calls(vec![Request::PutItem { id: am, item }])
        .unwrap();
    assert_eq!(calls.len(), 1);
    ask(&calls[0]);
    assert_eq!(read(&doc).chords[2].push, pushed);

    // Another extension reads where the chords are, not what they are.
    let [t, _] = source.timeline_calls().unwrap().unwrap();
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
