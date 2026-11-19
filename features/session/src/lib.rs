//! Keyflow in a session: Keyflow as the first host of the session
//! primitive (FastTrackStudios/session).
//!
//! A chart can live two ways, and the chart view does not fork:
//!
//! - **Alone**: a document of the app's, read through the host
//!   (`read_document`), as Keyflow's chart extension does today. No session
//!   is needed for a chart.
//! - **In a session**: Keyflow's data in a `.session`, an Architect
//!   document (`session::doc::kind()`) that syncs like any other: through
//!   the org's driver or a share link, every device's edits merged. Keyflow
//!   keeps its chart as its own extension data there, the text entry
//!   [`CHART`] in namespace [`EXTENSION`]: not a `.kf` file path, since
//!   `.kf` text is becoming an input method rather than the file format.
//!   The session knows nothing about Keyflow; Keyflow chose the entry and
//!   its merge (per character, so two people typing both land).
//!
//! What a host does with it:
//!
//! - [`ChartSession`]: open or make a `.session` file, read and write the
//!   chart, bind it to an editor ([`ChartSession::binding`]: Architect's
//!   `TextBinding`, carets included), hand its document to the sync
//!   driver ([`ChartSession::crdt`]).
//! - [`Provider`]: the sessions this host has open, served as `fts.session`
//!   to extension views (`session::extension`). An app that hosts Keyflow
//!   registers its three actions; the chart view calls the service.
//! - [`ChartSource`]: what a chart pane's state names, and the service call
//!   that reads or edits it. The chart view's one change to be in a
//!   session: read through [`ChartSource`] instead of `read_document`.

use std::path::Path;

use crdt::file::{DiskStorage, MAIN};
use crdt::persistence::Persistence as _;
use crdt::{CrdtDoc, DocFile, TextBinding};
use session::doc::SessionDoc;
use session::extension::{call_json, response_of};
use session::service::{READ, Request, Response, Sessions, WRITE};
use session::{Entry, ExtensionId};
use uuid::Uuid;

/// Keyflow's namespace in a session: its Architect extension id.
pub const EXTENSION: &str = "keyflow";
/// The text entry the chart's source is kept in.
pub const CHART: &str = "chart";
/// A pane state that names a session's chart: `session:<document id>`.
pub const SESSION_PREFIX: &str = "session:";

