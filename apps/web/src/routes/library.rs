//! The library — the screens an account is actually for.
//!
//! [`SaveToLibrary`] is a button in the editor's pane header,
//! [`Library`] is `/library`: a workspace's song lists, its songs, and
//! the charts kept there, and [`LibraryChart`] is `/library/:org/:slug`,
//! one stored chart open in the editor and bound to it. Between them
//! they are the entire user-facing surface of [`crate::library`], and
//! all hold to the same rule the account itself does: **nothing here
//! gates the editor**. Signed out, the button is an invitation and the
//! screen is a sentence explaining what an account would give you. The
//! chart in the URL keeps working either way.
//!
//! # What the shelf shows
//!
//! Three groups, in the order a band thinks about them:
//!
//! * **Song lists** — "Adult Jam", a set, a repertoire. Each opens to
//!   its songs in order, and a song can be taken out of a list without
//!   leaving the library. A list that names a song the library no
//!   longer has shows the slug, greyed, rather than silently shrinking:
//!   a missing song is something to fix, not something to hide. New
//!   lists are made right here, by title.
//! * **Songs** — everything in the workspace's library, with who wrote
//!   it and its usual key, and a way to put each into a list.
//! * **Charts** — the unattached ones, saved before there were songs
//!   to attach to. A chart that belongs to a song is reached through
//!   the song.
//!
//! # Editing is in place
//!
//! Opening a song opens *the* chart of it — the one the server marks
//! default, else its first — **bound**: the editor knows which stored
//! chart it is showing, and Save writes back to that chart, keeping its
//! song and arrangement. Nothing is copied into the URL first. A chart
//! saved from a plain editor becomes a new song, with the chart as its
//! default, and the editor then navigates to the bound view so the next
//! Save is an edit rather than a second song with the same name.
//!
//! # Every state a save can be in
//!
//! * **signed out** — an invitation, not a failure.
//! * **saving** — the button says so and refuses a second click.
//! * **saved** — with a way through to the library.
//! * **failed** — the reason, in words, and the chart untouched.
//! * **not available yet** — [`crate::library::LibraryError::Unsupported`].
//!
//! # Which org
//!
//! Task auto-provisions a personal org for every account, so the common
//! case is exactly one and nobody is asked anything. Someone in several
//! is asked once, the answer is remembered ([`crate::library::ORG_KEY`]),
//! and the shelf has a picker to change it — a person in a band and a
//! church has two libraries. The decision itself is
//! [`crate::library::choose_org`], which is pure and tested.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::Route;
use crate::auth::{AuthState, use_auth};
use crate::library::{
    self, ChartEntry, LibraryError, Org, OrgTarget, SongEntry, SongList, StoredChart,
};
use crate::routes::{EditorScreen, Shell};

// ── A chart the editor is bound to ───────────────────────────────────

/// The stored chart an editor is an edit of. Everything Save needs to
/// write back in place: where it lives, what it is called on the shelf,
/// and the song and arrangement it must keep.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundChart {
    pub org: String,
    pub slug: String,
    pub title: String,
    pub song: Option<String>,
    pub arrangement: Option<String>,
}

impl BoundChart {
    fn of(org: &str, chart: &StoredChart) -> Self {
        Self {
            org: org.to_owned(),
            slug: chart.slug.clone(),
            title: chart.title.clone(),
            song: chart.song.clone(),
            arrangement: chart.arrangement.clone(),
        }
    }
}

