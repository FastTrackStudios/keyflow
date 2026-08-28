//! Keyflow for iOS.
//!
//! Two halves. This binary is the app: a chart library, an editor, and a
//! preview of the chart as it is written. The other half is the keyboard —
//! an iOS app extension that gives Keyflow syntax its own system keyboard,
//! usable in *any* app. Its layout and suggestions live in [`keyboard`],
//! in Rust, so the keyboard cannot drift away from the language; the Swift
//! shell that draws it is in `ios/`.
//!
//! The app renders charts through the same `wasm-graphics` surface the
//! website uses rather than the desktop wgpu path — see `keyflow-ui`.

mod keyboard;
mod library;
mod views;

use dioxus::prelude::*;

/// App routes.
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    ChartList {},
    #[route("/chart/:id")]
    ChartEditor { id: String },
    #[route("/keyboard")]
    KeyboardPreview {},
}

use views::{ChartEditor, ChartList, KeyboardPreview};

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    tracing_subscriber::fmt::init();

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: asset!("/assets/mobile.css") }
        Router::<Route> {}
    }
}