/// What went wrong with a chart's session.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the session file: {0}")]
    File(String),
    #[error("the session document: {0}")]
    Doc(String),
    #[error("the session service: {0}")]
    Service(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A `.session` file holding a chart, open on this device.
pub struct ChartSession {
    id: Uuid,
    // Kept open: the file the document's persistence writes to.
    file: DocFile,
    doc: CrdtDoc,
    session: SessionDoc,
}

impl ChartSession {
    /// Makes a `.session` at `path` holding `chart` (Keyflow source).
    ///
    /// # Errors
    ///
    /// The file could not be made, or the document refused the chart.
    pub async fn create(path: &Path, chart: &str) -> Result<Self> {
        let storage = DiskStorage::open(path).map_err(|e| Error::File(e.to_string()))?;
        let file = DocFile::create(storage, &session::doc::kind())
            .await
            .map_err(|e| Error::File(e.to_string()))?;
        let this = Self::over(file).await?;
        this.set_chart(chart)?;
        this.save().await?;
        Ok(this)
    }

    /// Opens the `.session` at `path`.
    ///
    /// # Errors
    ///
    /// No such file, not a session, or from a newer build.
    pub async fn open(path: &Path) -> Result<Self> {
        let storage = DiskStorage::open(path).map_err(|e| Error::File(e.to_string()))?;
        let file = DocFile::open(storage, &session::doc::kind())
            .await
            .map_err(|e| Error::File(e.to_string()))?;
        Self::over(file).await
    }

    /// A `.session` at `path` for the session `id` made on another device:
    /// empty until it syncs, of the same lineage (`DocFile::adopt`).
    ///
    /// # Errors
    ///
    /// The file could not be made.
    pub async fn adopt(path: &Path, id: Uuid) -> Result<Self> {
        let storage = DiskStorage::open(path).map_err(|e| Error::File(e.to_string()))?;
        let file = DocFile::adopt(storage, &session::doc::kind(), id)
            .await
            .map_err(|e| Error::File(e.to_string()))?;
        Self::over(file).await
    }

    async fn over(file: DocFile) -> Result<Self> {
        let id = file
            .id()
            .await
            .ok_or_else(|| Error::File("the file names no document".into()))?;
        let doc = CrdtDoc::open(id, file.persistence(MAIN))
            .await
            .map_err(|e| Error::Doc(e.to_string()))?;
        let session = SessionDoc::from_doc(doc.loro().clone());
        Ok(Self {
            id,
            file,
            doc,
            session,
        })
    }

    /// The session's document id: what a pane state, a share link and the
    /// sync driver name it by.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// The session.
    #[must_use]
    pub const fn session(&self) -> &SessionDoc {
        &self.session
    }

    /// The document (for the org's driver; a single session syncs with
    /// [`Self::synced`]).
    #[must_use]
    pub fn crdt(&self) -> CrdtDoc {
        self.doc.clone()
    }

    /// Where the chart's bar 1 is in this session, seconds (the song's
    /// bar 1 after a count-in; `0` when the session sets none).
    #[must_use]
    pub fn chart_start(&self) -> f64 {
        self.session
            .bar_one()
            .map_or(0.0, |at| at.seconds(&self.session.tempo()))
    }

    /// The chart's source; empty when it has none yet.
    #[must_use]
    pub fn chart(&self) -> String {
        self.session
            .read_ext(&ExtensionId::from(EXTENSION))
            .text(CHART)
            .unwrap_or_default()
            .to_owned()
    }

    /// Makes the chart read `source`, as a diff: someone else's typing
    /// elsewhere in it survives.
    ///
    /// # Errors
    ///
    /// The document refused the edit.
    pub fn set_chart(&self, source: &str) -> Result<()> {
        let id = ExtensionId::from(EXTENSION);
        let ext = self
            .session
            .ext(&id)
            .map_err(|e| Error::Doc(e.to_string()))?;
        ext.set_text(CHART, source)
            .map_err(|e| Error::Doc(e.to_string()))?;
        self.session.doc().commit();
        Ok(())
    }

    /// The chart's text bound to an editor: the editor reports its edits
    /// (`TextBinding::on_local`), others' come back transformed
    /// (`sync_into`), and carets are stable cursors.
    ///
    /// # Errors
    ///
    /// The document refused declaring the chart's text.
    pub fn binding(&self) -> Result<TextBinding> {
        let text = self
            .session
            .ext(&ExtensionId::from(EXTENSION))
            .and_then(|ext| ext.text(CHART))
            .map_err(|e| Error::Doc(e.to_string()))?;
        Ok(TextBinding::new(self.session.doc(), text))
    }

    /// Calls `changed` whenever an edit made on another replica arrives
    /// (an import: a peer's typing, through [`Self::synced`] or a host's
    /// fan-out), never for this replica's own edits. A view re-reads the
    /// chart then, as a document chart does on `doc.saved`. Dropping the
    /// subscription stops it.
    #[must_use = "dropping the subscription stops it"]
    pub fn on_remote_change(
        &self,
        changed: impl Fn() + Send + Sync + 'static,
    ) -> crdt::loro::Subscription {
        self.doc
            .loro()
            .subscribe_root(std::sync::Arc::new(move |event| {
                if matches!(event.triggered_by, crdt::loro::EventTriggerKind::Import) {
                    changed();
                }
            }))
    }

    /// Writes everything the session holds to its file and folds the
    /// file's records into one.
    ///
    /// A local edit reaches the file through the document's own writer, a
    /// task of its own that appends each commit when it runs; folding
    /// alone (`CrdtDoc::compact` on a file branch folds only what is on
    /// disk) could miss the newest edits, and a file opened again before
    /// that task ran had lost them. So the whole state is written as a
    /// step first: the writer's later copies of the same commits change
    /// nothing (an update already in the document is a no-op).
    ///
    /// # Errors
    ///
    /// The file could not be written.
    pub async fn save(&self) -> Result<()> {
        let state = self
            .doc
            .loro()
            .export(crdt::loro::ExportMode::Snapshot)
            .map_err(|e| Error::Doc(e.to_string()))?;
        self.file
            .persistence(MAIN)
            .write_snapshot(self.id, &state)
            .await
            .map_err(|e| Error::File(e.to_string()))?;
        self.doc
            .compact(self.id)
            .await
            .map_err(|e| Error::File(e.to_string()))
    }

    /// The sync driver for this session: Architect's `SyncedDoc`, made
    /// `durable`, so what arrives from other devices is written to this
    /// file as it arrives, not only this device's own edits. Run it
    /// against a client of the org's lane or a share link
    /// (`SyncedDoc::run`), again on reconnect.
    #[must_use]
    pub fn synced(&self) -> crdt::sync::SyncedDoc {
        crdt::sync::SyncedDoc::new(self.id, self.doc.clone()).durable()
    }
}

/// The sessions this host serves to extension views as `fts.session`.
///
/// An app hosting Keyflow declares, in the manifest of the extension that
/// provides it (its prefix here `SESSION_`): `[provides] "fts.session" =
/// "0.1.0"` and actions `SESSION_READ` (`effect = "read_only"`),
/// `SESSION_WRITE` and `SESSION_STRUCTURE` (`effect = "mutating"`), each
/// `.action_in(id, |args, cx| …)` calling [`Provider::answer`] with its
/// method and `cx["caller"]`. Keyflow's chart extension declares
/// `[requires] "fts.session" = "^0.1"`.
#[derive(Debug, Clone, Default)]
pub struct Provider {
    sessions: Sessions,
}

impl Provider {
    /// Serves `chart`'s session.
    pub fn open(&self, chart: &ChartSession) {
        self.sessions
            .open(chart.id().to_string(), chart.session().clone());
    }

    /// Serves session `doc` (a session held other than as this crate's
    /// file: the node's copy of a vault document, through the host).
    pub fn open_doc(&self, id: Uuid, doc: SessionDoc) {
        self.sessions.open(id.to_string(), doc);
    }

    /// Whether it serves session `id`.
    #[must_use]
    pub fn holds(&self, id: Uuid) -> bool {
        self.sessions.get(&id.to_string()).is_some()
    }

    /// Stops serving session `id`.
    pub fn close(&self, id: Uuid) {
        self.sessions.close(&id.to_string());
    }

    /// The provider's action: one call to `method` with `args` (JSON), from
    /// extension `caller` (the action context's `caller`).
    ///
    /// # Errors
    ///
    /// The service refused or failed (`session::extension::answer`).
    pub fn answer(&self, method: &str, args: &str, caller: Option<&str>) -> Result<String> {
        session::extension::answer(&self.sessions, method, args, caller).map_err(Error::Service)
    }
}

/// Where a chart pane's chart is: its pane state, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartSource {
    /// A document of the app's, by path: the chart alone.
    Document(String),
    /// Keyflow's chart in session `doc`.
    Session { doc: String },
}

