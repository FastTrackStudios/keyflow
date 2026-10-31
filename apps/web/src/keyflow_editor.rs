//! The Keyflow editor pane, as the site mounts it.
//!
//! The pane itself is `editor_keyflow::view::KeyflowEditor` — the one
//! Studio mounts too, so the browser and the app edit charts with the
//! same component. What this wrapper adds is what belongs to the SITE:
//!
//! - **Vim, remembered in `localStorage`** ([`crate::prefs`]). The pane
//!   only takes `vim` and reports toggles; where a choice is kept is the
//!   host's business.
//! - **A shared library chart** ([`crate::collab`]): other people's
//!   carets as extra decorations, and the shared document fed from the
//!   editor's transaction sink.
//! - **The site's stylesheets**: `editor.css` as a linked asset and the
//!   pane chrome from `site.css`, so the component's own `<style>` copies
//!   are turned off.

use dioxus::prelude::*;

use crate::collab::{COLLAB_STYLE, ChartCollab};
use crate::prefs;
use editor::{EditorState, TransactionEvent, editor_view};

/// An editable Keyflow buffer.
///
/// Owns its `EditorState` unless given one, and mirrors the text out
/// through `on_change`, so the caller can drive a preview without knowing
/// about editor internals.
#[component]
pub fn KeyflowEditor(
    /// Initial chart source.
    initial: String,
    /// Fired with the full text whenever it changes.
    on_change: EventHandler<String>,
    /// Optional aside for the pane header — "Opened from a link", say.
    #[props(default)]
    note: Option<String>,
    /// Controls for the pane header, to the left of the Vim toggle.
    ///
    /// A slot rather than a `save: bool`, because what belongs to a
    /// buffer is different in each screen that shows one: the editor
    /// saves to the library, the workbench is a scratchpad in a guide
    /// chapter and saves nothing.
    #[props(default)]
    actions: Option<Element>,
    /// The buffer, when the screen owns it — a shared library chart,
    /// whose collaborative session writes remote edits into it. Absent,
    /// the editor owns its own, seeded from `initial`.
    #[props(default)]
    state: Option<Signal<EditorState>>,
    /// The shared chart this buffer is one copy of: other people's
    /// carets are drawn from it.
    #[props(default)]
    collab: Option<ChartCollab>,
) -> Element {
    // Owned here rather than in the pane, because the shared chart's sink
    // reads the buffer after each transaction.
    let own = use_signal(|| EditorState::new(initial));
    let state = state.unwrap_or(own);

    // Off unless someone asked for it, and remembered once they have.
    let mut vim = use_signal(|| prefs::bool_or(prefs::VIM_MODE, false));

    // Both built ONCE: `collab` is fixed for the life of the screen, and
    // a new sink per render handed the editor a new prop on every
    // presence update (each caret move, ours included).
    let carets = use_hook(|| {
        collab.map(|collab| {
            editor_view::DecorationSource::new(move |st: &EditorState| collab.carets(st))
        })
    });
    let on_transaction = use_hook(|| {
        collab.map(|collab| {
            EventHandler::new(move |event: TransactionEvent| {
                collab.on_transaction(&event, &state.peek());
            })
        })
    });

    rsx! {
        document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
        if collab.is_some() {
            style { dangerous_inner_html: COLLAB_STYLE }
        }
        editor_keyflow::view::KeyflowEditor {
            // The buffer is always ours (above), so the pane never seeds
            // one of its own.
            initial: String::new(),
            on_change,
            note,
            actions,
            state: Some(state),
            vim: vim(),
            on_vim: move |next| {
                vim.set(next);
                prefs::set_bool(prefs::VIM_MODE, next);
            },
            extra_decorations: carets.clone(),
            on_transaction,
            // site.css carries the pane chrome, and editor.css is linked.
            stylesheet: false,
        }
    }
}
