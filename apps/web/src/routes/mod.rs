//! The site's screens.

mod editor;
mod guide_page;
mod home;

pub use editor::{Chart, Editor};
pub use guide_page::{GuideIndex, GuidePage};
pub use home::Home;

use dioxus::prelude::*;

use crate::Route;

/// Shared chrome: the header every screen sits under.
#[component]
pub fn Shell(children: Element) -> Element {
    rsx! {
        div { class: "kf-shell",
            header { class: "kf-header",
                Link { to: Route::Home {}, class: "kf-wordmark", "Keyflow" }
                nav { class: "kf-nav",
                    Link { to: Route::Editor {}, "Editor" }
                    Link { to: Route::GuideIndex {}, "Guide" }
                    a {
                        href: "https://github.com/FastTrackStudios/keyflow",
                        rel: "noreferrer",
                        "Source"
                    }
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
