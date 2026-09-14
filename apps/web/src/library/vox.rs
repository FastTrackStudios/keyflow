//! The library calls, spoken as vox.
//!
//! **This is the transport half, and the only file in the site that
//! dials a socket.** [`super`] asks for a song list, a song's charts or
//! a chart and gets [`super::SongList`], [`super::ChartEntry`] or
//! [`super::StoredChart`] back; the screens above that ask [`super`].
//! Nothing above this file knows there is a WebSocket, a handshake or a
//! generated client.
//!
//! # What it talks to
//!
//! Task's **org lane**: `wss://task.fasttrackstudio.app/org/<slug>/vox`,
//! one connection per org, with the signed-in person's token presented
//! once at the handshake. On that lane two generated clients do all the
//! work:
//!
//! * `resources_proto::ResourcesServiceClient` — songs
//!   (`list_songs`) and charts (`list_charts`, `chart`, `upsert_chart`,
//!   `delete_chart`). A chart belongs to a song through its `song`
//!   field, a `song:<slug>` reference; a chart with none is
//!   *unattached*, which is what the editor's Save produces.
//! * `collection_proto::CollectionServiceClient` — song lists. A song
//!   list is a Task collection of kind [`SONGLIST_KIND`] whose items
//!   are `song:<slug>` references, in order. Task promises the ordering
//!   and the membership and nothing about what a song list *means*;
//!   the meaning is here.
//!
//! This is the same wire Task's own web app, CLI and desktop client
//! use, which is the point: a song the CLI adds is on the shelf here
//! without any translation in between, and the errors are typed rather
//! than prose to be pattern-matched.
//!
//! # How the identity rides
//!
//! A browser cannot set headers on a WebSocket upgrade, so the token
//! goes on the **subprotocol list** as `vox.bearer.<token>` beside
//! `vox.v1` — the channel Task's server reads it from. It never rides a
//! query parameter, which every proxy and access log on the path would
//! keep. Presented once, at open: every call on the connection is then
//! that person's.
//!
//! # What is pure, and why
//!
//! Everything except the dial. The URL, the subprotocol list, the error
//! mapping and every conversion from a wire type to a library type are
//! ordinary functions `just test` runs on the host, exactly the way
//! [`crate::oidc`] keeps PKCE testable on a target with no browser in
//! it. The browser half — [`dial`] — is thin enough to read in one
//! screen, and is a port of the dial Task's own web app uses.

use std::fmt::Display;

use architect::vox::VoxError;
use collection_proto::Collection;
use links_proto::{NodeKind, NodeRef};
use resources_proto::{ChartDoc, ChartSummary, SongSummary};

use super::{ChartEntry, Draft, LibraryError, SaveOutcome, SongEntry, SongList, StoredChart};

/// The collection kind a song list is. Lower-case, because
/// `CollectionKind` folds case and this is the folded spelling; the
/// CLI's `collection create --kind songlist` lands on exactly this.
pub const SONGLIST_KIND: &str = "songlist";

/// The vox subprotocol every dial offers.
const VOX_SUBPROTOCOL: &str = "vox.v1";

/// Prefix of the subprotocol carrying the session token.
const VOX_BEARER_SUBPROTOCOL_PREFIX: &str = "vox.bearer.";

// ── Where, and as whom ───────────────────────────────────────────────

/// The org lane's WebSocket URL for `org`, from the HTTP base the rest
/// of the site uses. `https://` becomes `wss://`, `http://` becomes
/// `ws://`, so a developer pointing the site at a local Task
/// (`KEYFLOW_TASK_URL=http://localhost:18080`) dials it in the clear.
#[must_use]
pub fn org_vox_url(base: &str, org: &str) -> String {
    let base = base.trim_end_matches('/');
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_owned()
    };
    format!("{ws}/org/{org}/vox")
}

/// The subprotocol list a dial offers: always [`VOX_SUBPROTOCOL`], plus
/// `vox.bearer.<token>` for the signed-in person.
///
/// The issuer's tokens are JWT-shaped — base64url segments joined by
/// `.` — which fits the RFC 7230 token charset a subprotocol value must
/// use. A token containing anything outside that charset is dropped
/// rather than sent as a malformed header that would fail the whole
/// handshake; the server then sees an anonymous dial and says so.
#[must_use]
pub fn subprotocols(bearer: &str) -> Vec<String> {
    let mut protos = vec![VOX_SUBPROTOCOL.to_owned()];
    let fits = !bearer.is_empty()
        && bearer
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'));
    if fits {
        protos.push(format!("{VOX_BEARER_SUBPROTOCOL_PREFIX}{bearer}"));
    }
    protos
}

