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
use editor::{editor_view, Editor, EditorState};
use editor_keyflow::{font_face_css, render_svg_live};
use editor_keyflow_lang::{
    highlight_css, keyflow_decorations, keyflow_hover, overlays_enabled, toggle_overlays,
    HighlightTheme,
};

/// Idle delay before re-engraving the preview. Typing within this window keeps
/// resetting the timer, so layout/serialization runs once you pause — never on
/// the keystroke itself. ~150ms reads as "instant" while fully unblocking the
/// main thread during a fast typing burst.
const PREVIEW_DEBOUNCE_MS: u32 = 150;

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

    // Live engraved preview — DEBOUNCED. The synchronous "render on every
    // keystroke" approach blocked the main thread (~16ms layout + a 4MB
    // font-embedded SVG rebuilt and re-inserted into the DOM each key). Now the
    // preview is a signal updated by a debounced task: each edit bumps a
    // generation and schedules a render `PREVIEW_DEBOUNCE_MS` later; a newer
    // edit supersedes it, so only the last (after you pause) actually engraves.
    // The render is also font-less (`render_svg_live`) — fonts are injected once
    // below — so each pass serializes ~38KB of glyphs, not ~4MB.
    let mut preview = use_signal(String::new);
    let mut preview_gen = use_signal(|| 0u64);
    use_effect(move || {
        let src = state.read().doc.to_string(); // subscribe to document edits
        let my_gen = preview_gen.peek().wrapping_add(1);
        preview_gen.set(my_gen);
        spawn(async move {
            // wasm-only dep (this app only runs in the browser); the cfg keeps
            // host builds of the workspace (clippy --all-targets) compiling.
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(PREVIEW_DEBOUNCE_MS).await;
            // A keystroke landed during the wait → a newer task owns the render.
            if *preview_gen.peek() != my_gen {
                return;
            }
            let svg = render_svg_live(&src)
                .unwrap_or_else(|e| format!("<pre class=\"kf-render-error\">{e}</pre>"));
            preview.set(svg);
        });
    });

    // keyflow token colors + diagnostic squiggle rules. Static (default-dark
    // palette), but generated rather than vendored so it tracks the theme.
    let css = use_memo(|| highlight_css(&HighlightTheme::default_dark()));

    // Engraving `@font-face` rules — injected ONCE (the bytes are multi-MB; the
    // browser parses and caches them a single time). `render_svg_live` then
    // emits font-less SVGs that reference these families by name.
    let font_css = use_memo(|| font_face_css().unwrap_or_default());

    rsx! {
        document::Link { rel: "stylesheet", href: editor::EDITOR_STYLE }
        document::Link { rel: "stylesheet", href: STYLE }
        // Inject the generated keyflow highlight stylesheet.
        style { dangerous_inner_html: "{css}" }
        // Engraving fonts, injected once so live previews stay font-less.
        style { dangerous_inner_html: "{font_css}" }

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
