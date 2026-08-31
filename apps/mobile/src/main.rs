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
    // Telemetry before anything else: the OTLP exporter is built once at
    // process start, so it has to exist before the first span. Fully inert
    // unless the build baked an endpoint -- a dev build ships nothing.
    // Guards are leaked deliberately: `dioxus::launch` hands control to the
    // event loop and never returns, so dropping them is not a thing that
    // happens, and a Drop-based shutdown would just be dead code.
    #[cfg(not(target_arch = "wasm32"))]
    {
        use tracing_subscriber::layer::SubscriberExt as _;
        use tracing_subscriber::util::SubscriberInitExt as _;

        if let Some(guard) = architect_telemetry::init("keyflow") {
            std::mem::forget(guard);
        }
        let registry = tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .with(architect_telemetry::tracing_layer());
        match architect_telemetry::otel::init("keyflow") {
            Some((otel_guard, layers)) => {
                registry.with(layers).init();
                std::mem::forget(otel_guard);
            }
            None => registry.init(),
        }
    }

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: asset!("/assets/mobile.css") }
        Router::<Route> {}
    }
}
