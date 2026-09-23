//! `/library` — a workspace's repertoire, laid out the way the editor is:
//! a working surface, full width, not a page of prose.
//!
//! ```text
//! ┌ rail ────────────┬ list ─────────────────────┬ preview ──────────────┐
//! │ Rockstars of …   │ All songs       204 songs │ AFRICA        [Open]  │
//! │                  │ [search…………]  [Title ▾]  │ Toto · Standard Live  │
//! │ All songs    204 │ ☐ Song     Artist  Key    │ ┌───────────────────┐ │
//! │ Loose charts   2 │ ☐ 1979     Smashing …     │ │  engraved page 1  │ │
//! │ Song lists       │ ▌ AFRICA   Toto     A     │ │                   │ │
//! │   Adult Jam  204 │ ☐ …                       │ │  engraved page 2  │ │
//! │ [New list…]      │   [3 selected · Add to ▾] │ └───────────────────┘ │
//! └──────────────────┴───────────────────────────┴───────────────────────┘
//! ```
//!
//! The rail is where you are; the list is one thing at a time — the whole
//! library, one song list in running order, or the charts that have no
//! song; the preview is the chart of whichever row is picked, every page
//! of it, engraved. Picking a row shows its chart without leaving the
//! list, and ↑/↓ walks the list with the preview following, so reading
//! down a set is reading its charts. Below the width where three columns
//! fit, there is no preview, and picking a row opens its page instead.
//!
//! Musical facts — a key, a chart's sections — are set in the editor's
//! mono face and its syntax colours, so a key on the shelf is the same
//! blue it is in the source.
//!
//! Every behaviour decision (sorting, searching, what a move sends, what a
//! bulk add skips) lives in [`super::library`] as a tested function; this
//! file is the screen.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;

use dioxus::prelude::*;

use super::library::{
    ListRow, SORT_KEY, Shelf, SortBy, bulk_plan, bulk_summary, matches, move_plan, plural, rows_of,
    shelf_for, short_date, sort_songs, unattached,
};
use crate::Route;
use crate::auth::{AuthState, use_auth};
use crate::chart::{Chart, ChartFonts, ChartShape};
use crate::library::{self, ChartEntry, LibraryError, Org, SongEntry, SongList};
use crate::routes::Shell;

/// The width at which the preview column appears. Kept in step with the
/// `min-width` on `.kf-lib-preview` in `site.css`.
const PREVIEW_QUERY: &str = "(min-width: 75rem)";

/// Is the preview column on screen? When it is, picking a row previews
/// it; when it is not, picking a row has to go somewhere, and that is the
/// song's own page.
fn preview_showing() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.match_media(PREVIEW_QUERY).ok().flatten())
            .is_some_and(|m| m.matches())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = PREVIEW_QUERY;
        true
    }
}

/// The id of a row's pick button, so the keyboard can move focus to it.
/// Slugs are `[a-z0-9-]`, so they are safe in an id and in the selector
/// the script builds from it.
fn row_id(key: &str) -> String {
    format!("kf-pick-{key}")
}

/// Which part of the shelf the list shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum View {
    All,
    List(String),
    Loose,
}

/// The screen's state, as signals, so every pane can read and write it
/// without it being threaded through as a dozen props — and so none of it
/// resets when the shelf reloads underneath it after a change.
#[derive(Clone, Copy, PartialEq)]
struct Ui {
    reload: Signal<u32>,
    trouble: Signal<Option<String>>,
    notice: Signal<Option<String>>,
    view: Signal<View>,
    query: Signal<String>,
    sort: Signal<SortBy>,
    selected: Signal<BTreeSet<String>>,
    busy: Signal<bool>,
    /// The row the preview shows: a song slug in All songs and in a list,
    /// a chart slug among the loose charts. `None` is "the first row".
    focus: Signal<Option<String>>,
    /// That row once resolved against what the list is showing — the
    /// first row when nothing is picked. Rows highlight from this.
    shown: Signal<Option<String>>,
}

impl Ui {
    /// Run one change against the server, say what went wrong if anything
    /// did, and reload either way — the server is the truth about the
    /// shelf.
    fn run(self, what: String, work: impl Future<Output = Result<(), LibraryError>> + 'static) {
        let Self {
            mut reload,
            mut trouble,
            ..
        } = self;
        spawn(async move {
            trouble.set(None);
            if let Err(error) = work.await {
                trouble.set(Some(format!("{what}: {error}")));
            }
            *reload.write() += 1;
        });
    }

    fn show(mut self, view: View) {
        self.view.set(view);
        self.query.set(String::new());
        self.selected.set(BTreeSet::new());
        self.notice.set(None);
        self.focus.set(None);
    }

    /// A row was picked: preview it where there is room, else go to it.
    fn pick(mut self, key: String, elsewhere: Route) {
        if preview_showing() {
            self.focus.set(Some(key));
        } else {
            navigator().push(elsewhere);
        }
    }
}

/// ↑/↓ on a picked row: pick the next one, and move keyboard focus with
/// it so the next press carries on from there.
fn step_focus(mut ui: Ui, order: &[String], current: Option<&String>, key: &Key) -> bool {
    let down = match key {
        Key::ArrowDown => true,
        Key::ArrowUp => false,
        _ => return false,
    };
    let at = current.and_then(|c| order.iter().position(|k| k == c));
    let next = match (at, down) {
        (None, _) => order.first(),
        (Some(i), true) => order.get(i + 1),
        (Some(i), false) => i.checked_sub(1).and_then(|j| order.get(j)),
    };
    if let Some(next) = next {
        ui.focus.set(Some(next.clone()));
        document::eval(&format!(
            "document.getElementById('{}')?.focus({{preventScroll:false}});",
            row_id(next)
        ));
    }
    true
}

