//! Keyflow's editor using the native Blitz input and IME route.
#![cfg(feature = "native")]

use dioxus::prelude::*;
use dioxus_test::{
    BlitzImeEvent, by_testid,
    matchers::{eq, inner_html},
    render,
};
use editor_keyflow::view::{KeyflowEditor, editor_state::EditorState};

#[component]
fn Harness() -> Element {
    let state = use_signal(|| EditorState::new("C"));
    let text = state.read().doc.to_string();
    let mut reported = use_signal(String::new);
    let reported_text = reported();
    rsx! {
        KeyflowEditor { initial: "C".to_string(), state, on_change: move |text| reported.set(text) }
        div { "data-testid": "source", "{text}" }
        div { "data-testid": "reported", "{reported_text}" }
    }
}

#[tokio::test]
async fn native_ime_commits_once_into_the_keyflow_source() {
    let tester = render(Harness).build();
    tester.query(".editor-root").immediately().unwrap().focus();
    assert_eq!(tester.ime_enabled(), Some(true));
    tester.ime(BlitzImeEvent::Preedit("ni".into(), Some((2, 2))));
    tester
        .query(".ed-preedit")
        .expect(inner_html(eq("ni".to_string())))
        .await
        .unwrap();
    tester.ime(BlitzImeEvent::Preedit(String::new(), None));
    tester.ime(BlitzImeEvent::Commit("你".into()));
    tester
        .query(by_testid("reported"))
        .expect(inner_html(eq("你C".to_string())))
        .await
        .unwrap();
    tester
        .query(by_testid("source"))
        .expect(inner_html(eq("你C".to_string())))
        .await
        .unwrap();
}
