//! The Keyflow editor pane.
//!
//! The real editor, with Keyflow wired in as a *language*: syntax
//! decorations, IDE diagnostics, and hover; a Vim toggle and the command
//! palette. The resolved-chord overlays are off — they annotate every
//! chord at once, which is a lot of text laid across text someone is
//! trying to write.
//!
//! Host-neutral: it reads no browser storage and touches no DOM, so it
//! mounts the same in the keyflow site and in a Blitz window. What a host
//! owns comes in through props — whether Vim is on (and where that is
//! remembered), extra decorations (collaborators' carets), a transaction
//! sink (a shared document), header controls.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use editor_view::editor_state::{EditorState, HoverSource, Keymap, commands};
use editor_view::palette::{CommandPalette, PaletteState};
use editor_view::{DecorationSource, EDITOR_CSS, Editor, TransactionEvent};

use super::style::PANE_CSS;
use crate::lang::{HighlightTheme, highlight_css, keyflow_decorations, keyflow_hover};

/// An editable Keyflow buffer.
///
/// Owns its `EditorState` unless given one, and mirrors the text out
/// through `on_change`, so the caller can drive a preview without knowing
/// about editor internals.
///
/// Mount it in a box with a height: the pane fills its parent
/// (`height: 100%`) and scrolls inside.
#[component]
pub fn KeyflowEditor(
    /// Initial chart source. Read once; `key` the component to start over.
    initial: String,
    /// Fired with the full text whenever it changes.
    on_change: EventHandler<String>,
    /// Optional aside for the pane header — "Opened from a link", say.
    #[props(default)]
    note: Option<String>,
    /// Controls for the pane header, to the left of the Vim toggle.
    ///
    /// A slot rather than a `save: bool`, because what belongs to a
    /// buffer is different in each screen that shows one. The pane should
    /// not know which of those it is inside.
    #[props(default)]
    actions: Option<Element>,
    /// The buffer, when the host owns it — a shared chart whose session
    /// writes remote edits into it. Absent, the editor owns its own,
    /// seeded from `initial`.
    #[props(default)]
    state: Option<Signal<EditorState>>,
    /// Whether Vim (modal) editing is on.
    ///
    /// With `on_vim`, this is controlled: the toggle reports the new value
    /// and the host passes it back here (and remembers it wherever it
    /// remembers things). Without `on_vim`, the pane keeps its own, seeded
    /// from this. Off by default: to someone who did not choose it, Normal
    /// mode is a text box that ignores typing.
    #[props(default)]
    vim: bool,
    /// Fired with the new value when the Vim toggle is pressed.
    #[props(default)]
    on_vim: Option<EventHandler<bool>>,
    /// Decorations drawn over Keyflow's own — other people's carets on a
    /// shared chart, say. Run on every decoration pass; signals read
    /// inside it re-run the pass when they change.
    #[props(default)]
    extra_decorations: Option<DecorationSource>,
    /// The editor's transaction sink: every edit and selection change,
    /// synchronously, before any effect runs — how a shared document
    /// stays in step with the buffer.
    #[props(default)]
    on_transaction: Option<EventHandler<TransactionEvent>>,
    /// Emit the editor's stylesheets — editor-view's `EDITOR_CSS` and the
    /// pane chrome ([`PANE_CSS`](super::PANE_CSS)) — as `<style>`
    /// elements. On by default; a host that ships both in its own sheet
    /// turns this off. The highlight colours are always emitted.
    #[props(default = true)]
    stylesheet: bool,
) -> Element {
    let own = use_signal(|| EditorState::new(initial));
    let state = state.unwrap_or(own);

    // The host's sink and decorations, read through a cell so the props
    // the editor itself sees are built ONCE. A new callback or source per
    // render handed the editor a new prop mid-typing on every presence
    // update; a cell is not a signal, so reading it subscribes to nothing.
    let sink = use_hook(|| Rc::new(RefCell::new(None::<EventHandler<TransactionEvent>>)));
    *sink.borrow_mut() = on_transaction;
    let extra = use_hook(|| Rc::new(RefCell::new(None::<DecorationSource>)));
    if *extra.borrow() != extra_decorations {
        *extra.borrow_mut() = extra_decorations;
    }
    let forward = use_hook(|| {
        let sink = sink.clone();
        Callback::new(move |event: TransactionEvent| {
            let host = *sink.borrow();
            if let Some(host) = host {
                host.call(event);
            }
        })
    });
    let on_transaction = on_transaction.map(|_| forward);
    let decorations = use_hook(|| {
        let extra = extra.clone();
        DecorationSource::new(move |st: &EditorState| {
            let mut out = keyflow_decorations(st);
            if let Some(more) = extra.borrow().as_ref() {
                out.extend(more.run(st));
            }
            out
        })
    });

    let keymap = use_hook(chart_keymap);

    let mut own_vim = use_signal(|| vim);
    let vim_on = if on_vim.is_some() { vim } else { own_vim() };
    let vim_state = use_signal(editor_vim::VimState::new);
    // `None` is what actually disables it — the `Editor` takes an
    // `Option<Signal<VimState>>` and plain editing is the absent case.
    let vim_signal = vim_on.then_some(vim_state);
    let palette = use_signal(|| None::<PaletteState>);

    // Mirror the text out. `use_effect` and not the transaction sink,
    // because the caller wants the resulting *text*, and a host-owned
    // buffer can change without a local transaction (a remote edit).
    use_effect(move || {
        on_change.call(state.read().doc.to_string());
    });

    let css = use_memo(|| highlight_css(&HighlightTheme::default_dark()));

    rsx! {
        if stylesheet {
            style { dangerous_inner_html: EDITOR_CSS }
            style { dangerous_inner_html: PANE_CSS }
        }
        style { dangerous_inner_html: "{css}" }

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
                    class: if vim_on { "kf-button kf-button-on" } else { "kf-button" },
                    // The control says what it toggles, and its state
                    // says whether it is on — a button labelled "Vim: off"
                    // reads as a button that turns vim off.
                    "aria-pressed": if vim_on { "true" } else { "false" },
                    title: "Modal editing. Esc for Normal mode, i to insert.",
                    onclick: move |_| {
                        let next = !vim_on;
                        match on_vim {
                            Some(on_vim) => on_vim.call(next),
                            None => own_vim.set(next),
                        }
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
                        hover: keyflow_hover as HoverSource,
                        vim: vim_signal,
                        palette: Some(palette),
                        on_transaction,
                    }
                    CommandPalette { state, palette }
                }
            }
        }
    }
}