/// `/library`.
#[component]
pub fn Library() -> Element {
    let mut auth = use_auth();
    let mut picked = use_signal(|| None::<String>);
    let ui = Ui {
        reload: use_signal(|| 0),
        trouble: use_signal(|| None),
        notice: use_signal(|| None),
        view: use_signal(|| View::All),
        query: use_signal(String::new),
        sort: use_signal(|| {
            crate::prefs::string(SORT_KEY).map_or(SortBy::Title, |id| SortBy::from_id(&id))
        }),
        selected: use_signal(BTreeSet::new),
        busy: use_signal(|| false),
        focus: use_signal(|| None),
        shown: use_signal(|| None),
    };
    let mut reload = ui.reload;

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

    let body = match (&state, &*shelf.read_unchecked()) {
        (AuthState::Loading, _) | (_, None) => rsx! {
            div { class: "kf-lib-alone",
                p { class: "kf-lib-quiet", "Opening your library…" }
            }
        },

        (_, Some(Err(LibraryError::SignedOut))) => rsx! {
            div { class: "kf-lib-alone",
                h1 { "Your library" }
                p {
                    "Sign in to keep your charts, and to see the songs and set lists of every
                     workspace you are in. The editor works without an account — a chart lives
                     in its link."
                }
                div { class: "kf-lib-actions",
                    button {
                        class: "kf-button kf-button-primary",
                        disabled: (auth.pending)(),
                        onclick: move |_| auth.begin_sign_in(),
                        if (auth.pending)() {
                            "Taking you there…"
                        } else {
                            "Sign in"
                        }
                    }
                    Link { class: "kf-button", to: Route::Editor {}, "Open the editor" }
                }
            }
        },

        (_, Some(Err(error))) => rsx! {
            div { class: "kf-lib-alone",
                h1 { "Your library did not open" }
                p { role: "alert", "{error}" }
                div { class: "kf-lib-actions",
                    button {
                        class: "kf-button kf-button-primary",
                        onclick: move |_| reload += 1,
                        "Try again"
                    }
                    Link { class: "kf-button", to: Route::Editor {}, "Open the editor" }
                }
            }
        },

        (_, Some(Ok(Shelf::Pick(orgs)))) => rsx! {
            div { class: "kf-lib-alone",
                h1 { "Which library?" }
                p { "You are in more than one workspace. Pick one — you can switch any time." }
                div { class: "kf-lib-choices",
                    for org in orgs.clone() {
                        button {
                            key: "{org.slug}",
                            class: "kf-lib-choice",
                            onclick: move |_| {
                                library::remember_org(&org.slug);
                                picked.set(Some(org.slug.clone()));
                            },
                            "{org.name}"
                        }
                    }
                }
            }
        },

        (
            _,
            Some(Ok(Shelf::Library {
                org,
                elsewhere,
                songlists,
                songs,
                charts,
            })),
        ) => rsx! {
            Workspace {
                org: org.clone(),
                elsewhere: elsewhere.clone(),
                songlists: songlists.clone(),
                songs: songs.clone(),
                charts: charts.clone(),
                picked,
                ui,
            }
        },
    };

    rsx! {
        Shell {
            ChartFonts {}
            div { class: "kf-lib", {body} }
        }
    }
}

/// A song's charts, as the shelf needs them: every one, and which one is
/// "the chart".
#[derive(Clone, Debug, Default, PartialEq)]
struct ChartsOf {
    all: Vec<ChartEntry>,
    main: Option<String>,
}

fn charts_by_song(charts: &[ChartEntry]) -> HashMap<String, ChartsOf> {
    let mut grouped: HashMap<String, Vec<ChartEntry>> = HashMap::new();
    for chart in charts {
        if let Some(song) = &chart.song {
            grouped.entry(song.clone()).or_default().push(chart.clone());
        }
    }
    grouped
        .into_iter()
        .map(|(song, mut charts)| {
            // Main first, then as the server listed them: the order the
            // preview offers them in.
            charts.sort_by_key(|c| !c.is_default);
            let main = library::default_chart(&charts).map(|c| c.slug.clone());
            (song, ChartsOf { all: charts, main })
        })
        .collect()
}

/// The rows the list is showing, in order, by the key the preview uses.
/// Computed once here so the default preview, the keyboard and the list
/// itself agree about what "next" means.
fn shown_songs(songs: &[SongEntry], query: &str, by: SortBy) -> Vec<SongEntry> {
    let mut shown: Vec<SongEntry> = songs
        .iter()
        .filter(|s| matches(s, query))
        .cloned()
        .collect();
    sort_songs(&mut shown, by);
    shown
}

fn shown_rows(
    list: &SongList,
    by_slug: &HashMap<String, SongEntry>,
    query: &str,
) -> Vec<(usize, ListRow)> {
    rows_of(list, by_slug)
        .into_iter()
        .enumerate()
        .filter(|(_, row)| match row {
            ListRow::Song(song) => matches(song, query),
            ListRow::Missing(slug) => query.trim().is_empty() || slug.contains(query.trim()),
        })
        .collect()
}

