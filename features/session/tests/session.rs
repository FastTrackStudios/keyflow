//! Keyflow as a session's host: a chart alone in its file, two Keyflows
//! sharing it through Architect's sync, typed through the editor binding,
//! and the chart view reading and editing it through `fts.session`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::missing_panics_doc
)]

use std::time::Duration;

use architect::{LayerRouter, LocalServer, Scope};
use crdt::sync::{DocSyncClient, DocSyncDispatcher, doc_sync_service_descriptor};
use crdt::text::{EditorBinding, TextOp};
use crdt::{CrdtDoc, DocRegistry};
use keyflow_session::{ChartSession, ChartSource, EXTENSION, Provider};
use session::service::{READ, WRITE};

const VERSE: &str = "Verse: C G Am F\n";

/// An editor's text, as a chord-chart editor holds it.
#[derive(Default)]
struct Editor(Vec<char>);

impl Editor {
    fn text(&self) -> String {
        self.0.iter().collect()
    }
}

impl EditorBinding for Editor {
    fn apply_remote(&mut self, ops: &[TextOp]) {
        for op in ops {
            match op {
                TextOp::Insert { pos, text } => {
                    let at = usize::try_from(*pos).unwrap();
                    for (i, c) in text.chars().enumerate() {
                        self.0.insert(at + i, c);
                    }
                }
                TextOp::Delete { pos, len } => {
                    let at = usize::try_from(*pos).unwrap();
                    self.0.drain(at..at + usize::try_from(*len).unwrap());
                }
            }
        }
    }
}

async fn eventually(what: &str, mut pred: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !pred() {
        assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chart_alone_lives_in_its_session_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Belief.session");
    let id = {
        let chart = ChartSession::create(&path, VERSE).await.unwrap();
        assert_eq!(chart.session().kind().as_deref(), Some(session::doc::KIND));
        chart.set_chart(&format!("{VERSE}Chorus: F G C\n")).unwrap();
        chart.save().await.unwrap();
        chart.id()
    };
    let chart = ChartSession::open(&path).await.unwrap();
    assert_eq!(chart.id(), id);
    assert_eq!(chart.chart(), format!("{VERSE}Chorus: F G C\n"));
    // Keyflow's data is Keyflow's: the session holds no chart of its own.
    let ids: Vec<String> = chart
        .session()
        .read()
        .extension_ids()
        .map(|e| e.as_str().to_owned())
        .collect();
    assert_eq!(ids, [EXTENSION]);
}

#[tokio::test(flavor = "multi_thread")]
async fn two_keyflows_share_a_session_and_type_into_it() {
    // The host the two sync through: Architect's sync service, as an org
    // lane or a share link serves it, here in process.
    let scope = Scope::new();
    let registry = DocRegistry::new(|_| Box::pin(async { Ok(CrdtDoc::ephemeral()) }));
    let router = LayerRouter::new().with(
        doc_sync_service_descriptor(),
        DocSyncDispatcher::new(registry),
    );
    let local = LocalServer::serve(router, scope.clone());
    let sync = |chart: &ChartSession| {
        let synced = chart.synced();
        let local = &local;
        async move {
            let client: DocSyncClient = local.establish().await.unwrap();
            let mut synced = synced;
            tokio::spawn(async move {
                let _ = synced.run(&client).await;
            });
        }
    };

    let dir = tempfile::tempdir().unwrap();
    let a = ChartSession::create(&dir.path().join("a.session"), VERSE)
        .await
        .unwrap();
    // B adopts the session A made: its own file, the same lineage.
    let b_path = dir.path().join("b.session");
    let b = ChartSession::adopt(&b_path, a.id()).await.unwrap();
    sync(&a).await;
    sync(&b).await;
    eventually("B has A's chart", || b.chart() == VERSE).await;

    // Both type, each in an editor bound to the chart.
    let a_text = a.binding().unwrap();
    let b_text = b.binding().unwrap();
    let mut a_editor = Editor(a_text.shown().chars().collect());
    let mut b_editor = Editor(b_text.shown().chars().collect());
    a_text
        .on_local(&[TextOp::Insert {
            pos: 0,
            text: "Intro: C\n".into(),
        }])
        .unwrap();
    a_editor.apply_remote(&[TextOp::Insert {
        pos: 0,
        text: "Intro: C\n".into(),
    }]);
    let end = u32::try_from(b_editor.0.len()).unwrap();
    b_text
        .on_local(&[TextOp::Insert {
            pos: end,
            text: "Chorus: F G C\n".into(),
        }])
        .unwrap();
    b_editor.apply_remote(&[TextOp::Insert {
        pos: end,
        text: "Chorus: F G C\n".into(),
    }]);
    let both = format!("Intro: C\n{VERSE}Chorus: F G C\n");
    eventually("both editors show both edits", || {
        a_text.sync_into(&mut a_editor);
        b_text.sync_into(&mut b_editor);
        a_editor.text() == both && b_editor.text() == both
    })
    .await;
    assert_eq!(a.chart(), both);

    // B's file holds what it received as it arrived (a durable sync): the
    // process ends with no save, and the file, reopened, reads the chart.
    drop(b_text);
    drop(b);
    let reopened = ChartSession::open(&b_path).await.unwrap();
    assert_eq!(reopened.chart(), both);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_chart_view_reads_and_edits_through_fts_session() {
    let dir = tempfile::tempdir().unwrap();
    let chart = ChartSession::create(&dir.path().join("song.session"), VERSE)
        .await
        .unwrap();
    let provider = Provider::default();
    provider.open(&chart);

    // The pane's state names the session; the view asks the service, as
    // the host runs the provider's action for it (`cx.caller`).
    let source = ChartSource::of_pane_state(&format!("session:{}", chart.id()));
    assert_eq!(
        ChartSource::of_pane_state(&source.pane_state()),
        source,
        "a pane state round-trips"
    );
    let read = source.read_call().unwrap().unwrap();
    assert_eq!((read.service, read.method), ("fts.session", READ));
    let reply = provider
        .answer(read.method, &read.args, Some(EXTENSION))
        .unwrap();
    assert_eq!(ChartSource::chart_of(&reply).unwrap(), VERSE);

    // An edit on the chart: a splice, through `write`.
    let edit = source.splice_call(7, 1, "D").unwrap().unwrap();
    assert_eq!(edit.method, WRITE);
    provider
        .answer(edit.method, &edit.args, Some(EXTENSION))
        .unwrap();
    assert_eq!(chart.chart(), "Verse: D G Am F\n");

    // The same edit through `read` (all a read-only link's guest may run)
    // is refused, and writes nothing.
    assert!(provider.answer(READ, &edit.args, Some(EXTENSION)).is_err());
    assert_eq!(chart.chart(), "Verse: D G Am F\n");

    // Another extension reading the session sees no chart: Keyflow's data
    // is in Keyflow's namespace.
    let theirs = provider
        .answer(read.method, &read.args, Some("ignition"))
        .unwrap();
    assert_eq!(ChartSource::chart_of(&theirs).unwrap(), "");

    // A chart alone needs no session: its source names a document, read
    // as before.
    let alone = ChartSource::of_pane_state("Charts/Belief.kf");
    assert_eq!(alone, ChartSource::Document("Charts/Belief.kf".into()));
    assert!(alone.read_call().unwrap().is_none());
}
