//! Which workspace the charts go in.
//!
//! Task hosts several orgs and a person may belong to one, several, or
//! (briefly, before their personal org is provisioned) none. Charts are
//! written into one of them, so before anything can be saved this
//! question has to have an answer.
//!
//! It is answered by `GET /.well-known/task-server.json` with the
//! caller's bearer token: the document lists every org the server hosts
//! and tags each with whether that token validates there. Without the
//! token every org comes back `member: null` — the server has no way to
//! know — and a picker listing every org on a shared server, most of
//! which the person cannot write to, is worse than no picker at all.
//!
//! # Why this is not in [`super::mcp`]
//!
//! Because it does not change when the transport does. The chart calls
//! are MCP-over-HTTP today and vox tomorrow (see the module docs above);
//! discovery is a plain HTTP GET on a well-known path either way — it is
//! how a client learns what a server hosts *before* it has a session or
//! a transport. Tangling it into the MCP module would mean deleting and
//! rewriting it for no reason when the pin moves.
//!
//! # What is pure
//!
//! Everything except the GET. Parsing the document and deciding what to
//! do about it are ordinary functions over strings, tested on the host.

use serde_json::Value;

use super::{LibraryError, http, task_base_url};

/// Where the org choice is remembered, when there was a choice to make.
pub const ORG_KEY: &str = "keyflow.library.org";

/// Org discovery. Public, but answered *better* with a bearer token —
/// see the module docs.
#[must_use]
pub fn discovery_url(base: &str) -> String {
    format!(
        "{}/.well-known/task-server.json",
        base.trim_end_matches('/')
    )
}

/// One workspace the account might write charts into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Org {
    pub slug: String,
    /// What to call it in a picker. Falls back to the slug, because an
    /// org with no display name is still an org.
    pub name: String,
    pub is_home: bool,
    /// Does the caller's token validate here?
    ///
    /// `None` means the question was not asked — discovery ran without
    /// a token, or against a server old enough not to answer it. It is
    /// deliberately three-valued: `None` must not read as "not a
    /// member", or a signed-out person would be told they belong
    /// nowhere.
    pub member: Option<bool>,
}

/// Read the org list out of `/.well-known/task-server.json`.
///
/// # Errors
///
/// [`LibraryError::Malformed`] if the document is not JSON with an
/// `orgs` array.
pub fn orgs_from(body: &str) -> Result<Vec<Org>, LibraryError> {
    let doc: Value =
        serde_json::from_str(body).map_err(|e| LibraryError::Malformed(e.to_string()))?;
    let orgs = doc
        .get("orgs")
        .and_then(Value::as_array)
        .ok_or_else(|| LibraryError::Malformed("no `orgs` in the discovery document".to_owned()))?;
    Ok(orgs
        .iter()
        .filter_map(|org| {
            let slug = org.get("slug").and_then(Value::as_str)?.to_owned();
            let name = org
                .get("display_name")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&slug)
                .to_owned();
            Some(Org {
                is_home: org.get("is_home").and_then(Value::as_bool).unwrap_or(false),
                member: org.get("member").and_then(Value::as_bool),
                slug,
                name,
            })
        })
        .collect())
}

/// The orgs this account may actually write to.
///
/// `Some(false)` is a positive "not a member" and is dropped. `None` is
/// "not asked", and is kept — a server that does not tag membership
/// must not leave a signed-in person with an empty list.
#[must_use]
pub fn mine(orgs: &[Org]) -> Vec<Org> {
    orgs.iter()
        .filter(|org| org.member != Some(false))
        .cloned()
        .collect()
}

/// What to do about the org, once the list is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrgTarget {
    /// Exactly one candidate, or a remembered choice that still exists.
    /// Used silently — nobody should be asked a question with one
    /// answer.
    One(String),
    /// Several, and no remembered choice among them. The person picks.
    Choose(Vec<Org>),
    /// The account belongs to nothing that can hold a chart.
    None,
}

/// Decide which org to write to.
///
/// The rules, in order, and each exists because of a person rather than
/// a case in a match:
///
/// * A remembered choice that is still in the list wins. Someone who
///   picked once should not be asked again on the next chart.
/// * A remembered choice that is *not* in the list is discarded rather
///   than used — an org they were removed from, or a stale value from
///   another account on the same browser. Saving into it would fail on
///   the wire; asking again succeeds.
/// * One candidate is used without asking.
/// * Several is the only case that is a question.
#[must_use]
pub fn choose_org(orgs: &[Org], remembered: Option<&str>) -> OrgTarget {
    let mine = mine(orgs);
    if let Some(slug) = remembered
        && mine.iter().any(|org| org.slug == slug)
    {
        return OrgTarget::One(slug.to_owned());
    }
    match mine.len() {
        0 => OrgTarget::None,
        1 => OrgTarget::One(mine[0].slug.clone()),
        _ => OrgTarget::Choose(mine),
    }
}

