//! The chart library — the two screens an account is actually for.
//!
//! [`SaveToLibrary`] is a button in the editor's pane header, and
//! [`Library`] is `/library`, the list of everything kept. Between them
//! they are the entire user-facing surface of [`crate::library`], and
//! both hold to the same rule the account itself does: **nothing here
//! gates the editor**. Signed out, the button is an invitation and the
//! screen is a sentence explaining what an account would give you. The
//! chart in the URL keeps working either way.
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
//!   carry the chart tools yet and "your library is not on this server
//!   yet" is not a failure anyone can act on. See
//!   [`crate::library::LibraryError::Unsupported`].
//!
//! # Which org
//!
//! Task auto-provisions a personal org for every account, so the common
//! case is exactly one and nobody is asked anything. Someone in several
//! is asked once, and the answer is remembered
//! ([`crate::library::ORG_KEY`]). The decision itself is
//! [`crate::library::choose_org`], which is pure and tested; what is
//! here is only the panel that shows it.
//!
//! # Opening a chart puts it back in the URL
//!
//! "Open" reads the chart in full and navigates to `/c/:data` — the
//! same shareable route a link produces. So an opened chart behaves
//! exactly like every other chart on the site: shareable, bookmarkable,
//! and editable with no further round trips. Saving it again derives the
//! same slug from the same title, which makes that a new *version*
//! rather than a second chart.

use dioxus::prelude::*;

use crate::Route;
use crate::auth::{AuthState, use_auth};
use crate::chart_url;
use crate::library::{self, ChartEntry, LibraryError, Org, OrgTarget, StoredChart};
use crate::routes::Shell;

// ── Saving ───────────────────────────────────────────────────────────

/// What the save control is currently saying.
#[derive(Clone, Debug, PartialEq)]
enum SaveState {
    /// Nothing to say — just the button.
    Idle,
    /// Signed out, and the person asked to save anyway.
    Invited,
    Saving,
    /// Saved, and which chart it became.
    Saved(String),
    /// The account is in several orgs and has not picked one.
    Pick(Vec<Org>),
    Failed(String),
}

