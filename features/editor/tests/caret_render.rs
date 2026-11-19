//! Keyflow's source editor with the lab's caret CSS, rendered in Blitz, to see
//! whether the caret paints. Prints what the harness finds and writes a PNG.
#![cfg(feature = "native")]

use dioxus::prelude::*;
use dioxus_test::{by_testid, render};
use editor_keyflow::view::{KeyflowEditor, PANE_CSS, editor_view::EDITOR_CSS};

/// The lab's own tweaks to the editor's chrome and caret, copied from
/// `LAB_OVERRIDES` in `apps/lab/ui/src/keyflow.rs`.
const LAB_OVERRIDES: &str = ".kf-pane-head { display: none; } .ed-native-caret, .ed-native-caret-end { display: inline-block; width: 2px; height: 1.1em; margin: 0; border: none; background: #ff0000; vertical-align: text-bottom; } .ed-native-caret-end { width: 2px; }";

const PNG_PATH: &str = "/run/media/Development/integration/logs/caret-isolated.png";

#[component]
fn CaretHarness() -> Element {
    let css = format!("{EDITOR_CSS}\n{PANE_CSS}\n{LAB_OVERRIDES}");
    // `document::Style` does not reach the harness DOM (only the editor's own
    // highlight `<style>` does), so the sheets go in as the editor emits its
    // own: a `<style>` with `dangerous_inner_html`.
    rsx! {
        style { dangerous_inner_html: "{css}" }
        div {
            "data-testid": "host",
            style: "height: 300px;",
            KeyflowEditor {
                initial: "120bpm 4/4 #C\n\nvs\nC F G C\n".to_string(),
                stylesheet: false,
                on_change: move |_| {},
            }
        }
    }
}

#[tokio::test]
async fn caret_renders_in_isolated_editor() {
    let tester = render(CaretHarness).build();
    tester.query(".editor-root").immediately().unwrap().focus();
    // The editor writes its focus signal and the caret decoration on its own
    // tick. Let that run, then lay out again so the caret's box is measured.
    let _ = tester.pump().await;
    tester.relayout();

    // Whether the sheets reached the document at all: the lab's caret rules
    // only do anything if one of these `<style>` elements carries them.
    let styles = tester.query_all("style").immediately();
    eprintln!("style.count = {}", styles.len());
    for (i, style) in styles.iter().enumerate() {
        let html = style.outer_html();
        eprintln!(
            "style[{i}].len = {} mentions_pane_head = {} mentions_caret = {}",
            html.len(),
            html.contains(".kf-pane-head"),
            html.contains("ed-native-caret"),
        );
    }

    let host = tester.query(by_testid("host")).immediately().unwrap();
    let host_html = host.outer_html();
    eprintln!("caret.html_mentions_ed_native_caret = {}", host_html.matches("ed-native-caret").count());

    let carets = tester
        .query_all(".ed-native-caret, .ed-native-caret-end")
        .immediately();
    eprintln!("caret.query_matches = {}", carets.len());
    let caret = carets.first();
    eprintln!("caret.exists = {}", caret.is_some());
    if let Some(caret) = caret {
        let (width, height) = caret.size();
        let center = caret.center();
        eprintln!("caret.class = {:?}", caret.attribute("class"));
        eprintln!("caret.size = {width} x {height}");
        eprintln!("caret.center.client = ({}, {})", center.client().x, center.client().y);
        eprintln!("caret.outer_html = {}", caret.outer_html());
    }

    tester.render_png(PNG_PATH);
    eprintln!("caret.png = {PNG_PATH}");

    let caret = caret.expect("no .ed-native-caret or .ed-native-caret-end element in the DOM");
    assert!(caret.size().0 > 0.0, "caret has zero width");
}
