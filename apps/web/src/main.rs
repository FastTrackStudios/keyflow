//! keyflow.fasttrackstudio.app
//!
//! Three things: a landing page that shows what Keyflow is, a live editor
//! whose whole state travels in the URL, and the guide.
//!
//! The editor has no backend. A chart is compressed into the address bar
//! (see [`chart_url`]), so sharing one is sharing a link — no account, no
//! database, nothing to sign up for. Accounts come later, and when they do
//! they are for *keeping* charts, not for using the editor.

mod account_menu;
mod auth;
mod chart;
mod chart_gpu;
mod chart_preview;
mod chart_url;
mod guide;
mod highlight;
mod keyflow_editor;
mod notation;
mod prefs;
mod routes;
mod typewriter;

use dioxus::prelude::*;

/// Site routes.
///
/// `/c/:data` is the shareable chart: the editor, seeded from a chart
/// encoded in the path. `/editor` is the same screen with the default
/// example, and redirects to a `/c/…` URL as soon as anything is typed.
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/editor")]
    Editor {},
    #[route("/c/:data")]
    Chart { data: String },
    #[route("/guide")]
    GuideIndex {},
    // Before `/guide/:slug`, or "graph" would match as a page slug.
    #[route("/guide/graph")]
    GuideGraph {},
    #[route("/guide/:slug")]
    GuidePage { slug: String },
    // The workbench: the same chapter, with an editor and a live chart.
    #[route("/learn/:slug")]
    Workbench { slug: String },
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

use routes::{Chart, Editor, GuideGraph, GuideIndex, GuidePage, Home, NotFound, Workbench};

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        // INFO, not the default TRACE. Dioxus traces every template
        // creation, signal subscription and mount at TRACE, so the level
        // is a function of how often the page re-renders — and the hero
        // re-renders ~26 times a second, forever. `dx serve` forwards the
        // browser console into its own log: at TRACE that was 5.3 million
        // lines and 788 MB in ten minutes of sitting on the landing page,
        // with the console too busy to inspect.
        tracing_wasm::set_as_global_default_with_config(
            tracing_wasm::WASMLayerConfigBuilder::new()
                .set_max_level(tracing::Level::INFO)
                .build(),
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    tracing_subscriber::fmt::init();

    // The editor renders ```kf fences through a registry rather than a
    // dependency — `editor-state` sits below the notation domain and must
    // not know keyflow exists. Without this, charts in the guide render as
    // source instead of engraving.
    editor_state::fence_renderer::register_fence_renderer(
        "kf",
        std::sync::Arc::new(editor_keyflow::Fences),
    );

    dioxus::LaunchBuilder::new()
        .with_cfg(server_only! {
            dioxus::server::ServeConfig::builder().incremental(
                dioxus::server::IncrementalRendererConfig::new()
                    // `public` beside the executable is where the CLI
                    // also puts the web bundle, so the pre-rendered
                    // pages and the assets they reference land in one
                    // directory — and that directory is what deploys.
                    .static_dir(
                        std::env::current_exe()
                            .expect("the server knows its own path")
                            .parent()
                            .expect("an executable has a parent directory")
                            .join("public"),
                    )
                    // Emphatically false. The cache directory is shared
                    // with the wasm bundle and every asset; clearing it
                    // per render would delete the site around the pages
                    // being written into it.
                    .clear_cache(false),
            )
        })
        .launch(App);
}

/// The paths `dx build --ssg` should pre-render.
///
/// The CLI looks for a server function at exactly this endpoint, calls
/// it once, and requests every path it returns — which is what writes
/// them to disk as HTML.
///
/// Two sources, and the second is the point. `Route::static_routes()`
/// gives the routes with no parameters — `/`, `/editor`, `/guide`,
/// `/guide/graph`. It cannot give the guide's chapters, because
/// `/guide/:slug` is a *single* parameterised route and only the vault
/// knows the slugs; so the vault supplies them.
///
/// What is deliberately absent is as important. `/learn/:slug` — the
/// workbench — is a live editor whose whole content is what the reader
/// types, and `/c/:data` is a chart decoded out of the URL. Neither has
/// a meaningful pre-rendered form, and both stay ordinary client-side
/// routes. That is what makes this *partial* static generation.
#[cfg(feature = "server")]
#[server(endpoint = "static_routes")]
async fn static_routes() -> ServerFnResult<Vec<String>> {
    let mut routes: Vec<String> = Route::static_routes()
        .iter()
        .map(ToString::to_string)
        .collect();

    for route in guide::VAULT.routes(guide::BASE) {
        if !routes.contains(&route) {
            routes.push(route);
        }
    }

    Ok(routes)
}

#[component]
fn App() -> Element {
    // Installed above the router so the session survives navigation and
    // is resolved once, not per screen.
    auth::use_auth_provider();

    rsx! {
        // The UI face and its matching mono. A sans and a mono from the
        // same family is the point rather than a detail: this site is
        // about the relationship between the text someone types and the
        // music it becomes, and the source pane sits beside prose
        // everywhere on it. Siblings make that one system instead of a
        // code block bolted to a document.
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link {
            rel: "preconnect",
            href: "https://fonts.gstatic.com",
            crossorigin: "anonymous",
        }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Geist:wght@300..800&family=Geist+Mono:wght@400..600&display=swap",
        }

        // Order matters: Tailwind first (it carries the architect-ui design
        // tokens the imported components resolve against), then the site's
        // own sheet, which styles the chart surface and page chrome and
        // must win where the two overlap.
        document::Link { rel: "stylesheet", href: asset!("/assets/tailwind.css") }
        document::Link { rel: "stylesheet", href: asset!("/assets/site.css") }
        Router::<Route> {}
    }
}
