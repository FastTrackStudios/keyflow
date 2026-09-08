//! The chart library — charts kept in a FastTrackStudio account.
//!
//! The editor's persistence story is the URL ([`crate::chart_url`]): a
//! chart deflates into a `/c/:data` path, so sharing one is sharing a
//! link and nothing needs an account. That is deliberately enough to be
//! useful, and none of it changes here. What it cannot do is give
//! someone their charts back on a different machine, or a list of
//! everything they have written. That is what an account is for — and
//! only that. **The editor stays fully usable signed out**; this module
//! adds a place to keep charts, never a gate in front of writing one.
//!
//! # Where the charts actually live
//!
//! In Task (`task.fasttrackstudio.app`), the vault product, under the
//! signed-in person's org. Keyflow does not run a server and is not
//! about to start: Task already is one — files on disk, versioned,
//! reachable from the desktop app, the CLI and the phone — and a chart
//! is a small text document, which is precisely what it stores. Keyflow
//! writes into someone's existing vault rather than inventing a second
//! account with a second copy of their work in it.
//!
//! # This module is a seam
//!
//! Five operations, in Keyflow's own words: [`list_charts`],
//! [`read_chart`], [`save_chart`], [`delete_chart`], and
//! [`org_target`] for the workspace they act in. Callers speak charts
//! and orgs; nothing above this file mentions a wire format, and nothing
//! above it should ever start to. That is the whole design constraint,
//! and the next section is why.
//!
//! ```text
//! routes::library      the screens — charts, orgs, states
//!        │
//!   library (here)     the operations, in domain terms
//!        ├── mcp       ← today's transport. Replaceable.
//!        ├── discovery ← orgs. HTTP either way.
//!        └── http      ← window.fetch
//! ```
//!
//! # The transport, and why it is this one
//!
//! **The chart calls go over Task's MCP JSON-RPC endpoint
//! (`POST /mcp`) with `window.fetch`, and that is a stepping stone, not
//! the destination.** The intended transport is vox — architect's RPC
//! layer, the same one Task's own web app, CLI and desktop client use,
//! reaching `resources-proto`'s chart methods directly, with generated
//! clients, typed errors and live subscriptions for free. That is what
//! ADR 0003 (the cross-app backend: Task as the shared store for
//! Session, Signal, Ignition and Keyflow) always intended.
//!
//! It could not be done when this was written, and the reason is a link
//! error rather than a design view. Keyflow was pinned to architect
//! `v0.0.2`; Task is on `v0.7.1`. Two architect versions in one wasm
//! binary means two reqwest majors in one wasm binary, and each compiles
//! its own copy of the wasm-streams glue — which comes out of rust-lld
//! as a hundred `duplicate symbol: intounderlyingsource_*` errors.
//! `apps/web/Cargo.toml` says the same thing at length in three separate
//! places, because it is the binding constraint on this crate's whole
//! dependency list: it is why the guide's graph rail uses *this* repo's
//! `view-knowledge-graph` rather than task's, why `dioxus/fullstack` is
//! not in the `web` feature, and why signing in is hand-rolled OIDC over
//! `fetch` instead of `auth-http`.
//!
//! So: no `resources-proto`, no second `architect`, no client crate at
//! all. Task exposes the same vault operations over MCP as plain JSON
//! over HTTP, and JSON over HTTP is something `window.fetch` and
//! `serde_json` — both already here for [`crate::auth`] — can do without
//! adding a byte to the dependency graph.
//!
//! **The pin is being lifted.** When it reaches v0.7.1, the change is
//! meant to be: take a vox client, rewrite the four functions at the
//! bottom of this file to call it, delete [`mcp`]. Nothing in
//! [`crate::routes::library`] should need to change, and if it does,
//! this seam was drawn in the wrong place. [`discovery`] does not change
//! either — a well-known document fetched before any session exists is
//! HTTP under every transport.
//!
//! # What is pure, and why
//!
//! Everything except the `fetch` calls in [`http`]. Building a JSON-RPC
//! envelope, reading a tool result out of the two-layer MCP response,
//! deciding which org to write to — all of it is ordinary functions over
//! strings that `just test` runs on the host, exactly the way
//! [`crate::oidc`] keeps PKCE testable on a target with no browser in
//! it. The browser half is then thin enough to read in one screen.
//!
//! # Degrading when a tool is not there
//!
//! `list_charts`, `read_chart` and `write_chart` are live. `delete_chart`
//! is not yet, and an older deployment has none of them: either answers
//! `tools/call` with `unknown tool`, and that part of the library is
//! then simply not available — which is a sentence on a page, not a
//! panic and not a blank screen. [`LibraryError::Unsupported`] is that
//! case, kept separate from every other failure precisely so the UI can
//! say the true thing about it. It is a fallback rather than the normal
//! path, and Remove is the one control that routinely takes it today.