/// The orgs the signed-in account can keep charts in.
///
/// # Errors
///
/// [`LibraryError::SignedOut`] with no session, or a transport failure.
pub async fn my_orgs() -> Result<Vec<Org>, LibraryError> {
    let token = crate::auth::access_token()
        .await
        .ok_or(LibraryError::SignedOut)?;
    let body = http::get(&discovery_url(&task_base_url()), &token).await?;
    Ok(mine(&orgs_from(&body)?))
}

/// The org to write to, remembering a choice once one is made.
///
/// # Errors
///
/// As [`my_orgs`].
pub async fn org_target() -> Result<OrgTarget, LibraryError> {
    let orgs = my_orgs().await?;
    let target = choose_org(&orgs, crate::prefs::string(ORG_KEY).as_deref());
    // Remember a silent single-org resolution too, so the next save
    // does not fetch discovery again to learn the same thing.
    if let OrgTarget::One(slug) = &target {
        remember_org(slug);
    }
    Ok(target)
}

/// Remember an org the person picked, so they are not asked again.
pub fn remember_org(slug: &str) {
    crate::prefs::set_string(ORG_KEY, slug);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn discovery(orgs: Value) -> String {
        json!({ "version": 1, "orgs": orgs }).to_string()
    }

    #[test]
    fn discovery_reads_the_orgs_and_their_membership() {
        let body = discovery(json!([
            { "slug": "codywright", "display_name": "Cody Wright (personal)",
              "is_home": true, "member": true },
            { "slug": "cbu", "display_name": "California Baptist University",
              "is_home": false, "member": false },
            { "slug": "unknown", "display_name": "", "is_home": false, "member": null },
        ]));
        let orgs = orgs_from(&body).unwrap();
        assert_eq!(orgs.len(), 3);
        assert!(orgs[0].is_home);
        assert_eq!(orgs[0].member, Some(true));
        // An org with no display name is still an org.
        assert_eq!(orgs[2].name, "unknown");

        // `Some(false)` is a positive "not a member" and is dropped;
        // `None` is "not asked" and is kept, or a server that does not
        // tag membership would leave someone with an empty list.
        let mine = mine(&orgs);
        assert_eq!(
            mine.iter().map(|o| o.slug.as_str()).collect::<Vec<_>>(),
            ["codywright", "unknown"]
        );
    }

    #[test]
    fn a_discovery_document_we_cannot_read_is_malformed() {
        assert!(matches!(orgs_from("{}"), Err(LibraryError::Malformed(_))));
        assert!(matches!(
            orgs_from("<html>"),
            Err(LibraryError::Malformed(_))
        ));
    }

    /// Nobody should be asked a question with one answer — and Task
    /// auto-provisions a personal org, so one answer is the common case.
    #[test]
    fn one_org_is_used_without_asking() {
        let orgs = vec![Org {
            slug: "codywright".to_owned(),
            name: "Cody Wright (personal)".to_owned(),
            is_home: true,
            member: Some(true),
        }];
        assert_eq!(
            choose_org(&orgs, None),
            OrgTarget::One("codywright".to_owned())
        );
    }

    #[test]
    fn several_orgs_are_a_question_until_one_is_remembered() {
        let orgs = vec![
            Org {
                slug: "codywright".to_owned(),
                name: "Personal".to_owned(),
                is_home: true,
                member: Some(true),
            },
            Org {
                slug: "cbu".to_owned(),
                name: "CBU".to_owned(),
                is_home: false,
                member: Some(true),
            },
        ];
        assert!(matches!(choose_org(&orgs, None), OrgTarget::Choose(_)));
        assert_eq!(choose_org(&orgs, Some("cbu")), OrgTarget::One("cbu".into()));

        // A remembered org they are no longer in is discarded, not
        // used: saving into it would fail on the wire, and asking again
        // succeeds.
        assert!(matches!(
            choose_org(&orgs, Some("somewhere-else")),
            OrgTarget::Choose(_)
        ));
    }

    #[test]
    fn belonging_to_nothing_is_said_plainly() {
        assert_eq!(choose_org(&[], None), OrgTarget::None);
        let not_mine = vec![Org {
            slug: "cbu".to_owned(),
            name: "CBU".to_owned(),
            is_home: false,
            member: Some(false),
        }];
        assert_eq!(choose_org(&not_mine, None), OrgTarget::None);
    }

    #[test]
    fn the_well_known_path_is_the_one_task_publishes() {
        assert_eq!(
            discovery_url("https://task.test/"),
            "https://task.test/.well-known/task-server.json"
        );
    }
}