/// `/library/:org/:slug` — one stored chart, open and bound.
#[component]
pub fn LibraryChart(org: String, slug: String) -> Element {
    let auth = use_auth();
    let chart = use_resource(move || {
        let org = org.clone();
        let slug = slug.clone();
        let signed_in = matches!((auth.state)(), AuthState::SignedIn(_));
        async move {
            if !signed_in {
                return Err(LibraryError::SignedOut);
            }
            library::read_chart(&org, &slug)
                .await
                .map(|chart| (org, chart))
        }
    });

    match (&(auth.state)(), &*chart.read_unchecked()) {
        (AuthState::Loading, _) | (_, None) => rsx! {
            Shell { section { class: "kf-prose", p { class: "kf-note", "One moment…" } } }
        },
        (_, Some(Ok((org, chart)))) => rsx! {
            EditorScreen {
                initial: chart.source.clone(),
                from_link: false,
                bound: Some(BoundChart::of(org, chart)),
            }
        },
        (_, Some(Err(error))) => rsx! {
            Shell {
                section { class: "kf-prose",
                    h1 { "That chart could not be opened" }
                    p { role: "alert", "{error}" }
                    Link { class: "kf-account-link", to: Route::Library {}, "Back to your library" }
                }
            }
        },
    }
}

// ── Save ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum SaveState {
    Idle,
    Invited,
    Saving,
    Saved(String),
    Pick(Vec<Org>),
    Failed(String),
}

/// The Save button in the editor, and the panel under it.
///
/// With `bound`, Save is an edit of that chart. Without, it makes a new
/// song from the chart's title and keeps the chart as its default, then
/// moves the editor to the bound view of what it just made.
#[component]
pub fn SaveToLibrary(source: String, bound: Option<BoundChart>) -> Element {
    let mut auth = use_auth();
    let mut state = use_signal(|| SaveState::Idle);
    let navigator = use_navigator();

    let signed_in = matches!((auth.state)(), AuthState::SignedIn(_));
    let showing = state();
    let saving = matches!(showing, SaveState::Saving);
    let inviting = matches!(showing, SaveState::Invited);
    let label = match showing {
        SaveState::Saving => "Saving…",
        SaveState::Saved(_) => "Saved",
        _ => "Save",
    };

    let here = format!("/c/{}", crate::chart_url::encode(&source));

    let bound_for_save = bound.clone();
    let start_save = move |source: String, org: Option<String>| {
        let bound = bound_for_save.clone();
        spawn(async move {
            state.set(SaveState::Saving);
            let mut draft = library::draft_from_source(&source);

            if let Some(chart) = bound {
                draft.org = Some(chart.org);
                draft.slug = Some(chart.slug);
                draft.song = chart.song;
                draft.arrangement = chart.arrangement;
                state.set(match library::save_chart(&draft).await {
                    Ok(_) => SaveState::Saved(draft.title),
                    Err(error) => SaveState::Failed(error.to_string()),
                });
                return;
            }

            let org = match org {
                Some(slug) => slug,
                None => match library::org_target().await {
                    Ok(OrgTarget::One(slug)) => slug,
                    Ok(OrgTarget::Choose(orgs)) => {
                        state.set(SaveState::Pick(orgs));
                        return;
                    }
                    // As on the shelf: an account with no workspace gets
                    // its personal one now, rather than being sent to Task.
                    Ok(OrgTarget::None) => match library::ensure_personal_org().await {
                        Ok(slug) => {
                            library::remember_org(&slug);
                            slug
                        }
                        Err(error) => {
                            state.set(SaveState::Failed(error.to_string()));
                            return;
                        }
                    },
                    Err(error) => {
                        state.set(SaveState::Failed(error.to_string()));
                        return;
                    }
                },
            };

            let song =
                match library::create_song(&org, &draft.title, draft.key.as_deref(), &[]).await {
                    Ok(slug) => slug,
                    Err(error) => {
                        state.set(SaveState::Failed(error.to_string()));
                        return;
                    }
                };
            draft.org = Some(org.clone());
            draft.song = Some(song);
            match library::save_chart(&draft).await {
                Ok(saved) => {
                    state.set(SaveState::Saved(draft.title.clone()));
                    navigator.push(Route::LibraryChart {
                        org,
                        slug: saved.slug,
                    });
                }
                Err(error) => state.set(SaveState::Failed(error.to_string())),
            }
        });
    };

    let on_click_source = source.clone();
    let save_on_click = start_save.clone();
    rsx! {
        div { class: "kf-save",
            button {
                class: "kf-button",
                disabled: saving,
                title: if bound.is_some() {
                    "Save your changes to this chart"
                } else {
                    "Keep this chart as a song in your FastTrackStudio library"
                },
                onclick: move |_| {
                    if signed_in {
                        save_on_click(on_click_source.clone(), None);
                    } else if inviting {
                        state.set(SaveState::Idle);
                    } else {
                        state.set(SaveState::Invited);
                    }
                },
                {label}
            }

            match showing {
                SaveState::Idle | SaveState::Saving => rsx! {},

                SaveState::Invited => rsx! {
                    div { class: "kf-save-panel",
                        p { class: "kf-account-note",
                            "Sign in to keep this chart in your library and open it from any
                             browser. The editor works without an account — your chart is in
                             this link either way."
                        }
                        div { class: "kf-account-actions",
                            button {
                                class: "kf-account-submit",
                                disabled: (auth.pending)(),
                                onclick: {
                                    let here = here.clone();
                                    move |_| auth.begin_sign_in_returning_to(here.clone())
                                },
                                if (auth.pending)() { "Taking you there…" } else { "Sign in" }
                            }
                            button {
                                class: "kf-account-link",
                                onclick: move |_| state.set(SaveState::Idle),
                                "Not now"
                            }
                        }
                    }
                },

                SaveState::Saved(title) => rsx! {
                    div { class: "kf-save-panel",
                        p { class: "kf-account-note", "Saved “{title}”." }
                        div { class: "kf-account-actions",
                            Link { class: "kf-account-link", to: Route::Library {},
                                "Your library"
                            }
                            button {
                                class: "kf-account-link",
                                onclick: move |_| state.set(SaveState::Idle),
                                "Close"
                            }
                        }
                    }
                },

                SaveState::Pick(orgs) => rsx! {
                    div { class: "kf-save-panel",
                        p { class: "kf-account-note", "Which workspace should keep it?" }
                        div { class: "kf-save-orgs",
                            for org in orgs {
                                button {
                                    key: "{org.slug}",
                                    class: "kf-button",
                                    onclick: {
                                        let slug = org.slug.clone();
                                        let source = source.clone();
                                        let start_save = start_save.clone();
                                        move |_| {
                                            library::remember_org(&slug);
                                            start_save(source.clone(), Some(slug.clone()));
                                        }
                                    },
                                    "{org.name}"
                                }
                            }
                        }
                    }
                },

                SaveState::Failed(message) => rsx! {
                    div { class: "kf-save-panel",
                        p { class: "kf-account-error", role: "alert", "{message}" }
                        p { class: "kf-account-note",
                            "Your chart is untouched, and it is still in this editor."
                        }
                        div { class: "kf-account-actions",
                            button {
                                class: "kf-account-link",
                                onclick: move |_| state.set(SaveState::Idle),
                                "Close"
                            }
                        }
                    }
                },
            }
        }
    }
}