/// A service call to make through the host (`HostRequest::Service`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceCall {
    pub service: &'static str,
    pub method: &'static str,
    /// The arguments, as JSON.
    pub args: String,
}

impl ChartSource {
    /// The source a pane state names: `session:<id>` or a document path.
    #[must_use]
    pub fn of_pane_state(state: &str) -> Self {
        state.strip_prefix(SESSION_PREFIX).map_or_else(
            || Self::Document(state.to_owned()),
            |doc| Self::Session {
                doc: doc.to_owned(),
            },
        )
    }

    /// The pane state that names this source.
    #[must_use]
    pub fn pane_state(&self) -> String {
        match self {
            Self::Document(path) => path.clone(),
            Self::Session { doc } => format!("{SESSION_PREFIX}{doc}"),
        }
    }

    /// The call that reads the chart from a session; `None` for a document
    /// (read it with `read_document`, as before).
    ///
    /// # Errors
    ///
    /// The request could not be encoded.
    pub fn read_call(&self) -> Result<Option<ServiceCall>> {
        let Self::Session { doc } = self else {
            return Ok(None);
        };
        Ok(Some(ServiceCall {
            service: session::extension::SERVICE,
            method: READ,
            args: call_json(doc, Request::ReadData).map_err(Error::Service)?,
        }))
    }

    /// The call that reads the session's structure (its tempo map and
    /// where the song's bar 1 is); `None` for a document.
    ///
    /// # Errors
    ///
    /// The request could not be encoded.
    pub fn structure_call(&self) -> Result<Option<ServiceCall>> {
        let Self::Session { doc } = self else {
            return Ok(None);
        };
        Ok(Some(ServiceCall {
            service: session::extension::SERVICE,
            method: READ,
            args: call_json(doc, Request::ReadStructure).map_err(Error::Service)?,
        }))
    }

    /// Where the chart's bar 1 is in the session, seconds, from the reply
    /// to [`Self::structure_call`]: the song's bar 1 (`bar_one`, after a
    /// count-in) under the session's tempo map; `0` when it has none.
    ///
    /// # Errors
    ///
    /// The reply is not the session's structure.
    pub fn chart_start_of(reply: &str) -> Result<f64> {
        match response_of(reply).map_err(Error::Service)? {
            Response::Structure { tempo, bar_one, .. } => {
                let map = tempo
                    .to_tempo_map()
                    .map_err(|e| Error::Service(e.to_string()))?;
                Ok(bar_one.map_or(0.0, |at| at.seconds(&map)))
            }
            other => Err(Error::Service(format!("not the structure: {other:?}"))),
        }
    }

