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
    let mut new_list = use_signal(String::new);
    let mut making_list = use_signal(|| false);
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

    // Every mutation of the shelf goes through here: run it, show what
    // went wrong if anything did, and reload the shelf either way — the
    // server is the truth about what is in a list.
    let mutate = move |what: String,
                       work: std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), LibraryError>>>,
    >| {
        spawn(async move {
            trouble.set(None);
            if let Err(error) = work.await {
                trouble.set(Some(format!("{what}: {error}")));
            }
            reload += 1;
        });
    };

    let open_song = move |org: String, slug: String| {
        spawn(async move {
            opening.set(Some(format!("song:{slug}")));
            trouble.set(None);
            match library::list_charts(&org, Some(&slug)).await {
                Ok(charts) => match library::default_chart(&charts) {
                    Some(chart) => {
                        navigator.push(Route::LibraryChart {
                            org,
                            slug: chart.slug.clone(),
                        });
                    }
                    None => trouble.set(Some(format!(
                        "“{slug}” has no chart yet. Write one in the editor and save it."
                    ))),
                },
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
                        let list_choices: Vec<(String, String)> = songlists
                            .iter()
                            .map(|list| (list.id.clone(), list.title.clone()))
                            .collect();
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
                                     it becomes a song in this library."
                                }
                                Link { class: "kf-account-link", to: Route::Editor {},
                                    "Open the editor"
                                }
                            }

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
                                        if list.songs.is_empty() {
                                            p { class: "kf-library-meta kf-library-empty",
                                                "Empty. Add songs from the list below."
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
                                                            lists: Vec::<(String, String)>::new(),
                                                            in_list: Some(list.title.clone()),
                                                            on_open: {
                                                                let org = org.clone();
                                                                let slug = song.slug.clone();
                                                                move |()| open_song(org.clone(), slug.clone())
                                                            },
                                                            on_add: move |_| {},
                                                            on_remove: {
                                                                let org = org.clone();
                                                                let list_id = list.id.clone();
                                                                let slug = song.slug.clone();
                                                                let title = song.title.clone();
                                                                move |()| {
                                                                    let (org, list_id, slug) = (org.clone(), list_id.clone(), slug.clone());
                                                                    mutate(format!("removing “{title}”"), Box::pin(async move {
                                                                        library::remove_from_songlist(&org, &list_id, &slug).await.map(|_| ())
                                                                    }));
                                                                }
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
                                                            div { class: "kf-library-actions",
                                                                button {
                                                                    class: "kf-button",
                                                                    onclick: {
                                                                        let org = org.clone();
                                                                        let list_id = list.id.clone();
                                                                        let slug = slug.clone();
                                                                        move |_| {
                                                                            let (org, list_id, slug) = (org.clone(), list_id.clone(), slug.clone());
                                                                            mutate(format!("removing “{slug}”"), Box::pin(async move {
                                                                                library::remove_from_songlist(&org, &list_id, &slug).await.map(|_| ())
                                                                            }));
                                                                        }
                                                                    },
                                                                    "Remove"
                                                                }
                                                            }
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                                form { class: "kf-library-new",
                                    onsubmit: {
                                        let org = org.clone();
                                        move |e: FormEvent| {
                                            e.prevent_default();
                                            let title = new_list().trim().to_owned();
                                            if title.is_empty() || making_list() {
                                                return;
                                            }
                                            making_list.set(true);
                                            let org = org.clone();
                                            spawn(async move {
                                                trouble.set(None);
                                                match library::create_songlist(&org, &title).await {
                                                    Ok(_) => new_list.set(String::new()),
                                                    Err(error) => trouble.set(Some(format!("making “{title}”: {error}"))),
                                                }
                                                making_list.set(false);
                                                reload += 1;
                                            });
                                        }
                                    },
                                    input {
                                        class: "kf-input",
                                        r#type: "text",
                                        placeholder: "New song list…",
                                        "aria-label": "New song list",
                                        value: "{new_list}",
                                        oninput: move |e| new_list.set(e.value()),
                                    }
                                    button {
                                        class: "kf-button",
                                        r#type: "submit",
                                        disabled: making_list() || new_list().trim().is_empty(),
                                        if making_list() { "Making…" } else { "Make list" }
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
                                                lists: list_choices.clone(),
                                                in_list: None,
                                                on_open: {
                                                    let org = org.clone();
                                                    let slug = song.slug.clone();
                                                    move |()| open_song(org.clone(), slug.clone())
                                                },
                                                on_add: {
                                                    let org = org.clone();
                                                    let slug = song.slug.clone();
                                                    let title = song.title.clone();
                                                    move |list_id: String| {
                                                        let (org, slug) = (org.clone(), slug.clone());
                                                        mutate(format!("adding “{title}”"), Box::pin(async move {
                                                            library::add_to_songlist(&org, &list_id, &slug).await.map(|_| ())
                                                        }));
                                                    }
                                                },
                                                on_remove: move |()| {},
                                            }
                                        }
                                    }
                                }
                            }

                            if !charts.is_empty() {
                                div { class: "kf-library-section",
                                    h2 { "Charts" }
                                    p { class: "kf-library-meta",
                                        "Charts saved before they had a song. Open one and save it to
                                         keep editing it here."
                                    }
                                    ul { class: "kf-library-list",
                                        for chart in charts {
                                            ChartRow {
                                                key: "{chart.slug}",
                                                chart: chart.clone(),
                                                org: org.clone(),
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

/// A song on the shelf: title, who wrote it, its key, and what can be
/// done with it — Open always; "Add to" a list when there are lists to
/// add to; Remove when the row is inside a list.
#[component]
fn SongRow(
    song: SongEntry,
    busy: bool,
    lists: Vec<(String, String)>,
    in_list: Option<String>,
    on_open: EventHandler<()>,
    on_add: EventHandler<String>,
    on_remove: EventHandler<()>,
) -> Element {
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
                if !lists.is_empty() {
                    select {
                        class: "kf-select",
                        "aria-label": "Add to a song list",
                        value: "",
                        onchange: move |e| {
                            let id = e.value();
                            if !id.is_empty() {
                                on_add.call(id);
                            }
                        },
                        option { value: "", "Add to…" }
                        for (id, title) in lists.clone() {
                            option { key: "{id}", value: "{id}", "{title}" }
                        }
                    }
                }
                if let Some(list) = in_list {
                    button {
                        class: "kf-button",
                        title: "Take this song out of “{list}”",
                        onclick: move |_| on_remove.call(()),
                        "Remove"
                    }
                }
            }
        }
    }
}

/// An unattached chart on the shelf: what it is, Open (bound, so a save
/// edits it), and Remove behind a confirmation.
#[component]
fn ChartRow(chart: ChartEntry, org: String, on_deleted: EventHandler<()>) -> Element {
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
                Link {
                    class: "kf-button",
                    to: Route::LibraryChart { org: org.clone(), slug: chart.slug.clone() },
                    "Open"
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
    fn counts_read_as_english() {
        assert_eq!(plural(1, "song"), "1 song");
        assert_eq!(plural(0, "song"), "0 songs");
        assert_eq!(plural(199, "song"), "199 songs");
    }
}