#[component]
fn Workspace(
    org: String,
    elsewhere: Vec<Org>,
    songlists: Vec<SongList>,
    songs: Vec<SongEntry>,
    charts: Vec<ChartEntry>,
    picked: Signal<Option<String>>,
    ui: Ui,
) -> Element {
    let loose = unattached(charts.clone(), &songs);
    let of_song = charts_by_song(&charts);
    let by_slug: HashMap<String, SongEntry> = songs
        .iter()
        .map(|song| (song.slug.clone(), song.clone()))
        .collect();

    // A list that was deleted, or loose charts that were all filed, is
    // not somewhere to be left standing. Read, not written back: a write
    // during render would re-enter it.
    let view = match (ui.view)() {
        View::List(id) if !songlists.iter().any(|l| l.id == id) => View::All,
        View::Loose if loose.is_empty() => View::All,
        v => v,
    };
    let q = (ui.query)();

    // What the list shows, in order, keyed the way the preview is.
    let (pane, order): (Element, Vec<String>) = match &view {
        View::All => {
            let shown = shown_songs(&songs, &q, (ui.sort)());
            let order = shown.iter().map(|s| s.slug.clone()).collect();
            (
                rsx! {
                    AllSongs {
                        org: org.clone(),
                        songs: songs.clone(),
                        shown,
                        songlists: songlists.clone(),
                        of_song: of_song.clone(),
                        ui,
                    }
                },
                order,
            )
        }
        View::List(id) => {
            let list = songlists
                .iter()
                .find(|l| &l.id == id)
                .cloned()
                .expect("an unknown list falls back to All above");
            let rows = shown_rows(&list, &by_slug, &q);
            let order = rows
                .iter()
                .filter_map(|(_, row)| match row {
                    ListRow::Song(song) => Some(song.slug.clone()),
                    ListRow::Missing(_) => None,
                })
                .collect();
            (
                rsx! {
                    ListPane { key: "{list.id}", org: org.clone(), list, rows, ui }
                },
                order,
            )
        }
        View::Loose => (
            rsx! {
                LoosePane { org: org.clone(), charts: loose.clone(), ui }
            },
            loose.iter().map(|c| c.slug.clone()).collect(),
        ),
    };

    // The row on show: the one picked, while it is still in the list,
    // else the first.
    let focus = (ui.focus)()
        .filter(|k| order.contains(k))
        .or_else(|| order.first().cloned());

    let preview = match (&view, &focus) {
        (_, None) => rsx! {
            div { class: "kf-lib-preview-empty",
                p { "Nothing to show here yet." }
            }
        },
        (View::Loose, Some(slug)) => {
            let chart = loose.iter().find(|c| &c.slug == slug).cloned();
            match chart {
                Some(chart) => rsx! {
                    Preview {
                        key: "loose/{chart.slug}",
                        org: org.clone(),
                        title: chart.title.clone(),
                        by: String::new(),
                        song: None,
                        charts: vec![chart],
                    }
                },
                None => rsx! {},
            }
        }
        (_, Some(slug)) => {
            let song = by_slug.get(slug).cloned();
            match song {
                Some(song) => rsx! {
                    Preview {
                        key: "song/{song.slug}",
                        org: org.clone(),
                        title: song.title.clone(),
                        by: song.writers.join(", "),
                        song: Some(song.slug.clone()),
                        charts: of_song.get(&song.slug).map(|c| c.all.clone()).unwrap_or_default(),
                    }
                },
                None => rsx! {},
            }
        }
    };

    // Published after render rather than written during it, which would
    // re-enter the render that wrote it.
    let mut shown_signal = ui.shown;
    use_effect(use_reactive!(|(focus)| {
        if *shown_signal.peek() != focus {
            shown_signal.set(focus);
        }
    }));

    let keys = {
        let order = order.clone();
        let focus = focus.clone();
        move |e: KeyboardEvent| {
            if step_focus(ui, &order, focus.as_ref(), &e.key()) {
                e.prevent_default();
            }
        }
    };

    rsx! {
        Rail {
            org: org.clone(),
            current: view.clone(),
            elsewhere,
            songlists: songlists.clone(),
            song_count: songs.len(),
            loose_count: loose.len(),
            picked,
            ui,
        }
        section {
            class: "kf-lib-main",
            "data-focus": focus.clone().unwrap_or_default(),
            onkeydown: keys,
            if let Some(message) = (ui.trouble)() {
                div { class: "kf-lib-trouble", role: "alert",
                    span { "{message}" }
                    button {
                        class: "kf-lib-link",
                        onclick: move |_| {
                            let mut t = ui.trouble;
                            t.set(None);
                        },
                        "Dismiss"
                    }
                }
            }
            {pane}
        }
        aside { class: "kf-lib-preview", "aria-label": "Chart preview", {preview} }
    }
}

/// Is `key` the row on show in the preview?
fn use_focused(ui: Ui, key: &str) -> bool {
    (ui.shown)().as_deref() == Some(key)
}

// ── The rail ─────────────────────────────────────────────────────────