// ── Reading answers ──────────────────────────────────────────────────

/// One [`LibraryError`] for every way a call can fail.
///
/// The service's own error (`NotFound`, `BadRequest`) is a refusal in
/// the server's words. Task's permission gate answers a call the token
/// is not allowed to make as `InvalidPayload("permission denied: …")`,
/// and that is a refusal too — the sentence names the org and the
/// call, which is what the person needs to read. A method the server
/// does not have is an older deployment, and the transport failing is
/// the transport failing.
pub fn error_from<E: Display>(error: VoxError<E>) -> LibraryError {
    match error {
        VoxError::User(inner) => LibraryError::Refused(inner.to_string()),
        VoxError::UnknownMethod => LibraryError::Unsupported,
        VoxError::InvalidPayload(why) => {
            if why.contains("not a member") || why.contains("permission denied") {
                LibraryError::Refused(why)
            } else {
                LibraryError::Malformed(why)
            }
        }
        VoxError::ConnectionClosed
        | VoxError::ConnectionShutdown
        | VoxError::SendFailed
        | VoxError::TimedOut => {
            LibraryError::Transport("the connection to your library dropped".to_owned())
        }
        VoxError::Cancelled | VoxError::Indeterminate => {
            LibraryError::Transport("the call did not complete".to_owned())
        }
    }
}

fn blank_to_none(s: String) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[must_use]
pub fn entry_from(chart: ChartSummary) -> ChartEntry {
    ChartEntry {
        title: if chart.title.trim().is_empty() {
            chart.slug.clone()
        } else {
            chart.title
        },
        slug: chart.slug,
        key: blank_to_none(chart.key),
        notation: blank_to_none(chart.notation),
        sections: chart.sections,
        song: song_slug(&chart.song),
        arrangement: blank_to_none(chart.arrangement),
        is_default: chart.is_default,
        updated_at: blank_to_none(chart.updated_at),
    }
}

#[must_use]
pub fn chart_from(chart: ChartDoc) -> StoredChart {
    StoredChart {
        title: if chart.title.trim().is_empty() {
            chart.slug.clone()
        } else {
            chart.title
        },
        slug: chart.slug,
        source: chart.source,
        key: blank_to_none(chart.key),
        notation: blank_to_none(chart.notation),
        sections: chart.sections,
        song: song_slug(&chart.song),
    }
}

#[must_use]
pub fn song_from(song: SongSummary) -> SongEntry {
    SongEntry {
        title: if song.title.trim().is_empty() {
            song.slug.clone()
        } else {
            song.title
        },
        slug: song.slug,
        writers: song.writers,
        key: blank_to_none(song.key),
        tags: song.tags,
    }
}

/// A collection, read as a song list: its `song:<slug>` items in the
/// order the collection keeps them. Items of any other kind are not
/// songs and are left out rather than shown as ones.
#[must_use]
pub fn songlist_from(collection: Collection) -> SongList {
    let mut collection = collection;
    collection.sort_items();
    SongList {
        id: collection.id,
        title: collection.title,
        songs: collection
            .items
            .into_iter()
            .filter(|item| item.node.kind == NodeKind::Song && item.node.domain.is_empty())
            .map(|item| item.node.id)
            .collect(),
    }
}

/// The slug out of a chart's `song` field, which the server stores as
/// a `song:<slug>` token. Empty is unattached, and stays `None`.
fn song_slug(token: &str) -> Option<String> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    match NodeRef::parse(token) {
        Some(node) if node.kind == NodeKind::Song => Some(node.id),
        Some(_) => None,
        // A bare slug is what the server reads as the local song.
        None if !token.contains(':') => Some(token.to_owned()),
        None => None,
    }
}

