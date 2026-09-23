//! `/devices` — the site at phone and tablet sizes, side by side.
//!
//! The editor's layout is responsive, and the only honest way to check a
//! responsive layout is to give it the viewport it is responding to. Every
//! cheaper trick lies in a way that matters here:
//!
//! - Resizing the desktop window will not go below ~500px, so the phone
//!   breakpoint cannot be reached at all on a normal display.
//! - Scaling a screenshot down shows what a phone looks like at desktop
//!   width, which is the one thing nobody needs to know.
//! - Wrapping the editor in a narrow `div` changes nothing: `@media` asks
//!   the VIEWPORT how wide it is, not the box the component landed in.
//!
//! An `<iframe>` has its own viewport, so the app inside one lays out
//! exactly as it would on a device that size — same media queries, same
//! breakpoints, same `dvh`. Frames are same-origin, so the page can read
//! each one's width back and show it.
//!
//! This is a development view. It is not linked from the site's navigation;
//! it is a URL you type when you are working on the layout.

use dioxus::prelude::*;

use crate::routes::Shell;

/// A viewport worth checking, in CSS pixels.
struct Device {
    name: &'static str,
    width: u32,
    height: u32,
}

/// The sizes that actually decide the layout.
///
/// Not a catalogue of every handset — three phones that bracket the range
/// (the narrowest still sold, the common one, the largest), the tablet
/// width where the panes are still stacked, and the one just above the
/// 60rem breakpoint, which is the case most likely to be got wrong because
/// it is the first width where both panes appear again.
const DEVICES: &[Device] = &[
    Device {
        name: "iPhone SE",
        width: 375,
        height: 667,
    },
    Device {
        name: "iPhone 15",
        width: 393,
        height: 852,
    },
    Device {
        name: "iPhone 15 Pro Max",
        width: 430,
        height: 932,
    },
    Device {
        name: "iPad mini",
        width: 744,
        height: 1000,
    },
    Device {
        name: "Just past the breakpoint",
        width: 980,
        height: 800,
    },
];

/// The routes worth looking at in a small viewport.
const SCREENS: &[(&str, &str)] = &[
    ("Editor", "/editor"),
    ("Guide", "/guide/chords"),
    ("Home", "/"),
];

#[component]
pub fn Devices() -> Element {
    let mut screen = use_signal(|| SCREENS[0].1.to_string());

    rsx! {
        Shell {
            section { class: "kf-devices",
                header { class: "kf-devices-head",
                    h1 { "Device preview" }
                    p { class: "kf-note",
                        "Each frame is a real viewport of that width, so the layout "
                        "breaks exactly where it would on the device. Interact with "
                        "them directly."
                    }
                    div { class: "kf-devices-picker",
                        for (label, path) in SCREENS.iter() {
                            button {
                                key: "{path}",
                                class: if screen() == *path { "kf-button kf-button-on" } else { "kf-button" },
                                onclick: move |_| screen.set(path.to_string()),
                                "{label}"
                            }
                        }
                    }
                }

                div { class: "kf-devices-strip",
                    for device in DEVICES.iter() {
                        div { key: "{device.name}", class: "kf-device",
                            div { class: "kf-device-label",
                                strong { "{device.name}" }
                                span { class: "kf-note", " {device.width}×{device.height}" }
                            }
                            iframe {
                                class: "kf-device-frame",
                                // Keyed on the path so picking another
                                // screen reloads the frame rather than
                                // leaving the old one on screen.
                                key: "{device.name}-{screen()}",
                                src: "{screen()}",
                                width: "{device.width}",
                                height: "{device.height}",
                                title: "{device.name} preview",
                            }
                        }
                    }
                }
            }
        }
    }
}
