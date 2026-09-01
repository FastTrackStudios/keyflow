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
    let vim = use_signal(editor::editor_vim::VimState::new);
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
            }
            div { class: "kf-code-editor-pane",
                div { class: "kf-code-editor-frame",
                Editor {
                    state,
                    keymap: keymap.clone(),
                    decorations: editor_view::DecorationSource::ptr(keyflow_decorations),
                    hover: keyflow_hover as editor::HoverSource,
                    vim: Some(vim),
                    slash: Some(slash),
                }
                editor_view::slash::SlashMenu { state, slash }
                }
            }
        }
    }
}
