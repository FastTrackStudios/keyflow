//! `/library/:org/song/:slug` — one song, and everything hung on it.
//!
//! A song on the shelf is a title and a credit; what a band actually
//! reaches for is one of its **charts** — the default one, an acoustic
//! arrangement in G, the condensed live cut. This page is where those
//! sit side by side, where the song's own facts (title, writers, usual
//! key) are corrected, and where it is put into or seen in song lists.
//!
//! Nothing here is new server surface: the song is `song`/`upsert_song`,
//! its arrangements are `list_charts` filtered to it, and a new
//! arrangement is an `upsert_chart` naming the song — the same calls the
//! shelf and the editor already make.

use dioxus::prelude::*;

use crate::Route;
use crate::auth::{AuthState, use_auth};
use crate::chart::{Chart, ChartFonts, ChartShape};
use crate::library::{self, ChartEntry, Draft, LibraryError, SongEntry, SongList};
use crate::routes::Shell;
use crate::routes::library::short_date;

/// Everything the page shows, fetched together.
#[derive(Clone, Debug, PartialEq)]
struct SongView {
    song: SongEntry,
    charts: Vec<ChartEntry>,
    lists: Vec<SongList>,
}

async fn load(org: &str, slug: &str) -> Result<SongView, LibraryError> {
    let song = library::read_song(org, slug).await?;
    let charts = library::list_charts(org, Some(slug)).await?;
    let lists = library::list_songlists(org).await?;
    Ok(SongView {
        song,
        charts,
        lists,
    })
}

/// The slug a new arrangement is saved under.
///
/// Explicit rather than derived: the server derives a chart slug from
/// its title, and a copy of the default chart has the default chart's
/// title — so a derived slug would be the *same* chart, and "add an
/// arrangement" would silently overwrite the one being copied.
#[must_use]
pub fn arrangement_slug(song: &str, arrangement: &str, taken: &[String]) -> String {
    let words: String = arrangement
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let words = words
        .split('-')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if words.is_empty() {
        format!("{song}-arrangement")
    } else {
        format!("{song}-{words}")
    };
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unbounded range finds a free slug")
}

