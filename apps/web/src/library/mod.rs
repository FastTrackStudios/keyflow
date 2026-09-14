//! The library — the songs, song lists and charts kept in a
//! FastTrackStudio workspace.
//!
//! The editor's persistence story is the URL ([`crate::chart_url`]): a
//! chart deflates into a `/c/:data` path, so sharing one is sharing a
//! link and nothing needs an account. That is deliberately enough to be
//! useful, and none of it changes here. What it cannot do is give
//! someone their charts back on a different machine, or show a band its
//! repertoire. That is what an account is for — and only that. **The
//! editor stays fully usable signed out**; this module adds a place to
//! keep charts and a way to see a workspace's library, never a gate in
//! front of writing one.
//!
//! # Where the library actually lives
//!
//! In Task (`task.fasttrackstudio.app`), the vault product, under each
//! workspace ("org") the signed-in person belongs to. Keyflow does not
//! run a server and is not about to start: Task already is one — files
//! on disk, versioned, reachable from the desktop app, the CLI and the
//! phone — and a chart is a small text document, which is precisely
//! what it stores. Keyflow reads and writes someone's existing vault
//! rather than inventing a second account with a second copy of their
//! work in it.
//!
//! # What is on the shelf
//!
//! Three things, and the words are Task's (ADR 0003/0004):
//!
//! * A **song** — a title, who wrote it, its usual key. The thing a
//!   band means when it says "we do Wonderwall".
//! * A **chart** — Keyflow source, the document this editor edits. A
//!   chart is usually an *arrangement of a song* (`song:<slug>`), and a
//!   song's charts have one **default**: the one to open when somebody
//!   asks for "the chart". A chart with no song is *unattached*, which
//!   is what the editor's Save makes and is a perfectly ordinary state.
//! * A **song list** — an ordered list of songs: a repertoire, a set,
//!   "Adult Jam". Task keeps it as a collection of `song:` references
//!   and knows nothing about what it is for.
//!
//! # This module is a seam
//!
//! The operations, in Keyflow's own words: [`list_songlists`],
//! [`list_songs`], [`list_charts`], [`read_chart`], [`save_chart`],
//! [`delete_chart`], and [`org_target`] for the workspace they act in.
//! Callers speak songs, charts and orgs; nothing above this file
//! mentions a wire, and nothing above it should ever start to.
//!
//! ```text
//! routes::library      the screens — lists, songs, charts, states
//!        │
//!   library (here)     the operations, in domain terms
//!        ├── vox       ← the transport: Task's org lane, typed
//!        ├── discovery ← orgs. HTTP either way.
//!        └── http      ← window.fetch, for discovery
//! ```
//!
//! # The transport
//!
//! **vox** — architect's RPC layer, the same one Task's own web app,
//! CLI and desktop client use — reaching `resources-proto`'s song and
//! chart methods and `collection-proto`'s song lists directly, with
//! generated clients and typed errors. That is what ADR 0003 (the
//! cross-app backend: Task as the shared store for Session, Signal,
//! Ignition and Keyflow) always intended, and [`vox`] says how.
//!
//! It was MCP JSON-RPC over `fetch` for a while, because this repo and
//! Task were pinned to different architect tags and two copies of the
//! same crate from two git sources do not link into one wasm binary.
//! The pins are one tag now (`Cargo.toml`, the architect block), the
//! proto crates come from Task by git, and the stepping stone is gone:
//! four hand-parsed tool calls became six typed ones, and the screens
//! above did not change shape — which is what the seam was for.
//! [`discovery`] did not change either: a well-known document fetched
//! before any session exists is HTTP under every transport.
//!
//! # What is pure, and why
//!
//! Everything except the dial in [`vox`] and the `fetch` in [`http`].
//! Deciding which org to write to, reading a chart's title and key out
//! of the chart, turning a wire row into a shelf row — all of it is
//! ordinary functions that `just test` runs on the host, exactly the
//! way [`crate::oidc`] keeps PKCE testable on a target with no browser
//! in it.
//!
//! # Degrading when the server cannot do something
//!
//! An older deployment without a method answers `UnknownMethod`, and
//! that part of the library is then simply not available — which is a
//! sentence on a page, not a panic and not a blank screen.
//! [`LibraryError::Unsupported`] is that case, kept separate from every
//! other failure precisely so the UI can say the true thing about it.

