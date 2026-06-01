//! Keyflow web editor.
//!
//! Mounts the shared [`Editor`] (the standalone Editor repo's
//! contenteditable widget) with keyflow language support wired in:
//!
//! - [`keyflow_decorations`] paints syntax colors + live IDE-diagnostic
//!   squiggles as you type (a `DecorationSource` over `EditorState`).
//! - [`highlight_css`] supplies the `.kf-*` color rules those decorations
//!   reference.
//! - [`render_svg`] re-engraves the chart into the right-hand preview pane on
//!   every edit, via the CPU-only `svg` tier (wasm-friendly, no GPU).
//!
//! Web/wasm only — the contenteditable view doesn't run on
//! dioxus-native/Blitz yet. Run with:
//!
//! ```sh
//! dx serve --package web-editor --platform web
//! ```

use dioxus::prelude::*;
use editor::{Editor, EditorState, editor_view};
use editor_keyflow::render_svg;
use editor_keyflow_lang::{
    HighlightTheme, highlight_css, keyflow_decorations, keyflow_hover, overlays_enabled,
    toggle_overlays,
};

/// Layout-only styles for this example (the split panes + preview chrome).
/// Token colors come from [`highlight_css`], injected at runtime.
const STYLE: Asset = asset!("/assets/web-editor.css");

/// Seed chart — written in the Nashville number system in the key of C so the
/// resolved-chord overlays (1 → C, 5 → G, 6m → Am, …) are visible on open.
const SEED: &str = "Number System Demo\n\
                    4/4 120bpm #C\n\
                    \n\
                    VS\n\
                    1 | 5 | 6m | 4\n\
                    1 | 5 | 4 1 | 1\n\
                    \n\
                    CH\n\
                    4 | 5 | 1 | 6m\n\
                    4 | 5 | 1 | 1\n";

fn main() {
    #[cfg(target_arch = "wasm32")]
    tracing_wasm::set_as_global_default();
    tracing::info!("keyflow web-editor starting");
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // The whole editor state lives here; `<Editor>` reads it to render and
    // writes a fresh state on every input.
    let mut state = use_signal(|| EditorState::new(SEED.to_string()));

    // Resolved-chord overlay toggle. The decoration source reads a process
    // global; this signal mirrors it for the button label, and toggling nudges
    // `state` so the editor recomputes decorations.
    let mut overlays_on = use_signal(overlays_enabled);
    let mut flip_overlays = move |_| {
        overlays_on.set(toggle_overlays());
        state.with_mut(|_| {}); // mark dirty → editor re-runs the decoration source
    };

    // Standard editing keymap (Backspace/Delete/Tab/select-all + markdown
    // niceties that are harmless on a chart). Enter/typing go through the
    // view's beforeinput bridge.
    let keymap = editor::standard_markdown_keymap();

    // Vim + slash palette, same wiring the Editor playground uses.
    let vim = use_signal(editor::editor_vim::VimState::new);
    let slash = use_signal(|| None::<editor_view::slash::SlashState>);

    // Live engraved preview — re-renders whenever the document changes.
    let preview = use_memo(move || {
        let src = state.read().doc.to_string();
        render_svg(&src).unwrap_or_else(|e| format!("<pre class=\"kf-render-error\">{e}</pre>"))
    });

    // keyflow token colors + diagnostic squiggle rules. Static (default-dark
    // palette), but generated rather than vendored so it tracks the theme.
    let css = use_memo(|| highlight_css(&HighlightTheme::default_dark()));

    rsx! {
        document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
        document::Link { rel: "stylesheet", href: STYLE }
        // Inject the generated keyflow highlight stylesheet.
        style { dangerous_inner_html: "{css}" }

        div { class: "page",
            header { class: "bar",
                h1 { "Keyflow Editor" }
                p { class: "hint", "Type a chart on the left — colors, squiggles, resolved-chord overlays, and hover info. Engraved preview updates live." }
                button {
                    class: "toggle",
                    onclick: move |e| flip_overlays(e),
                    if overlays_on() { "Resolved overlays: on" } else { "Resolved overlays: off" }
                }
            }
            div { class: "split",
                section { class: "editor-pane",
                    div { class: "editor-frame",
                        Editor {
                            state,
                            keymap: keymap.clone(),
                            decorations: keyflow_decorations as editor_view::DecorationSource,
                            hover: keyflow_hover as editor::HoverSource,
                            vim: Some(vim),
                            slash: Some(slash),
                        }
                        editor_view::slash::SlashMenu { state, slash }
                    }
                }
                section { class: "preview-pane",
                    div { class: "kf-render", dangerous_inner_html: "{preview}" }
                }
            }
        }
    }
}
