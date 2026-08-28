//! keyflow.fasttrackstudio.app
//!
//! Three things: a landing page that shows what Keyflow is, a live editor
//! whose whole state travels in the URL, and the guide.
//!
//! The editor has no backend. A chart is compressed into the address bar
//! (see [`chart_url`]), so sharing one is sharing a link — no account, no
//! database, nothing to sign up for. Accounts come later, and when they do
//! they are for *keeping* charts, not for using the editor.

mod chart_url;
mod chart_view;
mod guide;
mod routes;

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
    #[route("/guide/:slug")]
    GuidePage { slug: String },
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

use routes::{Chart, Editor, GuideIndex, GuidePage, Home, NotFound};

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();
    }
    #[cfg(not(target_arch = "wasm32"))]
    tracing_subscriber::fmt::init();

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: asset!("/assets/site.css") }
        Router::<Route> {}
    }
}
