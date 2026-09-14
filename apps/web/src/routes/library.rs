//! The library — the two screens an account is actually for.
//!
//! [`SaveToLibrary`] is a button in the editor's pane header, and
//! [`Library`] is `/library`: a workspace's song lists, its songs, and
//! the charts kept there. Between them they are the entire user-facing
//! surface of [`crate::library`], and both hold to the same rule the
//! account itself does: **nothing here gates the editor**. Signed out,
//! the button is an invitation and the screen is a sentence explaining
//! what an account would give you. The chart in the URL keeps working
//! either way.
//!
//! # What the shelf shows
//!
//! Three groups, in the order a band thinks about them:
//!
//! * **Song lists** — "Adult Jam", a set, a repertoire. Each opens to
//!   its songs in order. A list that names a song the library no
//!   longer has shows the slug, greyed, rather than silently shrinking:
//!   a missing song is something to fix, not something to hide.
//! * **Songs** — everything in the workspace's library, with who wrote
//!   it and its usual key.
//! * **Charts** — the unattached ones, which is what the editor's Save
//!   makes. A chart that belongs to a song is reached through the song.
//!
//! Opening a song opens *the* chart of it — the one the server marks
//! default, else its first — because "open Wonderwall" means the chart,
//! and asking which arrangement every time would be asking a question
//! the person did not have.
//!
//! # Every state a save can be in
//!
//! Saving over a network has more states than "done", and each of them
//! is a different thing to say:
//!
//! * **signed out** — an invitation, not a failure. The person has not
//!   done anything wrong; they have not signed in.
//! * **saving** — the button says so and refuses a second click, because
//!   two saves of the same chart is two versions of it.
//! * **saved** — with a way through to the library, since "where did it
//!   go" is the immediate next question.
//! * **failed** — the reason, in words, and the chart untouched. A
//!   failed save must never look like a lost chart.
//! * **not available yet** — its own case, because the server may not
//!   carry the library methods yet and "your library is not on this
//!   server yet" is not a failure anyone can act on. See
//!   [`crate::library::LibraryError::Unsupported`].
//!
//! # Which org
//!
//! Task auto-provisions a personal org for every account, so the common
//! case is exactly one and nobody is asked anything. Someone in several
//! is asked once, and the answer is remembered
//! ([`crate::library::ORG_KEY`]); the shelf then has a picker to change
//! it, since a person in a band and a church has two libraries. The
//! decision itself is [`crate::library::choose_org`], which is pure and
//! tested; what is here is only the panel that shows it.
//!
//! # Opening a chart puts it back in the URL
//!
//! "Open" reads the chart in full and navigates to `/c/:data` — the
//! same shareable route a link produces. So an opened chart behaves
//! exactly like every other chart on the site: shareable, bookmarkable,
//! and editable with no further round trips. Saving it again derives the
//! same slug from the same title, which makes that a new *version*
//! rather than a second chart.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::Route;
use crate::auth::{AuthState, use_auth};
use crate::chart_url;
use crate::library::{
    self, ChartEntry, LibraryError, Org, OrgTarget, SongEntry, SongList, StoredChart,
};
use crate::routes::Shell;

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
#[component]
pub fn SaveToLibrary(source: String) -> Element {
    let mut auth = use_auth();
    let mut state = use_signal(|| SaveState::Idle);

    let signed_in = matches!((auth.state)(), AuthState::SignedIn(_));
    let showing = state();
    let saving = matches!(showing, SaveState::Saving);
    let inviting = matches!(showing, SaveState::Invited);
    let label = match showing {
        SaveState::Saving => "Saving…",
        SaveState::Saved(_) => "Saved",
        _ => "Save",
    };

    let here = format!("/c/{}", chart_url::encode(&source));

    let start_save = move |source: String, org: Option<String>| {
        spawn(async move {
            state.set(SaveState::Saving);
            let org = match org {
                Some(slug) => Some(slug),
                None => match library::org_target().await {
                    Ok(OrgTarget::One(slug)) => Some(slug),
                    Ok(OrgTarget::Choose(orgs)) => {
                        state.set(SaveState::Pick(orgs));
                        return;
                    }
                    Ok(OrgTarget::None) => {
                        state.set(SaveState::Failed(LibraryError::NoOrg.to_string()));
                        return;
                    }
                    Err(error) => {
                        state.set(SaveState::Failed(error.to_string()));
                        return;
                    }
                },
            };
            let mut draft = library::draft_from_source(&source);
            draft.org = org;
            state.set(match library::save_chart(&draft).await {
                Ok(saved) => SaveState::Saved(saved.slug),
                Err(error) => SaveState::Failed(error.to_string()),
            });
        });
    };

    let on_click_source = source.clone();
    rsx! {
        div { class: "kf-save",
            button {
                class: "kf-button",
                disabled: saving,
                title: "Keep this chart in your FastTrackStudio account",
                onclick: move |_| {
                    if signed_in {
                        start_save(on_click_source.clone(), None);
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

                SaveState::Saved(slug) => rsx! {
                    div { class: "kf-save-panel",
                        p { class: "kf-account-note", "Kept as “{slug}”." }
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
                            "Your chart is untouched, and it is still in this link."
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
enum Shelf {
    /// Several workspaces and no remembered choice: ask once.
    Pick(Vec<Org>),
    /// One workspace's library, and the others it could switch to.
    Library {
        org: String,
        elsewhere: Vec<Org>,
        songlists: Vec<SongList>,
        songs: Vec<SongEntry>,
        /// The unattached charts only; a song's charts are reached
        /// through the song.
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
async fn shelf_for(chosen: Option<String>) -> Result<Shelf, LibraryError> {
    let orgs = library::my_orgs().await?;
    let remembered = chosen.or_else(|| crate::prefs::string(library::ORG_KEY));
    match library::choose_org(&orgs, remembered.as_deref()) {
        OrgTarget::None => Err(LibraryError::NoOrg),
        OrgTarget::Choose(orgs) => Ok(Shelf::Pick(orgs)),
        OrgTarget::One(slug) => {
            let songlists = library::list_songlists(&slug).await?;
            let songs = library::list_songs(&slug).await?;
            let charts = library::list_charts(&slug, None)
                .await?
                .into_iter()
                .filter(|chart| chart.song.is_none())
                .collect();
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

/// A song list's rows: its songs in the list's order, resolved against
/// the library, with a missing one kept as its slug.
fn rows_of(list: &SongList, songs: &HashMap<String, SongEntry>) -> Vec<ListRow> {
    list.songs
        .iter()
        .map(|slug| match songs.get(slug) {
            Some(song) => ListRow::Song(song.clone()),
            None => ListRow::Missing(slug.clone()),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum ListRow {
    Song(SongEntry),
    Missing(String),
}

/// `/library`.
#[component]
pub fn Library() -> Element {
    let mut auth = use_auth();
    let mut picked = use_signal(|| None::<String>);
    let mut reload = use_signal(|| 0_u32);
    // What is being opened right now, as `song:<slug>` or
    // `chart:<slug>`, so exactly that row says "Opening…".
    let mut opening = use_signal(|| None::<String>);
    let mut trouble = use_signal(|| None::<String>);
    let navigator = use_navigator();

    let state = (auth.state)();
    let shelf = use_resource(move || {
        let chosen = picked();
        let _ = reload();
        let signed_in = matches!((auth.state)(), AuthState::SignedIn(_));
        async move {
            if !signed_in {
                return Err(LibraryError::SignedOut);
            }
            shelf_for(chosen).await
        }
    });

    let open_chart = move |org: String, slug: String| {
        spawn(async move {
            opening.set(Some(format!("chart:{slug}")));
            trouble.set(None);
            match library::read_chart(&org, &slug).await {
                Ok(StoredChart { source, .. }) => {
                    navigator.push(Route::Chart {
                        data: chart_url::encode(&source),
                    });
                }
                Err(error) => trouble.set(Some(error.to_string())),
            }
            opening.set(None);
        });
    };

    let open_song = move |org: String, slug: String| {
        spawn(async move {
            opening.set(Some(format!("song:{slug}")));
            trouble.set(None);
            let opened = match library::list_charts(&org, Some(&slug)).await {
                Ok(charts) => match library::default_chart(&charts) {
                    Some(chart) => library::read_chart(&org, &chart.slug).await,
                    None => Err(LibraryError::Refused(format!(
                        "“{slug}” has no chart yet. Write one in the editor and save it."
                    ))),
                },
                Err(error) => Err(error),
            };
            match opened {
                Ok(StoredChart { source, .. }) => {
                    navigator.push(Route::Chart {
                        data: chart_url::encode(&source),
                    });
                }
                Err(error) => trouble.set(Some(error.to_string())),
            }
            opening.set(None);
        });
    };

    rsx! {
        Shell {
            section { class: "kf-prose kf-library",
                h1 { "Your library" }

                if let Some(message) = trouble() {
                    p { class: "kf-account-error", role: "alert", "{message}" }
                }

                match (&state, &*shelf.read_unchecked()) {
                    (AuthState::Loading, _) | (_, None) => rsx! {
                        p { class: "kf-note", "One moment…" }
                    },

                    (_, Some(Err(LibraryError::SignedOut))) => rsx! {
                        p {
                            "A FastTrackStudio account keeps your charts, and shows you the
                             songs and set lists of every workspace you are in. The editor
                             does not need one — a chart lives in its link — but a link is
                             not a shelf."
                        }
                        div { class: "kf-account-actions",
                            button {
                                class: "kf-account-submit",
                                disabled: (auth.pending)(),
                                onclick: move |_| auth.begin_sign_in(),
                                "Sign in"
                            }
                            Link { class: "kf-account-link", to: Route::Editor {},
                                "Back to the editor"
                            }
                        }
                    },

                    (_, Some(Err(error))) => rsx! {
                        p { role: "alert", "{error}" }
                        div { class: "kf-account-actions",
                            button {
                                class: "kf-button",
                                onclick: move |_| reload += 1,
                                "Try again"
                            }
                            Link { class: "kf-account-link", to: Route::Editor {},
                                "Back to the editor"
                            }
                        }
                    },

                    (_, Some(Ok(Shelf::Pick(orgs)))) => rsx! {
                        p { "You are in more than one workspace. Which library do you want?" }
                        div { class: "kf-save-orgs",
                            for org in orgs.clone() {
                                button {
                                    key: "{org.slug}",
                                    class: "kf-button",
                                    onclick: move |_| {
                                        library::remember_org(&org.slug);
                                        picked.set(Some(org.slug.clone()));
                                    },
                                    "{org.name}"
                                }
                            }
                        }
                    },

                    (_, Some(Ok(Shelf::Library { org, elsewhere, songlists, songs, charts }))) => {
                        let org = org.clone();
                        let elsewhere = elsewhere.clone();
                        let songlists = songlists.clone();
                        let charts = charts.clone();
                        let by_slug: HashMap<String, SongEntry> = songs
                            .iter()
                            .map(|song| (song.slug.clone(), song.clone()))
                            .collect();
                        let songs = songs.clone();
                        let empty = songlists.is_empty() && songs.is_empty() && charts.is_empty();
                        rsx! {
                            if elsewhere.len() > 1 {
                                p { class: "kf-library-where",
                                    label { r#for: "kf-library-org", "Workspace" }
                                    select {
                                        id: "kf-library-org",
                                        class: "kf-select",
                                        value: "{org}",
                                        onchange: move |e| {
                                            library::remember_org(&e.value());
                                            picked.set(Some(e.value()));
                                        },
                                        for choice in elsewhere.clone() {
                                            option { key: "{choice.slug}", value: "{choice.slug}",
                                                "{choice.name}"
                                            }
                                        }
                                    }
                                }
                            }

                            if empty {
                                p {
                                    "Nothing here yet. Write a chart and press Save above it —
                                     it will be waiting the next time you sign in."
                                }
                                Link { class: "kf-account-link", to: Route::Editor {},
                                    "Open the editor"
                                }
                            }

                            if !songlists.is_empty() {
                                div { class: "kf-library-section",
                                    h2 { "Song lists" }
                                    for list in songlists {
                                        details {
                                            key: "{list.id}",
                                            class: "kf-library-group",
                                            summary {
                                                "{list.title}"
                                                span { class: "kf-library-count",
                                                    {plural(list.songs.len(), "song")}
                                                }
                                            }
                                            ul { class: "kf-library-list",
                                                for row in rows_of(&list, &by_slug) {
                                                    match row {
                                                        ListRow::Song(song) => rsx! {
                                                            SongRow {
                                                                key: "{list.id}/{song.slug}",
                                                                song: song.clone(),
                                                                busy: opening() == Some(format!("song:{}", song.slug)),
                                                                on_open: {
                                                                    let org = org.clone();
                                                                    let slug = song.slug.clone();
                                                                    move |()| open_song(org.clone(), slug.clone())
                                                                },
                                                            }
                                                        },
                                                        ListRow::Missing(slug) => rsx! {
                                                            li { key: "{list.id}/{slug}", class: "kf-library-row",
                                                                div { class: "kf-library-what",
                                                                    span { class: "kf-library-title kf-library-missing",
                                                                        "{slug}"
                                                                    }
                                                                    span { class: "kf-library-meta",
                                                                        "not in this library any more"
                                                                    }
                                                                }
                                                            }
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if !songs.is_empty() {
                                div { class: "kf-library-section",
                                    h2 { "Songs" }
                                    ul { class: "kf-library-list",
                                        for song in songs {
                                            SongRow {
                                                key: "{song.slug}",
                                                song: song.clone(),
                                                busy: opening() == Some(format!("song:{}", song.slug)),
                                                on_open: {
                                                    let org = org.clone();
                                                    let slug = song.slug.clone();
                                                    move |()| open_song(org.clone(), slug.clone())
                                                },
                                            }
                                        }
                                    }
                                }
                            }

                            if !charts.is_empty() {
                                div { class: "kf-library-section",
                                    h2 { "Charts" }
                                    ul { class: "kf-library-list",
                                        for chart in charts {
                                            ChartRow {
                                                key: "{chart.slug}",
                                                chart: chart.clone(),
                                                org: org.clone(),
                                                busy: opening() == Some(format!("chart:{}", chart.slug)),
                                                on_open: {
                                                    let slug = chart.slug.clone();
                                                    let org = org.clone();
                                                    move |()| open_chart(org.clone(), slug.clone())
                                                },
                                                on_deleted: move |()| reload += 1,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `1 song`, `12 songs`.
fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// A song on the shelf: title, who wrote it, its key, and Open.
#[component]
fn SongRow(song: SongEntry, busy: bool, on_open: EventHandler<()>) -> Element {
    let meta: Vec<String> = [
        (!song.writers.is_empty()).then(|| song.writers.join(", ")),
        song.key.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    rsx! {
        li { class: "kf-library-row",
            div { class: "kf-library-what",
                span { class: "kf-library-title", "{song.title}" }
                if !meta.is_empty() {
                    span { class: "kf-library-meta", "{meta.join(\" · \")}" }
                }
            }
            div { class: "kf-library-actions",
                button {
                    class: "kf-button",
                    disabled: busy,
                    onclick: move |_| on_open.call(()),
                    if busy { "Opening…" } else { "Open" }
                }
            }
        }
    }
}

/// An unattached chart on the shelf: what it is, Open, and Remove
/// behind a confirmation.
#[component]
fn ChartRow(
    chart: ChartEntry,
    org: String,
    busy: bool,
    on_open: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut confirming = use_signal(|| false);
    let mut deleting = use_signal(|| false);
    let mut failed = use_signal(|| None::<String>);

    let meta: Vec<String> = [
        chart.key.clone(),
        (!chart.sections.is_empty()).then(|| chart.sections.join(" ")),
        chart.notation.clone().filter(|n| n != "keyflow"),
        chart.updated_at.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    rsx! {
        li { class: "kf-library-row",
            div { class: "kf-library-what",
                span { class: "kf-library-title", "{chart.title}" }
                if !meta.is_empty() {
                    span { class: "kf-library-meta", "{meta.join(\" · \")}" }
                }
                if let Some(message) = failed() {
                    span { class: "kf-account-error", role: "alert", "{message}" }
                }
            }
            div { class: "kf-library-actions",
                button {
                    class: "kf-button",
                    disabled: busy,
                    onclick: move |_| on_open.call(()),
                    if busy { "Opening…" } else { "Open" }
                }
                if confirming() {
                    button {
                        class: "kf-button kf-button-on",
                        disabled: deleting(),
                        onclick: {
                            let slug = chart.slug.clone();
                            let org = org.clone();
                            move |_| {
                                let slug = slug.clone();
                                let org = org.clone();
                                spawn(async move {
                                    deleting.set(true);
                                    match library::delete_chart(&org, &slug).await {
                                        Ok(()) => on_deleted.call(()),
                                        Err(error) => failed.set(Some(error.to_string())),
                                    }
                                    deleting.set(false);
                                    confirming.set(false);
                                });
                            }
                        },
                        if deleting() { "Removing…" } else { "Really remove" }
                    }
                    button {
                        class: "kf-button",
                        onclick: move |_| confirming.set(false),
                        "Keep"
                    }
                } else {
                    button {
                        class: "kf-button",
                        onclick: move |_| confirming.set(true),
                        "Remove"
                    }
                }
            }
        }
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
        }
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

    #[test]
    fn counts_read_as_english() {
        assert_eq!(plural(1, "song"), "1 song");
        assert_eq!(plural(0, "song"), "0 songs");
        assert_eq!(plural(199, "song"), "199 songs");
    }
}
