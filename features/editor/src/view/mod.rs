//! The chart editor for Dioxus hosts — the `view` feature.
//!
//! [`KeyflowEditor`] is the source pane (the editor, Keyflow's
//! highlighting, diagnostics and hover, Vim, the command palette);
//! [`use_engraved_pages`] is the live preview's debounced engraving, with
//! the camera math beside it in [`crate::preview`]. Both are host-neutral:
//! no web_sys, no browser storage, inline `<style>` sheets only — the
//! keyflow site mounts them in a browser and Studio in a Blitz window.
//!
//! Pick the renderer with this crate's `native` (Blitz) or `web` feature.
//!
//! ```ignore
//! use dioxus::prelude::*;
//! use editor_keyflow::view::KeyflowEditor;
//!
//! #[component]
//! fn Charts() -> Element {
//!     let mut source = use_signal(|| "VS: | 1 4 | 5 6- |\n".to_string());
//!     rsx! {
//!         div { style: "height: 100%; min-height: 0;",
//!             KeyflowEditor {
//!                 initial: source.peek().clone(),
//!                 on_change: move |text| source.set(text),
//!             }
//!         }
//!     }
//! }
//! ```

mod editor;
mod pages;
mod palette;
mod style;

pub use editor::{KeyflowEditor, KeyflowEditorProps};
pub use pages::{DEBOUNCE_MS, use_engraved_pages};
pub use palette::palette_commands;
pub use style::PANE_CSS;

/// The editor layers the component is built on, re-exported so a host
/// names the same `EditorState`, `TransactionEvent` and
/// `DecorationSource` the props take without a dependency of its own.
pub use editor_view;
pub use editor_view::editor_state;
