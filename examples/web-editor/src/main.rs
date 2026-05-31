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
use editor_keyflow_lang::{HighlightTheme, highlight_css, keyflow_decorations};

/// Layout-only styles for this example (the split panes + preview chrome).
/// Token colors come from [`highlight_css`], injected at runtime.
const STYLE: Asset = asset!("/assets/web-editor.css");

/// Seed chart — a clean lead sheet so the editor opens on something real.
const SEED: &str = "Blue Bossa\n\
                    4/4 140bpm #C\n\
                    \n\
                    VS {head}\n\
                    Cm7 | Fm7 | Dm7b5 | G7\n\
                    Cm7 | Ebm7 Ab7 | Dbmaj7 | Dm7b5 G7\n";

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
    let state = use_signal(|| EditorState::new(SEED.to_string()));

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
                p { class: "hint", "Type a chart on the left — colors, squiggles, and the engraved preview update live." }
            }
            div { class: "split",
                section { class: "editor-pane",
                    div { class: "editor-frame",
                        Editor {
                            state,
                            keymap: keymap.clone(),
                            decorations: keyflow_decorations as editor_view::DecorationSource,
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