// ── The shelf ────────────────────────────────────────────────────────

/// What `/library` has to show once it knows who is asking.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Shelf {
    /// Several workspaces and no remembered choice: ask once.
    Pick(Vec<Org>),
    /// One workspace's library, and the others it could switch to.
    Library {
        org: String,
        elsewhere: Vec<Org>,
        songlists: Vec<SongList>,
        songs: Vec<SongEntry>,
        /// Every chart in the workspace. The shelf counts a song's
        /// charts and links its main one from this, and lists the
        /// unattached ones on their own — one call for all three.
        charts: Vec<ChartEntry>,
    },
}

/// The shelf for the chosen (or remembered, or only) workspace.
///
/// Three calls on one connection, in the order the page shows them.
/// The chart listing is the whole shelf and is filtered here to the
/// unattached ones, because "every chart" and "the charts a song owns"
/// are the same server call with and without a filter, and one call
/// for the shelf beats one per song.
pub(super) async fn shelf_for(chosen: Option<String>) -> Result<Shelf, LibraryError> {
    let orgs = library::my_orgs().await?;
    let remembered = chosen.or_else(|| crate::prefs::string(library::ORG_KEY));
    match library::choose_org(&orgs, remembered.as_deref()) {
        // Nobody's first chart should need a second product. An account
        // that belongs to no workspace gets its personal one here, on the
        // spot — the call is idempotent, so the worst case of asking is
        // being told the slug it already had.
        OrgTarget::None => {
            let slug = library::ensure_personal_org().await?;
            Ok(Shelf::Library {
                org: slug,
                elsewhere: Vec::new(),
                songlists: Vec::new(),
                songs: Vec::new(),
                charts: Vec::new(),
            })
        }
        OrgTarget::Choose(orgs) => Ok(Shelf::Pick(orgs)),
        OrgTarget::One(slug) => {
            let songlists = library::list_songlists(&slug).await?;
            let songs = library::list_songs(&slug).await?;
            let charts = library::list_charts(&slug, None).await?;
            Ok(Shelf::Library {
                org: slug,
                elsewhere: orgs,
                songlists,
                songs,
                charts,
            })
        }
    }
}