/// "Save" for the chart currently in the editor.
///
/// Takes the source rather than reading a global buffer, for the same
/// reason [`crate::routes::editor`] keeps the chart in a component
/// signal: the chart belongs to the screen showing it.
#[component]
pub fn SaveToLibrary(source: String) -> Element {
    let mut auth = use_auth();
    let mut state = use_signal(|| SaveState::Idle);

    let signed_in = matches!((auth.state)(), AuthState::SignedIn(_));
    let showing = state();
    // Read out before the match below consumes `showing`: the button's
    // label and its disabled-ness are facts about the state, and the
    // panel is a rendering of it.
    let saving = matches!(showing, SaveState::Saving);
    let inviting = matches!(showing, SaveState::Invited);
    let label = match showing {
        SaveState::Saving => "Saving…",
        SaveState::Saved(_) => "Saved",
        _ => "Save",
    };

    // The chart, encoded as the route that carries it. Two jobs: it is
    // where a sign-in redirect must come back to (the editor's URL does
    // not track typing, so parking the current path would park an empty
    // editor), and it is what makes "sign in to save" safe to click
    // mid-chart.
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

                // Signed out. An invitation, and an honest sentence
                // about what is and is not at stake — the chart is in
                // the link either way, and saying so is what makes
                // leaving for the issuer a safe-looking thing to do.
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

                // Several workspaces, and no answer remembered. Asked
                // once; the choice is remembered on the way through.
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

// ── The library screen ───────────────────────────────────────────────

/// What `/library` found.
#[derive(Clone, Debug, PartialEq)]
enum Shelf {
    /// The account is in several orgs and has not picked one.
    Pick(Vec<Org>),
    Charts {
        /// The org these came from.
        org: String,
        /// Everywhere else they could have come from, for the switcher.
        elsewhere: Vec<Org>,
        charts: Vec<ChartEntry>,
    },
}

/// `/library` — everything this account has kept.
#[component]
pub fn Library() -> Element {
    let mut auth = use_auth();
    let mut picked = use_signal(|| None::<String>);
    let mut reload = use_signal(|| 0_u32);
    let mut opening = use_signal(|| None::<String>);
    let mut trouble = use_signal(|| None::<String>);
    let navigator = use_navigator();

    let state = (auth.state)();
    let shelf = use_resource(move || {
        // Read both so the listing re-runs when the org changes and
        // when something asks for a refresh — a save from the editor,
        // or a delete from this screen.
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

    let open = move |slug: String, org: Option<String>| {
        spawn(async move {
            opening.set(Some(slug.clone()));
            trouble.set(None);
            match library::read_chart(&slug, org.as_deref()).await {
                Ok(StoredChart { source, .. }) => {
                    // Straight back into the URL the rest of the site
                    // speaks. See the module docs.
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
                h1 { "Your charts" }

                if let Some(message) = trouble() {
                    p { class: "kf-account-error", role: "alert", "{message}" }
                }

                match (&state, &*shelf.read_unchecked()) {
                    // Before the stored session has resolved. Nothing,
                    // rather than a flash of "sign in" at someone who
                    // turns out to be signed in — the same reason
                    // `AccountMenu` draws nothing while loading.
                    (AuthState::Loading, _) | (_, None) => rsx! {
                        p { class: "kf-note", "One moment…" }
                    },

                    (_, Some(Err(LibraryError::SignedOut))) => rsx! {
                        p {
                            "A FastTrackStudio account keeps your charts, so you can open them
                             again from any browser. The editor does not need one — a chart
                             lives in its link — but a link is not a shelf."
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
                        p { "You are in more than one workspace. Which one holds your charts?" }
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

                    (_, Some(Ok(Shelf::Charts { org, elsewhere, charts }))) => {
                        let org = org.clone();
                        let charts = charts.clone();
                        let elsewhere = elsewhere.clone();
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

                            if charts.is_empty() {
                                p {
                                    "Nothing here yet. Write a chart and press Save above it —
                                     it will be waiting the next time you sign in."
                                }
                                Link { class: "kf-account-link", to: Route::Editor {},
                                    "Open the editor"
                                }
                            } else {
                                ul { class: "kf-library-list",
                                    for chart in charts {
                                        LibraryRow {
                                            key: "{chart.slug}",
                                            chart: chart.clone(),
                                            org: org.clone(),
                                            busy: opening() == Some(chart.slug.clone()),
                                            on_open: {
                                                let slug = chart.slug.clone();
                                                let org = org.clone();
                                                move |()| open(slug.clone(), Some(org.clone()))
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

/// One chart on the shelf.
///
/// Delete is two clicks, not one and not a browser `confirm()`. A chart
/// is somebody's work and the row is small; one misplaced click should
/// not be able to remove it, and a modal dialog for a list item is more
/// interruption than the decision deserves.
#[component]
fn LibraryRow(
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
        chart.notation.clone(),
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
                                    match library::delete_chart(&slug, Some(&org)).await {
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

/// Resolve the org and fetch its charts.
///
/// `chosen` is a pick made in this session, which beats the remembered
/// one — someone using the switcher is answering the question again.
async fn shelf_for(chosen: Option<String>) -> Result<Shelf, LibraryError> {
    let orgs = library::my_orgs().await?;
    let remembered = chosen.or_else(|| crate::prefs::string(library::ORG_KEY));
    match library::choose_org(&orgs, remembered.as_deref()) {
        OrgTarget::None => Err(LibraryError::NoOrg),
        OrgTarget::Choose(orgs) => Ok(Shelf::Pick(orgs)),
        OrgTarget::One(slug) => Ok(Shelf::Charts {
            charts: library::list_charts(Some(&slug)).await?,
            elsewhere: orgs,
            org: slug,
        }),
    }
}