#[component]
fn Rail(
    org: String,
    current: View,
    elsewhere: Vec<Org>,
    songlists: Vec<SongList>,
    song_count: usize,
    loose_count: usize,
    picked: Signal<Option<String>>,
    ui: Ui,
) -> Element {
    let mut new_list = use_signal(String::new);
    let mut making = use_signal(|| false);
    let org_name = elsewhere
        .iter()
        .find(|o| o.slug == org)
        .map_or_else(|| org.clone(), |o| o.name.clone());

    let make_list = {
        let org = org.clone();
        move |e: FormEvent| {
            e.prevent_default();
            let title = new_list().trim().to_owned();
            if title.is_empty() || making() {
                return;
            }
            let org = org.clone();
            let mut trouble = ui.trouble;
            let mut reload = ui.reload;
            spawn(async move {
                making.set(true);
                trouble.set(None);
                match library::create_songlist(&org, &title).await {
                    Ok(list) => {
                        new_list.set(String::new());
                        ui.show(View::List(list.id));
                    }
                    Err(error) => trouble.set(Some(format!("making “{title}”: {error}"))),
                }
                making.set(false);
                *reload.write() += 1;
            });
        }
    };

    rsx! {
        aside { class: "kf-lib-rail",
            div { class: "kf-lib-org",
                if elsewhere.len() > 1 {
                    label { class: "kf-lib-org-hint", r#for: "kf-lib-org", "Workspace" }
                    select {
                        id: "kf-lib-org",
                        class: "kf-lib-org-select",
                        onchange: move |e| {
                            library::remember_org(&e.value());
                            ui.show(View::All);
                            picked.set(Some(e.value()));
                        },
                        for choice in elsewhere.clone() {
                            option {
                                key: "{choice.slug}",
                                value: "{choice.slug}",
                                selected: choice.slug == org,
                                "{choice.name}"
                            }
                        }
                    }
                } else {
                    span { class: "kf-lib-org-hint", "Workspace" }
                    span { class: "kf-lib-org-name", "{org_name}" }
                }
            }

            nav { class: "kf-lib-views", "aria-label": "Library",
                RailItem {
                    label: "All songs".to_owned(),
                    count: song_count,
                    on: current == View::All,
                    onpick: move |()| ui.show(View::All),
                }
                if loose_count > 0 {
                    RailItem {
                        label: "Loose charts".to_owned(),
                        count: loose_count,
                        on: current == View::Loose,
                        onpick: move |()| ui.show(View::Loose),
                    }
                }
                h2 { class: "kf-lib-views-head", "Song lists" }
                if songlists.is_empty() {
                    p { class: "kf-lib-views-empty", "None yet. Name one below." }
                }
                for list in songlists.clone() {
                    RailItem {
                        key: "{list.id}",
                        label: list.title.clone(),
                        count: list.songs.len(),
                        on: current == View::List(list.id.clone()),
                        onpick: {
                            let id = list.id.clone();
                            move |()| ui.show(View::List(id.clone()))
                        },
                    }
                }
            }

            form { class: "kf-lib-newlist", onsubmit: make_list,
                input {
                    class: "kf-lib-input",
                    r#type: "text",
                    placeholder: "New song list",
                    "aria-label": "New song list name",
                    value: "{new_list}",
                    oninput: move |e| new_list.set(e.value()),
                }
                button {
                    class: "kf-button",
                    r#type: "submit",
                    disabled: making() || new_list().trim().is_empty(),
                    if making() {
                        "Making…"
                    } else {
                        "Make"
                    }
                }
            }
        }
    }
}

#[component]
fn RailItem(label: String, count: usize, on: bool, onpick: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: if on { "kf-lib-view kf-lib-view-on" } else { "kf-lib-view" },
            "aria-current": if on { "page" } else { "false" },
            onclick: move |_| onpick.call(()),
            span { class: "kf-lib-view-name", "{label}" }
            span { class: "kf-lib-view-count", "{count}" }
        }
    }
}

// ── The list ─────────────────────────────────────────────────────────

/// The heading every pane opens with: what you are looking at, how much
/// of it there is, and the pane's own verbs on the right.
#[component]
fn PaneHead(title: Element, sub: String, #[props(default)] actions: Option<Element>) -> Element {
    rsx! {
        header { class: "kf-lib-head",
            div { class: "kf-lib-head-text",
                {title}
                p { class: "kf-lib-sub", "{sub}" }
            }
            if let Some(actions) = actions {
                div { class: "kf-lib-head-actions", {actions} }
            }
        }
    }
}