// The transport half is browser-only, and the host build of the site
// (`cargo check --workspace`, which is not `just web-check`) compiles
// these files with the pure half alone. See CLAUDE.md.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub mod discovery;
mod http;
mod vox;

pub use discovery::{ORG_KEY, Org, OrgTarget, choose_org, my_orgs, org_target, remember_org};

/// Where the vault server lives.
///
/// Overridable at build time, the same way [`crate::auth::auth_base_url`]
/// is, so a developer can point the site at a local Task
/// (`KEYFLOW_TASK_URL=http://localhost:18080 dx serve`) without editing
/// code.
#[must_use]
pub fn task_base_url() -> String {
    option_env!("KEYFLOW_TASK_URL")
        .unwrap_or("https://task.fasttrackstudio.app")
        .trim_end_matches('/')
        .to_owned()
}

// ── Failures ─────────────────────────────────────────────────────────

/// Why a library operation did not happen.
///
/// Deliberately transport-free: not one variant names a socket, a
/// handshake or a generated client, so the same set survived the move
/// from MCP to vox and the screens kept matching on the same cases.
/// Every variant is a different sentence on the screen, which is the
/// only reason there is more than one of them. In particular
/// [`Self::Unsupported`] is not folded into [`Self::Refused`]: "your
/// charts are not available on this server yet" and "the server said
/// no" ask the person to do completely different things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    /// Nobody is signed in, so there is no token to present. Not an
    /// error the person needs to see as one — it is the signed-out
    /// state, and the UI shows an invitation rather than a failure.
    SignedOut,
    /// Signed in, but the token reaches no org that can hold charts.
    /// Task auto-provisions a personal org on first ask, so this is
    /// rare; when it happens it is said plainly rather than retried.
    NoOrg,
    /// The server does not offer this part of the library. An older
    /// deployment, or one with the vault plugin turned off.
    Unsupported,
    /// The server understood and declined, and said why.
    Refused(String),
    /// The server answered with something this client cannot read.
    Malformed(String),
    /// The request never got an answer — offline, DNS, a dead host, a
    /// socket that would not open.
    Transport(String),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignedOut => f.write_str("sign in to keep charts in your library."),
            Self::NoOrg => f.write_str(
                "your account has no workspace to keep charts in yet — open Task once and it \
                 will make you one.",
            ),
            Self::Unsupported => f.write_str(
                "the chart library is not available on this server yet. Your chart is safe in \
                 the link.",
            ),
            Self::Refused(why) => write!(f, "the server would not do that: {why}"),
            Self::Malformed(why) => write!(f, "the server's answer could not be read: {why}"),
            Self::Transport(why) => write!(f, "could not reach your library: {why}"),
        }
    }
}

impl std::error::Error for LibraryError {}

// ── What is on the shelf ─────────────────────────────────────────────

/// A song in a workspace's library.
///
/// The thing a set list names. It carries no chart: a song's charts are
/// asked for when someone opens it ([`list_charts`] with the song), and
/// a library of two hundred songs is not two hundred documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongEntry {
    pub slug: String,
    pub title: String,
    /// Who wrote it, as the server has it. For a covers repertoire this
    /// is the artist; empty is ordinary.
    pub writers: Vec<String>,
    /// The song's usual key; an arrangement may differ and says so on
    /// its own chart.
    pub key: Option<String>,
    pub tags: Vec<String>,
}

/// An ordered list of songs — a repertoire, a set, "Adult Jam".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongList {
    pub id: String,
    pub title: String,
    /// Song slugs, in the list's order. Slugs rather than
    /// [`SongEntry`]s because the list and the library are two calls,
    /// and a slug in a list whose song has since gone is shown as the
    /// slug rather than dropped — a missing song is information.
    pub songs: Vec<String>,
}

