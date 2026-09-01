//! The Keyflow editor pane.
//!
//! The real editor, with Keyflow wired in as a *language*: syntax
//! decorations, IDE diagnostics, and hover.
//!
//! The resolved-chord overlays — which write what `1 5 6m 4` means in the
//! chart's key over the top of it — are off here. They annotate every
//! chord at once, which is a lot of text laid across text someone is
//! trying to write, and the toggle for them was one of two nearly-empty
//! toolbar rows above the pane.
//!
//! Ported from the pre-split site's studio
//! (`apps/site/src/components/live_editor.rs`). The first version of the
//! workbench used a bare `<textarea>`, which meant none of the above —
//! and a guide chapter about chord syntax is considerably less useful
//! beside a box that cannot show you the syntax.

use dioxus::prelude::*;

use crate::prefs;
use editor::{Editor, EditorState, editor_view};
use editor_keyflow_lang::{HighlightTheme, highlight_css, keyflow_decorations, keyflow_hover};

/// An editable Keyflow buffer.
///
/// Owns its `EditorState` and mirrors the text out through `on_change`, so
/// the caller can drive a preview without knowing about editor internals.
#[component]
pub fn KeyflowEditor(
    /// Initial chart source.
    initial: String,
    /// Fired with the full text whenever it changes.
    on_change: EventHandler<String>,
    /// Optional aside for the pane header — "Opened from a link", say.
    #[props(default)]
    note: Option<String>,
) -> Element {
    let state = use_signal(|| EditorState::new(initial));

    let keymap = editor::standard_markdown_keymap();

    // Off unless someone asked for it, and remembered once they have.
    //
    // Vim mode is modal: the editor opens in Normal, where the letter
    // keys are commands rather than text. To someone who did not choose
    // it that is not a mode, it is a text box that ignores typing — so
    // it cannot be the default, and it has to be discoverable enough to
    // turn back off, hence the toggle in the pane header rather than a
    // key chord.
    let mut vim_on = use_signal(|| prefs::bool_or(prefs::VIM_MODE, false));
    let vim_state = use_signal(editor::editor_vim::VimState::new);
    // `None` is what actually disables it — the `Editor` takes an
    // `Option<Signal<VimState>>` and plain editing is the absent case.
    let vim = vim_on().then_some(vim_state);
    let slash = use_signal(|| None::<editor_view::slash::SlashState>);

    // Mirror the text out. `use_effect` and not the editor's transaction
    // sink, because the caller wants the resulting *text*, not the edits.
    use_effect(move || {
        on_change.call(state.read().doc.to_string());
    });

    let css = use_memo(|| highlight_css(&HighlightTheme::default_dark()));

    rsx! {
        document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
        style { dangerous_inner_html: "{css}" }

        div { class: "kf-code-editor",
            div { class: "kf-pane-head",
                span { class: "kf-pane-name", "Source" }
                if let Some(n) = note {
                    span { class: "kf-note", "{n}" }
                }
                span { class: "kf-pane-spacer" }
                button {
                    class: if vim_on() { "kf-button kf-button-on" } else { "kf-button" },
                    // The control says what it toggles, and its state
                    // says whether it is on — a button labelled "Vim: off"
                    // reads as a button that turns vim off.
                    "aria-pressed": if vim_on() { "true" } else { "false" },
                    title: "Modal editing. Esc for Normal mode, i to insert.",
                    onclick: move |_| {
                        let next = !vim_on();
                        vim_on.set(next);
                        prefs::set_bool(prefs::VIM_MODE, next);
                    },
                    "Vim"
                }
            }
            div { class: "kf-code-editor-pane",
                div { class: "kf-code-editor-frame",
                Editor {
                    state,
                    keymap: keymap.clone(),
                    decorations: editor_view::DecorationSource::ptr(keyflow_decorations),
                    hover: keyflow_hover as editor::HoverSource,
                    vim,
                    slash: Some(slash),
                }
                editor_view::slash::SlashMenu { state, slash }
                }
            }
        }
    }
}