/// The source a song's first chart starts from: its title and key, and
/// nothing else — enough that the editor opens on something that is
/// recognisably this song.
#[must_use]
pub fn starter_source(song: &SongEntry) -> String {
    let credit = if song.writers.is_empty() {
        String::new()
    } else {
        format!(" - {}", song.writers.join(", "))
    };
    match &song.key {
        Some(key) => format!("{}{credit}\n4/4 #{key}\n\n", song.title),
        None => format!("{}{credit}\n4/4\n\n", song.title),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Edit {
    Closed,
    Open,
    Saving,
    Failed(String),
}

/// What an arrangement is called on the page. A chart with no label is
/// the song's standard reading, and says so rather than repeating the
/// song's title back.
fn arrangement_name(chart: &ChartEntry) -> String {
    chart
        .arrangement
        .clone()
        .unwrap_or_else(|| "Standard".to_owned())
}

#[component]
pub fn LibrarySong(org: String, slug: String) -> Element {
    let auth = use_auth();
    let mut reload = use_signal(|| 0_u32);
    let mut trouble = use_signal(|| None::<String>);
    let navigator = use_navigator();

    let view = {
        let org = org.clone();
        let slug = slug.clone();
        use_resource(move || {
            let org = org.clone();
            let slug = slug.clone();
            let _ = reload();
            let signed_in = matches!((auth.state)(), AuthState::SignedIn(_));
            async move {
                if !signed_in {
                    return Err(LibraryError::SignedOut);
                }
                load(&org, &slug).await
            }
        })
    };

    let body = match (&(auth.state)(), &*view.read_unchecked()) {
        (AuthState::Loading, _) | (_, None) => rsx! {
            div { class: "kf-lib-alone", p { class: "kf-lib-quiet", "Opening the song…" } }
        },
        (_, Some(Err(error))) => rsx! {
            div { class: "kf-lib-alone",
                h1 { "That song did not open" }
                p { role: "alert", "{error}" }
                div { class: "kf-lib-actions",
                    Link { class: "kf-button", to: Route::Library {}, "Back to the library" }
                }
            }
        },
        (_, Some(Ok(view))) => {
            let view = view.clone();
            rsx! {
                SongBody {
                    key: "{view.song.slug}",
                    org: org.clone(),
                    view,
                    trouble: trouble(),
                    on_changed: move |()| reload += 1,
                    on_trouble: move |message: Option<String>| trouble.set(message),
                    on_deleted: move |()| { navigator.push(Route::Library {}); },
                }
            }
        }
    };

    rsx! {
        Shell {
            ChartFonts {}
            {body}
        }
    }
}

#[component]
fn SongBody(
    org: String,
    view: SongView,
    trouble: Option<String>,
    on_changed: EventHandler<()>,
    on_trouble: EventHandler<Option<String>>,
    on_deleted: EventHandler<()>,
) -> Element {
    let song = view.song.clone();
    let main = library::default_chart(&view.charts).map(|c| c.slug.clone());
    // The arrangement on the paper. The main one until another is picked.
    let mut showing = use_signal(|| main.clone());
    // A pick that no longer exists (just deleted) falls back to the main.
    let current = showing()
        .filter(|slug| view.charts.iter().any(|c| &c.slug == slug))
        .or_else(|| main.clone());

    let mut title = use_signal(|| song.title.clone());
    let mut writers = use_signal(|| song.writers.join(", "));
    let mut song_key = use_signal(|| song.key.clone().unwrap_or_default());
    let mut edit = use_signal(|| Edit::Closed);
    let mut arrangement = use_signal(String::new);
    let mut adding = use_signal(|| false);
    let mut removing = use_signal(|| false);
    // The chart whose Delete has been pressed once, awaiting the second.
    let mut deleting = use_signal(|| None::<String>);
    let navigator = use_navigator();

    // The paper: the chosen chart's source, engraved. Read on its own so
    // switching arrangements redraws the page and nothing else.
    let paper = {
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

    let in_lists: Vec<String> = view
        .lists
        .iter()
        .filter(|list| list.songs.contains(&song.slug))
        .map(|list| list.title.clone())
        .collect();
    let other_lists: Vec<(String, String)> = view
        .lists
        .iter()
        .filter(|list| !list.songs.contains(&song.slug))
        .map(|list| (list.id.clone(), list.title.clone()))
        .collect();
    let chart_slugs: Vec<String> = view.charts.iter().map(|c| c.slug.clone()).collect();

    let save = {
        let org = org.clone();
        let song = song.clone();
        move |e: FormEvent| {
            e.prevent_default();
            let next = SongEntry {
                title: title().trim().to_owned(),
                writers: library::writers_from(&writers()),
                key: Some(song_key().trim().to_owned()).filter(|k| !k.is_empty()),
                ..song.clone()
            };
            if next.title.is_empty() {
                edit.set(Edit::Failed("A song needs a title.".to_owned()));
                return;
            }
            let org = org.clone();
            spawn(async move {
                edit.set(Edit::Saving);
                match library::save_song(&org, &next).await {
                    Ok(()) => {
                        edit.set(Edit::Closed);
                        on_changed.call(());
                    }
                    Err(error) => edit.set(Edit::Failed(error.to_string())),
                }
            });
        }
    };

    let add_arrangement = {
        let org = org.clone();
        let song = song.clone();
        let template = main.clone();
        move |e: FormEvent| {
            e.prevent_default();
            if adding() {
                return;
            }
            let label = arrangement().trim().to_owned();
            let (org, song, template, taken) = (
                org.clone(),
                song.clone(),
                template.clone(),
                chart_slugs.clone(),
            );
            spawn(async move {
                adding.set(true);
                on_trouble.call(None);
                // Start from the song's main chart when it has one — an
                // arrangement is usually the chart, changed — else from a
                // blank chart of this song.
                let source = match &template {
                    Some(slug) => match library::read_chart(&org, slug).await {
                        Ok(chart) => chart.source,
                        Err(error) => {
                            on_trouble.call(Some(format!("reading the chart to copy: {error}")));
                            adding.set(false);
                            return;
                        }
                    },
                    None => starter_source(&song),
                };
                let mut draft: Draft = library::draft_from_source(&source);
                draft.org = Some(org.clone());
                draft.song = Some(song.slug.clone());
                draft.slug = Some(arrangement_slug(&song.slug, &label, &taken));
                draft.arrangement = Some(label).filter(|l| !l.is_empty());
                match library::save_chart(&draft).await {
                    Ok(saved) => {
                        arrangement.set(String::new());
                        navigator.push(Route::LibraryChart {
                            org,
                            slug: saved.slug,
                        });
                    }
                    Err(error) => on_trouble.call(Some(format!("adding the chart: {error}"))),
                }
                adding.set(false);
            });
        }
    };

    let artist = song.writers.join(", ");
    let shown = view
        .charts
        .iter()
        .find(|c| Some(&c.slug) == current.as_ref())
        .cloned();

    rsx! {
        div { class: "kf-song",
            div { class: "kf-song-side",
                nav { class: "kf-song-crumb", "aria-label": "Breadcrumb",
                    Link { to: Route::Library {}, "Library" }
                }

                header { class: "kf-song-head",
                    h1 { "{song.title}" }
                    if !artist.is_empty() || song.key.is_some() {
                        p { class: "kf-song-by",
                            if !artist.is_empty() { span { "{artist}" } }
                            if let Some(key) = &song.key {
                                span { class: "kf-key", title: "Usual key", "{key}" }
                            }
                        }
                    }
                    if in_lists.is_empty() {
                        p { class: "kf-song-lists kf-song-lists-none", "Not in a song list yet" }
                    } else {
                        p { class: "kf-song-lists",
                            span { "In " }
                            for (i, name) in in_lists.iter().enumerate() {
                                if i > 0 { span { ", " } }
                                span { class: "kf-song-list-name", "{name}" }
                            }
                        }
                    }
                }

                if let Some(message) = &trouble {
                    div { class: "kf-lib-trouble", role: "alert",
                        span { "{message}" }
                        button { class: "kf-lib-link", onclick: move |_| on_trouble.call(None), "Dismiss" }
                    }
                }

                section { class: "kf-song-block", "aria-labelledby": "kf-song-arr",
                    h2 { id: "kf-song-arr", "Arrangements" }
                    if view.charts.is_empty() {
                        p { class: "kf-lib-quiet", "No chart of this song yet." }
                    }
                    ul { class: "kf-song-arrs",
                        for chart in view.charts.clone() {
                            li { key: "{chart.slug}",
                                class: if Some(&chart.slug) == current.as_ref() { "kf-song-arr kf-song-arr-on" } else { "kf-song-arr" },
                                button {
                                    class: "kf-song-arr-pick",
                                    "aria-pressed": if Some(&chart.slug) == current.as_ref() { "true" } else { "false" },
                                    onclick: {
                                        let slug = chart.slug.clone();
                                        move |_| { deleting.set(None); showing.set(Some(slug.clone())); }
                                    },
                                    span { class: "kf-song-arr-name",
                                        "{arrangement_name(&chart)}"
                                        if chart.is_default {
                                            span { class: "kf-song-main", "Main" }
                                        }
                                    }
                                    span { class: "kf-song-arr-meta",
                                        if let Some(key) = &chart.key {
                                            span { class: "kf-key", "{key}" }
                                        }
                                        if let Some(at) = &chart.updated_at {
                                            span { class: "kf-lib-date", "Changed {short_date(at)}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    form { class: "kf-song-add", onsubmit: add_arrangement,
                        input {
                            class: "kf-lib-input",
                            r#type: "text",
                            placeholder: if view.charts.is_empty() { "Name it (optional)" } else { "New arrangement, e.g. Acoustic in G" },
                            "aria-label": "Arrangement name",
                            value: "{arrangement}",
                            oninput: move |e| arrangement.set(e.value()),
                        }
                        button {
                            class: "kf-button",
                            r#type: "submit",
                            disabled: adding(),
                            if adding() { "Making…" } else if view.charts.is_empty() { "Write a chart" } else { "Add" }
                        }
                    }
                    if !view.charts.is_empty() {
                        p { class: "kf-song-hint",
                            "A new arrangement starts as a copy of the main chart."
                        }
                    }
                }

                if !other_lists.is_empty() {
                    section { class: "kf-song-block", "aria-labelledby": "kf-song-lists",
                        h2 { id: "kf-song-lists", "Add to a song list" }
                        select {
                            class: "kf-lib-select",
                            "aria-label": "Add to a song list",
                            onchange: {
                                let org = org.clone();
                                let slug = song.slug.clone();
                                move |e: FormEvent| {
                                    let list = e.value();
                                    if list.is_empty() {
                                        return;
                                    }
                                    let (org, slug) = (org.clone(), slug.clone());
                                    spawn(async move {
                                        on_trouble.call(None);
                                        if let Err(error) = library::add_to_songlist(&org, &list, &slug).await {
                                            on_trouble.call(Some(format!("adding to the list: {error}")));
                                        }
                                        on_changed.call(());
                                    });
                                }
                            },
                            option { value: "", selected: true, "Choose a list…" }
                            for (id, name) in other_lists {
                                option { key: "{id}", value: "{id}", "{name}" }
                            }
                        }
                    }
                }

                section { class: "kf-song-block", "aria-labelledby": "kf-song-details",
                    div { class: "kf-song-block-head",
                        h2 { id: "kf-song-details", "Details" }
                        if edit() == Edit::Closed {
                            button { class: "kf-lib-link", onclick: move |_| edit.set(Edit::Open), "Edit" }
                        }
                    }
                    if edit() == Edit::Closed {
                        dl { class: "kf-song-facts",
                            dt { "Title" } dd { "{song.title}" }
                            dt { "Artist" } dd { if artist.is_empty() { span { class: "kf-lib-quiet", "Not set" } } else { "{artist}" } }
                            dt { "Usual key" } dd {
                                match &song.key {
                                    Some(key) => rsx! { span { class: "kf-key", "{key}" } },
                                    None => rsx! { span { class: "kf-lib-quiet", "Not set" } },
                                }
                            }
                        }
                    } else {
                        form { class: "kf-song-form", onsubmit: save,
                            label { r#for: "kf-song-title", "Title" }
                            input {
                                id: "kf-song-title",
                                class: "kf-lib-input",
                                r#type: "text",
                                value: "{title}",
                                oninput: move |e| title.set(e.value()),
                            }
                            label { r#for: "kf-song-writers", "Artist" }
                            input {
                                id: "kf-song-writers",
                                class: "kf-lib-input",
                                r#type: "text",
                                placeholder: "Separate names with commas",
                                value: "{writers}",
                                oninput: move |e| writers.set(e.value()),
                            }
                            label { r#for: "kf-song-key", "Usual key" }
                            input {
                                id: "kf-song-key",
                                class: "kf-lib-input kf-song-key-input",
                                r#type: "text",
                                placeholder: "G, F#m, Bb",
                                value: "{song_key}",
                                oninput: move |e| song_key.set(e.value()),
                            }
                            div { class: "kf-song-form-actions",
                                button {
                                    class: "kf-button kf-button-primary",
                                    r#type: "submit",
                                    disabled: edit() == Edit::Saving,
                                    if edit() == Edit::Saving { "Saving…" } else { "Save details" }
                                }
                                button {
                                    class: "kf-button",
                                    r#type: "button",
                                    onclick: {
                                        let song = song.clone();
                                        move |_| {
                                            title.set(song.title.clone());
                                            writers.set(song.writers.join(", "));
                                            song_key.set(song.key.clone().unwrap_or_default());
                                            edit.set(Edit::Closed);
                                        }
                                    },
                                    "Cancel"
                                }
                            }
                            if let Edit::Failed(message) = edit() {
                                p { class: "kf-lib-error", role: "alert", "{message}" }
                            }
                        }
                    }
                }

                div { class: "kf-song-remove",
                    if removing() {
                        p { "Remove “{song.title}” from the library? Its charts stay, under Loose charts." }
                        div { class: "kf-lib-actions",
                            button {
                                class: "kf-button kf-button-danger",
                                onclick: {
                                    let (org, slug) = (org.clone(), song.slug.clone());
                                    move |_| {
                                        let (org, slug) = (org.clone(), slug.clone());
                                        spawn(async move {
                                            match library::delete_song(&org, &slug).await {
                                                Ok(()) => on_deleted.call(()),
                                                Err(error) => {
                                                    on_trouble.call(Some(format!("removing the song: {error}")));
                                                    removing.set(false);
                                                }
                                            }
                                        });
                                    }
                                },
                                "Remove song"
                            }
                            button { class: "kf-button", onclick: move |_| removing.set(false), "Keep it" }
                        }
                    } else {
                        button { class: "kf-lib-link kf-lib-link-danger", onclick: move |_| removing.set(true), "Remove this song…" }
                    }
                }
            }

            div { class: "kf-song-stage",
                match shown {
                    None => rsx! {
                        div { class: "kf-song-blank",
                            p { "This song has no chart yet." }
                            p { class: "kf-lib-quiet", "Write one with the form on the left — it opens in the editor, already attached to the song." }
                        }
                    },
                    Some(chart) => rsx! {
                        div { class: "kf-song-stage-bar",
                            div { class: "kf-song-stage-what",
                                span { class: "kf-song-stage-name", "{arrangement_name(&chart)}" }
                                if !chart.sections.is_empty() {
                                    span { class: "kf-sections", "{chart.sections.join(\" \")}" }
                                }
                            }
                            div { class: "kf-song-stage-actions",
                                if deleting() == Some(chart.slug.clone()) {
                                    span { class: "kf-lib-confirm",
                                        if chart.is_default && view.charts.len() > 1 {
                                            "Delete it? The oldest other arrangement becomes the main one."
                                        } else {
                                            "Delete this chart?"
                                        }
                                    }
                                    button {
                                        class: "kf-button kf-button-danger",
                                        onclick: {
                                            let (org, slug) = (org.clone(), chart.slug.clone());
                                            move |_| {
                                                let (org, slug) = (org.clone(), slug.clone());
                                                spawn(async move {
                                                    on_trouble.call(None);
                                                    if let Err(error) = library::delete_chart(&org, &slug).await {
                                                        on_trouble.call(Some(format!("deleting the chart: {error}")));
                                                    }
                                                    deleting.set(None);
                                                    on_changed.call(());
                                                });
                                            }
                                        },
                                        "Delete chart"
                                    }
                                    button { class: "kf-button", onclick: move |_| deleting.set(None), "Keep" }
                                } else {
                                    if !chart.is_default {
                                        button {
                                            class: "kf-button",
                                            title: "Open this one when somebody opens the song",
                                            onclick: {
                                                let (org, slug) = (org.clone(), chart.slug.clone());
                                                move |_| {
                                                    let (org, slug) = (org.clone(), slug.clone());
                                                    spawn(async move {
                                                        on_trouble.call(None);
                                                        if let Err(error) = library::make_main_chart(&org, &slug).await {
                                                            on_trouble.call(Some(format!("making it the main chart: {error}")));
                                                        }
                                                        on_changed.call(());
                                                    });
                                                }
                                            },
                                            "Make main"
                                        }
                                    }
                                    button {
                                        class: "kf-button",
                                        onclick: {
                                            let slug = chart.slug.clone();
                                            move |_| deleting.set(Some(slug.clone()))
                                        },
                                        "Delete"
                                    }
                                    Link {
                                        class: "kf-button kf-button-primary",
                                        to: Route::LibraryChart { org: org.clone(), slug: chart.slug.clone() },
                                        "Open in editor"
                                    }
                                }
                            }
                        }
                        div { class: "kf-song-paper",
                            match &*paper.read_unchecked() {
                                None => rsx! { div { class: "kf-song-paper-wait", "Engraving…" } },
                                Some(Err(error)) => rsx! { div { class: "kf-song-paper-wait", role: "alert", "{error}" } },
                                Some(Ok(None)) => rsx! {},
                                Some(Ok(Some(stored))) => rsx! {
                                    Chart { source: stored.source.clone(), shape: ChartShape::Page }
                                },
                            }
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A new arrangement never lands on a chart that is already there —
    /// that would overwrite the chart it was copied from.
    #[test]
    fn an_arrangement_gets_a_slug_of_its_own() {
        let taken = vec![
            "wonderwall".to_owned(),
            "wonderwall-acoustic-in-g".to_owned(),
        ];
        assert_eq!(
            arrangement_slug("wonderwall", "Live cut", &taken),
            "wonderwall-live-cut"
        );
        assert_eq!(
            arrangement_slug("wonderwall", " Acoustic in G! ", &taken),
            "wonderwall-acoustic-in-g-2"
        );
        assert_eq!(
            arrangement_slug("wonderwall", "", &taken),
            "wonderwall-arrangement"
        );
    }

    /// A song's first chart opens on the song: its title, credit and key.
    #[test]
    fn a_first_chart_starts_from_the_song() {
        let song = SongEntry {
            slug: "africa".to_owned(),
            title: "Africa".to_owned(),
            writers: vec!["Toto".to_owned()],
            key: Some("A".to_owned()),
            tags: Vec::new(),
            updated_at: None,
        };
        let source = starter_source(&song);
        assert!(source.starts_with("Africa - Toto\n"));
        assert!(source.contains("#A"));
        let draft = library::draft_from_source(&source);
        assert!(
            draft.title.contains("Africa"),
            "the chart names the song: {draft:?}"
        );
        assert_eq!(draft.key.as_deref(), Some("A"));
    }
}