/// A row in a chart listing.
///
/// Named for what it is rather than `ChartSummary`, which is taken:
/// [`keyflow::summary::ChartSummary`] is the language's own projection
/// of a *parsed* chart, derived locally. This is what the server holds
/// about a chart, and it deliberately carries no `source` — a listing of
/// two hundred charts is not two hundred documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartEntry {
    pub slug: String,
    pub title: String,
    pub key: Option<String>,
    pub notation: Option<String>,
    /// The shape of the song — `["in", "vs-1", "ch"]`. Enough to tell
    /// two charts with similar titles apart at a glance, which is what
    /// a shelf is for.
    pub sections: Vec<String>,
    /// The song this chart arranges, by slug; `None` is unattached.
    pub song: Option<String>,
    /// Which reading of the song this is, in a person's own words —
    /// `acoustic in G`. Empty for the first chart of most songs.
    pub arrangement: Option<String>,
    /// The song's main chart: the one to open when nobody names an
    /// arrangement. Never true for an unattached chart.
    pub is_default: bool,
    /// As the server spelled it. Not parsed into a date type: it is
    /// shown, not computed with, and inventing a chrono dependency to
    /// reformat a string nobody sorts by is not worth it.
    pub updated_at: Option<String>,
}

/// A chart read back in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChart {
    pub slug: String,
    pub title: String,
    /// The chart text, byte-identical to what was saved. This is the
    /// whole contract of the library: what comes back opens in the
    /// editor as the document that went in.
    pub source: String,
    pub key: Option<String>,
    pub notation: Option<String>,
    pub sections: Vec<String>,
    /// The song it arranges, by slug; `None` is unattached.
    pub song: Option<String>,
}

/// What a save answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    pub slug: String,
    /// Where it landed in the vault, for a person who also uses Task.
    pub rel_path: Option<String>,
    /// New, rather than a new version of one that was there.
    pub created: bool,
}

/// A chart on its way to be kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub title: String,
    pub source: String,
    pub key: Option<String>,
    pub sections: Vec<String>,
    /// Absent means "derive it from the title", which is what makes
    /// saving the same chart twice a second version rather than a
    /// second chart.
    pub slug: Option<String>,
    pub org: Option<String>,
    /// The song to attach it to, by slug. Absent is unattached — what
    /// the editor's Save makes, since a person saves a chart before
    /// they have said what song it is.
    pub song: Option<String>,
}

/// Title, key and sections for a chart, read out of the chart itself.
///
/// The language already knows how to describe a chart —
/// [`keyflow::summary::ChartSummary`] exists so that a server indexing
/// on upload and a browser drawing a card cannot disagree about what
/// key something is in. So this asks it, rather than growing a second
/// answer here.
///
/// A chart that does not parse still saves. The text is the thing being
/// kept, and refusing to keep a half-written chart because it is
/// half-written would be exactly backwards — a draft is when you most
/// want it saved.
///
/// # `notation` is never sent, and `sections` always is
///
/// **`notation` is deliberately omitted.** On the server the field
/// names the *dialect* the source is written in — `keyflow`
/// (the default), `chordpro`, `nashville` — and everything this editor
/// saves is Keyflow, so the server's own default is already the right
/// answer. What it emphatically does not mean is the chord notation a
/// chart is written in. That has no single value: "as written" is the
/// truth for nearly every chart, one that mixes letters and numbers
/// keeps the mix, and the editor's notation picker is a *view* setting
/// ([`crate::notation`]). Stamping that here would file someone's
/// number chart as a letter chart because they once looked at it in
/// letters.
///
/// **`sections` must be declared, and only this side can.** The server
/// does not parse chart source — by design; the notation domain lives
/// here. A section it was not told about is not addressable as
/// `chart:<slug>#<section>`, so the anchors a collection or a link can
/// point at are exactly the ones sent from here. They go up
/// anchor-shaped (`VS 1` → `vs-1`) because that is what they are for,
/// and deduplicated, because two choruses are one anchor.
#[must_use]
pub fn draft_from_source(source: &str) -> Draft {
    let summary = keyflow::summary::ChartSummary::parse(source).ok();
    Draft {
        title: summary
            .as_ref()
            .and_then(|s| s.title.clone())
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Untitled chart".to_owned()),
        key: summary.as_ref().and_then(|s| s.key.clone()),
        sections: summary.map(|s| anchors(&s.sections)).unwrap_or_default(),
        source: source.to_owned(),
        slug: None,
        org: None,
        song: None,
    }
}