/// The document a draft goes up as.
///
/// Optional fields go up empty rather than absent — that is what the
/// wire type is — and the server reads empty as unset. `notation` is
/// never filled in and `updated_at` is stamped here, both for the
/// reasons on [`super::draft_from_source`]. An empty `slug` is the
/// derive-from-title behaviour that makes re-saving a chart a new
/// *version* rather than a second chart called the same thing.
#[must_use]
pub fn chart_doc_from(draft: &Draft, updated_at: String) -> ChartDoc {
    ChartDoc {
        slug: draft
            .slug
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned(),
        title: draft.title.clone(),
        source: draft.source.clone(),
        key: draft
            .key
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned(),
        notation: String::new(),
        sections: draft.sections.clone(),
        song: draft
            .song
            .as_deref()
            .map(|s| NodeRef::song(s.trim()).to_token())
            .unwrap_or_default(),
        arrangement: String::new(),
        is_default: false,
        updated_at,
    }
}

// ── The dial ─────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
mod dial {
    //! One vox connection per org, opened on first use and kept for the
    //! life of the page. A port of the dial Task's own web app makes.

    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::super::task_base_url;
    use super::{LibraryError, org_vox_url, subprotocols};

    /// The raw lane a connection hands back, from which typed clients
    /// are made. Deliberately service-less: it is the connection, not a
    /// client of anything.
    #[derive(Clone)]
    struct RootLane {
        caller: vox_core::Caller,
        _connection: Option<vox_core::ConnectionHandle>,
    }

    impl vox_core::FromVoxLane for RootLane {
        const SERVICE_NAME: &'static str = "Noop";

        fn from_vox_lane(
            caller: vox_core::Caller,
            connection: Option<vox_core::ConnectionHandle>,
        ) -> Self {
            Self {
                caller,
                _connection: connection,
            }
        }
    }

    thread_local! {
        /// Live connections by `(org, token)`. Keyed on the token too,
        /// so signing out and back in as someone else dials afresh
        /// rather than reusing the previous person's lane.
        static ROOTS: RefCell<HashMap<(String, String), RootLane>> = RefCell::new(HashMap::new());
    }

    /// The caller for `org`, as the signed-in person.
    pub async fn caller(org: &str) -> Result<vox_core::Caller, LibraryError> {
        let token = crate::auth::access_token()
            .await
            .ok_or(LibraryError::SignedOut)?;
        let key = (org.to_owned(), token.clone());
        let cached = ROOTS.with(|roots| {
            roots
                .borrow()
                .get(&key)
                .filter(|root| root.caller.is_connected())
                .map(|root| root.caller.clone())
        });
        if let Some(caller) = cached {
            return Ok(caller);
        }
        let url = org_vox_url(&task_base_url(), org);
        let link = dial_ws(&url, &token).await?;
        let root = vox_core::initiator_on(link)
            .establish::<RootLane>()
            .await
            .map_err(|e| LibraryError::Transport(format!("establish `{url}`: {e:?}")))?;
        let caller = root.caller.clone();
        ROOTS.with(|roots| {
            roots.borrow_mut().insert(key, root);
        });
        Ok(caller)
    }

