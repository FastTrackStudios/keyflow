//! The app's screens.
//!
//! Phone-shaped: a list, a detail, and one settings-ish screen. Nothing
//! here couples to a DAW or a session — the phone app is for *writing*
//! charts, and the machinery that plays them lives in the desktop app.

use dioxus::prelude::*;

use crate::Route;
use crate::keyboard::{Action, KeyboardLayout, PlaneId};
use crate::library::{Entry, Library, title_of};

/// The chart library, most-recently-edited first.
#[component]
pub fn ChartList() -> Element {
    let library = use_library();

    rsx! {
        div { class: "kf-screen",
            header { class: "kf-topbar",
                h1 { "Charts" }
                Link { to: Route::KeyboardPreview {}, class: "kf-icon-link", "Keyboard" }
            }
            if library.read().entries.is_empty() {
                section { class: "kf-empty",
                    p { "No charts yet." }
                    p { class: "kf-note",
                        "Charts you write here sync to the Keyflow keyboard, so you can "
                        "use them from any app."
                    }
                }
            } else {
                ul { class: "kf-chart-list",
                    for entry in library.read().entries.iter() {
                        li { key: "{entry.id}",
                            Link { to: Route::ChartEditor { id: entry.id.clone() },
                                span { class: "kf-chart-title", "{entry.title}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One chart: source above, engraved preview below.
#[component]
pub fn ChartEditor(id: String) -> Element {
    let mut library = use_library();
    let mut source = use_signal(|| {
        library
            .read()
            .get(&id)
            .map_or_else(String::new, |e| e.source.clone())
    });

    rsx! {
        div { class: "kf-screen",
            header { class: "kf-topbar",
                Link { to: Route::ChartList {}, class: "kf-icon-link", "Charts" }
                h1 { "{title_of(&source.read())}" }
            }
            textarea {
                class: "kf-source-input",
                spellcheck: false,
                value: "{source}",
                oninput: {
                    let id = id.clone();
                    move |e: FormEvent| {
                        let text = e.value();
                        source.set(text.clone());
                        library.write().upsert(Entry::new(id.clone(), text, now_secs()));
                    }
                },
            }
        }
    }
}

/// A look at the Keyflow keyboard from inside the app.
///
/// The real keyboard is a system-wide extension the user enables in
/// Settings; this screen exists so they can see what they are enabling, and
/// so the layout can be tried without leaving the app.
#[component]
pub fn KeyboardPreview() -> Element {
    let mut plane = use_signal(|| PlaneId::Chords);
    let mut typed = use_signal(String::new);

    rsx! {
        div { class: "kf-screen",
            header { class: "kf-topbar",
                Link { to: Route::ChartList {}, class: "kf-icon-link", "Charts" }
                h1 { "Keyboard" }
            }
            pre { class: "kf-keyboard-output", "{typed}" }

            nav { class: "kf-plane-tabs",
                for id in PlaneId::ALL.iter().copied() {
                    button {
                        class: if plane() == id { "kf-tab kf-tab-current" } else { "kf-tab" },
                        onclick: move |_| plane.set(id),
                        "{id.title()}"
                    }
                }
            }

            div { class: "kf-keyboard",
                for (r, row) in KeyboardLayout::plane(plane()).rows.iter().enumerate() {
                    div { class: "kf-key-row", key: "{plane()}-{r}",
                        for key in row.iter() {
                            button {
                                class: "kf-key",
                                title: "{key.hint}",
                                onclick: {
                                    let action = key.action.clone();
                                    move |_| apply(&action, &mut typed, &mut plane)
                                },
                                "{key.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Apply a key press to the buffer.
///
/// The same set of effects the Swift extension performs against
/// `UITextDocumentProxy`; keeping it here means the preview and the real
/// keyboard cannot disagree about what a key does.
fn apply(action: &Action, buffer: &mut Signal<String>, plane: &mut Signal<PlaneId>) {
    match action {
        Action::Insert(text) => buffer.write().push_str(text),
        Action::InsertLine(text) => {
            let mut b = buffer.write();
            b.push_str(text);
            b.push('\n');
        }
        Action::Backspace => {
            buffer.write().pop();
        }
        Action::Plane(id) => plane.set(*id),
    }
}

/// The chart library, loaded once and shared by every screen.
fn use_library() -> Signal<Library> {
    use_context_provider(|| Signal::new(load_library()))
}

fn load_library() -> Library {
    match Library::load(&library_path()) {
        Ok(lib) => lib,
        Err(e) => {
            // Starting empty would look like the charts were deleted. Say so
            // and start empty anyway — a phone app that refuses to open is
            // worse than one that reports a problem.
            tracing::error!("{e}");
            Library::default()
        }
    }
}

/// Where the library file lives.
///
/// On iOS this must be the App Group container so the keyboard extension
/// can read it; the extension runs in its own sandbox and cannot see the
/// app's documents directory. See `ios/README.md`.
fn library_path() -> std::path::PathBuf {
    std::env::var_os("KEYFLOW_LIBRARY_DIR")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from)
        .join("library.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