/// Section labels as addressable anchors, in order, without repeats.
///
/// `["IN", "VS 1", "CH", "VS 2", "CH"]` becomes
/// `["in", "vs-1", "ch", "vs-2"]`. The order is the shape of the song
/// and is kept; the repeat is dropped because an anchor names a place,
/// and the second chorus is the same place by that name.
#[must_use]
fn anchors(labels: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for label in labels {
        let anchor: String = label
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let anchor = anchor.trim_matches('-').to_owned();
        // Runs of punctuation collapse: "VS  1" is one anchor, not one
        // with a hole in it.
        let anchor = anchor
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        if !anchor.is_empty() && !out.contains(&anchor) {
            out.push(anchor);
        }
    }
    out
}

/// The chart to open when somebody asks for a song and names no
/// arrangement: the one the server marks default, else the first.
///
/// The fallback matters for a library that was imported rather than
/// written here — every song has at least one chart, and a song whose
/// only chart is not flagged still opens.
#[must_use]
pub fn default_chart(charts: &[ChartEntry]) -> Option<&ChartEntry> {
    charts
        .iter()
        .find(|chart| chart.is_default)
        .or_else(|| charts.first())
}

// ── The operations ───────────────────────────────────────────────────
//
// The seam. Free async functions rather than a trait, because there is
// exactly one implementation at a time and a trait would buy a vtable,
// an object-safety problem around `async fn`, and a second place to
// change when the transport moves. What matters is that the signatures
// name songs, charts and orgs. Every one takes the org explicitly: the
// transport is one connection *per org*, and "whichever org the server
// picks" is not a lane anybody can dial.

/// The song lists kept in `org`, each with its songs in order.
///
/// # Errors
///
/// [`LibraryError::SignedOut`] with no session,
/// [`LibraryError::Unsupported`] where the server has no collections,
/// [`LibraryError::Refused`] where the token is not a member of `org`,
/// or whatever the server said.
pub async fn list_songlists(org: &str) -> Result<Vec<SongList>, LibraryError> {
    vox::list_songlists(org).await
}

/// Every song in `org`'s library.
///
/// # Errors
///
/// As [`list_songlists`].
pub async fn list_songs(org: &str) -> Result<Vec<SongEntry>, LibraryError> {
    vox::list_songs(org).await
}

/// Every chart in `org` — or, with `song`, one song's arrangements.
///
/// # Errors
///
/// As [`list_songlists`].
pub async fn list_charts(org: &str, song: Option<&str>) -> Result<Vec<ChartEntry>, LibraryError> {
    vox::list_charts(org, song).await
}

/// One chart, in full, ready to open in the editor.
///
/// # Errors
///
/// As [`list_songlists`].
pub async fn read_chart(org: &str, slug: &str) -> Result<StoredChart, LibraryError> {
    vox::read_chart(org, slug).await
}

/// Keep a chart: a new one, or a new version of one already there.
///
/// Named for what a person is doing. The method it lands on is called
/// `upsert_chart` — see [`vox`] — and the difference is the seam doing
/// its job: "save" is the word on the button, `upsert_*` is the pattern
/// every ADR-0003 asset lane follows.
///
/// # Errors
///
/// [`LibraryError::NoOrg`] when the draft names no workspace, else as
/// [`list_songlists`].
pub async fn save_chart(draft: &Draft) -> Result<SaveOutcome, LibraryError> {
    vox::save_chart(draft).await
}

