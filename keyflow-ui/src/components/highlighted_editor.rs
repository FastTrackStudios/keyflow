//! Syntax-highlighted editor for Keyflow notation.
//!
//! A transparent textarea overlaid on highlighted code display,
//! providing real-time syntax highlighting as the user types.

use crate::prelude::*;
use dioxus_core::Task;
use keyflow_proto::highlighting::{Highlighter, Renderer, Theme};

/// Highlighted editor component.
///
/// Renders a textarea with a syntax-highlighted code display underneath.
/// The textarea is transparent so the highlighting shows through, and
/// scroll positions are synchronized between the two layers.
#[component]
pub fn HighlightedEditor(
    /// The current source text.
    source: String,
    /// Callback when source text changes.
    on_change: EventHandler<String>,
) -> Element {
    let theme = use_memo(Theme::default_dark);

    // Local text state for immediate textarea responsiveness.
    // This updates on every keystroke so the user sees their typing instantly,
    // while the debounced on_change only fires after 150ms of inactivity.
    let mut local_text = use_signal(|| source.clone());

    // Sync local text when source prop changes from outside (e.g. example loaded)
    let mut last_source = use_signal(|| source.clone());
    if *last_source.read() != source {
        local_text.set(source.clone());
        last_source.set(source.clone());
    }

    // Debounce task handle — cancelled and replaced on each keystroke
    let mut debounce_task: Signal<Option<Task>> = use_signal(|| None);

    // Use local_text for highlighting so it updates immediately on keystroke
    let highlighted_html = use_memo(move || {
        let source = local_text.read().clone();
        let theme = &*theme.read();
        let mut html = String::with_capacity(source.len() * 3);

        for line in source.lines() {
            let spans = Highlighter::highlight_line(line);
            html.push_str(&Renderer::to_html_inline(line, &spans, theme));
            html.push('\n');
        }

        // Ensure trailing newline for cursor positioning
        if source.ends_with('\n') || source.is_empty() {
            html.push('\n');
        }

        html
    });

    // Syntax theme foreground color for unhighlighted text
    let fg_color = {
        let theme = theme.read();
        theme.foreground.to_css()
    };

    let display_text = local_text.read().clone();

    rsx! {
        div {
            class: "relative flex-1 overflow-hidden",
            style: "font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace; font-size: 14px; line-height: 21px;",

            // Highlighted code display (underneath)
            pre {
                class: "absolute inset-0 overflow-auto pointer-events-none m-0 p-3 whitespace-pre bg-card",
                id: "highlight-display",
                style: "color: {fg_color}; tab-size: 4;",
                dangerous_inner_html: "{highlighted_html}"
            }

            // Transparent textarea (on top, receives input)
            textarea {
                class: "absolute inset-0 w-full h-full resize-none outline-none m-0 p-3 whitespace-pre",
                id: "editor-textarea",
                style: "background: transparent; color: transparent; caret-color: var(--foreground); tab-size: 4; font: inherit; line-height: inherit;",
                spellcheck: false,
                value: "{display_text}",
                oninput: move |evt| {
                    let value = evt.value().clone();
                    // Update local state immediately for responsive typing
                    local_text.set(value.clone());

                    // Cancel any pending debounce task
                    if let Some(prev) = *debounce_task.read() {
                        prev.cancel();
                    }

                    // Spawn a new debounced task — fires on_change after 150ms of inactivity
                    let task = spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                        on_change.call(value);
                    });
                    debounce_task.set(Some(task));
                },
                // Synchronize scroll position with highlight display
                onscroll: move |_| {
                    #[cfg(feature = "web")]
                    {
                        let js = r#"
                            (function() {
                                var ta = document.getElementById('editor-textarea');
                                var hl = document.getElementById('highlight-display');
                                if (ta && hl) {
                                    hl.scrollTop = ta.scrollTop;
                                    hl.scrollLeft = ta.scrollLeft;
                                }
                            })();
                        "#;
                        dioxus::prelude::document::eval(js);
                    }
                }
            }
        }
    }
}