    async fn dial_ws(url: &str, bearer: &str) -> Result<vox_websocket::WsLink, LibraryError> {
        use std::rc::Rc;

        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;

        struct Dial {
            ws: web_sys::WebSocket,
            _onopen: Closure<dyn FnMut()>,
            _onerror: Closure<dyn FnMut(web_sys::Event)>,
            _onclose: Closure<dyn FnMut(web_sys::CloseEvent)>,
            keep_open: bool,
        }
        impl Drop for Dial {
            fn drop(&mut self) {
                self.ws.set_onopen(None);
                self.ws.set_onerror(None);
                self.ws.set_onclose(None);
                if !self.keep_open {
                    let _ = self.ws.close();
                }
            }
        }

        let protocols = js_sys::Array::new();
        for proto in subprotocols(bearer) {
            protocols.push(&wasm_bindgen::JsValue::from_str(&proto));
        }
        let ws = web_sys::WebSocket::new_with_str_sequence(url, &protocols)
            .map_err(|e| LibraryError::Transport(format!("WebSocket::new `{url}`: {e:?}")))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let (tx, rx) = futures_channel::oneshot::channel::<Result<(), String>>();
        let tx = Rc::new(RefCell::new(Some(tx)));

        let tx_open = Rc::clone(&tx);
        let onopen = Closure::wrap(Box::new(move || {
            if let Some(tx) = tx_open.borrow_mut().take() {
                let _ = tx.send(Ok(()));
            }
        }) as Box<dyn FnMut()>);
        let tx_error = Rc::clone(&tx);
        let err_url = url.to_owned();
        let onerror = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Some(tx) = tx_error.borrow_mut().take() {
                let _ = tx.send(Err(format!("could not open `{err_url}`")));
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        let tx_close = Rc::clone(&tx);
        let close_url = url.to_owned();
        let onclose = Closure::wrap(Box::new(move |e: web_sys::CloseEvent| {
            if let Some(tx) = tx_close.borrow_mut().take() {
                let _ = tx.send(Err(format!(
                    "`{close_url}` closed during open (code {})",
                    e.code()
                )));
            }
        }) as Box<dyn FnMut(web_sys::CloseEvent)>);

        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

        let mut dial = Dial {
            ws,
            _onopen: onopen,
            _onerror: onerror,
            _onclose: onclose,
            keep_open: false,
        };

        rx.await
            .map_err(|_| LibraryError::Transport("dial cancelled".to_owned()))?
            .map_err(LibraryError::Transport)?;

        dial.keep_open = true;
        let ws = dial.ws.clone();
        drop(dial);
        Ok(vox_websocket::WsLink::new(ws))
    }
}

#[cfg(target_arch = "wasm32")]
fn resources(caller: vox_core::Caller) -> resources_proto::ResourcesServiceClient {
    use vox_core::FromVoxLane as _;
    resources_proto::ResourcesServiceClient::from_vox_lane(caller, None)
}

#[cfg(target_arch = "wasm32")]
fn collections(caller: vox_core::Caller) -> collection_proto::CollectionServiceClient {
    use vox_core::FromVoxLane as _;
    collection_proto::CollectionServiceClient::from_vox_lane(caller, None)
}

// ── The operations ───────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
pub async fn list_songs(org: &str) -> Result<Vec<SongEntry>, LibraryError> {
    let songs = resources(dial::caller(org).await?)
        .list_songs()
        .await
        .map_err(error_from)?;
    Ok(songs.into_iter().map(song_from).collect())
}

#[cfg(target_arch = "wasm32")]
pub async fn list_songlists(org: &str) -> Result<Vec<SongList>, LibraryError> {
    let lists = collections(dial::caller(org).await?)
        .list(
            org.to_owned(),
            Some(collection_proto::CollectionKind::new(SONGLIST_KIND)),
        )
        .await
        .map_err(error_from)?;
    Ok(lists.into_iter().map(songlist_from).collect())
}

/// Every chart in `org` when `song` is `None`, or the charts of one
/// song. The server reads an empty filter as "no filter", which is how
/// one call serves both the whole shelf and a song's arrangements.
#[cfg(target_arch = "wasm32")]
pub async fn list_charts(org: &str, song: Option<&str>) -> Result<Vec<ChartEntry>, LibraryError> {
    let charts = resources(dial::caller(org).await?)
        .list_charts(song.unwrap_or_default().to_owned())
        .await
        .map_err(error_from)?;
    Ok(charts.into_iter().map(entry_from).collect())
}

#[cfg(target_arch = "wasm32")]
pub async fn read_chart(org: &str, slug: &str) -> Result<StoredChart, LibraryError> {
    let chart = resources(dial::caller(org).await?)
        .chart(slug.to_owned())
        .await
        .map_err(error_from)?;
    Ok(chart_from(chart))
}

#[cfg(target_arch = "wasm32")]
pub async fn save_chart(draft: &Draft) -> Result<SaveOutcome, LibraryError> {
    let org = draft.org.as_deref().ok_or(LibraryError::NoOrg)?;
    let stamp = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    let saved = resources(dial::caller(org).await?)
        .upsert_chart(chart_doc_from(draft, stamp))
        .await
        .map_err(error_from)?;
    Ok(SaveOutcome {
        slug: saved.slug,
        rel_path: blank_to_none(saved.rel_path),
        created: saved.created,
    })
}

#[cfg(target_arch = "wasm32")]
pub async fn delete_chart(org: &str, slug: &str) -> Result<(), LibraryError> {
    // Idempotent on the server: deleting twice answers `false`, and
    // "it is gone" is the outcome either way.
    resources(dial::caller(org).await?)
        .delete_chart(slug.to_owned())
        .await
        .map(|_deleted| ())
        .map_err(error_from)
}

// The host build has no socket to dial (see the module docs). It keeps
// the same signatures so the seam and the screens type-check there, and
// every call answers the way [`super::http`] does on the host.
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::unused_async)]
mod host {
    use super::{ChartEntry, Draft, LibraryError, SaveOutcome, SongEntry, SongList, StoredChart};