/// The charts the shelf lists on their own: those with no song, **and**
/// those whose song is no longer in the library. Deleting a song keeps
/// its charts by design, and a chart reached only through a song that is
/// gone would otherwise be on the server and nowhere on the screen.
pub(super) fn unattached(charts: Vec<ChartEntry>, songs: &[SongEntry]) -> Vec<ChartEntry> {
    charts
        .into_iter()
        .filter(|chart| match &chart.song {
            None => true,
            Some(slug) => !songs.iter().any(|song| &song.slug == slug),
        })
        .collect()
}

/// How the Songs section is ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SortBy {
    Title,
    Writer,
    Key,
    Recent,
}

impl SortBy {
    pub(super) const ALL: [Self; 4] = [Self::Title, Self::Writer, Self::Key, Self::Recent];

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Writer => "writer",
            Self::Key => "key",
            Self::Recent => "recent",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Writer => "Writer",
            Self::Key => "Key",
            Self::Recent => "Recently changed",
        }
    }

    pub(super) fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.id() == id)
            .unwrap_or(Self::Title)
    }
}

/// Where the sort choice is remembered: a band that thinks in keys
/// should not have to say so every visit.
pub(super) const SORT_KEY: &str = "keyflow.library.sort";

/// Order songs for the shelf. Every order falls back to the title, so
/// two songs by one writer (or in one key) still read alphabetically,
/// and songs missing the field sort last rather than first.
pub(super) fn sort_songs(songs: &mut [SongEntry], by: SortBy) {
    let title = |s: &SongEntry| s.title.to_lowercase();
    match by {
        SortBy::Title => songs.sort_by_key(title),
        SortBy::Writer => songs.sort_by_key(|s| {
            (
                s.writers.is_empty(),
                s.writers.first().map(|w| w.to_lowercase()),
                title(s),
            )
        }),
        SortBy::Key => songs.sort_by_key(|s| {
            (
                s.key.is_none(),
                s.key.as_deref().map(str::to_lowercase),
                title(s),
            )
        }),
        // Newest first; RFC 3339 in one zone sorts as text.
        SortBy::Recent => songs.sort_by(|a, b| {
            (b.updated_at.is_some(), &b.updated_at)
                .cmp(&(a.updated_at.is_some(), &a.updated_at))
                .then_with(|| title(a).cmp(&title(b)))
        }),
    }
}

