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

use crate::collab::{COLLAB_STYLE, ChartCollab};
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
    /// Controls for the pane header, to the left of the Vim toggle.
    ///
    /// A slot rather than a `save: bool`, because what belongs to a
    /// buffer is different in each screen that shows one: the editor
    /// saves to the library, the workbench is a scratchpad in a guide
    /// chapter and saves nothing. The editor pane should not know which
    /// of those it is inside.
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
    let own = use_signal(|| EditorState::new(initial));
    let state = state.unwrap_or(own);
    // The shared chart's transaction sink, built ONCE. Rebuilt per render,
    // every presence update (each caret move, ours included) handed the
    // editor a new callback prop mid-typing.
    let on_transaction = use_hook(|| {
        collab.map(|collab| {
            Callback::new(move |event: editor::editor_view::TransactionEvent| {
                collab.on_transaction(&event, &state.peek());
            })
        })
    });
    // Built once: the source is compared by identity, and `collab` is
    // fixed for the life of the screen.
    let decorations = use_hook(|| match collab {
        None => editor_view::DecorationSource::ptr(keyflow_decorations),
        Some(collab) => editor_view::DecorationSource::new(move |st: &EditorState| {
            let mut out = keyflow_decorations(st);
            out.extend(collab.carets(st));
            out
        }),
    });

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
    let palette = use_signal(|| None::<editor_view::palette::PaletteState>);

    // Mirror the text out. `use_effect` and not the editor's transaction
    // sink, because the caller wants the resulting *text*, not the edits.
    use_effect(move || {
        on_change.call(state.read().doc.to_string());
    });

    let css = use_memo(|| highlight_css(&HighlightTheme::default_dark()));

    rsx! {
        document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
        style { dangerous_inner_html: "{css}" }
        if collab.is_some() {
            style { dangerous_inner_html: COLLAB_STYLE }
        }

        div { class: "kf-code-editor",
            div { class: "kf-pane-head",
                span { class: "kf-pane-name", "Source" }
                if let Some(n) = note {
                    span { class: "kf-note", "{n}" }
                }
                span { class: "kf-pane-spacer" }
                if let Some(actions) = actions {
                    {actions}
                }
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
                    decorations: decorations.clone(),
                    hover: keyflow_hover as editor::HoverSource,
                    vim,
                    palette: Some(palette),
                    on_transaction: on_transaction,
                }
                editor_view::palette::CommandPalette { state, palette }
                }
            }
        }
    }
}
