//! The site's screens.

mod appendix_page;
mod editor;
mod graph;
mod guide_page;
mod home;
mod workbench;

pub use appendix_page::{AppendixIndex, AppendixPage};
pub use editor::{Chart, Editor};
pub use graph::GuideGraph;
pub use guide_page::{GuideIndex, GuidePage};
pub use home::Home;
pub use workbench::Workbench;

use dioxus::prelude::*;

use crate::Route;
use crate::account_menu::AccountMenu;

/// The mark beside the wordmark. The iOS app's icon, which is the only
/// drawn identity Keyflow has — one file, so the two cannot drift.
const ICON: Asset = asset!("/assets/icon.svg");

/// Shared chrome: the header every screen sits under.
#[component]
pub fn Shell(children: Element) -> Element {
    rsx! {
        div { class: "kf-shell",
            // Above the header, not inside it: the state of the project
            // is not navigation, and someone arriving mid-page should
            // still meet it before they meet a syntax they might build
            // on.
            div { class: "kf-alpha-banner", role: "status",
                // The release tag, from `build.rs` — the workspace
                // version lags the tags, so compiling that in would show
                // a number nobody bumped.
                strong { "Keyflow {env!(\"KEYFLOW_VERSION\")}" }
                " — Keyflow is in Early Alpha, many things are currently broken but are being worked on."
            }
            header { class: "kf-header",
                Link { to: Route::Home {}, class: "kf-wordmark",
                    img { class: "kf-wordmark-icon", src: ICON, alt: "" }
                    span { "Keyflow" }
                }
                nav { class: "kf-nav",
                    Link { to: Route::Editor {}, "Editor" }
                    Link { to: Route::GuideIndex {}, "Guide" }
                    Link { to: Route::AppendixIndex {}, "Appendix" }
                    a {
                        href: "https://github.com/FastTrackStudios/keyflow",
                        rel: "noreferrer",
                        "Source"
                    }
                    AccountMenu {}
                }
            }
            main { class: "kf-main", {children} }
        }
    }
}

/// Anything the router could not match.
#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    rsx! {
        Shell {
            section { class: "kf-prose",
                h1 { "Not found" }
                p { "There is no page at /{segments.join(\"/\")}." }
                Link { to: Route::GuideIndex {}, "Read the guide" }
            }
        }
    }
}