/// Take a chart off the shelf.
///
/// # Errors
///
/// As [`list_songlists`].
pub async fn delete_chart(org: &str, slug: &str) -> Result<(), LibraryError> {
    vox::delete_chart(org, slug).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chart describes itself; this module does not grow a second
    /// opinion about what key something is in.
    #[test]
    fn a_draft_takes_its_title_and_key_from_the_chart() {
        let draft = draft_from_source(keyflow_ui::examples::EXAMPLE_THRILLER);
        assert!(!draft.title.is_empty());
        assert_ne!(draft.title, "Untitled chart");
        assert!(
            !draft.sections.is_empty(),
            "a real chart has sections to file it under"
        );
    }

    /// A half-written chart still saves, and the text goes up
    /// verbatim. A draft is exactly when someone most wants it kept, so
    /// nothing here may refuse a chart for being unfinished.
    ///
    /// Note what the parser does with nonsense: it is lenient, and the
    /// first line of anything is that thing's title. So the fallback
    /// below is for a chart with no first line to speak of, not for a
    /// chart that "failed" — very little fails.
    #[test]
    fn a_half_written_chart_still_gets_a_draft() {
        let messy = "|||| not a chart ((";
        let draft = draft_from_source(messy);
        assert_eq!(draft.source, messy, "the text goes up as typed");
        assert!(!draft.title.is_empty(), "something has to name it");

        for nameless in ["", "   \n\n"] {
            assert_eq!(draft_from_source(nameless).title, "Untitled chart");
        }
    }

    /// A draft never carries a notation, and the type has no field for
    /// one. See [`draft_from_source`] for why that is a decision rather
    /// than an omission; this destructuring is what makes the shape
    /// hard to change by accident. It is also unattached and homeless
    /// until the screen says otherwise.
    #[test]
    fn a_draft_makes_no_claim_about_notation_song_or_org() {
        let Draft {
            title: _,
            source: _,
            key: _,
            sections: _,
            slug,
            org,
            song,
        } = draft_from_source(keyflow_ui::examples::EXAMPLE_THRILLER);
        assert!(slug.is_none() && org.is_none() && song.is_none());
    }

    /// Sections go up as anchors, in order, once each. They are what
    /// `chart:<slug>#<section>` addresses, and the server cannot derive
    /// them — it does not parse chart source.
    #[test]
    fn sections_become_anchors_in_order_without_repeats() {
        let labels: Vec<String> = ["IN", "VS 1", "CH", "VS  2", "CH", "  "]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(anchors(&labels), ["in", "vs-1", "ch", "vs-2"]);
        assert!(anchors(&[]).is_empty());
    }

    #[test]
    fn a_real_chart_declares_the_sections_it_has() {
        let draft = draft_from_source(keyflow_ui::examples::EXAMPLE_THRILLER);
        assert!(!draft.sections.is_empty());
        assert!(
            draft
                .sections
                .iter()
                .all(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')),
            "an anchor with a space in it does not address anything: {:?}",
            draft.sections
        );
    }

    fn chart(slug: &str, is_default: bool) -> ChartEntry {
        ChartEntry {
            slug: slug.to_owned(),
            title: slug.to_owned(),
            key: None,
            notation: None,
            sections: Vec::new(),
            song: Some("s".to_owned()),
            arrangement: None,
            is_default,
            updated_at: None,
        }
    }

    /// "The chart" of a song is the flagged one; an imported song whose
    /// chart was never flagged still opens.
    #[test]
    fn a_songs_default_chart_is_the_flagged_one_else_the_first() {
        let flagged = [chart("a", false), chart("b", true)];
        assert_eq!(default_chart(&flagged).map(|c| c.slug.as_str()), Some("b"));
        let unflagged = [chart("a", false), chart("b", false)];
        assert_eq!(
            default_chart(&unflagged).map(|c| c.slug.as_str()),
            Some("a")
        );
        assert!(default_chart(&[]).is_none());
    }

    #[test]
    fn the_base_url_is_the_production_server() {
        assert_eq!(task_base_url(), "https://task.fasttrackstudio.app");
    }
}