/// The bindings a chart buffer gets: the editor's standard markdown
/// keymap (`editor::standard_markdown_keymap`), restated here because
/// that lives in the umbrella crate this one does not take.
fn chart_keymap() -> Keymap {
    Keymap::new()
        .with("Mod-a", |s: &_| commands::select_all(s))
        .with("Mod-b", |s: &_| commands::toggle_bold(s))
        .with("Mod-i", |s: &_| commands::toggle_italic(s))
        .with("Mod-k", |s: &_| commands::toggle_link(s))
        .with("Mod-Shift-k", |s: &_| {
            commands::add_block_id(s).map(|(t, _)| t)
        })
        .with("Mod-l", |s: &_| commands::cycle_list(s))
        .with("Mod-t", |s: &_| commands::toggle_task(s))
        .with("Mod-1", |s: &_| commands::set_heading(s, 1))
        .with("Mod-2", |s: &_| commands::set_heading(s, 2))
        .with("Mod-3", |s: &_| commands::set_heading(s, 3))
        .with("Mod-4", |s: &_| commands::set_heading(s, 4))
        .with("Mod-5", |s: &_| commands::set_heading(s, 5))
        .with("Mod-6", |s: &_| commands::set_heading(s, 6))
        .with("Mod-0", |s: &_| commands::set_heading(s, 0))
        .with("Mod-e", |s: &_| commands::toggle_reading_mode(s))
        .with("Tab", |s: &_| {
            commands::tab_list_indent(s, false).or_else(|| commands::indent_more(s))
        })
        .with("Shift-Tab", |s: &_| {
            commands::tab_list_indent(s, true).or_else(|| commands::indent_less(s))
        })
        .with("Backspace", |s: &_| commands::delete_backward(s))
        .with("Delete", |s: &_| commands::delete_forward(s))
}