/// A song list's rows: its songs in the list's order, resolved against
/// the library, with a missing one kept as its slug.
pub(super) fn rows_of(list: &SongList, songs: &HashMap<String, SongEntry>) -> Vec<ListRow> {
    list.songs
        .iter()
        .map(|slug| match songs.get(slug) {
            Some(song) => ListRow::Song(song.clone()),
            None => ListRow::Missing(slug.clone()),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ListRow {
    Song(SongEntry),
    Missing(String),
}

/// Does this song answer to what was typed in the search box?
///
/// Title first, then writers, because "who is this by" is the second way
/// a person looks for a song they half-remember. Case-insensitive and
/// substring rather than prefix: a repertoire is full of titles whose
/// distinguishing word is in the middle.
pub(super) fn matches(song: &SongEntry, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    song.title.to_lowercase().contains(&q)
        || song.writers.iter().any(|w| w.to_lowercase().contains(&q))
        || song.key.as_deref().is_some_and(|k| k.to_lowercase() == q)
}

/// The one reorder that moves `slug` a place `up` (or down) in `order`:
/// which song to move, and which song it should land after.
///
/// **Every move names a predecessor.** The collection lane places an item
/// *after* another one and reads a missing predecessor as "put it at the
/// end" — so asking to move to the top by naming nothing sends the song
/// to the bottom of the list instead, which is the opposite of what the
/// button says. Promoting the second song is therefore expressed as
/// demoting the first: one call, same result, and no way to mean "the
/// end" by accident.
///
/// `None` when the move would fall off an end, so the button can be
/// disabled rather than send a request that means nothing.
pub(super) fn move_plan(order: &[String], slug: &str, up: bool) -> Option<(String, String)> {
    let at = order.iter().position(|s| s == slug)?;
    if up {
        match at {
            0 => None,
            // Swap with the song above by moving *it* after this one.
            1 => Some((order[0].clone(), slug.to_owned())),
            _ => Some((slug.to_owned(), order[at - 2].clone())),
        }
    } else if at + 1 >= order.len() {
        None
    } else {
        Some((slug.to_owned(), order[at + 1].clone()))
    }
}

/// What a bulk add will do to one list: the picked songs it does not
/// hold yet, in the order picked, and how many it already had. Adding a
/// song a list already holds is refused by the lane, so those are
/// counted rather than sent.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct BulkPlan {
    pub(super) to_add: Vec<String>,
    pub(super) already: usize,
}

pub(super) fn bulk_plan(picked: &[String], list: &SongList) -> BulkPlan {
    let (already, to_add): (Vec<&String>, Vec<&String>) =
        picked.iter().partition(|slug| list.songs.contains(slug));
    BulkPlan {
        to_add: to_add.into_iter().cloned().collect(),
        already: already.len(),
    }
}

/// `Added 3 songs to “Adult Jam” (2 were already there).`
pub(super) fn bulk_summary(added: usize, already: usize, list: &str) -> String {
    let mut out = format!("Added {} to “{list}”", plural(added, "song"));
    match already {
        0 => {}
        1 => out.push_str(" (1 was already there)"),
        n => out.push_str(&format!(" ({n} were already there)")),
    }
    out.push('.');
    out
}

/// `1 song`, `12 songs`.
/// `2026-09-22T22:45:00.527Z` → `22 Sep 2026`. The server's stamps are
/// RFC 3339; a person reads a day, not a millisecond. Anything that does
/// not start with a date is shown as it came rather than hidden.
pub(crate) fn short_date(stamp: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let date = stamp.get(..10).unwrap_or(stamp);
    let mut parts = date.split('-');
    let parsed = (|| {
        let year: u32 = parts.next()?.parse().ok()?;
        let month: usize = parts.next()?.parse().ok()?;
        let day: u32 = parts.next()?.parse().ok()?;
        let name = MONTHS.get(month.checked_sub(1)?)?;
        Some(format!("{day} {name} {year}"))
    })();
    parsed.unwrap_or_else(|| stamp.to_owned())
}

pub(super) fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(slug: &str) -> SongEntry {
        SongEntry {
            slug: slug.to_owned(),
            title: slug.to_uppercase(),
            writers: Vec::new(),
            key: None,
            tags: Vec::new(),
            updated_at: None,
        }
    }

    fn titled(
        slug: &str,
        title: &str,
        writer: Option<&str>,
        key: Option<&str>,
        at: Option<&str>,
    ) -> SongEntry {
        SongEntry {
            title: title.to_owned(),
            writers: writer.map(|w| vec![w.to_owned()]).unwrap_or_default(),
            key: key.map(str::to_owned),
            updated_at: at.map(str::to_owned),
            ..song(slug)
        }
    }

    fn order(songs: &[SongEntry]) -> Vec<&str> {
        songs.iter().map(|s| s.slug.as_str()).collect()
    }

    /// Every order breaks ties by title, and a song missing the field
    /// sorts after the ones that have it.
    #[test]
    fn songs_sort_by_each_field_with_title_as_the_tiebreak() {
        let mut songs = vec![
            titled(
                "c",
                "cecilia",
                Some("Simon"),
                None,
                Some("2026-09-01T00:00:00Z"),
            ),
            titled("a", "Africa", Some("Toto"), Some("A"), None),
            titled(
                "b",
                "Birdland",
                None,
                Some("A"),
                Some("2026-09-20T00:00:00Z"),
            ),
            titled(
                "d",
                "Dreams",
                Some("Simon"),
                Some("C"),
                Some("2026-09-10T00:00:00Z"),
            ),
        ];
        sort_songs(&mut songs, SortBy::Title);
        assert_eq!(order(&songs), ["a", "b", "c", "d"], "case does not matter");
        sort_songs(&mut songs, SortBy::Writer);
        assert_eq!(order(&songs), ["c", "d", "a", "b"]);
        sort_songs(&mut songs, SortBy::Key);
        assert_eq!(order(&songs), ["a", "b", "d", "c"]);
        sort_songs(&mut songs, SortBy::Recent);
        assert_eq!(
            order(&songs),
            ["b", "d", "c", "a"],
            "newest first, undated last"
        );
        assert_eq!(SortBy::from_id("nonsense"), SortBy::Title);
        for by in SortBy::ALL {
            assert_eq!(SortBy::from_id(by.id()), by);
        }
    }

    /// A bulk add sends only what the list lacks, and says how many it
    /// already had rather than failing on them.
    #[test]
    fn a_bulk_add_skips_what_the_list_already_holds() {
        let list = SongList {
            id: "set".to_owned(),
            title: "Set".to_owned(),
            songs: vec!["b".to_owned()],
        };
        let picked: Vec<String> = ["c", "b", "a"].map(String::from).to_vec();
        let plan = bulk_plan(&picked, &list);
        assert_eq!(plan.to_add, ["c", "a"], "in the order picked");
        assert_eq!(plan.already, 1);
        assert_eq!(
            bulk_summary(2, 1, "Set"),
            "Added 2 songs to “Set” (1 was already there)."
        );
        assert_eq!(bulk_summary(1, 0, "Set"), "Added 1 song to “Set”.");
    }

    /// A chart whose song was deleted is still on the shelf.
    #[test]
    fn a_chart_of_a_deleted_song_counts_as_unattached() {
        let chart = |slug: &str, song: Option<&str>| ChartEntry {
            slug: slug.to_owned(),
            title: slug.to_owned(),
            key: None,
            notation: None,
            sections: Vec::new(),
            song: song.map(str::to_owned),
            arrangement: None,
            is_default: false,
            updated_at: None,
        };
        let shelf = unattached(
            vec![
                chart("loose", None),
                chart("kept", Some("a")),
                chart("orphan", Some("gone")),
            ],
            &[song("a")],
        );
        let slugs: Vec<&str> = shelf.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(slugs, ["loose", "orphan"]);
    }

    /// The move that promotes the second song is the one that demotes the
    /// first — because the lane can only place a song *after* another,
    /// and naming nothing means the end of the list. Sending `None` here
    /// once put the second song of a 204-song set at number 204.
    #[test]
    fn every_move_names_the_song_it_lands_after() {
        let order: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();

        assert_eq!(move_plan(&order, "a", true), None, "the first cannot rise");
        assert_eq!(move_plan(&order, "d", false), None, "the last cannot fall");

        // Second one up: move `a` to sit after `b`.
        assert_eq!(
            move_plan(&order, "b", true),
            Some(("a".to_owned(), "b".to_owned()))
        );
        // Third one up: it lands after the first.
        assert_eq!(
            move_plan(&order, "c", true),
            Some(("c".to_owned(), "a".to_owned()))
        );
        // Anything down lands after its successor.
        assert_eq!(
            move_plan(&order, "b", false),
            Some(("b".to_owned(), "c".to_owned()))
        );
        assert_eq!(move_plan(&order, "unknown", true), None);
    }

    /// Searching answers on what a person half-remembers: part of the
    /// title, or who it is by.
    #[test]
    fn search_matches_a_title_or_a_writer_and_ignores_case() {
        let mut tune = song("africa");
        tune.title = "AFRICA".to_owned();
        tune.writers = vec!["TOTO".to_owned()];
        tune.key = Some("A".to_owned());

        assert!(matches(&tune, ""), "an empty box hides nothing");
        assert!(matches(&tune, "fri"), "a word in the middle still finds it");
        assert!(matches(&tune, "toto"), "and so does who it is by");
        assert!(matches(&tune, "a"), "a one-letter key is an exact match");
        assert!(!matches(&tune, "hosanna"));
    }

    /// A list keeps its order and does not lose a song the library
    /// lacks — that row says so instead.
    #[test]
    fn a_song_list_resolves_in_order_and_keeps_what_is_missing() {
        let library: HashMap<String, SongEntry> = [song("wonderwall"), song("basket-case")]
            .into_iter()
            .map(|s| (s.slug.clone(), s))
            .collect();
        let list = SongList {
            id: "adult-jam".to_owned(),
            title: "Adult Jam".to_owned(),
            songs: vec![
                "basket-case".to_owned(),
                "gone".to_owned(),
                "wonderwall".to_owned(),
            ],
        };
        let rows = rows_of(&list, &library);
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[0], ListRow::Song(s) if s.slug == "basket-case"));
        assert!(matches!(&rows[1], ListRow::Missing(slug) if slug == "gone"));
        assert!(matches!(&rows[2], ListRow::Song(s) if s.slug == "wonderwall"));
    }

    /// What Save writes back for a bound chart is exactly what it was
    /// bound to: same org, same slug, same song, same arrangement.
    #[test]
    fn a_bound_chart_carries_everything_a_save_must_keep() {
        let stored = StoredChart {
            slug: "wonderwall-default".to_owned(),
            title: "Wonderwall".to_owned(),
            source: "Wonderwall\n".to_owned(),
            key: Some("F#m".to_owned()),
            notation: None,
            sections: vec!["vs-1".to_owned()],
            song: Some("wonderwall".to_owned()),
            arrangement: Some("Default".to_owned()),
        };
        let bound = BoundChart::of("rockstars-of-tomorrow", &stored);
        assert_eq!(bound.org, "rockstars-of-tomorrow");
        assert_eq!(bound.slug, "wonderwall-default");
        assert_eq!(bound.song.as_deref(), Some("wonderwall"));
        assert_eq!(bound.arrangement.as_deref(), Some("Default"));
    }

    #[test]
    fn stamps_read_as_a_day() {
        assert_eq!(short_date("2026-09-22T22:45:00.527Z"), "22 Sep 2026");
        assert_eq!(short_date("2026-01-03"), "3 Jan 2026");
        assert_eq!(short_date("yesterday"), "yesterday");
        assert_eq!(short_date("2026-13-01"), "2026-13-01");
    }

    #[test]
    fn counts_read_as_english() {
        assert_eq!(plural(1, "song"), "1 song");
        assert_eq!(plural(0, "song"), "0 songs");
        assert_eq!(plural(199, "song"), "199 songs");
    }
}
