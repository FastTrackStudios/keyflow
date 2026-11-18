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
use crdt::{CrdtDoc, DocFile, Persistence as _, TextBinding};
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
    /// The file the document's persistence writes to.
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

    /// The document, for the sync driver (`crdt::sync::SyncedDoc::new(id,
    /// chart.crdt())`, or the org's driver).
    #[must_use]
    pub fn crdt(&self) -> CrdtDoc {
        self.doc.clone()
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

    /// Writes everything this replica holds to the file, then folds it
    /// into one record.
    ///
    /// This device's own edits reach the file as they are made. What
    /// arrives by sync does not: Architect's `SyncedDoc` applies remote
    /// updates without persisting them (`CrdtDoc::apply_remote`, not
    /// `apply_remote_durable`), so a synced `.session` keeps another
    /// device's edits only once this has run. It writes only what the
    /// file lacks (`Persistence::write_snapshot` on the branch).
    ///
    /// # Errors
    ///
    /// The file could not be written.
    pub async fn save(&self) -> Result<()> {
        let snapshot = self
            .doc
            .loro()
            .export(crdt::loro::ExportMode::Snapshot)
            .map_err(|e| Error::Doc(e.to_string()))?;
        self.file
            .persistence(MAIN)
            .write_snapshot(self.id, &snapshot)
            .await
            .map_err(|e| Error::File(e.to_string()))?;
        self.doc
            .compact(self.id)
            .await
            .map_err(|e| Error::File(e.to_string()))
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