    fn offline() -> LibraryError {
        LibraryError::Transport("not in a browser".to_owned())
    }

    pub async fn list_songs(_org: &str) -> Result<Vec<SongEntry>, LibraryError> {
        Err(offline())
    }
    pub async fn list_songlists(_org: &str) -> Result<Vec<SongList>, LibraryError> {
        Err(offline())
    }
    pub async fn list_charts(
        _org: &str,
        _song: Option<&str>,
    ) -> Result<Vec<ChartEntry>, LibraryError> {
        Err(offline())
    }
    pub async fn read_chart(_org: &str, _slug: &str) -> Result<StoredChart, LibraryError> {
        Err(offline())
    }
    pub async fn save_chart(draft: &Draft) -> Result<SaveOutcome, LibraryError> {
        draft.org.as_deref().ok_or(LibraryError::NoOrg)?;
        Err(offline())
    }
    pub async fn delete_chart(_org: &str, _slug: &str) -> Result<(), LibraryError> {
        Err(offline())
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use host::{delete_chart, list_charts, list_songlists, list_songs, read_chart, save_chart};

#[cfg(test)]
mod tests {
    use collection_proto::{CollectionItem, CollectionKind};
    use resources_proto::ResourcesError;

    use super::*;

    #[test]
    fn the_org_lane_is_dialled_over_the_same_host_as_the_site_uses() {
        assert_eq!(
            org_vox_url("https://task.fasttrackstudio.app", "rockstars-of-tomorrow"),
            "wss://task.fasttrackstudio.app/org/rockstars-of-tomorrow/vox"
        );
        assert_eq!(
            org_vox_url("http://localhost:18080/", "acme"),
            "ws://localhost:18080/org/acme/vox"
        );
    }

    #[test]
    fn the_token_rides_the_subprotocol_list_beside_vox() {
        assert_eq!(
            subprotocols("eyJhb.GciOi-J_~"),
            ["vox.v1", "vox.bearer.eyJhb.GciOi-J_~"]
        );
    }

    /// A token the header charset cannot carry is dropped, not sent
    /// malformed: an anonymous dial gets a readable refusal, a broken
    /// handshake gets nothing.
    #[test]
    fn a_token_outside_the_header_charset_is_not_offered() {
        assert_eq!(subprotocols(""), ["vox.v1"]);
        assert_eq!(subprotocols("has space"), ["vox.v1"]);
        assert_eq!(subprotocols("has/slash"), ["vox.v1"]);
    }

    #[test]
    fn the_servers_own_refusal_is_a_refusal_in_its_words() {
        let error: VoxError<ResourcesError> =
            VoxError::User(Box::new(ResourcesError::NotFound("chart nope".into())));
        assert_eq!(
            error_from(error),
            LibraryError::Refused("not found: chart nope".to_owned())
        );
    }

    #[test]
    fn a_permission_refusal_is_a_refusal_not_a_malformed_answer() {
        let error: VoxError<ResourcesError> = VoxError::InvalidPayload(
            "permission denied: anonymous is not a member (resources/list_songs)".into(),
        );
        assert!(matches!(error_from(error), LibraryError::Refused(_)));
        let junk: VoxError<ResourcesError> = VoxError::InvalidPayload("bad frame".into());
        assert!(matches!(error_from(junk), LibraryError::Malformed(_)));
    }

    #[test]
    fn a_server_without_the_method_is_unsupported_and_a_dead_socket_is_transport() {
        let missing: VoxError<ResourcesError> = VoxError::UnknownMethod;
        assert_eq!(error_from(missing), LibraryError::Unsupported);
        let dead: VoxError<ResourcesError> = VoxError::ConnectionClosed;
        assert!(matches!(error_from(dead), LibraryError::Transport(_)));
    }

    #[test]
    fn a_listing_row_keeps_what_the_shelf_shows_and_drops_blanks() {
        let entry = entry_from(ChartSummary {
            slug: "highway-to-hell".into(),
            title: "Highway to Hell".into(),
            key: "A".into(),
            notation: String::new(),
            sections: vec!["in".into(), "vs-1".into()],
            song: "song:highway-to-hell".into(),
            arrangement: String::new(),
            is_default: true,
            rel_path: "Charts/highway-to-hell.md".into(),
            updated_at: "2026-09-14T10:00:00Z".into(),
        });
        assert_eq!(entry.key.as_deref(), Some("A"));
        assert_eq!(entry.notation, None);
        assert_eq!(entry.song.as_deref(), Some("highway-to-hell"));
        assert!(entry.is_default);
        assert_eq!(entry.sections, ["in", "vs-1"]);
    }

    #[test]
    fn an_untitled_row_still_shows_something() {
        let entry = entry_from(ChartSummary {
            slug: "untitled-chart".into(),
            ..ChartSummary::default()
        });
        assert_eq!(entry.title, "untitled-chart");
        assert_eq!(entry.song, None);
    }

    #[test]
    fn a_chart_reads_back_byte_identical() {
        let source = "Build My Life - Housefires\n4/4 #G\n\nVS 1: | 1 4 | 5 6m |\n";
        let chart = chart_from(ChartDoc {
            slug: "build-my-life".into(),
            title: "Build My Life".into(),
            source: source.into(),
            song: "build-my-life".into(),
            ..ChartDoc::default()
        });
        assert_eq!(chart.source, source);
        assert_eq!(chart.song.as_deref(), Some("build-my-life"));
    }

    #[test]
    fn a_song_list_is_its_song_items_in_collection_order() {
        let mut list = Collection::new(
            "rockstars-of-tomorrow",
            "Adult Jam",
            CollectionKind::new(SONGLIST_KIND),
        );
        list.id = "adult-jam".into();
        list.items = vec![
            CollectionItem::new(NodeRef::song("wonderwall"), "b"),
            CollectionItem::new(NodeRef::song("basket-case"), "a"),
            CollectionItem::new(NodeRef::new(NodeKind::Note, "Journal/2026.md"), "c"),
        ];
        let songlist = songlist_from(list);
        assert_eq!(songlist.title, "Adult Jam");
        assert_eq!(songlist.songs, ["basket-case", "wonderwall"]);
    }

    /// The document a draft becomes: the text verbatim, `notation`
    /// never claimed, `slug` empty so the server derives it, and the
    /// song it is attached to as the reference the server stores.
    #[test]
    fn a_draft_goes_up_verbatim_and_makes_no_claim_about_notation() {
        let source = "Café — 4/4 #G\n\nVS 1: | 1 4 |\n\t| 5 6m |\n\n";
        let draft = Draft {
            title: "Café".to_owned(),
            source: source.to_owned(),
            key: Some(" G ".to_owned()),
            sections: vec!["vs-1".to_owned()],
            slug: None,
            org: Some("acme".to_owned()),
            song: Some("cafe".to_owned()),
        };
        let doc = chart_doc_from(&draft, "2026-09-14T10:00:00Z".to_owned());
        assert_eq!(doc.source, source);
        assert_eq!(doc.slug, "");
        assert_eq!(doc.key, "G");
        assert_eq!(doc.notation, "");
        assert_eq!(doc.song, "song:cafe");
        assert_eq!(doc.sections, ["vs-1"]);
        assert_eq!(doc.updated_at, "2026-09-14T10:00:00Z");
        assert!(!doc.is_default);
    }

    #[test]
    fn an_unattached_draft_names_no_song() {
        let draft = Draft {
            title: "T".to_owned(),
            source: "T\n".to_owned(),
            key: None,
            sections: Vec::new(),
            slug: Some("t".to_owned()),
            org: None,
            song: None,
        };
        let doc = chart_doc_from(&draft, String::new());
        assert_eq!(doc.song, "");
        assert_eq!(doc.slug, "t");
        assert_eq!(doc.key, "");
    }
}