#[component]
fn SearchBox(ui: Ui, placeholder: String, #[props(default)] sortable: bool) -> Element {
    let mut query = ui.query;
    let mut sort = ui.sort;
    rsx! {
        div { class: "kf-lib-tools",
            input {
                class: "kf-lib-input kf-lib-search",
                r#type: "search",
                placeholder: "{placeholder}",
                "aria-label": "{placeholder}",
                value: "{query}",
                oninput: move |e| query.set(e.value()),
            }
            if sortable {
                label { class: "kf-lib-sort",
                    span { "Sort by" }
                    select {
                        class: "kf-lib-select",
                        onchange: move |e| {
                            let by = SortBy::from_id(&e.value());
                            crate::prefs::set_string(SORT_KEY, by.id());
                            sort.set(by);
                        },
                        for by in SortBy::ALL {
                            option {
                                key: "{by.id()}",
                                value: "{by.id()}",
                                selected: by == sort(),
                                "{by.label()}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A row's name, as the control that picks it for the preview. A button
/// rather than a link: on a wide screen it does not leave the page.
#[component]
fn PickName(key_id: String, title: String, by: String, to: Route, ui: Ui) -> Element {
    let on = use_focused(ui, &key_id);
    rsx! {
        button {
            id: row_id(&key_id),
            class: "kf-lib-song",
            "aria-pressed": if on { "true" } else { "false" },
            title: "Show the chart of “{title}”",
            onclick: {
                let (key_id, to) = (key_id.clone(), to.clone());
                move |_| ui.pick(key_id.clone(), to.clone())
            },
            span { class: "kf-lib-song-title", "{title}" }
            if !by.is_empty() {
                span { class: "kf-lib-song-artist kf-lib-artist-inline", "{by}" }
            }
        }
    }
}

#[component]
fn AllSongs(
    org: String,
    songs: Vec<SongEntry>,
    shown: Vec<SongEntry>,
    songlists: Vec<SongList>,
    of_song: HashMap<String, ChartsOf>,
    ui: Ui,
) -> Element {
    let q = (ui.query)();
    let by = (ui.sort)();
    let searching = !q.trim().is_empty();

    // In the order the shelf is sorted, so a batch lands in a list in the
    // order the person was looking at it — including picks a search has
    // since hidden.
    let picked: Vec<String> = {
        let chosen = (ui.selected)();
        let mut all = songs.clone();
        sort_songs(&mut all, by);
        all.into_iter()
            .map(|s| s.slug)
            .filter(|slug| chosen.contains(slug))
            .collect()
    };
    let all_shown_picked = !shown.is_empty() && shown.iter().all(|s| picked.contains(&s.slug));
    let show_changed = by == SortBy::Recent;

    let sub = if searching {
        format!("{} of {}", shown.len(), plural(songs.len(), "song"))
    } else {
        plural(songs.len(), "song")
    };

    rsx! {
        PaneHead {
            title: rsx! {
                h1 { "All songs" }
            },
            sub,
        }

        if songs.is_empty() {
            div { class: "kf-lib-empty",
                p {
                    "No songs yet. Write a chart in the editor and press Save — it becomes a song here."
                }
                Link {
                    class: "kf-button kf-button-primary",
                    to: Route::Editor {},
                    "Open the editor"
                }
            }
        } else {
            SearchBox {
                ui,
                placeholder: "Search by title or artist".to_owned(),
                sortable: true,
            }

            if shown.is_empty() {
                div { class: "kf-lib-empty",
                    p { "No song matches “{q}”." }
                    button {
                        class: "kf-button",
                        onclick: move |_| {
                            let mut query = ui.query;
                            query.set(String::new());
                        },
                        "Clear the search"
                    }
                }
            } else {
                table { class: "kf-lib-table",
                    thead {
                        tr {
                            th { class: "kf-lib-col-pick",
                                input {
                                    r#type: "checkbox",
                                    class: "kf-lib-check",
                                    "aria-label": if searching { "Select every song shown" } else { "Select every song" },
                                    checked: all_shown_picked,
                                    onchange: {
                                        let shown: Vec<String> = shown.iter().map(|s| s.slug.clone()).collect();
                                        move |e: FormEvent| {
                                            let on = e.checked();
                                            let (mut selected, mut notice) = (ui.selected, ui.notice);
                                            notice.set(None);
                                            selected
                                                .with_mut(|set| {
                                                    for slug in &shown {
                                                        if on {
                                                            set.insert(slug.clone());
                                                        } else {
                                                            set.remove(slug);
                                                        }
                                                    }
                                                });
                                        }
                                    },
                                }
                            }
                            th { class: "kf-lib-col-song", "Song" }
                            th { class: "kf-lib-col-artist", "Artist" }
                            th { class: "kf-lib-col-key", "Key" }
                            th { class: "kf-lib-col-num", "Charts" }
                            if show_changed {
                                th { class: "kf-lib-col-date", "Changed" }
                            }
                        }
                    }
                    tbody {
                        for song in shown.clone() {
                            SongLine {
                                key: "{song.slug}",
                                org: org.clone(),
                                charts: of_song.get(&song.slug).map(|c| c.all.len()).unwrap_or_default(),
                                picked: picked.contains(&song.slug),
                                show_changed,
                                song,
                                ui,
                            }
                        }
                    }
                }
            }
        }

        BulkBar { org: org.clone(), picked, songlists, ui }
    }
}

#[component]
fn SongLine(
    org: String,
    song: SongEntry,
    charts: usize,
    picked: bool,
    show_changed: bool,
    ui: Ui,
) -> Element {
    let artist = song.writers.join(", ");
    let on = use_focused(ui, &song.slug);
    let class = match (picked, on) {
        (_, true) => "kf-lib-row kf-lib-row-on",
        (true, false) => "kf-lib-row kf-lib-row-picked",
        (false, false) => "kf-lib-row",
    };
    rsx! {
        tr { class,
            td { class: "kf-lib-col-pick",
                input {
                    r#type: "checkbox",
                    class: "kf-lib-check",
                    "aria-label": "Select “{song.title}”",
                    checked: picked,
                    onchange: {
                        let slug = song.slug.clone();
                        move |e: FormEvent| {
                            let on = e.checked();
                            let (mut selected, mut notice) = (ui.selected, ui.notice);
                            notice.set(None);
                            selected
                                .with_mut(|set| {
                                    if on {
                                        set.insert(slug.clone());
                                    } else {
                                        set.remove(&slug);
                                    }
                                });
                        }
                    },
                }
            }
            td { class: "kf-lib-col-song",
                PickName {
                    key_id: song.slug.clone(),
                    title: song.title.clone(),
                    by: artist.clone(),
                    to: Route::LibrarySong {
                        org: org.clone(),
                        slug: song.slug.clone(),
                    },
                    ui,
                }
            }
            td { class: "kf-lib-col-artist", "{artist}" }
            td { class: "kf-lib-col-key",
                if let Some(key) = &song.key {
                    span { class: "kf-key", "{key}" }
                }
            }
            td { class: "kf-lib-col-num",
                span { class: if charts == 0 { "kf-lib-count kf-lib-count-none" } else { "kf-lib-count" },
                    "{charts}"
                }
            }
            if show_changed {
                td { class: "kf-lib-col-date",
                    {song.updated_at.as_deref().map(short_date).unwrap_or_default()}
                }
            }
        }
    }
}

/// What to do with the songs that are ticked. Appears only while some
/// are, pinned to the bottom of the list so it is in reach from any row.
#[component]
fn BulkBar(org: String, picked: Vec<String>, songlists: Vec<SongList>, ui: Ui) -> Element {
    let notice = (ui.notice)();
    if picked.is_empty() && notice.is_none() {
        return rsx! {};
    }
    let busy = (ui.busy)();

    let add_to = {
        let org = org.clone();
        let picked = picked.clone();
        let songlists = songlists.clone();
        move |e: FormEvent| {
            let Some(list) = songlists.iter().find(|l| l.id == e.value()).cloned() else {
                return;
            };
            let plan = bulk_plan(&picked, &list);
            let org = org.clone();
            let Ui {
                mut busy,
                mut notice,
                mut trouble,
                mut selected,
                mut reload,
                ..
            } = ui;
            spawn(async move {
                busy.set(true);
                trouble.set(None);
                notice.set(None);
                let mut added = 0_usize;
                let mut failed = Vec::new();
                // One at a time, in order: the list grows in the order the
                // songs were picked, and one connection is not flooded.
                for slug in &plan.to_add {
                    match library::add_to_songlist(&org, &list.id, slug).await {
                        Ok(_) => added += 1,
                        Err(error) => failed.push(format!("{slug}: {error}")),
                    }
                }
                notice.set(Some(bulk_summary(added, plan.already, &list.title)));
                if failed.is_empty() {
                    selected.set(BTreeSet::new());
                } else {
                    trouble.set(Some(format!(
                        "some songs were not added — {}",
                        failed.join("; ")
                    )));
                }
                busy.set(false);
                *reload.write() += 1;
            });
        }
    };

    rsx! {
        div {
            class: "kf-lib-bulk",
            role: "region",
            "aria-label": "Selected songs",
            if picked.is_empty() {
                if let Some(message) = notice {
                    span { class: "kf-lib-bulk-note", role: "status", "{message}" }
                    button {
                        class: "kf-lib-link",
                        onclick: move |_| {
                            let mut n = ui.notice;
                            n.set(None);
                        },
                        "OK"
                    }
                }
            } else {
                span { class: "kf-lib-bulk-count", "{plural(picked.len(), \"song\")} selected" }
                if songlists.is_empty() {
                    span { class: "kf-lib-bulk-note", "Make a song list in the sidebar to add them to it." }
                } else {
                    select {
                        class: "kf-lib-select kf-lib-bulk-add",
                        "aria-label": "Add the selected songs to a list",
                        disabled: busy,
                        onchange: add_to,
                        option { value: "", selected: true,
                            if busy {
                                "Adding…"
                            } else {
                                "Add to list…"
                            }
                        }
                        for list in songlists.clone() {
                            option { key: "{list.id}", value: "{list.id}", "{list.title}" }
                        }
                    }
                }
                button {
                    class: "kf-lib-link",
                    onclick: move |_| {
                        let mut s = ui.selected;
                        s.set(BTreeSet::new());
                    },
                    "Clear"
                }
            }
        }
    }
}

/// One song list, in running order.
#[component]
fn ListPane(org: String, list: SongList, rows: Vec<(usize, ListRow)>, ui: Ui) -> Element {
    // The title being edited, while it is. One editor at a time.
    let mut renaming = use_signal(|| None::<String>);
    let mut confirming = use_signal(|| false);
    let q = (ui.query)();
    let searching = !q.trim().is_empty();
    let total = list.songs.len();

    let title = match renaming() {
        Some(draft) => rsx! {
            form {
                class: "kf-lib-rename",
                onsubmit: {
                    let (org, id) = (org.clone(), list.id.clone());
                    move |e: FormEvent| {
                        e.prevent_default();
                        let title = draft_title(&renaming());
                        if title.is_empty() {
                            return;
                        }
                        renaming.set(None);
                        let (org, id) = (org.clone(), id.clone());
                        ui.run(
                            format!("renaming to “{title}”"),
                            async move { library::rename_songlist(&org, &id, &title).await.map(|_| ()) },
                        );
                    }
                },
                input {
                    class: "kf-lib-rename-input",
                    r#type: "text",
                    "aria-label": "List name",
                    autofocus: true,
                    value: "{draft}",
                    oninput: move |e| renaming.set(Some(e.value())),
                    onkeydown: move |e: KeyboardEvent| {
                        if e.key() == Key::Escape {
                            renaming.set(None);
                        }
                    },
                }
                button { class: "kf-button kf-button-primary", r#type: "submit", "Save name" }
                button {
                    class: "kf-button",
                    r#type: "button",
                    onclick: move |_| renaming.set(None),
                    "Cancel"
                }
            }
        },
        None => rsx! {
            h1 { "{list.title}" }
        },
    };

    let actions = if renaming().is_some() {
        None
    } else if confirming() {
        Some(rsx! {
            span { class: "kf-lib-confirm", "Delete this list? Its songs stay in the library." }
            button {
                class: "kf-button kf-button-danger",
                onclick: {
                    let (org, id) = (org.clone(), list.id.clone());
                    move |_| {
                        confirming.set(false);
                        let (org, id) = (org.clone(), id.clone());
                        ui.run(
                            "deleting the list".to_owned(),
                            async move { library::delete_songlist(&org, &id).await },
                        );
                    }
                },
                "Delete list"
            }
            button { class: "kf-button", onclick: move |_| confirming.set(false), "Keep it" }
        })
    } else {
        Some(rsx! {
            button {
                class: "kf-button",
                onclick: {
                    let title = list.title.clone();
                    move |_| renaming.set(Some(title.clone()))
                },
                "Rename"
            }
            button { class: "kf-button", onclick: move |_| confirming.set(true), "Delete list" }
        })
    };

    let sub = if searching {
        format!(
            "{} of {} in running order",
            rows.len(),
            plural(total, "song")
        )
    } else {
        format!("{} in running order", plural(total, "song"))
    };

    rsx! {
        PaneHead { title, sub, actions }

        if total == 0 {
            div { class: "kf-lib-empty",
                p {
                    "This list is empty. In All songs, tick the songs you want and choose Add to list."
                }
                button {
                    class: "kf-button kf-button-primary",
                    onclick: move |_| ui.show(View::All),
                    "Go to All songs"
                }
            }
        } else {
            SearchBox { ui, placeholder: "Find a song in this list".to_owned() }
            if rows.is_empty() {
                div { class: "kf-lib-empty",
                    p { "No song in this list matches “{q}”." }
                }
            }
            ol { class: "kf-lib-order",
                for (at , row) in rows {
                    match row {
                        ListRow::Song(song) => rsx! {
                            OrderLine {
                                key: "{song.slug}",
                                org: org.clone(),
                                list: list.clone(),
                                at,
                                song,
                                ui,
                            }
                        },
                        ListRow::Missing(slug) => rsx! {
                            li { key: "{slug}", class: "kf-lib-line kf-lib-line-missing",
                                span { class: "kf-lib-place", "{at + 1}" }
                                div { class: "kf-lib-song kf-lib-song-static",
                                    span { class: "kf-lib-song-title", "{slug}" }
                                    span { class: "kf-lib-song-artist", "No longer in the library" }
                                }
                                span {}
                                div { class: "kf-lib-line-actions",
                                    button {
                                        class: "kf-lib-link kf-lib-takeout",
                                        onclick: {
                                            let (org, id, slug) = (org.clone(), list.id.clone(), slug.clone());
                                            move |_| {
                                                let (org, id, slug) = (org.clone(), id.clone(), slug.clone());
                                                ui.run(
                                                    format!("removing “{slug}”"),
                                                    async move {
                                                        library::remove_from_songlist(&org, &id, &slug).await.map(|_| ())
                                                    },
                                                );
                                            }
                                        },
                                        "Take out"
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

fn draft_title(draft: &Option<String>) -> String {
    draft.as_deref().unwrap_or_default().trim().to_owned()
}

#[component]
fn OrderLine(org: String, list: SongList, at: usize, song: SongEntry, ui: Ui) -> Element {
    let artist = song.writers.join(", ");
    let on = use_focused(ui, &song.slug);
    let last = list.songs.len().saturating_sub(1);
    let step = {
        let (org, list, slug, title) = (
            org.clone(),
            list.clone(),
            song.slug.clone(),
            song.title.clone(),
        );
        move |up: bool| {
            let Some((moving, after)) = move_plan(&list.songs, &slug, up) else {
                return;
            };
            let (org, id) = (org.clone(), list.id.clone());
            ui.run(format!("moving “{title}”"), async move {
                library::move_in_songlist(&org, &id, &moving, Some(&after))
                    .await
                    .map(|_| ())
            });
        }
    };
    let (step_up, step_down) = (step.clone(), step);

    rsx! {
        li { class: if on { "kf-lib-line kf-lib-line-on" } else { "kf-lib-line" },
            span { class: "kf-lib-place", "{at + 1}" }
            PickName {
                key_id: song.slug.clone(),
                title: song.title.clone(),
                by: artist.clone(),
                to: Route::LibrarySong {
                    org: org.clone(),
                    slug: song.slug.clone(),
                },
                ui,
            }
            span { class: "kf-lib-line-key",
                if let Some(key) = &song.key {
                    span { class: "kf-key", "{key}" }
                }
            }
            div { class: "kf-lib-line-actions",
                button {
                    class: "kf-lib-step",
                    disabled: at == 0,
                    title: "Move up",
                    "aria-label": "Move “{song.title}” up",
                    onclick: move |_| step_up(true),
                    "↑"
                }
                button {
                    class: "kf-lib-step",
                    disabled: at >= last,
                    title: "Move down",
                    "aria-label": "Move “{song.title}” down",
                    onclick: move |_| step_down(false),
                    "↓"
                }
                button {
                    class: "kf-lib-link kf-lib-takeout",
                    title: "Take “{song.title}” out of this list. It stays in the library.",
                    onclick: {
                        let (org, id, slug, title) = (
                            org.clone(),
                            list.id.clone(),
                            song.slug.clone(),
                            song.title.clone(),
                        );
                        move |_| {
                            let (org, id, slug) = (org.clone(), id.clone(), slug.clone());
                            ui.run(
                                format!("taking out “{title}”"),
                                async move {
                                    library::remove_from_songlist(&org, &id, &slug).await.map(|_| ())
                                },
                            );
                        }
                    },
                    "Take out"
                }
            }
        }
    }
}

/// Charts with no song: saved before there was one, or left when their
/// song was removed.
#[component]
fn LoosePane(org: String, charts: Vec<ChartEntry>, ui: Ui) -> Element {
    let confirming = use_signal(|| None::<String>);
    rsx! {
        PaneHead {
            title: rsx! {
                h1 { "Loose charts" }
            },
            sub: format!(
                "{} without a song — saved before there was one, or left when their song was removed",
                plural(charts.len(), "chart"),
            ),
        }
        ul { class: "kf-lib-order",
            for chart in charts {
                LooseLine {
                    key: "{chart.slug}",
                    org: org.clone(),
                    chart,
                    confirming,
                    ui,
                }
            }
        }
    }
}

#[component]
fn LooseLine(
    org: String,
    chart: ChartEntry,
    confirming: Signal<Option<String>>,
    ui: Ui,
) -> Element {
    let on = use_focused(ui, &chart.slug);
    let by = [
        (!chart.sections.is_empty()).then(|| chart.sections.join(" ")),
        chart.updated_at.as_deref().map(short_date),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("  ");
    rsx! {
        li { class: if on { "kf-lib-line kf-lib-line-loose kf-lib-line-on" } else { "kf-lib-line kf-lib-line-loose" },
            PickName {
                key_id: chart.slug.clone(),
                title: chart.title.clone(),
                by,
                to: Route::LibraryChart {
                    org: org.clone(),
                    slug: chart.slug.clone(),
                },
                ui,
            }
            span { class: "kf-lib-line-key",
                if let Some(key) = &chart.key {
                    span { class: "kf-key", "{key}" }
                }
            }
            div { class: "kf-lib-line-actions",
                if confirming() == Some(chart.slug.clone()) {
                    button {
                        class: "kf-button kf-button-danger",
                        onclick: {
                            let (org, slug) = (org.clone(), chart.slug.clone());
                            move |_| {
                                confirming.set(None);
                                let (org, slug) = (org.clone(), slug.clone());
                                ui.run(
                                    "deleting the chart".to_owned(),
                                    async move { library::delete_chart(&org, &slug).await },
                                );
                            }
                        },
                        "Delete chart"
                    }
                    button { class: "kf-button", onclick: move |_| confirming.set(None), "Keep" }
                } else {
                    button {
                        class: "kf-lib-link kf-lib-takeout",
                        onclick: {
                            let slug = chart.slug.clone();
                            move |_| confirming.set(Some(slug.clone()))
                        },
                        "Delete"
                    }
                }
            }
        }
    }
}

// ── The preview ──────────────────────────────────────────────────────

/// What an arrangement is called in the preview: its label, or
/// "Standard" for the unlabelled reading of a song.
fn arrangement_name(chart: &ChartEntry) -> String {
    chart
        .arrangement
        .clone()
        .unwrap_or_else(|| "Standard".to_owned())
}

/// The picked row's chart, every page of it. Keyed by the row, so the
/// arrangement picked inside it resets when the row changes.
#[component]
fn Preview(
    org: String,
    title: String,
    by: String,
    song: Option<String>,
    charts: Vec<ChartEntry>,
) -> Element {
    let main = library::default_chart(&charts).map(|c| c.slug.clone());
    let mut chosen = use_signal(|| None::<String>);
    // A new row's chart starts at its first page, not wherever the last
    // one was scrolled to. Runs once per row: the component is keyed.
    use_effect(|| {
        document::eval("document.querySelector('.kf-lib-preview-paper')?.scrollTo(0, 0);");
    });
    let current = chosen()
        .filter(|slug| charts.iter().any(|c| &c.slug == slug))
        .or(main);

    let doc = {
        let org = org.clone();
        use_resource(use_reactive!(|(current)| {
            let org = org.clone();
            async move {
                match current {
                    Some(slug) => library::read_chart(&org, &slug).await.map(Some),
                    None => Ok(None),
                }
            }
        }))
    };

    let open = current.clone().map(|slug| Route::LibraryChart {
        org: org.clone(),
        slug,
    });
    let page = song.clone().map(|slug| Route::LibrarySong {
        org: org.clone(),
        slug,
    });

    rsx! {
        header { class: "kf-lib-preview-head",
            div { class: "kf-lib-preview-what",
                h2 { class: "kf-lib-preview-title", "{title}" }
                if !by.is_empty() {
                    p { class: "kf-lib-preview-by", "{by}" }
                }
            }
            div { class: "kf-lib-preview-actions",
                if let Some(page) = page {
                    Link { class: "kf-button", to: page, "Song page" }
                }
                if let Some(open) = open {
                    Link { class: "kf-button kf-button-primary", to: open, "Open in editor" }
                }
            }
        }
        if charts.len() > 1 {
            div {
                class: "kf-lib-preview-arrs",
                role: "group",
                "aria-label": "Arrangement",
                for chart in charts.clone() {
                    button {
                        key: "{chart.slug}",
                        class: if Some(&chart.slug) == current.as_ref() { "kf-lib-arr kf-lib-arr-on" } else { "kf-lib-arr" },
                        "aria-pressed": if Some(&chart.slug) == current.as_ref() { "true" } else { "false" },
                        onclick: {
                            let slug = chart.slug.clone();
                            move |_| chosen.set(Some(slug.clone()))
                        },
                        "{arrangement_name(&chart)}"
                        if chart.is_default {
                            span { class: "kf-lib-arr-main", "Main" }
                        }
                    }
                }
            }
        }
        div { class: "kf-lib-preview-paper",
            match (&current, &*doc.read_unchecked()) {
                (None, _) => rsx! {
                    div { class: "kf-lib-preview-blank",
                        p { "No chart of this song yet." }
                        if let Some(slug) = &song {
                            Link {
                                class: "kf-button",
                                to: Route::LibrarySong {
                                    org: org.clone(),
                                    slug: slug.clone(),
                                },
                                "Write one"
                            }
                        }
                    }
                },
                (_, None) => rsx! {
                    div { class: "kf-lib-preview-blank",
                        p { "Engraving…" }
                    }
                },
                (_, Some(Err(error))) => rsx! {
                    div { class: "kf-lib-preview-blank", role: "alert",
                        p { "{error}" }
                    }
                },
                (_, Some(Ok(None))) => rsx! {},
                (_, Some(Ok(Some(stored)))) => rsx! {
                    Chart { source: stored.source.clone(), shape: ChartShape::Page }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(slug: &str, song: Option<&str>, is_default: bool) -> ChartEntry {
        ChartEntry {
            slug: slug.to_owned(),
            title: slug.to_owned(),
            key: None,
            notation: None,
            sections: Vec::new(),
            song: song.map(str::to_owned),
            arrangement: None,
            is_default,
            updated_at: None,
        }
    }

    /// The preview offers a song's charts main-first, and opens the main
    /// one, without a call per row.
    #[test]
    fn each_song_knows_its_charts_main_first() {
        let charts = [
            chart("africa", Some("africa"), false),
            chart("africa-live", Some("africa"), true),
            chart("creep", Some("creep"), false),
            chart("loose", None, false),
        ];
        let of = charts_by_song(&charts);
        let africa: Vec<&str> = of["africa"].all.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(africa, ["africa-live", "africa"]);
        assert_eq!(of["africa"].main.as_deref(), Some("africa-live"));
        assert_eq!(
            of["creep"].main.as_deref(),
            Some("creep"),
            "unflagged: the first"
        );
        assert!(!of.contains_key("loose"));
    }

    /// The ids the keyboard focuses are the ids the rows carry.
    #[test]
    fn a_row_id_is_the_key_prefixed() {
        assert_eq!(row_id("africa"), "kf-pick-africa");
    }
}