    /// [`Self::chart_start_of`] for a chart that may leave the session's
    /// lead-in out.
    ///
    /// The session's bar 1 can sit at the start of an intro (or a count-in)
    /// the chart does not write: the tracks play four bars, the chart's
    /// bar 1 is the verse. With `chart_has_lead_in` false and the session's
    /// bar 1 inside a region named as a lead-in (`Intro…`, `Count…`,
    /// `Click…`), the chart starts where the first region after it that is
    /// not one starts: the two then agree section by section. Otherwise as
    /// the session says.
    ///
    /// # Errors
    ///
    /// The reply is not the session's structure.
    pub fn chart_start_aligned(reply: &str, chart_has_lead_in: bool) -> Result<f64> {
        let Response::Structure {
            tempo,
            bar_one,
            regions,
            ..
        } = response_of(reply).map_err(Error::Service)?
        else {
            return Err(Error::Service("not the structure".to_owned()));
        };
        let map = tempo
            .to_tempo_map()
            .map_err(|e| Error::Service(e.to_string()))?;
        let start = bar_one.map_or(0.0, |at| at.seconds(&map));
        if chart_has_lead_in {
            return Ok(start);
        }
        let lead_in = |name: &str| {
            let n = name.trim().to_ascii_lowercase();
            n.starts_with("intro") || n.starts_with("count") || n.starts_with("click")
        };
        let spans: Vec<(f64, f64, bool)> = regions
            .values()
            .map(|r| (r.start.seconds(&map), r.end.seconds(&map), lead_in(&r.name)))
            .collect();
        const NEAR: f64 = 1e-3;
        let Some(&(_, lead_end, _)) = spans
            .iter()
            .find(|(from, to, lead)| *lead && *from <= start + NEAR && start < *to - NEAR)
        else {
            return Ok(start);
        };
        Ok(spans
            .iter()
            .filter(|(from, _, lead)| !*lead && *from >= lead_end - NEAR)
            .map(|(from, _, _)| *from)
            .fold(None, |best: Option<f64>, from| {
                Some(best.map_or(from, |b| b.min(from)))
            })
            .unwrap_or(start))
    }

    /// The call that replaces `delete` characters at `at` of the chart
    /// with `insert` (positions in Unicode scalars); `None` for a document.
    ///
    /// # Errors
    ///
    /// The request could not be encoded.
    pub fn splice_call(&self, at: u32, delete: u32, insert: &str) -> Result<Option<ServiceCall>> {
        let Self::Session { doc } = self else {
            return Ok(None);
        };
        Ok(Some(ServiceCall {
            service: session::extension::SERVICE,
            method: WRITE,
            args: call_json(
                doc,
                Request::TextSplice {
                    name: CHART.to_owned(),
                    at,
                    delete,
                    insert: insert.to_owned(),
                },
            )
            .map_err(Error::Service)?,
        }))
    }

    /// The call that makes the chart read `new` when it now reads `old`:
    /// the one splice between their common prefix and suffix (positions in
    /// Unicode scalars), so a whole-text write from an editor that
    /// re-serializes the chart (note input) merges per character with
    /// someone else's typing, as the host's diffed `DocEdit::Replace` does
    /// for a document. `None` for a document, or when nothing changed.
    ///
    /// # Errors
    ///
    /// The request could not be encoded.
    pub fn replace_call(&self, old: &str, new: &str) -> Result<Option<ServiceCall>> {
        let (at, delete, insert) = splice_between(old, new);
        if delete == 0 && insert.is_empty() {
            return Ok(None);
        }
        self.splice_call(at, delete, &insert)
    }

    /// The chart's source in the reply to [`Self::read_call`]; empty when
    /// the session has no chart yet.
    ///
    /// # Errors
    ///
    /// The reply is not the session's data.
    pub fn chart_of(reply: &str) -> Result<String> {
        match response_of(reply).map_err(Error::Service)? {
            Response::Data(data) => Ok(match data.get(CHART) {
                Some(Entry::Text(source)) => source.clone(),
                _ => String::new(),
            }),
            other => Err(Error::Service(format!("not the chart's data: {other:?}"))),
        }
    }
}

/// The one splice that turns `old` into `new`: where it starts, how many
/// characters it deletes, and what it inserts (Unicode scalars).
#[must_use]
pub fn splice_between(old: &str, new: &str) -> (u32, u32, String) {
    let old: Vec<char> = old.chars().collect();
    let new: Vec<char> = new.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()).saturating_sub(prefix);
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    let deleted = old.len().saturating_sub(prefix).saturating_sub(suffix);
    let inserted: String = new
        .get(prefix..new.len().saturating_sub(suffix))
        .unwrap_or_default()
        .iter()
        .collect();
    (
        u32::try_from(prefix).unwrap_or(u32::MAX),
        u32::try_from(deleted).unwrap_or(u32::MAX),
        inserted,
    )
}