// The transport half is browser-only, and the host build of the site
// (`cargo check --workspace`, which is not `just web-check`) compiles
// these files with the pure half alone. See CLAUDE.md.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub mod discovery;
mod http;
mod mcp;

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

/// The MCP JSON-RPC endpoint — the *account* lane, which resolves the
/// caller's orgs from their membership rather than being pinned to one.
///
/// Here rather than in [`mcp`] only because it is one line of the same
/// base URL; it goes when that module does.
#[must_use]
pub fn mcp_url(base: &str) -> String {
    format!("{}/mcp", base.trim_end_matches('/'))
}

// ── Failures ─────────────────────────────────────────────────────────

/// Why a library operation did not happen.
///
/// Deliberately transport-free: not one variant names HTTP, JSON-RPC or
/// MCP, so the same set survives the move to vox and the screens keep
/// matching on the same cases. Every variant is a different sentence on
/// the screen, which is the only reason there is more than one of them.
/// In particular [`Self::Unsupported`] is not folded into
/// [`Self::Refused`]: "your charts are not available on this server yet"
/// and "the server said no" ask the person to do completely different
/// things.
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
    /// The server does not offer chart storage. An older deployment, or
    /// one with the vault plugin turned off.
    Unsupported,
    /// The server understood and declined, and said why.
    Refused(String),
    /// The server answered with something this client cannot read.
    Malformed(String),
    /// The request never got an answer — offline, DNS, CORS, a dead
    /// host.
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

// ── What a kept chart is ─────────────────────────────────────────────

/// A row in a listing.
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
    /// The shape of the song — `["IN", "VS 1", "CH"]`. Enough to tell
    /// two charts with similar titles apart at a glance, which is what
    /// a shelf is for.
    pub sections: Vec<String>,
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

// ── The operations ───────────────────────────────────────────────────
//
// The seam. Free async functions rather than a trait, because there is
// exactly one implementation at a time and a trait would buy a vtable,
// an object-safety problem around `async fn`, and a second place to
// change when the transport moves. What matters is that the signatures
// name charts and orgs, and that swapping `mcp` for a vox client leaves
// them alone.

/// Everything this account has kept, in `org` (or its default org).
///
/// # Errors
///
/// [`LibraryError::SignedOut`] with no session,
/// [`LibraryError::Unsupported`] where the server has no chart storage,
/// or whatever the server said.
pub async fn list_charts(org: Option<&str>) -> Result<Vec<ChartEntry>, LibraryError> {
    mcp::list_charts(org).await
}

/// One chart, in full, ready to open in the editor.
///
/// # Errors
///
/// As [`list_charts`].
pub async fn read_chart(slug: &str, org: Option<&str>) -> Result<StoredChart, LibraryError> {
    mcp::read_chart(slug, org).await
}

/// Keep a chart: a new one, or a new version of one already there.
///
/// Named for what a person is doing. The lane it lands on is called
/// `write_chart` — see [`mcp`] — and the difference is the seam doing
/// its job: "save" is the word on the button, `write_*` is the pattern
/// every ADR-0003 asset lane follows.
///
/// # Errors
///
/// As [`list_charts`].
pub async fn save_chart(draft: &Draft) -> Result<SaveOutcome, LibraryError> {
    mcp::save_chart(draft).await
}

/// Take a chart off the shelf.
///
/// # Errors
///
/// As [`list_charts`].
pub async fn delete_chart(slug: &str, org: Option<&str>) -> Result<(), LibraryError> {
    mcp::delete_chart(slug, org).await
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
    /// hard to change by accident.
    #[test]
    fn a_draft_makes_no_claim_about_notation() {
        let Draft {
            title: _,
            source: _,
            key: _,
            sections: _,
            slug,
            org,
        } = draft_from_source(keyflow_ui::examples::EXAMPLE_THRILLER);
        assert!(slug.is_none() && org.is_none());
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

    #[test]
    fn urls_are_built_off_one_base() {
        assert_eq!(mcp_url("https://task.test/"), "https://task.test/mcp");
        assert_eq!(task_base_url(), "https://task.fasttrackstudio.app");
    }
}
