//! Smoke tests of the `view` components, rendered to HTML with
//! dioxus-ssr: the pane mounts, takes its props, and the preview hook
//! engraves.
#![cfg(feature = "view")]

use std::time::Duration;

use dioxus::dioxus_core::NoOpMutations;
use dioxus::prelude::*;
use editor_keyflow::view::{KeyflowEditor, PANE_CSS, use_engraved_pages};

const CHART: &str = "VS: | 1 4 | 5 6- |\n";

fn render(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn the_pane_mounts_with_its_header_and_styles() {
    fn app() -> Element {
        rsx! {
            KeyflowEditor {
                initial: CHART.to_string(),
                on_change: |_| {},
                note: "Opened from a link".to_string(),
                actions: rsx! { button { class: "host-action", "Save" } },
            }
        }
    }
    let html = render(app);
    for needle in [
        "kf-code-editor",
        "kf-code-editor-frame",
        "Source",
        "Opened from a link",
        "host-action",
        "Vim",
        "aria-pressed=\"false\"",
    ] {
        assert!(html.contains(needle), "missing {needle:?} in:\n{html}");
    }
    // The pane carries its own sheets by default — a Blitz host loads no
    // <link>ed stylesheet — and they are Blitz-safe.
    assert!(
        html.contains(".kf-code-editor-pane"),
        "the pane chrome is emitted"
    );
    assert!(!PANE_CSS.contains("position: fixed") && !PANE_CSS.contains("overflow: auto"));
}

#[test]
fn vim_and_the_stylesheet_are_the_hosts_to_set() {
    fn app() -> Element {
        rsx! {
            KeyflowEditor {
                initial: CHART.to_string(),
                on_change: |_| {},
                vim: true,
                on_vim: |_| {},
                stylesheet: false,
            }
        }
    }
    let html = render(app);
    assert!(html.contains("aria-pressed=\"true\""), "vim is on:\n{html}");
    assert!(html.contains("kf-button-on"));
    assert!(
        !html.contains(".kf-code-editor-pane {"),
        "stylesheet: false leaves the chrome to the host"
    );
}

#[tokio::test]
async fn the_preview_hook_engraves_after_its_debounce() {
    fn app() -> Element {
        let pages = use_engraved_pages(CHART.to_string());
        let pages = pages.read();
        let drawn = pages.iter().filter(|p| !p.svg.is_empty()).count();
        rsx! {
            div { "data-pages": "{pages.len()}", "data-drawn": "{drawn}" }
        }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    // Before the debounce: one blank sheet, never an empty preview.
    assert!(dioxus_ssr::render(&dom).contains("data-drawn=\"0\""));

    let mut html = String::new();
    for _ in 0..40 {
        let _ = tokio::time::timeout(Duration::from_millis(100), dom.wait_for_work()).await;
        dom.render_immediate(&mut NoOpMutations);
        html = dioxus_ssr::render(&dom);
        if html.contains("data-drawn=\"1\"") {
            break;
        }
    }
    assert!(
        html.contains("data-drawn=\"1\""),
        "the chart engraved: {html}"
    );
}
